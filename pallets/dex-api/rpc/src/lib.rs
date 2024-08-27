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

// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use codec::Codec;
use common::BalanceWrapper;
use jsonrpsee::{
    core::{Error as RpcError, RpcResult as Result},
    proc_macros::rpc,
    types::error::CallError,
};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay, MaybeFromStr, Zero};
use std::sync::Arc;

// Runtime API imports.
use dex_runtime_api::SwapOutcomeInfo;
pub use dex_runtime_api::DEXAPI as DEXRuntimeAPI;

#[rpc(server, client)]
pub trait DEXAPI<BlockHash, AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant, SwapResponse>
{
    #[method(name = "dexApi_quote")]
    fn quote(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        amount: BalanceWrapper,
        swap_variant: SwapVariant,
        at: Option<BlockHash>,
    ) -> Result<SwapResponse>;

    #[method(name = "dexApi_canExchange")]
    fn can_exchange(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<bool>;

    #[method(name = "dexApi_listSupportedSources")]
    fn list_supported_sources(&self, at: Option<BlockHash>) -> Result<Vec<LiquiditySourceType>>;
}

pub struct DEX<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> DEX<C, B> {
    /// Construct default DEX as intermediary impl for rpc.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant>
    DEXAPIServer<
        <Block as BlockT>::Hash,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
        Option<SwapOutcomeInfo<Balance, AssetId>>,
    > for DEX<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: DEXRuntimeAPI<Block, AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant>,
    AssetId: Codec
        + MaybeFromStr
        + MaybeDisplay
        + Ord
        + From<common::AssetId32<common::PredefinedAssetId>>,
    DEXId: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay + Copy + Zero,
    SwapVariant: Codec,
    LiquiditySourceType: Codec,
{
    fn quote(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        amount: BalanceWrapper,
        swap_variant: SwapVariant,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<SwapOutcomeInfo<Balance, AssetId>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );

        let version = api
            .api_version::<dyn DEXRuntimeAPI<Block, AssetId, DEXId, Balance,LiquiditySourceType, SwapVariant>>(at)
            .map_err(|e| RpcError::Custom(format!("Runtime API error: {}", e)))?;

        let outcome = if version == Some(1) {
            #[allow(deprecated)]
            {
                api.quote_before_version_2(
                    at,
                    dex_id,
                    liquidity_source_type,
                    input_asset_id,
                    output_asset_id,
                    amount,
                    swap_variant,
                )
                .map_err(|e| RpcError::Call(CallError::Failed(e.into())))?
                .map(Into::into)
            }
        } else if version == Some(2) {
            api.quote(
                at,
                dex_id,
                liquidity_source_type,
                input_asset_id,
                output_asset_id,
                amount,
                swap_variant,
            )
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))?
        } else {
            return Err(RpcError::Custom(
                "Unsupported or invalid DEXRuntimeAPI version".to_string(),
            ));
        };
        Ok(outcome)
    }

    fn can_exchange(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.can_exchange(
            at,
            dex_id,
            liquidity_source_type,
            input_asset_id,
            output_asset_id,
        )
        .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn list_supported_sources(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<LiquiditySourceType>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.list_supported_sources(at)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
