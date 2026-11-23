use super::mock::*;
use crate::requests::*;
use crate::{AccountRequests, LoadToIncomingRequestHash, RequestStatuses, Requests};
use common::mock::alice;
use common::AssetInfoProvider;
use sp_core::{H160, H256};

fn load() -> OffchainRequest<Runtime> {
    OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
        LoadIncomingTransactionRequest {
            author: alice(),
            hash: H256::repeat_byte(31),
            kind: IncomingTransactionRequestKind::Transfer,
            timepoint: Default::default(),
            network_id: 0,
        },
    ))
}

#[test]
fn completed_load_queries_redirect_to_current_incoming_state_and_handle_missing_targets() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let load = load();
        let incoming = OffchainRequest::incoming(IncomingRequest::Transfer(IncomingTransfer {
            from: H160::repeat_byte(3),
            to: alice(),
            asset_id: common::VAL,
            asset_kind: AssetKind::Thischain,
            amount: 10,
            author: alice(),
            tx_hash: load.hash(),
            at_height: 8,
            timepoint: Default::default(),
            network_id: 0,
            should_take_fee: false,
        }));
        let hash = load.hash();
        let incoming_hash = incoming.hash();
        Requests::<Runtime>::insert(0, hash, load.clone());
        Requests::<Runtime>::insert(0, incoming_hash, incoming.clone());
        RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Done);
        LoadToIncomingRequestHash::<Runtime>::insert(0, hash, incoming_hash);
        for network in [Some(0), None] {
            assert_eq!(
                EthBridge::get_requests(&[hash], network, false).unwrap(),
                vec![(load.clone(), RequestStatus::Done)]
            );
            // Legacy incoming records can lack a status; RPC defaults these to pending.
            assert_eq!(
                EthBridge::get_requests(&[hash], network, true).unwrap(),
                vec![(incoming.clone(), RequestStatus::Pending)]
            );
        }
        RequestStatuses::<Runtime>::insert(0, incoming_hash, RequestStatus::Done);
        for network in [Some(0), None] {
            assert_eq!(
                EthBridge::get_requests(&[hash], network, true).unwrap(),
                vec![(incoming.clone(), RequestStatus::Done)]
            );
        }
        Requests::<Runtime>::remove(0, incoming_hash);
        for network in [Some(0), None] {
            assert!(EthBridge::get_requests(&[hash], network, true)
                .unwrap()
                .is_empty());
        }
        RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        for network in [Some(0), None] {
            assert_eq!(
                EthBridge::get_requests(&[hash], network, true).unwrap(),
                vec![(load.clone(), RequestStatus::Pending)]
            );
        }
    });
}

#[test]
fn approval_queries_never_export_incoming_loads_even_with_an_approval_status() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let load = load();
        let hash = load.hash();
        Requests::<Runtime>::insert(0, hash, load);
        RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::ApprovalsReady);
        for network in [Some(0), None] {
            assert!(EthBridge::get_approved_requests(&[hash], network)
                .unwrap()
                .is_empty());
        }
        let missing = H256::repeat_byte(32);
        AccountRequests::<Runtime>::insert(alice(), vec![(0, hash), (0, missing)]);
        assert_eq!(
            EthBridge::get_account_requests(&alice(), Some(RequestStatus::ApprovalsReady)).unwrap(),
            vec![(0, hash)]
        );
        assert!(
            EthBridge::get_account_requests(&alice(), Some(RequestStatus::Done))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            EthBridge::get_account_requests(&alice(), None).unwrap(),
            vec![(0, hash), (0, missing)]
        );
    });
}

#[test]
fn networkless_asset_query_includes_each_registered_network() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let token = H160::repeat_byte(92);
        crate::RegisteredAsset::<Runtime>::insert(1, common::VAL, AssetKind::Sidechain);
        crate::RegisteredSidechainToken::<Runtime>::insert(1, common::VAL, token);
        crate::SidechainAssetPrecision::<Runtime>::insert(1, common::VAL, 6);
        let foreign_asset = (
            AssetKind::Sidechain,
            (
                common::VAL,
                assets::Pallet::<Runtime>::get_asset_info(&common::VAL).2,
            ),
            Some((token, 6)),
        );
        let network_assets = EthBridge::get_registered_assets(Some(0)).unwrap();
        let all_assets = EthBridge::get_registered_assets(None).unwrap();
        assert!(!network_assets.is_empty());
        assert!(all_assets.contains(&foreign_asset));
        assert!(!network_assets.contains(&foreign_asset));
        for asset in network_assets {
            assert!(all_assets.contains(&asset));
        }
        assert!(EthBridge::get_registered_assets(Some(100))
            .unwrap()
            .is_empty());
    });
}

#[test]
fn global_approval_queries_hide_decommissioned_legacy_xor_evidence() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let request = OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer {
            from: alice(),
            to: H160::repeat_byte(2),
            asset_id: common::XOR,
            amount: 1,
            nonce: 0,
            network_id: 0,
            timepoint: Default::default(),
        }));
        let hash = request.hash();
        Requests::<Runtime>::insert(0, hash, request);
        RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::ApprovalsReady);
        crate::RequestApprovals::<Runtime>::insert(
            0,
            hash,
            std::collections::BTreeSet::from([crate::offchain::SignatureParams {
                r: [1; 32],
                s: [2; 32],
                v: 27,
            }]),
        );
        crate::LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        for network in [None, Some(0)] {
            assert!(EthBridge::get_approved_requests(&[hash], network)
                .unwrap()
                .is_empty());
            assert!(EthBridge::get_approvals(&[hash], network)
                .unwrap()
                .iter()
                .all(Vec::is_empty));
        }
        assert_eq!(
            RequestStatuses::<Runtime>::get(0, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert_eq!(crate::RequestApprovals::<Runtime>::get(0, hash).len(), 1);
        assert_eq!(
            crate::recovery::ensure_incoming_transaction_active::<Runtime>(0, hash),
            Ok(())
        );
        sp_io::storage::set(
            &bridge_types::recovery::message_key(
                bridge_types::GenericNetworkId::EVMLegacy(0),
                hash,
            ),
            &[1],
        );
        assert_eq!(
            crate::recovery::ensure_incoming_transaction_active::<Runtime>(0, hash),
            Err(crate::Error::<Runtime>::RecoveryHalted)
        );
        assert_eq!(
            crate::recovery::ensure_incoming_transaction_active::<Runtime>(1, hash),
            Ok(())
        );
    });
}
