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
