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

use codec::Codec;

use jsonrpsee::{
    core::{Error as RpcError, RpcResult as Result},
    proc_macros::rpc,
    types::error::CallError,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay, MaybeFromStr};

use std::sync::Arc;

// Runtime API imports.
pub use assets_runtime_api::AssetsAPI as AssetsRuntimeAPI;
use assets_runtime_api::{AssetInfo, BalanceInfo};
use common::IsValid;

#[rpc(client, server)]
pub trait AssetsAPI<
    BlockHash,
    AccountId,
    AssetId,
    Balance,
    OptionBalanceInfo,
    OptionAssetInfo,
    VecAssetInfo,
    VecAssetId,
>
{
    #[method(name = "assets_freeBalance")]
    fn free_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<OptionBalanceInfo>;

    #[method(name = "assets_usableBalance")]
    fn usable_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<OptionBalanceInfo>;

    #[method(name = "assets_totalBalance")]
    fn total_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<OptionBalanceInfo>;

    #[method(name = "assets_totalSupply")]
    fn total_supply(&self, asset_id: AssetId, at: Option<BlockHash>) -> Result<OptionBalanceInfo>;

    #[method(name = "assets_listAssetIds")]
    fn list_asset_ids(&self, at: Option<BlockHash>) -> Result<VecAssetId>;

    #[method(name = "assets_listAssetInfos")]
    fn list_asset_infos(&self, at: Option<BlockHash>) -> Result<VecAssetInfo>;

    #[method(name = "assets_getAssetInfo")]
    fn get_asset_info(&self, asset_id: AssetId, at: Option<BlockHash>) -> Result<OptionAssetInfo>;
}

pub struct AssetsClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> AssetsClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<
        C,
        Block,
        AccountId,
        AssetId,
        Balance,
        AssetSymbol,
        AssetName,
        Precision,
        ContentSource,
        Description,
    >
    AssetsAPIServer<
        <Block as BlockT>::Hash,
        AccountId,
        AssetId,
        Balance,
        Option<BalanceInfo<Balance>>,
        Option<AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description>>,
        Vec<AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description>>,
        Vec<AssetId>,
    > for AssetsClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: AssetsRuntimeAPI<
        Block,
        AccountId,
        AssetId,
        Balance,
        AssetSymbol,
        AssetName,
        Precision,
        ContentSource,
        Description,
    >,
    AccountId: Codec,
    AssetId: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
    AssetSymbol: Codec + MaybeFromStr + MaybeDisplay + IsValid,
    AssetName: Codec + MaybeFromStr + MaybeDisplay + IsValid,
    Precision: Codec + MaybeFromStr + MaybeDisplay,
    ContentSource: Codec + MaybeFromStr + MaybeDisplay + IsValid,
    Description: Codec + MaybeFromStr + MaybeDisplay + IsValid,
{
    fn free_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.free_balance(at, account_id, asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn usable_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.usable_balance(at, account_id, asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn total_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.total_balance(at, account_id, asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn total_supply(
        &self,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.total_supply(at, asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn list_asset_ids(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<AssetId>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.list_asset_ids(at)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn list_asset_infos(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<
        Vec<AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description>>,
    > {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.list_asset_infos(at)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn get_asset_info(
        &self,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<
        Option<AssetInfo<AssetId, AssetSymbol, AssetName, Precision, ContentSource, Description>>,
    > {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.get_asset_info(at, asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
