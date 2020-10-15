#![warn(missing_docs)]

use parachain_runtime::{opaque::Block, AssetId, DEXId, LiquiditySourceType};
pub use sc_rpc::DenyUnsafe;
pub use sc_rpc::SubscriptionTaskExecutor;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_transaction_pool::TransactionPool;
use std::sync::Arc;

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
    C::Api: dex_api_rpc::DEXRuntimeAPI<Block, AssetId, DEXId, sp_core::U256, LiquiditySourceType>,
    C::Api: dex_manager_rpc::DEXManagerRuntimeAPI<Block, DEXId>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + Send + Sync + 'static,
{
    use dex_api_rpc::{DEX, DEXAPI};
    use dex_manager_rpc::{DEXManager, DEXManagerAPI};
    let mut io = jsonrpc_core::IoHandler::default();
    let FullDeps { client, .. } = deps;
    io.extend_with(DEXAPI::to_delegate(DEX::new(client.clone())));
    io.extend_with(DEXManagerAPI::to_delegate(DEXManager::new(client.clone())));
    io
}
