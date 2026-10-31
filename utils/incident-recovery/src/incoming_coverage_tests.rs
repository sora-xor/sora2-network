use super::*;
use common::recovery::{
    invariants::{IssuanceInvariant, StoredAmount, TerminalAudit},
    AccountReplacement, GuardedStorageEdit, RecoveryAuditChunk, RecoveryChunk, StorageWitness,
};
use eth_bridge::requests::{AssetKind, IncomingTransfer};
use framenode_runtime::incident_recovery_incoming::{
    IncomingRequestExpectation, NetworkAttackerRegistry,
};
use scale_info::{meta_type, Registry};

// Published EIP-155 example is only a deterministic signed-transaction vector.
// This fixture does not claim that its transaction emitted a bridge Deposit.
const SOURCE_RAW: &str = "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";

fn edit_source_operation(f: &mut Fixture, edit: impl FnOnce(&mut IncomingOperation)) {
    let RecoveryAction::Incoming(bytes) = &mut f.work[1].action else {
        panic!("source fixture incoming slot")
    };
    let mut operation: IncomingOperation = decode(bytes).unwrap();
    edit(&mut operation);
    *bytes = operation.encode();
}

fn authenticated_source_fixture() -> Fixture {
    let mut f = Fixture::new();
    f.incoming(false);
    f.external(Obligation::Admit, true);
    f.decision(Action::Process, true);
    let raw = hex::decode(SOURCE_RAW).unwrap();
    let tx_hash = H256(sp_io::hashing::keccak_256(&raw));
    edit_source_operation(&mut f, |operation| {
        let IncomingChannel::Hashi {
            request, approvals, ..
        } = &operation.channel
        else {
            unreachable!()
        };
        let IncomingRequest::Transfer(mut t) = decode::<IncomingRequest<Runtime>>(request).unwrap()
        else {
            unreachable!()
        };
        t.from = bridge_types::H160::zero();
        t.tx_hash = tx_hash;
        operation.channel = IncomingChannel::HashiAuthenticatedSource {
            network: 0,
            request: IncomingRequest::Transfer(t).encode(),
            raw_transaction: raw,
            expected_chain_id: Some(bridge_types::U256::one()),
            approvals: approvals.clone(),
        };
    });
    f.external.deliveries[0].message_id = hx(tx_hash.as_bytes());
    f.review.decisions[0].message_id = hx(tx_hash.as_bytes());
    f
}

#[test]
fn authenticated_hashi_source_coverage_retains_native_identity_and_beneficiary() {
    let mut f = authenticated_source_fixture();
    let before = f.work[1].encode();
    let report = f.verify().unwrap();
    assert_eq!(f.work[1].encode(), before);
    assert_eq!(report.work_deliveries, 1);
    assert_eq!(
        report.deliveries[0].message_id,
        f.external.deliveries[0].message_id
    );
    assert_eq!(
        report.deliveries[0].original_beneficiary,
        Some(hx(&[1; 32]))
    );
    assert_eq!(
        report.deliveries[0].effective_beneficiary,
        Some(hx(&[2; 32]))
    );
    assert!(
        !report.authentication_verified
            && !report.external_finality_verified
            && !report.external_archive_completeness_verified
            && !report.exact_artifact_execution_verified
            && !report.production_ready
    );
}

#[test]
fn authenticated_hashi_source_coverage_uses_actor_only_for_exclusion() {
    let mut f = authenticated_source_fixture();
    edit_source_operation(&mut f, |operation| {
        let IncomingChannel::HashiAuthenticatedSource {
            request,
            raw_transaction,
            expected_chain_id,
            ..
        } = &operation.channel
        else {
            unreachable!()
        };
        let actor = authenticated_hashi_source(request, raw_transaction, *expected_chain_id)
            .unwrap()
            .signer;
        operation.registries[0]
            .accounts
            .push(GenericAccount::EVM(actor));
    });
    assert!(f.verify().unwrap_err().contains("admission, exclusion"));
    f.review.decisions[0].action = Action::Exclude;
    let report = f.verify().unwrap();
    assert_eq!(
        report.deliveries[0].original_beneficiary,
        Some(hx(&[1; 32]))
    );
    assert_eq!(
        report.deliveries[0].effective_beneficiary,
        Some(hx(&[2; 32]))
    );
    f.review.decisions[0].effective_beneficiary = Some(hx(&[3; 32]));
    assert!(f
        .verify()
        .unwrap_err()
        .contains("effective beneficiary mismatch"));
}

#[test]
fn authenticated_hashi_source_coverage_rejects_source_domain_and_original_payload_changes() {
    for mutation in 0..7 {
        let mut f = authenticated_source_fixture();
        edit_source_operation(&mut f, |operation| {
            let IncomingChannel::HashiAuthenticatedSource {
                network,
                request,
                raw_transaction,
                expected_chain_id,
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let IncomingRequest::Transfer(mut t) =
                decode::<IncomingRequest<Runtime>>(request).unwrap()
            else {
                unreachable!()
            };
            match mutation {
                0 => raw_transaction[10] ^= 1,
                1 => *expected_chain_id = Some(bridge_types::U256::from(2u8)),
                2 => *expected_chain_id = None,
                3 => *network = 1,
                4 => t.from = bridge_types::H160::repeat_byte(7),
                5 => t.amount = 0,
                6 => t.to = AccountId32::new([3; 32]),
                _ => unreachable!(),
            }
            *request = IncomingRequest::Transfer(t).encode();
        });
        assert!(f.verify().is_err(), "source/payload mutation {mutation}");
    }
}

#[test]
fn authenticated_hashi_source_coverage_binds_exact_checkpoint_request_and_prevents_replay() {
    let mut f = authenticated_source_fixture();
    let RecoveryAction::Incoming(bytes) = &f.work[1].action else {
        unreachable!()
    };
    let operation: IncomingOperation = decode(bytes).unwrap();
    let IncomingChannel::HashiAuthenticatedSource { request, .. } = operation.channel else {
        unreachable!()
    };
    let original: IncomingRequest<Runtime> = decode(&request).unwrap();
    let record = f.insert(OffchainRequest::incoming(original), RequestStatus::Pending);
    f.review.decisions[0]
        .checkpoint_records
        .push(record.clone());
    assert!(f.verify().is_ok());
    let original_work = f.work[1].clone();
    edit_source_operation(&mut f, |operation| {
        let IncomingChannel::HashiAuthenticatedSource { request, .. } = &mut operation.channel
        else {
            unreachable!()
        };
        let IncomingRequest::Transfer(mut t) = decode::<IncomingRequest<Runtime>>(request).unwrap()
        else {
            unreachable!()
        };
        t.amount += 1;
        *request = IncomingRequest::Transfer(t).encode();
    });
    assert!(f.verify().unwrap_err().contains("exact historical adapter"));
    f.work[1] = original_work;
    let key = hex_bytes(&record.key).unwrap();
    let native: OffchainRequest<Runtime> = decode(f.storage.top.get(&key).unwrap()).unwrap();
    let status_key = hex_bytes(&record.status_key).unwrap();
    f.storage
        .top
        .insert(status_key, RequestStatus::Done.encode());
    f.review.decisions[0].checkpoint_records.clear();
    assert_eq!(native.encode(), f.storage.top[&key]);
    assert!(f.verify().unwrap_err().contains("replay a consumed"));
}

#[test]
fn unauthenticated_legacy_hashi_zero_sender_cannot_clear_coverage() {
    let mut f = authenticated_source_fixture();
    edit_source_operation(&mut f, |operation| {
        let IncomingChannel::HashiAuthenticatedSource {
            network,
            request,
            approvals,
            ..
        } = &operation.channel
        else {
            unreachable!()
        };
        operation.channel = IncomingChannel::Hashi {
            network: *network,
            request: request.clone(),
            approvals: approvals.clone(),
        };
    });
    assert!(f
        .verify()
        .unwrap_err()
        .contains("lacks its native source adapter"));
}

struct Fixture {
    schema: MetadataSchema,
    storage: Storage,
    policy: RecoveryPolicy,
    external: ExternalInventory,
    review: Review,
    work: Vec<RecoveryInherent>,
    witnesses: Vec<StorageWitness>,
}
fn envelope(action: RecoveryAction) -> RecoveryInherent {
    RecoveryInherent {
        format_version: 1,
        action,
    }
}
fn transfer() -> IncomingTransfer<Runtime> {
    IncomingTransfer {
        from: bridge_types::H160::repeat_byte(7),
        to: AccountId32::new([1; 32]),
        asset_id: common::XOR,
        asset_kind: AssetKind::SidechainOwned,
        amount: 100,
        author: AccountId32::new([1; 32]),
        tx_hash: H256::repeat_byte(8),
        at_height: 10,
        timepoint: Default::default(),
        network_id: 0,
        should_take_fee: false,
    }
}
fn terminal_fixture() -> TerminalAudit {
    TerminalAudit {
        issuance: vec![IssuanceInvariant {
            asset: [0; 32],
            current: StoredAmount {
                scope: StorageScope::Top,
                key: b"issuance".to_vec(),
                record_hash: [7; 32],
                offset: 0,
            },
            checkpoint_issuance: 100,
            authenticated_mints: 0,
            burned: 0,
        }],
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![],
    }
}

impl Fixture {
    fn new() -> Self {
        let mut registry = Registry::new();
        let network = registry.register_type(&meta_type::<u32>()).id;
        let id = registry.register_type(&meta_type::<H256>()).id;
        let values = [
            registry
                .register_type(&meta_type::<OffchainRequest<Runtime>>())
                .id,
            registry.register_type(&meta_type::<RequestStatus>()).id,
        ];
        let schema = MetadataSchema {
            registry: registry.into(),
            metadata_hash: hx(&[9; 32]),
            descriptors: ["Requests", "RequestStatuses"]
                .into_iter()
                .enumerate()
                .map(|(index, item)| Descriptor {
                    trie: "top".into(),
                    name: format!("EthBridge.{item}"),
                    prefix: prefix(item),
                    key_types: vec![network, id],
                    hashers: vec![Hasher::Twox64Concat, Hasher::Identity],
                    value_type: values[index],
                })
                .collect(),
        };
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![AccountReplacement {
                old: AccountId32::new([1; 32]),
                new: AccountId32::new([2; 32]),
            }],
            council: vec![AccountId32::new([3; 32])],
            chunk_hashes: vec![],
            audit_chunk_hashes: vec![],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [0; 32],
        };
        let review = Review {
            schema: "sora-nexus-incoming-reconciliation-v1".into(),
            checkpoint_hash: hx(&recovery::CHECKPOINT_HASH),
            mapping_commitment: hx(&hash(&policy.replacements.encode())),
            external_inventory_sha256: "22".repeat(32),
            decisions: vec![],
        };
        Self {
            schema,
            storage: Storage::default(),
            policy,
            review,
            external: ExternalInventory {
                schema: "sora-nexus-incoming-delivery-inventory-v1".into(),
                checkpoint_hash: hx(&recovery::CHECKPOINT_HASH),
                frozen_heads_sha256: "33".repeat(32),
                deliveries: vec![],
            },
            work: vec![envelope(RecoveryAction::Apply(RecoveryChunk {
                index: 0,
                edits: vec![GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: b"fixture".to_vec(),
                    before: None,
                    after: Some(vec![1]),
                }],
            }))],
            witnesses: vec![StorageWitness {
                scope: StorageScope::Top,
                key: b"fixture".to_vec(),
                value_hash: Some(hash(&[1])),
            }],
        }
    }
    fn decision(&mut self, action: Action, beneficiary: bool) {
        self.review.decisions.push(Decision {
            network_scale: hx(&GenericNetworkId::EVMLegacy(0).encode()),
            message_id: hx(&[8; 32]),
            action,
            evidence_hash: hx(&[5; 32]),
            original_beneficiary: beneficiary.then(|| hx(&[1; 32])),
            effective_beneficiary: beneficiary.then(|| hx(&[2; 32])),
            checkpoint_records: vec![],
            quarantine_reason: (action == Action::Quarantine)
                .then_some(QuarantineReason::MissingReceipt),
        });
    }
    fn external(&mut self, obligation: Obligation, beneficiary: bool) {
        self.external.deliveries.push(ExternalDelivery {
            network_scale: hx(&GenericNetworkId::EVMLegacy(0).encode()),
            message_id: hx(&[8; 32]),
            evidence_hash: hx(&[5; 32]),
            original_beneficiary: beneficiary.then(|| hx(&[1; 32])),
            obligation,
        });
    }
    fn insert(&mut self, native: OffchainRequest<Runtime>, status: RequestStatus) -> RecordBinding {
        let key = request_key(&self.schema.descriptors[0], 0, native.hash());
        let status_key = request_key(&self.schema.descriptors[1], 0, native.hash());
        let value = native.encode();
        let status = status.encode();
        let result = RecordBinding {
            key: hx(&key),
            value_hash: hx(&hash(&value)),
            status_key: hx(&status_key),
            status_hash: hx(&hash(&status)),
        };
        self.storage.top.insert(key, value);
        self.storage.top.insert(status_key, status);
        result
    }
    fn retain(&mut self, record: RecordBinding) {
        for (key, value) in [
            (&record.key, &record.value_hash),
            (&record.status_key, &record.status_hash),
        ] {
            self.witnesses.push(StorageWitness {
                scope: StorageScope::Top,
                key: hex_bytes(key).unwrap(),
                value_hash: Some(h32(value).unwrap()),
            });
        }
        self.review.decisions[0].checkpoint_records.push(record);
    }
    fn load(&mut self) -> RecordBinding {
        self.load_with_status(RequestStatus::Failed(
            sp_runtime::DispatchError::CannotLookup,
        ))
    }
    fn load_with_status(&mut self, status: RequestStatus) -> RecordBinding {
        self.insert(
            OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
                LoadIncomingTransactionRequest::new(
                    AccountId32::new([1; 32]),
                    H256::repeat_byte(8),
                    Default::default(),
                    IncomingTransactionRequestKind::Transfer,
                    0,
                ),
            )),
            status,
        )
    }
    fn incoming(&mut self, excluded: bool) {
        let mut transfer = transfer();
        if excluded {
            transfer.from = bridge_types::H160::from(recovery::ATTACKER_EVM);
        }
        self.work.push(envelope(RecoveryAction::Incoming(
            IncomingOperation {
                index: self.work.len() as u32,
                channel: IncomingChannel::Hashi {
                    network: 0,
                    request: IncomingRequest::Transfer(transfer).encode(),
                    approvals: vec![],
                },
                registries: vec![
                    NetworkAttackerRegistry {
                        network: GenericNetworkId::EVMLegacy(0),
                        accounts: vec![GenericAccount::EVM(bridge_types::H160::from(
                            recovery::ATTACKER_EVM,
                        ))],
                        no_known_attackers: false,
                        attestation: [6; 32],
                    },
                    NetworkAttackerRegistry {
                        network: GenericNetworkId::Sub(SubNetworkId::Mainnet),
                        accounts: vec![GenericAccount::Sora(AccountId32::new(
                            recovery::ATTACKER_ACCOUNT,
                        ))],
                        no_known_attackers: false,
                        attestation: [6; 32],
                    },
                ],
                finalized_source_evidence: [5; 32],
            }
            .encode(),
        )));
    }
    fn audits(&self) -> Result<Vec<RecoveryInherent>> {
        let mut audits = vec![envelope(RecoveryAction::Audit(RecoveryAuditChunk {
            index: 0,
            witnesses: self.witnesses.clone(),
        }))];
        for action in &self.work {
            let RecoveryAction::Incoming(bytes) = &action.action else {
                continue;
            };
            let operation: IncomingOperation = decode(bytes)?;
            if let IncomingChannel::Substrate {
                commitment: GenericCommitment::Sub(batch),
                ..
            } = &operation.channel
            {
                if !batch.messages.iter().all(|message| {
                    classify_substrate_control(&message.payload)
                        .is_ok_and(|control| control.is_some())
                }) {
                    return Err("test Substrate fixture supports controls only".into());
                }
                let audit = IncomingAudit {
                    index: audits.len() as u32,
                    operation_index: operation.index,
                    operation_commitment: operation.commitment(),
                    requests: vec![],
                    prior_policy: None,
                };
                audits.push(envelope(RecoveryAction::IncomingAudit(audit.encode())));
                continue;
            }
            let (network, request, actor) = match &operation.channel {
                IncomingChannel::Hashi {
                    network, request, ..
                } => {
                    let IncomingRequest::Transfer(transfer) =
                        decode::<IncomingRequest<Runtime>>(request)?
                    else {
                        unreachable!()
                    };
                    (*network, request, transfer.from)
                }
                IncomingChannel::HashiAuthenticatedSource {
                    network,
                    request,
                    raw_transaction,
                    expected_chain_id,
                    ..
                } => {
                    let actor =
                        authenticated_hashi_source(request, raw_transaction, *expected_chain_id)
                            .map_err(|error| format!("test source evidence: {error:?}"))?
                            .signer;
                    (*network, request, actor)
                }
                _ => return Err("test audit fixture supports HASHI only".into()),
            };
            let IncomingRequest::Transfer(transfer) = decode::<IncomingRequest<Runtime>>(request)?
            else {
                unreachable!()
            };
            let excluded = operation.registries.iter().any(|registry| {
                (registry.network == GenericNetworkId::EVMLegacy(network)
                    && registry.accounts.contains(&GenericAccount::EVM(actor)))
                    || (registry.network == GenericNetworkId::Sub(SubNetworkId::Mainnet)
                        && registry
                            .accounts
                            .contains(&GenericAccount::Sora(transfer.to.clone())))
            });
            let audit = IncomingAudit {
                index: audits.len() as u32,
                operation_index: operation.index,
                operation_commitment: operation.commitment(),
                requests: vec![(!excluded).then(|| IncomingRequestExpectation {
                    source: GenericAccount::EVM(transfer.from),
                    beneficiary: mapped(&self.policy, &transfer.to),
                    asset_id: transfer.asset_id,
                    amount: transfer.amount,
                    start_timepoint: GenericTimepoint::EVM(transfer.at_height),
                })],
                prior_policy: None,
            };
            audits.push(envelope(RecoveryAction::IncomingAudit(audit.encode())));
        }
        Ok(audits)
    }
    fn checked(&mut self) -> Result<CheckedWork> {
        self.policy.chunk_hashes = self
            .work
            .iter()
            .map(|e| match &e.action {
                RecoveryAction::Apply(c) => c.commitment(),
                RecoveryAction::Incoming(bytes) => {
                    decode::<IncomingOperation>(bytes).unwrap().commitment()
                }
                _ => panic!("fixture action"),
            })
            .collect();
        let audits = self.audits()?;
        self.policy.audit_chunk_hashes = audits
            .iter()
            .map(|e| match &e.action {
                RecoveryAction::Audit(a) => a.commitment(),
                RecoveryAction::IncomingAudit(bytes) => {
                    decode::<IncomingAudit>(bytes).unwrap().commitment()
                }
                _ => unreachable!(),
            })
            .collect();
        let terminal = terminal_fixture();
        self.policy.terminal_audit_commitment = terminal.commitment();
        check_work(
            &self.policy,
            self.work.clone(),
            audits,
            envelope(RecoveryAction::TerminalAudit(terminal)),
        )
    }
    fn verify(&mut self) -> Result<Report> {
        let work = self.checked()?;
        verify(
            &self.schema,
            &self.storage,
            &self.policy,
            &work,
            Evidence {
                inventory: &self.external,
                reconciliation: &self.review,
                inventory_sha256: &"22".repeat(32),
                finalized_heads_sha256: &"33".repeat(32),
            },
        )
    }
}

#[test]
fn empty_coverage_never_claims_external_finality_or_execution() {
    let report = Fixture::new().verify().unwrap();
    assert!(report.local_pending_coverage_verified);
    assert!(
        !report.external_archive_completeness_verified
            && !report.external_finality_verified
            && !report.authentication_verified
            && !report.exact_artifact_execution_verified
            && !report.production_ready
    );
}

fn checked_audits(f: &mut Fixture, audits: Vec<RecoveryInherent>) -> Result<CheckedWork> {
    f.checked()?;
    f.policy.audit_chunk_hashes = audits
        .iter()
        .map(|envelope| match &envelope.action {
            RecoveryAction::Audit(audit) => Ok(audit.commitment()),
            RecoveryAction::IncomingAudit(bytes) => {
                Ok(decode::<IncomingAudit>(bytes)?.commitment())
            }
            _ => Err("unexpected test audit action".into()),
        })
        .collect::<Result<Vec<_>>>()?;
    check_work(
        &f.policy,
        f.work.clone(),
        audits,
        envelope(RecoveryAction::TerminalAudit(terminal_fixture())),
    )
}

#[test]
fn semantic_audit_coverage_rejects_omission_duplicates_wrong_work_and_unproved_prior_policy() {
    let mut f = Fixture::new();
    f.incoming(false);
    let base = f.audits().unwrap();
    assert!(checked_audits(&mut f, base.clone()).is_ok());
    let mut omitted = base.clone();
    omitted.pop();
    assert!(checked_audits(&mut f, omitted)
        .err()
        .unwrap()
        .contains("every incoming"));
    for mutation in 0..4 {
        let mut audits = base.clone();
        let RecoveryAction::IncomingAudit(bytes) = &mut audits[1].action else {
            unreachable!()
        };
        let mut audit: IncomingAudit = decode(bytes).unwrap();
        match mutation {
            0 => audit.operation_index = 0,
            1 => audit.operation_commitment = [42; 32],
            2 => audit.prior_policy = Some([43; 32]),
            _ => audit.index = 9,
        }
        *bytes = audit.encode();
        let error = checked_audits(&mut f, audits).err().unwrap();
        if mutation == 2 {
            assert!(error.contains("authenticated recovery-parent companion evidence"));
        }
    }
    let mut repeated = base;
    let RecoveryAction::IncomingAudit(bytes) = &repeated[1].action else {
        unreachable!()
    };
    let mut audit: IncomingAudit = decode(bytes).unwrap();
    audit.index = 2;
    repeated.push(envelope(RecoveryAction::IncomingAudit(audit.encode())));
    assert!(checked_audits(&mut f, repeated)
        .err()
        .unwrap()
        .contains("duplicate incoming"));
}

#[test]
fn semantic_audit_cannot_change_exclusion_owner_source_start_or_hashi_value() {
    for mutation in 0..7 {
        let mut f = Fixture::new();
        f.incoming(false);
        f.external(Obligation::Admit, true);
        f.decision(Action::Process, true);
        let mut audits = f.audits().unwrap();
        let RecoveryAction::IncomingAudit(bytes) = &mut audits[1].action else {
            unreachable!()
        };
        let mut audit: IncomingAudit = decode(bytes).unwrap();
        let request = audit.requests[0].as_mut().unwrap();
        match mutation {
            0 => request.beneficiary = AccountId32::new([3; 32]),
            1 => request.source = GenericAccount::EVM(bridge_types::H160::repeat_byte(4)),
            2 => request.start_timepoint = GenericTimepoint::EVM(999),
            3 => request.amount += 1,
            4 => request.asset_id = common::DAI,
            5 => audit.requests[0] = None,
            _ => audit.requests.push(None),
        }
        *bytes = audit.encode();
        let work = checked_audits(&mut f, audits).unwrap();
        let result = verify(
            &f.schema,
            &f.storage,
            &f.policy,
            &work,
            Evidence {
                inventory: &f.external,
                reconciliation: &f.review,
                inventory_sha256: &"22".repeat(32),
                finalized_heads_sha256: &"33".repeat(32),
            },
        );
        assert!(result.is_err(), "mutation {mutation}");
    }
}
#[test]
fn failed_load_requires_exact_quarantine_and_retains_effective_query_owner() {
    let mut f = Fixture::new();
    let record = f.load();
    assert!(f.verify().unwrap_err().contains("incomplete"));
    f.decision(Action::Quarantine, false);
    f.retain(record);
    let report = f.verify().unwrap();
    assert_eq!(report.checkpoint_unconsumed_records, 1);
    assert_eq!(
        report.deliveries[0].checkpoint_authors,
        vec![(hx(&[1; 32]), hx(&[2; 32]))]
    );
    assert!(report.deliveries[0].original_beneficiary.is_none());
    f.witnesses.pop();
    assert!(f.verify().unwrap_err().contains("preservation witnesses"));
}
#[test]
fn done_load_without_completed_transfer_requires_preserved_unresolved_disposition() {
    let mut f = Fixture::new();
    let record = f.load_with_status(RequestStatus::Done);
    assert!(f.verify().unwrap_err().contains("incomplete"));
    f.decision(Action::RetainConsumed, false);
    f.retain(record);
    assert!(f.verify().unwrap_err().contains("lacks native completion"));
    f.review.decisions[0].action = Action::Quarantine;
    f.review.decisions[0].quarantine_reason = Some(QuarantineReason::MissingReceipt);
    let report = f.verify().unwrap();
    assert_eq!(report.checkpoint_unresolved_done_load_records, 1);
    assert_eq!(report.checkpoint_unconsumed_records, 1);
    assert_eq!(report.deliveries[0].checkpoint_records, 1);
    assert_eq!(
        report.deliveries[0].checkpoint_authors,
        vec![(hx(&[1; 32]), hx(&[2; 32]))]
    );
    f.witnesses.pop();
    assert!(f.verify().unwrap_err().contains("preservation witnesses"));
}

#[test]
fn done_load_and_pending_transfer_require_both_records_but_completed_pair_needs_no_replay() {
    let mut f = Fixture::new();
    let load = f.load_with_status(RequestStatus::Done);
    let transfer = f.insert(
        OffchainRequest::incoming(IncomingRequest::Transfer(transfer())),
        RequestStatus::Pending,
    );
    f.decision(Action::Quarantine, true);
    f.retain(transfer.clone());
    assert!(f
        .verify()
        .unwrap_err()
        .contains("every exact checkpoint record"));
    f.retain(load);
    let report = f.verify().unwrap();
    assert_eq!(report.checkpoint_unresolved_done_load_records, 1);
    assert_eq!(report.deliveries[0].checkpoint_records, 2);
    f.storage.top.insert(
        hex_bytes(&transfer.status_key).unwrap(),
        RequestStatus::Done.encode(),
    );
    f.review.decisions.clear();
    let report = f.verify().unwrap();
    assert_eq!(report.checkpoint_unresolved_done_load_records, 0);
    assert_eq!(report.checkpoint_unconsumed_records, 0);
    assert!(report.deliveries.is_empty());
}

#[test]
fn unrelated_completed_transfer_cannot_hide_an_orphan_load_by_matching_beneficiary() {
    let mut f = Fixture::new();
    f.load_with_status(RequestStatus::Done);
    let mut other = transfer();
    other.tx_hash = H256::repeat_byte(33);
    f.insert(
        OffchainRequest::incoming(IncomingRequest::Transfer(other)),
        RequestStatus::Done,
    );
    assert!(f.verify().unwrap_err().contains("incomplete"));
}

#[test]
fn quarantine_cannot_invent_a_beneficiary_or_mutate_retained_liability() {
    let mut f = Fixture::new();
    let record = f.load();
    f.decision(Action::Quarantine, true);
    f.retain(record.clone());
    assert!(f
        .verify()
        .unwrap_err()
        .contains("no payload or source evidence"));
    f.review.decisions[0].original_beneficiary = None;
    f.review.decisions[0].effective_beneficiary = None;
    let RecoveryAction::Apply(chunk) = &mut f.work[0].action else {
        unreachable!()
    };
    chunk.edits[0].key = hex_bytes(&record.status_key).unwrap();
    assert!(f.verify().unwrap_err().contains("unchanged state"));
}
#[test]
fn admitted_external_delivery_cannot_be_omitted_or_quarantined() {
    let mut f = Fixture::new();
    f.external(Obligation::Admit, true);
    assert!(f.verify().unwrap_err().contains("incomplete"));
    f.decision(Action::Quarantine, true);
    assert!(f
        .verify()
        .unwrap_err()
        .contains("quarantine cannot replace"));
    f.review.decisions[0].action = Action::Process;
    f.review.decisions[0].quarantine_reason = None;
    assert!(f.verify().unwrap_err().contains("no executable work"));
    f.incoming(false);
    let report = f.verify().unwrap();
    assert_eq!(report.work_deliveries, 1);
    assert_eq!(
        report.deliveries[0].effective_beneficiary,
        Some(hx(&[2; 32]))
    );
    f.review.decisions[0].effective_beneficiary = Some(hx(&[1; 32]));
    assert!(f
        .verify()
        .unwrap_err()
        .contains("effective beneficiary mismatch"));
}
#[test]
fn undeclared_work_wrong_exclusion_and_unresolved_admission_are_rejected() {
    let mut f = Fixture::new();
    f.incoming(true);
    f.decision(Action::Process, true);
    assert!(f.verify().unwrap_err().contains("admission, exclusion"));
    f.external(Obligation::Admit, true);
    assert!(f.verify().unwrap_err().contains("admission, exclusion"));
    f.review.decisions[0].action = Action::Exclude;
    assert!(f.verify().is_ok());
    f.external.deliveries[0].obligation = Obligation::Unresolved;
    assert!(f.verify().unwrap_err().contains("admission, exclusion"));
}
#[test]
fn done_transfer_cannot_be_replayed_or_retained_without_complete_final_witnesses() {
    let mut f = Fixture::new();
    let record = f.insert(
        OffchainRequest::incoming(IncomingRequest::Transfer(transfer())),
        RequestStatus::Done,
    );
    f.external(Obligation::Admit, true);
    f.decision(Action::Process, true);
    f.incoming(false);
    assert!(f.verify().unwrap_err().contains("replay a consumed"));
    f.work.pop();
    f.review.decisions[0].action = Action::RetainConsumed;
    assert!(f
        .verify()
        .unwrap_err()
        .contains("every exact checkpoint record"));
    f.retain(record);
    assert!(f.verify().is_ok());
    f.witnesses.pop();
    assert!(f.verify().unwrap_err().contains("preservation witnesses"));
}
#[test]
fn pending_payload_and_historical_wrapper_must_not_be_reinterpreted() {
    let mut f = Fixture::new();
    let record = f.insert(
        OffchainRequest::Incoming(IncomingRequest::Transfer(transfer()), H256::repeat_byte(44)),
        RequestStatus::Pending,
    );
    f.external(Obligation::Admit, true);
    f.decision(Action::Process, true);
    f.review.decisions[0].checkpoint_records.push(record);
    f.incoming(false);
    assert!(f.verify().unwrap_err().contains("exact historical adapter"));
}
#[test]
fn stale_context_duplicate_decisions_and_hidden_work_are_rejected() {
    let mut f = Fixture::new();
    f.external(Obligation::Admit, true);
    f.decision(Action::Process, true);
    f.incoming(false);
    let decision = f.review.decisions[0].clone();
    f.review.decisions.push(decision);
    assert!(f
        .verify()
        .unwrap_err()
        .contains("duplicate incoming disposition"));
    f.review.decisions.pop();
    f.external.frozen_heads_sha256 = "44".repeat(32);
    assert!(f.verify().unwrap_err().contains("context"));
    f.external.frozen_heads_sha256 = "33".repeat(32);
    let mut checked = f.checked().unwrap();
    checked.policy.pop();
    assert!(verify(
        &f.schema,
        &f.storage,
        &f.policy,
        &checked,
        Evidence {
            inventory: &f.external,
            reconciliation: &f.review,
            inventory_sha256: &"22".repeat(32),
            finalized_heads_sha256: &"33".repeat(32),
        }
    )
    .is_err());
    let mut work = f.work.clone();
    work.pop();
    assert!(check_work(
        &f.policy,
        work,
        vec![],
        envelope(RecoveryAction::Incoming(vec![1]))
    )
    .is_err());
}

#[test]
fn work_index_checks_every_shard_slot_and_never_accepts_a_filtered_incoming_list() {
    let mut f = Fixture::new();
    f.incoming(false);
    f.checked().unwrap();
    let path = std::env::temp_dir().join(format!(
        "sora-nexus-incoming-index-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&path).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    let write = |name: &str, envelope: &RecoveryInherent| {
        let bytes = envelope.encode_remark();
        std::fs::write(path.join(name), &bytes).unwrap();
        WorkFile {
            file: name.into(),
            bytes: bytes.len(),
            sha256: hex::encode(Sha256::digest(&bytes)),
        }
    };
    let index = WorkIndex {
        terminal_prelude: vec![],
        schema: "sora-nexus-recovery-work-index-v1".into(),
        policy_sha256: hex::encode(Sha256::digest(f.policy.encode())),
        work: f
            .work
            .iter()
            .enumerate()
            .map(|(i, e)| write(&format!("work-{i}.scale"), e))
            .collect(),
        audits: f
            .audits()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, audit)| write(&format!("audit-{index}.scale"), audit))
            .collect(),
        terminal: write(
            "terminal.scale",
            &envelope(RecoveryAction::TerminalAudit(terminal_fixture())),
        ),
    };
    assert_eq!(
        load_work(&path, index.clone(), &f.policy)
            .unwrap()
            .incoming
            .len(),
        1
    );
    let mut changed = index.clone();
    changed.work.remove(0);
    assert!(load_work(&path, changed, &f.policy).is_err());
    let mut changed = index.clone();
    changed.work.swap(0, 1);
    assert!(load_work(&path, changed, &f.policy).is_err());
    let mut changed = index.clone();
    changed.work[1].file = changed.work[0].file.clone();
    assert!(load_work(&path, changed, &f.policy).is_err());
    let mut changed = index.clone();
    changed.audits[0].file = "../audit.scale".into();
    assert!(load_work(&path, changed, &f.policy).is_err());
    let mut changed = index.clone();
    changed.terminal.bytes = recovery::MAX_INHERENT_BYTES + 1;
    assert!(load_work(&path, changed, &f.policy).is_err());
    std::fs::write(path.join("work-1.scale"), vec![0; index.work[1].bytes]).unwrap();
    assert!(load_work(&path, index, &f.policy).is_err());
}

#[test]
fn storage_preservation_skips_empty_and_completed_inventory_but_keeps_unresolved_done_loads() {
    let mut f = Fixture::new();
    assert!(retained_storage_witnesses(&f.schema, &f.storage)
        .unwrap()
        .witnesses
        .is_empty());
    f.load_with_status(RequestStatus::Done);
    let retained = retained_storage_witnesses(&f.schema, &f.storage).unwrap();
    assert_eq!(retained.witnesses.len(), 2);
    assert_eq!(retained.report.unresolved_done_load_records, 1);
    f.insert(
        OffchainRequest::incoming(IncomingRequest::Transfer(transfer())),
        RequestStatus::Pending,
    );
    assert_eq!(
        retained_storage_witnesses(&f.schema, &f.storage)
            .unwrap()
            .witnesses
            .len(),
        4
    );
    f.insert(
        OffchainRequest::incoming(IncomingRequest::Transfer(transfer())),
        RequestStatus::Done,
    );
    let retained = retained_storage_witnesses(&f.schema, &f.storage).unwrap();
    assert!(retained.witnesses.is_empty());
    assert_eq!(retained.report.unresolved_records, 0);
}

#[test]
fn nine_unresolved_loads_emit_eighteen_exact_source_witnesses() {
    let mut f = Fixture::new();
    for byte in 1..=9 {
        f.insert(
            OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
                LoadIncomingTransactionRequest::new(
                    AccountId32::new([1; 32]),
                    H256::repeat_byte(byte),
                    Default::default(),
                    IncomingTransactionRequestKind::Transfer,
                    0,
                ),
            )),
            RequestStatus::Failed(sp_runtime::DispatchError::CannotLookup),
        );
    }
    let retained = retained_storage_witnesses(&f.schema, &f.storage).unwrap();
    assert_eq!(retained.report.unresolved_records, 9);
    assert_eq!(retained.report.witnesses, 18);
    assert!(!retained.report.incoming_execution_verified && !retained.report.production_ready);
    for witness in retained.witnesses {
        assert_eq!(witness.scope, StorageScope::Top);
        assert_eq!(
            witness.value_hash,
            f.storage.top.get(&witness.key).map(|value| hash(value))
        );
    }
}

#[test]
fn storage_composer_rehearses_preservation_and_rejects_conflicting_status_edits() {
    let mut f = Fixture::new();
    let record = f.load();
    f.storage.top.insert(b"issuance".to_vec(), 100u128.encode());
    let mut terminal = terminal_fixture();
    terminal.issuance[0].current.record_hash = hash(&100u128.encode());
    let retained = retained_storage_witnesses(&f.schema, &f.storage).unwrap();
    let RecoveryAction::Apply(chunk) = &f.work[0].action else {
        unreachable!()
    };
    let compile = |edits| {
        crate::composer::compose_with_audits(
            &f.storage,
            f.policy.replacements.clone(),
            f.policy.council.clone(),
            crate::composer::PolicyIdentity::public_fixture([4; 32]),
            vec![edits],
            crate::composer::AuditRequests {
                keys: retained.witnesses.clone(),
                empty_prefixes: vec![],
            },
            terminal.clone(),
        )
    };
    let result = compile(chunk.edits.clone()).unwrap();
    assert!(result.report.storage_engine_rehearsed);
    for expected in &retained.witnesses {
        assert!(result
            .audits
            .iter()
            .flat_map(|audit| &audit.witnesses)
            .any(|actual| actual == expected));
    }
    let error = compile(vec![GuardedStorageEdit {
        scope: StorageScope::Top,
        key: hex_bytes(&record.status_key).unwrap(),
        before: Some(h32(&record.status_hash).unwrap()),
        after: Some(RequestStatus::Done.encode()),
    }])
    .err()
    .expect("conflicting preservation must fail");
    assert!(error.contains("additional audit conflicts"));
}

#[test]
fn preservation_includes_prior_completion_proof_for_an_unresolved_duplicate_load() {
    let mut f = Fixture::new();
    let load = f.load();
    let completed = f.insert(
        OffchainRequest::incoming(IncomingRequest::Transfer(transfer())),
        RequestStatus::Done,
    );
    let retained = retained_storage_witnesses(&f.schema, &f.storage).unwrap();
    assert_eq!(retained.report.unresolved_records, 1);
    assert_eq!(retained.witnesses.len(), 4);
    f.decision(Action::RetainConsumed, true);
    f.review.decisions[0].checkpoint_records = vec![load, completed.clone()];
    f.witnesses.extend(retained.witnesses);
    assert!(f.verify().is_ok());
    f.witnesses
        .retain(|w| w.key != hex_bytes(&completed.status_key).unwrap());
    assert!(f.verify().unwrap_err().contains("preservation witnesses"));
}

#[test]
fn prior_certificate_loader_keeps_current_and_prior_same_index_distinct() {
    let mut f = Fixture::new();
    f.incoming(false);
    let original: IncomingOperation = match &f.work[1].action {
        RecoveryAction::Incoming(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    let mut prior: IncomingAudit = match &f.audits().unwrap()[1].action {
        RecoveryAction::IncomingAudit(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    edit_source_operation(&mut f, |operation| {
        let IncomingChannel::Hashi { request, .. } = &mut operation.channel else {
            unreachable!()
        };
        let IncomingRequest::Transfer(mut transfer) =
            decode::<IncomingRequest<Runtime>>(request).unwrap()
        else {
            unreachable!()
        };
        transfer.tx_hash = H256::repeat_byte(99);
        *request = IncomingRequest::Transfer(transfer).encode();
    });
    f.checked().unwrap();
    let mut audits = f.audits().unwrap();
    prior.index = audits.len() as u32;
    prior.prior_policy = Some([77; 32]);
    audits.push(envelope(RecoveryAction::IncomingAudit(prior.encode())));
    f.policy.audit_chunk_hashes.push(prior.commitment());
    let certificate = crate::incoming_replay::test_certificate(&f.policy, &prior, original);
    let stream = || {
        f.work
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, e)| Ok((0, i, e)))
            .chain(
                audits
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, e)| Ok((1, i, e))),
            )
            .chain(std::iter::once(Ok((
                2,
                0,
                envelope(RecoveryAction::TerminalAudit(terminal_fixture())),
            ))))
    };
    let checked = check_stream_with_replay(
        &f.policy,
        f.work.len(),
        audits.len(),
        stream(),
        Some(&certificate),
    )
    .unwrap();
    assert_eq!(checked.incoming.len(), 1);
    assert_eq!(checked.prior_incoming.len(), 1);
    assert_eq!(checked.incoming[0].index, checked.prior_incoming[0].index);
    assert_eq!(checked.incoming_audits.len(), 2);
    assert!(checked.replay.is_some());
    assert!(
        check_stream(&f.policy, f.work.len(), audits.len(), stream())
            .err()
            .unwrap()
            .contains("checkpoint-only")
    );
    let mut wrong = f.policy.clone();
    wrong.evidence_commitment = [78; 32];
    assert!(check_stream_with_replay(
        &wrong,
        f.work.len(),
        audits.len(),
        stream(),
        Some(&certificate)
    )
    .err()
    .unwrap()
    .contains("no accepted exact-replay"));
}

#[test]
fn historical_audit_omitted_by_later_repair_is_imported_only_from_final_certificate() {
    let mut original = Fixture::new();
    original.incoming(false);
    original.checked().unwrap();
    let operation: IncomingOperation = match &original.work[1].action {
        RecoveryAction::Incoming(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    let mut prior: IncomingAudit = match &original.audits().unwrap()[1].action {
        RecoveryAction::IncomingAudit(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    prior.prior_policy = Some([77; 32]);
    original.policy.audit_chunk_hashes[1] = prior.commitment();
    let audited = original.policy.clone();
    let mut final_fixture = Fixture::new();
    final_fixture.checked().unwrap();
    let audits = final_fixture.audits().unwrap();
    let certificate = crate::incoming_replay::test_historical_certificate(
        &final_fixture.policy,
        &audited,
        &prior,
        operation,
    );
    let stream = || {
        final_fixture
            .work
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, e)| Ok((0, i, e)))
            .chain(
                audits
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, e)| Ok((1, i, e))),
            )
            .chain(std::iter::once(Ok((
                2,
                0,
                envelope(RecoveryAction::TerminalAudit(terminal_fixture())),
            ))))
    };
    let checked = check_stream_with_replay(
        &final_fixture.policy,
        final_fixture.work.len(),
        audits.len(),
        stream(),
        Some(&certificate),
    )
    .unwrap();
    assert!(checked.incoming.is_empty());
    assert_eq!(checked.prior_incoming.len(), 1);
    assert_eq!(checked.incoming_audits.len(), 1);
    assert!(checked.replay.is_some());
    let without = check_stream(
        &final_fixture.policy,
        final_fixture.work.len(),
        audits.len(),
        stream(),
    )
    .unwrap();
    assert!(without.prior_incoming.is_empty());
    let mut different = final_fixture.policy.clone();
    different.evidence_commitment = [112; 32];
    assert!(check_stream_with_replay(
        &different,
        final_fixture.work.len(),
        audits.len(),
        stream(),
        Some(&certificate)
    )
    .err()
    .unwrap()
    .contains("final-policy"));
}

#[test]
fn historical_audit_cannot_duplicate_a_later_policy_operation() {
    let mut f = Fixture::new();
    f.incoming(false);
    f.checked().unwrap();
    let operation: IncomingOperation = match &f.work[1].action {
        RecoveryAction::Incoming(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    let audits = f.audits().unwrap();
    let mut prior: IncomingAudit = match &audits[1].action {
        RecoveryAction::IncomingAudit(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    prior.prior_policy = Some([77; 32]);
    let mut audited = f.policy.clone();
    audited.audit_chunk_hashes[1] = prior.commitment();
    let certificate =
        crate::incoming_replay::test_historical_certificate(&f.policy, &audited, &prior, operation);
    let stream = f
        .work
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, e)| Ok((0, i, e)))
        .chain(
            audits
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, e)| Ok((1, i, e))),
        )
        .chain(std::iter::once(Ok((
            2,
            0,
            envelope(RecoveryAction::TerminalAudit(terminal_fixture())),
        ))));
    assert!(check_stream_with_replay(
        &f.policy,
        f.work.len(),
        audits.len(),
        stream,
        Some(&certificate)
    )
    .err()
    .unwrap()
    .contains("duplicates current"));
}

#[test]
fn completed_same_policy_incoming_audit_is_preserved_by_later_repair_coverage() {
    let mut old = Fixture::new();
    old.incoming(false);
    old.checked().unwrap();
    let operation: IncomingOperation = match &old.work[1].action {
        RecoveryAction::Incoming(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    let audit: IncomingAudit = match &old.audits().unwrap()[1].action {
        RecoveryAction::IncomingAudit(bytes) => decode(bytes).unwrap(),
        _ => unreachable!(),
    };
    assert!(audit.prior_policy.is_none());
    let mut final_fixture = Fixture::new();
    final_fixture.checked().unwrap();
    let audits = final_fixture.audits().unwrap();
    let certificate = crate::incoming_replay::test_historical_certificate(
        &final_fixture.policy,
        &old.policy,
        &audit,
        operation,
    );
    let stream = final_fixture
        .work
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, e)| Ok((0, i, e)))
        .chain(
            audits
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, e)| Ok((1, i, e))),
        )
        .chain(std::iter::once(Ok((
            2,
            0,
            envelope(RecoveryAction::TerminalAudit(terminal_fixture())),
        ))));
    let checked = check_stream_with_replay(
        &final_fixture.policy,
        final_fixture.work.len(),
        audits.len(),
        stream,
        Some(&certificate),
    )
    .unwrap();
    assert_eq!(checked.prior_incoming.len(), 1);
    assert_eq!(checked.incoming_audits.len(), 1);
    assert_eq!(certificate.reports().len(), 1);
}

fn control_fixture() -> Fixture {
    use bridge_types::substrate::{BridgeMessage, MultisigVerifierCall};
    let mut fixture = Fixture::new();
    let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
    let calls = [
        BridgeCall::SubstrateApp(SubstrateAppCall::ReportTransferResult {
            message_id: H256::repeat_byte(55),
            message_status: bridge_types::types::MessageStatus::Failed,
        }),
        BridgeCall::MultisigVerifier(MultisigVerifierCall::AddPeer {
            peer: sp_core::ecdsa::Public::from_raw([3; 33]),
        }),
    ];
    let operation = IncomingOperation {
        index: fixture.work.len() as u32,
        channel: IncomingChannel::Substrate {
            network: SubNetworkId::Liberland,
            commitment: GenericCommitment::Sub(bridge_types::substrate::Commitment {
                nonce: 1,
                messages: calls
                    .iter()
                    .map(|call| BridgeMessage {
                        payload: call.encode().try_into().unwrap(),
                        timepoint: GenericTimepoint::Sora(1),
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            }),
            proof: framenode_runtime::MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof {
                proof: vec![],
            }),
        },
        registries: vec![
            NetworkAttackerRegistry {
                network,
                accounts: vec![],
                no_known_attackers: true,
                attestation: [6; 32],
            },
            NetworkAttackerRegistry {
                network: GenericNetworkId::Sub(SubNetworkId::Mainnet),
                accounts: vec![GenericAccount::Sora(AccountId32::new(
                    recovery::ATTACKER_ACCOUNT,
                ))],
                no_known_attackers: false,
                attestation: [6; 32],
            },
        ], // Proofs remain empty: this structural fixture is never executable evidence.
        finalized_source_evidence: [5; 32],
    };
    fixture
        .work
        .push(envelope(RecoveryAction::Incoming(operation.encode())));
    for (index, action) in [Action::JournalControl, Action::ApplyVerifierPeer]
        .into_iter()
        .enumerate()
    {
        let id = MessageId::batched(
            network,
            GenericNetworkId::Sub(SubNetworkId::Mainnet),
            1,
            index as u64,
        )
        .hash();
        fixture.external.deliveries.push(ExternalDelivery {
            network_scale: hx(&network.encode()),
            message_id: hx(id.as_bytes()),
            evidence_hash: hx(&[5; 32]),
            original_beneficiary: None,
            obligation: Obligation::Admit,
        });
        fixture.review.decisions.push(Decision {
            network_scale: hx(&network.encode()),
            message_id: hx(id.as_bytes()),
            action,
            evidence_hash: hx(&[5; 32]),
            original_beneficiary: None,
            effective_beneficiary: None,
            checkpoint_records: vec![],
            quarantine_reason: None,
        });
    }
    fixture
}

#[test]
fn control_coverage_requires_explicit_nonfinancial_dispositions_without_claiming_execution() {
    let mut fixture = control_fixture();
    let report = fixture.verify().unwrap();
    assert_eq!(report.work_deliveries, 0);
    assert_eq!(report.work_control_messages, 2);
    assert_eq!(report.deliveries.len(), 2);
    assert!(!report.deferred_control_execution_verified);
    assert!(
        !report.authentication_verified
            && !report.exact_artifact_execution_verified
            && !report.production_ready
    );
    for action in [
        Action::Process,
        Action::Exclude,
        Action::Quarantine,
        Action::RetainConsumed,
        Action::ApplyVerifierPeer,
        Action::ApplyAssetRegistration,
    ] {
        let mut invalid = control_fixture();
        invalid.review.decisions[0].action = action;
        assert!(invalid.verify().is_err());
    }
    let mut invalid = control_fixture();
    invalid.review.decisions[0].original_beneficiary = Some(hx(&[1; 32]));
    invalid.review.decisions[0].effective_beneficiary = Some(hx(&[2; 32]));
    assert!(invalid.verify().is_err());
    let mut invalid = control_fixture();
    invalid.external.deliveries[0].obligation = Obligation::Unresolved;
    assert!(invalid.verify().is_err());
    let mut invalid = control_fixture();
    invalid.external.deliveries.pop();
    assert!(invalid.verify().is_err());
    let mut invalid = control_fixture();
    edit_source_operation(&mut invalid, |operation| operation.registries.clear());
    assert!(invalid.verify().is_err());
}

#[test]
fn financial_audit_index_skips_control_messages_without_changing_native_message_id() {
    let fixture = control_fixture();
    let RecoveryAction::Incoming(raw) = &fixture.work[1].action else {
        unreachable!()
    };
    let mut operation: IncomingOperation = decode(raw).unwrap();
    let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
    operation.registries = vec![
        NetworkAttackerRegistry {
            network,
            accounts: vec![],
            no_known_attackers: true,
            attestation: [6; 32],
        },
        NetworkAttackerRegistry {
            network: GenericNetworkId::Sub(SubNetworkId::Mainnet),
            accounts: vec![GenericAccount::Sora(AccountId32::new(
                recovery::ATTACKER_ACCOUNT,
            ))],
            no_known_attackers: false,
            attestation: [6; 32],
        },
    ];
    let payload = BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
        sender: GenericAccount::Liberland(AccountId32::new([4; 32])),
        recipient: GenericAccount::Sora(AccountId32::new([1; 32])),
        asset_id: bridge_types::GenericAssetId::Sora(common::DAI.into()),
        amount: bridge_types::GenericBalance::Substrate(1),
    })
    .encode();
    let IncomingChannel::Substrate {
        commitment: GenericCommitment::Sub(batch),
        ..
    } = &mut operation.channel
    else {
        unreachable!()
    };
    batch
        .messages
        .try_push(bridge_types::substrate::BridgeMessage {
            payload: payload.clone().try_into().unwrap(),
            timepoint: GenericTimepoint::Sora(9),
        })
        .unwrap();
    let id = MessageId::batched(network, GenericNetworkId::Sub(SubNetworkId::Mainnet), 1, 2);
    let mut planned = BTreeMap::new();
    add_payload(&mut planned, &operation, network, id, &payload, true).unwrap();
    let value = planned.get(&(network.encode(), id.hash().0)).unwrap();
    assert_eq!(value.message_index, 0);
    assert_eq!(value.start_timepoint, GenericTimepoint::Sora(9));
}

#[test]
fn unattributed_transfer_coverage_never_promotes_an_unknown_sender_to_attacker_clearance() {
    let fixture = control_fixture();
    let RecoveryAction::Incoming(bytes) = &fixture.work[1].action else {
        unreachable!()
    };
    let operation: IncomingOperation = decode(bytes).unwrap();
    for sender in [
        Some(GenericAccount::Unknown),
        Some(GenericAccount::Root),
        None,
    ] {
        let call = match sender {
            Some(sender) => BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                sender,
                recipient: GenericAccount::Sora(AccountId32::new([1; 32])),
                asset_id: bridge_types::GenericAssetId::Sora(common::DAI.into()),
                amount: bridge_types::GenericBalance::Substrate(1),
            }),
            None => BridgeCall::ParachainApp(ParachainAppCall::Transfer {
                sender: None,
                recipient: AccountId32::new([1; 32]),
                asset_id: common::DAI.into(),
                amount: 1,
            }),
        };
        let mut out = BTreeMap::new();
        let error = add_payload(
            &mut out,
            &operation,
            GenericNetworkId::Sub(SubNetworkId::Liberland),
            MessageId::basic(
                GenericNetworkId::Sub(SubNetworkId::Liberland),
                GenericNetworkId::Sub(SubNetworkId::Mainnet),
                1,
            ),
            &call.encode(),
            true,
        )
        .unwrap_err();
        assert!(error.contains("source-attribution adapter"));
        assert!(out.is_empty());
    }
}

#[test]
fn registration_control_coverage_requires_native_finalization_disposition() {
    for parachain in [false, true] {
        let mut fixture = control_fixture();
        edit_source_operation(&mut fixture, |operation| {
            let IncomingChannel::Substrate {
                commitment: GenericCommitment::Sub(batch),
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let call = if parachain {
                BridgeCall::ParachainApp(ParachainAppCall::FinalizeAssetRegistration {
                    asset_id: common::DAI.into(),
                    asset_kind: bridge_types::types::AssetKind::Sidechain,
                })
            } else {
                BridgeCall::SubstrateApp(SubstrateAppCall::FinalizeAssetRegistration {
                    asset_id: bridge_types::GenericAssetId::Sora(common::DAI.into()),
                    sidechain_asset: bridge_types::GenericAssetId::Sora(common::DAI.into()),
                    asset_kind: bridge_types::types::AssetKind::Sidechain,
                    precision: 18,
                })
            };
            batch.messages[0].payload = call.encode().try_into().unwrap();
        });
        assert!(
            fixture.verify().is_err(),
            "finalization must not be marked journal-only"
        );
        fixture.review.decisions[0].action = Action::ApplyAssetRegistration;
        let report = fixture.verify().unwrap();
        assert_eq!(report.work_control_messages, 2);
        assert!(
            !report.authentication_verified
                && !report.exact_artifact_execution_verified
                && !report.production_ready
        );
        fixture.review.decisions[0].action = Action::ApplyVerifierPeer;
        assert!(fixture.verify().is_err());
    }
}
