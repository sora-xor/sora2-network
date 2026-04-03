// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::mock::{
    new_tester, new_tester_no_registered_assets, BalancePrecisionConverterImpl, Currencies,
    RuntimeOrigin, PARA_A, PARA_C,
};
use crate::mock::{AssetId, ParachainApp, Test};
use crate::{Error, RelaychainAsset};
use bridge_types::substrate::{ParachainAssetId, PARENT_PARACHAIN_ASSET};
use bridge_types::test_utils::BridgeAssetLockerImpl;
use bridge_types::traits::{BalancePrecisionConverter, BridgeOriginOutput};
use bridge_types::types::AssetKind;
use bridge_types::{
    substrate::{Junction, VersionedMultiLocation},
    SubNetworkId,
};
use frame_support::{assert_noop, assert_ok};
use frame_system::Origin;
use sp_core::H256;
use sp_keyring::sr25519::Keyring;
use staging_xcm::v3::Junctions::{X1, X3};
use staging_xcm::v3::{Junction::Parachain, Junctions::X2, MultiLocation};
use traits::MultiCurrency;

#[test]
fn it_works_mint() {
    new_tester().execute_with(|| {
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Custom(1);
        let sender = None;
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        assert_ok!(ParachainApp::mint(
            origin_kusama,
            asset_id,
            sender,
            recipient.clone(),
            amount
        ));
        let (_, sidechain_amount) =
            BalancePrecisionConverterImpl::from_sidechain(&AssetId::Custom(1), 0, amount).unwrap();
        assert_eq!(
            Currencies::total_balance(asset_id, &recipient),
            sidechain_amount
        );
    });
}

#[test]
fn it_fails_mint_not_registered() {
    new_tester().execute_with(|| {
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Eth;
        let sender = None;
        let recipient = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        assert_noop!(
            ParachainApp::mint(origin_kusama, asset_id, sender, recipient, amount),
            Error::<Test>::TokenIsNotRegistered
        );
    });
}

#[test]
fn it_fails_mint_no_precision() {
    new_tester().execute_with(|| {
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Eth;
        let sender = None;
        let recipient = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Kusama, asset_id, AssetKind::Thischain);

        assert_noop!(
            ParachainApp::mint(origin_kusama, asset_id, sender, recipient, amount),
            Error::<Test>::UnknownPrecision
        );
    });
}

#[test]
fn it_fails_mint_wrong_amount() {
    new_tester().execute_with(|| {
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Custom(1);
        let sender = None;
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 0;

        assert_noop!(
            ParachainApp::mint(origin_kusama, asset_id, sender, recipient, amount),
            Error::<Test>::WrongAmount
        );
    });
}

#[test]
fn it_works_burn() {
    new_tester().execute_with(|| {
        let location = MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        );
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let recipient = VersionedMultiLocation::V3(location);
        let amount = 1_000_000;

        // send XOR
        assert_ok!(ParachainApp::burn(
            origin.clone().into(),
            network_id,
            AssetId::Xor,
            recipient,
            amount
        ));

        let bridge_acc = BridgeAssetLockerImpl::<Currencies>::bridge_account(network_id.into());
        assert_eq!(Currencies::total_balance(AssetId::Xor, &bridge_acc), amount);
        assert_eq!(
            Currencies::total_balance(AssetId::Xor, &Keyring::Alice.into()),
            1_000_000_000_000_000_000 - amount
        );

        // send relaychain asset (KSM)
        let relay_asset = RelaychainAsset::<Test>::get(network_id).unwrap();
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        assert_ok!(ParachainApp::mint(
            origin_kusama,
            relay_asset,
            None,
            Keyring::Alice.into(),
            1_000_000_000_000_000_000
        ));
        let alice_balance_before = Currencies::total_balance(relay_asset, &Keyring::Alice.into());

        assert_ok!(ParachainApp::burn(
            origin.into(),
            network_id,
            relay_asset,
            VersionedMultiLocation::V3(MultiLocation::new(
                1,
                X1(Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },),
            )),
            amount
        ));
        assert_eq!(
            Currencies::total_balance(relay_asset, &Keyring::Alice.into()),
            alice_balance_before - amount
        );
    });
}

#[test]
fn it_fails_burn_invalid_destination_params() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let asset_id = AssetId::Xor;
        let amount = 100;

        // XCM v2 is not supported
        assert_noop!(
            ParachainApp::burn(
                origin.clone().into(),
                network_id,
                asset_id,
                VersionedMultiLocation::V2(staging_xcm::v2::MultiLocation::default()),
                amount
            ),
            Error::<Test>::InvalidDestinationParams
        );
        // XCM destination != Parachain(id) not supported
        assert_noop!(
            ParachainApp::burn(
                origin.clone().into(),
                network_id,
                asset_id,
                MultiLocation::new(
                    1,
                    X2(
                        Junction::PalletInstance(1),
                        Junction::AccountId32 {
                            network: None,
                            id: Keyring::Bob.into(),
                        },
                    ),
                )
                .into(),
                amount
            ),
            Error::<Test>::InvalidDestinationParams
        );
        // XCM destination > X2 not supported
        assert_noop!(
            ParachainApp::burn(
                origin.into(),
                network_id,
                asset_id,
                VersionedMultiLocation::V3(MultiLocation::new(
                    1,
                    X3(
                        Parachain(PARA_A),
                        Junction::PalletInstance(1),
                        Junction::AccountId32 {
                            network: None,
                            id: Keyring::Bob.into(),
                        },
                    ),
                )),
                amount
            ),
            Error::<Test>::InvalidDestinationParams
        );
    });
}

#[test]
fn it_fails_burn_relaychain_asset_not_registered() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let asset_id = AssetId::Xor;
        let amount = 100;

        let sidechain_asset = ParachainAssetId::Concrete(MultiLocation::new(
            1,
            X2(
                Parachain(1),
                staging_xcm::v3::Junction::GeneralKey {
                    length: 32,
                    data: [0u8; 32],
                },
            ),
        ));
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        ParachainApp::register_thischain_asset(
            Origin::<Test>::Root.into(),
            SubNetworkId::Kusama,
            asset_id,
            sidechain_asset,
            Vec::new(),
            10,
        )
        .expect("XOR registration failed");
        ParachainApp::finalize_asset_registration(
            origin_kusama,
            AssetId::Xor,
            AssetKind::Thischain,
        )
        .expect("XOR registration finalization failed");
        assert_noop!(
            ParachainApp::burn(
                origin.into(),
                network_id,
                asset_id,
                VersionedMultiLocation::V3(MultiLocation::new(
                    1,
                    X1(Junction::AccountId32 {
                        network: None,
                        id: Keyring::Bob.into(),
                    }),
                )),
                amount
            ),
            Error::<Test>::RelaychainAssetNotRegistered
        );
    });
}

#[test]
fn it_fails_not_relay_transferable_asset() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let amount = 100;
        let asset_id = AssetId::Dai;

        ParachainApp::register_thischain_asset(
            Origin::<Test>::Root.into(),
            SubNetworkId::Kusama,
            asset_id,
            ParachainAssetId::Concrete(MultiLocation::new(
                1,
                X2(
                    Parachain(1),
                    staging_xcm::v3::Junction::GeneralKey {
                        length: 32,
                        data: [0u8; 32],
                    },
                ),
            )),
            Vec::new(),
            100,
        )
        .expect("DAI registration failed");
        let origin_kusama: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Kusama,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        ParachainApp::finalize_asset_registration(origin_kusama, asset_id, AssetKind::Thischain)
            .expect("DAI registration finalization failed");

        assert_noop!(
            ParachainApp::burn(
                origin.into(),
                network_id,
                asset_id,
                VersionedMultiLocation::V3(MultiLocation::new(
                    1,
                    X1(Junction::AccountId32 {
                        network: None,
                        id: Keyring::Bob.into(),
                    },),
                )),
                amount
            ),
            Error::<Test>::NotRelayTransferableAsset
        );
    });
}

#[test]
fn it_fails_burn_invalid_destination_parachain() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let asset_id = AssetId::Xor;
        let amount = 100;

        assert_noop!(
            ParachainApp::burn(
                origin.into(),
                network_id,
                asset_id,
                VersionedMultiLocation::V3(MultiLocation::new(
                    1,
                    X2(
                        Parachain(PARA_C),
                        Junction::AccountId32 {
                            network: None,
                            id: Keyring::Bob.into(),
                        }
                    ),
                )),
                amount
            ),
            Error::<Test>::InvalidDestinationParachain
        );
    });
}

#[test]
fn it_fails_burn_token_not_registered() {
    new_tester().execute_with(|| {
        let location = MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        );
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let recipient = VersionedMultiLocation::V3(location);
        let amount = 1_000_000;

        assert_noop!(
            ParachainApp::burn(origin.into(), network_id, AssetId::Eth, recipient, amount),
            Error::<Test>::TokenIsNotRegistered
        );
    });
}

#[test]
fn it_fails_burn_unknown_presicion() {
    new_tester().execute_with(|| {
        let location = MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        );
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let recipient = VersionedMultiLocation::V3(location);
        let amount = 1_000_000;
        let asset_id = AssetId::Dai;

        crate::AllowedParachainAssets::<Test>::insert(SubNetworkId::Kusama, PARA_A, vec![asset_id]);
        crate::AssetKinds::<Test>::insert(SubNetworkId::Kusama, asset_id, AssetKind::Thischain);

        assert_noop!(
            ParachainApp::burn(origin.into(), network_id, asset_id, recipient, amount),
            Error::<Test>::UnknownPrecision
        );
    });
}

#[test]
fn it_fails_burn_lock_asset() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let amount = 1_000_000;
        let relay_asset = RelaychainAsset::<Test>::get(network_id).unwrap();

        assert_noop!(
            ParachainApp::burn(
                origin.into(),
                network_id,
                relay_asset,
                VersionedMultiLocation::V3(MultiLocation::new(
                    1,
                    X1(Junction::AccountId32 {
                        network: None,
                        id: Keyring::Bob.into(),
                    },),
                )),
                amount
            ),
            tokens::Error::<Test>::BalanceTooLow
        );
    });
}

#[test]
fn it_fails_burn_outbound_channel_submit() {
    new_tester().execute_with(|| {
        let location = MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        );
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Kusama;
        let recipient = VersionedMultiLocation::V3(location);
        let amount = 1_000_000;

        // send XOR
        assert_ok!(ParachainApp::burn(
            origin.clone().into(),
            network_id,
            AssetId::Xor,
            recipient.clone(),
            amount
        ));
        assert_ok!(ParachainApp::burn(
            origin.clone().into(),
            network_id,
            AssetId::Xor,
            recipient.clone(),
            amount
        ));
        assert_ok!(ParachainApp::burn(
            origin.clone().into(),
            network_id,
            AssetId::Xor,
            recipient.clone(),
            amount
        ));
        assert_noop!(
            ParachainApp::burn(origin.into(), network_id, AssetId::Xor, recipient, amount),
            substrate_bridge_channel::outbound::Error::<Test>::QueueSizeLimitReached
        );
    });
}

#[test]
fn it_works_register_thischain_asset() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin = Origin::<Test>::Root;
        let network_id = SubNetworkId::Mainnet;
        let asset_id = AssetId::Xor;
        let sidechain_asset = ParachainAssetId::Concrete(MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        ));
        let allowed_parachains = Vec::new();
        let minimal_xcm_amount = 10;

        assert_ok!(ParachainApp::register_thischain_asset(
            origin.into(),
            network_id,
            asset_id,
            sidechain_asset,
            allowed_parachains,
            minimal_xcm_amount
        ));
    });
}

#[test]
fn it_works_register_asset_inner() {
    new_tester_no_registered_assets().execute_with(|| {
        let network_id = SubNetworkId::Mainnet;
        let asset_id = AssetId::Dai;
        let sidechain_asset = ParachainAssetId::Concrete(MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        ));
        let asset_kind = AssetKind::Thischain;
        let sidechain_precision = 12;
        let allowed_parachains = Vec::<u32>::new();
        let minimal_xcm_amount = 1_000_000;

        assert_ok!(ParachainApp::register_asset_inner(
            network_id,
            asset_id,
            sidechain_asset,
            asset_kind,
            sidechain_precision,
            allowed_parachains,
            minimal_xcm_amount
        ));
    });
}

#[test]
fn it_fails_register_asset_inner_already_registered() {
    new_tester().execute_with(|| {
        let network_id = SubNetworkId::Kusama;
        let asset_id = AssetId::Custom(1);
        let asset_kind = AssetKind::Thischain;
        let sidechain_precision = 12;
        let allowed_parachains = Vec::<u32>::new();
        let minimal_xcm_amount = 1_000_000;

        assert_noop!(
            ParachainApp::register_asset_inner(
                network_id,
                asset_id,
                PARENT_PARACHAIN_ASSET,
                asset_kind,
                sidechain_precision,
                allowed_parachains,
                minimal_xcm_amount
            ),
            Error::<Test>::RelaychainAssetRegistered
        );
    });
}

#[test]
fn it_works_register_sidechain_asset() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin = Origin::<Test>::Root;
        let network_id = SubNetworkId::Mainnet;
        let sidechain_asset = ParachainAssetId::Concrete(MultiLocation::new(
            1,
            X2(
                Parachain(PARA_A),
                Junction::AccountId32 {
                    network: None,
                    id: Keyring::Bob.into(),
                },
            ),
        ));
        let symbol = "AssetSymbol";
        let name = "AssetName";
        let decimals = 10;
        let allowed_parachains = Vec::new();
        let minimal_xcm_amount = 10;

        assert_ok!(ParachainApp::register_sidechain_asset(
            origin.into(),
            network_id,
            sidechain_asset,
            symbol.into(),
            name.into(),
            decimals,
            allowed_parachains,
            minimal_xcm_amount
        ));
    });
}

#[test]
fn it_works_add_assetid_paraid() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Root;
        let network_id = SubNetworkId::Kusama;
        let asset_id = AssetId::Xor;

        assert_ok!(ParachainApp::add_assetid_paraid(
            origin.into(),
            network_id,
            PARA_C,
            asset_id,
        ));
    });
}

#[test]
fn it_works_remove_assetid_paraid() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Root;
        let network_id = SubNetworkId::Kusama;
        let asset_id = AssetId::Xor;

        assert_ok!(ParachainApp::remove_assetid_paraid(
            origin.into(),
            network_id,
            PARA_A,
            asset_id,
        ));
    });
}
