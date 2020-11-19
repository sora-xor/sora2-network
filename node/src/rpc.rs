#![warn(missing_docs)]

use common::TradingPair;
use parachain_runtime::{opaque::Block, AssetId, Balance, DEXId, LiquiditySourceType, SwapVariant};
pub use sc_rpc::DenyUnsafe;
pub use sc_rpc::SubscriptionTaskExecutor;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_transaction_pool::TransactionPool;
use std::sync::Arc; // not reexported in parachain_runtime due to name clash

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
    C::Api: dex_api_rpc::DEXRuntimeAPI<
        Block,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
    >,
    C::Api: dex_manager_rpc::DEXManagerRuntimeAPI<Block, DEXId>,
    C::Api: template_rpc::TemplateRuntimeAPI<Block, Balance>,
    C::Api: trading_pair_rpc::TradingPairRuntimeAPI<Block, DEXId, TradingPair<AssetId>, AssetId>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + Send + Sync + 'static,
{
    use dex_api_rpc::{DEX, DEXAPI};
    use dex_manager_rpc::{DEXManager, DEXManagerAPI};
    use template_rpc::{Template, TemplateAPI};
    use trading_pair_rpc::{TradingPairAPI, TradingPairClient};
    let mut io = jsonrpc_core::IoHandler::default();
    let FullDeps { client, .. } = deps;
    io.extend_with(DEXAPI::to_delegate(DEX::new(client.clone())));
    io.extend_with(DEXManagerAPI::to_delegate(DEXManager::new(client.clone())));
    io.extend_with(TemplateAPI::to_delegate(Template::new(client.clone())));
    io.extend_with(TradingPairAPI::to_delegate(TradingPairClient::new(
        client.clone(),
    )));
    io
}
