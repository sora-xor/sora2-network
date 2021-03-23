use codec::Codec;

use common::InvokeRPCError;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay, MaybeFromStr};

use std::sync::Arc;

// Runtime API imports.
pub use rewards_runtime_api::{BalanceInfo, RewardsAPI as RewardsRuntimeAPI};

#[rpc]
pub trait RewardsAPI<BlockHash, EthereumAddress, VecBalanceInfo> {
    #[rpc(name = "rewards_claimables")]
    fn claimables(
        &self,
        eth_address: EthereumAddress,
        at: Option<BlockHash>,
    ) -> Result<VecBalanceInfo>;
}

pub struct RewardsClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> RewardsClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, EthereumAddress, Balance>
    RewardsAPI<<Block as BlockT>::Hash, EthereumAddress, Vec<BalanceInfo<Balance>>>
    for RewardsClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: RewardsRuntimeAPI<Block, EthereumAddress, Balance>,
    EthereumAddress: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
{
    fn claimables(
        &self,
        eth_address: EthereumAddress,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.claimables(&at, eth_address).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get claimables.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
