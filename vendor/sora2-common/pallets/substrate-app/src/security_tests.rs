use crate::{mock::*, AssetKinds, Error, PendingAssetRegistrations, SidechainAssetId, SidechainPrecision};
use bridge_types::{traits::BridgeOriginOutput, types::AssetKind, GenericAssetId, GenericTimepoint, SubNetworkId};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

fn origin() -> RuntimeOrigin {
    dispatch::RawOrigin::new(BridgeOriginOutput::new(
        SubNetworkId::Liberland, H256::repeat_byte(7), GenericTimepoint::Unknown, (),
    )).into()
}

#[test]
fn remote_registration_cannot_create_or_reclassify_native_assets() {
    new_tester_no_registered_assets().execute_with(|| {
        let network = SubNetworkId::Liberland;
        let remote = GenericAssetId::Sora(H256::zero());
        assert_noop!(SubstrateApp::finalize_asset_registration(
            origin(), AssetId::Xor, remote, AssetKind::Sidechain, 18,
        ), Error::<Test>::AssetRegistrationNotAuthorized);
        assert_noop!(SubstrateApp::incoming_thischain_asset_registration(
            origin(), AssetId::Xor, remote,
        ), Error::<Test>::AssetRegistrationNotAuthorized);

        assert_ok!(SubstrateApp::authorize_asset_registration(
            RuntimeOrigin::root(), network, AssetId::Xor, remote, AssetKind::Thischain,
        ));
        assert_noop!(SubstrateApp::finalize_asset_registration(
            origin(), AssetId::Xor, remote, AssetKind::Sidechain, 18,
        ), Error::<Test>::AssetRegistrationNotAuthorized);
        assert_ok!(SubstrateApp::incoming_thischain_asset_registration(
            origin(), AssetId::Xor, remote,
        ));
        assert_eq!(AssetKinds::<Test>::get(network, AssetId::Xor), Some(AssetKind::Thischain));
        assert!(!PendingAssetRegistrations::<Test>::contains_key(network, AssetId::Xor));
        let precision = SidechainPrecision::<Test>::get(network, AssetId::Xor).unwrap();
        assert_ok!(SubstrateApp::finalize_asset_registration(
            origin(), AssetId::Xor, remote, AssetKind::Thischain, precision,
        ));
        assert_noop!(SubstrateApp::finalize_asset_registration(
            origin(), AssetId::Xor, remote, AssetKind::Sidechain, precision,
        ), Error::<Test>::AssetRegistrationMismatch);
        assert_noop!(SubstrateApp::finalize_asset_registration(
            origin(), AssetId::Xor, GenericAssetId::Sora(H256::repeat_byte(1)), AssetKind::Thischain, precision,
        ), Error::<Test>::AssetRegistrationMismatch);
        assert_eq!(SidechainAssetId::<Test>::get(network, AssetId::Xor), Some(remote));
    });
}

#[test]
fn only_local_root_can_authorize_a_registration() {
    new_tester_no_registered_assets().execute_with(|| {
        assert_noop!(SubstrateApp::authorize_asset_registration(
            origin(), SubNetworkId::Liberland, AssetId::Xor,
            GenericAssetId::Sora(H256::zero()), AssetKind::Sidechain,
        ), sp_runtime::DispatchError::BadOrigin);
    });
}

#[test]
fn remote_cannot_report_local_refund_or_pending_states() {
    new_tester().execute_with(|| {
        use bridge_types::types::MessageStatus;
        for status in [MessageStatus::Refunded, MessageStatus::InQueue, MessageStatus::Committed,
            MessageStatus::Approved] {
            assert_noop!(SubstrateApp::update_transaction_status(origin(), H256::zero(), status),
                Error::<Test>::InvalidTransferResult);
        }
        assert_ok!(SubstrateApp::update_transaction_status(origin(), H256::zero(), MessageStatus::Done));
        assert_ok!(SubstrateApp::update_transaction_status(origin(), H256::zero(), MessageStatus::Failed));
    });
}

#[test]
fn burn_rejects_undeliverable_recipients_before_custody_or_queue_changes() {
    use bridge_types::{substrate::VersionedMultiLocation, GenericAccount};
    use sp_keyring::sr25519::Keyring;
    for recipient in [
        GenericAccount::EVM(Default::default()),
        GenericAccount::Root,
        GenericAccount::Unknown,
        GenericAccount::TON(bridge_types::ton::TonAddress::empty()),
        GenericAccount::Parachain(VersionedMultiLocation::V3(Default::default())),
    ] {
        new_tester().execute_with(|| {
            // Exercise the raw helper as well as the extrinsic: rejection must
            // precede the lock, without relying on a caller's transaction.
            assert_noop!(SubstrateApp::burn_inner(
                Keyring::Alice.into(), SubNetworkId::Liberland, AssetId::Xor,
                recipient.clone(), 10,
            ), Error::<Test>::WrongAccountId);
            assert_noop!(SubstrateApp::burn(
                RuntimeOrigin::signed(Keyring::Alice.into()), SubNetworkId::Liberland,
                AssetId::Xor, recipient, 10,
            ), Error::<Test>::WrongAccountId);
        });
    }
}

#[test]
fn burn_preserves_both_deliverable_account_id_forms() {
    use bridge_types::GenericAccount;
    use sp_keyring::sr25519::Keyring;
    use traits::MultiCurrency;
    for recipient in [
        GenericAccount::Sora(Keyring::Bob.into()),
        GenericAccount::Liberland(Keyring::Bob.into()),
    ] {
        new_tester().execute_with(|| {
            let sender = Keyring::Alice.into();
            let before = Currencies::total_balance(AssetId::Xor, &sender);
            assert_ok!(SubstrateApp::burn(
                RuntimeOrigin::signed(sender.clone()), SubNetworkId::Liberland,
                AssetId::Xor, recipient, 10,
            ));
            assert_eq!(Currencies::total_balance(AssetId::Xor, &sender), before - 10);
        });
    }
}
