use super::mock::*;
use crate::offchain::SignatureParams;
use crate::requests::{
    IncomingTransactionRequestKind, LoadIncomingRequest, LoadIncomingTransactionRequest,
    OffchainRequest, OutgoingAddAsset, OutgoingMigrate, OutgoingRequest, RequestStatus,
};
use crate::{BridgeSignatureVersion, BridgeSignatureVersions, Requests};
use codec::Encode;
use common::{mock::alice, AssetName, AssetSymbol, XOR};
use frame_support::{
    assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::Get,
    weights::Weight,
};
use sp_core::{ecdsa, H160, H256};
use std::collections::BTreeSet;

fn fallback() -> u64 {
    let limits: frame_system::limits::BlockLength =
        <Runtime as frame_system::Config>::BlockLength::get();
    2 * u64::from(*limits.max.get(DispatchClass::Operational))
}

fn migration(tokens: usize) -> OffchainRequest<Runtime> {
    OffchainRequest::outgoing(OutgoingRequest::Migrate(OutgoingMigrate {
        author: alice(),
        new_contract_address: H160::repeat_byte(42),
        erc20_native_tokens: (0..tokens)
            .map(|index| H160::from_low_u64_be(index as u64 + 1))
            .collect(),
        nonce: 0,
        network_id: 0,
        timepoint: Default::default(),
        new_signature_version: BridgeSignatureVersion::V3,
    }))
}

fn declared_weights(hash: H256) -> (Weight, Weight) {
    (
        crate::Call::<Runtime>::approve_request {
            ocw_public: ecdsa::Public::from_raw([0; 33]),
            hash,
            signature_params: SignatureParams::default(),
            network_id: 0,
        }
        .get_dispatch_info()
        .call_weight,
        crate::Call::<Runtime>::finalize_approved_request {
            network_id: 0,
            hash,
        }
        .get_dispatch_info()
        .call_weight,
    )
}

#[test]
fn approval_byte_bound_covers_migration_abi_without_decoding_the_request() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let mut previous_weights = None;
        for tokens in [0, 1, 64, 4096] {
            let request = migration(tokens);
            let hash = request.hash();
            let bytes = request.encode();
            assert_eq!(bytes[0], 0, "Outgoing SCALE tag is part of the bound");
            Requests::<Runtime>::insert(0, hash, &request);
            let bound = EthBridge::approval_message_byte_bound(0, hash);
            assert_eq!(bound, 2 * bytes.len() as u64 + 1024);
            for version in [
                BridgeSignatureVersion::V1,
                BridgeSignatureVersion::V2,
                BridgeSignatureVersion::V3,
            ] {
                BridgeSignatureVersions::<Runtime>::insert(0, version);
                let actual = request.as_outgoing().unwrap().0.to_eth_abi(hash).unwrap();
                assert!(actual.as_raw().len() as u64 <= bound);
            }
            let weights = declared_weights(hash);
            if let Some((approval, finalize)) = previous_weights {
                assert!(weights.0.ref_time() > approval);
                assert!(weights.1.ref_time() > finalize);
            } else {
                assert!(
                    bound < fallback(),
                    "small approvals avoid the global envelope"
                );
            }
            previous_weights = Some((weights.0.ref_time(), weights.1.ref_time()));
        }
    });
}

#[test]
fn approval_byte_bound_uses_global_fallback_for_absent_empty_and_non_outgoing_values() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let hash = H256::repeat_byte(8);
        let key = Requests::<Runtime>::hashed_key_for(0, hash);
        let expected_weights = declared_weights(hash);
        assert_eq!(EthBridge::approval_message_byte_bound(0, hash), fallback());
        for bytes in [vec![], vec![1], vec![2], vec![255], vec![1; 8192]] {
            sp_io::storage::set(&key, &bytes);
            assert_eq!(EthBridge::approval_message_byte_bound(0, hash), fallback());
            assert_eq!(declared_weights(hash), expected_weights);
        }
    });
}

#[test]
fn public_hint_at_future_outgoing_hash_cannot_supply_a_small_declaration() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let request = migration(4096);
        let hash = request.hash();
        let hint = OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                alice(),
                hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                0,
            ),
        ));
        Requests::<Runtime>::insert(0, hash, hint);
        crate::RequestStatuses::<Runtime>::insert(
            0,
            hash,
            RequestStatus::Failed(crate::Error::<Runtime>::UnknownEvent.into()),
        );
        assert_eq!(EthBridge::approval_message_byte_bound(0, hash), fallback());
        Requests::<Runtime>::insert(0, hash, &request);
        BridgeSignatureVersions::<Runtime>::insert(0, BridgeSignatureVersion::V3);
        let actual = request.as_outgoing().unwrap().0.to_eth_abi(hash).unwrap();
        assert!(EthBridge::approval_message_byte_bound(0, hash) >= actual.as_raw().len() as u64);
        assert_eq!(
            EthBridge::approval_message_byte_bound(0, hash),
            2 * request.encoded_size() as u64 + 1024,
        );
    });
}

#[test]
fn approval_declarations_keep_maximum_recovery_allowance_as_stored_count_changes() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let request = migration(0);
        let hash = request.hash();
        Requests::<Runtime>::insert(0, hash, request);
        let before = declared_weights(hash);
        for count in [0, 1, crate::MAX_PEERS, crate::MAX_PEERS + 1] {
            let approvals = (0..count)
                .map(|index| {
                    let mut r = [0; 32];
                    r[..8].copy_from_slice(&(index as u64).to_le_bytes());
                    SignatureParams {
                        r,
                        s: [1; 32],
                        v: 0,
                    }
                })
                .collect::<BTreeSet<_>>();
            crate::RequestApprovals::<Runtime>::insert(0, hash, approvals);
            assert_eq!(declared_weights(hash), before);
        }
        let maximum = EthBridge::approval_reconciliation_weight_for(crate::MAX_PEERS as u32);
        assert!(before.0.all_gte(maximum.saturating_mul(3)));
        assert!(before.1.all_gte(maximum));
    });
}

#[test]
fn approval_byte_margin_covers_later_asset_metadata_and_signature_version_changes() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let request = OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset {
            author: alice(),
            asset_id: XOR,
            nonce: 0,
            network_id: 0,
            timepoint: Default::default(),
        }));
        let hash = request.hash();
        Requests::<Runtime>::insert(0, hash, &request);
        let bound = EthBridge::approval_message_byte_bound(0, hash);
        for (symbol, name) in [("A".into(), "A".into()), ("A".repeat(7), "A".repeat(33))] {
            assert_ok!(Assets::update_info(
                RuntimeOrigin::root(),
                XOR,
                Some(AssetSymbol(symbol.into_bytes())),
                Some(AssetName(name.into_bytes())),
            ));
            for version in [
                BridgeSignatureVersion::V1,
                BridgeSignatureVersion::V2,
                BridgeSignatureVersion::V3,
            ] {
                BridgeSignatureVersions::<Runtime>::insert(0, version);
                let encoded = request.as_outgoing().unwrap().0.to_eth_abi(hash).unwrap();
                assert!(encoded.as_raw().len() as u64 <= bound);
                assert_eq!(EthBridge::approval_message_byte_bound(0, hash), bound);
            }
        }
    });
}
