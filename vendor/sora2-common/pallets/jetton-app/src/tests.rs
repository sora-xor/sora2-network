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

use crate::mock::*;
use crate::*;
use bridge_types::ton::AdditionalTONInboundData;
use bridge_types::ton::TonAddress;
use bridge_types::ton::TonAddressWithPrefix;
use bridge_types::types::CallOriginOutput;
use bridge_types::types::GenericAdditionalInboundData;
use frame_support::assert_noop;
use frame_support::assert_ok;
use sp_core::H256;
use sp_keyring::AccountKeyring as Keyring;
use traits::MultiCurrency;

use crate::mock::{BASE_NETWORK_ID, TON};

fn last_event() -> RuntimeEvent {
    System::events().pop().expect("Event expected").event
}

#[test]
fn mints_after_ton_transfer() {
    ExtBuilder::with_ton().build().execute_with(|| {
        let asset_id = TON;
        let token = TokenAddresses::<Test>::get(asset_id).unwrap();
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 10;

        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_ok!(JettonApp::mint(
            dispatch::RawOrigin::new(CallOriginOutput {
                network_id: BASE_NETWORK_ID.into(),
                additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                    source: TON_APP_ADDRESS
                }),
                ..Default::default()
            })
            .into(),
            token.into(),
            sender.into(),
            recipient.clone(),
            amount.into(),
        ));
        assert_eq!(Tokens::total_balance(asset_id, &recipient), amount);

        assert_eq!(
            RuntimeEvent::JettonApp(crate::Event::<Test>::Minted {
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
fn mint_fails_with_zero_amount() {
    ExtBuilder::with_ton().build().execute_with(|| {
        let asset_id = TON;
        let token = TokenAddresses::<Test>::get(asset_id).unwrap();
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 0u32;

        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_noop!(
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TON_APP_ADDRESS
                    }),
                    ..Default::default()
                })
                .into(),
                token.into(),
                sender.into(),
                recipient,
                amount.into(),
            ),
            Error::<Test>::WrongAmount
        );
    });
}

#[test]
fn mint_fails_without_app() {
    ExtBuilder::empty().build().execute_with(|| {
        let asset_id = TON;
        let token = TON_APP_ADDRESS;
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 10u32;

        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_noop!(
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TON_APP_ADDRESS
                    }),
                    ..Default::default()
                })
                .into(),
                token.into(),
                sender.into(),
                recipient,
                amount.into(),
            ),
            Error::<Test>::TokenIsNotRegistered
        );
    });
}

#[test]
fn mint_fails_with_wrong_address() {
    ExtBuilder::with_ton().build().execute_with(|| {
        let asset_id = TON;
        let token = TokenAddresses::<Test>::get(asset_id).unwrap();
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 10u32;

        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_noop!(
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TON_APP_ADDRESS
                    }),
                    ..Default::default()
                })
                .into(),
                TonAddressWithPrefix::new(11, token),
                sender.into(),
                recipient.clone(),
                amount.into(),
            ),
            Error::<Test>::WrongAccountPrefix
        );

        assert_noop!(
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TON_APP_ADDRESS
                    }),
                    ..Default::default()
                })
                .into(),
                token.into(),
                TonAddressWithPrefix::new(11, sender),
                recipient,
                amount.into(),
            ),
            Error::<Test>::WrongAccountPrefix
        );
    });
}

#[test]
fn mint_fails_with_wrong_asset() {
    ExtBuilder::with_ton().build().execute_with(|| {
        let asset_id = TON;
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 10u32;

        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_noop!(
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TON_APP_ADDRESS
                    }),
                    ..Default::default()
                })
                .into(),
                TonAddress::new(0, H256::random()).into(),
                sender.into(),
                recipient,
                amount.into(),
            ),
            Error::<Test>::TokenIsNotRegistered
        );
    });
}

#[test]
fn mint_fails_with_bad_origin() {
    ExtBuilder::with_ton().build().execute_with(|| {
        let asset_id = TON;
        let token = TokenAddresses::<Test>::get(asset_id).unwrap();
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = 10u32;

        Tokens::deposit(asset_id, &bob, 500).unwrap();

        assert_noop!(
            JettonApp::mint(
                frame_system::RawOrigin::Root.into(),
                token.into(),
                sender.into(),
                recipient.clone(),
                amount.into(),
            ),
            DispatchError::BadOrigin
        );

        assert_noop!(
            JettonApp::mint(
                frame_system::RawOrigin::Signed(Keyring::Alice.into()).into(),
                token.into(),
                sender.into(),
                recipient.clone(),
                amount.into(),
            ),
            DispatchError::BadOrigin
        );

        assert_noop!(
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TonAddress::new(0, H256::repeat_byte(2))
                    }),
                    ..Default::default()
                })
                .into(),
                token.into(),
                sender.into(),
                recipient,
                amount.into(),
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_register_network() {
    ExtBuilder::empty().build().execute_with(|| {
        assert!(!AppInfo::<Test>::exists());
        JettonApp::register_network(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID,
            TON_APP_ADDRESS,
            "TON".into(),
            "TON".into(),
            18,
        )
        .unwrap();
        assert!(AppInfo::<Test>::exists());
    })
}

#[test]
fn test_register_network_with_existing_asset() {
    ExtBuilder::empty().build().execute_with(|| {
        assert!(!AppInfo::<Test>::exists());
        JettonApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID,
            TON_APP_ADDRESS,
            TON,
            18,
        )
        .unwrap();
        assert!(AppInfo::<Test>::exists());
    })
}
