use std::sync::Arc;

use crate::prelude::*;
use crate::substrate::ApiInner;
use ethereum_gen::{BasicOutboundChannel, IncentivizedOutboundChannel};
use ethers::prelude::Middleware;

pub async fn incentivized_outbound_channel<M: Middleware>(
    sub: &ApiInner,
    eth: Arc<M>,
) -> AnyResult<IncentivizedOutboundChannel<M>> {
    let incentivized_contract = sub
        .storage()
        .incentivized_inbound_channel()
        .source_channel(None)
        .await?;
    let incentivized_contract = IncentivizedOutboundChannel::new(incentivized_contract, eth);
    Ok(incentivized_contract)
}

pub async fn basic_outbound_channel<M: Middleware>(
    sub: &ApiInner,
    eth: Arc<M>,
) -> AnyResult<BasicOutboundChannel<M>> {
    let basic_contract = sub
        .storage()
        .basic_inbound_channel()
        .source_channel(None)
        .await?;
    let basic_contract = BasicOutboundChannel::new(basic_contract, eth);
    Ok(basic_contract)
}
