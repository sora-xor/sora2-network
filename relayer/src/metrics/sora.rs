use bridge_types::GenericNetworkId;
use sp_core::H256;
use subxt::{
    metadata::DecodeWithMetadata,
    storage::{address::Yes, StorageAddress},
};

use crate::prelude::*;

pub const SUB_ETHASH_LIGHT_CLIENT_BEST: &str = "bridge_sub_ethash_light_client_best";
pub const SUB_ETHASH_LIGHT_CLIENT_FINALIZED: &str = "bridge_sub_ethash_light_client_finalized";
pub const SUB_INBOUND_NONCE: &str = "bridge_sub_inbound_nonce";
pub const SUB_INBOUND_DISPATCHED_NONCE: &str = "bridge_sub_inbound_dispatched_nonce";
pub const SUB_OUTBOUND_NONCE: &str = "bridge_sub_outbound_nonce";
pub const SUB_BEEFY_LIGHT_CLIENT_CURRENT_ID: &str = "bridge_sub_beefy_light_client_current_id";
pub const SUB_BEEFY_LIGHT_CLIENT_CURRENT_LEN: &str = "bridge_sub_beefy_light_client_current_len";
pub const SUB_BEEFY_LIGHT_CLIENT_NEXT_ID: &str = "bridge_sub_beefy_light_client_next_id";
pub const SUB_BEEFY_LIGHT_CLIENT_NEXT_LEN: &str = "bridge_sub_beefy_light_client_next_len";
pub const SUB_BEEFY_LIGHT_CLIENT_LATEST_BLOCK: &str = "bridge_sub_beefy_light_client_latest_block";
pub const SUB_BEEFY_CURRENT_ID: &str = "bridge_sub_beefy_current_id";
pub const SUB_BEEFY_CURRENT_LEN: &str = "bridge_sub_beefy_current_len";
pub const SUB_BEEFY_NEXT_ID: &str = "bridge_sub_beefy_next_id";
pub const SUB_BEEFY_NEXT_LEN: &str = "bridge_sub_beefy_next_len";
pub const SUB_SUBSTRATE_INBOUND_NONCE: &str = "bridge_sub_substrate_inbound_nonce";
pub const SUB_SUBSTRATE_OUTBOUND_NONCE: &str = "bridge_sub_substrate_outbound_nonce";

#[derive(Default)]
pub struct MetricsCollectorBuilder {
    client: Option<SubUnsignedClient<MainnetConfig>>,
    network_id: Option<GenericNetworkId>,
}

impl MetricsCollectorBuilder {
    pub fn with_client(mut self, client: SubUnsignedClient<MainnetConfig>) -> Self {
        self.client = Some(client);
        self
    }

    pub fn with_network_id(mut self, network_id: GenericNetworkId) -> Self {
        self.network_id = Some(network_id);
        self
    }

    pub async fn build(self) -> AnyResult<MetricsCollector> {
        let client = self
            .client
            .expect("client is not specified. It's developer mistake");
        let network_id = self
            .network_id
            .expect("client is not specified. It's developer mistake");

        Ok(MetricsCollector { client, network_id })
    }
}

pub struct MetricsCollector {
    client: SubUnsignedClient<MainnetConfig>,
    network_id: GenericNetworkId,
}

impl MetricsCollector {
    pub async fn update_metric<S, F>(&self, address: &S, block_hash: H256, update_fn: F)
    where
        S: StorageAddress<IsFetchable = Yes>,
        F: Fn(<S::Target as DecodeWithMetadata>::Target),
    {
        match self.client.storage_fetch(address, block_hash).await {
            Ok(Some(value)) => {
                update_fn(value);
            }
            Err(err) => {
                log::warn!("Failed to get value from storage: {err}");
            }
            Ok(None) => {}
        }
    }

    pub async fn run(self) -> AnyResult<()> {
        let mut interval = tokio::time::interval(MainnetConfig::average_block_time());
        loop {
            interval.tick().await;
            let Ok(block_hash) = self.client.block_hash(()).await else {
                continue;
            };

            let address = mainnet_runtime::storage().mmr_leaf().beefy_authorities();
            self.update_metric(&address, block_hash, |vset| {
                metrics::gauge!(SUB_BEEFY_CURRENT_ID, vset.id as f64);
                metrics::gauge!(SUB_BEEFY_CURRENT_LEN, vset.len as f64);
            })
            .await;

            let address = mainnet_runtime::storage()
                .mmr_leaf()
                .beefy_next_authorities();
            self.update_metric(&address, block_hash, |vset| {
                metrics::gauge!(SUB_BEEFY_NEXT_ID, vset.id as f64);
                metrics::gauge!(SUB_BEEFY_NEXT_LEN, vset.len as f64);
            })
            .await;

            match self.network_id {
                GenericNetworkId::EVM(chain_id) => {
                    let labels = &[("chain_id", chain_id.to_string())];

                    let address = mainnet_runtime::storage()
                        .ethereum_light_client()
                        .best_block(chain_id);
                    self.update_metric(&address, block_hash, |(id, _)| {
                        metrics::absolute_counter!(SUB_ETHASH_LIGHT_CLIENT_BEST, id.number, labels);
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .ethereum_light_client()
                        .finalized_block(chain_id);
                    self.update_metric(&address, block_hash, |id| {
                        metrics::absolute_counter!(
                            SUB_ETHASH_LIGHT_CLIENT_FINALIZED,
                            id.number,
                            labels
                        );
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .bridge_inbound_channel()
                        .channel_nonces(chain_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(SUB_INBOUND_NONCE, nonce, labels);
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .bridge_inbound_channel()
                        .inbound_channel_nonces(chain_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(SUB_INBOUND_DISPATCHED_NONCE, nonce, labels);
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .bridge_outbound_channel()
                        .channel_nonces(chain_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(SUB_OUTBOUND_NONCE, nonce, labels);
                    })
                    .await;
                }
                GenericNetworkId::Sub(network_id) => {
                    let labels = &[("network_id", format!("{:?}", network_id))];

                    let address = mainnet_runtime::storage()
                        .beefy_light_client()
                        .current_validator_set(network_id);
                    self.update_metric(&address, block_hash, |vset| {
                        metrics::gauge!(SUB_BEEFY_LIGHT_CLIENT_CURRENT_ID, vset.id as f64, labels);
                        metrics::gauge!(
                            SUB_BEEFY_LIGHT_CLIENT_CURRENT_LEN,
                            vset.len as f64,
                            labels
                        );
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .beefy_light_client()
                        .next_validator_set(network_id);
                    self.update_metric(&address, block_hash, |vset| {
                        metrics::gauge!(SUB_BEEFY_LIGHT_CLIENT_NEXT_ID, vset.id as f64, labels);
                        metrics::gauge!(SUB_BEEFY_LIGHT_CLIENT_NEXT_LEN, vset.len as f64, labels);
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .beefy_light_client()
                        .latest_beefy_block(network_id);
                    self.update_metric(&address, block_hash, |block| {
                        metrics::absolute_counter!(
                            SUB_BEEFY_LIGHT_CLIENT_LATEST_BLOCK,
                            block,
                            labels
                        );
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .substrate_bridge_inbound_channel()
                        .channel_nonces(network_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(SUB_SUBSTRATE_INBOUND_NONCE, nonce, labels);
                    })
                    .await;

                    let address = mainnet_runtime::storage()
                        .substrate_bridge_outbound_channel()
                        .channel_nonces(network_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(SUB_SUBSTRATE_OUTBOUND_NONCE, nonce, labels);
                    })
                    .await;
                }
                GenericNetworkId::EVMLegacy(_) => {
                    unimplemented!("HASHI bridge metrics is not supported")
                }
            }
        }
    }

    pub fn spawn(self) {
        tokio::spawn(self.run());
    }
}

pub fn describe_metrics() {
    metrics::describe_counter!(
        SUB_ETHASH_LIGHT_CLIENT_BEST,
        "Ethash light client best block"
    );
    metrics::describe_counter!(
        SUB_ETHASH_LIGHT_CLIENT_FINALIZED,
        "Ethash light client finalized block"
    );
    metrics::describe_counter!(SUB_INBOUND_NONCE, "EVM bridge inbound channel nonce");
    metrics::describe_counter!(
        SUB_INBOUND_DISPATCHED_NONCE,
        "EVM bridge inbound channel dispatched nonce"
    );
    metrics::describe_counter!(SUB_OUTBOUND_NONCE, "EVM bridge outbound channel nonce");
    metrics::describe_gauge!(
        SUB_BEEFY_LIGHT_CLIENT_CURRENT_ID,
        "Current validator set id in SORA BEEFY light client"
    );
    metrics::describe_gauge!(
        SUB_BEEFY_LIGHT_CLIENT_CURRENT_LEN,
        "Current validator set length in SORA BEEFY light client"
    );
    metrics::describe_gauge!(
        SUB_BEEFY_LIGHT_CLIENT_NEXT_ID,
        "Next validator set id in SORA BEEFY light client"
    );
    metrics::describe_gauge!(
        SUB_BEEFY_LIGHT_CLIENT_NEXT_LEN,
        "Next validator set length in SORA BEEFY light client"
    );
    metrics::describe_counter!(
        SUB_BEEFY_LIGHT_CLIENT_LATEST_BLOCK,
        "Latest sent block in SORA BEEFY light client"
    );
    metrics::describe_gauge!(
        SUB_BEEFY_CURRENT_ID,
        "Current validator set id in SORA BEEFY"
    );
    metrics::describe_gauge!(
        SUB_BEEFY_CURRENT_LEN,
        "Current validator set length in SORA BEEFY"
    );
    metrics::describe_gauge!(SUB_BEEFY_NEXT_ID, "Next validator set id in SORA BEEFY");
    metrics::describe_gauge!(
        SUB_BEEFY_NEXT_LEN,
        "Next validator set length in SORA BEEFY"
    );
    metrics::describe_counter!(
        SUB_SUBSTRATE_INBOUND_NONCE,
        "SORA substrate bridge inbound channel nonce"
    );
    metrics::describe_counter!(
        SUB_SUBSTRATE_OUTBOUND_NONCE,
        "SORA substrate bridge outbound channel nonce"
    );
}
