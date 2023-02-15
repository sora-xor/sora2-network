use super::*;
use crate::prelude::*;
use clap::*;
use common::{DAI, PSWAP, VAL, XOR, XST, XSTUSD};

#[derive(Args, Clone, Debug)]
pub(super) struct Command {
    #[clap(long)]
    mainnet_url: String,
}

impl Command {
    pub(super) async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        let sub = args.get_signed_substrate().await?;
        let mainnet = SubUnsignedClient::new(self.mainnet_url.clone()).await?;
        for (dex_id, base) in [(0, XOR), (1, XSTUSD)] {
            for asset_id in [XOR, PSWAP, DAI, XSTUSD, VAL, XST] {
                let reserves = mainnet
                    .api()
                    .storage()
                    .pool_xyk()
                    .reserves(false, &base, &asset_id, None)
                    .await?;
                let current_reserves = sub
                    .api()
                    .storage()
                    .pool_xyk()
                    .reserves(false, &base, &asset_id, None)
                    .await?;
                if reserves.0 <= 1
                    || reserves.1 <= 1
                    || current_reserves.0 > 1
                    || current_reserves.1 > 1
                {
                    continue;
                }
                info!("Add liquidity {}-{}: {:?}", base, asset_id, reserves);
                info!("Mint {}: {}", base, reserves.0 as i128 * 2);
                sub.api().tx().sudo().sudo(
                    false,
                    sub_runtime::runtime_types::framenode_runtime::Call::Currencies(
                        sub_runtime::runtime_types::orml_currencies::module::Call::update_balance {
                            who: sub.account_id(),
                            currency_id: base,
                            amount: reserves.0 as i128 * 2,
                        },
                    ),
                )?.sign_and_submit_then_watch_default(&sub).await?
                    .wait_for_in_block().await?
                    .wait_for_success().await?;
                info!("Mint {}: {}", asset_id, reserves.1 as i128 * 2);
                sub.api().tx().sudo().sudo(
                    false,
                    sub_runtime::runtime_types::framenode_runtime::Call::Currencies(
                        sub_runtime::runtime_types::orml_currencies::module::Call::update_balance {
                            who: sub.account_id(),
                            currency_id: asset_id,
                            amount: reserves.1 as i128 * 2,
                        },
                    ),
                )?.sign_and_submit_then_watch_default(&sub).await?
                    .wait_for_in_block().await?
                    .wait_for_success().await?;
                let tp = sub
                    .api()
                    .storage()
                    .trading_pair()
                    .enabled_sources(
                        false,
                        &dex_id,
                        &sub_runtime::runtime_types::common::primitives::TradingPair {
                            base_asset_id: base,
                            target_asset_id: asset_id,
                        },
                        None,
                    )
                    .await?;
                if tp.is_none() {
                    info!("Registering trading pair");
                    sub.api()
                        .tx()
                        .trading_pair()
                        .register(false, dex_id, base, asset_id)?
                        .sign_and_submit_then_watch_default(&sub)
                        .await?
                        .wait_for_in_block()
                        .await?
                        .wait_for_success()
                        .await?;
                }
                info!("Initializing pool");
                sub.api()
                    .tx()
                    .pool_xyk()
                    .initialize_pool(false, dex_id, base, asset_id)?
                    .sign_and_submit_then_watch_default(&sub)
                    .await?
                    .wait_for_in_block()
                    .await?
                    .wait_for_success()
                    .await?;
                info!("Deposit liquidity");
                sub.api()
                    .tx()
                    .pool_xyk()
                    .deposit_liquidity(false, dex_id, base, asset_id, reserves.0, reserves.1, 1, 1)?
                    .sign_and_submit_then_watch_default(&sub)
                    .await?
                    .wait_for_in_block()
                    .await?
                    .wait_for_success()
                    .await?;
            }
        }
        Ok(())
    }
}
