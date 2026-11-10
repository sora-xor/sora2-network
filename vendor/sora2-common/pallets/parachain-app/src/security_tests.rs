use crate::{mock::*, AssetKinds, Error, PendingAssetKinds, SidechainPrecision};
use bridge_types::{traits::BridgeOriginOutput, types::AssetKind, GenericTimepoint, SubNetworkId};
use frame_support::{assert_noop, assert_ok};
use sp_core::H256;

#[test]
fn acknowledgements_preserve_locally_authorized_asset_kind() {
    new_tester_no_registered_assets().execute_with(|| {
        let network = SubNetworkId::Kusama;
        let origin = || -> RuntimeOrigin {
            dispatch::RawOrigin::new(BridgeOriginOutput::new(
                network, H256::repeat_byte(7), GenericTimepoint::Unknown, (),
            )).into()
        };
        // This is also the pre-upgrade pending layout. It requires explicit local
        // review; precision alone cannot authorize a remote minting capability.
        SidechainPrecision::<Test>::insert(network, AssetId::Xor, 18);
        assert_noop!(ParachainApp::finalize_asset_registration(
            origin(), AssetId::Xor, AssetKind::Sidechain,
        ), Error::<Test>::AssetRegistrationNotAuthorized);
        assert_ok!(ParachainApp::authorize_pending_asset_registration(
            RuntimeOrigin::root(), network, AssetId::Xor, AssetKind::Thischain,
        ));
        assert_noop!(ParachainApp::finalize_asset_registration(
            origin(), AssetId::Xor, AssetKind::Sidechain,
        ), Error::<Test>::AssetRegistrationNotAuthorized);
        assert_ok!(ParachainApp::finalize_asset_registration(
            origin(), AssetId::Xor, AssetKind::Thischain,
        ));
        assert!(!PendingAssetKinds::<Test>::contains_key(network, AssetId::Xor));
        assert_noop!(ParachainApp::finalize_asset_registration(
            origin(), AssetId::Xor, AssetKind::Sidechain,
        ), Error::<Test>::AssetRegistrationMismatch);
        assert_eq!(AssetKinds::<Test>::get(network, AssetId::Xor), Some(AssetKind::Thischain));
        assert_ok!(ParachainApp::finalize_asset_registration(
            origin(), AssetId::Xor, AssetKind::Thischain,
        ));
    });
}
