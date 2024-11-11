use framenode_runtime::{
    opaque::{Block, BlockId},
    ResolveTime, Symbol,
};
use oracle_proxy_rpc::OracleProxyRuntimeApi;
use prometheus_endpoint::{register, Gauge, Opts, PrometheusError, Registry, U64};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Multiplier to convert rate last_update timestamp in seconds to millis
const MILLISECS_MULTIPLIER: u128 = 1_000;

#[derive(PartialEq)]
enum SymbolStatus {
    Outdated,
    UpToDate,
    InvalidTime,
}

pub struct Metrics<C> {
    pub client: Arc<C>,
    pub registry: Arc<Registry>,
    pub outdated_symbols: Gauge<U64>,
    pub invalid_symbols: Gauge<U64>,
    pub symbols_update_timestamps: BTreeMap<String, Gauge<U64>>,
    pub period: std::time::Duration,
}

impl<C> Metrics<C>
where
    Block: BlockT,
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: OracleProxyRuntimeApi<Block, Symbol, ResolveTime>,
{
    pub fn register(
        registry: Arc<Registry>,
        client: Arc<C>,
        period: std::time::Duration,
    ) -> Result<Self, PrometheusError> {
        let outdated_symbols = register(
            Gauge::new("data_feed_outdated_symbols", "Number of outdated symbols")?,
            &registry,
        )?;
        let invalid_symbols = register(
            Gauge::new(
                "data_feed_invalid_symbols",
                "Number of symbols with invalid timestamp",
            )?,
            &registry,
        )?;
        Ok(Self {
            client,
            registry,
            outdated_symbols,
            invalid_symbols,
            symbols_update_timestamps: BTreeMap::new(),
            period,
        })
    }

    async fn get_symbols(&self) -> Result<BTreeMap<String, (SymbolStatus, u64)>, String> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(self.client.info().best_hash);
        let enabled_symbols = api
            .list_enabled_symbols(&at)
            .map_err(|rpc_error| format!("RPC error: {:?}", rpc_error))?
            .map_err(|dispatch_error| format!("Dispatch error: {:?}", dispatch_error))?;

        let outdated_threshold = framenode_runtime::GetBandRateStalePeriod::get() as u128;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();

        let enabled_symbols_info = enabled_symbols
            .iter()
            .map(|(symbol, last_updated)| {
                let last_updated_timestamp = (*last_updated as u128) * MILLISECS_MULTIPLIER;
                let current_status = now.checked_sub(last_updated_timestamp).map_or_else(
                    || SymbolStatus::InvalidTime,
                    |current_period| {
                        if current_period > outdated_threshold {
                            SymbolStatus::Outdated
                        } else {
                            SymbolStatus::UpToDate
                        }
                    },
                );

                (symbol.to_string(), (current_status, *last_updated))
            })
            .collect::<BTreeMap<String, (SymbolStatus, u64)>>();

        Ok(enabled_symbols_info)
    }

    fn create_symbol_last_updated_gauge(
        &self,
        symbol: &str,
    ) -> Result<Gauge<U64>, PrometheusError> {
        let opts = Opts::new(
            "data_feed_symbol_last_updated",
            "Timestamp of symbol last update",
        )
        .const_label("symbol_name", symbol.to_string());

        let gauge = register(Gauge::<U64>::with_opts(opts)?, &self.registry)?;
        Ok(gauge)
    }

    async fn set_symbol_last_update(
        &mut self,
        symbol: &str,
        last_updated: u64,
    ) -> Result<(), String> {
        if !self.symbols_update_timestamps.contains_key(symbol) {
            let gauge = self
                .create_symbol_last_updated_gauge(symbol)
                .map_err(|e| format!("Prometheus gauge creation error: {:?}", e))?;
            self.symbols_update_timestamps
                .insert(symbol.to_string(), gauge);
        }
        self.symbols_update_timestamps
            .get_mut(symbol)
            .ok_or_else(|| {
                format!(
                    "data_feed_symbol_last_updated Gauge not found for symbol: {:?}",
                    symbol
                )
            })?
            .set(last_updated);
        Ok(())
    }

    pub async fn run(mut self) {
        loop {
            match self.get_symbols().await {
                Ok(enabled_symbols_info) => {
                    let outdated_symbols_count: u64 = enabled_symbols_info
                        .iter()
                        .filter(|(_, (status, _))| *status == SymbolStatus::Outdated)
                        .count() as u64;

                    let invalid_symbols_count: u64 = enabled_symbols_info
                        .iter()
                        .filter(|(_, (status, _))| *status == SymbolStatus::InvalidTime)
                        .count() as u64;

                    self.outdated_symbols.set(outdated_symbols_count);
                    self.invalid_symbols.set(invalid_symbols_count);

                    for (symbol, (_, last_updated)) in enabled_symbols_info {
                        if let Err(err) = self.set_symbol_last_update(&symbol, last_updated).await {
                            log::error!("Failed to set symbol update timestamp: {}", err);
                        }
                    }
                }
                Err(err) => {
                    log::error!("Failed to get oracle symbols: {:?}", err);
                }
            }

            futures_timer::Delay::new(self.period).await;
        }
    }
}
