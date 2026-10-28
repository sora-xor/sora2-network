//! Synthetic native library cases only. These do not manufacture actual receipt
//! inclusion, production key ownership, peer approval or source-chain finality.
use super::*;
use common::recovery::AccountReplacement;
use eth_bridge::requests::{AssetKind, IncomingTransfer};
use sp_core::storage::Storage;

const RAW:&str="f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";
fn prefix(pallet: &str, item: &str) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(pallet.as_bytes()),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn twox(data: &[u8]) -> Vec<u8> {
    [sp_io::hashing::twox_64(data).as_slice(), data].concat()
}
fn blake(data: &[u8]) -> Vec<u8> {
    [sp_io::hashing::blake2_128(data).as_slice(), data].concat()
}
fn storage(
    owned: bool,
    local_precision: u8,
    sidechain_precision: u8,
    denominator: u128,
) -> Storage {
    let mut storage = Storage::default();
    let network = 0u32.encode();
    let asset = if owned { common::XOR } else { common::DAI };
    storage.top.insert(
        [prefix("EthBridge", "BridgeContractAddress"), twox(&network)].concat(),
        H160::repeat_byte(0x41).encode(),
    );
    storage.top.insert(
        [
            prefix("EthBridge", "RegisteredAsset"),
            twox(&network),
            asset.encode(),
        ]
        .concat(),
        if owned {
            AssetKind::Thischain
        } else {
            AssetKind::Sidechain
        }
        .encode(),
    );
    storage.top.insert(
        [
            prefix("EthBridge", "RegisteredSidechainAsset"),
            twox(&network),
            blake(&H160::repeat_byte(0x22).encode()),
        ]
        .concat(),
        asset.encode(),
    );
    storage.top.insert(
        [
            prefix("EthBridge", "SidechainAssetPrecision"),
            twox(&network),
            asset.encode(),
        ]
        .concat(),
        sidechain_precision.encode(),
    );
    storage.top.insert(
        [prefix("Assets", "AssetInfosV2"), twox(&asset.encode())].concat(),
        common::AssetInfo {
            precision: local_precision,
            ..Default::default()
        }
        .encode(),
    );
    storage
        .top
        .insert(prefix("Denomination", "Denominator"), denominator.encode());
    storage
}
struct Fixture {
    row: Row,
    source: SourceReport,
    policy: RecoveryPolicy,
    operation: IncomingOperation,
    load: LoadIncomingRequest<Runtime>,
}
impl Fixture {
    fn new(owned: bool, raw_amount: U256, expected_amount: u128, retained: bool) -> Self {
        let raw = hex::decode(RAW).unwrap();
        let tx = H256(sp_io::hashing::keccak_256(&raw));
        let owner = AccountId::new([0x11; 32]);
        let author = if retained {
            AccountId::new([0x31; 32])
        } else {
            owner.clone()
        };
        let timepoint = BridgeMultisig::sidechain_timepoint(100, 0);
        let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            author.clone(),
            tx,
            timepoint,
            Kind::Transfer,
            0,
        ));
        let asset = if owned { common::XOR } else { common::DAI };
        let mut data = owner.encode();
        data.extend(raw_amount.to_big_endian());
        data.extend([0; 12]);
        data.extend([0x22; 20]);
        data.extend(if owned { asset.encode() } else { vec![0; 32] });
        let source:SourceReport=serde_json::from_value(json!({
            "schema":"sora-nexus-offline-evm-hashi-candidate-verification-v1",
            "range":{"schema":"sora-nexus-evm-receipt-range-consistency-v1","declaredChainId":"1","logs":[{"blockNumber":"100","blockHash":hx(&[0x55;32]),"transactionIndex":0,"transactionHash":hx(tx.as_bytes()),"receiptLogIndex":0,"logIndex":4,"address":hx(&[0x41;20]),"topics":[hx(eth_bridge::DEPOSIT_TOPIC.as_bytes())],"data":hx(&data),"status":1}]},
            "extraction":{"scope":{"nativeNetwork":"0","declaredChainId":"1","declaredGenesisHash":hx(&[0x64;32])},"deposits":[{"nativeMessageId":hx(tx.as_bytes()),"transactionHash":hx(tx.as_bytes()),"blockNumber":"100","blockHash":hx(&[0x55;32]),"transactionIndex":0,"logIndex":4,"receiptLogIndex":0,"sourceContract":hx(&[0x41;20]),"rawTopic":hx(eth_bridge::DEPOSIT_TOPIC.as_bytes()),"rawData":hx(&data)}]}
        })).unwrap();
        let transfer = IncomingTransfer::<Runtime> {
            from: H160::zero(),
            to: owner.clone(),
            asset_id: asset,
            asset_kind: if owned {
                AssetKind::Thischain
            } else {
                AssetKind::Sidechain
            },
            amount: expected_amount,
            author: author.clone(),
            tx_hash: tx,
            at_height: 100,
            timepoint,
            network_id: 0,
            should_take_fee: false,
        };
        let evidence = source_evidence(
            0,
            &source.extraction.scope,
            &source.extraction.deposits[0],
            &raw,
        )
        .unwrap();
        let operation = IncomingOperation {
            index: 0,
            channel: IncomingChannel::HashiAuthenticatedSource {
                network: 0,
                request: IncomingRequest::Transfer(transfer).encode(),
                raw_transaction: raw,
                expected_chain_id: Some(1u64.into()),
                approvals: vec![],
            },
            registries: vec![],
            finalized_source_evidence: evidence,
        };
        let mut replacements = vec![AccountReplacement {
            old: owner,
            new: AccountId::new([0x21; 32]),
        }];
        if retained {
            replacements.push(AccountReplacement {
                old: author,
                new: AccountId::new([0x41; 32]),
            });
        }
        let policy = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements,
            council: vec![AccountId::new([0x51; 32])],
            chunk_hashes: vec![operation.commitment()],
            audit_chunk_hashes: vec![[0x52; 32]],
            evidence_commitment: [0x53; 32],
            terminal_audit_commitment: [0x54; 32],
        };
        let row = Row {
            source_report: WorkFile {
                file: "synthetic-unexecuted-source.json".into(),
                bytes: 1,
                sha256: "0".repeat(64),
            },
            network: 0,
            transaction_hash: hx(tx.as_bytes()),
            load_scale: hx(&load.encode()),
            incoming_operation_scale: hx(&operation.encode()),
            load_provenance: if retained {
                LoadProvenance::CheckpointPendingLoad
            } else {
                LoadProvenance::AutomaticEventImport
            },
        };
        Self {
            row,
            source,
            policy,
            operation,
            load,
        }
    }
    fn rebind(&mut self) {
        self.row.incoming_operation_scale = hx(&self.operation.encode());
        self.row.load_scale = hx(&self.load.encode());
        self.policy.chunk_hashes[0] = self.operation.commitment();
    }
    fn change_transfer(&mut self, change: impl FnOnce(&mut IncomingTransfer<Runtime>)) {
        let IncomingChannel::HashiAuthenticatedSource { request, .. } = &mut self.operation.channel
        else {
            unreachable!()
        };
        let IncomingRequest::Transfer(mut t) = decode::<IncomingRequest<Runtime>>(request).unwrap()
        else {
            unreachable!()
        };
        change(&mut t);
        *request = IncomingRequest::Transfer(t).encode();
        self.rebind();
    }
    fn rebind_source(&mut self) {
        let IncomingChannel::HashiAuthenticatedSource {
            raw_transaction, ..
        } = &self.operation.channel
        else {
            unreachable!()
        };
        self.operation.finalized_source_evidence = source_evidence(
            0,
            &self.source.extraction.scope,
            &self.source.extraction.deposits[0],
            raw_transaction,
        )
        .unwrap();
        self.rebind();
    }
    fn retain(&self) {
        eth_bridge::Requests::<Runtime>::insert(
            0,
            self.load.hash(),
            OffchainRequest::LoadIncoming(self.load.clone()),
        );
        eth_bridge::RequestStatuses::<Runtime>::insert(0, self.load.hash(), RequestStatus::Pending);
        eth_bridge::RequestSubmissionHeight::<Runtime>::insert(
            0,
            self.load.hash(),
            recovery::CHECKPOINT_NUMBER,
        );
        eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![self.load.hash()]);
    }
    fn check(&self) -> Result<Value> {
        let before = sp_io::storage::root(StateVersion::V0);
        let out = associate(&self.row, &self.source, &self.policy);
        assert_eq!(sp_io::storage::root(StateVersion::V0), before);
        out
    }
}

#[test]
fn hashi_association_native_precision_floor_and_automatic_author_recipe() {
    let fixture = Fixture::new(false, U256::from(12_345_678u64), 123_456, false);
    let mut ext = sp_io::TestExternalities::new(storage(false, 6, 8, 1));
    ext.execute_with(|| {
        let out = fixture.check().unwrap();
        assert_eq!(out["nativeAmount"], "123456");
        assert_eq!(out["nativeFromPreserved"], hx(&[0; 20]));
        assert_eq!(out["projectedReplacementBeneficiary"], hx(&[0x21; 32]));
        assert_eq!(out["nativeFeeDebited"], "0");
        assert_eq!(out["outerActorIsBeneficialOwner"], false);
    });
}
#[test]
fn hashi_association_retained_author_and_beneficiary_are_independent() {
    let fixture = Fixture::new(false, 7u64.into(), 7, true);
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| {
        fixture.retain();
        let out = fixture.check().unwrap();
        assert_eq!(out["originalAuthor"], hx(&[0x31; 32]));
        assert_eq!(out["originalBeneficiary"], hx(&[0x11; 32]));
        assert_eq!(out["projectedReplacementQueryAuthor"], hx(&[0x41; 32]));
        assert_eq!(out["projectedReplacementBeneficiary"], hx(&[0x21; 32]));
    });
}
#[test]
fn hashi_association_denomination_precedes_u128_conversion() {
    let raw = U256::from(u128::MAX) * U256::from(10u8) + U256::from(7u8);
    let fixture = Fixture::new(true, raw, u128::MAX, false);
    let mut ext = sp_io::TestExternalities::new(storage(true, 18, 18, 10));
    ext.execute_with(|| {
        let out = fixture.check().unwrap();
        assert_eq!(out["nativeAmount"], u128::MAX.to_string());
        assert_eq!(out["checkpointDenominationFactor"], "10");
    });
}
#[test]
fn hashi_association_rejects_wrong_amount_asset_recipient_height_and_fee() {
    for field in 0..5 {
        let mut fixture = Fixture::new(false, 7u64.into(), 7, false);
        fixture.change_transfer(|t| match field {
            0 => t.amount = 8,
            1 => t.asset_id = common::VAL,
            2 => t.to = AccountId::new([0x12; 32]),
            3 => t.at_height = 99,
            _ => t.should_take_fee = true,
        });
        let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
        ext.execute_with(|| assert!(fixture.check().is_err()));
    }
}
#[test]
fn hashi_association_transfer_xor_and_nonpending_retained_loads_require_quarantine() {
    let mut fixture = Fixture::new(false, 7u64.into(), 7, true);
    let load = &fixture.load;
    fixture.load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
        load.author().clone(),
        load.hash(),
        load.timepoint(),
        Kind::TransferXOR,
        0,
    ));
    fixture.rebind();
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| {
        fixture.retain();
        assert!(fixture.check().unwrap_err().contains("TransferXOR"));
    });
    let fixture = Fixture::new(false, 7u64.into(), 7, true);
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| {
        fixture.retain();
        eth_bridge::RequestStatuses::<Runtime>::insert(0, fixture.load.hash(), RequestStatus::Done);
        assert!(fixture.check().unwrap_err().contains("quarantine"));
    });
}
#[test]
fn hashi_association_duplicate_queue_replay_and_wrong_load_author_rejected() {
    let mut fixture = Fixture::new(false, 7u64.into(), 7, true);
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| {
        fixture.retain();
        eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![fixture.load.hash(); 2]);
        assert!(fixture.check().is_err());
        eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![fixture.load.hash()]);
        eth_bridge::LoadToIncomingRequestHash::<Runtime>::insert(
            0,
            fixture.load.hash(),
            H256::repeat_byte(4),
        );
        assert!(fixture.check().is_err());
    });
    let load = &fixture.load;
    fixture.load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
        AccountId::new([0x12; 32]),
        load.hash(),
        load.timepoint(),
        Kind::Transfer,
        0,
    ));
    fixture.rebind();
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| assert!(fixture.check().unwrap_err().contains("author")));
}
#[test]
fn hashi_association_unknown_contract_and_multiple_logs_never_choose_first() {
    let mut fixture = Fixture::new(false, 7u64.into(), 7, false);
    fixture.source.range.logs[0].address = hx(&[0x42; 20]);
    fixture.source.extraction.deposits[0].source_contract = hx(&[0x42; 20]);
    fixture.rebind_source();
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| assert!(fixture.check().is_err()));
    let mut fixture = Fixture::new(false, 7u64.into(), 7, false);
    fixture.source.extraction.deposits.push(DepositLocator {
        native_message_id: fixture.row.transaction_hash.clone(),
        transaction_hash: fixture.row.transaction_hash.clone(),
        block_hash: hx(&[0x55; 32]),
        block_number: "100".into(),
        transaction_index: 0,
        log_index: 5,
        receipt_log_index: 1,
        source_contract: hx(&[0x41; 20]),
        raw_topic: hx(eth_bridge::DEPOSIT_TOPIC.as_bytes()),
        raw_data: fixture.source.extraction.deposits[0].raw_data.clone(),
    });
    let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
    ext.execute_with(|| assert!(fixture.check().unwrap_err().contains("exactly one")));
}
#[test]
fn hashi_association_source_proof_domain_hash_evidence_and_policy_are_bound() {
    for field in 0..4 {
        let mut fixture = Fixture::new(false, 7u64.into(), 7, false);
        match field {
            0 => {
                let IncomingChannel::HashiAuthenticatedSource {
                    raw_transaction, ..
                } = &mut fixture.operation.channel
                else {
                    unreachable!()
                };
                raw_transaction[2] ^= 1
            }
            1 => fixture.operation.finalized_source_evidence[0] ^= 1,
            2 => fixture.source.range.declared_chain_id = "2".into(),
            _ => fixture.policy.chunk_hashes[0] = [0; 32],
        };
        if field != 3 {
            fixture.rebind();
        }
        let mut ext = sp_io::TestExternalities::new(storage(false, 18, 18, 1));
        ext.execute_with(|| assert!(fixture.check().is_err()));
    }
}
#[test]
fn hashi_association_zero_denominator_and_precision_overflow_fail_without_writes() {
    let fixture = Fixture::new(true, 7u64.into(), 7, false);
    let mut ext = sp_io::TestExternalities::new(storage(true, 18, 18, 0));
    ext.execute_with(|| assert!(fixture.check().is_err()));
    let fixture = Fixture::new(false, 7u64.into(), 7, false);
    let mut ext = sp_io::TestExternalities::new(storage(false, 255, 0, 1));
    ext.execute_with(|| assert!(fixture.check().is_err()));
}
#[test]
fn hashi_association_evidence_codec_matches_frozen_python_composition() {
    let value: Value = serde_json::from_str(include_str!("hashi_association_golden.json")).unwrap();
    let source: SourceReport = serde_json::from_value(value["sourceReport"].clone()).unwrap();
    let raw = hex_bytes(value["rawTransaction"].as_str().unwrap()).unwrap();
    let evidence = source_evidence(
        0,
        &source.extraction.scope,
        &source.extraction.deposits[0],
        &raw,
    )
    .unwrap();
    assert_eq!(hx(&evidence), value["expectedEvidenceHash"]);
}
