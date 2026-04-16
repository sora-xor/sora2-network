use codec::Decode;
use framenode_runtime::eth_bridge::{
    OUTGOING_APPROVAL_FAILURE_FAILED_SEND_SIGNED_TX, OUTGOING_APPROVAL_FAILURE_FAILED_SIGN,
    OUTGOING_APPROVAL_FAILURE_NO_LOCAL_PEER_KEY, OUTGOING_APPROVAL_FAILURE_SIDECHAIN_RPC_PREFLIGHT,
    STORAGE_BOOTSTRAP_READY_KEY, STORAGE_FAILED_PENDING_TRANSACTIONS_KEY,
    STORAGE_LOCAL_PEER_READY_KEY, STORAGE_LOCAL_SIGNING_KEY_READY_KEY, STORAGE_NETWORK_IDS_KEY,
    STORAGE_OUTGOING_APPROVAL_FAILURES_KEY, STORAGE_OUTGOING_PENDING_REQUESTS_KEY,
    STORAGE_OUTGOING_ZERO_APPROVAL_REQUESTS_KEY, STORAGE_PENDING_TRANSACTIONS_KEY,
    STORAGE_SIDECHAIN_RPC_CONFIGURED_KEY, STORAGE_SUBSTRATE_RPC_CONFIGURED_KEY,
    STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY,
};
use framenode_runtime::{eth_bridge::offchain::SignedTransactionData, opaque::Block, Runtime};
use prometheus_endpoint::{register, Gauge, Opts, PrometheusError, Registry, U64};
use sp_core::H256;
use sp_runtime::offchain::OffchainStorage;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const OUTGOING_APPROVAL_FAILURE_REASONS: [&str; 4] = [
    OUTGOING_APPROVAL_FAILURE_NO_LOCAL_PEER_KEY,
    OUTGOING_APPROVAL_FAILURE_FAILED_SIGN,
    OUTGOING_APPROVAL_FAILURE_FAILED_SEND_SIGNED_TX,
    OUTGOING_APPROVAL_FAILURE_SIDECHAIN_RPC_PREFLIGHT,
];

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
    pub bootstrap_ready: Gauge<U64>,
    pub local_signing_key_ready: Gauge<U64>,
    pub substrate_rpc_configured: Gauge<U64>,
    pub pending_transactions: Gauge<U64>,
    pub failed_pending_transactions: Gauge<U64>,
    pub local_peer_ready: BTreeMap<framenode_runtime::NetworkId, Gauge<U64>>,
    pub sidechain_rpc_configured: BTreeMap<framenode_runtime::NetworkId, Gauge<U64>>,
    pub outgoing_pending_requests: BTreeMap<framenode_runtime::NetworkId, Gauge<U64>>,
    pub outgoing_zero_approval_requests: BTreeMap<framenode_runtime::NetworkId, Gauge<U64>>,
    pub outgoing_approval_failures:
        BTreeMap<(framenode_runtime::NetworkId, &'static str), Gauge<U64>>,
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
        let mut local_peer_ready = BTreeMap::new();
        let mut sidechain_rpc_configured = BTreeMap::new();
        let mut outgoing_pending_requests = BTreeMap::new();
        let mut outgoing_zero_approval_requests = BTreeMap::new();
        let mut outgoing_approval_failures = BTreeMap::new();

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

                        let opts = Opts::new(
                            "eth_bridge_local_peer_ready",
                            "Whether the local node is currently eligible to act as a bridge peer for the network",
                        )
                        .const_label("network_id", format!("{}", network));
                        local_peer_ready
                            .insert(network, register(Gauge::with_opts(opts)?, registry)?);

                        let opts = Opts::new(
                            "eth_bridge_sidechain_rpc_configured",
                            "Whether sidechain RPC parameters are configured for the network",
                        )
                        .const_label("network_id", format!("{}", network));
                        sidechain_rpc_configured
                            .insert(network, register(Gauge::with_opts(opts)?, registry)?);

                        let opts = Opts::new(
                            "eth_bridge_outgoing_pending_requests",
                            "Number of pending outgoing legacy bridge requests for the network",
                        )
                        .const_label("network_id", format!("{}", network));
                        outgoing_pending_requests
                            .insert(network, register(Gauge::with_opts(opts)?, registry)?);

                        let opts = Opts::new(
                            "eth_bridge_outgoing_zero_approval_requests",
                            "Number of pending outgoing legacy bridge requests with zero approvals",
                        )
                        .const_label("network_id", format!("{}", network));
                        outgoing_zero_approval_requests
                            .insert(network, register(Gauge::with_opts(opts)?, registry)?);

                        for reason in OUTGOING_APPROVAL_FAILURE_REASONS {
                            let opts = Opts::new(
                                "eth_bridge_outgoing_approval_failure_total",
                                "Total outgoing approval failures recorded by reason",
                            )
                            .const_label("network_id", format!("{}", network))
                            .const_label("reason", reason.to_string());
                            outgoing_approval_failures.insert(
                                (network, reason),
                                register(Gauge::with_opts(opts)?, registry)?,
                            );
                        }
                    }
                    Ok(())
                },
            )?;
        }

        Ok(Self {
            bootstrap_ready: register(
                Gauge::new(
                    "eth_bridge_bootstrap_ready",
                    "Whether local bridge bootstrap completed successfully",
                )?,
                registry,
            )?,
            local_signing_key_ready: register(
                Gauge::new(
                    "eth_bridge_local_signing_key_ready",
                    "Whether a local ethereum bridge signing keypair is available",
                )?,
                registry,
            )?,
            substrate_rpc_configured: register(
                Gauge::new(
                    "eth_bridge_substrate_rpc_configured",
                    "Whether local substrate RPC is configured for bridge offchain workers",
                )?,
                registry,
            )?,
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
            local_peer_ready,
            sidechain_rpc_configured,
            outgoing_pending_requests,
            outgoing_zero_approval_requests,
            outgoing_approval_failures,
            ethereum_from_height,
            ethereum_height,
            period,
            backend,
        })
    }

    pub async fn run(self) {
        loop {
            if let Some(mut storage) = self.backend.offchain_storage() {
                self.bootstrap_ready.set(
                    get_offchain_value(&storage, STORAGE_BOOTSTRAP_READY_KEY, "bootstrap ready")
                        .unwrap_or(0),
                );
                self.local_signing_key_ready.set(
                    get_offchain_value(
                        &storage,
                        STORAGE_LOCAL_SIGNING_KEY_READY_KEY,
                        "local signing key ready",
                    )
                    .unwrap_or(0),
                );
                self.substrate_rpc_configured.set(
                    get_offchain_value(
                        &storage,
                        STORAGE_SUBSTRATE_RPC_CONFIGURED_KEY,
                        "substrate rpc configured",
                    )
                    .unwrap_or(0),
                );

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

                for (network, gauge) in self.local_peer_ready.iter() {
                    let key = format!("{}-{:?}", STORAGE_LOCAL_PEER_READY_KEY, network);
                    gauge.set(
                        get_offchain_value(&storage, key.as_bytes(), "local peer ready")
                            .unwrap_or(0),
                    );
                }

                for (network, gauge) in self.sidechain_rpc_configured.iter() {
                    let key = format!("{}-{:?}", STORAGE_SIDECHAIN_RPC_CONFIGURED_KEY, network);
                    gauge.set(
                        get_offchain_value(&storage, key.as_bytes(), "sidechain rpc configured")
                            .unwrap_or(0),
                    );
                }

                for (network, gauge) in self.outgoing_pending_requests.iter() {
                    let key = format!("{}-{:?}", STORAGE_OUTGOING_PENDING_REQUESTS_KEY, network);
                    gauge.set(
                        get_offchain_value(&storage, key.as_bytes(), "outgoing pending requests")
                            .unwrap_or(0),
                    );
                }

                for (network, gauge) in self.outgoing_zero_approval_requests.iter() {
                    let key = format!(
                        "{}-{:?}",
                        STORAGE_OUTGOING_ZERO_APPROVAL_REQUESTS_KEY, network
                    );
                    gauge.set(
                        get_offchain_value(
                            &storage,
                            key.as_bytes(),
                            "outgoing zero approval requests",
                        )
                        .unwrap_or(0),
                    );
                }

                for ((network, reason), gauge) in self.outgoing_approval_failures.iter() {
                    let key = format!(
                        "{}-{:?}-{}",
                        STORAGE_OUTGOING_APPROVAL_FAILURES_KEY, network, reason
                    );
                    gauge.set(
                        get_offchain_value(
                            &storage,
                            key.as_bytes(),
                            "outgoing approval failure total",
                        )
                        .unwrap_or(0),
                    );
                }

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
