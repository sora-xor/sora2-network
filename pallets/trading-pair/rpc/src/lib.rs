use codec::Codec;
use common::InvokeRPCError;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use sp_std::vec::Vec;
use std::sync::Arc;

pub use trading_pair_runtime_api::TradingPairAPI as TradingPairRuntimeAPI;

#[rpc]
pub trait TradingPairAPI<BlockHash, DEXId, TradingPair, AssetId> {
    #[rpc(name = "tradingPair_listEnabledPairs")]
    fn list_enabled_pairs(&self, dex_id: DEXId, at: Option<BlockHash>) -> Result<Vec<TradingPair>>;

    #[rpc(name = "tradingPair_isPairEnabled")]
    fn is_pair_enabled(
        &self,
        dex_id: DEXId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<bool>;
}

pub struct TradingPairClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> TradingPairClient<C, B> {
    /// Construct default `TradingPairClient`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, DEXId, TradingPair, AssetId>
    TradingPairAPI<<Block as BlockT>::Hash, DEXId, TradingPair, AssetId>
    for TradingPairClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: TradingPairRuntimeAPI<Block, DEXId, TradingPair, AssetId>,
    DEXId: Codec,
    TradingPair: Codec,
    AssetId: Codec,
{
    fn list_enabled_pairs(
        &self,
        dex_id: DEXId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<TradingPair>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_enabled_pairs(&at, dex_id).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to list enabled pairs.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn is_pair_enabled(
        &self,
        dex_id: DEXId,
        base_asset_id: AssetId,
        target_asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.is_pair_enabled(&at, dex_id, base_asset_id, target_asset_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to query pair state.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }
}
