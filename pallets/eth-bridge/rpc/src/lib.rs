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
pub use eth_bridge_runtime_api::EthBridgeRuntimeApi;
use jsonrpsee::{
    core::{Error as RpcError, RpcResult},
    proc_macros::rpc,
    types::error::CallError,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;
use std::sync::Arc;

#[rpc(server, client)]
pub trait EthBridgeApi<
    BlockHash,
    Hash,
    Approval,
    AccountId,
    AssetKind,
    AssetId,
    EthAddress,
    OffchainRequest,
    RequestStatus,
    OutgoingRequestEncoded,
    DispatchError,
    NetworkId,
    BalancePrecision,
>
{
    #[method(name = "ethBridge_getRequests")]
    fn get_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        redirect_finished_load_requests: Option<bool>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>>;

    #[method(name = "ethBridge_getApprovedRequests")]
    fn get_approved_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(OutgoingRequestEncoded, Vec<Approval>)>, DispatchError>>;

    #[method(name = "ethBridge_getApprovals")]
    fn get_approvals(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<Vec<Approval>>, DispatchError>>;

    #[method(name = "ethBridge_getAccountRequests")]
    fn get_account_requests(
        &self,
        account_id: AccountId,
        status_filter: Option<RequestStatus>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(NetworkId, Hash)>, DispatchError>>;

    #[method(name = "ethBridge_getRegisteredAssets")]
    fn get_registered_assets(
        &self,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<
        Result<
            Vec<(
                AssetKind,
                (AssetId, BalancePrecision),
                Option<(EthAddress, BalancePrecision)>,
            )>,
            DispatchError,
        >,
    >;
}

pub struct EthBridgeRpc<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> EthBridgeRpc<C, B> {
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
        Hash,
        Approval,
        AccountId,
        AssetKind,
        AssetId,
        EthAddress,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        NetworkId,
        BalancePrecision,
    >
    EthBridgeApiServer<
        <Block as BlockT>::Hash,
        Hash,
        Approval,
        AccountId,
        AssetKind,
        AssetId,
        EthAddress,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        DispatchError,
        NetworkId,
        BalancePrecision,
    > for EthBridgeRpc<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EthBridgeRuntimeApi<
        Block,
        Hash,
        Approval,
        AccountId,
        AssetKind,
        AssetId,
        EthAddress,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        NetworkId,
        BalancePrecision,
    >,
    Approval: Codec,
    Hash: Codec,
    AccountId: Codec,
    AssetKind: Codec,
    AssetId: Codec,
    EthAddress: Codec,
    OffchainRequest: Codec,
    RequestStatus: Codec,
    OutgoingRequestEncoded: Codec,
    NetworkId: Codec,
    BalancePrecision: Codec,
{
    fn get_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        redirect_finished_load_requests: Option<bool>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_requests(
            &at,
            request_hashes,
            network_id,
            redirect_finished_load_requests.unwrap_or(true),
        )
        .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn get_approved_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(OutgoingRequestEncoded, Vec<Approval>)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_approved_requests(&at, request_hashes, network_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn get_approvals(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<Vec<Approval>>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_approvals(&at, request_hashes, network_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn get_account_requests(
        &self,
        account_id: AccountId,
        status_filter: Option<RequestStatus>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(NetworkId, Hash)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_account_requests(&at, account_id, status_filter)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }

    fn get_registered_assets(
        &self,
        network_id: Option<NetworkId>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<
        Result<
            Vec<(
                AssetKind,
                (AssetId, BalancePrecision),
                Option<(EthAddress, BalancePrecision)>,
            )>,
            DispatchError,
        >,
    > {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_registered_assets(&at, network_id)
            .map_err(|e| RpcError::Call(CallError::Failed(e.into())))
    }
}
