use codec::Decode;
use framenode_runtime::eth_bridge::{
    STORAGE_FAILED_PENDING_TRANSACTIONS_KEY, STORAGE_NETWORK_IDS_KEY,
    STORAGE_PENDING_TRANSACTIONS_KEY, STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY,
};
use framenode_runtime::{eth_bridge::offchain::SignedTransactionData, opaque::Block, Runtime};
use prometheus_endpoint::{register, Gauge, Opts, PrometheusError, Registry, U64};
use sp_core::H256;
use sp_runtime::offchain::OffchainStorage;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn get_offchain_value<T, S>(storage: &S, key: &[u8], description: &str) -> Option<T>
where
    T: Decode,
    S: OffchainStorage,
{
    storage
        .get(sp_core::offchain::STORAGE_PREFIX, key)
        .and_then(|value| {
            T::decode(&mut &value[..])
                .map_err(|e| {
                    log::error!("Failed to decode {} offchain value: {:?}", description, e);
                })
                .ok()
        })
}

fn get_offchain_value_or_clear<T, S>(storage: &mut S, key: &[u8], description: &str) -> Option<T>
where
    T: Decode,
    S: OffchainStorage,
{
    storage
        .get(sp_core::offchain::STORAGE_PREFIX, key)
        .and_then(|value| match T::decode(&mut &value[..]) {
            Ok(value) => Some(value),
            Err(e) => {
                log::error!("Failed to decode {} offchain value: {:?}", description, e);
                storage.remove(sp_core::offchain::STORAGE_PREFIX, key);
                log::warn!(
                    "Cleared stale {} offchain value after decode failure",
                    description
                );
                None
            }
        })
}

pub struct Metrics<B> {
    pub backend: Arc<B>,
    pub period: std::time::Duration,
    pub pending_transactions: Gauge<U64>,
    pub failed_pending_transactions: Gauge<U64>,
    pub ethereum_from_height: BTreeMap<framenode_runtime::NetworkId, Gauge<U64>>,
    pub ethereum_height: BTreeMap<framenode_runtime::NetworkId, Gauge<U64>>,
    pub substrate_from_height: Gauge<U64>,
}

impl<B> Metrics<B>
where
    B: sc_client_api::Backend<Block> + Send + Sync + 'static,
    B::State: sc_client_api::StateBackend<sp_runtime::traits::HashingFor<Block>>,
{
    pub fn register(
        registry: &Registry,
        backend: Arc<B>,
        period: std::time::Duration,
    ) -> Result<Self, PrometheusError> {
        let mut ethereum_from_height = BTreeMap::new();
        let mut ethereum_height = BTreeMap::new();

        if let Some(storage) = backend.offchain_storage() {
            get_offchain_value(&storage, STORAGE_NETWORK_IDS_KEY, "network ids").map_or_else(
                || {
                    log::warn!("No network ids found in offchain storage. If you don't run bridge peer, this is fine");
                    Ok::<(), PrometheusError>(())
                },
                |networks: BTreeSet<framenode_runtime::NetworkId>| {
                    for network in networks {
                        let opts = Opts::new(
                            "eth_bridge_ethereum_to_handle_from_height",
                            "To handle from height for Ethereum network",
                        )
                        .const_label("network_id", format!("{}", network));
                        ethereum_from_height
                            .insert(network, register(Gauge::with_opts(opts)?, registry)?);
                        let opts =
                            Opts::new("eth_bridge_ethereum_height", "Height for Ethereum network")
                                .const_label("network_id", format!("{}", network));
                        ethereum_height.insert(network, register(Gauge::with_opts(opts)?, registry)?);
                    }
                    Ok(())
                },
            )?;
        }

        Ok(Self {
            pending_transactions: register(
                Gauge::new(
                    "eth_bridge_pending_transactions",
                    "Number of pending transactions",
                )?,
                registry,
            )?,
            failed_pending_transactions: register(
                Gauge::new(
                    "eth_bridge_failed_pending_transactions",
                    "Number of failed pending transactions",
                )?,
                registry,
            )?,
            substrate_from_height: register(
                Gauge::new(
                    "eth_bridge_substrate_from_height",
                    "To handle from height for Substrate network",
                )?,
                registry,
            )?,
            ethereum_from_height,
            ethereum_height,
            period,
            backend,
        })
    }

    pub async fn run(self) {
        loop {
            if let Some(mut storage) = self.backend.offchain_storage() {
                let pending_transactions = get_offchain_value_or_clear(
                    &mut storage,
                    STORAGE_PENDING_TRANSACTIONS_KEY,
                    "pending transactions",
                )
                .map(|value: BTreeMap<H256, SignedTransactionData<Runtime>>| value.len() as u64)
                .unwrap_or(0);
                self.pending_transactions.set(pending_transactions);

                let failed_pending_transactions = get_offchain_value_or_clear(
                    &mut storage,
                    STORAGE_FAILED_PENDING_TRANSACTIONS_KEY,
                    "failed pending transactions",
                )
                .map(|value: BTreeMap<H256, SignedTransactionData<Runtime>>| value.len() as u64)
                .unwrap_or(0);
                self.failed_pending_transactions
                    .set(failed_pending_transactions);

                get_offchain_value(
                    &storage,
                    STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY,
                    "handle from height for Substrate network",
                )
                .and_then(|value: framenode_runtime::BlockNumber| {
                    self.substrate_from_height.set(value as u64);
                    Some(())
                });

                for (network, gauge) in self.ethereum_from_height.iter() {
                    get_offchain_value(
                        &storage,
                        format!("eth-bridge-ocw::eth-to-handle-from-height-{:?}", network)
                            .as_bytes(),
                        &format!("handle from height for Ethereum network {:?}", network),
                    )
                    .and_then(|value: u64| {
                        gauge.set(value as u64);
                        Some(())
                    });
                }

                for (network, gauge) in self.ethereum_height.iter() {
                    get_offchain_value(
                        &storage,
                        &format!("eth-bridge-ocw::eth-height-{:?}", network).as_bytes(),
                        &format!("height for Ethereum network {:?}", network),
                    )
                    .and_then(|value: u64| {
                        gauge.set(value as u64);
                        Some(())
                    });
                }
            }
            futures_timer::Delay::new(self.period).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{get_offchain_value, get_offchain_value_or_clear};
    use codec::Encode;
    use sp_core::offchain::storage::InMemOffchainStorage;
    use sp_core::offchain::OffchainStorage;

    #[test]
    fn invalid_value_is_cleared_after_decode_failure() {
        let mut storage = InMemOffchainStorage::default();
        storage.set(
            sp_core::offchain::STORAGE_PREFIX,
            b"bad-key",
            &[0xff, 0x00, 0x01],
        );

        let value = get_offchain_value_or_clear::<u32, _>(&mut storage, b"bad-key", "bad key");

        assert!(value.is_none());
        assert_eq!(
            storage.get(sp_core::offchain::STORAGE_PREFIX, b"bad-key"),
            None
        );
    }

    #[test]
    fn valid_value_is_returned_without_clearing() {
        let mut storage = InMemOffchainStorage::default();
        storage.set(
            sp_core::offchain::STORAGE_PREFIX,
            b"good-key",
            &42u32.encode(),
        );

        let value = get_offchain_value::<u32, _>(&storage, b"good-key", "good key");

        assert_eq!(value, Some(42));
        assert_eq!(
            storage.get(sp_core::offchain::STORAGE_PREFIX, b"good-key"),
            Some(42u32.encode())
        );
    }
}
