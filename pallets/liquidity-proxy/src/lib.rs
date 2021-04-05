#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use core::convert::{TryFrom, TryInto};

use codec::{Decode, Encode};

use common::prelude::fixnum::ops::{Bounded, CheckedMul, One, Zero as _};
use common::prelude::{Balance, FixedWrapper, SwapAmount, SwapOutcome, SwapVariant};
use common::{
    fixed, fixed_wrapper, linspace, FilterMode, Fixed, FixedInner, GetMarketInfo, GetPoolReserves,
    IntervalEndpoints, LiquidityRegistry, LiquiditySource, LiquiditySourceFilter,
    LiquiditySourceId, LiquiditySourceType,
};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, RuntimeDebug};
use frame_system::ensure_signed;
use sp_runtime::traits::{UniqueSaturatedFrom, Zero};
use sp_runtime::DispatchError;
use sp_std::prelude::*;

type LiquiditySourceIdOf<T> = LiquiditySourceId<<T as common::Config>::DEXId, LiquiditySourceType>;

mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod algo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"liquidity-proxy";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub enum ExchangePath<T: Config> {
    Direct {
        from_asset_id: T::AssetId,
        to_asset_id: T::AssetId,
    },
    Twofold {
        from_asset_id: T::AssetId,
        intermediate_asset_id: T::AssetId,
        to_asset_id: T::AssetId,
    },
}

impl<T: Config> ExchangePath<T> {
    pub fn as_vec(self) -> Vec<(T::AssetId, T::AssetId)> {
        match self {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => [(from_asset_id, to_asset_id)].into(),
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => [
                (from_asset_id, intermediate_asset_id),
                (intermediate_asset_id, to_asset_id),
            ]
            .into(),
        }
    }
}

/// Output of the aggregated LiquidityProxy::quote() price.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AggregatedSwapOutcome<LiquiditySourceType, AmountType> {
    /// A distribution of shares each liquidity sources gets to swap in the entire trade
    pub distribution: Vec<(LiquiditySourceType, Fixed)>,
    /// The best possible output/input amount for a given trade and a set of liquidity sources
    pub amount: AmountType,
    /// Total fee amount, nominated in XOR
    pub fee: AmountType,
}

impl<LiquiditySourceIdType, AmountType> AggregatedSwapOutcome<LiquiditySourceIdType, AmountType> {
    pub fn new(
        distribution: Vec<(LiquiditySourceIdType, Fixed)>,
        amount: AmountType,
        fee: AmountType,
    ) -> Self {
        Self {
            distribution,
            amount,
            fee,
        }
    }
}

/// Indicates that particular object can be used to perform exchanges with aggregation capability.
pub trait LiquidityProxyTrait<DEXId: PartialEq + Copy, AccountId, AssetId> {
    /// Get spot price of tokens based on desired amount, None returned if liquidity source
    /// does not have available exchange methods for indicated path.
    fn quote(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError>;

    /// Perform exchange based on desired amount.
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError>;
}

impl<DEXId: PartialEq + Copy, AccountId, AssetId> LiquidityProxyTrait<DEXId, AccountId, AssetId>
    for ()
{
    fn quote(
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }

    fn exchange(
        _sender: &AccountId,
        _receiver: &AccountId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }
}

pub trait WeightInfo {
    fn swap(amount: SwapVariant) -> Weight;
}

impl<T: Config> Pallet<T> {
    /// Sample a single liquidity source with a range of swap amounts to get respective prices for the exchange.
    fn sample_liquidity_source(
        liquidity_source_id: &LiquiditySourceIdOf<T>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
        num_samples: usize,
    ) -> Vec<SwapOutcome<Fixed>> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.sample_liquidity_source"),
            || match amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in: amount,
                    min_amount_out: min_out,
                } => linspace(Fixed::ZERO, amount, num_samples, IntervalEndpoints::Right)
                    .into_iter()
                    .zip(
                        linspace(Fixed::ZERO, min_out, num_samples, IntervalEndpoints::Right)
                            .into_iter(),
                    )
                    .map(|(x, y)| {
                        let amount = match (x.into_bits().try_into(), y.into_bits().try_into()) {
                            (Ok(x), Ok(y)) => {
                                let v = T::LiquidityRegistry::quote(
                                    liquidity_source_id,
                                    input_asset_id,
                                    output_asset_id,
                                    SwapAmount::with_desired_input(x, y),
                                )
                                .and_then(|o| {
                                    o.try_into()
                                        .map_err(|_| Error::<T>::CalculationError.into())
                                });
                                v
                            }
                            _ => Err(Error::<T>::CalculationError.into()),
                        };
                        amount.unwrap_or_else(|_| SwapOutcome::new(Fixed::ZERO, Fixed::ZERO))
                    })
                    .collect::<Vec<_>>(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: amount,
                    max_amount_in: max_in,
                } => linspace(Fixed::ZERO, amount, num_samples, IntervalEndpoints::Right)
                    .into_iter()
                    .zip(
                        linspace(Fixed::ZERO, max_in, num_samples, IntervalEndpoints::Right)
                            .into_iter(),
                    )
                    .map(|(x, y)| {
                        let amount = match (x.into_bits().try_into(), y.into_bits().try_into()) {
                            (Ok(x), Ok(y)) => {
                                let v = T::LiquidityRegistry::quote(
                                    liquidity_source_id,
                                    input_asset_id,
                                    output_asset_id,
                                    SwapAmount::with_desired_output(x, y),
                                )
                                .and_then(|o| {
                                    o.try_into()
                                        .map_err(|_| Error::<T>::CalculationError.into())
                                });
                                v
                            }
                            _ => Err(Error::<T>::CalculationError.into()),
                        };
                        amount.unwrap_or_else(|_| SwapOutcome::new(Fixed::MAX, Fixed::ZERO))
                    })
                    .collect::<Vec<_>>(),
            },
        )
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `exchange_single`.
    pub fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_benchmark(common::location_stamp!("liquidity-proxy.exchange"), || {
            ensure!(
                input_asset_id != output_asset_id,
                Error::<T>::UnavailableExchangePath
            );
            common::with_transaction(|| {
                match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
                    ExchangePath::Direct {
                        from_asset_id,
                        to_asset_id,
                    } => Self::exchange_single(
                        sender,
                        receiver,
                        &from_asset_id,
                        &to_asset_id,
                        amount,
                        filter,
                    ),
                    ExchangePath::Twofold {
                        from_asset_id,
                        intermediate_asset_id,
                        to_asset_id,
                    } => match amount {
                        SwapAmount::WithDesiredInput {
                            desired_amount_in,
                            min_amount_out,
                        } => {
                            let transit_account = T::GetTechnicalAccountId::get();
                            let first_swap = Self::exchange_single(
                                sender,
                                &transit_account,
                                &from_asset_id,
                                &intermediate_asset_id,
                                SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                                filter.clone(),
                            )?;
                            let second_swap = Self::exchange_single(
                                &transit_account,
                                receiver,
                                &intermediate_asset_id,
                                &to_asset_id,
                                SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                                filter,
                            )?;
                            ensure!(
                                second_swap.amount >= min_amount_out,
                                Error::<T>::SlippageNotTolerated
                            );
                            let cumulative_fee = first_swap
                                .fee
                                .checked_add(second_swap.fee)
                                .ok_or(Error::<T>::CalculationError)?;
                            Ok(SwapOutcome::new(second_swap.amount, cumulative_fee))
                        }
                        SwapAmount::WithDesiredOutput {
                            desired_amount_out,
                            max_amount_in,
                        } => {
                            let second_quote = Self::quote_single(
                                &intermediate_asset_id,
                                &to_asset_id,
                                SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                                filter.clone(),
                            )?;
                            let first_quote = Self::quote_single(
                                &from_asset_id,
                                &intermediate_asset_id,
                                SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                                filter.clone(),
                            )?;
                            ensure!(
                                first_quote.amount <= max_amount_in,
                                Error::<T>::SlippageNotTolerated
                            );
                            let transit_account = T::GetTechnicalAccountId::get();
                            let first_swap = Self::exchange_single(
                                sender,
                                &transit_account,
                                &from_asset_id,
                                &intermediate_asset_id,
                                SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                                filter.clone(),
                            )?;
                            let second_swap = Self::exchange_single(
                                &transit_account,
                                receiver,
                                &intermediate_asset_id,
                                &to_asset_id,
                                SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                                filter,
                            )?;
                            let cumulative_fee = first_swap
                                .fee
                                .checked_add(second_swap.fee)
                                .ok_or(Error::<T>::CalculationError)?;
                            Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                        }
                    },
                }
            })
        })
    }

    /// Performs a swap given a number of liquidity sources and a distribuition of the swap amount across the sources.
    fn exchange_single(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.exchange_single"),
            || {
                common::with_transaction(|| {
                    let fx_amount: SwapAmount<Fixed> = amount
                        .try_into()
                        .map_err(|_| Error::CalculationError::<T>)?;
                    let res = Self::quote_single(input_asset_id, output_asset_id, amount, filter)?
                        .distribution
                        .into_iter()
                        .filter(|(_src, share)| *share > Fixed::ZERO)
                        .map(|(src, share)| {
                            let filter = fx_amount * share;
                            let filter = filter
                                .try_into()
                                .map_err(|_| Error::CalculationError::<T>)?;
                            T::LiquidityRegistry::exchange(
                                sender,
                                receiver,
                                &src,
                                input_asset_id,
                                output_asset_id,
                                filter,
                            )
                        })
                        .collect::<Result<Vec<SwapOutcome<Balance>>, DispatchError>>()?;

                    let (amount, fee): (FixedWrapper, FixedWrapper) = res.into_iter().fold(
                        (fixed_wrapper!(0), fixed_wrapper!(0)),
                        |(amount_acc, fee_acc), x| {
                            (
                                amount_acc + FixedWrapper::from(x.amount),
                                fee_acc + FixedWrapper::from(x.fee),
                            )
                        },
                    );
                    let amount = amount
                        .try_into_balance()
                        .map_err(|_| Error::CalculationError::<T>)?;
                    let fee = fee
                        .try_into_balance()
                        .map_err(|_| Error::CalculationError::<T>)?;

                    Ok(SwapOutcome::new(amount, fee))
                })
            },
        )
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_single`.
    pub fn quote(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_benchmark(common::location_stamp!("liquidity-proxy.quote"), || {
            ensure!(
                input_asset_id != output_asset_id,
                Error::<T>::UnavailableExchangePath
            );
            match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
                ExchangePath::Direct {
                    from_asset_id,
                    to_asset_id,
                } => Self::quote_single(&from_asset_id, &to_asset_id, amount, filter)
                    .map(|aso| SwapOutcome::new(aso.amount, aso.fee).into()),
                ExchangePath::Twofold {
                    from_asset_id,
                    intermediate_asset_id,
                    to_asset_id,
                } => match amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in, ..
                    } => {
                        let first_quote = Self::quote_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(desired_amount_in, 0),
                            filter.clone(),
                        )?;
                        let second_quote = Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_quote.amount, 0),
                            filter,
                        )?;
                        let cumulative_fee = first_quote
                            .fee
                            .checked_add(second_quote.fee)
                            .ok_or(Error::<T>::CalculationError)?;
                        Ok(SwapOutcome::new(second_quote.amount, cumulative_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out, ..
                    } => {
                        let second_quote = Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                            filter.clone(),
                        )?;
                        let first_quote = Self::quote_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                            filter,
                        )?;
                        let cumulative_fee = first_quote
                            .fee
                            .checked_add(second_quote.fee)
                            .ok_or(Error::<T>::CalculationError)?;
                        Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                    }
                },
            }
        })
    }

    /// Computes the optimal distribution across available liquidity sources to exectute the requested trade
    /// given the input and output assets, the trade amount and a liquidity sources filter.
    ///
    /// - 'input_asset_id' - ID of the asset to sell,
    /// - 'output_asset_id' - ID of the asset to buy,
    /// - 'amount' - the amount with "direction" (sell or buy) together with the maximum price impact (slippage),
    /// - 'filter' - a filter composed of a list of liquidity sources IDs to accept or ban for this trade.
    ///
    fn quote_single(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>, DispatchError> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.quote_single"),
            || {
                let sources = T::LiquidityRegistry::list_liquidity_sources(
                    input_asset_id,
                    output_asset_id,
                    filter,
                )?;

                ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);

                // Check if we have exactly one source => no split required
                if sources.len() == 1 {
                    let src = sources.first().unwrap();
                    let outcome =
                        T::LiquidityRegistry::quote(src, input_asset_id, output_asset_id, amount)?;
                    return Ok(AggregatedSwapOutcome::new(
                        vec![(src.clone(), fixed!(1.0))],
                        outcome.amount,
                        outcome.fee,
                    ));
                }

                // Check if we have exactly two sources: the primary market and the secondary market
                // Do fast swap amount split
                if sources.len() == 2 {
                    let mut primary_market: Option<LiquiditySourceIdOf<T>> = None;
                    let mut secondary_market: Option<LiquiditySourceIdOf<T>> = None;

                    for src in &sources {
                        if src.liquidity_source_index
                            == LiquiditySourceType::MulticollateralBondingCurvePool
                        {
                            primary_market = Some(src.clone());
                        } else {
                            secondary_market = Some(src.clone());
                        }
                    }
                    if let (Some(mcbc), Some(xyk)) = (primary_market, secondary_market) {
                        return Self::fast_split_primary_vs_secondary(
                            mcbc,
                            xyk,
                            input_asset_id,
                            output_asset_id,
                            amount,
                        );
                    }
                }

                // Otherwise, fall back to the general source-agnostic procedure based on sampling
                Self::generic_split(sources, input_asset_id, output_asset_id, amount)
            },
        )
    }

    pub fn construct_trivial_path(
        input_asset_id: T::AssetId,
        output_asset_id: T::AssetId,
    ) -> ExchangePath<T> {
        let base_asset_id = T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id || output_asset_id == base_asset_id {
            ExchangePath::Direct {
                from_asset_id: input_asset_id,
                to_asset_id: output_asset_id,
            }
        } else {
            ExchangePath::Twofold {
                from_asset_id: input_asset_id,
                intermediate_asset_id: base_asset_id,
                to_asset_id: output_asset_id,
            }
        }
    }

    /// A wrapper function around the "fast" split algorithm implementations.
    /// Dispatches the call to the correct variant of the algorithm depending on
    /// whether the base asset is being bought or sold.
    ///
    /// - 'primary_source_id' - ID of the primary market liquidity source,
    /// - 'secondary_source_id' - ID of the secondary market liquidity source,
    /// - 'input_asset_id' - ID of the asset to sell,
    /// - 'output_asset_id' - ID of the asset to buy,
    /// - 'amount' - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    ///
    fn fast_split_primary_vs_secondary(
        primary_source_id: LiquiditySourceIdOf<T>,
        secondary_source_id: LiquiditySourceIdOf<T>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
    ) -> Result<AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>, DispatchError> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.fast_split_primary_vs_secondary"),
            || {
                let base_asset = &T::GetBaseAssetId::get();

                ensure!(
                    input_asset_id == base_asset || output_asset_id == base_asset,
                    Error::<T>::UnavailableExchangePath
                );
                let other_asset = if base_asset == input_asset_id {
                    output_asset_id
                } else {
                    input_asset_id
                };

                // Ensure secondary market exists for the pair of assets
                let (reserves_base, reserves_other) =
                    T::SecondaryMarket::reserves(base_asset, other_asset);
                if reserves_base == 0 || reserves_other == 0 {
                    // No reserves in secondaty market, resort to the primary market
                    let outcome = T::LiquidityRegistry::quote(
                        &primary_source_id,
                        input_asset_id,
                        output_asset_id,
                        amount.clone(),
                    )?;
                    return Ok(AggregatedSwapOutcome::new(
                        vec![
                            (primary_source_id, fixed!(1.0)),
                            (secondary_source_id, fixed!(0)),
                        ],
                        outcome.amount,
                        outcome.fee,
                    ));
                }

                let output = if output_asset_id == base_asset {
                    // XOR is being bought
                    Self::decide_amounts_buying_base_asset(
                        primary_source_id,
                        secondary_source_id,
                        base_asset,
                        other_asset,
                        amount,
                        (reserves_base, reserves_other),
                    )
                } else {
                    // XOR is being sold
                    Self::decide_amounts_selling_base_asset(
                        primary_source_id,
                        secondary_source_id,
                        base_asset,
                        other_asset,
                        amount,
                        (reserves_base, reserves_other),
                    )
                };
                output
            },
        )
    }

    /// Implements a generic source-agnostic split algorithm to partition a trade between
    /// an arbitrary number of liquidity sources of arbitrary types.
    ///
    /// - 'sources' - a vector of liquidity sources IDs,
    /// - 'input_asset_id' - ID of the asset to sell,
    /// - 'output_asset_id' - ID of the asset to buy,
    /// - 'amount' - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    ///
    fn generic_split(
        sources: Vec<LiquiditySourceIdOf<T>>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
    ) -> Result<AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>, DispatchError> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.generic_split"),
            || {
                let amount = <SwapAmount<Fixed>>::unique_saturated_from(amount);
                let num_samples = T::GetNumSamples::get();
                let (sample_data, sample_fees): (Vec<Vec<Fixed>>, Vec<Vec<Fixed>>) = sources
                    .iter()
                    .map(|src| {
                        Self::sample_liquidity_source(
                            src,
                            input_asset_id,
                            output_asset_id,
                            amount,
                            num_samples,
                        )
                    })
                    .map(|row| row.iter().map(|x| (x.amount, x.fee)).unzip())
                    .unzip();

                let (distr, best) = match amount {
                    SwapAmount::WithDesiredInput { .. } => {
                        algo::find_distribution(sample_data, false)
                    }
                    _ => algo::find_distribution(sample_data, true),
                };

                ensure!(
                    best > Fixed::ZERO && best < Fixed::MAX,
                    Error::<T>::AggregationError
                );

                let num_samples =
                    FixedInner::try_from(num_samples).map_err(|_| Error::CalculationError::<T>)?;
                let total_fee: FixedWrapper = (0..distr.len()).fold(fixed!(0), |acc, i| {
                    let idx = match distr[i].cmul(num_samples) {
                        Err(_) => return acc,
                        Ok(index) => index.rounding_to_i64(),
                    };
                    acc + *sample_fees[i]
                        .get((idx - 1) as usize)
                        .unwrap_or(&Fixed::ZERO)
                });
                let total_fee = total_fee.get().map_err(|_| Error::CalculationError::<T>)?;

                Ok(AggregatedSwapOutcome::new(
                    sources
                        .into_iter()
                        .zip(distr.into_iter())
                        .collect::<Vec<_>>(),
                    best.into_bits()
                        .try_into()
                        .map_err(|_| Error::CalculationError::<T>)?,
                    total_fee
                        .into_bits()
                        .try_into()
                        .map_err(|_| Error::CalculationError::<T>)?,
                ))
            },
        )
    }

    /// Implements a "fast" split algorithm to partition a trade between a single primary liquidity source
    /// (e.g. a multi-collateral bonding curve pool) and a single secondary source (e.g. an XYK pool)
    /// for the case when the base asset is being bought.
    ///
    /// - 'primary_source_id' - ID of the primary market liquidity source,
    /// - 'secondary_source_id' - ID of the secondary market liquidity source,
    /// - 'input_asset_id' - ID of the asset to sell,
    /// - 'output_asset_id' - ID of the asset to buy,
    /// - 'amount' - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    ///
    fn decide_amounts_buying_base_asset(
        primary_source_id: LiquiditySourceIdOf<T>,
        secondary_source_id: LiquiditySourceIdOf<T>,
        base_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        secondary_market_reserves: (Balance, Balance),
    ) -> Result<AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>, DispatchError> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.decide_amounts_buying_base_asset"),
            || {
                let (reserves_base, reserves_other) = secondary_market_reserves;
                let x: FixedWrapper = reserves_base.into();
                let y: FixedWrapper = reserves_other.into();
                let sqrt_k: FixedWrapper = x.multiply_and_sqrt(&y);
                let secondary_price: FixedWrapper = y.clone() / x.clone();

                let primary_buy_price: FixedWrapper =
                    T::PrimaryMarket::buy_price(base_asset_id, collateral_asset_id)
                        .map_err(|_| Error::<T>::CalculationError)?
                        .into();
                let sqrt_buy_price = primary_buy_price.clone().sqrt_accurate();

                match amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in, ..
                    } => {
                        let mut fraction_sec: Fixed = fixed!(0);
                        let wrapped_amount: FixedWrapper = desired_amount_in.into();
                        if secondary_price < primary_buy_price {
                            let delta_y = sqrt_k * sqrt_buy_price - y; // always > 0
                            fraction_sec = (delta_y / wrapped_amount.clone())
                                .get()
                                .unwrap_or(fixed!(0));
                            if fraction_sec > fixed!(1) {
                                fraction_sec = fixed!(1);
                            }
                            if fraction_sec < fixed!(0) {
                                fraction_sec = fixed!(0);
                            }
                        }
                        let fraction_prim: Fixed =
                            (Fixed::ONE - fraction_sec.into()).get().unwrap();

                        let distr = vec![
                            (primary_source_id, fraction_prim),
                            (secondary_source_id, fraction_sec),
                        ];
                        let mut best: Balance = 0;
                        let mut total_fee: Balance = 0;
                        for d in &distr {
                            let amt: Balance = (wrapped_amount.clone() * FixedWrapper::from(d.1))
                                .try_into_balance()
                                .map_err(|_| Error::<T>::CalculationError)?;
                            if amt > 0 {
                                let outcome: SwapOutcome<Balance> = T::LiquidityRegistry::quote(
                                    &d.0,
                                    collateral_asset_id,
                                    base_asset_id,
                                    SwapAmount::<Balance>::with_desired_input(amt, 0),
                                )
                                .map_err(|_| Error::<T>::CalculationError)?;
                                best += outcome.amount;
                                total_fee += outcome.fee;
                            }
                        }

                        Ok(AggregatedSwapOutcome::new(distr, best, total_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out, ..
                    } => {
                        let mut fraction_sec: Fixed = fixed!(0);
                        let wrapped_amount: FixedWrapper = desired_amount_out.into();
                        if secondary_price < primary_buy_price {
                            let delta_x = x - sqrt_k / sqrt_buy_price; // always > 0
                            fraction_sec = (delta_x / wrapped_amount.clone())
                                .get()
                                .unwrap_or(fixed!(0));
                            if fraction_sec > fixed!(1) {
                                fraction_sec = fixed!(1);
                            }
                            if fraction_sec < fixed!(0) {
                                fraction_sec = fixed!(0);
                            }
                        }
                        let fraction_prim: Fixed =
                            (Fixed::ONE - fraction_sec.into()).get().unwrap();

                        let distr = vec![
                            (primary_source_id, fraction_prim),
                            (secondary_source_id, fraction_sec),
                        ];
                        let mut best: Balance = 0;
                        let mut total_fee: Balance = 0;
                        for d in &distr {
                            let amt: Balance = (wrapped_amount.clone() * FixedWrapper::from(d.1))
                                .try_into_balance()
                                .map_err(|_| Error::<T>::CalculationError)?;
                            if amt > 0 {
                                let outcome: SwapOutcome<Balance> = T::LiquidityRegistry::quote(
                                    &d.0,
                                    collateral_asset_id,
                                    base_asset_id,
                                    SwapAmount::<Balance>::with_desired_output(amt, Balance::MAX),
                                )
                                .map_err(|_| Error::<T>::CalculationError)?;
                                best += outcome.amount;
                                total_fee += outcome.fee;
                            }
                        }

                        Ok(AggregatedSwapOutcome::new(distr, best, total_fee))
                    }
                }
            },
        )
    }

    /// Implements a "fast" split algorithm to partition a trade between a single primary liquidity source
    /// (e.g. a multi-collateral bonding curve pool) and a single secondary source (e.g. an XYK pool)
    /// for the case when the base asset is being sold.
    ///
    /// - 'primary_source_id' - ID of the primary market liquidity source,
    /// - 'secondary_source_id' - ID of the secondary market liquidity source,
    /// - 'input_asset_id' - ID of the asset to sell,
    /// - 'output_asset_id' - ID of the asset to buy,
    /// - 'amount' - the amount with "direction" (sell or buy) together with the maximum price impact (slippage).
    ///
    fn decide_amounts_selling_base_asset(
        primary_source_id: LiquiditySourceIdOf<T>,
        secondary_source_id: LiquiditySourceIdOf<T>,
        base_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        secondary_market_reserves: (Balance, Balance),
    ) -> Result<AggregatedSwapOutcome<LiquiditySourceIdOf<T>, Balance>, DispatchError> {
        common::with_benchmark(
            common::location_stamp!("liquidity-proxy.decide_amounts_selling_base_asset"),
            || {
                let (reserves_base, reserves_other) = secondary_market_reserves;
                let x: FixedWrapper = reserves_base.into();
                let y: FixedWrapper = reserves_other.into();
                let sqrt_k: FixedWrapper = x.multiply_and_sqrt(&y);
                let secondary_price: FixedWrapper = y.clone() / x.clone();

                let primary_sell_price: FixedWrapper =
                    T::PrimaryMarket::sell_price(base_asset_id, collateral_asset_id)
                        .map_err(|_| Error::<T>::CalculationError)?
                        .into();
                let sqrt_sell_price = primary_sell_price.clone().sqrt_accurate();

                match amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in, ..
                    } => {
                        let mut fraction_sec: Fixed = fixed!(0);
                        let wrapped_amount: FixedWrapper = desired_amount_in.into();
                        if secondary_price > primary_sell_price {
                            let delta_x = sqrt_k / sqrt_sell_price - x; // always > 0
                            fraction_sec = (delta_x / wrapped_amount.clone())
                                .get()
                                .unwrap_or(fixed!(0));
                            if fraction_sec > fixed!(1) {
                                fraction_sec = fixed!(1);
                            }
                            if fraction_sec < fixed!(0) {
                                fraction_sec = fixed!(0);
                            }
                        }
                        let fraction_prim: Fixed =
                            (Fixed::ONE - fraction_sec.into()).get().unwrap();

                        let distr = vec![
                            (primary_source_id, fraction_prim),
                            (secondary_source_id, fraction_sec),
                        ];
                        let mut best: Balance = 0;
                        let mut total_fee: Balance = 0;
                        for d in &distr {
                            let amt: Balance = (wrapped_amount.clone() * FixedWrapper::from(d.1))
                                .try_into_balance()
                                .map_err(|_| Error::<T>::CalculationError)?;
                            if amt > 0 {
                                let outcome: SwapOutcome<Balance> = T::LiquidityRegistry::quote(
                                    &d.0,
                                    base_asset_id,
                                    collateral_asset_id,
                                    SwapAmount::<Balance>::with_desired_input(amt, 0),
                                )
                                .map_err(|_| Error::<T>::CalculationError)?;
                                best += outcome.amount;
                                total_fee += outcome.fee;
                            }
                        }

                        Ok(AggregatedSwapOutcome::new(distr, best, total_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out, ..
                    } => {
                        // This sub-branch is prone to overflow because the fast split algorithm doesn't
                        // rely on the reserves of this particular collateral at the MCBC pool.
                        // So it is possible that the sell price at the bonding curve is high
                        // (thanks to other reserves stored in MCBC) in comparison with what the seller
                        // would get at the secondary market, thus incentivizing the seller to attempt to
                        // sell a larger share of XOR to the bonding curve.
                        // This situation must be specifically watched and guarded against by re-weighing
                        // the shares that are sold on primary and secondary markets.
                        let mut fraction_sec: Fixed = fixed!(0);
                        let wrapped_amount: FixedWrapper = desired_amount_out.into();
                        if secondary_price > primary_sell_price {
                            let delta_y = y - sqrt_k * sqrt_sell_price; // always > 0
                            fraction_sec = (delta_y / wrapped_amount.clone())
                                .get()
                                .unwrap_or(fixed!(0));
                            if fraction_sec > fixed!(1) {
                                fraction_sec = fixed!(1);
                            }
                            if fraction_sec < fixed!(0) {
                                fraction_sec = fixed!(0);
                            }
                        }
                        let mut fraction_prim: Fixed =
                            (Fixed::ONE - fraction_sec.into()).get().unwrap();

                        // Check if the requested amount of collateral would not exceed the reserves in MCBC pool
                        let amount_to_sell =
                            wrapped_amount.clone() * FixedWrapper::from(fraction_prim);
                        let collateral_reserves: FixedWrapper =
                            T::PrimaryMarket::collateral_reserves(collateral_asset_id)
                                .unwrap_or(Balance::zero())
                                .into();
                        if collateral_reserves < amount_to_sell {
                            fraction_prim = (fixed_wrapper!(0.9) * collateral_reserves
                                / wrapped_amount.clone())
                            .get()
                            .unwrap_or(fixed!(0));
                            fraction_sec = (Fixed::ONE - fraction_prim.into()).get().unwrap();
                        }

                        let distr = vec![
                            (primary_source_id, fraction_prim),
                            (secondary_source_id, fraction_sec),
                        ];
                        let mut best: Balance = 0;
                        let mut total_fee: Balance = 0;
                        for d in &distr {
                            let amt: Balance = (wrapped_amount.clone() * FixedWrapper::from(d.1))
                                .try_into_balance()
                                .map_err(|_| Error::<T>::CalculationError)?;
                            if amt > 0 {
                                let outcome: SwapOutcome<Balance> = T::LiquidityRegistry::quote(
                                    &d.0,
                                    base_asset_id,
                                    collateral_asset_id,
                                    SwapAmount::<Balance>::with_desired_output(amt, Balance::MAX),
                                )
                                .map_err(|_| Error::<T>::CalculationError)?;
                                best += outcome.amount;
                                total_fee += outcome.fee;
                            }
                        }

                        Ok(AggregatedSwapOutcome::new(distr, best, total_fee))
                    }
                }
            },
        )
    }
}

impl<T: Config> LiquidityProxyTrait<T::DEXId, T::AccountId, T::AssetId> for Pallet<T> {
    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_single`.
    fn quote(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => Self::quote_single(&from_asset_id, &to_asset_id, amount, filter)
                .map(|aso| SwapOutcome::new(aso.amount, aso.fee).into()),
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => match amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let first_quote = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                        filter.clone(),
                    )?;
                    let second_quote = Self::quote_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                        filter,
                    )?;
                    let cumulative_fee = first_quote.fee + second_quote.fee;
                    Ok(SwapOutcome::new(second_quote.amount, cumulative_fee))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let second_quote = Self::quote_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                        filter.clone(),
                    )?;
                    let first_quote = Self::quote_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                        filter,
                    )?;
                    let cumulative_fee = first_quote.fee + second_quote.fee;
                    Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                }
            },
        }
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `exchange_single`.
    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        common::with_transaction(|| {
            match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
                ExchangePath::Direct {
                    from_asset_id,
                    to_asset_id,
                } => Self::exchange_single(
                    sender,
                    receiver,
                    &from_asset_id,
                    &to_asset_id,
                    amount,
                    filter,
                ),
                ExchangePath::Twofold {
                    from_asset_id,
                    intermediate_asset_id,
                    to_asset_id,
                } => match amount {
                    SwapAmount::WithDesiredInput {
                        desired_amount_in,
                        min_amount_out,
                    } => {
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(desired_amount_in, Balance::zero()),
                            filter.clone(),
                        )?;
                        let second_swap = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        ensure!(
                            second_swap.amount >= min_amount_out,
                            Error::<T>::SlippageNotTolerated
                        );
                        let cumulative_fee = first_swap.fee + second_swap.fee;
                        Ok(SwapOutcome::new(second_swap.amount, cumulative_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out,
                        max_amount_in,
                    } => {
                        let second_quote = Self::quote_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_output(desired_amount_out, Balance::MAX),
                            filter.clone(),
                        )?;
                        let first_quote = Self::quote_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_output(second_quote.amount, Balance::MAX),
                            filter.clone(),
                        )?;
                        ensure!(
                            first_quote.amount <= max_amount_in,
                            Error::<T>::SlippageNotTolerated
                        );
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::exchange_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(first_quote.amount, Balance::zero()),
                            filter.clone(),
                        )?;
                        let second_swap = Self::exchange_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Balance::zero()),
                            filter,
                        )?;
                        let cumulative_fee = first_swap.fee + second_swap.fee;
                        Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                    }
                },
            }
        })
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use assets::AssetIdOf;
    use common::{AccountIdOf, DexIdOf};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + assets::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type LiquidityRegistry: LiquidityRegistry<
            Self::DEXId,
            Self::AccountId,
            Self::AssetId,
            LiquiditySourceType,
            Balance,
            DispatchError,
        >;
        type GetNumSamples: Get<usize>;
        type GetTechnicalAccountId: Get<Self::AccountId>;
        type PrimaryMarket: GetMarketInfo<Self::AssetId>;
        type SecondaryMarket: GetPoolReserves<Self::AssetId>;
        /// Weight information for the extrinsics in this Pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Perform swap of tokens (input/output defined via SwapAmount direction).
        ///
        /// - `origin`: the account on whose behalf the transaction is being executed,
        /// - `dex_id`: DEX ID for which liquidity sources aggregation is being done,
        /// - `input_asset_id`: ID of the asset being sold,
        /// - `output_asset_id`: ID of the asset being bought,
        /// - `swap_amount`: the exact amount to be sold (either in input_asset_id or output_asset_id units with corresponding slippage tolerance absolute bound),
        /// - `selected_source_types`: list of selected LiquiditySource types, selection effect is determined by filter_mode,
        /// - `filter_mode`: indicate either to allow or forbid selected types only, or disable filtering.
        #[pallet::weight(<T as Config>::WeightInfo::swap((*swap_amount).into()))]
        pub fn swap(
            origin: OriginFor<T>,
            dex_id: T::DEXId,
            input_asset_id: T::AssetId,
            output_asset_id: T::AssetId,
            swap_amount: SwapAmount<Balance>,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let outcome = Self::exchange(
                &who,
                &who,
                &input_asset_id,
                &output_asset_id,
                swap_amount,
                LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
            )?;
            let (input_amount, output_amount, fee_amount) = match swap_amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => (desired_amount_in, outcome.amount, outcome.fee),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => (outcome.amount, desired_amount_out, outcome.fee),
            };
            Self::deposit_event(Event::<T>::Exchange(
                who,
                dex_id,
                input_asset_id,
                output_asset_id,
                input_amount,
                output_amount,
                fee_amount,
            ));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", AssetIdOf<T> = "AssetId", DexIdOf<T> = "DEXId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Exchange of tokens has been performed
        /// [Caller Account, DEX Id, Input Asset Id, Output Asset Id, Input Amount, Output Amount, Fee Amount]
        Exchange(
            AccountIdOf<T>,
            DexIdOf<T>,
            AssetIdOf<T>,
            AssetIdOf<T>,
            Balance,
            Balance,
            Balance,
        ),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// No route exists in a given DEX for given parameters to carry out the swap
        UnavailableExchangePath,
        /// Max fee exceeded
        MaxFeeExceeded,
        /// Fee value outside of the basis points range [0..10000]
        InvalidFeeValue,
        /// None of the sources has enough reserves to execute a trade
        InsufficientLiquidity,
        /// Path exists but it's not possible to perform exchange with currently available liquidity on pools.
        AggregationError,
        /// Specified parameters lead to arithmetic error
        CalculationError,
        /// Slippage either exceeds minimum tolerated output or maximum tolerated input.
        SlippageNotTolerated,
    }
}
