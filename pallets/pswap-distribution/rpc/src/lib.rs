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

// Runtime API imports.
use pswap_distribution_runtime_api::BalanceInfo;
pub use pswap_distribution_runtime_api::PswapDistributionAPI as PswapDistributionRuntimeAPI;

#[rpc]
pub trait PswapDistributionAPI<BlockHash, AccountId, BalanceInfo> {
    #[rpc(name = "pswapDistribution_claimableAmount")]
    fn claimable_amount(&self, account_id: AccountId, at: Option<BlockHash>)
        -> Result<BalanceInfo>;
}

pub struct PswapDistributionClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> PswapDistributionClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AccountId, Balance>
    PswapDistributionAPI<<Block as BlockT>::Hash, AccountId, BalanceInfo<Balance>>
    for PswapDistributionClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: PswapDistributionRuntimeAPI<Block, AccountId, Balance>,
    AccountId: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
{
    fn claimable_amount(
        &self,
        account_id: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<BalanceInfo<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.claimable_amount(&at, account_id).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get claimable PSWAP amount.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
