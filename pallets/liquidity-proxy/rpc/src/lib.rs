use codec::Codec;
use common::{BalanceWrapper, InvokeRPCError};
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::generic::BlockId;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay, MaybeFromStr};
use std::sync::Arc;

// Custom imports
pub use liquidity_proxy_runtime_api::LiquidityProxyAPI as LiquidityProxyRuntimeAPI;
use liquidity_proxy_runtime_api::SwapOutcomeInfo;

#[rpc]
pub trait LiquidityProxyAPI<
    BlockHash,
    DEXId,
    AssetId,
    Balance,
    SwapVariant,
    LiquiditySourceType,
    FilterMode,
    OutputTy,
>
{
    #[rpc(name = "liquidityProxy_quote")]
    fn quote(
        &self,
        dex_id: DEXId,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        amount: BalanceWrapper,
        swap_variant: SwapVariant,
        selected_source_types: Vec<LiquiditySourceType>,
        filter_mode: FilterMode,
        at: Option<BlockHash>,
    ) -> Result<OutputTy>;

    #[rpc(name = "liquidityProxy_isPathAvailable")]
    fn is_path_available(
        &self,
        dex_id: DEXId,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<bool>;
}

pub struct LiquidityProxyClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> LiquidityProxyClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, DEXId, AssetId, Balance, SwapVariant, LiquiditySourceType, FilterMode>
    LiquidityProxyAPI<
        <Block as BlockT>::Hash,
        DEXId,
        AssetId,
        Balance,
        SwapVariant,
        LiquiditySourceType,
        FilterMode,
        Option<SwapOutcomeInfo<Balance, AssetId>>,
    > for LiquidityProxyClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: LiquidityProxyRuntimeAPI<
        Block,
        DEXId,
        AssetId,
        Balance,
        SwapVariant,
        LiquiditySourceType,
        FilterMode,
    >,
    DEXId: Codec,
    AssetId: Codec + MaybeFromStr + MaybeDisplay,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
    SwapVariant: Codec,
    LiquiditySourceType: Codec,
    FilterMode: Codec,
{
    fn quote(
        &self,
        dex_id: DEXId,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        amount: BalanceWrapper,
        swap_variant: SwapVariant,
        selected_source_types: Vec<LiquiditySourceType>,
        filter_mode: FilterMode,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<SwapOutcomeInfo<Balance, AssetId>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.quote(
            &at,
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            swap_variant,
            selected_source_types,
            filter_mode,
        )
        .map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to quote price.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn is_path_available(
        &self,
        dex_id: DEXId,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.is_path_available(&at, dex_id, input_asset_id, output_asset_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to query path availability.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }
}
