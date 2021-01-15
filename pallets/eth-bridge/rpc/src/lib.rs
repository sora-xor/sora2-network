use codec::Codec;
use common::InvokeRPCError;
pub use eth_bridge_runtime_api::EthBridgeRuntimeApi;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result as RpcResult};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT, DispatchError};
use sp_std::vec::Vec;
use std::sync::Arc;

#[rpc]
pub trait EthBridgeApi<
    BlockHash,
    Hash,
    Approve,
    AccountId,
    AssetKind,
    AssetId,
    Address,
    OffchainRequest,
    RequestStatus,
    OutgoingRequestEncoded,
    DispatchError,
>
{
    #[rpc(name = "ethBridge_getRequests")]
    fn get_requests(
        &self,
        request_hashes: Vec<Hash>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>>;

    #[rpc(name = "ethBridge_getApprovedRequests")]
    fn get_approved_requests(
        &self,
        request_hashes: Vec<Hash>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<(OutgoingRequestEncoded, Vec<Approve>)>, DispatchError>>;

    #[rpc(name = "ethBridge_getApproves")]
    fn get_approves(
        &self,
        request_hashes: Vec<Hash>,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<Vec<Approve>>, DispatchError>>;

    #[rpc(name = "ethBridge_getAccountRequests")]
    fn get_account_requests(
        &self,
        account_id: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<Result<Vec<Hash>, DispatchError>>;

    #[rpc(name = "ethBridge_getRegisteredAssets")]
    fn get_registered_assets(
        &self,
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
        Approve,
        AccountId,
        AssetKind,
        AssetId,
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
    >
    EthBridgeApi<
        <Block as BlockT>::Hash,
        Hash,
        Approve,
        AccountId,
        AssetKind,
        AssetId,
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
        DispatchError,
    > for EthBridgeRpc<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: EthBridgeRuntimeApi<
        Block,
        Hash,
        Approve,
        AccountId,
        AssetKind,
        AssetId,
        Address,
        OffchainRequest,
        RequestStatus,
        OutgoingRequestEncoded,
    >,
    Approve: Codec,
    Hash: Codec,
    AccountId: Codec,
    AssetKind: Codec,
    AssetId: Codec,
    Address: Codec,
    OffchainRequest: Codec,
    RequestStatus: Codec,
    OutgoingRequestEncoded: Codec,
{
    fn get_requests(
        &self,
        request_hashes: Vec<Hash>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(OffchainRequest, RequestStatus)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_requests(&at, request_hashes).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get off-chain requests and statuses.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_approved_requests(
        &self,
        request_hashes: Vec<Hash>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(OutgoingRequestEncoded, Vec<Approve>)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_approved_requests(&at, request_hashes)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get encoded off-chain requests and approves.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn get_approves(
        &self,
        request_hashes: Vec<Hash>,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<Vec<Approve>>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_approves(&at, request_hashes).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get approves of the requests.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_account_requests(
        &self,
        account_id: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<Hash>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_account_requests(&at, account_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get account requests.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn get_registered_assets(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Result<Vec<(AssetKind, AssetId, Option<Address>)>, DispatchError>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        api.get_registered_assets(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get registered assets and tokens.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
