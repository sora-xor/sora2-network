use crate::mock::{new_tester_no_registered_assets, AssetId, RuntimeOrigin, SubstrateApp, Test};
use crate::{AssetKinds, Error, PendingAssetRegistrations, ThischainAssetId};
use bridge_types::traits::BridgeOriginOutput;
use bridge_types::types::AssetKind;
use bridge_types::{GenericAssetId, GenericTimepoint, SubNetworkId, H256};
use frame_support::{assert_noop, assert_ok};

fn origin() -> RuntimeOrigin {
    dispatch::RawOrigin::new(BridgeOriginOutput::new(
        SubNetworkId::Liberland,
        H256::repeat_byte(91),
        GenericTimepoint::Unknown,
        (),
    ))
    .into()
}

#[test]
fn authorization_and_incoming_registration_reject_missing_local_assets() {
    new_tester_no_registered_assets().execute_with(|| {
        let asset = AssetId::Custom(u8::MAX);
        let remote = GenericAssetId::Sora(H256::repeat_byte(92));
        assert_noop!(
            SubstrateApp::authorize_asset_registration(
                RuntimeOrigin::root(),
                SubNetworkId::Liberland,
                asset,
                remote,
                AssetKind::Thischain,
            ),
            Error::<Test>::TokenIsNotRegistered
        );
        assert_noop!(
            SubstrateApp::incoming_thischain_asset_registration(origin(), asset, remote),
            Error::<Test>::TokenIsNotRegistered
        );
        assert!(!PendingAssetRegistrations::<Test>::contains_key(
            SubNetworkId::Liberland,
            asset
        ));
        assert_eq!(
            ThischainAssetId::<Test>::get(SubNetworkId::Liberland, remote),
            None
        );
    });
}

#[test]
fn reserved_remote_identifier_cannot_be_rebound_before_or_after_activation() {
    new_tester_no_registered_assets().execute_with(|| {
        let network = SubNetworkId::Liberland;
        let remote = GenericAssetId::Sora(H256::repeat_byte(93));
        assert_ok!(SubstrateApp::authorize_asset_registration(
            RuntimeOrigin::root(),
            network,
            AssetId::Xor,
            remote,
            AssetKind::Thischain,
        ));
        assert_eq!(
            PendingAssetRegistrations::<Test>::get(network, AssetId::Xor),
            Some((remote, AssetKind::Thischain))
        );
        assert_eq!(
            ThischainAssetId::<Test>::get(network, remote),
            Some(AssetId::Xor)
        );
        assert_eq!(AssetKinds::<Test>::get(network, AssetId::Xor), None);
        assert_noop!(
            SubstrateApp::authorize_asset_registration(
                RuntimeOrigin::root(),
                network,
                AssetId::Eth,
                remote,
                AssetKind::Sidechain,
            ),
            Error::<Test>::AssetRegistrationMismatch
        );
        assert_noop!(
            SubstrateApp::register_sidechain_asset(
                RuntimeOrigin::root(),
                network,
                remote,
                b"ETH".to_vec(),
                b"Ether".to_vec(),
            ),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_ok!(SubstrateApp::incoming_thischain_asset_registration(
            origin(),
            AssetId::Xor,
            remote,
        ));
        assert!(!PendingAssetRegistrations::<Test>::contains_key(
            network,
            AssetId::Xor
        ));
        assert_eq!(
            AssetKinds::<Test>::get(network, AssetId::Xor),
            Some(AssetKind::Thischain)
        );
        assert_noop!(
            SubstrateApp::authorize_asset_registration(
                RuntimeOrigin::root(),
                network,
                AssetId::Xor,
                remote,
                AssetKind::Sidechain,
            ),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_eq!(
            ThischainAssetId::<Test>::get(network, remote),
            Some(AssetId::Xor)
        );
    });
}
