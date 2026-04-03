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

use crate::mock::{new_tester, new_tester_no_registered_assets, Currencies, RuntimeOrigin};
use crate::mock::{AssetId, SubstrateApp, System, Test};
use crate::Error;
use bridge_types::test_utils::BridgeAssetLockerImpl;
use bridge_types::traits::BridgeOriginOutput;
use bridge_types::types::AssetKind;
use bridge_types::SubNetworkId;
use bridge_types::{GenericAccount, GenericAssetId, GenericBalance};
use frame_support::{assert_noop, assert_ok};
use frame_system::Origin;
use sp_core::H256;
use sp_keyring::sr25519::Keyring;
use traits::MultiCurrency;

#[test]
fn it_works_deposit_event_mint_not_registered() {
    new_tester().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Custom(1);
        let sender = bridge_types::GenericAccount::Sora(Keyring::Alice.into());
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        assert_ok!(SubstrateApp::mint(
            origin_liberland,
            asset_id,
            sender,
            recipient,
            GenericBalance::Substrate(amount),
        ));
        assert!(System::events().iter().any(|r| r.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([0; 32]),
                Error::<Test>::TokenIsNotRegistered.into()
            ))));
    });
}

#[test]
fn it_fails_mint_no_precision() {
    new_tester().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Custom(1);
        let sender = bridge_types::GenericAccount::Sora(Keyring::Alice.into());
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 1_000_000_000_000_000_000;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Liberland, asset_id, AssetKind::Thischain);

        assert_ok!(SubstrateApp::mint(
            origin_liberland,
            asset_id,
            sender,
            recipient,
            GenericBalance::Substrate(amount),
        ));
        assert!(System::events().iter().any(|r| r.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([0; 32]),
                Error::<Test>::UnknownPrecision.into()
            ))));
    });
}

#[test]
fn it_fails_mint_wrong_amount() {
    new_tester().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Xor;
        let sender = bridge_types::GenericAccount::Sora(Keyring::Alice.into());
        let recipient: <Test as frame_system::Config>::AccountId = Keyring::Alice.into();
        let amount = 0;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Liberland, asset_id, AssetKind::Thischain);

        assert_ok!(SubstrateApp::mint(
            origin_liberland,
            asset_id,
            sender,
            recipient,
            GenericBalance::Substrate(amount),
        ));
        assert!(System::events().iter().any(|r| r.event
            == crate::mock::RuntimeEvent::SubstrateApp(crate::Event::FailedToMint(
                H256([0; 32]),
                Error::<Test>::WrongAmount.into()
            ))));
    });
}

#[test]
fn it_works_burn() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Liberland;
        let amount = 1_000_000;

        // send XOR
        assert_ok!(SubstrateApp::burn(
            origin.into(),
            network_id,
            AssetId::Xor,
            GenericAccount::Sora(Keyring::Alice.into()),
            amount
        ));

        let bridge_acc = BridgeAssetLockerImpl::<Currencies>::bridge_account(network_id.into());
        assert_eq!(Currencies::total_balance(AssetId::Xor, &bridge_acc), amount);
        assert_eq!(
            Currencies::total_balance(AssetId::Xor, &Keyring::Alice.into()),
            1_000_000_000_000_000_000 - amount
        );
    });
}

#[test]
fn it_fails_burn_token_not_registered() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Liberland;
        let amount = 1_000_000;

        // send XOR
        assert_noop!(
            SubstrateApp::burn(
                origin.into(),
                network_id,
                AssetId::Eth,
                GenericAccount::Sora(Keyring::Alice.into()),
                amount
            ),
            Error::<Test>::TokenIsNotRegistered
        );
    });
}

#[test]
fn it_fails_burn_unknown_presicion() {
    new_tester().execute_with(|| {
        let origin = Origin::<Test>::Signed(Keyring::Alice.into());
        let network_id = SubNetworkId::Liberland;
        let amount = 1_000_000;
        let asset_id = AssetId::Dai;

        crate::AssetKinds::<Test>::insert(SubNetworkId::Liberland, asset_id, AssetKind::Thischain);

        assert_noop!(
            SubstrateApp::burn(
                origin.into(),
                network_id,
                asset_id,
                GenericAccount::Sora(Keyring::Alice.into()),
                amount
            ),
            Error::<Test>::UnknownPrecision
        );
    });
}

#[test]
fn it_works_register_sidechain_asset() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin = Origin::<Test>::Root;
        let network_id = SubNetworkId::Mainnet;
        let symbol = "LLD";
        let name = "LLD";
        let asset_id = GenericAssetId::Liberland(bridge_types::LiberlandAssetId::LLD);

        assert_ok!(SubstrateApp::register_sidechain_asset(
            origin.into(),
            network_id,
            asset_id,
            symbol.into(),
            name.into(),
        ));
    });
}

#[test]
fn it_works_incoming_thischain_asset_registration() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Xor;
        let generic_asset_id = GenericAssetId::Sora(H256([0; 32]));

        assert_ok!(SubstrateApp::incoming_thischain_asset_registration(
            origin_liberland,
            asset_id,
            generic_asset_id,
        ));
    });
}

#[test]
fn it_works_finalize_asset_registration() {
    new_tester_no_registered_assets().execute_with(|| {
        let origin_liberland: RuntimeOrigin = dispatch::RawOrigin::new(BridgeOriginOutput::new(
            SubNetworkId::Liberland,
            H256([0; 32]),
            bridge_types::GenericTimepoint::Unknown,
            (),
        ))
        .into();
        let asset_id = AssetId::Xor;
        let generic_asset_id = GenericAssetId::Sora(H256([0; 32]));

        assert_ok!(SubstrateApp::finalize_asset_registration(
            origin_liberland,
            asset_id,
            generic_asset_id,
            AssetKind::Thischain,
            12,
        ));
    });
}
