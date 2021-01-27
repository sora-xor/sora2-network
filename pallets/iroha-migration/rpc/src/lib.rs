use common::InvokeRPCError;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

use std::sync::Arc;

// Runtime API imports.
pub use iroha_migration_runtime_api::IrohaMigrationAPI as IrohaMigrationRuntimeAPI;

#[rpc]
pub trait IrohaMigrationAPI<BlockHash> {
    #[rpc(name = "irohaMigration_needsMigration")]
    fn needs_migration(&self, iroha_address: String, at: Option<BlockHash>) -> Result<bool>;
}

pub struct IrohaMigrationClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> IrohaMigrationClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block> IrohaMigrationAPI<<Block as BlockT>::Hash> for IrohaMigrationClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: IrohaMigrationRuntimeAPI<Block>,
{
    fn needs_migration(&self, iroha_address: String, at: Option<Block::Hash>) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.needs_migration(&at, iroha_address).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to check if needs migration.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
