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
    new_tester, AccountId, FungibleApp, RuntimeEvent, RuntimeOrigin, System, Test, Tokens,
    BASE_NETWORK_ID, ETH, XOR,
};
use crate::Error;
use crate::{AppAddresses, AssetKinds, AssetsByAddresses, TokenAddresses};
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::{AssetKind, CallOriginOutput, GenericAdditionalInboundData};
use bridge_types::{EVMChainId, GenericNetworkId, H160};
use frame_support::assert_noop;
use frame_support::assert_ok;
use sp_keyring::AccountKeyring as Keyring;
use traits::MultiCurrency;

fn last_event() -> RuntimeEvent {
    System::events().pop().expect("Event expected").event
}

#[test]
fn mints_after_handling_ethereum_event() {
    new_tester().execute_with(|| {
        let peer_contract = H160::repeat_byte(2);
        let asset_id = XOR;
        let token = TokenAddresses::<Test>::get(BASE_NETWORK_ID, asset_id).unwrap();
        let sender = H160::repeat_byte(3);
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 10;

        Tokens::deposit(asset_id, &bob, 500).unwrap();
        assert_ok!(FungibleApp::burn(
            RuntimeOrigin::signed(bob),
            BASE_NETWORK_ID,
            asset_id,
            H160::repeat_byte(9),
            amount
        ));

        assert_ok!(FungibleApp::mint(
            dispatch::RawOrigin::new(CallOriginOutput {
                network_id: GenericNetworkId::EVM(BASE_NETWORK_ID),
                additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData {
                    source: peer_contract,
                }),
                ..Default::default()
            })
            .into(),
            token,
            sender,
            recipient.clone(),
            amount.into(),
        ));
        assert_eq!(Tokens::total_balance(asset_id, &recipient), amount);

        assert_eq!(
            RuntimeEvent::FungibleApp(crate::Event::<Test>::Minted {
                network_id: BASE_NETWORK_ID,
                asset_id,
                sender,
                recipient,
                amount
            }),
            last_event()
        );
    });
}

#[test]
fn mint_zero_amount_must_fail() {
    new_tester().execute_with(|| {
        let peer_contract = H160::repeat_byte(2);
        let asset_id = XOR;
        let token = TokenAddresses::<Test>::get(BASE_NETWORK_ID, asset_id).unwrap();
        let sender = H160::repeat_byte(3);
        let recipient: AccountId = Keyring::Charlie.into();
        let amount = 0;

        assert_noop!(
            FungibleApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: GenericNetworkId::EVM(BASE_NETWORK_ID),
                    additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData {
                        source: peer_contract,
                    }),
                    ..Default::default()
                })
                .into(),
                token,
                sender,
                recipient,
                amount.into(),
            ),
            Error::<Test>::WrongAmount
        );
    });
}

#[test]
fn burn_should_emit_bridge_event() {
    new_tester().execute_with(|| {
        let asset_id = XOR;
        let recipient = H160::repeat_byte(2);
        let bob: AccountId = Keyring::Bob.into();
        let amount = 20;
        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_ok!(FungibleApp::burn(
            RuntimeOrigin::signed(bob.clone()),
            BASE_NETWORK_ID,
            asset_id,
            recipient,
            amount
        ));

        assert_eq!(
            RuntimeEvent::FungibleApp(crate::Event::<Test>::Burned {
                network_id: BASE_NETWORK_ID,
                asset_id,
                sender: bob,
                recipient,
                amount
            }),
            last_event()
        );
    });
}

#[test]
fn burn_zero_amount_must_fail() {
    new_tester().execute_with(|| {
        let asset_id = XOR;
        let recipient = H160::repeat_byte(2);
        let bob: AccountId = Keyring::Bob.into();
        let amount = 0;
        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_noop!(
            FungibleApp::burn(
                RuntimeOrigin::signed(bob),
                BASE_NETWORK_ID,
                asset_id,
                recipient,
                amount
            ),
            Error::<Test>::WrongAmount
        );
    });
}

#[test]
fn test_register_asset_internal() {
    new_tester().execute_with(|| {
        let asset_id = ETH;
        let who = AppAddresses::<Test>::get(BASE_NETWORK_ID).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {
            network_id: GenericNetworkId::EVM(BASE_NETWORK_ID),
            additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData { source: who }),
            ..Default::default()
        });
        let address = H160::repeat_byte(98);
        assert!(!TokenAddresses::<Test>::contains_key(
            BASE_NETWORK_ID,
            asset_id
        ));
        FungibleApp::register_asset_internal(origin.into(), asset_id, address).unwrap();
        assert_eq!(
            AssetKinds::<Test>::get(BASE_NETWORK_ID, asset_id),
            Some(AssetKind::Thischain)
        );
        assert!(TokenAddresses::<Test>::contains_key(
            BASE_NETWORK_ID,
            asset_id
        ));
    })
}

#[test]
fn test_register_erc20_asset() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(98);
        let network_id = BASE_NETWORK_ID;
        assert!(!AssetsByAddresses::<Test>::contains_key(
            network_id, address
        ));
        FungibleApp::register_sidechain_asset(
            RuntimeOrigin::root(),
            network_id,
            address,
            "ETH".to_string().into(),
            "ETH".to_string().into(),
            18,
        )
        .unwrap();
        assert!(AssetsByAddresses::<Test>::contains_key(network_id, address));
    })
}

#[test]
fn test_register_native_asset() {
    new_tester().execute_with(|| {
        let asset_id = ETH;
        let network_id = BASE_NETWORK_ID;
        assert!(!TokenAddresses::<Test>::contains_key(network_id, asset_id));
        FungibleApp::register_thischain_asset(RuntimeOrigin::root(), network_id, asset_id).unwrap();
        assert!(!TokenAddresses::<Test>::contains_key(network_id, asset_id));
    })
}

#[test]
fn test_register_erc20_app() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(98);
        let network_id = EVMChainId::from_low_u64_be(2);
        assert!(!AppAddresses::<Test>::contains_key(network_id,));
        FungibleApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            network_id,
            address,
            ETH,
            18,
        )
        .unwrap();
        assert!(AppAddresses::<Test>::contains_key(network_id,));
    })
}

#[test]
fn test_register_network() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(98);
        let network_id = EVMChainId::from_low_u64_be(2);
        assert!(!AppAddresses::<Test>::contains_key(network_id,));
        FungibleApp::register_network(
            RuntimeOrigin::root(),
            network_id,
            address,
            "ETH".into(),
            "ETH".into(),
            18,
        )
        .unwrap();
        assert!(AppAddresses::<Test>::contains_key(network_id,));
    })
}
