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
pub use assets_runtime_api::AssetsAPI as AssetsRuntimeAPI;
use assets_runtime_api::{AssetInfo, BalanceInfo};

#[rpc]
pub trait AssetsAPI<
    BlockHash,
    AccountId,
    AssetId,
    Balance,
    OptionBalanceInfo,
    OptionAssetInfo,
    VecAssetInfo,
    VecAssetId,
>
{
    #[rpc(name = "assets_freeBalance")]
    fn free_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<OptionBalanceInfo>;

    #[rpc(name = "assets_totalBalance")]
    fn total_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<BlockHash>,
    ) -> Result<OptionBalanceInfo>;

    #[rpc(name = "assets_listAssetIds")]
    fn list_asset_ids(&self, at: Option<BlockHash>) -> Result<VecAssetId>;

    #[rpc(name = "assets_listAssetInfos")]
    fn list_asset_infos(&self, at: Option<BlockHash>) -> Result<VecAssetInfo>;

    #[rpc(name = "assets_getAssetInfo")]
    fn get_asset_info(&self, asset_id: AssetId, at: Option<BlockHash>) -> Result<OptionAssetInfo>;
}

pub struct AssetsClient<C, B> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<B>,
}

impl<C, B> AssetsClient<C, B> {
    /// Construct default `Template`.
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AccountId, AssetId, Balance, AssetSymbol, Precision>
    AssetsAPI<
        <Block as BlockT>::Hash,
        AccountId,
        AssetId,
        Balance,
        Option<BalanceInfo<Balance>>,
        Option<AssetInfo<AssetId, AssetSymbol, Precision>>,
        Vec<AssetInfo<AssetId, AssetSymbol, Precision>>,
        Vec<AssetId>,
    > for AssetsClient<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block>,
    C::Api: AssetsRuntimeAPI<Block, AccountId, AssetId, Balance, AssetSymbol, Precision>,
    AccountId: Codec,
    AssetId: Codec,
    Balance: Codec + MaybeFromStr + MaybeDisplay,
    AssetSymbol: Codec + MaybeFromStr + MaybeDisplay,
    Precision: Codec + MaybeFromStr + MaybeDisplay,
{
    fn free_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.free_balance(&at, account_id, asset_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get free balance.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn total_balance(
        &self,
        account_id: AccountId,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<BalanceInfo<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.total_balance(&at, account_id, asset_id)
            .map_err(|e| RpcError {
                code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
                message: "Unable to get total balance.".into(),
                data: Some(format!("{:?}", e).into()),
            })
    }

    fn list_asset_ids(&self, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<AssetId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_asset_ids(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to list registered Asset Ids.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn list_asset_infos(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<AssetInfo<AssetId, AssetSymbol, Precision>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.list_asset_infos(&at).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to list registered Asset Infos.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_asset_info(
        &self,
        asset_id: AssetId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<AssetInfo<AssetId, AssetSymbol, Precision>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or(
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash,
        ));
        api.get_asset_info(&at, asset_id).map_err(|e| RpcError {
            code: ErrorCode::ServerError(InvokeRPCError::RuntimeError.into()),
            message: "Unable to get Asset Info.".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
