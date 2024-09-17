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
// to endorse or promote products derived from this software without specific prior written
// permission.

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
use sp_runtime::traits::{Block as BlockT, MaybeDisplay, MaybeFromStr};

use std::sync::Arc;

// Runtime API imports.
pub use vested_rewards_runtime_api::{
    BalanceInfo, CrowdloanLease, VestedRewardsApi as VestedRewardsRuntimeApi,
};

#[rpc(server, client)]
pub trait VestedRewardsApi<BlockHash, AccountId, AssetId, OptionBalanceInfo, CrowdloanTag> {
    #[method(name = "vestedRewards_crowdloanClaimable")]
    fn crowdloan_claimable(
        &self,
        tag: CrowdloanTag,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<OptionBalanceInfo>;

    #[method(name = "vestedRewards_crowdloanLease")]
    fn crowdloan_lease(&self, tag: CrowdloanTag, at: Option<BlockHash>) -> Result<CrowdloanLease>;
}

pub struct VestedRewardsClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> VestedRewardsClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AccountId, AssetId, Balance, CrowdloanTag>
    VestedRewardsApiServer<
        <Block as BlockT>::Hash,
        AccountId,
        AssetId,
        Option<BalanceInfo<Balance>>,
        CrowdloanTag,
    > for VestedRewardsClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: VestedRewardsRuntimeApi<Block, AccountId, AssetId, Balance, CrowdloanTag>,
    AccountId: Codec,
    AssetId: Codec,
    CrowdloanTag: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
{
    fn crowdloan_claimable(
        &self,
        tag: CrowdloanTag,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.crowdloan_claimable(at, tag, account_id, asset_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn crowdloan_lease(
        &self,
        tag: CrowdloanTag,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<CrowdloanLease> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        let lease = api
            .crowdloan_lease(at, tag)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))?
            .ok_or(RpcError::Custom("Crowdloan not found".into()))?;
        Ok(lease)
    }
}
