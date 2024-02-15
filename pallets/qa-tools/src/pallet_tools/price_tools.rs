use crate::{Config, Error};
use common::prelude::BalanceUnit;
use common::{balance, fixed_wrapper, Balance, PriceToolsProvider, PriceVariant, XOR};
use frame_support::dispatch::{DispatchError, DispatchResult};
use sp_arithmetic::traits::{CheckedDiv, One};

/// Set price for the asset in `price_tools` ignoring internal limits on change of the price.
pub(crate) fn set_price<T: Config>(
    asset_id: &T::AssetId,
    price: Balance,
    variant: PriceVariant,
) -> DispatchResult {
    let _ = price_tools::Pallet::<T>::register_asset(asset_id);

    // feed failures in order to ignore the limits
    for _ in 0..price_tools::AVG_BLOCK_SPAN {
        price_tools::Pallet::<T>::incoming_spot_price_failure(asset_id, variant);
    }

    for _ in 0..price_tools::AVG_BLOCK_SPAN + 1 {
        price_tools::Pallet::<T>::incoming_spot_price(asset_id, price, variant)?;
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AssetPrices {
    pub buy: Balance,
    pub sell: Balance,
}

/// Prices with 10^18 precision. Amount of the asset per 1 XOR. The same format as used
/// in price tools.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct XorPrices {
    /// Amount of asset A per XOR
    pub asset_a: AssetPrices,
    /// Amount of asset B per XOR
    pub asset_b: AssetPrices,
}

/// Calculate prices of XOR in the assets A and B given the expected relative price A in terms of B.
/// The resulting prices can be directly used for [`set_price`]/`price_tools::incoming_spot_price`,
/// as they require prices of XOR in terms of an asset.
pub fn calculate_xor_prices<T: Config>(
    asset_a: &T::AssetId,
    asset_b: &T::AssetId,
    b_per_a_buy: Balance,
    b_per_a_sell: Balance,
) -> Result<XorPrices, DispatchError> {
    match (asset_a, asset_b) {
        (xor, _b) if xor == &XOR.into() => {
            Ok(XorPrices {
                // xor
                asset_a: AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1),
                },
                // price of xor in b
                asset_b: AssetPrices {
                    buy: b_per_a_buy,
                    sell: b_per_a_sell,
                },
            })
        }
        (_a, xor) if xor == &XOR.into() => {
            // Variant is inverted, just like in `price_tools`
            let a_per_xor_sell = *BalanceUnit::one()
                .checked_div(&BalanceUnit::divisible(b_per_a_buy))
                .ok_or::<Error<T>>(Error::<T>::ArithmeticError.into())?
                .balance();
            let a_per_xor_buy = *BalanceUnit::one()
                .checked_div(&BalanceUnit::divisible(b_per_a_sell))
                .ok_or::<Error<T>>(Error::<T>::ArithmeticError.into())?
                .balance();
            Ok(XorPrices {
                // price of xor in a
                asset_a: AssetPrices {
                    buy: a_per_xor_buy,
                    sell: a_per_xor_sell,
                },
                // xor
                asset_b: AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1),
                },
            })
        }
        (input, output) => {
            // To obtain XOR prices, these formulae should be followed:
            //
            // Buy:
            // (A -buy-> B) = (A -buy-> XOR) * (XOR -buy-> B) =
            // = (1 / (XOR -sell-> A)) * (XOR -buy-> B)
            //
            // Sell:
            // (A -sell-> B) = (A -sell-> XOR) * (XOR -sell-> B) =
            // = (1 / (XOR -buy-> A)) * (XOR -sell-> B)

            // in the code notation "A -sell-> B" is represented by `a_sell_b`
            // because it's easier to comprehend the formulae with this instead of `b_per_a_sell`

            // Get known values from the formula:
            let a_buy_b = BalanceUnit::divisible(b_per_a_buy);
            let xor_buy_b = price_tools::Pallet::<T>::get_average_price(
                &XOR.into(),
                &asset_b,
                PriceVariant::Buy,
            )
            .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?;
            let xor_buy_b = BalanceUnit::divisible(xor_buy_b);
            let a_sell_b = BalanceUnit::divisible(b_per_a_sell);
            let xor_sell_b = price_tools::Pallet::<T>::get_average_price(
                &XOR.into(),
                &asset_b,
                PriceVariant::Sell,
            )
            .map_err(|_| Error::<T>::ReferenceAssetPriceNotFound)?;
            let xor_sell_b = BalanceUnit::divisible(xor_sell_b);

            // Buy:
            // (A -buy-> B) = (XOR -buy-> B) / (XOR -sell-> A)
            //
            // known:
            // (A -buy-> B), (XOR -buy-> B)
            //
            // solving for unknown:
            // (XOR -sell-> A) = (XOR -buy-> B) / (A -buy-> B)
            let xor_sell_a = xor_buy_b / a_buy_b;

            // Sell:
            // (A -sell-> B) = (XOR -sell-> B) / (XOR -buy-> A)
            //
            // known:
            // (A -sell-> B), (XOR -sell-> B)
            //
            // solving for unknown:
            // (XOR -buy-> A) = (XOR -sell-> B) / (A -sell-> B)
            let xor_buy_a = xor_sell_b / a_sell_b;
            Ok(XorPrices {
                // xor
                asset_a: AssetPrices {
                    buy: *xor_buy_a.balance(),
                    sell: *xor_sell_a.balance(),
                },
                asset_b: AssetPrices {
                    buy: *xor_buy_b.balance(),
                    sell: *xor_sell_b.balance(),
                },
            })
        }
    }
}
