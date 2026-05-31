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

use super::mock::*;
use super::{Assets, Error, EthBridge};
use crate::contract::{ContractEvent, DepositEvent};
use crate::offchain::SignatureParams;
use crate::requests::{
    AssetKind, IncomingAddToken, IncomingCancelOutgoingRequest, IncomingMarkAsDoneRequest,
    IncomingMetaRequestKind, IncomingRequest, IncomingRequestKind, IncomingTransactionRequestKind,
    IncomingTransfer, LoadIncomingRequest, LoadIncomingTransactionRequest, OffchainRequest,
    OutgoingAddAsset, OutgoingAddToken, OutgoingRequest, OutgoingTransfer, RequestStatus,
};
use crate::tests::mock::{get_account_id_from_seed, ExtBuilder};
use crate::tests::{
    approve_last_request, assert_incoming_request_done, request_incoming, ETH_NETWORK_ID,
};
use crate::types::{self, Log};
use crate::{
    AssetConfig, BridgeAccount, DeprecatedSidechainTokens, EthAddress,
    LegacyEthereumXorDecommissioned, LegacyEthereumXorDecommissionedAt, RegisteredAsset,
    RegisteredSidechainAsset, RegisteredSidechainToken,
    LEGACY_ETHEREUM_XOR_MASTER_CONTRACT_ADDRESS, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
};
use bridge_types::evm::EVMAppKind;
use bridge_types::traits::BridgeApp;
use bridge_types::types::{BridgeAppInfo, BridgeAssetInfo};
use bridge_types::GenericNetworkId;
use codec::Encode;
use common::{
    balance, AssetId32, AssetInfoProvider, AssetName, AssetSymbol, Balance, PredefinedAssetId,
    DEFAULT_BALANCE_PRECISION, VAL, XOR,
};
use frame_support::assert_noop;
use frame_support::sp_runtime::app_crypto::sp_core::{self, sr25519};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use sp_core::H256;
use std::collections::BTreeSet;
use std::str::FromStr;

fn val_master_deposit_log(
    destination: AccountId,
    amount: u128,
    token: EthAddress,
    sidechain_asset: H256,
) -> Log {
    deposit_log_from(
        EthBridge::val_master_contract_address(),
        destination,
        amount,
        token,
        sidechain_asset,
    )
}

fn deposit_log_from(
    address: EthAddress,
    destination: AccountId,
    amount: u128,
    token: EthAddress,
    sidechain_asset: H256,
) -> Log {
    Log {
        address: types::H160(address.0),
        topics: vec![types::H256(crate::DEPOSIT_TOPIC.0)],
        data: types::Bytes(ethabi::encode(&[
            ethabi::Token::FixedBytes(destination.encode()),
            ethabi::Token::Uint(amount.into()),
            ethabi::Token::Address(types::EthAddress::from(token.0)),
            ethabi::Token::FixedBytes(sidechain_asset.0.to_vec()),
        ])),
        removed: Some(false),
        ..Default::default()
    }
}

#[test]
fn should_mint_and_burn_sidechain_asset() {
    let (mut ext, state) = ExtBuilder::default().build();

    #[track_caller]
    fn check_invariant(asset_id: &AssetId32<PredefinedAssetId>, val: u32) {
        assert_eq!(Assets::total_issuance(asset_id).unwrap(), val.into());
    }

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = EthAddress::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        EthBridge::register_sidechain_asset(
            token_address,
            DEFAULT_BALANCE_PRECISION,
            AssetSymbol(b"TEST".to_vec()),
            AssetName(b"TEST Asset".to_vec()),
            net_id,
        )
        .unwrap();
        let (asset_id, asset_kind) =
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address, net_id)
                .unwrap()
                .unwrap();
        assert_eq!(asset_kind, AssetKind::Sidechain);
        check_invariant(&asset_id, 0);
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: EthAddress::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind,
            amount: 100u32.into(),
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
            should_take_fee: false,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        check_invariant(&asset_id, 100);
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        check_invariant(&asset_id, 0);
    });
}

#[test]
fn should_not_burn_or_mint_sidechain_owned_asset() {
    let (mut ext, state) = ExtBuilder::default().build();

    #[track_caller]
    fn check_invariant() {
        assert_eq!(
            Assets::total_issuance(&XOR.into()).unwrap(),
            balance!(350000)
        );
    }

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_eq!(
            EthBridge::registered_asset(net_id, AssetId32::from(XOR)).unwrap(),
            AssetKind::SidechainOwned
        );
        check_invariant();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: EthAddress::from([1; 20]),
            to: alice.clone(),
            asset_id: XOR.into(),
            asset_kind: AssetKind::SidechainOwned,
            amount: 100u32.into(),
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
            should_take_fee: false,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        check_invariant();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        check_invariant();
    });
}

#[test]
fn should_register_and_find_asset_ids() {
    let (mut ext, _state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        // gets a known asset
        let (asset_id, asset_kind) = EthBridge::get_asset_by_raw_asset_id(
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code),
            &EthAddress::zero(),
            net_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(asset_id, XOR.into());
        assert_eq!(asset_kind, AssetKind::Thischain);
        let token_address = EthAddress::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        // registers unknown token
        assert!(
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address, net_id)
                .unwrap()
                .is_none()
        );
        // gets registered asset ID, associated with the token
        EthBridge::register_sidechain_asset(
            token_address,
            DEFAULT_BALANCE_PRECISION,
            AssetSymbol(b"TEST".to_vec()),
            AssetName(b"TEST Asset".to_vec()),
            net_id,
        )
        .unwrap();
        let (asset_id, asset_kind) =
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address, net_id)
                .unwrap()
                .unwrap();
        assert_eq!(
            asset_id,
            AssetId32::from_bytes(hex!(
                "00998577153deb622b5d7faabf23846281a8b074e1d4eebd31bca9dbe2c23006"
            ))
        );
        assert_eq!(asset_kind, AssetKind::Sidechain);
        assert_eq!(
            EthBridge::registered_sidechain_token(net_id, &asset_id).unwrap(),
            token_address
        );
        assert_eq!(
            EthBridge::registered_sidechain_asset(net_id, &token_address).unwrap(),
            asset_id
        );
    });
}

#[test]
fn should_add_asset() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = Assets::register_from(
            &alice,
            AssetSymbol(b"TEST".to_vec()),
            AssetName(b"TEST Asset".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id,
        ));
        assert!(EthBridge::registered_asset(net_id, asset_id).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            EthBridge::registered_asset(net_id, asset_id).unwrap(),
            AssetKind::Thischain
        );
    });
}

#[test]
fn add_asset_pending_helper_tracks_request_lifecycle() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = Assets::register_from(
            &alice,
            AssetSymbol(b"PEND".to_vec()),
            AssetName(b"Pending Asset".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert!(!EthBridge::is_add_asset_request_pending(net_id, asset_id));
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id,
        ));
        assert!(EthBridge::is_add_asset_request_pending(net_id, asset_id));

        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(!EthBridge::is_add_asset_request_pending(net_id, asset_id));
    });
}

#[test]
fn add_asset_pending_helper_ignores_finished_stale_queue_entries() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let authority = EthBridge::authority_account().unwrap();

        for (nonce, status) in [
            (
                100u64,
                Some(RequestStatus::Failed(Error::UnsupportedToken.into())),
            ),
            (101u64, Some(RequestStatus::Done)),
            (102u64, None),
        ] {
            let asset_id = Assets::register_from(
                &alice,
                AssetSymbol(format!("STL{nonce}").into_bytes()),
                AssetName(format!("Stale Add Asset {nonce}").into_bytes()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                common::AssetType::Regular,
                None,
                None,
            )
            .unwrap();
            let request =
                OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset::<Runtime> {
                    author: authority.clone(),
                    asset_id,
                    nonce,
                    network_id: net_id,
                    timepoint: Default::default(),
                }));
            let hash = request.hash();
            crate::Requests::<Runtime>::insert(net_id, hash, request);
            if let Some(status) = status {
                crate::RequestStatuses::<Runtime>::insert(net_id, hash, status);
            }
            crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));

            assert!(!EthBridge::is_add_asset_request_pending(net_id, asset_id));
            assert_ok!(EthBridge::add_asset(
                RuntimeOrigin::root(),
                asset_id,
                net_id,
            ));
            assert!(EthBridge::is_add_asset_request_pending(net_id, asset_id));
        }
    });
}

#[test]
fn should_reject_duplicate_pending_add_asset_request() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = Assets::register_from(
            &alice,
            AssetSymbol(b"DUPA".to_vec()),
            AssetName(b"Duplicate Pending Add".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id,
        ));
        let queue_len = crate::RequestsQueue::<Runtime>::get(net_id).len();

        assert_noop!(
            EthBridge::add_asset(RuntimeOrigin::root(), asset_id, net_id),
            Error::TokenIsAlreadyAdded
        );
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id).len(),
            queue_len
        );
    });
}

#[test]
fn should_add_token() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let symbol = "TEST".into();
        let name = "Runtime Token".into();
        let decimals = 18;
        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            symbol,
            name,
            decimals,
            ETH_NETWORK_ID,
        ));
        assert!(EthBridge::registered_sidechain_asset(net_id, &token_address).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        let asset_id_opt = EthBridge::registered_sidechain_asset(net_id, &token_address);
        assert!(asset_id_opt.is_some());
        assert_eq!(
            EthBridge::registered_asset(net_id, asset_id_opt.unwrap()).unwrap(),
            AssetKind::Sidechain
        );
    });
}

#[test]
fn add_sidechain_token_pending_helper_tracks_request_lifecycle() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));

        assert!(!EthBridge::is_add_token_request_pending(
            net_id,
            token_address
        ));
        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            "PENDTOK".into(),
            "Pending Token".into(),
            DEFAULT_BALANCE_PRECISION,
            net_id,
        ));
        assert!(EthBridge::is_add_token_request_pending(
            net_id,
            token_address
        ));

        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(!EthBridge::is_add_token_request_pending(
            net_id,
            token_address
        ));
    });
}

#[test]
fn add_sidechain_token_pending_helper_ignores_finished_stale_queue_entries() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let authority = EthBridge::authority_account().unwrap();

        for (nonce, status) in [
            (
                110u64,
                Some(RequestStatus::Failed(Error::UnsupportedToken.into())),
            ),
            (111u64, Some(RequestStatus::Done)),
            (112u64, None),
        ] {
            let token_address = EthAddress::from([nonce as u8; 20]);
            let request =
                OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                    author: authority.clone(),
                    token_address,
                    symbol: format!("STK{nonce}"),
                    name: format!("Stale Token {nonce}"),
                    decimals: DEFAULT_BALANCE_PRECISION,
                    nonce,
                    network_id: net_id,
                    timepoint: Default::default(),
                }));
            let hash = request.hash();
            crate::Requests::<Runtime>::insert(net_id, hash, request);
            if let Some(status) = status {
                crate::RequestStatuses::<Runtime>::insert(net_id, hash, status);
            }
            crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));

            assert!(!EthBridge::is_add_token_request_pending(
                net_id,
                token_address
            ));
            assert_ok!(EthBridge::add_sidechain_token(
                RuntimeOrigin::root(),
                token_address,
                format!("TOK{nonce}"),
                format!("Token {nonce}"),
                DEFAULT_BALANCE_PRECISION,
                net_id,
            ));
            assert!(EthBridge::is_add_token_request_pending(
                net_id,
                token_address
            ));
        }
    });
}

#[test]
fn should_reject_duplicate_pending_add_sidechain_token_request() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = EthAddress::from(hex!("f88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));

        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            "DUPTOK".into(),
            "Duplicate Pending Token".into(),
            DEFAULT_BALANCE_PRECISION,
            net_id,
        ));
        let queue_len = crate::RequestsQueue::<Runtime>::get(net_id).len();

        assert_noop!(
            EthBridge::add_sidechain_token(
                RuntimeOrigin::root(),
                token_address,
                "DUPTOK".into(),
                "Duplicate Pending Token".into(),
                DEFAULT_BALANCE_PRECISION,
                net_id,
            ),
            Error::SidechainAssetIsAlreadyRegistered
        );
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id).len(),
            queue_len
        );
    });
}

#[test]
fn should_not_add_token_if_not_bridge_account() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let symbol = "TEST".into();
        let name = "Runtime Token".into();
        let decimals = 18;
        assert_err!(
            EthBridge::add_sidechain_token(
                RuntimeOrigin::signed(bob),
                token_address,
                symbol,
                name,
                decimals,
                net_id,
            ),
            frame_support::sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn should_reserve_owned_asset_on_different_networks() {
    let mut builder = ExtBuilder::default();
    let net_id_0 = ETH_NETWORK_ID;
    let net_id_1 = builder.add_network(vec![], None, None, Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = XOR.into();
        Assets::mint_to(&asset_id, &alice, &alice, 100u32.into()).unwrap();
        Assets::mint_to(
            &asset_id,
            &alice,
            &state.networks[&net_id_0].config.bridge_account_id,
            100u32.into(),
        )
        .unwrap();
        Assets::mint_to(
            &asset_id,
            &alice,
            &state.networks[&net_id_1].config.bridge_account_id,
            100u32.into(),
        )
        .unwrap();
        let supply = Assets::total_issuance(&asset_id).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_0,
        ));
        approve_last_request(&state, net_id_0).expect("request wasn't approved");
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");
        assert_eq!(Assets::total_issuance(&asset_id).unwrap(), supply);

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id_0,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: EthAddress::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount: 50u32.into(),
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_0,
            should_take_fee: false,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id_1,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: EthAddress::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount: 50u32.into(),
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_1,
            should_take_fee: false,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        assert_eq!(Assets::total_issuance(&asset_id).unwrap(), supply);
    });
}

#[test]
fn should_handle_sidechain_and_thischain_asset_on_different_networks() {
    let mut builder = ExtBuilder::default();
    let net_id_0 = ETH_NETWORK_ID;
    let net_id_1 = builder.add_network(vec![], None, None, Default::default());
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Register token on the first network.
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            "TEST".into(),
            "Runtime Token".into(),
            DEFAULT_BALANCE_PRECISION,
            net_id_0,
        ));
        approve_last_request(&state, net_id_0).expect("request wasn't approved");
        let asset_id = EthBridge::registered_sidechain_asset(net_id_0, &token_address)
            .expect("Asset wasn't found.");
        assert_eq!(
            EthBridge::registered_asset(net_id_0, asset_id).unwrap(),
            AssetKind::Sidechain
        );

        // Register the newly generated asset in the second network
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");
        assert_eq!(
            EthBridge::registered_asset(net_id_1, asset_id).unwrap(),
            AssetKind::Thischain
        );
        Assets::mint_to(
            &asset_id,
            &state.networks[&net_id_0].config.bridge_account_id,
            &state.networks[&net_id_1].config.bridge_account_id,
            100u32.into(),
        )
        .unwrap();
        let supply = Assets::total_issuance(&asset_id).unwrap();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id_0,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: EthAddress::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Sidechain,
            amount: 50u32.into(),
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_0,
            should_take_fee: false,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();

        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id_1,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: EthAddress::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount: 50u32.into(),
            author: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_1,
            should_take_fee: false,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();

        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_0,
        ));
        approve_last_request(&state, net_id_0).expect("request wasn't approved");
        assert_eq!(Assets::total_issuance(&asset_id).unwrap(), supply);
    });
}

#[test]
fn should_convert_amount_for_a_token_with_non_default_precision() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = "USDT".into();
        let name = "Tether USD".into();
        let decimals = 6;
        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            ticker,
            name,
            decimals,
            net_id,
        ));
        assert!(EthBridge::registered_sidechain_asset(net_id, &token_address).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        let asset_id = EthBridge::registered_sidechain_asset(net_id, &token_address)
            .expect("failed to register sidechain asset");
        assert_eq!(
            EthBridge::registered_asset(net_id, &asset_id).unwrap(),
            AssetKind::Sidechain
        );
        assert_eq!(
            EthBridge::sidechain_asset_precision(net_id, &asset_id),
            decimals
        );
        assert_eq!(
            Assets::get_asset_info(&asset_id).2,
            DEFAULT_BALANCE_PRECISION
        );
        // Incoming transfer part.
        assert_eq!(
            Assets::total_balance(&asset_id, &alice).unwrap(),
            balance!(0)
        );
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let sidechain_amount = 1 * 10_u128.pow(decimals as u32);
        let incoming_trasfer = IncomingRequest::try_from_contract_event(
            ContractEvent::Deposit(DepositEvent::new(
                alice.clone(),
                sidechain_amount.into(),
                token_address,
                H256::zero(),
            )),
            LoadIncomingTransactionRequest::new(
                alice.clone(),
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        )
        .unwrap();
        assert_incoming_request_done(&state, incoming_trasfer).unwrap();
        assert_eq!(
            Assets::total_balance(&asset_id, &alice).unwrap(),
            balance!(1)
        );
        // Outgoing transfer part.
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id.clone(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            balance!(1),
            net_id,
        ));
        let outgoing_transfer =
            match approve_last_request(&state, net_id).expect("request wasn't approved") {
                (OutgoingRequest::Transfer(transfer), _) => transfer,
                _ => unreachable!(),
            };
        assert_eq!(outgoing_transfer.amount, balance!(1));
        assert_eq!(
            outgoing_transfer.sidechain_amount().unwrap().0,
            sidechain_amount
        );
        assert_eq!(
            Assets::total_balance(&asset_id, &alice).unwrap(),
            balance!(0)
        );
    });
}

#[test]
fn should_convert_amount_for_indivisible_token() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = AssetSymbol::from_str("NFT").unwrap();
        let name = AssetName::from_str("NonFungTok").unwrap();
        let decimals = 0;
        let amount = 1;
        let asset_id = Assets::register_from(
            &alice,
            ticker,
            name,
            decimals,
            amount,
            false,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id
        ));
        assert!(EthBridge::registered_asset(net_id, asset_id).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            EthBridge::registered_asset(net_id, asset_id).unwrap(),
            AssetKind::Thischain
        );
        // Outgoing transfer part.
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id.clone(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            1,
            net_id,
        ));
        let outgoing_transfer =
            match approve_last_request(&state, net_id).expect("request wasn't approved") {
                (OutgoingRequest::Transfer(transfer), _) => transfer,
                _ => unreachable!(),
            };
        assert_eq!(outgoing_transfer.amount, amount);
        assert_eq!(outgoing_transfer.sidechain_amount().unwrap().0, amount);
        assert_eq!(Assets::total_balance(&asset_id, &alice).unwrap(), 0);

        // Incoming transfer part.
        assert_eq!(Assets::total_balance(&asset_id, &alice).unwrap(), 0);
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let incoming_trasfer = IncomingRequest::try_from_contract_event(
            ContractEvent::Deposit(DepositEvent::new(
                alice.clone(),
                amount.into(),
                token_address,
                asset_id.into(),
            )),
            LoadIncomingTransactionRequest::new(
                alice.clone(),
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        )
        .unwrap();
        assert_incoming_request_done(&state, incoming_trasfer).unwrap();
        assert_eq!(Assets::total_balance(&asset_id, &alice).unwrap(), amount);
    });
}

#[test]
fn should_fail_convert_amount_for_a_token_with_non_default_precision() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = "USDT".into();
        let name = "Tether USD".into();
        let decimals = 6;
        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            ticker,
            name,
            decimals,
            net_id,
        ));
        assert!(EthBridge::registered_sidechain_asset(net_id, &token_address).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        let asset_id = EthBridge::registered_sidechain_asset(net_id, &token_address)
            .expect("failed to register sidechain asset");
        assert_eq!(
            Assets::total_balance(&asset_id, &alice).unwrap(),
            balance!(0)
        );
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let sidechain_amount = 1_000_000_000_000_000_000_000 * 10_u128.pow(decimals as u32);
        let incoming_trasfer_result = IncomingRequest::try_from_contract_event(
            ContractEvent::Deposit(DepositEvent::new(
                alice.clone(),
                sidechain_amount.into(),
                token_address,
                H256::zero(),
            )),
            LoadIncomingTransactionRequest::new(
                alice.clone(),
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        );
        assert_eq!(
            incoming_trasfer_result,
            Err(Error::UnsupportedAssetPrecision)
        );
    });
}

#[test]
fn should_fail_tranfer_amount_with_dust_for_a_token_with_non_default_precision() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = "USDT".into();
        let name = "Tether USD".into();
        let decimals = 6;
        assert_ok!(EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            ticker,
            name,
            decimals,
            net_id,
        ));
        assert!(EthBridge::registered_sidechain_asset(net_id, &token_address).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        let asset_id = EthBridge::registered_sidechain_asset(net_id, &token_address)
            .expect("failed to register sidechain asset");
        assert_eq!(
            Assets::total_balance(&asset_id, &alice).unwrap(),
            balance!(0)
        );
        Assets::mint_to(
            &asset_id,
            &state.networks[&net_id].config.bridge_account_id,
            &alice,
            balance!(0.1000009),
        )
        .unwrap();
        assert_noop!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                asset_id.clone(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                balance!(0.1000009),
                net_id,
            ),
            Error::NonZeroDust
        );
    });
}

#[test]
fn should_not_allow_registering_sidechain_token_with_big_precision() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = EthAddress::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = "USDT".into();
        let name = "Tether USD".into();
        let decimals = DEFAULT_BALANCE_PRECISION + 1;
        assert_noop!(
            EthBridge::add_sidechain_token(
                RuntimeOrigin::root(),
                token_address,
                ticker,
                name,
                decimals,
                net_id,
            ),
            Error::UnsupportedAssetPrecision
        );
    });
}

#[test]
fn should_remove_asset() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        assert_ok!(EthBridge::remove_sidechain_asset(
            RuntimeOrigin::root(),
            XOR,
            net_id,
        ));
        assert!(EthBridge::registered_asset(net_id, XOR).is_none());
    });
}

#[test]
fn should_not_remove_asset_with_active_outgoing_transfer_request() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(Assets::mint_to(
            &XOR,
            &state.networks[&net_id].config.bridge_account_id,
            &alice,
            100u32.into(),
        ));
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            10u32.into(),
            net_id,
        ));
        assert_noop!(
            EthBridge::remove_sidechain_asset(RuntimeOrigin::root(), XOR, net_id),
            Error::ActiveOutgoingTransferRequest
        );
    });
}

#[test]
fn should_remove_asset_after_outgoing_transfer_request_is_aborted() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(Assets::mint_to(
            &XOR,
            &state.networks[&net_id].config.bridge_account_id,
            &alice,
            100u32.into(),
        ));
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice),
            XOR,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            10u32.into(),
            net_id,
        ));
        let request_hash = *EthBridge::requests_queue(net_id)
            .last()
            .expect("outgoing transfer request should be queued");
        assert_ok!(EthBridge::abort_request(
            RuntimeOrigin::signed(state.networks[&net_id].config.bridge_account_id.clone()),
            request_hash,
            Error::Cancelled.into(),
            net_id,
        ));
        assert_ok!(EthBridge::remove_sidechain_asset(
            RuntimeOrigin::root(),
            XOR,
            net_id,
        ));
        assert!(EthBridge::registered_asset(net_id, XOR).is_none());
    });
}

#[test]
fn should_not_remove_thischain_asset_with_active_outgoing_transfer_request() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = Assets::register_from(
            &alice,
            AssetSymbol(b"TRM1".to_vec()),
            AssetName(b"Thischain Remove Guard".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            net_id,
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            EthBridge::registered_asset(net_id, asset_id),
            Some(AssetKind::Thischain)
        );

        assert_ok!(Assets::mint_to(&asset_id, &alice, &alice, 100u32.into()));
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            asset_id,
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            10u32.into(),
            net_id,
        ));

        assert_err!(
            EthBridge::remove_thischain_asset(net_id, asset_id),
            Error::ActiveOutgoingTransferRequest
        );

        let request_hash = *EthBridge::requests_queue(net_id)
            .last()
            .expect("outgoing transfer request should be queued");
        assert_ok!(EthBridge::abort_request(
            RuntimeOrigin::signed(state.networks[&net_id].config.bridge_account_id.clone()),
            request_hash,
            Error::Cancelled.into(),
            net_id,
        ));
        assert_ok!(EthBridge::remove_thischain_asset(net_id, asset_id));
        assert!(EthBridge::registered_asset(net_id, asset_id).is_none());
    });
}

#[test]
fn should_register_removed_asset() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();
        assert_ok!(EthBridge::remove_sidechain_asset(
            RuntimeOrigin::root(),
            VAL,
            net_id,
        ));
        assert!(EthBridge::registered_asset(net_id, VAL).is_none());
        assert_ok!(EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            VAL,
            token_address,
            net_id,
        ));
        assert!(EthBridge::registered_asset(net_id, VAL).is_some());
    });
}

#[test]
fn should_not_register_existing_asset() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();
        assert_err!(
            EthBridge::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                VAL,
                token_address,
                net_id,
            ),
            Error::TokenIsAlreadyAdded
        );
    });
}

#[test]
fn should_reject_legacy_ethereum_xor_token_address() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;

        assert_err!(
            EthBridge::get_asset_by_raw_asset_id(
                H256::zero(),
                &LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::get_asset_by_raw_asset_id(
                H256::repeat_byte(1),
                &LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        RegisteredSidechainAsset::<Runtime>::insert(net_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS, VAL);
        assert_err!(
            EthBridge::get_asset_by_raw_asset_id(
                H256::zero(),
                &LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                XOR,
                LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
    });
}

#[test]
fn should_allow_legacy_xor_address_on_non_ethereum_network() {
    let mut builder = ExtBuilder::default();
    let net_id = builder.add_network(vec![], None, None, Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        assert_ok!(EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            XOR,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
            net_id,
        ));
        assert_eq!(
            RegisteredSidechainToken::<Runtime>::get(net_id, XOR),
            Some(LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS)
        );
        assert_eq!(
            EthBridge::get_asset_by_raw_asset_id(
                H256::zero(),
                &LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                net_id,
            )
            .unwrap(),
            Some((XOR.into(), AssetKind::Sidechain))
        );
        assert_eq!(
            EthBridge::registered_asset(net_id, XOR),
            Some(AssetKind::Sidechain)
        );
    });
}

#[test]
fn should_allow_xor_thischain_add_request_after_legacy_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let authority = EthBridge::authority_account().unwrap();
        let initial_nonce = frame_system::Pallet::<Runtime>::account_nonce(&authority);
        let initial_queue_len = crate::RequestsQueue::<Runtime>::get(net_id).len();

        assert_noop!(
            EthBridge::add_asset(RuntimeOrigin::root(), XOR.into(), net_id),
            Error::TokenIsAlreadyAdded
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::account_nonce(&authority),
            initial_nonce
        );
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id).len(),
            initial_queue_len
        );

        assert_noop!(
            EthBridge::add_sidechain_token(
                RuntimeOrigin::root(),
                LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                "OLD".into(),
                "OLD".into(),
                DEFAULT_BALANCE_PRECISION + 1,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::account_nonce(&authority),
            initial_nonce
        );
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id).len(),
            initial_queue_len
        );
        assert!(RegisteredSidechainAsset::<Runtime>::get(
            net_id,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        )
        .is_none());

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        let post_migration_nonce = frame_system::Pallet::<Runtime>::account_nonce(&authority);
        let post_migration_queue_len = crate::RequestsQueue::<Runtime>::get(net_id).len();

        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        assert_eq!(
            frame_system::Pallet::<Runtime>::account_nonce(&authority),
            post_migration_nonce + 1
        );
        let queue = crate::RequestsQueue::<Runtime>::get(net_id);
        assert_eq!(queue.len(), post_migration_queue_len + 1);
        let request = crate::Requests::<Runtime>::get(net_id, queue.last().unwrap()).unwrap();
        assert!(matches!(
            request,
            OffchainRequest::Outgoing(OutgoingRequest::AddAsset(req), _)
                if req.asset_id == XOR.into()
        ));
    });
}

#[test]
fn should_reject_fresh_ethereum_xor_add_before_decommission_even_after_partial_cleanup() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let authority = EthBridge::authority_account().unwrap();
        let xor_asset_id: AssetId = XOR.into();

        RegisteredAsset::<Runtime>::remove(net_id, &xor_asset_id);
        RegisteredSidechainToken::<Runtime>::remove(net_id, &xor_asset_id);
        RegisteredSidechainAsset::<Runtime>::remove(net_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);

        assert!(!LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_none());
        assert!(RegisteredSidechainToken::<Runtime>::get(net_id, XOR).is_none());

        let nonce = frame_system::Pallet::<Runtime>::account_nonce(&authority);
        assert_noop!(
            EthBridge::add_asset(RuntimeOrigin::root(), XOR.into(), net_id),
            Error::DeprecatedLegacyXor
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::account_nonce(&authority),
            nonce
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).is_empty());

        let add_xor_asset = OutgoingAddAsset::<Runtime> {
            author: authority,
            asset_id: XOR.into(),
            nonce,
            network_id: net_id,
            timepoint: Default::default(),
        };
        assert_err!(add_xor_asset.finalize(), Error::DeprecatedLegacyXor);
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_none());
    });
}

#[test]
fn should_not_treat_predecommission_xor_thischain_storage_as_clean_registration() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);
        let xor_asset_id: AssetId = XOR.into();

        RegisteredAsset::<Runtime>::insert(net_id, &xor_asset_id, AssetKind::Thischain);
        RegisteredSidechainToken::<Runtime>::remove(net_id, &xor_asset_id);
        RegisteredSidechainAsset::<Runtime>::remove(net_id, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);

        assert!(!LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert!(
            !EthBridge::is_ethereum_xor_thischain_registration(net_id, &xor_asset_id),
            "XOR must not become a clean Hashi registration before legacy decommission"
        );
        assert_eq!(
            EthBridge::bridge_denomination_factor(net_id, &xor_asset_id),
            10
        );
        assert!(!<EthBridge as BridgeApp<
            AccountId,
            EthAddress,
            AssetId,
            Balance,
        >>::is_asset_supported(
            generic_network_id, xor_asset_id,
        ));
    });
}

#[test]
fn should_reject_manually_deprecated_sidechain_token_before_other_errors() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let deprecated_token = EthAddress::from([66; 20]);
        let authority = EthBridge::authority_account().unwrap();
        let initial_nonce = frame_system::Pallet::<Runtime>::account_nonce(&authority);
        let initial_queue_len = crate::RequestsQueue::<Runtime>::get(net_id).len();
        DeprecatedSidechainTokens::<Runtime>::insert(net_id, deprecated_token, true);
        RegisteredSidechainAsset::<Runtime>::insert(net_id, deprecated_token, VAL);

        assert_err!(
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &deprecated_token, net_id),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::get_asset_by_raw_asset_id(H256::repeat_byte(2), &deprecated_token, net_id),
            Error::DeprecatedLegacyXor
        );
        assert_noop!(
            EthBridge::add_sidechain_token(
                RuntimeOrigin::root(),
                deprecated_token,
                "OLD".into(),
                "OLD".into(),
                DEFAULT_BALANCE_PRECISION + 1,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                VAL,
                deprecated_token,
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_eq!(
            frame_system::Pallet::<Runtime>::account_nonce(&authority),
            initial_nonce
        );
        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id).len(),
            initial_queue_len
        );
    });
}

#[test]
fn should_finalize_xor_thischain_add_request_after_legacy_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let authority = EthBridge::authority_account().unwrap();
        let initial_xor_kind = RegisteredAsset::<Runtime>::get(net_id, XOR);

        let add_xor_asset = OutgoingRequest::AddAsset(OutgoingAddAsset::<Runtime> {
            author: authority.clone(),
            asset_id: XOR.into(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        });
        assert_err!(
            add_xor_asset.finalize(H256::repeat_byte(31)),
            Error::TokenIsAlreadyAdded
        );
        assert_eq!(
            RegisteredAsset::<Runtime>::get(net_id, XOR),
            initial_xor_kind
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_ok!(add_xor_asset.finalize(H256::repeat_byte(33)));
        assert_eq!(
            RegisteredAsset::<Runtime>::get(net_id, XOR),
            Some(AssetKind::Thischain)
        );
        assert!(RegisteredSidechainToken::<Runtime>::get(net_id, XOR).is_none());

        let add_legacy_token = OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
            author: authority,
            token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
            symbol: "OLD".into(),
            name: "OLD".into(),
            decimals: DEFAULT_BALANCE_PRECISION + 1,
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        });
        assert_err!(
            add_legacy_token.finalize(H256::repeat_byte(32)),
            Error::DeprecatedLegacyXor
        );
        assert!(RegisteredSidechainAsset::<Runtime>::get(
            net_id,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS
        )
        .is_none());
    });
}

#[test]
fn should_not_advertise_legacy_ethereum_xor_app_or_asset() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);

        assert_err!(
            EthBridge::ensure_known_contract(EthBridge::xor_master_contract_address(), net_id),
            Error::UnknownContractAddress
        );
        assert_err!(
            EthBridge::ensure_known_contract(LEGACY_ETHEREUM_XOR_MASTER_CONTRACT_ADDRESS, net_id),
            Error::UnknownContractAddress
        );

        let apps =
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::list_apps();
        assert!(!apps.iter().any(|app| {
            matches!(
                app,
                BridgeAppInfo::EVM(_, info)
                    if info.app_kind == EVMAppKind::XorMaster
                        || info.evm_address == LEGACY_ETHEREUM_XOR_MASTER_CONTRACT_ADDRESS
            )
        }));

        assert!(
            !<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::is_asset_supported(
                generic_network_id,
                XOR.into(),
            )
        );

        RegisteredSidechainToken::<Runtime>::insert(
            net_id,
            VAL,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
        );
        assert!(
            !<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::is_asset_supported(
                generic_network_id,
                VAL.into(),
            )
        );
        let assets =
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::list_supported_assets(
                generic_network_id,
            );
        assert!(!assets.iter().any(|asset| {
            matches!(
                asset,
                BridgeAssetInfo::EVMLegacy(info)
                    if info.app_kind == EVMAppKind::XorMaster
                        || info.evm_address == Some(LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS)
            )
        }));
    });
}

#[test]
fn should_not_advertise_manually_deprecated_sidechain_token_mapping() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);
        let val_token = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();

        assert!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::is_asset_supported(
                generic_network_id,
                VAL.into(),
            )
        );

        DeprecatedSidechainTokens::<Runtime>::insert(net_id, val_token, true);

        assert!(
            !<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::is_asset_supported(
                generic_network_id,
                VAL.into(),
            )
        );
        let assets =
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::list_supported_assets(
                generic_network_id,
            );
        assert!(!assets.iter().any(|asset| {
            matches!(
                asset,
                BridgeAssetInfo::EVMLegacy(info) if info.asset_id == VAL.into()
            )
        }));
    });
}

#[test]
fn should_reject_bridge_app_transfer_when_asset_points_to_deprecated_sidechain_token() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        RegisteredSidechainToken::<Runtime>::insert(net_id, VAL, LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS);
        Assets::mint_to(&VAL.into(), &alice, &alice, 1000u32.into()).unwrap();

        assert_err!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::transfer(
                generic_network_id,
                VAL.into(),
                alice.clone(),
                EthAddress::from([77; 20]),
                100u32.into(),
            ),
            Error::DeprecatedLegacyXor
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).is_empty());
        assert_eq!(
            Assets::total_balance(&VAL.into(), &alice).unwrap(),
            1000u32.into()
        );
    });
}

#[test]
fn should_decommission_legacy_ethereum_xor() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bridge_account = BridgeAccount::<Runtime>::get(net_id).unwrap();
        assert_eq!(
            Assets::total_balance(&XOR.into(), &bridge_account).unwrap(),
            balance!(350000)
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert!(DeprecatedSidechainTokens::<Runtime>::get(
            net_id,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
        ));
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_none());
        assert!(RegisteredSidechainToken::<Runtime>::get(net_id, XOR).is_none());
        assert_eq!(
            Assets::total_balance(&XOR.into(), &bridge_account).unwrap(),
            0
        );
    });
}

#[test]
fn should_record_decommission_block_when_legacy_flag_already_set() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let queue_before = crate::RequestsQueue::<Runtime>::get(net_id);

        LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        LegacyEthereumXorDecommissionedAt::<Runtime>::kill();
        frame_system::Pallet::<Runtime>::set_block_number(42);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert_eq!(
            LegacyEthereumXorDecommissionedAt::<Runtime>::get(),
            Some(42)
        );
        assert_eq!(crate::RequestsQueue::<Runtime>::get(net_id), queue_before);
    });
}

#[test]
fn should_scrub_legacy_ethereum_xor_blockers_when_legacy_flag_already_set() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice,
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        let queue_before = crate::RequestsQueue::<Runtime>::get(net_id);

        LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        LegacyEthereumXorDecommissionedAt::<Runtime>::put(7);
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestSubmissionHeight::<Runtime>::insert(net_id, request_hash, 1);
        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, request_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);
        crate::RequestApprovers::<Runtime>::insert(net_id, request_hash, approvers);

        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            1
        );

        frame_system::Pallet::<Runtime>::set_block_number(42);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert_eq!(LegacyEthereumXorDecommissionedAt::<Runtime>::get(), Some(7));
        assert_eq!(crate::RequestsQueue::<Runtime>::get(net_id), queue_before);
        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(crate::RequestApprovals::<Runtime>::get(net_id, request_hash).is_empty());
        assert!(crate::RequestApprovers::<Runtime>::get(net_id, request_hash).is_empty());
    });
}

#[test]
fn should_decommission_pending_legacy_ethereum_xor_transfer_and_refund_sender() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bridge_account = BridgeAccount::<Runtime>::get(net_id).unwrap();
        Assets::mint_to(&XOR.into(), &alice, &alice, 1000u32.into()).unwrap();

        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from([7; 20]),
            100u32.into(),
            net_id,
        ));
        let request_hash = *crate::RequestsQueue::<Runtime>::get(net_id)
            .last()
            .expect("pending XOR request should be queued");
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            900u32.into()
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &bridge_account).unwrap(),
            balance!(350000) + Balance::from(100u32)
        );
        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            1000u32.into()
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &bridge_account).unwrap(),
            0
        );
    });
}

#[test]
fn should_quarantine_unsafe_legacy_ethereum_xor_outgoing_transfers() {
    let cases = vec![
        ("approvals-ready", Some(RequestStatus::ApprovalsReady)),
        ("frozen", Some(RequestStatus::Frozen)),
        (
            "broken",
            Some(RequestStatus::Broken(
                Error::InvalidContractInput.into(),
                Error::InvalidFunctionInput.into(),
            )),
        ),
        ("statusless", None),
    ];

    for (label, status) in cases {
        let (mut ext, _state) = ExtBuilder::default().build();

        ext.execute_with(|| {
            let net_id = ETH_NETWORK_ID;
            let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
            let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
            let bridge_account = BridgeAccount::<Runtime>::get(net_id).unwrap();
            Assets::mint_to(&XOR.into(), &alice, &alice, 1000u32.into()).unwrap();

            assert_ok!(EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                XOR.into(),
                EthAddress::from([7; 20]),
                100u32.into(),
                net_id,
            ));
            let request_hash = *crate::RequestsQueue::<Runtime>::get(net_id)
                .last()
                .unwrap_or_else(|| panic!("{label}: pending XOR request should be queued"));
            if let Some(status) = status.clone() {
                crate::RequestStatuses::<Runtime>::insert(net_id, request_hash, status);
            } else {
                crate::RequestStatuses::<Runtime>::remove(net_id, request_hash);
            }
            let mut approvals = BTreeSet::new();
            approvals.insert(SignatureParams {
                r: [1; 32],
                s: [2; 32],
                v: 27,
            });
            crate::RequestApprovals::<Runtime>::insert(net_id, request_hash, approvals);
            let mut approvers = BTreeSet::new();
            approvers.insert(bob);
            crate::RequestApprovers::<Runtime>::insert(net_id, request_hash, approvers);

            assert_eq!(
                crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
                1,
                "{label}"
            );

            crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

            assert!(LegacyEthereumXorDecommissioned::<Runtime>::get(), "{label}");
            assert!(
                DeprecatedSidechainTokens::<Runtime>::get(
                    net_id,
                    LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                ),
                "{label}"
            );
            assert!(
                RegisteredAsset::<Runtime>::get(net_id, XOR).is_none(),
                "{label}"
            );
            assert!(
                RegisteredSidechainToken::<Runtime>::get(net_id, XOR).is_none(),
                "{label}"
            );
            assert!(
                matches!(
                    crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
                    Some(RequestStatus::Failed(_))
                ),
                "{label}"
            );
            assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
            assert!(crate::RequestApprovals::<Runtime>::get(net_id, request_hash).is_empty());
            assert!(crate::RequestApprovers::<Runtime>::get(net_id, request_hash).is_empty());
            assert_eq!(
                Assets::total_balance(&XOR.into(), &alice).unwrap(),
                900u32.into()
            );
            assert_eq!(
                Assets::total_balance(&XOR.into(), &bridge_account).unwrap(),
                0
            );
        });
    }
}

#[test]
fn should_roll_back_legacy_ethereum_xor_decommission_when_reserve_burn_fails() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bridge_account = BridgeAccount::<Runtime>::get(net_id).unwrap();
        let bridge_before = Assets::total_balance(&XOR.into(), &bridge_account).unwrap();
        let registered_token_before = RegisteredSidechainToken::<Runtime>::get(net_id, XOR);
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_some());

        frame_system::Account::<Runtime>::mutate(&bridge_account, |account| {
            account.data.frozen = 1;
        });

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(!LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert!(!DeprecatedSidechainTokens::<Runtime>::get(
            net_id,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
        ));
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_some());
        assert_eq!(
            RegisteredSidechainToken::<Runtime>::get(net_id, XOR),
            registered_token_before,
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &bridge_account).unwrap(),
            bridge_before
        );
    });
}

#[test]
fn should_quarantine_non_queued_unsafe_legacy_ethereum_xor_outgoing_transfer() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice,
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestSubmissionHeight::<Runtime>::insert(net_id, request_hash, 1);
        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, request_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);
        crate::RequestApprovers::<Runtime>::insert(net_id, request_hash, approvers);

        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            1
        );

        frame_system::Pallet::<Runtime>::set_block_number(2);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert!(DeprecatedSidechainTokens::<Runtime>::get(
            net_id,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(crate::RequestApprovals::<Runtime>::get(net_id, request_hash).is_empty());
        assert!(crate::RequestApprovers::<Runtime>::get(net_id, request_hash).is_empty());
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_none());
        assert!(RegisteredSidechainToken::<Runtime>::get(net_id, XOR).is_none());
    });
}

#[test]
fn should_quarantine_pending_legacy_ethereum_xor_transfer_even_if_cancel_fails() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice,
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(net_id, request_hash, RequestStatus::Pending);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(request_hash));

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&request_hash));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(Error::DeprecatedLegacyXor.into()))
        );
        assert!(crate::RequestApprovals::<Runtime>::get(net_id, request_hash).is_empty());
        assert!(crate::RequestApprovers::<Runtime>::get(net_id, request_hash).is_empty());
    });
}

#[test]
fn should_treat_xor_outgoing_as_legacy_when_decommission_block_is_missing() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");

        LegacyEthereumXorDecommissioned::<Runtime>::put(true);
        LegacyEthereumXorDecommissionedAt::<Runtime>::kill();
        RegisteredAsset::<Runtime>::insert(net_id, XOR, AssetKind::Thischain);

        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, request_hash, approvals);

        assert!(
            EthBridge::is_decommissioned_legacy_ethereum_xor_outgoing_transfer_request(
                net_id,
                &request_hash
            )
        );
        assert!(
            EthBridge::get_approved_requests(&[request_hash], Some(net_id))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            EthBridge::get_approvals(&[request_hash], Some(net_id)).unwrap(),
            vec![Vec::<SignatureParams>::new()]
        );
    });
}

#[test]
fn should_reject_mark_as_done_for_legacy_xor_outgoing_after_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bridge_account = BridgeAccount::<Runtime>::get(net_id).unwrap();
        frame_system::Pallet::<Runtime>::set_block_number(2);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        frame_system::Pallet::<Runtime>::set_block_number(1);
        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestSubmissionHeight::<Runtime>::insert(net_id, request_hash, 1);

        assert_noop!(
            EthBridge::request_from_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                request_hash,
                IncomingRequestKind::Meta(IncomingMetaRequestKind::MarkAsDone),
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );

        let incoming = IncomingRequest::MarkAsDone(IncomingMarkAsDoneRequest::<Runtime> {
            outgoing_request_hash: request_hash,
            initial_request_hash: H256::from([9; 32]),
            author: alice,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        let incoming_hash = OffchainRequest::incoming(incoming.clone()).hash();
        assert_ok!(EthBridge::register_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            incoming,
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, incoming_hash),
            Some(RequestStatus::Failed(Error::DeprecatedLegacyXor.into()))
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::ApprovalsReady)
        );
    });
}

#[test]
fn should_reject_cancel_for_legacy_xor_outgoing_after_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        frame_system::Pallet::<Runtime>::set_block_number(2);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        let outgoing_request = OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from([7; 20]),
            asset_id: XOR.into(),
            amount: 100u32.into(),
            nonce: 42,
            network_id: net_id,
            timepoint: Default::default(),
        });
        let request = OffchainRequest::outgoing(outgoing_request.clone());
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestSubmissionHeight::<Runtime>::insert(net_id, request_hash, 1);

        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, request_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(alice.clone());
        crate::RequestApprovers::<Runtime>::insert(net_id, request_hash, approvers);

        let incoming_cancel = IncomingCancelOutgoingRequest::<Runtime> {
            outgoing_request,
            outgoing_request_hash: request_hash,
            initial_request_hash: H256::from([9; 32]),
            tx_input: Vec::new(),
            author: alice,
            tx_hash: H256::from([8; 32]),
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        };

        assert_noop!(incoming_cancel.prepare(), Error::DeprecatedLegacyXor);
        assert_noop!(incoming_cancel.finalize(), Error::DeprecatedLegacyXor);
        assert_ok!(incoming_cancel.cancel());
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(crate::RequestApprovals::<Runtime>::get(net_id, request_hash).is_empty());
        assert!(crate::RequestApprovers::<Runtime>::get(net_id, request_hash).is_empty());
    });
}

#[test]
fn should_allow_mark_as_done_for_new_xor_outgoing_after_thischain_reregistration() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let old_request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let old_hash = old_request.hash();

        frame_system::Pallet::<Runtime>::set_block_number(2);
        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        crate::Requests::<Runtime>::insert(net_id, old_hash, old_request);
        crate::RequestStatuses::<Runtime>::insert(net_id, old_hash, RequestStatus::ApprovalsReady);
        crate::RequestSubmissionHeight::<Runtime>::insert(net_id, old_hash, 1);

        assert!(
            EthBridge::is_decommissioned_legacy_ethereum_xor_outgoing_transfer_request(
                net_id, &old_hash
            )
        );

        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &XOR.into()
        ));
        assert!(
            EthBridge::get_approved_requests(&[old_hash], Some(net_id))
                .unwrap()
                .is_empty(),
            "pre-decommission XOR outgoing requests must not be re-encoded after XOR reregistration"
        );

        frame_system::Pallet::<Runtime>::set_block_number(3);
        let new_request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([8; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 43,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let new_hash = new_request.hash();
        crate::Requests::<Runtime>::insert(net_id, new_hash, new_request);
        crate::RequestStatuses::<Runtime>::insert(net_id, new_hash, RequestStatus::ApprovalsReady);
        crate::RequestSubmissionHeight::<Runtime>::insert(net_id, new_hash, 3);

        assert!(
            !EthBridge::is_decommissioned_legacy_ethereum_xor_outgoing_transfer_request(
                net_id, &new_hash
            )
        );
        assert_eq!(
            EthBridge::get_approved_requests(&[new_hash], Some(net_id))
                .unwrap()
                .len(),
            1
        );

        let incoming = IncomingRequest::MarkAsDone(IncomingMarkAsDoneRequest::<Runtime> {
            outgoing_request_hash: new_hash,
            initial_request_hash: H256::from([9; 32]),
            author: alice,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_ok!(incoming.validate());
        assert_ok!(incoming.finalize());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, new_hash),
            Some(RequestStatus::Done)
        );
    });
}

#[test]
fn should_leave_non_queued_finished_legacy_ethereum_xor_history_untouched() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice,
                to: EthAddress::from([7; 20]),
                asset_id: XOR.into(),
                amount: 100u32.into(),
                nonce: 42,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(net_id, request_hash, RequestStatus::Done);
        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, request_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);
        crate::RequestApprovers::<Runtime>::insert(net_id, request_hash, approvers);

        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Done)
        );
        assert!(!crate::RequestApprovals::<Runtime>::get(net_id, request_hash).is_empty());
        assert!(!crate::RequestApprovers::<Runtime>::get(net_id, request_hash).is_empty());
    });
}

#[test]
fn should_allow_xor_transfer_after_thischain_reregistration() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 1000u32.into()).unwrap();

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert_err!(
            EthBridge::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                XOR,
                EthAddress::from([6; 20]),
                net_id,
            ),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::transfer(
                generic_network_id,
                XOR.into(),
                alice.clone(),
                EthAddress::from([7; 20]),
                100u32.into(),
            ),
            Error::DeprecatedLegacyXor
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).is_empty());
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            1000u32.into()
        );

        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            RegisteredAsset::<Runtime>::get(net_id, XOR),
            Some(AssetKind::Thischain)
        );
        assert!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::is_asset_supported(
                generic_network_id,
                XOR.into(),
            )
        );
        let assets =
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::list_supported_assets(
                generic_network_id,
            );
        assert!(assets.iter().any(|asset| {
            matches!(
                asset,
                BridgeAssetInfo::EVMLegacy(info)
                    if info.asset_id == XOR.into()
                        && info.app_kind == EVMAppKind::HashiBridge
                        && info.evm_address.is_none()
                        && info.precision.is_none()
            )
        }));

        let outgoing_transfer = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from([7; 20]),
            asset_id: XOR.into(),
            amount: 100u32.into(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        };
        let encoded = outgoing_transfer.to_eth_abi(H256::repeat_byte(8)).unwrap();
        assert_eq!(encoded.amount, 100u32.into());

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[16u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let raw_xor_asset_id = H256(
            AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code,
        );
        let incoming_transfer = IncomingRequest::<Runtime>::try_from_contract_event(
            ContractEvent::Deposit(DepositEvent::new(
                alice.clone(),
                100u32.into(),
                EthAddress::zero(),
                raw_xor_asset_id,
            )),
            LoadIncomingTransactionRequest::new(
                alice.clone(),
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        )
        .unwrap();
        match incoming_transfer {
            IncomingRequest::Transfer(req) => {
                assert_eq!(req.asset_id, XOR.into());
                assert_eq!(req.asset_kind, AssetKind::Thischain);
                assert_eq!(req.amount, 100u32.into());
            }
            other => panic!("unexpected incoming request: {:?}", other),
        }

        assert_ok!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::transfer(
                generic_network_id,
                XOR.into(),
                alice.clone(),
                EthAddress::from([7; 20]),
                100u32.into(),
            )
        );
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            900u32.into()
        );
    });
}

#[test]
fn should_reject_legacy_xor_token_deposit_after_thischain_reregistration() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let raw_xor_asset_id =
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &XOR.into()
        ));

        for (tx_hash, raw_asset_id) in [
            (H256::repeat_byte(21), H256::zero()),
            (H256::repeat_byte(22), raw_xor_asset_id),
        ] {
            assert_err!(
                IncomingRequest::<Runtime>::try_from_contract_event(
                    ContractEvent::Deposit(DepositEvent::new(
                        alice.clone(),
                        100u32.into(),
                        LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                        raw_asset_id,
                    )),
                    LoadIncomingTransactionRequest::new(
                        alice.clone(),
                        tx_hash,
                        Default::default(),
                        IncomingTransactionRequestKind::Transfer,
                        net_id,
                    ),
                    1,
                ),
                Error::DeprecatedLegacyXor
            );
        }
    });
}

#[test]
fn should_keep_new_xor_thischain_transfers_one_to_one_below_old_denominator() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let raw_xor_asset_id =
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &XOR.into()
        ));
        assert_eq!(
            EthBridge::bridge_denomination_factor(net_id, &XOR.into()),
            1
        );

        let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
        Assets::mint_to(&XOR.into(), &bridge_acc_id, &bridge_acc_id, 1u32.into()).unwrap();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::repeat_byte(23),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::<Runtime>::try_from_contract_event(
            ContractEvent::Deposit(DepositEvent::new(
                alice.clone(),
                1u32.into(),
                EthAddress::zero(),
                raw_xor_asset_id,
            )),
            LoadIncomingTransactionRequest::new(
                alice.clone(),
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        )
        .unwrap();
        match &incoming_transfer {
            IncomingRequest::Transfer(req) => {
                assert_eq!(req.asset_id, XOR.into());
                assert_eq!(req.asset_kind, AssetKind::Thischain);
                assert_eq!(req.amount, 1u32.into());
            }
            other => panic!("unexpected incoming request: {:?}", other),
        }
        assert_incoming_request_done(&state, incoming_transfer).unwrap();
        assert_eq!(Assets::total_balance(&XOR.into(), &alice).unwrap(), 1);
        assert_eq!(
            Assets::total_balance(&XOR.into(), &bridge_acc_id).unwrap(),
            0
        );

        let outgoing_transfer = OutgoingTransfer::<Runtime> {
            from: alice,
            to: EthAddress::from([7; 20]),
            asset_id: XOR.into(),
            amount: 1u32.into(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        };
        let encoded = outgoing_transfer.to_eth_abi(H256::repeat_byte(24)).unwrap();
        assert_eq!(encoded.amount, 1u32.into());
    });
}

#[test]
fn should_reject_xor_deposit_log_from_val_master_after_thischain_reregistration() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let raw_xor_asset_id =
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &XOR.into()
        ));

        let xor_log =
            val_master_deposit_log(alice.clone(), 100, EthAddress::zero(), raw_xor_asset_id);
        assert_err!(
            EthBridge::parse_main_event(
                net_id,
                &[xor_log],
                IncomingTransactionRequestKind::Transfer
            ),
            Error::UnsupportedAssetId
        );

        let val_token = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();
        let raw_val_asset_id =
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::VAL).code);
        let raw_val_log =
            val_master_deposit_log(alice.clone(), 100, EthAddress::zero(), raw_val_asset_id);
        assert_err!(
            EthBridge::parse_main_event(
                net_id,
                &[raw_val_log],
                IncomingTransactionRequestKind::Transfer
            ),
            Error::UnsupportedAssetId
        );

        let val_log = val_master_deposit_log(alice, 100, val_token, H256::zero());
        match EthBridge::parse_main_event(
            net_id,
            &[val_log],
            IncomingTransactionRequestKind::Transfer,
        )
        .unwrap()
        {
            ContractEvent::Deposit(event) => {
                assert_eq!(event.token, val_token);
                assert_eq!(event.sidechain_asset, H256::zero());
            }
            other => panic!("unexpected event: {:?}", other),
        }
    });
}

#[test]
fn should_apply_val_master_guard_to_log_scanner_path_after_thischain_reregistration() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let raw_xor_asset_id =
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            XOR.into(),
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &XOR.into()
        ));

        let xor_log =
            val_master_deposit_log(alice.clone(), 100, EthAddress::zero(), raw_xor_asset_id);
        assert_err!(
            EthBridge::parse_deposit_event_from_known_contract(net_id, &xor_log),
            Error::UnsupportedAssetId
        );

        let val_token = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();
        let val_log = val_master_deposit_log(alice, 100, val_token, H256::zero());
        let event = EthBridge::parse_deposit_event_from_known_contract(net_id, &val_log).unwrap();
        assert_eq!(event.token, val_token);
        assert_eq!(event.sidechain_asset, H256::zero());
    });
}

#[test]
fn should_reject_non_val_registered_token_from_val_master() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = EthAddress::from([55; 20]);
        let sidechain_asset_id = EthBridge::register_sidechain_asset(
            token_address,
            DEFAULT_BALANCE_PRECISION,
            AssetSymbol(b"BAD".to_vec()),
            AssetName(b"Bad Token".to_vec()),
            net_id,
        )
        .unwrap();

        let val_master_log =
            val_master_deposit_log(alice.clone(), 100, token_address, H256::zero());
        assert_err!(
            EthBridge::parse_deposit_event_from_known_contract(net_id, &val_master_log),
            Error::UnsupportedAssetId
        );
        assert_err!(
            EthBridge::parse_main_event(
                net_id,
                &[val_master_log],
                IncomingTransactionRequestKind::Transfer
            ),
            Error::UnsupportedAssetId
        );

        let bridge_log = deposit_log_from(
            EthBridge::bridge_contract_address(net_id),
            alice.clone(),
            100,
            token_address,
            H256::zero(),
        );
        let event =
            EthBridge::parse_deposit_event_from_known_contract(net_id, &bridge_log).unwrap();
        let incoming_request = IncomingRequest::<Runtime>::try_from_contract_event(
            ContractEvent::Deposit(event),
            LoadIncomingTransactionRequest::new(
                alice,
                H256::from([19; 32]),
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        )
        .unwrap();

        match incoming_request {
            IncomingRequest::Transfer(request) => {
                assert_eq!(request.asset_id, sidechain_asset_id);
                assert_eq!(request.asset_kind, AssetKind::Sidechain);
                assert_eq!(request.amount, 100u32.into());
            }
            other => panic!("unexpected incoming request: {:?}", other),
        }
    });
}

#[test]
fn should_reject_deprecated_val_token_from_val_master() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let val_token = RegisteredSidechainToken::<Runtime>::get(net_id, VAL).unwrap();

        DeprecatedSidechainTokens::<Runtime>::insert(net_id, val_token, true);

        let val_log = val_master_deposit_log(alice, 100, val_token, H256::zero());
        assert_err!(
            EthBridge::parse_deposit_event_from_known_contract(net_id, &val_log),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::parse_main_event(
                net_id,
                &[val_log],
                IncomingTransactionRequestKind::Transfer
            ),
            Error::DeprecatedLegacyXor
        );
    });
}

#[test]
fn should_ignore_non_deposit_log_from_val_master() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let change_peers_log = Log {
            address: types::H160(EthBridge::val_master_contract_address().0),
            topics: vec![types::H256(hex!(
                "a9fac23eb012e72fbd1f453498e7069c380385436763ee2c1c057b170d88d9f9"
            ))],
            data: types::Bytes(ethabi::encode(&[
                ethabi::Token::Address(types::EthAddress::from([9; 20])),
                ethabi::Token::Bool(false),
            ])),
            removed: Some(false),
            ..Default::default()
        };

        assert_err!(
            EthBridge::parse_main_event(
                net_id,
                &[change_peers_log],
                IncomingTransactionRequestKind::AddPeer
            ),
            Error::UnknownEvent
        );
    });
}

#[test]
fn should_block_xor_thischain_registration_with_stale_sidechain_mapping_after_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let xor_asset_id: AssetId = XOR.into();

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        RegisteredAsset::<Runtime>::insert(net_id, &xor_asset_id, AssetKind::Thischain);
        RegisteredSidechainToken::<Runtime>::insert(net_id, &xor_asset_id, EthAddress::from([9; 20]));
        Assets::mint_to(&xor_asset_id, &alice, &alice, 1000u32.into()).unwrap();

        assert!(
            !EthBridge::is_ethereum_xor_thischain_registration(net_id, &xor_asset_id),
            "stale sidechain token mapping must prevent clean XOR classification"
        );
        assert_eq!(EthBridge::bridge_denomination_factor(net_id, &xor_asset_id), 10);
        assert!(
            !<EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::is_asset_supported(
                generic_network_id,
                xor_asset_id,
            )
        );
        let assets =
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::list_supported_assets(
                generic_network_id,
            );
        assert!(!assets.iter().any(|asset| {
            matches!(asset, BridgeAssetInfo::EVMLegacy(info) if info.asset_id == XOR.into())
        }));

        assert_err!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::transfer(
                generic_network_id,
                XOR.into(),
                alice,
                EthAddress::from([10; 20]),
                100u32.into(),
            ),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::add_asset(RuntimeOrigin::root(), XOR.into(), net_id),
            Error::TokenIsAlreadyAdded
        );
    });
}

#[test]
fn should_reject_xor_add_with_stale_sidechain_token_after_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let authority = EthBridge::authority_account().unwrap();
        let xor_asset_id: AssetId = XOR.into();

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_none());
        RegisteredSidechainToken::<Runtime>::insert(
            net_id,
            &xor_asset_id,
            EthAddress::from([16; 20]),
        );

        assert_err!(
            EthBridge::add_asset(RuntimeOrigin::root(), XOR.into(), net_id),
            Error::DeprecatedLegacyXor
        );

        let add_xor_asset = OutgoingAddAsset::<Runtime> {
            author: authority,
            asset_id: XOR.into(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        };
        assert_err!(add_xor_asset.validate(), Error::DeprecatedLegacyXor);
        assert!(RegisteredAsset::<Runtime>::get(net_id, XOR).is_none());
    });
}

#[test]
fn should_reject_legacy_transfer_xor_load_after_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let load_xor = OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                alice,
                H256::from([17; 32]),
                Default::default(),
                IncomingTransactionRequestKind::TransferXOR,
                net_id,
            ),
        ));
        let load_xor_hash = load_xor.hash();

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_err!(
            EthBridge::add_request(&load_xor),
            Error::DeprecatedLegacyXor
        );

        crate::Requests::<Runtime>::insert(net_id, load_xor_hash, load_xor.clone());
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            load_xor_hash,
            RequestStatus::Failed(Error::DeprecatedLegacyXor.into()),
        );
        assert_err!(
            EthBridge::add_request(&load_xor),
            Error::DeprecatedLegacyXor
        );

        OutgoingAddAsset::<Runtime> {
            author: EthBridge::authority_account().unwrap(),
            asset_id: XOR.into(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        }
        .finalize()
        .unwrap();
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &XOR.into()
        ));
        assert_err!(
            EthBridge::add_request(&load_xor),
            Error::DeprecatedLegacyXor
        );
    });
}

#[test]
fn should_block_xor_registered_as_sidechain_after_decommission() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let generic_network_id = GenericNetworkId::EVMLegacy(net_id);
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let xor_asset_id: AssetId = XOR.into();

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        RegisteredAsset::<Runtime>::insert(net_id, &xor_asset_id, AssetKind::Sidechain);
        Assets::mint_to(&xor_asset_id, &alice, &alice, 1000u32.into()).unwrap();

        assert!(!EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &xor_asset_id
        ));
        assert_eq!(
            EthBridge::bridge_denomination_factor(net_id, &xor_asset_id),
            10
        );
        assert!(!<EthBridge as BridgeApp<
            AccountId,
            EthAddress,
            AssetId,
            Balance,
        >>::is_asset_supported(
            generic_network_id, xor_asset_id,
        ));
        assert_err!(
            <EthBridge as BridgeApp<AccountId, EthAddress, AssetId, Balance>>::transfer(
                generic_network_id,
                XOR.into(),
                alice,
                EthAddress::from([11; 20]),
                100u32.into(),
            ),
            Error::DeprecatedLegacyXor
        );
        assert_err!(
            EthBridge::add_asset(RuntimeOrigin::root(), XOR.into(), net_id),
            Error::TokenIsAlreadyAdded
        );
    });
}

#[test]
fn should_reject_xor_resolution_through_orphan_sidechain_asset_mapping() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let xor_asset_id: AssetId = XOR.into();
        let orphan_token = EthAddress::from([13; 20]);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            xor_asset_id,
            net_id
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert!(EthBridge::is_ethereum_xor_thischain_registration(
            net_id,
            &xor_asset_id
        ));

        RegisteredSidechainAsset::<Runtime>::insert(net_id, orphan_token, xor_asset_id);

        assert_err!(
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &orphan_token, net_id),
            Error::DeprecatedLegacyXor
        );

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[18u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        assert_err!(
            IncomingRequest::<Runtime>::try_from_contract_event(
                ContractEvent::Deposit(DepositEvent::new(
                    alice.clone(),
                    100u32.into(),
                    orphan_token,
                    H256::zero(),
                )),
                LoadIncomingTransactionRequest::new(
                    alice,
                    tx_hash,
                    Default::default(),
                    IncomingTransactionRequestKind::Transfer,
                    net_id,
                ),
                1,
            ),
            Error::DeprecatedLegacyXor
        );
    });
}

#[test]
fn should_keep_non_ethereum_xor_denominated() {
    let mut builder = ExtBuilder::default();
    let net_id = builder.add_network(
        vec![AssetConfig::Thischain { id: XOR.into() }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let xor_asset_id: AssetId = XOR.into();
        let raw_xor_asset_id =
            H256(AssetId32::<PredefinedAssetId>::from_asset_id(PredefinedAssetId::XOR).code);

        assert_eq!(
            EthBridge::bridge_denomination_factor(net_id, &xor_asset_id),
            10
        );

        let outgoing_transfer = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from([12; 20]),
            asset_id: XOR.into(),
            amount: 100u32.into(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        };
        let encoded = outgoing_transfer.to_eth_abi(H256::repeat_byte(12)).unwrap();
        assert_eq!(encoded.amount, 1000u32.into());

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[17u8; 32]),
            IncomingTransactionRequestKind::Transfer.into(),
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::<Runtime>::try_from_contract_event(
            ContractEvent::Deposit(DepositEvent::new(
                alice.clone(),
                1000u32.into(),
                EthAddress::zero(),
                raw_xor_asset_id,
            )),
            LoadIncomingTransactionRequest::new(
                alice,
                tx_hash,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net_id,
            ),
            1,
        )
        .unwrap();
        match incoming_transfer {
            IncomingRequest::Transfer(req) => {
                assert_eq!(req.asset_id, XOR.into());
                assert_eq!(req.asset_kind, AssetKind::Thischain);
                assert_eq!(req.amount, 100u32.into());
            }
            other => panic!("unexpected incoming request: {:?}", other),
        }
    });
}

#[test]
fn should_decommission_legacy_ethereum_xor_scrub_queued_requests_and_signatures() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let legacy_request =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice.clone(),
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                symbol: "OLD".into(),
                name: "Old".into(),
                decimals: DEFAULT_BALANCE_PRECISION,
                nonce: 1,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let legacy_hash = legacy_request.hash();
        let val_request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([8; 20]),
                asset_id: VAL.into(),
                amount: 100u32.into(),
                nonce: 2,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let val_hash = val_request.hash();

        crate::Requests::<Runtime>::insert(net_id, legacy_hash, legacy_request);
        crate::Requests::<Runtime>::insert(net_id, val_hash, val_request);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            legacy_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestStatuses::<Runtime>::insert(net_id, val_hash, RequestStatus::ApprovalsReady);
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| {
            queue.push(legacy_hash);
            queue.push(val_hash);
            queue.push(legacy_hash);
        });

        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [1; 32],
            s: [2; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, legacy_hash, approvals.clone());
        crate::RequestApprovals::<Runtime>::insert(net_id, val_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);
        crate::RequestApprovers::<Runtime>::insert(net_id, legacy_hash, approvers.clone());
        crate::RequestApprovers::<Runtime>::insert(net_id, val_hash, approvers);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, legacy_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&legacy_hash));
        assert!(crate::RequestApprovals::<Runtime>::get(net_id, legacy_hash).is_empty());
        assert!(crate::RequestApprovers::<Runtime>::get(net_id, legacy_hash).is_empty());

        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, val_hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&val_hash));
        assert!(!crate::RequestApprovals::<Runtime>::get(net_id, val_hash).is_empty());
        assert!(!crate::RequestApprovers::<Runtime>::get(net_id, val_hash).is_empty());
    });
}

#[test]
fn should_retain_missing_and_non_legacy_queue_entries_in_order_when_decommissioning_legacy_ethereum_xor(
) {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let missing_hash = H256::from([0x42; 32]);
        let legacy_request =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice.clone(),
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                symbol: "OLD".into(),
                name: "Old".into(),
                decimals: DEFAULT_BALANCE_PRECISION,
                nonce: 31,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let legacy_hash = legacy_request.hash();
        let val_request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice,
                to: EthAddress::from([9; 20]),
                asset_id: VAL.into(),
                amount: 100u32.into(),
                nonce: 32,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let val_hash = val_request.hash();

        crate::Requests::<Runtime>::insert(net_id, legacy_hash, legacy_request);
        crate::Requests::<Runtime>::insert(net_id, val_hash, val_request);
        crate::RequestStatuses::<Runtime>::insert(net_id, legacy_hash, RequestStatus::Frozen);
        crate::RequestStatuses::<Runtime>::insert(net_id, val_hash, RequestStatus::Frozen);
        crate::RequestsQueue::<Runtime>::insert(
            net_id,
            vec![
                missing_hash,
                legacy_hash,
                val_hash,
                legacy_hash,
                missing_hash,
            ],
        );

        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [7; 32],
            s: [8; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(net_id, legacy_hash, approvals.clone());
        crate::RequestApprovals::<Runtime>::insert(net_id, val_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);
        crate::RequestApprovers::<Runtime>::insert(net_id, legacy_hash, approvers.clone());
        crate::RequestApprovers::<Runtime>::insert(net_id, val_hash, approvers);

        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert_eq!(
            crate::RequestsQueue::<Runtime>::get(net_id),
            vec![missing_hash, val_hash, missing_hash]
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, legacy_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, val_hash),
            Some(RequestStatus::Frozen)
        );
        assert!(crate::RequestApprovals::<Runtime>::get(net_id, legacy_hash).is_empty());
        assert!(crate::RequestApprovers::<Runtime>::get(net_id, legacy_hash).is_empty());
        assert!(!crate::RequestApprovals::<Runtime>::get(net_id, val_hash).is_empty());
        assert!(!crate::RequestApprovers::<Runtime>::get(net_id, val_hash).is_empty());
    });
}

#[test]
fn should_count_unsafe_legacy_ethereum_xor_outgoing_transfers_as_decommission_blockers() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let insert_transfer = |nonce: u64,
                               status: Option<RequestStatus>,
                               queued: bool|
         -> sp_core::H256 {
            let request =
                OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                    from: alice.clone(),
                    to: EthAddress::from([nonce as u8; 20]),
                    asset_id: XOR.into(),
                    amount: 10u32.into(),
                    nonce,
                    network_id: net_id,
                    timepoint: Default::default(),
                }));
            let hash = request.hash();
            crate::Requests::<Runtime>::insert(net_id, hash, request);
            if let Some(status) = status {
                crate::RequestStatuses::<Runtime>::insert(net_id, hash, status);
            }
            if queued {
                crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));
            }
            hash
        };

        let non_queued_done = insert_transfer(41, Some(RequestStatus::Done), false);
        let queued_pending = insert_transfer(42, Some(RequestStatus::Pending), true);
        let queued_done = insert_transfer(43, Some(RequestStatus::Done), true);
        let queued_statusless_add_token =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice,
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                symbol: "OLD".into(),
                name: "Old".into(),
                decimals: DEFAULT_BALANCE_PRECISION,
                nonce: 44,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let queued_statusless_add_token_hash = queued_statusless_add_token.hash();
        crate::Requests::<Runtime>::insert(
            net_id,
            queued_statusless_add_token_hash,
            queued_statusless_add_token,
        );
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| {
            queue.push(queued_statusless_add_token_hash);
            queue.push(H256::from([0x99; 32]));
        });

        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, non_queued_done),
            Some(RequestStatus::Done)
        );
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&queued_pending));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&queued_done));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id)
            .contains(&queued_statusless_add_token_hash));
    });

    let (mut ext, _state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let blocker_request =
            OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
                from: alice.clone(),
                to: EthAddress::from([6; 20]),
                asset_id: XOR.into(),
                amount: 10u32.into(),
                nonce: 45,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let blocker_hash = blocker_request.hash();
        let legacy_add_token =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice,
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                symbol: "OLD".into(),
                name: "Old".into(),
                decimals: DEFAULT_BALANCE_PRECISION,
                nonce: 46,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let legacy_add_token_hash = legacy_add_token.hash();
        crate::Requests::<Runtime>::insert(net_id, blocker_hash, blocker_request);
        crate::Requests::<Runtime>::insert(net_id, legacy_add_token_hash, legacy_add_token);
        crate::RequestStatuses::<Runtime>::insert(net_id, blocker_hash, RequestStatus::Frozen);
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            legacy_add_token_hash,
            RequestStatus::Frozen,
        );
        crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| {
            queue.push(legacy_add_token_hash);
            queue.push(blocker_hash);
        });
        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            1
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert!(LegacyEthereumXorDecommissioned::<Runtime>::get());
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&blocker_hash));
        assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&legacy_add_token_hash));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, blocker_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, legacy_add_token_hash),
            Some(RequestStatus::Failed(_))
        ));
    });
}

#[test]
fn should_decommission_legacy_ethereum_xor_request_variants_without_treating_them_as_blockers() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let outgoing_add_asset =
            OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset::<Runtime> {
                author: alice.clone(),
                asset_id: XOR.into(),
                nonce: 51,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let outgoing_add_token =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice.clone(),
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                symbol: "OLD".into(),
                name: "Old".into(),
                decimals: DEFAULT_BALANCE_PRECISION,
                nonce: 52,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let incoming_transfer =
            OffchainRequest::incoming(IncomingRequest::Transfer(IncomingTransfer::<Runtime> {
                from: EthAddress::from([5; 20]),
                to: bob.clone(),
                asset_id: XOR.into(),
                asset_kind: AssetKind::Thischain,
                amount: 25u32.into(),
                author: alice.clone(),
                tx_hash: H256::from([0x51; 32]),
                at_height: 100,
                timepoint: Default::default(),
                network_id: net_id,
                should_take_fee: false,
            }));
        let incoming_add_token =
            OffchainRequest::incoming(IncomingRequest::AddToken(IncomingAddToken::<Runtime> {
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                asset_id: XOR.into(),
                precision: DEFAULT_BALANCE_PRECISION,
                symbol: AssetSymbol(b"OLD".to_vec()),
                name: AssetName(b"Old".to_vec()),
                author: alice,
                tx_hash: H256::from([0x52; 32]),
                at_height: 101,
                timepoint: Default::default(),
                network_id: net_id,
            }));
        let load_incoming_transfer_xor = OffchainRequest::LoadIncoming(
            LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
                bob.clone(),
                H256::from([0x53; 32]),
                Default::default(),
                IncomingTransactionRequestKind::TransferXOR,
                net_id,
            )),
        );

        let add_asset_hash = outgoing_add_asset.hash();
        let add_token_hash = outgoing_add_token.hash();
        let incoming_transfer_hash = incoming_transfer.hash();
        let incoming_add_token_hash = incoming_add_token.hash();
        let load_incoming_transfer_xor_hash = load_incoming_transfer_xor.hash();
        for (hash, request) in [
            (add_asset_hash, outgoing_add_asset),
            (add_token_hash, outgoing_add_token),
            (incoming_transfer_hash, incoming_transfer),
            (incoming_add_token_hash, incoming_add_token),
            (load_incoming_transfer_xor_hash, load_incoming_transfer_xor),
        ] {
            crate::Requests::<Runtime>::insert(net_id, hash, request);
            crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));
        }
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            add_asset_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            incoming_transfer_hash,
            RequestStatus::Frozen,
        );
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            incoming_add_token_hash,
            RequestStatus::Done,
        );
        crate::RequestStatuses::<Runtime>::insert(
            net_id,
            load_incoming_transfer_xor_hash,
            RequestStatus::Pending,
        );

        assert_eq!(
            crate::migration::legacy_ethereum_xor_decommission_blockers::<Runtime>(),
            0
        );

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        for hash in [
            add_asset_hash,
            add_token_hash,
            incoming_transfer_hash,
            incoming_add_token_hash,
            load_incoming_transfer_xor_hash,
        ] {
            assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&hash));
        }
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, add_asset_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, add_token_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, incoming_transfer_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, incoming_add_token_hash),
            Some(RequestStatus::Done)
        );
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, load_incoming_transfer_xor_hash),
            Some(RequestStatus::Failed(_))
        ));
    });
}

#[test]
fn should_decommission_legacy_ethereum_xor_requests_across_adversarial_statuses() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let failed_error: sp_runtime::DispatchError = Error::UnsupportedToken.into();
        let broken_first_error: sp_runtime::DispatchError = Error::InvalidContractInput.into();
        let broken_second_error: sp_runtime::DispatchError = Error::InvalidFunctionInput.into();

        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [3; 32],
            s: [4; 32],
            v: 28,
        });
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);

        let insert_legacy_request = |nonce: u64, status: Option<RequestStatus>| -> sp_core::H256 {
            let request =
                OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                    author: alice.clone(),
                    token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                    symbol: format!("OLD{}", nonce),
                    name: format!("Old {}", nonce),
                    decimals: DEFAULT_BALANCE_PRECISION,
                    nonce,
                    network_id: net_id,
                    timepoint: Default::default(),
                }));
            let hash = request.hash();
            crate::Requests::<Runtime>::insert(net_id, hash, request);
            if let Some(status) = status {
                crate::RequestStatuses::<Runtime>::insert(net_id, hash, status);
            }
            crate::RequestsQueue::<Runtime>::mutate(net_id, |queue| queue.push(hash));
            crate::RequestApprovals::<Runtime>::insert(net_id, hash, approvals.clone());
            crate::RequestApprovers::<Runtime>::insert(net_id, hash, approvers.clone());
            hash
        };

        let pending_hash = insert_legacy_request(11, Some(RequestStatus::Pending));
        let frozen_hash = insert_legacy_request(12, Some(RequestStatus::Frozen));
        let approvals_ready_hash = insert_legacy_request(13, Some(RequestStatus::ApprovalsReady));
        let failed_hash = insert_legacy_request(14, Some(RequestStatus::Failed(failed_error)));
        let done_hash = insert_legacy_request(15, Some(RequestStatus::Done));
        let broken_hash = insert_legacy_request(
            16,
            Some(RequestStatus::Broken(
                broken_first_error,
                broken_second_error,
            )),
        );
        let statusless_hash = insert_legacy_request(17, None);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        for hash in [
            pending_hash,
            frozen_hash,
            approvals_ready_hash,
            failed_hash,
            done_hash,
            broken_hash,
            statusless_hash,
        ] {
            assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&hash));
            assert!(crate::RequestApprovals::<Runtime>::get(net_id, hash).is_empty());
            assert!(crate::RequestApprovers::<Runtime>::get(net_id, hash).is_empty());
        }

        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, pending_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, frozen_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, approvals_ready_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, broken_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, statusless_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert!(matches!(
            crate::RequestStatuses::<Runtime>::get(net_id, failed_hash),
            Some(RequestStatus::Failed(_))
        ));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net_id, done_hash),
            Some(RequestStatus::Done)
        );
    });
}

#[test]
fn should_not_decommission_non_ethereum_legacy_xor_like_state() {
    let mut builder = ExtBuilder::default();
    let non_eth_net_id = builder.add_network(vec![], None, None, Default::default());
    let (mut ext, _state) = builder.build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        assert_ok!(EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            XOR,
            LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
            non_eth_net_id,
        ));

        let request =
            OffchainRequest::outgoing(OutgoingRequest::AddToken(OutgoingAddToken::<Runtime> {
                author: alice,
                token_address: LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS,
                symbol: "NETH".into(),
                name: "Non Ethereum".into(),
                decimals: DEFAULT_BALANCE_PRECISION,
                nonce: Default::default(),
                network_id: non_eth_net_id,
                timepoint: Default::default(),
            }));
        let request_hash = request.hash();
        crate::Requests::<Runtime>::insert(non_eth_net_id, request_hash, request);
        crate::RequestStatuses::<Runtime>::insert(
            non_eth_net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        crate::RequestsQueue::<Runtime>::mutate(non_eth_net_id, |queue| queue.push(request_hash));
        let mut approvals = BTreeSet::new();
        approvals.insert(SignatureParams {
            r: [5; 32],
            s: [6; 32],
            v: 27,
        });
        crate::RequestApprovals::<Runtime>::insert(non_eth_net_id, request_hash, approvals);
        let mut approvers = BTreeSet::new();
        approvers.insert(bob);
        crate::RequestApprovers::<Runtime>::insert(non_eth_net_id, request_hash, approvers);

        crate::migration::decommission_legacy_ethereum_xor::<Runtime>();

        assert_eq!(
            RegisteredSidechainToken::<Runtime>::get(non_eth_net_id, XOR),
            Some(LEGACY_ETHEREUM_XOR_TOKEN_ADDRESS)
        );
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(non_eth_net_id, request_hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert!(crate::RequestsQueue::<Runtime>::get(non_eth_net_id).contains(&request_hash));
        assert!(!crate::RequestApprovals::<Runtime>::get(non_eth_net_id, request_hash).is_empty());
        assert!(!crate::RequestApprovers::<Runtime>::get(non_eth_net_id, request_hash).is_empty());
    });
}
