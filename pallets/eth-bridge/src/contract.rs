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

use alloc::boxed::Box;
use ethabi::Function;
use once_cell::race::OnceBox;
#[cfg(feature = "std")]
use sp_core::RuntimeDebug;
use sp_core::H256;
use sp_std::collections::btree_map::BTreeMap;

/// Avoid of Contract struct never used warning
#[allow(dead_code)]
pub mod eth_bridge_contract {
    use alloc::string::String;
    use sp_std::prelude::*;

    #[derive(ethabi_derive::EthabiContract)]
    #[ethabi_contract_options(path = "src/res/contract.abi")]
    struct Contract;
}

pub const METHOD_ID_SIZE: usize = 4;
pub type MethodId = [u8; METHOD_ID_SIZE];

pub fn calculate_method_id(function: &Function) -> MethodId {
    let id = function.short_signature();
    id
}

pub static ADD_ETH_NATIVE_TOKEN_FN: OnceBox<Function> = OnceBox::new();
pub static ADD_ETH_NATIVE_TOKEN_ID: OnceBox<MethodId> = OnceBox::new();
pub static ADD_ETH_NATIVE_TOKEN_TX_HASH_ARG_POS: usize = 4;

pub static ADD_NEW_SIDECHAIN_TOKEN_FN: OnceBox<Function> = OnceBox::new();
pub static ADD_NEW_SIDECHAIN_TOKEN_ID: OnceBox<MethodId> = OnceBox::new();
pub static ADD_NEW_SIDECHAIN_TOKEN_TX_HASH_ARG_POS: usize = 5;

pub static ADD_PEER_BY_PEER_FN: OnceBox<Function> = OnceBox::new();
pub static ADD_PEER_BY_PEER_ID: OnceBox<MethodId> = OnceBox::new();
pub static ADD_PEER_BY_PEER_TX_HASH_ARG_POS: usize = 1;

pub static REMOVE_PEER_BY_PEER_FN: OnceBox<Function> = OnceBox::new();
pub static REMOVE_PEER_BY_PEER_ID: OnceBox<MethodId> = OnceBox::new();
pub static REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS: usize = 1;

pub static RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_FN: OnceBox<Function> = OnceBox::new();
pub static RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID: OnceBox<MethodId> = OnceBox::new();
pub static RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_TX_HASH_ARG_POS: usize = 4;

pub static RECEIVE_BY_SIDECHAIN_ASSET_ID_FN: OnceBox<Function> = OnceBox::new();
pub static RECEIVE_BY_SIDECHAIN_ASSET_ID_ID: OnceBox<MethodId> = OnceBox::new();
pub static RECEIVE_BY_SIDECHAIN_ASSET_ID_TX_HASH_ARG_POS: usize = 4;

pub struct FunctionMeta {
    pub function: Function,
    pub tx_hash_arg_pos: usize,
}

impl FunctionMeta {
    pub fn new(function: Function, tx_hash_arg_pos: usize) -> Self {
        FunctionMeta {
            function,
            tx_hash_arg_pos,
        }
    }
}

pub static FUNCTIONS: OnceBox<BTreeMap<MethodId, FunctionMeta>> = OnceBox::new();

pub fn init_add_peer_by_peer_fn() -> Box<MethodId> {
    let add_peer_by_peer_fn = ADD_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::add_peer_by_peer::function()));
    Box::new(calculate_method_id(&add_peer_by_peer_fn))
}

pub fn init_remove_peer_by_peer_fn() -> Box<MethodId> {
    let remove_peer_by_peer_fn = REMOVE_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::remove_peer_by_peer::function()));
    Box::new(calculate_method_id(&remove_peer_by_peer_fn))
}

pub fn functions() -> Box<BTreeMap<MethodId, FunctionMeta>> {
    let add_eth_native_token_fn = ADD_ETH_NATIVE_TOKEN_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::add_eth_native_token::function()));
    let add_new_sidechain_token_fn = ADD_NEW_SIDECHAIN_TOKEN_FN.get_or_init(|| {
        Box::new(eth_bridge_contract::functions::add_new_sidechain_token::function())
    });
    let add_peer_by_peer_fn = ADD_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::add_peer_by_peer::function()));
    let remove_peer_by_peer_fn = REMOVE_PEER_BY_PEER_FN
        .get_or_init(|| Box::new(eth_bridge_contract::functions::remove_peer_by_peer::function()));
    let receive_by_eth_asset_address_fn = RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_FN.get_or_init(|| {
        Box::new(eth_bridge_contract::functions::receive_by_ethereum_asset_address::function())
    });
    let receive_by_sidechain_asset_id_fn = RECEIVE_BY_SIDECHAIN_ASSET_ID_FN.get_or_init(|| {
        Box::new(eth_bridge_contract::functions::receive_by_sidechain_asset_id::function())
    });
    let map = vec![
        (
            *ADD_ETH_NATIVE_TOKEN_ID
                .get_or_init(|| Box::new(calculate_method_id(&add_eth_native_token_fn))),
            FunctionMeta::new(
                add_eth_native_token_fn.clone(),
                ADD_ETH_NATIVE_TOKEN_TX_HASH_ARG_POS,
            ),
        ),
        (
            *ADD_NEW_SIDECHAIN_TOKEN_ID
                .get_or_init(|| Box::new(calculate_method_id(&add_new_sidechain_token_fn))),
            FunctionMeta::new(
                add_new_sidechain_token_fn.clone(),
                ADD_NEW_SIDECHAIN_TOKEN_TX_HASH_ARG_POS,
            ),
        ),
        (
            *ADD_PEER_BY_PEER_ID.get_or_init(init_add_peer_by_peer_fn),
            FunctionMeta::new(
                add_peer_by_peer_fn.clone(),
                ADD_PEER_BY_PEER_TX_HASH_ARG_POS,
            ),
        ),
        (
            *REMOVE_PEER_BY_PEER_ID.get_or_init(init_remove_peer_by_peer_fn),
            FunctionMeta::new(
                remove_peer_by_peer_fn.clone(),
                REMOVE_PEER_BY_PEER_TX_HASH_ARG_POS,
            ),
        ),
        (
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID
                .get_or_init(|| Box::new(calculate_method_id(&receive_by_eth_asset_address_fn))),
            FunctionMeta::new(
                receive_by_eth_asset_address_fn.clone(),
                RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_TX_HASH_ARG_POS,
            ),
        ),
        (
            *RECEIVE_BY_SIDECHAIN_ASSET_ID_ID
                .get_or_init(|| Box::new(calculate_method_id(&receive_by_sidechain_asset_id_fn))),
            FunctionMeta::new(
                receive_by_sidechain_asset_id_fn.clone(),
                RECEIVE_BY_SIDECHAIN_ASSET_ID_TX_HASH_ARG_POS,
            ),
        ),
    ]
    .into_iter()
    .collect();
    Box::new(map)
}

/// Contract's deposit event, means that someone transferred some amount of the token/asset to the
/// bridge contract.
#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub struct DepositEvent<EthAddress, AccountId, Balance> {
    pub(crate) destination: AccountId,
    pub(crate) amount: Balance,
    pub(crate) token: EthAddress,
    pub(crate) sidechain_asset: H256,
}

impl<EthAddress, AccountId, Balance> DepositEvent<EthAddress, AccountId, Balance> {
    pub fn new(
        destination: AccountId,
        amount: Balance,
        token: EthAddress,
        sidechain_asset: H256,
    ) -> Self {
        DepositEvent {
            destination,
            amount,
            token,
            sidechain_asset,
        }
    }
}

/// Events that can be emitted by Sidechain smart-contract.
#[cfg_attr(feature = "std", derive(PartialEq, Eq, RuntimeDebug))]
pub enum ContractEvent<EthAddress, AccountId, Balance> {
    Deposit(DepositEvent<EthAddress, AccountId, Balance>),
    ChangePeers(EthAddress, bool),
    PreparedForMigration,
    Migrated(EthAddress),
}
