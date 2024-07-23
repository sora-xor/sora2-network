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
use super::Error;
use crate::requests::{OffchainRequest, OutgoingRequest, OutgoingTransfer};
use crate::tests::{
    approve_last_request, last_outgoing_request, last_request, Assets, ETH_NETWORK_ID,
};
use crate::{AssetConfig, EthAddress};
use common::{AssetInfoProvider, DEFAULT_BALANCE_PRECISION, KSM, PSWAP, USDT, XOR};
use frame_support::sp_runtime::app_crypto::sp_core::{self, sr25519};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use sp_core::H160;
use sp_std::prelude::*;
use std::str::FromStr;

#[test]
fn should_approve_outgoing_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 99999u32.into()).unwrap();
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            Assets::total_balance(&XOR.into(), &alice).unwrap(),
            99900u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
    });
}

#[test]
fn should_reserve_and_burn_sidechain_asset_in_outgoing_transfer() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Sidechain {
            id: USDT.into(),
            sidechain_id: H160(hex!("dAC17F958D2ee523a2206206994597C13D831ec7")),
            owned: false,
            precision: DEFAULT_BALANCE_PRECISION,
        }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&USDT.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            USDT.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::free_balance(&USDT.into(), &bridge_acc).unwrap(), 0);
        // Sidechain asset was reserved.
        assert_eq!(
            Assets::total_balance(&USDT.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        // Sidechain asset was burnt.
        assert_eq!(Assets::total_balance(&USDT.into(), &bridge_acc).unwrap(), 0);
        assert_eq!(
            Assets::free_balance(&USDT.into(), &bridge_acc).unwrap(),
            Assets::total_balance(&USDT.into(), &bridge_acc).unwrap()
        );
    });
}

#[test]
fn should_reserve_and_unreserve_thischain_asset_in_outgoing_transfer() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::new();
    builder.add_network(
        vec![AssetConfig::Thischain { id: PSWAP.into() }],
        None,
        None,
        Default::default(),
    );
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let bridge_acc = &state.networks[&net_id].config.bridge_account_id;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&PSWAP.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            PSWAP.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(Assets::free_balance(&PSWAP.into(), &bridge_acc).unwrap(), 0);
        // Thischain asset was reserved.
        assert_eq!(
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        // Thischain asset was unreserved.
        assert_eq!(
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            100u32.into()
        );
        assert_eq!(
            Assets::free_balance(&PSWAP.into(), &bridge_acc).unwrap(),
            Assets::total_balance(&PSWAP.into(), &bridge_acc).unwrap()
        );
    });
}

#[test]
fn should_not_transfer() {
    let (mut ext, _) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_err!(
            EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice.clone()),
                KSM.into(),
                EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::UnsupportedToken
        );
        assert!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_000_000_u32.into(),
            net_id,
        )
        .is_err());
    });
}

#[test]
fn should_register_outgoing_transfer() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100000u32.into()).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from([1; 20]),
            100u32.into(),
            net_id,
        ));
        let outgoing_transfer = OutgoingTransfer::<Runtime> {
            from: alice.clone(),
            to: EthAddress::from([1; 20]),
            asset_id: XOR.into(),
            amount: 100_u32.into(),
            nonce: 3,
            network_id: ETH_NETWORK_ID,
            timepoint: bridge_multisig::Pallet::<Runtime>::thischain_timepoint(),
        };
        let last_request = last_request(net_id).unwrap();
        match last_request {
            OffchainRequest::Outgoing(OutgoingRequest::Transfer(r), _) => {
                assert_eq!(r, outgoing_transfer)
            }
            _ => panic!("Invalid off-chain request"),
        }
    });
}

#[test]
fn ocw_should_handle_outgoing_request() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));
        state.run_next_offchain_and_dispatch_txs();
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
    });
}

#[test]
fn ocw_should_not_handle_outgoing_request_twice() {
    let (mut ext, mut state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        Assets::mint_to(&XOR.into(), &alice, &alice, 100).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(alice.clone()),
            XOR.into(),
            EthAddress::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100,
            net_id,
        ));
        state.run_next_offchain_and_dispatch_txs();
        let hash = last_outgoing_request(net_id).unwrap().1;
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
        state.run_next_offchain_and_dispatch_txs();
        assert_eq!(
            crate::RequestApprovals::<Runtime>::get(net_id, hash).len(),
            1
        );
    });
}
