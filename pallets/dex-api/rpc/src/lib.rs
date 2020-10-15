use codec::Codec;
use common::InvokeRPCError;
pub use dex_runtime_api::DEXAPI as DEXRuntimeAPI;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::U256;
use sp_rpc::number::NumberOrHex;
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use sp_std::convert::TryFrom;
use std::sync::Arc;

#[rpc]
pub trait DEXAPI<BlockHash, AssetId, DEXId, Balance, LiquiditySourceType>
where
    Balance: std::str::FromStr,
{
    #[rpc(name = "dex_getPriceWithDesiredInput")]
    fn get_price_with_desired_input(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        desired_input_amount: NumberOrHex,
        at: Option<BlockHash>,
    ) -> Result<Option<Balance>>;

    #[rpc(name = "dex_getPriceWithDesiredOutput")]
    fn get_price_with_desired_output(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        desired_output_amount: NumberOrHex,
        at: Option<BlockHash>,
    ) -> Result<Option<Balance>>;
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

impl<C, Block, AssetId, DEXId, Balance, LiquiditySourceType>
    DEXAPI<<Block as BlockT>::Hash, AssetId, DEXId, Balance, LiquiditySourceType> for DEX<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: DEXRuntimeAPI<Block, AssetId, DEXId, Balance, LiquiditySourceType>,
    AssetId: Codec,
    DEXId: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay + TryFrom<U256>,
    <Balance as TryFrom<U256>>::Error: sp_std::fmt::Debug,
    LiquiditySourceType: Codec,
{
    fn get_price_with_desired_input(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        desired_input_amount: NumberOrHex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        // TODO: U256 to U256 parsing, appropriate type for Balance needs to be derived
        let amount: Balance =
            TryFrom::try_from(desired_input_amount.into_u256()).map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Balance parsing error.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;
        api.get_price_with_desired_input(
            &at,
            dex_id,
            liquidity_source_type,
            input_asset_id,
            output_asset_id,
            amount,
        )
        .map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get price with desired input.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_price_with_desired_output(
        &self,
        dex_id: DEXId,
        liquidity_source_type: LiquiditySourceType,
        input_asset_id: AssetId,
        output_asset_id: AssetId,
        desired_output_amount: NumberOrHex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        // TODO: U256 to U256 parsing, appropriate type for Balance needs to be derived
        let amount: Balance =
            TryFrom::try_from(desired_output_amount.into_u256()).map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Balance parsing error.".into(),
                data: Some(format!("{:?}", e).into()),
            })?;
        api.get_price_with_desired_output(
            &at,
            dex_id,
            liquidity_source_type,
            input_asset_id,
            output_asset_id,
            amount,
        )
        .map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get price with desired output.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
