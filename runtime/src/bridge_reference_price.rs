//! Bridge-limit valuation in the existing DAI reference units.

use crate::{AssetId, Balance, GetReferenceAssetId, PriceTools};
use common::{PriceToolsProvider, PriceVariant, ReferencePriceProvider};
use sp_runtime::DispatchError;

pub struct AveragedBridgeReferencePrice;

impl ReferencePriceProvider<AssetId, Balance> for AveragedBridgeReferencePrice {
    fn get_reference_price(asset_id: &AssetId) -> Result<Balance, DispatchError> {
        // Standard PriceTools uses the established 30-block history. Its Buy
        // variant values an outgoing asset conservatively in reference units.
        // Missing or warming history propagates an error; there is no spot fallback.
        <PriceTools as PriceToolsProvider<AssetId>>::get_average_price(
            asset_id,
            &GetReferenceAssetId::get(),
            PriceVariant::Buy,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{balance, OnPoolReservesChanged, DAI, PSWAP, XOR};
    use price_tools::{AggregatedPriceInfo, AVG_BLOCK_SPAN, DEFAULT_PARAMETERS};

    fn history(buy: Balance, sell: Balance, blocks: usize) -> AggregatedPriceInfo {
        let mut history = AggregatedPriceInfo::default();
        for (variant, price) in [(PriceVariant::Buy, buy), (PriceVariant::Sell, sell)] {
            for _ in 0..blocks {
                history
                    .price_mut_of(variant)
                    .incoming_spot_price(price, variant, &DEFAULT_PARAMETERS)
                    .expect("valid fixture price");
            }
        }
        history
    }

    fn quote(asset: AssetId) -> Result<Balance, DispatchError> {
        <<crate::Runtime as bridge_proxy::Config>::ReferencePriceProvider as ReferencePriceProvider<AssetId, Balance>>::get_reference_price(&asset)
    }

    #[test]
    fn reference_units_and_cross_asset_buy_valuation_are_preserved() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            // One DAI remains one DAI even when no pool history exists.
            assert_eq!(quote(DAI.into()), Ok(balance!(1)));
            price_tools::PriceInfos::<crate::Runtime>::insert(
                AssetId::from(DAI),
                history(balance!(10), balance!(8), AVG_BLOCK_SPAN),
            );
            price_tools::PriceInfos::<crate::Runtime>::insert(
                AssetId::from(PSWAP),
                history(balance!(4), balance!(2), AVG_BLOCK_SPAN),
            );
            assert_eq!(quote(XOR.into()), Ok(balance!(10)));
            // Buy PSWAP->XOR uses inverted Sell=2, then XOR->DAI Buy=10.
            assert_eq!(quote(PSWAP.into()), Ok(balance!(5)));
        });
    }

    #[test]
    fn unsupported_warming_and_failed_price_history_rejects_quotes() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            assert_eq!(
                quote(XOR.into()),
                Err(price_tools::Error::<crate::Runtime>::UnsupportedQuotePath.into())
            );
            price_tools::PriceInfos::<crate::Runtime>::insert(
                AssetId::from(DAI),
                history(balance!(10), balance!(8), AVG_BLOCK_SPAN - 1),
            );
            assert_eq!(
                quote(XOR.into()),
                Err(price_tools::Error::<crate::Runtime>::InsufficientSpotPriceData.into())
            );
            let mut prices = history(balance!(10), balance!(8), AVG_BLOCK_SPAN);
            for _ in 0..AVG_BLOCK_SPAN {
                prices
                    .price_mut_of(PriceVariant::Buy)
                    .incoming_spot_price_failure();
            }
            price_tools::PriceInfos::<crate::Runtime>::insert(AssetId::from(DAI), prices);
            assert_eq!(
                quote(XOR.into()),
                Err(price_tools::Error::<crate::Runtime>::InsufficientSpotPriceData.into())
            );
        });
    }

    #[test]
    fn same_block_reserve_notification_and_fast_prices_do_not_change_limit_quote() {
        sp_io::TestExternalities::new_empty().execute_with(|| {
            price_tools::PriceInfos::<crate::Runtime>::insert(
                AssetId::from(DAI),
                history(balance!(10), balance!(8), AVG_BLOCK_SPAN),
            );
            price_tools::FastPriceInfos::<crate::Runtime>::insert(
                AssetId::from(DAI),
                history(balance!(0.0001), balance!(0.0001), AVG_BLOCK_SPAN),
            );
            assert_eq!(quote(XOR.into()), Ok(balance!(10)));
            <PriceTools as OnPoolReservesChanged<AssetId>>::reserves_changed(&DAI.into());
            assert_eq!(quote(XOR.into()), Ok(balance!(10)));
        });
    }
}
