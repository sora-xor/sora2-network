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

use beefy_light_client_runtime_api::SubNetworkId;
use codec::Codec;
use jsonrpsee::{core::RpcResult as Result, proc_macros::rpc, types::ErrorObjectOwned};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

pub use beefy_light_client_runtime_api::BeefyLightClientAPI as BeefyLightClientRuntimeAPI;

fn runtime_error_into_rpc_error(error: impl core::fmt::Debug) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(1, "Runtime error", Some(format!("{error:?}")))
}

#[rpc(server)]
pub trait BeefyLightClientAPI<BHash, Bitfield> {
    #[method(name = "beefyLightClient_getRandomBitfield")]
    fn get_random_bitfield(
        &self,
        network_id: SubNetworkId,
        prior: Bitfield,
        num_of_validators: u32,
        at: Option<BHash>,
    ) -> Result<Bitfield>;
}

pub struct BeefyLightClientClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> BeefyLightClientClient<C, B> {
    /// Construct default `TradingPairClient`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, B, Bitfield> BeefyLightClientAPIServer<<B as BlockT>::Hash, Bitfield>
    for BeefyLightClientClient<C, B>
where
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<B> + HeaderBackend<B>,
    C::Api: BeefyLightClientRuntimeAPI<B, Bitfield>,
    B: BlockT,
    Bitfield: Codec,
{
    fn get_random_bitfield(
        &self,
        network_id: SubNetworkId,
        prior: Bitfield,
        num_of_validators: u32,
        at: Option<<B as BlockT>::Hash>,
    ) -> Result<Bitfield> {
        let api = self.client.runtime_api();
        let at = at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        );
        api.get_random_bitfield(at, network_id, prior, num_of_validators)
            .map_err(runtime_error_into_rpc_error)
    }
}
