use codec::Codec;
use common::BalanceWrapper;
use common::InvokeRPCError;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

// Runtime API imports.
use dex_runtime_api::SwapOutcomeInfo;
pub use dex_runtime_api::DEXAPI as DEXRuntimeAPI;

#[rpc]
pub trait DEXAPI<BlockHash, AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant, SwapResponse>
{
    #[rpc(name = "dexApi_quote")]
    fn quote(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        amount: BalanceWrapper,
        swap_variant: SwapVariant,
        at: Option<BlockHash>,
    ) -> Result<SwapResponse>;

    #[rpc(name = "dexApi_canExchange")]
    fn can_exchange(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<bool>;

    #[rpc(name = "dexApi_listSupportedSources")]
    fn list_supported_sources(&self, at: Option<BlockHash>) -> Result<Vec<LiquiditySourceType>>;
}

pub struct DEX<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> DEX<C, B> {
    /// Construct default DEX as intermediary impl for rpc.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant>
    DEXAPI<
        <Block as BlockT>::Hash,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
        Option<SwapOutcomeInfo<Balance>>,
    > for DEX<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: DEXRuntimeAPI<Block, AssetId, DEXId, Balance, LiquiditySourceType, SwapVariant>,
    AssetId: Codec,
    DEXId: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
    SwapVariant: Codec,
    LiquiditySourceType: Codec,
{
    fn quote(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        amount: BalanceWrapper,
        swap_variant: SwapVariant,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<SwapOutcomeInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.quote(
            &at,
            dex_id,
            liquidity_source_type,
            input_asset_id,
            output_asset_id,
            amount,
            swap_variant,
        )
        .map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to quote price.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn can_exchange(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.can_exchange(
            &at,
            dex_id,
            liquidity_source_type,
            input_asset_id,
            output_asset_id,
        )
        .map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to query exchange capability of source.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn list_supported_sources(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<LiquiditySourceType>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_supported_sources(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to query supported liquidity source types.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
