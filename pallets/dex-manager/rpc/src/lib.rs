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
use sp_runtime::traits::Block as BlockT;
use sp_std::vec::Vec;
use std::sync::Arc;

// Runtime API imports.
pub use dex_manager_runtime_api::DEXManagerAPI as DEXManagerRuntimeAPI;

#[rpc(server, client)]
pub trait DEXManagerAPI<BlockHash, DEXId> {
    #[method(name = "dexManager_listDEXIds")]
    fn list_dex_ids(&self, at: Option<BlockHash>) -> Result<Vec<DEXId>>;
}

pub struct DEXManager<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> DEXManager<C, B> {
    /// Construct default `DEX`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, DEXId> DEXManagerAPIServer<<Block as BlockT>::Hash, DEXId> for DEXManager<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: DEXManagerRuntimeAPI<Block, DEXId>,
    DEXId: Codec,
{
    fn list_dex_ids(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<DEXId>> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.list_dex_ids(at)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
