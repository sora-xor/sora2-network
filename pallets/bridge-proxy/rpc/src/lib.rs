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

use bridge_types::{
    types::{BridgeAppInfo, BridgeAssetInfo},
    GenericNetworkId,
};
use codec::{Codec, Decode, Encode};

use jsonrpsee::{
    core::{Error as RpcError, RpcResult as Result},
    proc_macros::rpc,
    types::error::CallError,
};
use serde::{Deserialize, Serialize};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Block as BlockT;

use std::sync::Arc;

pub use bridge_proxy_runtime_api::BridgeProxyAPI as BridgeProxyRuntimeAPI;

#[derive(Eq, PartialEq, Encode, Decode, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct AppsWithSupportedAssets {
    apps: Vec<BridgeAppInfo>,
    assets: Vec<BridgeAssetInfo>,
}

#[rpc(server, client)]
pub trait BridgeProxyAPI<BlockHash, AssetId>
where
    BlockHash: Codec,
    AssetId: Codec + Serialize,
{
    #[method(name = "bridgeProxy_listApps")]
    fn list_apps(&self, at: Option<BlockHash>) -> Result<Vec<BridgeAppInfo>>;

    #[method(name = "bridgeProxy_listAssets")]
    fn list_supported_assets(
        &self,
        network_id: GenericNetworkId,
        at: Option<BlockHash>,
    ) -> Result<Vec<BridgeAssetInfo>>;
}

pub struct BridgeProxyClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> BridgeProxyClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AssetId> BridgeProxyAPIServer<<Block as BlockT>::Hash, AssetId>
    for BridgeProxyClient<C, Block>
where
    Block: BlockT,
    AssetId: Codec + Serialize + Clone,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: BridgeProxyRuntimeAPI<Block, AssetId>,
{
    fn list_apps(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<BridgeAppInfo>> {
        let at = at.unwrap_or_else(|| self.client.info().best_hash);
        let api = self.client.runtime_api();
        api.list_apps(at)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn list_supported_assets(
        &self,
        network_id: GenericNetworkId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<BridgeAssetInfo>> {
        let at = at.unwrap_or_else(|| self.client.info().best_hash);
        let api = self.client.runtime_api();
        api.list_supported_assets(at, network_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
