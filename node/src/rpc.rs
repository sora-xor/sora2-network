#![warn(missing_docs)]

use common::TradingPair;
use framenode_runtime::{
    opaque::Block, AccountId, AssetId, AssetSymbol, Balance, BalancePrecision, BlockNumber, DEXId,
    FarmId, FarmInfo, FarmerInfo, FilterMode, Index, LiquiditySourceType, SwapVariant,
    TechAccountId,
};
pub use sc_rpc::DenyUnsafe;
pub use sc_rpc::SubscriptionTaskExecutor;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_transaction_pool::TransactionPool;
use std::sync::Arc;

/// JsonRpcHandler
pub type JsonRpcHandler = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Full client dependencies.
pub struct FullDeps<C, P> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
}

/// Instantiate full RPC extensions.
pub fn create_full<C, P>(deps: FullDeps<C, P>) -> JsonRpcHandler
where
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: dex_api_rpc::DEXRuntimeAPI<
        Block,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
    >,
    C::Api: dex_manager_rpc::DEXManagerRuntimeAPI<Block, DEXId>,
    C::Api: trading_pair_rpc::TradingPairRuntimeAPI<Block, DEXId, TradingPair<AssetId>, AssetId>,
    C::Api: assets_rpc::AssetsRuntimeAPI<
        Block,
        AccountId,
        AssetId,
        Balance,
        AssetSymbol,
        BalancePrecision,
    >,
    C::Api: liquidity_proxy_rpc::LiquidityProxyRuntimeAPI<
        Block,
        DEXId,
        AssetId,
        Balance,
        SwapVariant,
        LiquiditySourceType,
        FilterMode,
    >,
    C::Api: farming_rpc::FarmingRuntimeApi<
        Block,
        AccountId,
        FarmId,
        FarmInfo<AccountId, AssetId, BlockNumber>,
        FarmerInfo<AccountId, TechAccountId, BlockNumber>,
    >,
    C::Api: iroha_migration_rpc::IrohaMigrationRuntimeAPI<Block>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + Send + Sync + 'static,
{
    use assets_rpc::{AssetsAPI, AssetsClient};
    use dex_api_rpc::{DEX, DEXAPI};
    use dex_manager_rpc::{DEXManager, DEXManagerAPI};
    use farming_rpc::*;
    use iroha_migration_rpc::{IrohaMigrationAPI, IrohaMigrationClient};
    use liquidity_proxy_rpc::{LiquidityProxyAPI, LiquidityProxyClient};
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
    //use substrate_frame_rpc_system::{FullSystem, SystemApi};
    use trading_pair_rpc::{TradingPairAPI, TradingPairClient};
    let mut io = jsonrpc_core::IoHandler::default();
    let FullDeps {
        client,
        pool: _,
        deny_unsafe: _,
    } = deps;
    /*
    io.extend_with(SystemApi::to_delegate(FullSystem::new(
        client.clone(),
        pool,
        deny_unsafe,
    )));
    */
    io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
        client.clone(),
    )));
    io.extend_with(DEXAPI::to_delegate(DEX::new(client.clone())));
    io.extend_with(DEXManagerAPI::to_delegate(DEXManager::new(client.clone())));
    io.extend_with(TradingPairAPI::to_delegate(TradingPairClient::new(
        client.clone(),
    )));
    io.extend_with(AssetsAPI::to_delegate(AssetsClient::new(client.clone())));
    io.extend_with(LiquidityProxyAPI::to_delegate(LiquidityProxyClient::new(
        client.clone(),
    )));
    io.extend_with(FarmingApi::to_delegate(FarmingRpc::new(client.clone())));
    io.extend_with(IrohaMigrationAPI::to_delegate(IrohaMigrationClient::new(
        client.clone(),
    )));
    io
}
