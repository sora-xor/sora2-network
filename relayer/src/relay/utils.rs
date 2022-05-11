use std::sync::Arc;

use crate::prelude::*;
use crate::substrate::ApiInner;
use bridge_types::EthNetworkId;
use ethereum_gen::{BasicOutboundChannel, IncentivizedOutboundChannel};
use ethers::prelude::Middleware;

pub async fn incentivized_outbound_channel<M: Middleware>(
    chain_id: EthNetworkId,
    sub: &ApiInner,
    eth: Arc<M>,
) -> AnyResult<IncentivizedOutboundChannel<M>> {
    let incentivized_contract = sub
        .storage()
        .incentivized_inbound_channel()
        .channel_addresses(&chain_id, None)
        .await?
        .ok_or(anyhow::anyhow!("Channel is not registered"))?;
    let incentivized_contract = IncentivizedOutboundChannel::new(incentivized_contract, eth);
    Ok(incentivized_contract)
}

pub async fn basic_outbound_channel<M: Middleware>(
    chain_id: EthNetworkId,
    sub: &ApiInner,
    eth: Arc<M>,
) -> AnyResult<BasicOutboundChannel<M>> {
    let basic_contract = sub
        .storage()
        .basic_inbound_channel()
        .channel_addresses(&chain_id, None)
        .await?
        .ok_or(anyhow::anyhow!("Channel is not registered"))?;
    let basic_contract = BasicOutboundChannel::new(basic_contract, eth);
    Ok(basic_contract)
}
