#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use core::convert::TryFrom;

use codec::{Decode, Encode};

use common::prelude::fixnum::ops::{CheckedAdd, CheckedMul, Numeric};
use common::prelude::{Balance, FixedWrapper, SwapAmount, SwapOutcome, SwapVariant};
use common::{
    fixed, linspace, FilterMode, Fixed, FixedInner, IntervalEndpoints, LiquidityRegistry,
    LiquiditySource, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType,
};
use frame_support::{
    decl_error, decl_event, decl_module, dispatch::DispatchResult, ensure, traits::Get,
    weights::Weight, RuntimeDebug,
};
use frame_system::ensure_signed;
use sp_runtime::DispatchError;
use sp_std::prelude::*;

mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod algo;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"liquidity-proxy";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub enum ExchangePath<T: Trait> {
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

impl<T: Trait> ExchangePath<T> {
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

/// Output of the aggregated LiquidityProxy::quote_with_filter() price.
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

pub trait WeightInfo {
    fn swap(amount: SwapVariant) -> Weight;
}

pub trait Trait: common::Trait + assets::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type LiquidityRegistry: LiquidityRegistry<
        Self::DEXId,
        Self::AccountId,
        Self::AssetId,
        LiquiditySourceType,
        Fixed,
        DispatchError,
    >;
    type GetNumSamples: Get<usize>;
    type GetTechnicalAccountId: Get<Self::AccountId>;

    /// Weight information for the extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as assets::Trait>::AssetId,
        DEXId = <T as common::Trait>::DEXId,
    {
        /// Exchange of tokens has been performed
        /// [Caller Account, DEX Id, Input Asset Id, Output Asset Id, Input Amount, Output Amount, Fee Amount]
        Exchange(AccountId, DEXId, AssetId, AssetId, Fixed, Fixed, Fixed),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
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

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Perform swap of tokens (input/output defined via SwapAmount direction).
        ///
        /// - `origin`: the account on whose behalf the transaction is being executed,
        /// - `dex_id`: DEX ID for which liquidity sources aggregation is being done,
        /// - `input_asset_id`: ID of the asset being sold,
        /// - `output_asset_id`: ID of the asset being bought,
        /// - `swap_amount`: the exact amount to be sold (either in input_asset_id or output_asset_id units with corresponding slippage tolerance absolute bound),
        /// - `selected_source_types`: list of selected LiquiditySource types, selection effect is determined by filter_mode,
        /// - `filter_mode`: indicate either to allow or forbid selected types only, or disable filtering.
        #[weight = <T as Trait>::WeightInfo::swap((*swap_amount).into())]
        pub fn swap(
            origin,
            dex_id: T::DEXId,
            input_asset_id: T::AssetId,
            output_asset_id: T::AssetId,
            swap_amount: SwapAmount<Fixed>,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let outcome = Self::perform_swap(
                &who,
                &who,
                &input_asset_id,
                &output_asset_id,
                swap_amount,
                LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
            )?;
            let (input_amount, output_amount, fee_amount) = match swap_amount {
                SwapAmount::WithDesiredInput{desired_amount_in, ..} => (desired_amount_in, outcome.amount, outcome.fee),
                SwapAmount::WithDesiredOutput{desired_amount_out, ..} => (outcome.amount, desired_amount_out, outcome.fee),
            };
            Self::deposit_event(
                RawEvent::Exchange(
                    who,
                    dex_id,
                    input_asset_id,
                    output_asset_id,
                    input_amount,
                    output_amount,
                    fee_amount,
                )
            );
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Sample a single liquidity source with a range of swap amounts to get respective prices for the exchange.
    fn sample_liquidity_source(
        liquidity_source_id: &LiquiditySourceId<T::DEXId, LiquiditySourceType>,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
    ) -> Vec<SwapOutcome<Fixed>> {
        let num_samples = T::GetNumSamples::get();
        match amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in: amount,
                min_amount_out: min_out,
            } => {
                let inputs: Vec<_> =
                    linspace(fixed!(0), amount, num_samples, IntervalEndpoints::Right)
                        .iter()
                        .zip(
                            linspace(fixed!(0), min_out, num_samples, IntervalEndpoints::Right)
                                .iter(),
                        )
                        .map(|(x, y)| {
                            T::LiquidityRegistry::quote(
                                liquidity_source_id,
                                input_asset_id,
                                output_asset_id,
                                SwapAmount::with_desired_input(*x, *y),
                            )
                            .unwrap_or_else(|_| SwapOutcome::new(fixed!(0), fixed!(0)))
                        })
                        .collect();
                inputs
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out: amount,
                max_amount_in: max_in,
            } => {
                let outputs: Vec<_> =
                    linspace(fixed!(0), amount, num_samples, IntervalEndpoints::Right)
                        .iter()
                        .zip(
                            linspace(fixed!(0), max_in, num_samples, IntervalEndpoints::Right)
                                .iter(),
                        )
                        .map(|(x, y)| {
                            T::LiquidityRegistry::quote(
                                liquidity_source_id,
                                input_asset_id,
                                output_asset_id,
                                SwapAmount::with_desired_output(*x, *y),
                            )
                            .unwrap_or_else(|_| SwapOutcome::new(Fixed::MAX, fixed!(0)))
                        })
                        .collect();
                outputs
            }
        }
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `perform_swap_single`.
    pub fn perform_swap(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        common::with_transaction(|| {
            match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
                ExchangePath::Direct {
                    from_asset_id,
                    to_asset_id,
                } => Self::perform_swap_single(
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
                        let first_swap = Self::perform_swap_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(desired_amount_in, Fixed::ZERO),
                            filter.clone(),
                        )?;
                        let second_swap = Self::perform_swap_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Fixed::ZERO),
                            filter,
                        )?;
                        ensure!(
                            second_swap.amount >= min_amount_out,
                            Error::<T>::SlippageNotTolerated
                        );
                        let cumulative_fee = first_swap
                            .fee
                            .cadd(second_swap.fee)
                            .map_err(|_| Error::<T>::CalculationError)?;
                        Ok(SwapOutcome::new(second_swap.amount, cumulative_fee))
                    }
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out,
                        max_amount_in,
                    } => {
                        let second_quote = Self::quote_with_filter_single(
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_output(desired_amount_out, Fixed::MAX),
                            filter.clone(),
                        )?;
                        let first_quote = Self::quote_with_filter_single(
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_output(second_quote.amount, Fixed::MAX),
                            filter.clone(),
                        )?;
                        ensure!(
                            first_quote.amount <= max_amount_in,
                            Error::<T>::SlippageNotTolerated
                        );
                        let transit_account = T::GetTechnicalAccountId::get();
                        let first_swap = Self::perform_swap_single(
                            sender,
                            &transit_account,
                            &from_asset_id,
                            &intermediate_asset_id,
                            SwapAmount::with_desired_input(first_quote.amount, Fixed::ZERO),
                            filter.clone(),
                        )?;
                        let second_swap = Self::perform_swap_single(
                            &transit_account,
                            receiver,
                            &intermediate_asset_id,
                            &to_asset_id,
                            SwapAmount::with_desired_input(first_swap.amount, Fixed::ZERO),
                            filter,
                        )?;
                        let cumulative_fee = first_swap
                            .fee
                            .cadd(second_swap.fee)
                            .map_err(|_| Error::<T>::CalculationError)?;
                        Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                    }
                },
            }
        })
    }

    /// Performs a swap given a number of liquidity sources and a distribuition of the swap amount across the sources.
    fn perform_swap_single(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        common::with_transaction(|| {
            let res =
                Self::quote_with_filter_single(input_asset_id, output_asset_id, amount, filter)?
                    .distribution
                    .into_iter()
                    .filter(|(_src, share)| *share > fixed!(0))
                    .map(|(src, share)| {
                        T::LiquidityRegistry::exchange(
                            sender,
                            receiver,
                            &src,
                            input_asset_id,
                            output_asset_id,
                            amount * share,
                        )
                    })
                    .collect::<Result<Vec<SwapOutcome<Fixed>>, DispatchError>>()?;

            let (amount, fee): (FixedWrapper, FixedWrapper) = res
                .into_iter()
                .fold((fixed!(0), fixed!(0)), |(amount_acc, fee_acc), x| {
                    (amount_acc + x.amount, fee_acc + x.fee)
                });
            let amount = amount.get().map_err(|_| Error::CalculationError::<T>)?;
            let fee = fee.get().map_err(|_| Error::CalculationError::<T>)?;

            Ok(SwapOutcome::new(amount, fee))
        })
    }

    /// Applies trivial routing (via Base Asset), resulting in a poly-swap which may contain several individual swaps.
    /// Those individual swaps are subject to liquidity aggregation algorithm.
    ///
    /// This a wrapper for `quote_with_filter_single`.
    pub fn quote_with_filter(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Fixed>, DispatchError> {
        ensure!(
            input_asset_id != output_asset_id,
            Error::<T>::UnavailableExchangePath
        );
        match Self::construct_trivial_path(*input_asset_id, *output_asset_id) {
            ExchangePath::Direct {
                from_asset_id,
                to_asset_id,
            } => Self::quote_with_filter_single(&from_asset_id, &to_asset_id, amount, filter)
                .map(|aso| SwapOutcome::new(aso.amount, aso.fee).into()),
            ExchangePath::Twofold {
                from_asset_id,
                intermediate_asset_id,
                to_asset_id,
            } => match amount {
                SwapAmount::WithDesiredInput {
                    desired_amount_in, ..
                } => {
                    let first_quote = Self::quote_with_filter_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_input(desired_amount_in, Fixed::ZERO),
                        filter.clone(),
                    )?;
                    let second_quote = Self::quote_with_filter_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_input(first_quote.amount, Fixed::ZERO),
                        filter,
                    )?;
                    let cumulative_fee = first_quote
                        .fee
                        .cadd(second_quote.fee)
                        .map_err(|_| Error::<T>::CalculationError)?;
                    Ok(SwapOutcome::new(second_quote.amount, cumulative_fee))
                }
                SwapAmount::WithDesiredOutput {
                    desired_amount_out, ..
                } => {
                    let second_quote = Self::quote_with_filter_single(
                        &intermediate_asset_id,
                        &to_asset_id,
                        SwapAmount::with_desired_output(desired_amount_out, Fixed::MAX),
                        filter.clone(),
                    )?;
                    let first_quote = Self::quote_with_filter_single(
                        &from_asset_id,
                        &intermediate_asset_id,
                        SwapAmount::with_desired_output(second_quote.amount, Fixed::MAX),
                        filter,
                    )?;
                    let cumulative_fee = first_quote
                        .fee
                        .cadd(second_quote.fee)
                        .map_err(|_| Error::<T>::CalculationError)?;
                    Ok(SwapOutcome::new(first_quote.amount, cumulative_fee))
                }
            },
        }
    }

    /// Computes the optimal distribution across available liquidity sources to exectute the requested trade
    /// given the input and output assets, the trade amount and a liquidity sources filter.
    ///
    /// - 'input_asset_id' - ID of the asset to sell,
    /// - 'output_asset_id' - ID of the asset to buy,
    /// - 'amount' - the amount with "direction" (sell or buy) together with the maximum price impact (slippage),
    /// - 'filter' - a filter composed of a list of liquidity sources IDs to accept or ban for this trade.
    ///
    fn quote_with_filter_single(
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: SwapAmount<Fixed>,
        filter: LiquiditySourceFilter<T::DEXId, LiquiditySourceType>,
    ) -> Result<
        AggregatedSwapOutcome<LiquiditySourceId<T::DEXId, LiquiditySourceType>, Fixed>,
        DispatchError,
    > {
        let num_samples = T::GetNumSamples::get();
        let sources =
            T::LiquidityRegistry::list_liquidity_sources(input_asset_id, output_asset_id, filter)?;

        ensure!(!sources.is_empty(), Error::<T>::UnavailableExchangePath);

        let sampling_outcome = sources
            .iter()
            .map(|src| Self::sample_liquidity_source(src, input_asset_id, output_asset_id, amount));
        let (sample_data, sample_fees): (Vec<_>, Vec<_>) = sampling_outcome
            .map(|row| {
                let data = row.iter().map(|x| x.amount).collect::<Vec<_>>();
                let fees = row.iter().map(|x| x.fee).collect::<Vec<_>>();
                (data, fees)
            })
            .unzip();

        let (distr, best) = match amount {
            SwapAmount::WithDesiredInput { .. } => algo::find_distribution(sample_data, false),
            _ => algo::find_distribution(sample_data, true),
        };

        ensure!(
            best > Fixed::ZERO && best < Fixed::MAX,
            Error::<T>::AggregationError
        );

        let num_samples =
            FixedInner::try_from(num_samples).map_err(|_| Error::CalculationError::<T>)?;
        let total_fee: FixedWrapper = (0..distr.len()).fold(Fixed::ZERO.into(), |acc, i| {
            let idx = match distr[i].cmul(num_samples) {
                Err(_) => return acc,
                Ok(index) => index.rounding_to_i64(),
            };
            acc + *sample_fees[i]
                .get((idx - 1) as usize)
                .unwrap_or(&Fixed::ZERO)
        });
        let total_fee = total_fee.get().map_err(|_| Error::CalculationError::<T>)?;

        Ok(AggregatedSwapOutcome::<
            LiquiditySourceId<T::DEXId, LiquiditySourceType>,
            Fixed,
        >::new(
            sources
                .into_iter()
                .zip(distr.into_iter())
                .collect::<Vec<_>>(),
            best,
            total_fee,
        ))
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
}

/// Implementation of LiquiditySource Trait for LiquidityProxy, it's actually exposes reduced set of parameters that can be passed,
/// therefore it's not used for extrinsics and user-querieable rpc, but intended for pallets that need to perform swap in an
/// automated manner, still conforming to general liquidity source interface.
impl<T: Trait> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Module<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        Self::construct_trivial_path(*input_asset_id, *output_asset_id)
            .as_vec()
            .iter()
            .map(|(from_asset_id, to_asset_id)| {
                T::LiquidityRegistry::list_liquidity_sources(
                    from_asset_id,
                    to_asset_id,
                    LiquiditySourceFilter::empty(*dex_id),
                )
                .unwrap_or(Vec::new())
                .iter()
                .map(|source| {
                    T::LiquidityRegistry::can_exchange(source, input_asset_id, output_asset_id)
                })
                .any(|b| b)
            })
            .all(|b| b)
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Self::quote_with_filter(
            input_asset_id,
            output_asset_id,
            swap_amount.into(),
            LiquiditySourceFilter::empty(*dex_id),
        )
        .map(|so| so.into())
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Self::perform_swap(
            sender,
            receiver,
            input_asset_id,
            output_asset_id,
            desired_amount.into(),
            LiquiditySourceFilter::empty(*dex_id),
        )
        .map(|so| so.into())
    }
}
