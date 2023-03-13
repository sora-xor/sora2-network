use crate::mock::{adar, AccountId, Assets, DEXId, LiquidityProxy};
use crate::{BatchReceiverInfo, SwapBatchInfo};
use common::prelude::{QuoteAmount, SwapOutcome};
use common::{
    assert_approx_eq, balance, AssetId32, Balance, LiquidityProxyTrait, LiquiditySourceFilter,
    LiquiditySourceType, PredefinedAssetId, XOR,
};

#[inline]
pub fn mcbc_excluding_filter(dex: DEXId) -> LiquiditySourceFilter<DEXId, LiquiditySourceType> {
    LiquiditySourceFilter::with_forbidden(
        dex,
        [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
    )
}

pub fn check_swap_batch_executed_amount(
    swap_batches: Vec<SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>>,
) {
    swap_batches.into_iter().for_each(|batch| {
        let asset_id = batch.outcome_asset_id;
        batch.receivers.into_iter().for_each(|receiver_info| {
            let BatchReceiverInfo {
                account_id,
                target_amount,
            } = receiver_info;

            assert_approx_eq!(
                target_amount,
                Assets::free_balance(&asset_id, &account_id).unwrap(),
                balance!(0.00001)
            )
        })
    });
}

pub fn check_adar_commission(
    swap_batches: &Vec<SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>>,
    sources: Vec<LiquiditySourceType>,
) {
    let actual_input_amount = calculate_swap_batch_input_amount(swap_batches, sources);

    let adar_fee = LiquidityProxy::calculate_adar_commission(actual_input_amount).unwrap();

    assert_approx_eq!(
        Assets::free_balance(&XOR, &adar()).unwrap(),
        adar_fee,
        balance!(0.02)
    );
}

pub fn calculate_swap_batch_input_amount(
    swap_batches: &Vec<SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>>,
    sources: Vec<LiquiditySourceType>,
) -> Balance {
    let actual_input_amount: Balance = swap_batches
        .iter()
        .cloned()
        .map(|batch| {
            let SwapBatchInfo {
                outcome_asset_id,
                dex_id,
                ..
            } = batch.clone();
            batch
                .receivers
                .into_iter()
                .map(|receiver_info| {
                    let BatchReceiverInfo { target_amount, .. } = receiver_info;
                    let filter = LiquiditySourceFilter::new(dex_id, sources.clone(), false);
                    let SwapOutcome { amount, .. } = LiquidityProxy::quote(
                        dex_id,
                        &XOR,
                        &outcome_asset_id,
                        QuoteAmount::WithDesiredOutput {
                            desired_amount_out: target_amount,
                        },
                        filter,
                        true,
                    )
                    .expect("Expected to quote the outcome of batch swap");
                    amount
                })
                .sum::<Balance>()
        })
        .sum();
    actual_input_amount
}

pub fn calculate_swap_batch_input_amount_with_adar_commission(
    swap_batches: &Vec<SwapBatchInfo<AssetId32<PredefinedAssetId>, DEXId, AccountId>>,
    sources: Vec<LiquiditySourceType>,
) -> Balance {
    let amount_in = calculate_swap_batch_input_amount(swap_batches, sources);
    let adar_fee = LiquidityProxy::calculate_adar_commission(amount_in).unwrap();

    amount_in
        .checked_add(adar_fee)
        .expect("Expected to calculate swap batch input amount with included adar fee")
}
