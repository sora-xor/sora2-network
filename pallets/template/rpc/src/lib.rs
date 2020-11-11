use codec::Codec;

use common::InvokeRPCError;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
};

use std::sync::Arc;

// Custom imports
use template_runtime_api::CustomInfo;
pub use template_runtime_api::TemplateAPI as TemplateRuntimeAPI;

#[rpc]
pub trait TemplateAPI<BlockHash, Balance, OutputTy> {
    #[rpc(name = "template_testMultiply2")]
    fn test_multiply_2(&self, amount: Balance, at: Option<BlockHash>) -> Result<OutputTy>;
}

pub struct Template<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> Template<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, Balance> TemplateAPI<<Block as BlockT>::Hash, Balance, Option<CustomInfo<Balance>>>
    for Template<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: TemplateRuntimeAPI<Block, Balance>,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
{
    fn test_multiply_2(
        &self,
        amount: Balance,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<CustomInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.test_multiply_2(&at, amount).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to invoke test function.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
