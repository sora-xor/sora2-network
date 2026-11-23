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
use sp_keyring::sr25519::Keyring;
use traits::MultiCurrency;

use crate::mock::{BASE_NETWORK_ID, TON};

#[test]
fn discovery_reports_registered_ton_assets_and_omits_incomplete_records() {
    use bridge_types::ton::{TonAppInfo, TonAssetInfo, TonNetworkId};
    use bridge_types::traits::BridgeApp;
    use bridge_types::types::{BridgeAppInfo, BridgeAssetInfo};
    ExtBuilder::empty().build().execute_with(|| {
        assert!(JettonApp::list_apps().is_empty());
        assert!(JettonApp::list_supported_assets(BASE_NETWORK_ID.into()).is_empty());
    });
    ExtBuilder::with_ton().build().execute_with(|| {
        assert_eq!(
            JettonApp::list_apps(),
            vec![BridgeAppInfo::TON(
                BASE_NETWORK_ID.into(),
                TonAppInfo {
                    address: TON_APP_ADDRESS
                }
            )]
        );
        assert_eq!(
            JettonApp::list_supported_assets(BASE_NETWORK_ID.into()),
            vec![BridgeAssetInfo::Ton(TonAssetInfo {
                asset_id: TON,
                address: TON_ADDRESS,
                precision: 18
            })]
        );
        assert!(JettonApp::list_supported_assets(TonNetworkId::Mainnet.into()).is_empty());
        SidechainPrecision::<Test>::remove(TON);
        assert!(JettonApp::list_supported_assets(BASE_NETWORK_ID.into()).is_empty());
        SidechainPrecision::<Test>::insert(TON, 18);
        TokenAddresses::<Test>::remove(TON);
        assert!(JettonApp::list_supported_assets(BASE_NETWORK_ID.into()).is_empty());
    });
}

#[test]
fn outbound_routes_are_explicitly_unsupported() {
    use bridge_types::traits::BridgeApp;
    ExtBuilder::with_ton().build().execute_with(|| {
        assert!(!JettonApp::is_asset_supported(BASE_NETWORK_ID.into(), TON));
        assert_noop!(
            JettonApp::transfer(
                BASE_NETWORK_ID.into(),
                TON,
                Keyring::Alice.into(),
                TON_ADDRESS,
                10
            ),
            Error::<Test>::InvalidNetwork
        );
        assert_noop!(
            JettonApp::refund(
                BASE_NETWORK_ID.into(),
                H256::zero(),
                Keyring::Alice.into(),
                TON,
                10
            ),
            Error::<Test>::InvalidNetwork
        );
        assert_eq!(
            JettonApp::transfer_weight(),
            frame_support::weights::Weight::zero()
        );
        assert_eq!(
            JettonApp::refund_weight(),
            frame_support::weights::Weight::zero()
        );
        assert_eq!(
            JettonApp::is_asset_supported_weight(),
            frame_support::weights::Weight::zero()
        );
    });
}

#[test]
fn registration_rejects_duplicates_and_unprivileged_callers() {
    ExtBuilder::with_ton().build().execute_with(|| {
        assert_noop!(
            JettonApp::register_asset_inner(
                TON,
                TON_ADDRESS,
                bridge_types::types::AssetKind::Sidechain,
                18
            ),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_noop!(
            JettonApp::register_network(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                TON_APP_ADDRESS,
                b"TON".to_vec(),
                b"TON".to_vec(),
                18
            ),
            Error::<Test>::AppAlreadyRegistered
        );
        assert_noop!(
            JettonApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                TON_APP_ADDRESS,
                TON,
                18
            ),
            Error::<Test>::AppAlreadyRegistered
        );
        assert_noop!(
            JettonApp::register_network(
                RuntimeOrigin::signed(Keyring::Alice.into()),
                BASE_NETWORK_ID,
                TON_APP_ADDRESS,
                b"TON".to_vec(),
                b"TON".to_vec(),
                18
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            JettonApp::register_network_with_existing_asset(
                RuntimeOrigin::signed(Keyring::Alice.into()),
                BASE_NETWORK_ID,
                TON_APP_ADDRESS,
                TON,
                18
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
    ExtBuilder::empty().build().execute_with(|| {
        assert_noop!(
            JettonApp::register_asset_inner(
                TON,
                TON_ADDRESS,
                bridge_types::types::AssetKind::Sidechain,
                18
            ),
            Error::<Test>::AppIsNotRegistered
        );
    });
}

#[test]
fn quarantined_mint_and_missing_registration_fields_do_not_credit_tokens() {
    ExtBuilder::with_ton().build().execute_with(|| {
        let message = H256::repeat_byte(43);
        let mint = || {
            JettonApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID.into(),
                    message_id: message,
                    additional: GenericAdditionalInboundData::TON(AdditionalTONInboundData {
                        source: TON_APP_ADDRESS,
                    }),
                    ..Default::default()
                })
                .into(),
                TON_ADDRESS.into(),
                TON_ADDRESS.into(),
                Keyring::Alice.into(),
                10u128.into(),
            )
        };
        sp_io::storage::set(
            &bridge_types::recovery::message_key(BASE_NETWORK_ID.into(), message),
            &[1],
        );
        assert_noop!(mint(), Error::<Test>::RecoveryHalted);
        sp_io::storage::clear(&bridge_types::recovery::message_key(
            BASE_NETWORK_ID.into(),
            message,
        ));
        SidechainPrecision::<Test>::remove(TON);
        assert_noop!(mint(), Error::<Test>::TokenIsNotRegistered);
        SidechainPrecision::<Test>::insert(TON, 18);
        AssetKinds::<Test>::remove(TON);
        assert_noop!(mint(), Error::<Test>::TokenIsNotRegistered);
        assert_eq!(Tokens::total_balance(TON, &Keyring::Alice.into()), 0);
    });
}

#[test]
fn bridge_call_conversion_preserves_transfer_fields() {
    let call: Call<Test> = bridge_types::substrate::JettonAppCall::Transfer {
        sender: TON_APP_ADDRESS.into(),
        recipient: Keyring::Alice.into(),
        amount: 123u128.into(),
        token: TON_ADDRESS.into(),
    }
    .into();
    assert_eq!(
        call,
        Call::<Test>::mint {
            sender: TON_APP_ADDRESS.into(),
            recipient: Keyring::Alice.into(),
            amount: 123u128.into(),
            token: TON_ADDRESS.into()
        }
    );
}

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

#[test]
fn disabled_bridge_preserves_registered_state_and_rejects_all_value_paths() {
    use bridge_types::traits::BridgeApp;
    ExtBuilder::with_ton().build().execute_with(|| {
        let app = AppInfo::<Test>::get();
        let token = TokenAddresses::<Test>::get(TON).unwrap();
        let recipient: AccountId = Keyring::Charlie.into();
        crate::mock::BridgeEnabled::set(false);
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
                token.into(),
                recipient.clone(),
                10u128.into(),
            ),
            Error::<Test>::BridgeDisabled
        );
        assert_noop!(
            JettonApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                TON_APP_ADDRESS,
                TON,
                18
            ),
            Error::<Test>::BridgeDisabled
        );
        assert_noop!(
            JettonApp::register_asset_inner(
                TON,
                token,
                bridge_types::types::AssetKind::Sidechain,
                18
            ),
            Error::<Test>::BridgeDisabled
        );
        assert_noop!(
            JettonApp::transfer(BASE_NETWORK_ID.into(), TON, recipient.clone(), token, 10),
            Error::<Test>::BridgeDisabled
        );
        assert_noop!(
            JettonApp::refund(
                BASE_NETWORK_ID.into(),
                H256::zero(),
                recipient.clone(),
                TON,
                10
            ),
            Error::<Test>::BridgeDisabled
        );
        assert!(JettonApp::list_apps().is_empty());
        assert!(JettonApp::list_supported_assets(BASE_NETWORK_ID.into()).is_empty());
        assert_eq!(AppInfo::<Test>::get(), app);
        assert_eq!(TokenAddresses::<Test>::get(TON), Some(token));
        assert_eq!(Tokens::total_balance(TON, &recipient), 0);
    });
}
