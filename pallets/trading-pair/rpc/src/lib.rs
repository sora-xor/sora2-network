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
use sp_runtime::traits::Block as BlockT;
use sp_std::vec::Vec;
use std::sync::Arc;

pub use trading_pair_runtime_api::TradingPairAPI as TradingPairRuntimeAPI;

#[rpc(client, server)]
pub trait TradingPairAPI<BlockHash, DexId, TradingPair, AssetId, LiquiditySourceType> {
    #[method(name = "tradingPair_listEnabledPairs")]
    fn list_enabled_pairs(&self, dex_id: DexId, at: Option<BlockHash>) -> Result<Vec<TradingPair>>;

    #[method(name = "tradingPair_isPairEnabled")]
    fn is_pair_enabled(
        &self,
        dex_id: DexId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<bool>;

    #[method(name = "tradingPair_listEnabledSourcesForPair")]
    fn list_enabled_sources_for_pair(
        &self,
        dex_id: DexId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<Vec<LiquiditySourceType>>;

    #[method(name = "tradingPair_isSourceEnabledForPair")]
    fn is_source_enabled_for_pair(
        &self,
        dex_id: DexId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        source_type: LiquiditySourceType,
        at: Option<BlockHash>,
    ) -> Result<bool>;
}

pub struct TradingPairClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> TradingPairClient<C, B> {
    /// Construct default `TradingPairClient`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, DexId, TradingPair, AssetId, LiquiditySourceType>
    TradingPairAPIServer<<Block as BlockT>::Hash, DexId, TradingPair, AssetId, LiquiditySourceType>
    for TradingPairClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: TradingPairRuntimeAPI<Block, DexId, TradingPair, AssetId, LiquiditySourceType>,
    DexId: Codec,
    TradingPair: Codec,
    AssetId: Codec,
    LiquiditySourceType: Codec,
{
    fn list_enabled_pairs(
        &self,
        dex_id: DexId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<TradingPair>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_enabled_pairs(&at, dex_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn is_pair_enabled(
        &self,
        dex_id: DexId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.is_pair_enabled(&at, dex_id, base_asset_id, target_asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn list_enabled_sources_for_pair(
        &self,
        dex_id: DexId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<LiquiditySourceType>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_enabled_sources_for_pair(&at, dex_id, base_asset_id, target_asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn is_source_enabled_for_pair(
        &self,
        dex_id: DexId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        source_type: LiquiditySourceType,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.is_source_enabled_for_pair(&at, dex_id, base_asset_id, target_asset_id, source_type)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
