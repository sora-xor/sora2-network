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
use crate::{AppAddresses, AssetKinds, AssetsByAddresses, SidechainPrecision, TokenAddresses};
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::traits::BridgeApp;
use bridge_types::types::{
    AssetKind, BridgeAssetInfo, CallOriginOutput, GenericAdditionalInboundData,
};
use bridge_types::{EVMChainId, GenericNetworkId, H160, H256};
use frame_support::assert_noop;
use frame_support::assert_ok;
use sp_keyring::Sr25519Keyring as Keyring;
use traits::MultiCurrency;

fn relayer_signature(who: &AccountId, network: EVMChainId) -> (H160, sp_core::ecdsa::Signature) {
    use sp_core::Pair;
    let pair = sp_core::ecdsa::Pair::from_seed(&[7; 32]);
    let message = FungibleApp::get_claim_prehashed_message(network, who);
    let signature = pair.sign_prehashed(&message.0);
    let public = sp_io::crypto::secp256k1_ecdsa_recover(&signature.0, &message.0)
        .ok()
        .unwrap();
    (
        H160::from_slice(&sp_io::hashing::keccak_256(&public)[12..]),
        signature,
    )
}

#[test]
fn base_fee_updates_require_newer_evm_blocks_and_expire_at_lifetime_boundary() {
    use bridge_types::traits::EVMFeeHandler;
    new_tester().execute_with(|| {
        assert_noop!(
            FungibleApp::get_latest_base_fee(BASE_NETWORK_ID),
            Error::<Test>::BaseFeeIsNotAvailable
        );
        assert!(FungibleApp::can_update_base_fee(
            BASE_NETWORK_ID,
            10u64.into(),
            100
        ));
        FungibleApp::update_base_fee(BASE_NETWORK_ID, 10u64.into(), 100);
        assert_eq!(
            FungibleApp::get_latest_base_fee(BASE_NETWORK_ID),
            Ok(10u64.into())
        );
        assert!(!FungibleApp::can_update_base_fee(
            BASE_NETWORK_ID,
            20u64.into(),
            100
        ));
        FungibleApp::update_base_fee(BASE_NETWORK_ID, 20u64.into(), 99);
        assert_eq!(
            FungibleApp::get_latest_base_fee(BASE_NETWORK_ID),
            Ok(10u64.into())
        );
        System::set_block_number(101);
        assert_eq!(
            FungibleApp::get_latest_base_fee(BASE_NETWORK_ID),
            Ok(10u64.into())
        );
        System::set_block_number(102);
        assert_noop!(
            FungibleApp::get_latest_base_fee(BASE_NETWORK_ID),
            Error::<Test>::BaseFeeLifetimeExceeded
        );
        FungibleApp::update_base_fee(BASE_NETWORK_ID, 30u64.into(), 101);
        assert_eq!(
            FungibleApp::get_latest_base_fee(BASE_NETWORK_ID),
            Ok(30u64.into())
        );
        let info = crate::BaseFees::<Test>::get(BASE_NETWORK_ID).unwrap();
        assert_eq!(info.updated, 102);
        assert_eq!(info.evm_block_number, 101);
    });
}

#[test]
fn transfer_fees_are_collected_and_signed_claims_pay_only_available_liability() {
    use crate::mock::Currencies;
    use bridge_types::traits::{EVMBridgeWithdrawFee, EVMFeeHandler};
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Bob.into();
        assert_noop!(
            FungibleApp::get_network_fee_asset(BASE_NETWORK_ID),
            Error::<Test>::AppIsNotRegistered
        );
        assert_ok!(FungibleApp::register_asset_inner(
            BASE_NETWORK_ID,
            ETH,
            H160::zero(),
            AssetKind::Sidechain,
            18
        ));
        assert_eq!(FungibleApp::get_network_fee_asset(BASE_NETWORK_ID), Ok(ETH));
        let initial = 10_000_000_000_000_000u128;
        assert_ok!(Tokens::deposit(ETH, &owner, initial));
        FungibleApp::update_base_fee(BASE_NETWORK_ID, 10u64.into(), 1);
        assert_ok!(FungibleApp::withdraw_transfer_fee(
            &owner,
            BASE_NETWORK_ID,
            XOR
        ));
        let charged = (u128::from(crate::TRANSFER_MAX_GAS) + 1) * (5_000_000_000u128 + 10);
        assert_eq!(FungibleApp::collected_fees(BASE_NETWORK_ID), charged.into());
        assert_eq!(Currencies::total_balance(ETH, &owner), initial - charged);
        let (relayer, signature) = relayer_signature(&owner, BASE_NETWORK_ID);
        FungibleApp::on_fee_paid(BASE_NETWORK_ID, relayer, (charged + 5).into());
        assert_ok!(FungibleApp::claim_relayer_fees(
            RuntimeOrigin::signed(owner.clone()),
            BASE_NETWORK_ID,
            relayer,
            signature
        ));
        assert_eq!(Currencies::total_balance(ETH, &owner), initial);
        assert_eq!(FungibleApp::collected_fees(BASE_NETWORK_ID), 0u64.into());
        assert_eq!(
            FungibleApp::spent_fees(BASE_NETWORK_ID, relayer),
            5u64.into()
        );
        assert_noop!(
            FungibleApp::claim_relayer_fees(
                RuntimeOrigin::signed(owner),
                BASE_NETWORK_ID,
                relayer,
                signature
            ),
            Error::<Test>::NotEnoughFeesCollected
        );
    });
}

#[test]
fn relayer_claims_bind_account_nonce_network_and_address_and_reject_empty_claims() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let (relayer, signature) = relayer_signature(&owner, BASE_NETWORK_ID);
        assert_ok!(FungibleApp::ensure_can_claim_relayer_fees(
            &owner,
            BASE_NETWORK_ID,
            relayer,
            signature
        ));
        assert_noop!(
            FungibleApp::ensure_can_claim_relayer_fees(
                &owner,
                BASE_NETWORK_ID,
                H160::zero(),
                signature
            ),
            Error::<Test>::InvalidSignature
        );
        assert_noop!(
            FungibleApp::ensure_can_claim_relayer_fees(
                &Keyring::Bob.into(),
                BASE_NETWORK_ID,
                relayer,
                signature
            ),
            Error::<Test>::InvalidSignature
        );
        assert_noop!(
            FungibleApp::ensure_can_claim_relayer_fees(
                &owner,
                H256::repeat_byte(42),
                relayer,
                signature
            ),
            Error::<Test>::InvalidSignature
        );
        assert_noop!(
            FungibleApp::ensure_can_claim_relayer_fees(
                &owner,
                BASE_NETWORK_ID,
                relayer,
                sp_core::ecdsa::Signature::from_raw([0; 65])
            ),
            Error::<Test>::InvalidSignature
        );
        assert_noop!(
            FungibleApp::claim_relayer_fees(
                RuntimeOrigin::signed(owner.clone()),
                BASE_NETWORK_ID,
                relayer,
                signature
            ),
            Error::<Test>::NothingToClaim
        );
        System::inc_account_nonce(&owner);
        assert_noop!(
            FungibleApp::ensure_can_claim_relayer_fees(&owner, BASE_NETWORK_ID, relayer, signature),
            Error::<Test>::InvalidSignature
        );
    });
}

#[test]
fn app_trait_routes_preserve_refunds_and_reject_invalid_networks() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Bob.into();
        assert_ok!(Tokens::deposit(XOR, &owner, 10));
        let invalid = GenericNetworkId::EVMLegacy(0);
        assert!(!FungibleApp::is_asset_supported(invalid, XOR));
        assert!(FungibleApp::list_supported_assets(invalid).is_empty());
        assert_eq!(FungibleApp::list_apps().len(), 1);
        assert_noop!(
            FungibleApp::transfer(invalid, XOR, owner.clone(), H160::zero(), 10),
            Error::<Test>::InvalidNetwork
        );
        assert_noop!(
            FungibleApp::refund(invalid, H256::zero(), owner.clone(), XOR, 10),
            Error::<Test>::InvalidNetwork
        );
        assert_noop!(
            FungibleApp::refund_inner(BASE_NETWORK_ID, owner.clone(), XOR, 0),
            Error::<Test>::WrongAmount
        );
        assert_noop!(
            FungibleApp::refund_inner(BASE_NETWORK_ID, owner.clone(), H256::repeat_byte(55), 10),
            Error::<Test>::TokenIsNotRegistered
        );
        assert_ok!(FungibleApp::transfer(
            BASE_NETWORK_ID.into(),
            XOR,
            owner.clone(),
            H160::zero(),
            10
        ));
        assert_ok!(FungibleApp::refund(
            BASE_NETWORK_ID.into(),
            H256::zero(),
            owner.clone(),
            XOR,
            10
        ));
        let id = H256::repeat_byte(8);
        sp_io::storage::set(
            &bridge_types::recovery::message_key(BASE_NETWORK_ID.into(), id),
            &[1],
        );
        assert_noop!(
            FungibleApp::refund(BASE_NETWORK_ID.into(), id, owner, XOR, 10),
            Error::<Test>::RecoveryHalted
        );
        assert_eq!(
            FungibleApp::is_asset_supported_weight(),
            frame_support::weights::Weight::zero()
        );
        assert_eq!(
            FungibleApp::refund_weight(),
            frame_support::weights::Weight::zero()
        );
        assert_eq!(
            FungibleApp::transfer_weight(),
            <() as crate::WeightInfo>::burn()
        );
    });
}

#[test]
fn bridge_call_conversion_preserves_uint256_and_registration_fields() {
    use bridge_types::substrate::FAAppCall;
    let call: crate::Call<Test> = FAAppCall::Transfer {
        sender: H160::repeat_byte(4),
        recipient: Keyring::Alice.into(),
        amount: H256::repeat_byte(5),
        token: H160::repeat_byte(6),
    }
    .into();
    assert_eq!(
        call,
        crate::Call::<Test>::mint {
            sender: H160::repeat_byte(4),
            recipient: Keyring::Alice.into(),
            amount: bridge_types::U256::from_big_endian(&[5; 32]),
            token: H160::repeat_byte(6)
        }
    );
    let call: crate::Call<Test> = FAAppCall::FinalizeAssetRegistration {
        asset_id: XOR,
        token: H160::repeat_byte(7),
    }
    .into();
    assert_eq!(
        call,
        crate::Call::<Test>::register_asset_internal {
            asset_id: XOR,
            contract: H160::repeat_byte(7)
        }
    );
}

#[test]
fn existing_sidechain_asset_registration_records_both_directions_and_rejects_duplicates() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(8);
        assert_ok!(FungibleApp::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID,
            address,
            ETH,
            12,
        ));
        assert_eq!(
            FungibleApp::asset_by_address(BASE_NETWORK_ID, address),
            Some(ETH)
        );
        assert_eq!(
            FungibleApp::token_address(BASE_NETWORK_ID, ETH),
            Some(address)
        );
        assert_eq!(
            FungibleApp::asset_kind(BASE_NETWORK_ID, ETH),
            Some(AssetKind::Sidechain)
        );
        assert_eq!(
            FungibleApp::sidechain_precision(BASE_NETWORK_ID, ETH),
            Some(12)
        );
        assert_noop!(
            FungibleApp::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                ETH,
                12,
            ),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_noop!(
            FungibleApp::register_sidechain_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                b"ETH".to_vec(),
                b"Ether".to_vec(),
                12,
            ),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_noop!(
            FungibleApp::register_asset_inner(
                BASE_NETWORK_ID,
                ETH,
                H160::repeat_byte(9),
                AssetKind::Sidechain,
                12,
            ),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_noop!(
            FungibleApp::register_thischain_asset(RuntimeOrigin::root(), BASE_NETWORK_ID, ETH),
            Error::<Test>::TokenAlreadyRegistered
        );
        assert_noop!(
            FungibleApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                ETH,
                12
            ),
            Error::<Test>::AppAlreadyRegistered
        );
        assert_noop!(
            FungibleApp::register_network(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                b"ETH".to_vec(),
                b"Ether".to_vec(),
                12
            ),
            Error::<Test>::AppAlreadyRegistered
        );
    });
}

#[test]
fn recovery_halt_blocks_registration_and_relayer_payouts_before_mutation() {
    new_tester().execute_with(|| {
        sp_io::storage::set(b":sora-nexus:recovery:bridge-halt:v1", &[1]);
        let address = H160::repeat_byte(8);
        assert_noop!(
            FungibleApp::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                ETH,
                12
            ),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::register_sidechain_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                b"ETH".to_vec(),
                b"Ether".to_vec(),
                12
            ),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::register_thischain_asset(RuntimeOrigin::root(), BASE_NETWORK_ID, ETH),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                ETH,
                12
            ),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::register_network(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                address,
                b"ETH".to_vec(),
                b"Ether".to_vec(),
                12
            ),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::register_asset_internal(RuntimeOrigin::root(), ETH, address),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::claim_relayer_fees(
                RuntimeOrigin::signed(Keyring::Alice.into()),
                BASE_NETWORK_ID,
                address,
                sp_core::ecdsa::Signature::from_raw([0; 65])
            ),
            Error::<Test>::RecoveryHalted
        );
    });
}

fn last_event() -> RuntimeEvent {
    System::events().pop().expect("Event expected").event
}

#[test]
fn recovery_halt_rejects_direct_burn_and_refund_before_balance_changes() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Bob.into();
        sp_io::storage::set(b":sora-nexus:recovery:bridge-halt:v1", &[1]);
        assert_noop!(
            FungibleApp::burn_inner(owner.clone(), BASE_NETWORK_ID, XOR, H160::repeat_byte(9), 1),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::refund_inner(BASE_NETWORK_ID, owner, XOR, 1),
            Error::<Test>::RecoveryHalted
        );
    });
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
fn deprecated_token_address_cannot_be_registered() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(99);
        let network_id = BASE_NETWORK_ID;

        assert_noop!(
            FungibleApp::register_sidechain_asset(
                RuntimeOrigin::root(),
                network_id,
                address,
                "OLD".to_string().into(),
                "OLD".to_string().into(),
                18,
            ),
            Error::<Test>::DeprecatedTokenAddress
        );

        assert_noop!(
            FungibleApp::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                network_id,
                address,
                ETH,
                18,
            ),
            Error::<Test>::DeprecatedTokenAddress
        );

        let who = AppAddresses::<Test>::get(network_id).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {
            network_id: GenericNetworkId::EVM(network_id),
            additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData { source: who }),
            ..Default::default()
        });
        assert_noop!(
            FungibleApp::register_asset_internal(origin.into(), ETH, address),
            Error::<Test>::DeprecatedTokenAddress
        );

        assert_noop!(
            FungibleApp::register_asset_inner(
                network_id,
                H256::repeat_byte(55),
                address,
                AssetKind::Thischain,
                18,
            ),
            Error::<Test>::DeprecatedTokenAddress
        );

        let other_network_id = EVMChainId::from_low_u64_be(77);
        assert_noop!(
            FungibleApp::register_network(
                RuntimeOrigin::root(),
                network_id,
                address,
                "OLD".to_string().into(),
                "OLD".to_string().into(),
                18,
            ),
            Error::<Test>::DeprecatedTokenAddress
        );
        assert_noop!(
            FungibleApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                network_id,
                address,
                ETH,
                18,
            ),
            Error::<Test>::DeprecatedTokenAddress
        );
        assert_ok!(FungibleApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            other_network_id,
            address,
            H256::repeat_byte(56),
            18,
        ));
    })
}

#[test]
fn deprecated_token_address_cannot_be_minted_or_burned() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(99);
        let network_id = BASE_NETWORK_ID;
        let asset_id = ETH;

        TokenAddresses::<Test>::insert(network_id, asset_id, address);
        AssetsByAddresses::<Test>::insert(network_id, address, asset_id);
        AssetKinds::<Test>::insert(network_id, asset_id, AssetKind::Thischain);
        SidechainPrecision::<Test>::insert(network_id, asset_id, 18);

        let who = AppAddresses::<Test>::get(network_id).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {
            network_id: GenericNetworkId::EVM(network_id),
            additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData { source: who }),
            ..Default::default()
        });
        let recipient: AccountId = Keyring::Charlie.into();
        assert_noop!(
            FungibleApp::mint(
                origin.into(),
                address,
                H160::repeat_byte(1),
                recipient.clone(),
                10u32.into(),
            ),
            Error::<Test>::DeprecatedTokenAddress
        );
        assert_eq!(Tokens::total_balance(asset_id, &recipient), 0);

        let bob: AccountId = Keyring::Bob.into();
        Tokens::deposit(asset_id, &bob, 500).unwrap();
        assert_eq!(Tokens::total_balance(asset_id, &bob), 500);
        assert_noop!(
            FungibleApp::burn(
                RuntimeOrigin::signed(bob.clone()),
                network_id,
                asset_id,
                H160::repeat_byte(2),
                10,
            ),
            Error::<Test>::DeprecatedTokenAddress
        );
        assert_eq!(Tokens::total_balance(asset_id, &bob), 500);
    })
}

#[test]
fn deprecated_token_address_is_not_reported_as_supported() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(99);
        let network_id = BASE_NETWORK_ID;
        let asset_id = ETH;

        TokenAddresses::<Test>::insert(network_id, asset_id, address);
        AssetsByAddresses::<Test>::insert(network_id, address, asset_id);
        AssetKinds::<Test>::insert(network_id, asset_id, AssetKind::Thischain);
        SidechainPrecision::<Test>::insert(network_id, asset_id, 18);

        assert!(
            !<FungibleApp as BridgeApp<AccountId, H160, H256, u128>>::is_asset_supported(
                GenericNetworkId::EVM(network_id),
                asset_id,
            )
        );
        assert!(
            <FungibleApp as BridgeApp<AccountId, H160, H256, u128>>::list_supported_assets(
                GenericNetworkId::EVM(network_id)
            )
            .into_iter()
            .all(|asset| !matches!(asset, BridgeAssetInfo::EVM(info) if info.asset_id == asset_id))
        );
    })
}

#[test]
fn deprecated_token_address_rejection_is_network_scoped() {
    new_tester().execute_with(|| {
        let address = H160::repeat_byte(99);
        let app_contract = H160::repeat_byte(100);
        let network_id = EVMChainId::from_low_u64_be(77);

        assert_ok!(FungibleApp::register_network(
            RuntimeOrigin::root(),
            network_id,
            app_contract,
            "NET".to_string().into(),
            "Network Asset".to_string().into(),
            18,
        ));
        assert_ok!(FungibleApp::register_sidechain_asset(
            RuntimeOrigin::root(),
            network_id,
            address,
            "OLD".to_string().into(),
            "OLD".to_string().into(),
            18,
        ));
        let asset_id = AssetsByAddresses::<Test>::get(network_id, address).unwrap();
        let recipient: AccountId = Keyring::Charlie.into();

        assert_ok!(FungibleApp::mint(
            dispatch::RawOrigin::new(CallOriginOutput {
                network_id: GenericNetworkId::EVM(network_id),
                additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData {
                    source: app_contract,
                }),
                ..Default::default()
            })
            .into(),
            address,
            H160::repeat_byte(1),
            recipient.clone(),
            10u32.into(),
        ));
        assert_eq!(Tokens::total_balance(asset_id, &recipient), 10);
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

#[test]
fn inbound_origin_domains_sources_and_quarantines_cannot_credit_assets() {
    new_tester().execute_with(|| {
        let origin = |network, additional, message_id| -> RuntimeOrigin {
            dispatch::RawOrigin::new(CallOriginOutput {
                network_id: network,
                additional,
                message_id,
                ..Default::default()
            })
            .into()
        };
        let bad_network = bridge_types::SubNetworkId::Mainnet.into();
        let evm_data = GenericAdditionalInboundData::EVM(AdditionalEVMInboundData {
            source: H160::repeat_byte(2),
        });
        for (network, additional) in [
            (bad_network, evm_data.clone()),
            (BASE_NETWORK_ID.into(), GenericAdditionalInboundData::Sub),
        ] {
            assert_noop!(
                FungibleApp::mint(
                    origin(network, additional.clone(), H256::zero()),
                    H160::repeat_byte(3),
                    H160::zero(),
                    Keyring::Alice.into(),
                    1u32.into()
                ),
                sp_runtime::DispatchError::BadOrigin
            );
            assert_noop!(
                FungibleApp::register_asset_internal(
                    origin(network, additional, H256::zero()),
                    ETH,
                    H160::repeat_byte(8)
                ),
                sp_runtime::DispatchError::BadOrigin
            );
        }
        let bad_source = GenericAdditionalInboundData::EVM(AdditionalEVMInboundData {
            source: H160::repeat_byte(99),
        });
        assert_noop!(
            FungibleApp::mint(
                origin(BASE_NETWORK_ID.into(), bad_source.clone(), H256::zero()),
                H160::repeat_byte(3),
                H160::zero(),
                Keyring::Alice.into(),
                1u32.into()
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            FungibleApp::register_asset_internal(
                origin(BASE_NETWORK_ID.into(), bad_source, H256::zero()),
                ETH,
                H160::repeat_byte(8)
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        let id = H256::repeat_byte(61);
        sp_io::storage::set(
            &bridge_types::recovery::message_key(BASE_NETWORK_ID.into(), id),
            &[1],
        );
        assert_noop!(
            FungibleApp::mint(
                origin(BASE_NETWORK_ID.into(), evm_data, id),
                H160::repeat_byte(3),
                H160::zero(),
                Keyring::Alice.into(),
                1u32.into()
            ),
            Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            FungibleApp::register_asset_inner(
                H256::repeat_byte(99),
                ETH,
                H160::repeat_byte(8),
                AssetKind::Sidechain,
                18
            ),
            Error::<Test>::AppIsNotRegistered
        );
    });
}
