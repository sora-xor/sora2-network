// This file is part of the SORA network and Polkaswap app.
//
// Copyright (c) 2026, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

use bridge_types::ton::{TonAddress, TonNetworkId};
use bridge_types::traits::{
    BridgeAssetLockChecker, BridgeAssetLocker, BridgeAssetRegistry, EVMBridgeWithdrawFee,
};
use bridge_types::{GenericAccount, GenericNetworkId, H160};
use common::{AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};
use frame_support::{assert_noop, assert_ok};
use framenode_chain_spec::ext;
use sccp::LegacyBridgeAssetChecker;
use sp_core::H256;
use traits::MultiCurrency;

use crate::{Assets, Currencies, LegacyBridgeChecker, Runtime, RuntimeOrigin, Sccp};

#[test]
fn sccp_add_token_rejects_asset_on_legacy_eth_bridge() {
    ext().execute_with(|| {
        let asset_id = common::XOR.into();
        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_with_pending_legacy_eth_add_asset_request() {
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
            AssetSymbol(b"SCCPPA".to_vec()),
            AssetName(b"SCCP Pending Add".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_ok!(crate::EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            evm_net_id,
        ));
        assert!(crate::EthBridge::registered_asset(evm_net_id, asset_id).is_none());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_on_secondary_legacy_eth_bridge_network() {
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
            AssetSymbol(b"SCCP2E".to_vec()),
            AssetName(b"SCCP Legacy EVM Net".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let secondary_net_id = crate::EthBridge::next_network_id();
        assert_ok!(crate::EthBridge::register_bridge(
            RuntimeOrigin::root(),
            H160::repeat_byte(0x31),
            vec![owner],
            eth_bridge::BridgeSignatureVersion::V3,
        ));
        assert_ok!(crate::EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(),
            asset_id,
            H160::repeat_byte(0x32),
            secondary_net_id,
        ));
        assert!(crate::EthBridge::registered_asset(secondary_net_id, asset_id).is_some());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_with_pending_legacy_eth_add_asset_on_secondary_network() {
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
            AssetSymbol(b"SCCPP2".to_vec()),
            AssetName(b"SCCP Pending Other Net".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let secondary_net_id = crate::EthBridge::next_network_id();
        assert_ok!(crate::EthBridge::register_bridge(
            RuntimeOrigin::root(),
            H160::repeat_byte(0x33),
            vec![owner],
            eth_bridge::BridgeSignatureVersion::V3,
        ));
        assert_ok!(crate::EthBridge::add_asset(
            RuntimeOrigin::root(),
            asset_id,
            secondary_net_id,
        ));
        assert!(crate::EthBridge::registered_asset(secondary_net_id, asset_id).is_none());
        assert!(LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_rejects_asset_on_legacy_ton_bridge() {
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
            AssetSymbol(b"SCCPTON".to_vec()),
            AssetName(b"SCCP TON Legacy".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(crate::JettonApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            TonNetworkId::Mainnet,
            TonAddress::new(0, H256::repeat_byte(0x44)),
            asset_id,
            9,
        ));

        assert_noop!(
            Sccp::add_token(RuntimeOrigin::root(), asset_id),
            sccp::Error::<Runtime>::AssetOnLegacyBridge
        );
    });
}

#[test]
fn sccp_add_token_accepts_non_legacy_asset() {
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
            AssetSymbol(b"SCCPNL".to_vec()),
            AssetName(b"SCCP Non Legacy".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert!(!LegacyBridgeChecker::is_legacy_bridge_asset(&asset_id));

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let token_state = Sccp::token_state(asset_id).expect("token should be registered");
        assert_eq!(token_state.status, sccp::TokenStatus::Pending);
    });
}

#[test]
fn sccp_asset_blocks_eth_bridge_add_asset_in_runtime() {
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
            AssetSymbol(b"SCCPEVM".to_vec()),
            AssetName(b"SCCP EVM Blocked".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::EthBridge::add_asset(RuntimeOrigin::root(), asset_id, evm_net_id),
            eth_bridge::Error::<Runtime>::SccpAssetNotAllowed
        );
    });
}

#[test]
fn sccp_asset_does_not_block_eth_bridge_add_sidechain_token_for_new_asset_id() {
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
            AssetSymbol(b"SCCPEST".to_vec()),
            AssetName(b"SCCP Eth Sidechain Token".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let token_address = H160::repeat_byte(0x9a);
        assert_ok!(crate::EthBridge::add_sidechain_token(
            RuntimeOrigin::root(),
            token_address,
            "SCCPETH".into(),
            "SCCP Eth Sidechain".into(),
            18,
            evm_net_id,
        ));
        assert!(crate::EthBridge::registered_sidechain_asset(evm_net_id, token_address).is_none());
        assert!(crate::EthBridge::is_add_token_request_pending(
            evm_net_id,
            token_address
        ));
    });
}

#[test]
fn sccp_asset_blocks_jetton_register_network_with_existing_asset_in_runtime() {
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
            AssetSymbol(b"SCCPTOB".to_vec()),
            AssetName(b"SCCP TON Blocked".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let err = crate::JettonApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            TonNetworkId::Mainnet,
            TonAddress::new(0, H256::repeat_byte(0x66)),
            asset_id,
            9,
        )
        .unwrap_err();
        assert_eq!(
            err,
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable.into()
        );
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_burn_in_runtime() {
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
            AssetSymbol(b"SCCPBP".to_vec()),
            AssetName(b"SCCP BridgeProxy Burn".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::BridgeProxy::burn(
                RuntimeOrigin::signed(owner),
                GenericNetworkId::EVMLegacy(evm_net_id),
                asset_id,
                GenericAccount::EVM(H160::repeat_byte(0x11)),
                1u32.into(),
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}

#[test]
fn sccp_asset_blocks_eth_bridge_transfer_to_sidechain_in_runtime() {
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
            AssetSymbol(b"SCCPET".to_vec()),
            AssetName(b"SCCP Eth Transfer".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(owner),
                asset_id,
                H160::repeat_byte(0x22),
                1u32.into(),
                evm_net_id,
            ),
            eth_bridge::Error::<Runtime>::SccpAssetNotAllowed
        );
    });
}

#[test]
fn sccp_failed_incoming_transfer_rolls_back_bridge_lock_accounting_in_runtime() {
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
            AssetSymbol(b"SCCPIR".to_vec()),
            AssetName(b"SCCP Incoming Revert".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let amount = 7u32.into();
        let incoming_transfer = eth_bridge::requests::IncomingTransfer::<Runtime> {
            from: H160::repeat_byte(0x2b),
            to: owner.clone(),
            asset_id,
            asset_kind: eth_bridge::requests::AssetKind::Sidechain,
            amount,
            author: owner.clone(),
            tx_hash: H256::repeat_byte(0xa7),
            at_height: 1,
            timepoint: Default::default(),
            network_id: evm_net_id,
            should_take_fee: false,
        };
        assert_ok!(incoming_transfer.prepare());
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            amount
        );

        let incoming_request = eth_bridge::requests::IncomingRequest::Transfer(incoming_transfer);
        let offchain_request = eth_bridge::requests::OffchainRequest::incoming(incoming_request);
        let request_hash = match &offchain_request {
            eth_bridge::requests::OffchainRequest::Incoming(_, hash) => *hash,
            _ => unreachable!(),
        };
        eth_bridge::Requests::<Runtime>::insert(evm_net_id, request_hash, offchain_request);
        eth_bridge::RequestsQueue::<Runtime>::mutate(evm_net_id, |queue| queue.push(request_hash));
        eth_bridge::RequestStatuses::<Runtime>::insert(
            evm_net_id,
            request_hash,
            eth_bridge::requests::RequestStatus::Pending,
        );

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::finalize_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            request_hash,
            evm_net_id,
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, request_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::SccpAssetNotAllowed.into()
            ))
        );
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            0u32.into()
        );
    });
}

#[test]
fn abort_outgoing_transfer_rolls_back_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let asset_id = common::XOR.into();
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            owner.clone(),
            asset_id,
            100i128,
        ));

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let amount = 9u32.into();
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);

        assert_ok!(crate::EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(owner.clone()),
            asset_id,
            H160::repeat_byte(0x55),
            amount,
            evm_net_id,
        ));
        let request_hash = *crate::EthBridge::requests_queue(evm_net_id)
            .last()
            .expect("outgoing request hash should be queued");
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked + amount
        );

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::abort_request(
            RuntimeOrigin::signed(bridge_account),
            request_hash,
            eth_bridge::Error::<Runtime>::Cancelled.into(),
            evm_net_id,
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, request_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::Cancelled.into()
            ))
        );
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked
        );
    });
}

#[test]
fn incoming_transfer_prepare_failure_rolls_back_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let asset_id = common::XOR.into();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");

        let bridge_free_balance = Currencies::free_balance(asset_id, &bridge_account);
        let amount = bridge_free_balance.saturating_add(1u32.into());
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);

        assert_ok!(crate::BridgeProxy::before_asset_lock(
            network_id,
            bridge_types::types::AssetKind::Thischain,
            &asset_id,
            &amount,
        ));
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked + amount
        );

        let incoming_transfer = eth_bridge::requests::IncomingTransfer::<Runtime> {
            from: H160::repeat_byte(0x33),
            to: owner,
            asset_id,
            asset_kind: eth_bridge::requests::AssetKind::Thischain,
            amount,
            author: common::mock::bob(),
            tx_hash: H256::repeat_byte(0xb3),
            at_height: 1,
            timepoint: Default::default(),
            network_id: evm_net_id,
            should_take_fee: false,
        };

        assert!(incoming_transfer.prepare().is_err());
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked + amount
        );
    });
}

#[test]
fn sccp_incoming_queue_full_registration_does_not_change_bridge_lock_accounting_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let asset_id = common::XOR.into();
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);
        let sidechain_tx_hash = H256::repeat_byte(0x71);

        assert_ok!(crate::EthBridge::request_from_sidechain(
            RuntimeOrigin::signed(owner.clone()),
            sidechain_tx_hash,
            eth_bridge::requests::IncomingRequestKind::Transaction(
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer
            ),
            evm_net_id,
        ));

        eth_bridge::RequestsQueue::<Runtime>::mutate(evm_net_id, |queue| {
            for i in 0..2048u64 {
                queue.push(H256::from_low_u64_be(10_000 + i));
            }
        });

        let incoming_transfer = eth_bridge::requests::IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: H160::repeat_byte(0x44),
                to: owner.clone(),
                asset_id,
                asset_kind: eth_bridge::requests::AssetKind::Sidechain,
                amount: 1u32.into(),
                author: owner,
                tx_hash: sidechain_tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: evm_net_id,
                should_take_fee: false,
            },
        );

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        let err = crate::EthBridge::register_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            incoming_transfer,
        )
        .unwrap_err();
        assert_eq!(
            err.error,
            eth_bridge::Error::<Runtime>::RequestsQueueFull.into()
        );

        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked
        );
    });
}

#[test]
fn sccp_import_incoming_registration_failure_aborts_load_request_in_runtime() {
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
            AssetSymbol(b"SCCPIM".to_vec()),
            AssetName(b"SCCP Import Failure".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();
        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));

        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let network_id = GenericNetworkId::EVMLegacy(evm_net_id);
        let baseline_locked = crate::BridgeProxy::locked_assets(network_id, asset_id);
        let sidechain_tx_hash = H256::repeat_byte(0x72);

        let load_incoming_request = eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                owner.clone(),
                sidechain_tx_hash,
                Default::default(),
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                evm_net_id,
            ),
        );
        let incoming_request = eth_bridge::requests::IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: H160::repeat_byte(0x45),
                to: owner.clone(),
                asset_id,
                asset_kind: eth_bridge::requests::AssetKind::Sidechain,
                amount: 1u32.into(),
                author: owner,
                tx_hash: sidechain_tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: evm_net_id,
                should_take_fee: false,
            },
        );
        let load_hash = sidechain_tx_hash;
        let incoming_offchain_request =
            eth_bridge::requests::OffchainRequest::incoming(incoming_request.clone());
        let incoming_hash = match &incoming_offchain_request {
            eth_bridge::requests::OffchainRequest::Incoming(_, hash) => *hash,
            _ => unreachable!(),
        };

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::import_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            load_incoming_request,
            Ok(incoming_request),
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, load_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::SccpAssetNotAllowed.into()
            ))
        );
        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, incoming_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::SccpAssetNotAllowed.into()
            ))
        );
        assert!(!eth_bridge::RequestsQueue::<Runtime>::get(evm_net_id).contains(&load_hash));
        assert!(eth_bridge::Requests::<Runtime>::get(evm_net_id, incoming_hash).is_none());
        assert_eq!(
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(evm_net_id, load_hash),
            H256::zero()
        );
        assert_eq!(
            crate::BridgeProxy::locked_assets(network_id, asset_id),
            baseline_locked
        );
    });
}

#[test]
fn sccp_import_incoming_network_mismatch_aborts_load_request_in_runtime() {
    ext().execute_with(|| {
        let owner = common::mock::alice();
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        let sidechain_tx_hash = H256::repeat_byte(0x73);

        let load_incoming_request = eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                owner.clone(),
                sidechain_tx_hash,
                Default::default(),
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                evm_net_id,
            ),
        );
        let incoming_request = eth_bridge::requests::IncomingRequest::Transfer(
            eth_bridge::requests::IncomingTransfer::<Runtime> {
                from: H160::repeat_byte(0x46),
                to: owner,
                asset_id: common::XOR.into(),
                asset_kind: eth_bridge::requests::AssetKind::Thischain,
                amount: 1u32.into(),
                author: common::mock::bob(),
                tx_hash: sidechain_tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: evm_net_id.saturating_add(1),
                should_take_fee: false,
            },
        );
        let load_hash = sidechain_tx_hash;
        let incoming_offchain_request =
            eth_bridge::requests::OffchainRequest::incoming(incoming_request.clone());
        let incoming_hash = match &incoming_offchain_request {
            eth_bridge::requests::OffchainRequest::Incoming(_, hash) => *hash,
            _ => unreachable!(),
        };

        let bridge_account =
            crate::EthBridge::bridge_account(evm_net_id).expect("bridge account must exist");
        assert_ok!(crate::EthBridge::import_incoming_request(
            RuntimeOrigin::signed(bridge_account),
            load_incoming_request,
            Ok(incoming_request),
        ));

        assert_eq!(
            crate::EthBridge::request_status(evm_net_id, load_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                eth_bridge::Error::<Runtime>::UnknownNetwork.into()
            ))
        );
        assert!(eth_bridge::Requests::<Runtime>::get(evm_net_id, incoming_hash).is_none());
        assert_eq!(
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(evm_net_id, load_hash),
            H256::zero()
        );
        assert!(!eth_bridge::RequestsQueue::<Runtime>::get(evm_net_id).contains(&load_hash));
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_manage_asset_in_runtime() {
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
            AssetSymbol(b"SCCPBM".to_vec()),
            AssetName(b"SCCP BridgeProxy Manage".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::BridgeProxy::manage_asset(GenericNetworkId::EVMLegacy(evm_net_id), asset_id),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}

#[test]
fn sccp_asset_blocks_eth_bridge_register_existing_sidechain_asset_in_runtime() {
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
            AssetSymbol(b"SCCPER".to_vec()),
            AssetName(b"SCCP Eth Register".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::EthBridge::register_existing_sidechain_asset(
                RuntimeOrigin::root(),
                asset_id,
                H160::repeat_byte(0x99),
                evm_net_id,
            ),
            eth_bridge::Error::<Runtime>::SccpAssetNotAllowed
        );
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_refund_in_runtime() {
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
            AssetSymbol(b"SCCPRF".to_vec()),
            AssetName(b"SCCP BridgeProxy Refund".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let evm_net_id = <Runtime as eth_bridge::Config>::GetEthNetworkId::get();
        assert_noop!(
            crate::BridgeProxy::refund(
                GenericNetworkId::EVMLegacy(evm_net_id),
                H256::repeat_byte(0x42),
                GenericAccount::Sora(owner.clone().into()),
                asset_id,
                1u32.into(),
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}

#[test]
fn sccp_asset_blocks_bridge_proxy_lock_unlock_and_fee_paths_in_runtime() {
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
            AssetSymbol(b"SCCPLF".to_vec()),
            AssetName(b"SCCP BridgeProxy Lock Fee".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            0u32.into(),
            true,
            common::AssetType::Regular,
            None,
            None,
        )
        .unwrap();

        assert_ok!(Sccp::add_token(RuntimeOrigin::root(), asset_id));
        let network_id =
            GenericNetworkId::EVMLegacy(<Runtime as eth_bridge::Config>::GetEthNetworkId::get());
        let amount = 1u32.into();

        assert_noop!(
            crate::BridgeProxy::lock_asset(
                network_id,
                bridge_types::types::AssetKind::Thischain,
                &owner,
                &asset_id,
                &amount,
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::unlock_asset(
                network_id,
                bridge_types::types::AssetKind::Thischain,
                &owner,
                &asset_id,
                &amount,
            ),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::withdraw_fee(network_id, &owner, &asset_id, &amount),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::refund_fee(network_id, &owner, &asset_id, &amount),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
        assert_noop!(
            crate::BridgeProxy::withdraw_transfer_fee(&owner, H256::zero(), asset_id,),
            bridge_proxy::Error::<Runtime>::PathIsNotAvailable
        );
    });
}
