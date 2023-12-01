use bridge_types::{EVMChainId, H160};
use ethereum_gen::{BeefyLightClient, InboundChannel, OutboundChannel};
use ethers::providers::Middleware;

use crate::{ethereum::UnsignedClientInner, prelude::*};

pub const ETH_INBOUND_NONCE: &str = "bridge_eth_inbound_nonce";
pub const ETH_OUTBOUND_NONCE: &str = "bridge_eth_outbound_nonce";
pub const ETH_BLOCK_NUMBER: &str = "bridge_eth_block_number";
pub const ETH_BEEFY_CURRENT_SET_ID: &str = "bridge_eth_beefy_current_set_id";
pub const ETH_BEEFY_CURRENT_SET_LEN: &str = "bridge_eth_beefy_current_set_len";
pub const ETH_BEEFY_NEXT_SET_ID: &str = "bridge_eth_beefy_next_set_id";
pub const ETH_BEEFY_NEXT_SET_LEN: &str = "bridge_eth_beefy_next_set_len";
pub const ETH_BEEFY_LATEST_BLOCK: &str = "bridge_eth_beefy_next_set_len";

#[derive(Default)]
pub struct MetricsCollectorBuilder {
    client: Option<EthUnsignedClient>,
    beefy: Option<H160>,
    inbound: Option<H160>,
    outbound: Option<H160>,
}

impl MetricsCollectorBuilder {
    pub fn with_client(mut self, client: EthUnsignedClient) -> Self {
        self.client = Some(client);
        self
    }

    pub fn with_beefy(mut self, beefy: H160) -> Self {
        self.beefy = Some(beefy);
        self
    }

    pub fn with_inbound_channel(mut self, inbound: H160) -> Self {
        self.inbound = Some(inbound);
        self
    }

    pub fn with_outbound_channel(mut self, outbound: H160) -> Self {
        self.outbound = Some(outbound);
        self
    }

    pub async fn build(self) -> AnyResult<MetricsCollector> {
        let client = self
            .client
            .expect("client is not specified. It's developer mistake");
        let beefy = self.beefy.map(|x| BeefyLightClient::new(x, client.inner()));
        let inbound = self.inbound.map(|x| InboundChannel::new(x, client.inner()));
        let outbound = self
            .outbound
            .map(|x| OutboundChannel::new(x, client.inner()));
        let chain_id = client.get_chainid().await?;

        Ok(MetricsCollector {
            client,
            beefy,
            inbound,
            outbound,
            chain_id,
        })
    }
}

pub struct MetricsCollector {
    client: EthUnsignedClient,
    beefy: Option<BeefyLightClient<UnsignedClientInner>>,
    inbound: Option<InboundChannel<UnsignedClientInner>>,
    outbound: Option<OutboundChannel<UnsignedClientInner>>,
    chain_id: EVMChainId,
}

impl MetricsCollector {
    pub async fn run(self) -> AnyResult<()> {
        let labels = &[("chain_id", self.chain_id.to_string())];
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(7));
        loop {
            interval.tick().await;
            if let Some(contract) = &self.beefy {
                let (current_id, current_len, _root): (u128, u128, _) =
                    contract.current_validator_set().call().await?;
                let (next_id, next_len, _root): (u128, u128, _) =
                    contract.next_validator_set().call().await?;
                let latest_block: u64 = contract.latest_beefy_block().call().await?;

                metrics::absolute_counter!(ETH_BEEFY_CURRENT_SET_ID, current_id as u64, labels);
                metrics::absolute_counter!(ETH_BEEFY_CURRENT_SET_LEN, current_len as u64, labels);
                metrics::absolute_counter!(ETH_BEEFY_NEXT_SET_ID, next_id as u64, labels);
                metrics::absolute_counter!(ETH_BEEFY_NEXT_SET_LEN, next_len as u64, labels);
                metrics::absolute_counter!(ETH_BEEFY_LATEST_BLOCK, latest_block, labels);
            }
            if let Some(contract) = &self.inbound {
                let nonce: u64 = contract.batch_nonce().call().await?;
                metrics::absolute_counter!(ETH_INBOUND_NONCE, nonce, labels);
            }
            if let Some(contract) = &self.outbound {
                let nonce: u64 = contract.nonce().call().await?;
                metrics::absolute_counter!(ETH_OUTBOUND_NONCE, nonce, labels);
            }
            let block_number = self.client.get_block_number().await?.as_u64();
            metrics::absolute_counter!(ETH_BLOCK_NUMBER, block_number, labels);
        }
    }

    pub fn spawn(self) {
        tokio::spawn(self.run());
    }
}

pub fn describe_metrics() {
    metrics::describe_counter!(
        ETH_BLOCK_NUMBER,
        "Current block number in connected EVM client"
    );
    metrics::describe_counter!(ETH_INBOUND_NONCE, "EVM Inbound channel nonce");
    metrics::describe_counter!(ETH_OUTBOUND_NONCE, "EVM Outbound channel nonce");
    metrics::describe_counter!(
        ETH_BEEFY_CURRENT_SET_ID,
        "EVM Beefy light client current validator set id"
    );
    metrics::describe_counter!(
        ETH_BEEFY_CURRENT_SET_LEN,
        "EVM Beefy light client current validator set length"
    );
    metrics::describe_counter!(
        ETH_BEEFY_NEXT_SET_ID,
        "EVM Beefy light client next validator set id"
    );
    metrics::describe_counter!(
        ETH_BEEFY_NEXT_SET_LEN,
        "EVM Beefy light client next validator set length"
    );
    metrics::describe_counter!(
        ETH_BEEFY_LATEST_BLOCK,
        "EVM Beefy light client latest block"
    );
}
