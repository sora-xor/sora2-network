//! Public synthetic source observations, never receipt inclusion or finality proof.
use super::*;
use crate::incident_recovery_incoming::tests::{hashi_fixture, sign_hashi};
use crate::{AccountId, RuntimeCall, System};
use codec::Decode;
use eth_bridge::requests::AssetKind;
use eth_bridge_runtime_api::recovery::RecoveryHashiSourceLog;
use frame_support::dispatch::GetDispatchInfo;
use sp_runtime::StateVersion;

// Public EIP-155 example. Its outer target is 0x3535..., deliberately distinct
// from the default event emitter to test native automatic routed deposits.
const RAW: &str = "f86c098504a817c800825208943535353535353535353535353535353535353535880de0b6b3a76400008025a028ef61340bd939bc2195fe537567866003e1a15d3c71ff63e1590620aa636276a067cbe9d8997f761aecb703304b3800ccf555c9f3dc64214b297fb1966a3b6d83";

fn value_key(item: &[u8], network: u32) -> Vec<u8> {
    let encoded = network.encode();
    [
        frame_support::storage::storage_prefix(b"EthBridge", item).as_slice(),
        sp_io::hashing::twox_64(&encoded).as_slice(),
        encoded.as_slice(),
    ]
    .concat()
}

pub(crate) fn fixture() -> (
    recovery::RecoveryPolicy,
    IncomingOperation,
    RecoveryHashiSource,
) {
    let (mut policy, mut operation, keys) = hashi_fixture(false);
    frame_system::BlockHash::<Runtime>::insert(0, H256(recovery::GENESIS_HASH));
    System::set_parent_hash(H256::repeat_byte(44));
    sp_io::storage::set(b":code", b"public synthetic stored code");
    let IncomingChannel::Hashi {
        request, approvals, ..
    } = &mut operation.channel
    else {
        unreachable!()
    };
    let original = IncomingRequest::<Runtime>::decode(&mut request.as_slice()).unwrap();
    let IncomingRequest::Transfer(mut transfer) = original else {
        unreachable!()
    };
    let raw: Vec<u8> = RAW
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    transfer.from = H160::zero();
    transfer.tx_hash = H256(sp_io::hashing::keccak_256(&raw));
    transfer.author = transfer.to.clone();
    transfer.timepoint = BridgeMultisig::sidechain_timepoint(123, 0);
    let load = LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
        transfer.author.clone(),
        transfer.tx_hash,
        transfer.timepoint,
        Kind::Transfer,
        0,
    ));
    let request = IncomingRequest::Transfer(transfer.clone()).encode();
    let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
        load_incoming_request: load.clone(),
        incoming_request_result: Ok(IncomingRequest::Transfer(transfer.clone())),
    });
    for (approval, key) in approvals.iter_mut().zip(&keys) {
        let mut call = RuntimeCall::decode(&mut approval.call.as_slice()).unwrap();
        let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
            maybe_timepoint,
            call: nested,
            max_weight,
            ..
        }) = &mut call
        else {
            unreachable!()
        };
        *maybe_timepoint = Some(transfer.timepoint);
        *nested = inner.encode();
        *max_weight = inner.get_dispatch_info().total_weight();
        approval.call = call.encode();
        sign_hashi(approval, key);
    }
    let approvals = approvals.clone();
    let mut data = transfer.to.encode();
    data.extend(U256::from(transfer.amount).to_big_endian());
    data.extend([0; 12]);
    data.extend([0x22; 20]);
    data.extend([0; 32]);
    let source = RecoveryHashiSource {
        format_version: 1,
        block_hash: [0x55; 32],
        block_number: 123,
        transaction_hash: *transfer.tx_hash.as_fixed_bytes(),
        transaction_index: 0,
        status: 1,
        logs: vec![RecoveryHashiSourceLog {
            address: [0x41; 20],
            topics: vec![*eth_bridge::DEPOSIT_TOPIC.as_fixed_bytes()],
            data,
            log_index: 4,
        }],
        load_scale: load.encode(),
    };
    sp_io::storage::set(
        &value_key(b"BridgeContractAddress", 0),
        &H160::repeat_byte(0x41).encode(),
    );
    let token = H160::repeat_byte(0x22).encode();
    let mut key = value_key(b"RegisteredSidechainAsset", 0);
    key.extend(sp_io::hashing::blake2_128(&token));
    key.extend(token);
    sp_io::storage::set(&key, &common::DAI.encode());
    let mut key = value_key(b"SidechainAssetPrecision", 0);
    key.extend(common::DAI.encode());
    sp_io::storage::set(&key, &18u8.encode());
    operation.finalized_source_evidence = source_commitment(&source, 0, &raw);
    operation.channel = IncomingChannel::HashiAuthenticatedSource {
        network: 0,
        request,
        raw_transaction: raw,
        expected_chain_id: Some(U256::one()),
        approvals,
    };
    policy.chunk_hashes = vec![operation.commitment()];
    recovery::storage::initialize(&policy).unwrap();
    recovery::storage::record_recovery_anchor([44; 32]).unwrap();
    (policy, operation, source)
}

fn rebind(operation: &mut IncomingOperation, source: &RecoveryHashiSource) {
    let IncomingChannel::HashiAuthenticatedSource {
        raw_transaction, ..
    } = &operation.channel
    else {
        unreachable!()
    };
    operation.finalized_source_evidence = source_commitment(source, 0, raw_transaction);
}

#[test]
fn source_native_association_preserves_original_owner_and_all_state() {
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| {
        let (_, operation, source) = fixture();
        let before = sp_io::storage::root(StateVersion::V0);
        let result = associate(&operation, &source.encode()).unwrap();
        assert_eq!(
            result.load_provenance,
            RecoveryHashiLoadProvenance::AutomaticEventImport
        );
        assert_eq!(result.authenticated_outer_destination, Some([0x35; 20]));
        assert_eq!(result.source_contract, [0x41; 20]);
        assert!(result.complete_original_request_matches);
        assert!(!result.source_inclusion_verified && !result.external_finality_verified);
        assert_eq!(sp_io::storage::root(StateVersion::V0), before);
    });
    assert_eq!(
        ext.overlayed_changes()
            .offchain()
            .overlay()
            .changes()
            .count(),
        0
    );
}

#[test]
fn source_preparation_runs_native_payloads_only_after_association_in_rollback() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, source) = fixture();
        crate::incident_recovery::with_test_policy(&policy, || {
            let header = super::super::tests::parent(&policy);
            let before = sp_io::storage::root(StateVersion::V0);
            let context = context(&header.encode(), &operation.encode()).unwrap();
            let result = rollback(|| {
                let association = associate(&context.operation, &source.encode())?;
                let preparation = prepare_native(&context)?;
                Ok(RecoveryHashiSourcePreparation {
                    preparation,
                    association,
                })
            })
            .unwrap();
            assert_eq!(result.preparation.approvals.len(), 3);
            assert_eq!(
                result.preparation.source_evidence_commitment,
                operation.finalized_source_evidence
            );
            assert_eq!(sp_io::storage::root(StateVersion::V0), before);
        });
    });
}

#[test]
fn source_rejects_complete_original_request_mismatches_before_replacement() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, operation, source) = fixture();
        for field in 0..8 {
            let mut changed = operation.clone();
            let IncomingChannel::HashiAuthenticatedSource { request, .. } = &mut changed.channel
            else {
                unreachable!()
            };
            let mut original = IncomingRequest::<Runtime>::decode(&mut request.as_slice()).unwrap();
            let IncomingRequest::Transfer(transfer) = &mut original else {
                unreachable!()
            };
            match field {
                0 => transfer.amount += 1,
                1 => transfer.to = AccountId::new([8; 32]), // actual replacement must not enter original bytes
                2 => transfer.asset_id = common::XOR,
                3 => transfer.asset_kind = AssetKind::Thischain,
                4 => transfer.author = AccountId::new([8; 32]),
                5 => transfer.at_height += 1,
                6 => transfer.should_take_fee = true,
                _ => transfer.timepoint = BridgeMultisig::sidechain_timepoint(124, 0),
            }
            *request = original.encode();
            assert!(
                associate(&changed, &source.encode()).is_err(),
                "field {field}"
            );
        }
    });
}

#[test]
fn source_rejects_failed_duplicate_noncanonical_unknown_or_unbound_logs() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, operation, source) = fixture();
        for case in 0..8 {
            let mut changed = source.clone();
            match case {
                0 => changed.status = 0,
                1 => {
                    let mut extra = changed.logs[0].clone();
                    extra.log_index += 1;
                    changed.logs.push(extra);
                }
                2 => changed.logs[0].data[64] = 1,
                3 => changed.logs[0].topics.push([0; 32]),
                4 => changed.logs[0].address = [0x77; 20],
                5 => changed.block_hash = [0x66; 32],
                6 => changed.transaction_index = 1,
                _ => {
                    let mut extra = changed.logs[0].clone();
                    extra.topics.clear();
                    extra.log_index += 2;
                    changed.logs.push(extra);
                }
            }
            assert!(
                associate(&operation, &changed.encode()).is_err(),
                "case {case}"
            );
        }
        let mut trailing = source.encode();
        trailing.push(0);
        assert!(associate(&operation, &trailing).is_err());
    });
}

fn retain(operation: &IncomingOperation, source: &RecoveryHashiSource) {
    let load = LoadIncomingRequest::<Runtime>::decode(&mut source.load_scale.as_slice()).unwrap();
    let hash = H256(source.transaction_hash);
    eth_bridge::Requests::<Runtime>::insert(0, hash, OffchainRequest::LoadIncoming(load));
    eth_bridge::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
    eth_bridge::RequestSubmissionHeight::<Runtime>::insert(0, hash, recovery::CHECKPOINT_NUMBER);
    eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![hash]);
    assert_ne!(operation.finalized_source_evidence, [0; 32]);
}

#[test]
fn source_retained_pending_load_requires_authenticated_outer_target() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, mut operation, mut source) = fixture();
        retain(&operation, &source);
        assert!(associate(&operation, &source.encode()).is_err());
        sp_io::storage::set(
            &value_key(b"BridgeContractAddress", 0),
            &H160::repeat_byte(0x35).encode(),
        );
        source.logs[0].address = [0x35; 20];
        rebind(&mut operation, &source);
        let result = associate(&operation, &source.encode()).unwrap();
        assert_eq!(
            result.load_provenance,
            RecoveryHashiLoadProvenance::CheckpointPending {
                submission_height: recovery::CHECKPOINT_NUMBER
            }
        );
        eth_bridge::RequestStatuses::<Runtime>::insert(
            0,
            H256(source.transaction_hash),
            RequestStatus::Done,
        );
        assert!(associate(&operation, &source.encode()).is_err());
    });
}

#[test]
fn source_replay_and_partial_load_identities_do_not_become_automatic_imports() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, operation, source) = fixture();
        let tx = H256(source.transaction_hash);
        eth_bridge::RequestStatuses::<Runtime>::insert(0, tx, RequestStatus::Done);
        assert!(associate(&operation, &source.encode()).is_err());
        eth_bridge::RequestStatuses::<Runtime>::remove(0, tx);
        eth_bridge::LoadToIncomingRequestHash::<Runtime>::insert(0, tx, H256::repeat_byte(9));
        assert!(associate(&operation, &source.encode()).is_err());
        eth_bridge::LoadToIncomingRequestHash::<Runtime>::remove(0, tx);
        let incoming = associate(&operation, &source.encode())
            .unwrap()
            .native_incoming_hash;
        eth_bridge::RequestStatuses::<Runtime>::insert(0, H256(incoming), RequestStatus::Pending);
        assert!(associate(&operation, &source.encode()).is_err());
    });
}

#[test]
fn source_native_precision_and_denominator_are_not_caller_selected() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, operation, source) = fixture();
        let mut key = value_key(b"SidechainAssetPrecision", 0);
        key.extend(common::DAI.encode());
        sp_io::storage::set(&key, &6u8.encode());
        assert!(associate(&operation, &source.encode()).is_err());
    });
}
