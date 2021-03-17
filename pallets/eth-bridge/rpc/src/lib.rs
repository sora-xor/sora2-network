use codec::Codec;
use common::InvokeRPCError;
pub use eth_bridge_runtime_api::EthBridgeRuntimeApi;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result as RpcResult};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;
use std::sync::Arc;

#[rpc]
pub trait EthBridgeApi<
    BlockHash,
    Hash,
    Approval,
    AccountId,
    AssetKind,
    AssetId,
    Address,
    OffchainRequest,
    RequestStatus,
    OutgoingRequestEncoded,
    DispatchError,
    NetworkId,
>
{
    #[rpc(name = "ethBridge_getRequests")]
    fn get_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>>;

    #[rpc(name = "ethBridge_getApprovedRequests")]
    fn get_approved_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(OutgoingRequestEncoded, Vec<Approval>)>, DispatchError>>;

    #[rpc(name = "ethBridge_getApprovals")]
    fn get_approvals(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<Vec<Approval>>, DispatchError>>;

    #[rpc(name = "ethBridge_getAccountRequests")]
    fn get_account_requests(
        &self,
        account_id: AccountId,
        status_filter: Option<RequestStatus>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(NetworkId, Hash)>, DispatchError>>;

    #[rpc(name = "ethBridge_getRegisteredAssets")]
    fn get_registered_assets(
        &self,
        network_id: Option<NetworkId>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(AssetKind, AssetId, Option<Address>)>, DispatchError>>;
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
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        NetworkId,
    >
    EthBridgeApi<
        <Block as BlockT>::Hash,
        Hash,
        Approval,
        AccountId,
        AssetKind,
        AssetId,
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        DispatchError,
        NetworkId,
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
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        NetworkId,
    >,
    Approval: Codec,
    Hash: Codec,
    AccountId: Codec,
    AssetKind: Codec,
    AssetId: Codec,
    Address: Codec,
    OffchainRequest: Codec,
    RequestStatus: Codec,
    OutgoingRequestEncoded: Codec,
    NetworkId: Codec,
{
    fn get_requests(
        &self,
        request_hashes: Vec<Hash>,
        network_id: Option<NetworkId>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_requests(&at, request_hashes, network_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get off-chain requests and statuses.".into(),
                data: Some(format!("{:?}", e).into()),
            })
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
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get encoded off-chain requests and approvals.".into(),
                data: Some(format!("{:?}", e).into()),
            })
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
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get approvals of the requests.".into(),
                data: Some(format!("{:?}", e).into()),
            })
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
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get account requests.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn get_registered_assets(
        &self,
        network_id: Option<NetworkId>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(AssetKind, AssetId, Option<Address>)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_registered_assets(&at, network_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get registered assets and tokens.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }
}
