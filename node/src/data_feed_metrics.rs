use framenode_runtime::{
    opaque::{Block, BlockId},
    ResolveTime, Symbol,
};
use oracle_proxy_rpc::OracleProxyRuntimeApi;
use prometheus_endpoint::{register, Gauge, PrometheusError, Registry, U64};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::error::Error;
use std::sync::Arc;

pub struct Metrics<C> {
    pub client: Arc<C>,
    pub outdated_symbols: Gauge<U64>,
    pub period: std::time::Duration,
}

impl<C> Metrics<C>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: OracleProxyRuntimeApi<Block, Symbol, ResolveTime>,
{
    pub fn register(
        registry: &Registry,
        client: Arc<C>,
        period: std::time::Duration,
    ) -> Result<Self, PrometheusError> {
        Ok(Self {
            client,
            outdated_symbols: register(
                Gauge::new("data_feed_outdated_symbols", "Number of outdated symbols")?,
                registry,
            )?,
            period,
        })
    }

    pub async fn check_outdated_symbols(&self) -> Result<u64, Box<dyn Error>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(self.client.info().best_hash);
        let enabled_symbols = api
            .list_enabled_symbols(&at)
            .map_err(|rpc_error| format!("RPC error: {:?}", rpc_error))?
            .map_err(|dispatch_error| format!("Dispatch error: {:?}", dispatch_error))?;

        let outdated_threshold: u128 = 300 * 1000; // 5 minutes in seconds
        let outdated_symbols = enabled_symbols
            .iter()
            .filter(|(_, last_updated)| {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("Time went backwards")
                    .as_millis();
                now.checked_sub(*last_updated as u128).map_or_else(
                    || {
                        log::error!("Symbol last_updated field is greater than current time");
                        false
                    },
                    |current_period| current_period > outdated_threshold,
                )
            })
            .count() as u64;

        Ok(outdated_symbols)
    }

    pub async fn run(self) {
        loop {
            match self.check_outdated_symbols().await {
                Ok(outdated_symbols_count) => {
                    self.outdated_symbols.set(outdated_symbols_count);
                }
                Err(err) => {
                    log::error!("Failed to check outdated symbols: {:?}", err);
                }
            }

            futures_timer::Delay::new(self.period).await;
        }
    }
}
