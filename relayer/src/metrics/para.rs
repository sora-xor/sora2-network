use bridge_types::GenericNetworkId;
use sp_core::H256;
use subxt::{
    metadata::DecodeWithMetadata,
    storage::{address::Yes, StorageAddress},
};

use crate::prelude::*;

pub const PARA_BEEFY_LIGHT_CLIENT_CURRENT_ID: &str = "bridge_para_beefy_light_client_current_id";
pub const PARA_BEEFY_LIGHT_CLIENT_CURRENT_LEN: &str = "bridge_para_beefy_light_client_current_len";
pub const PARA_BEEFY_LIGHT_CLIENT_NEXT_ID: &str = "bridge_para_beefy_light_client_next_id";
pub const PARA_BEEFY_LIGHT_CLIENT_NEXT_LEN: &str = "bridge_para_beefy_light_client_next_len";
pub const PARA_BEEFY_LIGHT_CLIENT_LATEST_BLOCK: &str =
    "bridge_para_beefy_light_client_latest_block";
pub const PARA_SUBSTRATE_INBOUND_NONCE: &str = "bridge_para_substrate_inbound_nonce";
pub const PARA_SUBSTRATE_OUTBOUND_NONCE: &str = "bridge_para_substrate_outbound_nonce";
pub const PARA_BEEFY_CURRENT_ID: &str = "bridge_para_beefy_current_id";
pub const PARA_BEEFY_CURRENT_LEN: &str = "bridge_para_beefy_current_len";
pub const PARA_BEEFY_NEXT_ID: &str = "bridge_para_beefy_next_id";
pub const PARA_BEEFY_NEXT_LEN: &str = "bridge_para_beefy_next_len";

#[derive(Default)]
pub struct MetricsCollectorBuilder {
    client: Option<SubUnsignedClient<ParachainConfig>>,
    network_id: Option<GenericNetworkId>,
}

impl MetricsCollectorBuilder {
    pub fn with_client(mut self, client: SubUnsignedClient<ParachainConfig>) -> Self {
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
    client: SubUnsignedClient<ParachainConfig>,
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
        let mut interval = tokio::time::interval(ParachainConfig::average_block_time());
        loop {
            interval.tick().await;
            let Ok(block_hash) = self.client.block_hash(()).await else {
                continue;
            };

            let address = parachain_runtime::storage().beefy_mmr().beefy_authorities();
            self.update_metric(&address, block_hash, |vset| {
                metrics::gauge!(PARA_BEEFY_CURRENT_ID, vset.id as f64);
                metrics::gauge!(PARA_BEEFY_CURRENT_LEN, vset.len as f64);
            })
            .await;

            let address = parachain_runtime::storage()
                .beefy_mmr()
                .beefy_next_authorities();

            self.update_metric(&address, block_hash, |vset| {
                metrics::gauge!(PARA_BEEFY_NEXT_ID, vset.id as f64);
                metrics::gauge!(PARA_BEEFY_NEXT_LEN, vset.len as f64);
            })
            .await;

            match self.network_id {
                GenericNetworkId::EVM(_) => {
                    unimplemented!("EVM bridge is not supported in parachain");
                }
                GenericNetworkId::Sub(network_id) => {
                    let labels = &[("network_id", format!("{:?}", network_id))];

                    let address = parachain_runtime::storage()
                        .beefy_light_client()
                        .current_validator_set(network_id);
                    self.update_metric(&address, block_hash, |vset| {
                        metrics::gauge!(PARA_BEEFY_LIGHT_CLIENT_CURRENT_ID, vset.id as f64, labels);
                        metrics::gauge!(
                            PARA_BEEFY_LIGHT_CLIENT_CURRENT_LEN,
                            vset.len as f64,
                            labels
                        );
                    })
                    .await;

                    let address = parachain_runtime::storage()
                        .beefy_light_client()
                        .next_validator_set(network_id);
                    self.update_metric(&address, block_hash, |vset| {
                        metrics::gauge!(PARA_BEEFY_LIGHT_CLIENT_NEXT_ID, vset.id as f64, labels);
                        metrics::gauge!(PARA_BEEFY_LIGHT_CLIENT_NEXT_LEN, vset.len as f64, labels);
                    })
                    .await;

                    let address = parachain_runtime::storage()
                        .beefy_light_client()
                        .latest_beefy_block(network_id);
                    self.update_metric(&address, block_hash, |block| {
                        metrics::absolute_counter!(
                            PARA_BEEFY_LIGHT_CLIENT_LATEST_BLOCK,
                            block,
                            labels
                        );
                    })
                    .await;

                    let address = parachain_runtime::storage()
                        .substrate_bridge_inbound_channel()
                        .channel_nonces(network_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(PARA_SUBSTRATE_INBOUND_NONCE, nonce, labels);
                    })
                    .await;

                    let address = parachain_runtime::storage()
                        .substrate_bridge_outbound_channel()
                        .channel_nonces(network_id);
                    self.update_metric(&address, block_hash, |nonce| {
                        metrics::absolute_counter!(PARA_SUBSTRATE_OUTBOUND_NONCE, nonce, labels);
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
    metrics::describe_gauge!(
        PARA_BEEFY_LIGHT_CLIENT_CURRENT_ID,
        "Current validator set id in parachain BEEFY light client"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_LIGHT_CLIENT_CURRENT_LEN,
        "Current validator set length in parachain BEEFY light client"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_LIGHT_CLIENT_NEXT_ID,
        "Next validator set id in parachain BEEFY light client"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_LIGHT_CLIENT_NEXT_LEN,
        "Next validator set length in parachain BEEFY light client"
    );
    metrics::describe_counter!(
        PARA_BEEFY_LIGHT_CLIENT_LATEST_BLOCK,
        "Latest sent block in parachain BEEFY light client"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_CURRENT_ID,
        "Current validator set id in parachain BEEFY"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_CURRENT_LEN,
        "Current validator set length in parachain BEEFY"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_NEXT_ID,
        "Next validator set id in parachain BEEFY"
    );
    metrics::describe_gauge!(
        PARA_BEEFY_NEXT_LEN,
        "Next validator set length in parachain BEEFY"
    );
    metrics::describe_counter!(
        PARA_SUBSTRATE_INBOUND_NONCE,
        "Parachain substrate bridge inbound channel nonce"
    );
    metrics::describe_counter!(
        PARA_SUBSTRATE_OUTBOUND_NONCE,
        "Parachain substrate bridge outbound channel nonce"
    );
}
