//! Emergency halts affect workers, not consensus bridge admission.

use super::mock::*;
use super::{last_outgoing_request, ETH_NETWORK_ID};
use crate::{IncomingTransactionRequestKind, RequestStatus};
use codec::Encode;
use common::recovery::{self, bridge_admission_fixtures as fixtures};
use common::PSWAP;
use frame_support::{assert_err, assert_noop, assert_ok};
use sp_core::{ecdsa, sr25519, H160, H256};

#[test]
fn ocw_only_halt_completed_recovery_accepts_incoming_load_with_both_signals_present() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        fixtures::complete([1; 32]);
        sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        sp_io::storage::set(common::bridge_halt::GUARDIAN_HALT_STORAGE_KEY, &[255]);
        let who = get_account_id_from_seed::<sp_core::sr25519::Public>("Alice");
        let hash = H256::repeat_byte(71);
        assert_ok!(EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(who),
            hash,
            IncomingTransactionRequestKind::Transfer.into(),
            ETH_NETWORK_ID,
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(ETH_NETWORK_ID, hash),
            Some(RequestStatus::Pending)
        );
        assert!(crate::offchain::stopped_by_guardian_halt());
        assert!(crate::offchain::stopped_by_recovery::<Runtime>());
    });
}

#[test]
fn ocw_only_halt_recovery_acknowledgement_is_local_and_does_not_cover_next_guardian_halt() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        fixtures::complete([1; 32]);
        sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        let completion = bridge_types::recovery::completion().unwrap().unwrap();
        let genesis: [u8; 32] = System::block_hash(0).into();
        let ack = bridge_types::recovery::RecoveryAcknowledgement::new(genesis, &completion);
        assert!(crate::offchain::stopped_by_recovery::<Runtime>());
        sp_io::offchain::local_storage_set(
            sp_core::offchain::StorageKind::PERSISTENT,
            bridge_types::recovery::RECOVERY_ACK_STORAGE_KEY,
            &ack.encode(),
        );
        assert!(!crate::offchain::stopped_by_recovery::<Runtime>());
        for marker in [vec![], vec![0], vec![255]] {
            sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &marker);
            assert!(!crate::offchain::stopped_by_recovery::<Runtime>());
        }
        sp_io::storage::set(
            common::bridge_halt::GUARDIAN_HALT_STORAGE_KEY,
            &common::bridge_halt::GuardianHalt {
                number: 1,
                block: 1,
            }
            .encode(),
        );
        assert!(crate::offchain::stopped_by_guardian_halt());
        let mut status = recovery::storage::status().unwrap().unwrap();
        status.operation_counter += 1;
        sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
        assert!(crate::offchain::stopped_by_recovery::<Runtime>());
    });
}

#[test]
fn ocw_only_halt_unfinished_recovery_and_forged_maintenance_context_fail_closed() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        fixtures::incomplete([1; 32]);
        sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
        let who = get_account_id_from_seed::<sp_core::sr25519::Public>("Alice");
        assert_noop!(
            EthBridge::request_from_sidechain(
                RuntimeOrigin::signed(who),
                H256::repeat_byte(72),
                IncomingTransactionRequestKind::Transfer.into(),
                ETH_NETWORK_ID,
            ),
            crate::Error::<Runtime>::RecoveryHalted
        );
        assert!(crate::offchain::stopped_by_recovery::<Runtime>());
        fixtures::complete([1; 32]);
        sp_io::storage::set(
            recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
            &System::block_number().encode(),
        );
        assert!(!crate::recovery::maintenance_inbound_active::<Runtime>());
        assert!(!crate::recovery::inbound_allowed::<Runtime>());
        assert_eq!(
            crate::recovery::ensure_ordinary_transfer_source::<Runtime>(&H160::zero()),
            Err(crate::Error::<Runtime>::InvalidAddress)
        );
    });
}

#[test]
fn ocw_only_halt_completion_never_removes_permanent_request_quarantine() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        fixtures::complete([1; 32]);
        let hash = H256::repeat_byte(73);
        for key in [
            bridge_types::recovery::hashi_key(ETH_NETWORK_ID, hash),
            bridge_types::recovery::message_key(
                bridge_types::GenericNetworkId::EVMLegacy(ETH_NETWORK_ID),
                hash,
            ),
        ] {
            sp_io::storage::set(&key, &[0]);
            assert!(
                crate::recovery::ensure_outgoing_active::<Runtime>(ETH_NETWORK_ID, hash).is_err()
            );
            sp_io::storage::clear(&key);
        }
    });
}

#[test]
fn ocw_only_halt_rejects_stored_requests_with_another_embedded_network() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(Assets::mint_to(&PSWAP, &alice, &alice, 1_000));
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            PSWAP,
            H160::repeat_byte(25),
            100,
            0
        ));
        let (request, hash) = last_outgoing_request(0).unwrap();
        let crate::requests::OutgoingRequest::Transfer(mut transfer) = request else {
            panic!("transfer fixture");
        };
        transfer.network_id = 1;
        crate::Requests::<Runtime>::insert(
            0,
            hash,
            crate::requests::OffchainRequest::Outgoing(
                crate::requests::OutgoingRequest::Transfer(transfer),
                hash,
            ),
        );
        fixtures::complete([1; 32]);
        let (_, peer, _) = &state.networks[&0].ocw_keypairs[0];
        let call = crate::Call::<Runtime>::approve_request {
            ocw_public: ecdsa::Public::from_raw([2; 33]),
            hash,
            signature_params: Default::default(),
            network_id: 0,
        };
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &call),
            crate::Error::<Runtime>::UnknownNetwork
        );
        assert_noop!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(peer.clone()),
                ecdsa::Public::from_raw([2; 33]),
                hash,
                Default::default(),
                0
            )
            .map_err(|error| error.error),
            crate::Error::<Runtime>::UnknownNetwork
        );
        let mut cached = crate::offchain::SignedTransactionData::<Runtime>::new(
            H256::repeat_byte(26),
            None,
            call,
        );
        let signer = EthBridge::get_signer().unwrap();
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(!cached.resend(&signer));
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert!(state.pool_state.read().transactions.is_empty());

        let incoming = crate::requests::OffchainRequest::incoming(
            crate::requests::IncomingRequest::PrepareForMigration(
                crate::IncomingPrepareForMigration {
                    author: alice,
                    tx_hash: H256::repeat_byte(27),
                    at_height: 1,
                    timepoint: Default::default(),
                    network_id: 1,
                },
            ),
        );
        let incoming_hash = incoming.hash();
        crate::Requests::<Runtime>::insert(0, incoming_hash, incoming);
        crate::RequestStatuses::<Runtime>::insert(
            0,
            incoming_hash,
            crate::requests::RequestStatus::Pending,
        );
        let bridge = crate::BridgeAccount::<Runtime>::get(0).unwrap();
        let call = crate::Call::<Runtime>::finalize_incoming_request {
            hash: incoming_hash,
            network_id: 0,
        };
        assert_err!(
            EthBridge::validate_peer_protocol_call(&bridge, &call),
            crate::Error::<Runtime>::UnknownNetwork
        );
        assert_noop!(
            EthBridge::finalize_incoming_request(RuntimeOrigin::signed(bridge), incoming_hash, 0),
            crate::Error::<Runtime>::UnknownNetwork
        );
    });
}
