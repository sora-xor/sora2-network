use codec::Codec;
use common::InvokeRPCError;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use sp_std::vec::Vec;
use std::sync::Arc;

// Runtime API imports.
pub use dex_manager_runtime_api::DEXManagerAPI as DEXManagerRuntimeAPI;

#[rpc]
pub trait DEXManagerAPI<BlockHash, DEXId> {
    #[rpc(name = "dexManager_listDEXIds")]
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

impl<C, Block, DEXId> DEXManagerAPI<<Block as BlockT>::Hash, DEXId> for DEXManager<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: DEXManagerRuntimeAPI<Block, DEXId>,
    DEXId: Codec,
{
    fn list_dex_ids(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<DEXId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_dex_ids(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to list DEXIds.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
