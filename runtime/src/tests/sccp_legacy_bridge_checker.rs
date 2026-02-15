// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

use bridge_types::ton::{TonAddress, TonNetworkId};
use bridge_types::types::AssetKind;
use bridge_types::H160;
use common::{AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_support::assert_ok;
use framenode_chain_spec::ext;
use sccp::LegacyBridgeAssetChecker;
use sp_core::H256;

use crate::{
    Assets, Currencies, EthBridge, JettonApp, LegacyBridgeChecker, Runtime, RuntimeOrigin,
};

#[test]
fn legacy_bridge_checker_detects_eth_bridge_asset() {
    ext().execute_with(|| {
        let asset_id = common::XOR.into();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();

        assert!(EthBridge::registered_asset(evm_net_id, asset_id).is_some());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));
    });
}

#[test]
fn legacy_bridge_checker_detects_jetton_bridge_asset() {
    ext().execute_with(|| {
        let asset_id: crate::AssetId = H256::repeat_byte(0x42).into();

        if JettonApp::token_address(asset_id).is_none() {
            if JettonApp::app_info().is_none() {
                assert_ok!(JettonApp::register_network_with_existing_asset(
                    RuntimeOrigin::root(),
                    TonNetworkId::Mainnet,
                    TonAddress::new(0, H256::repeat_byte(0x11)),
                    asset_id,
                    9,
                ));
            } else {
                assert_ok!(JettonApp::register_asset_inner(
                    asset_id,
                    TonAddress::new(0, H256::repeat_byte(0x12)),
                    AssetKind::Sidechain,
                    9,
                ));
            }
        }

        assert!(JettonApp::token_address(asset_id).is_some());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));
    });
}

#[test]
fn legacy_bridge_checker_returns_false_for_untracked_asset() {
    ext().execute_with(|| {
        let asset_id: crate::AssetId = H256::repeat_byte(0x7f).into();
        assert!(!LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));
    });
}

#[test]
fn legacy_bridge_checker_detects_pending_eth_bridge_add_asset_request() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"LBPEND".to_vec()),
            AssetName(b"Legacy EVM Pending".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            evm_net_id,
        ));
        assert!(EthBridge::registered_asset(evm_net_id, asset_id).is_none());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));
    });
}

#[test]
fn legacy_bridge_checker_detects_pending_eth_bridge_add_asset_on_non_default_network() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"LBPNDN".to_vec()),
            AssetName(b"Legacy EVM Pending Other Net".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let new_net_id = EthBridge::next_network_id();
        assert_ok!(EthBridge::register_bridge(
            RuntimeOrigin::root(),
            H160::repeat_byte(0x79),
            vec![owner],
            eth_bridge::BridgeSignatureVersion::V3,
        ));
        assert_ok!(EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            new_net_id,
        ));
        assert!(EthBridge::registered_asset(new_net_id, asset_id).is_none());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));
    });
}

#[test]
fn legacy_bridge_checker_detects_asset_on_non_default_eth_bridge_network() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            common::XOR.into(),
            1i128,
        ));
        let asset_id = Assets::register_from(
            &owner,
            AssetSymbol(b"LBETHN".to_vec()),
            AssetName(b"Legacy EVM Other Net".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let new_net_id = EthBridge::next_network_id();
        assert_ok!(EthBridge::register_bridge(
            RuntimeOrigin::root(),
            H160::repeat_byte(0x77),
            vec![owner],
            eth_bridge::BridgeSignatureVersion::V3,
        ));
        assert_ok!(EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            asset_id,
            H160::repeat_byte(0x78),
            new_net_id,
        ));
        assert!(EthBridge::registered_asset(new_net_id, asset_id).is_some());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));
    });
}
