#![cfg_attr(not(feature = "std"), no_std)]

use common::prelude::fixnum::ops::{CheckedAdd, CheckedSub};
use common::prelude::{FixedWrapper, SwapAmount};
use common::{
    balance::Balance, fixed, fixnum::ops::Numeric, EnsureDEXManager, Fixed, LiquiditySourceFilter,
    LiquiditySourceType,
};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchError, DispatchResult, Weight},
    ensure, fail,
    traits::Get,
    IterableStorageDoubleMap, IterableStorageMap,
};
use frame_system::{self as system, ensure_signed};
use liquidity_proxy::LiquidityProxyTrait;
use sp_arithmetic::traits::{Saturating, Zero};
use tokens::Accounts;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"pswap-distribution";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

type CurrencyIdOf<T> = <T as tokens::Trait>::CurrencyId;
type Assets<T> = assets::Module<T>;
type System<T> = frame_system::Module<T>;

pub trait OnPswapBurned {
    fn on_pswap_burned(amount: Balance);
}

impl OnPswapBurned for () {
    fn on_pswap_burned(_amount: Balance) {
        // do nothing
    }
}

pub trait Trait: common::Trait + assets::Trait + technical::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    type GetIncentiveAssetId: Get<Self::AssetId>;
    type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
    type CompatBalance: From<<Self as tokens::Trait>::Balance>
        + Into<common::balance::Balance>
        + From<common::balance::Balance>
        + Clone
        + Zero;
    type GetTechnicalAccountId: Get<Self::AccountId>;
    type GetDefaultSubscriptionFrequency: Get<Self::BlockNumber>;
    type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
    type OnPswapBurnedAggregator: OnPswapBurned;
}

decl_storage! {
    trait Store for Module<T: Trait> as PswapDistribution {
        /// Store for information about accounts containing fees, that participate in incentive distribution mechanism.
        /// Fees Account Id -> (DEX Id, Pool Marker Asset Id, Distribution Frequency, Block Offset) Frequency MUST be non-zero.
        pub SubscribedAccounts get(fn subscribed_accounts): map hasher(blake2_128_concat) T::AccountId => Option<(T::DEXId, T::AssetId, T::BlockNumber, T::BlockNumber)>;

        /// Amount of incentive tokens to be burned on each distribution.
        pub BurnRate get(fn burn_rate): Fixed;

        /// (Burn Rate Increase Delta, Burn Rate Max)
        BurnUpdateInfo get(fn burn_update_info): (Fixed, Fixed);

        /// Burn Rate update frequency in blocks. MUST be non-zero.
        BurnUpdateFrequency get(fn burn_update_frequency): T::BlockNumber;

        /// Information about owned portion of stored incentive tokens. Shareholder -> Owned Fraction
        pub ShareholderAccounts get(fn shareholder_accounts): map hasher(blake2_128_concat) T::AccountId => Fixed;

        /// Sum of all shares of incentive token owners.
        pub ClaimableShares get(fn claimable_shares): Fixed;

        /// This is needed for farm id 0, now it is hardcoded, in future it will be resolved and
        /// used in a more convenient way.
        pub BurnedPswapDedicatedForOtherPallets get(fn burned_pswap_dedicated_for_other_pallets): Fixed;
    }
    add_extra_genesis {
        /// (Fees Account, (DEX Id, Marker Token Id, Distribution Frequency, Block Offset))
        config(subscribed_accounts): Vec<(T::AccountId, (T::DEXId, T::AssetId, T::BlockNumber, T::BlockNumber))>;
        /// (Initial Burn Rate, Burn Rate Increase Delta, Burn Rate Max, Update Frequency)
        config(burn_info): (Fixed, Fixed, Fixed, T::BlockNumber);

        build(|config: &GenesisConfig<T>| {
            config.subscribed_accounts.iter().for_each(|(fees_account, (dex_id, pool_asset, freq, block_offset))| {
                SubscribedAccounts::<T>::insert(fees_account, (dex_id, pool_asset, freq, block_offset));
            });
            let (initial_rate, increase_delta, max, freq) = config.burn_info;
            BurnRate::mutate(|rate| *rate = initial_rate);
            BurnUpdateInfo::mutate(|info| *info = (increase_delta, max));
            BurnUpdateFrequency::<T>::mutate(|f| *f = freq);
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        DEXId = <T as common::Trait>::DEXId,
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as assets::Trait>::AssetId,
    {
        /// Fees successfully exchanged for appropriate amount of pool tokens.
        /// [DEX Id, Fees Account Id, Fees Asset Id, Fees Spent Amount, Incentive Asset Id, Incentive Received Amount]
        FeesExchanged(DEXId, AccountId, AssetId, Balance, AssetId, Balance),
        /// Problem occurred that resulted in fees exchange not done.
        /// [DEX Id, Fees Account Id, Fees Asset Id, Available Fees Amount, Incentive Asset Id]
        FeesExchangeFailed(DEXId, AccountId, AssetId, Balance, AssetId),
        /// Incentives successfully sent out to shareholders.
        /// [DEX Id, Fees Account Id, Incentive Asset Id, Incentive Total Distributed Amount, Number of shareholders]
        IncentiveDistributed(DEXId, AccountId, AssetId, Balance, u128),
        /// Problem occurred that resulted in incentive distribution not done.
        /// [DEX Id, Fees Account Id, Incentive Asset Id, Available Incentive Amount]
        IncentiveDistributionFailed(DEXId, AccountId, AssetId, Balance),
        /// Burn rate updated.
        /// [Current Burn Rate]
        BurnRateChanged(Fixed),
        /// Fees Account contains zero base tokens, thus exchange is dismissed.
        /// [DEX Id, Fees Account Id]
        NothingToExchange(DEXId, AccountId),
        /// Fees Account contains zero incentive tokens, thus distribution is dismissed.
        /// [DEX Id, Fees Account Id]
        NothingToDistribute(DEXId, AccountId),
        /// This is needed for other pallet that will use this variables, for example this is
        /// farming pallet.
        /// [DEX Id, Incentive Asset Id, Total exchanged incentives (Incentives burned after exchange),
        /// Incentives burned (Incentives that is not revived (to burn)]).
        IncentivesBurnedAfterExchange(DEXId, AssetId, Balance, Balance),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Error occurred during calculation, e.g. underflow/overflow of share amount.
        CalculationError,
        /// Error while attempting to subscribe Account which is already subscribed.
        SubscriptionActive,
        /// Error while attempting to unsubscribe Account which is not subscribed.
        UnknownSubscription,
        /// Error while setting frequency, subscription can only be invoked for frequency value >= 1.
        InvalidFrequency,
        /// Can't claim incentives as none is available for account at the moment.
        ZeroClaimableIncentives,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call
    where
        origin: T::Origin,
    {
        type Error = Error<T>;

        fn deposit_event() = default;

        #[weight = 0]
        pub fn claim_incentive(origin) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::claim_by_account(who)
        }

        /// Perform exchange and distribution routines for all substribed accounts
        /// with respect to thir configured frequencies.
        fn on_initialize(block_num: T::BlockNumber) -> Weight {
            Self::incentive_distribution_routine(block_num);
            Self::burn_rate_update_routine(block_num);
            0
        }
    }
}

macro_rules! into_currency {
    ($t:ty, $asset_id:expr) => {
        <<$t>::AssetId as Into<CurrencyIdOf<$t>>>::into($asset_id)
    };
}

impl<T: Trait> Module<T> {
    /// Check if given fees account is subscribed to incentive distribution.
    ///
    /// - `fees_account_id`: Id of Accout which accumulates fees from swaps.
    pub fn is_subscribed(fees_account_id: &T::AccountId) -> bool {
        SubscribedAccounts::<T>::get(fees_account_id).is_some()
    }

    /// Add fees account to list of periodic incentives distribution.
    /// Balance of `marker_token_id` will be used to determine marker tokens owners and their shares.
    /// Must only be called from environment where caller is ensured to be owner of given DEX.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
    /// - `dex_id`: Id of DEX to which given account belongs.
    /// - `marker_token_id`: Namely Pool Token, Asset Id by which shares of LP's are determined.
    /// - `frequency`: Number of blocks between incentive distribution operations.
    pub fn subscribe(
        fees_account_id: T::AccountId,
        dex_id: T::DEXId,
        marker_token_id: T::AssetId,
        frequency: Option<T::BlockNumber>,
    ) -> DispatchResult {
        ensure!(
            !Self::is_subscribed(&fees_account_id),
            Error::<T>::SubscriptionActive
        );
        let frequency = frequency.unwrap_or(T::GetDefaultSubscriptionFrequency::get());
        ensure!(!frequency.is_zero(), Error::<T>::InvalidFrequency);
        Assets::<T>::ensure_asset_exists(&marker_token_id)?;
        let current_block = System::<T>::block_number();
        SubscribedAccounts::<T>::insert(
            fees_account_id.clone(),
            (dex_id, marker_token_id, frequency, current_block),
        );
        Ok(())
    }

    /// Remove fees account from list of periodic distribution of incentives.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
    pub fn unsubscribe(fees_account_id: T::AccountId) -> DispatchResult {
        let value = SubscribedAccounts::<T>::take(&fees_account_id);
        ensure!(value.is_some(), Error::<T>::UnknownSubscription);
        Ok(())
    }

    fn claim_by_account(account_id: T::AccountId) -> DispatchResult {
        let current_position = ShareholderAccounts::<T>::get(&account_id);
        if current_position != fixed!(0) {
            // get definitions
            let incentives_asset_id = T::GetIncentiveAssetId::get();
            let tech_account_id = T::GetTechnicalAccountId::get();
            let claimable_incentives: FixedWrapper =
                assets::Module::<T>::free_balance(&incentives_asset_id, &tech_account_id)?.into();
            let shares_total = FixedWrapper::from(ClaimableShares::get());

            // clean up shares info
            ShareholderAccounts::<T>::mutate(&account_id, |current| *current = fixed!(0));
            ClaimableShares::mutate(|current| *current = current.csub(current_position).unwrap());

            // perform claimed tokens transfer
            let incentives_to_claim = FixedWrapper::from(current_position)
                / (shares_total / claimable_incentives.clone());
            let incentives_to_claim = incentives_to_claim
                .get()
                .map_err(|_| Error::CalculationError::<T>)?;

            let _result = Assets::<T>::transfer_from(
                &incentives_asset_id,
                &tech_account_id,
                &account_id,
                Balance(
                    incentives_to_claim.min(
                        // TODO: consider cases where this is bad, can it accumulate?
                        claimable_incentives
                            .get()
                            .map_err(|_| Error::CalculationError::<T>)?,
                    ),
                ),
            )?;
            Ok(())
        } else {
            fail!(Error::<T>::ZeroClaimableIncentives)
        }
    }

    /// Perform exchange of Base Asset to Incentive Asset.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
    /// - `dex_id`: Id of DEX to which given account belongs.
    fn exchange_fees_to_incentive(
        fees_account_id: &T::AccountId,
        dex_id: &T::DEXId,
    ) -> DispatchResult {
        let base_total = Assets::<T>::free_balance(&T::GetBaseAssetId::get(), &fees_account_id)?;
        if base_total == fixed!(0) {
            Self::deposit_event(RawEvent::NothingToExchange(
                dex_id.clone(),
                fees_account_id.clone(),
            ));
            return Ok(());
        }
        let outcome = T::LiquidityProxy::exchange(
            fees_account_id,
            fees_account_id,
            &T::GetBaseAssetId::get(),
            &T::GetIncentiveAssetId::get(),
            SwapAmount::with_desired_input(base_total.clone(), Balance(Fixed::ZERO)),
            LiquiditySourceFilter::with_allowed(
                dex_id.clone(),
                [LiquiditySourceType::XYKPool].into(),
            ),
        );
        match outcome {
            Ok(swap_outcome) => Self::deposit_event(RawEvent::FeesExchanged(
                dex_id.clone(),
                fees_account_id.clone(),
                T::GetBaseAssetId::get(),
                base_total,
                T::GetIncentiveAssetId::get(),
                swap_outcome.amount,
            )),
            // TODO: put error in event
            Err(_error) => Self::deposit_event(RawEvent::FeesExchangeFailed(
                dex_id.clone(),
                fees_account_id.clone(),
                T::GetBaseAssetId::get(),
                base_total,
                T::GetIncentiveAssetId::get(),
            )),
        }
        Ok(())
    }

    /// Perform distribution of Incentive Asset, i.e. transfer portions of accumulated Incentive Asset
    /// to shareholders according to amount of owned marker token.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
    /// - `dex_id`: Id of DEX to which given account belongs.
    /// - `marker_token_id`: Namely Pool Token, Asset Id by which shares of LP's are determined.
    /// - `tech_account_id`: Id of Account which holds permissions needed for mint/burn of arbitrary tokens, stores claimable incentives.
    fn distribute_incentive(
        fees_account_id: &T::AccountId,
        dex_id: &T::DEXId,
        marker_asset_id: &T::AssetId,
        tech_account_id: &T::AccountId,
    ) -> DispatchResult {
        // Get state of incentive availability and corresponding definitions.
        let incentive_asset_id = T::GetIncentiveAssetId::get();
        let marker_total = Assets::<T>::total_issuance(&marker_asset_id)?;
        let incentive_total = Assets::<T>::free_balance(&incentive_asset_id, &fees_account_id)?;
        if incentive_total == fixed!(0) {
            Self::deposit_event(RawEvent::NothingToDistribute(
                dex_id.clone(),
                fees_account_id.clone(),
            ));
            return Ok(());
        }

        // Adjust values and burn portion of incentive.
        let incentive_to_burn =
            FixedWrapper::from(incentive_total.clone()) * FixedWrapper::from(BurnRate::get());
        let incentive_to_revive =
            FixedWrapper::from(incentive_total.clone()) - incentive_to_burn.clone();
        assets::Module::<T>::burn_from(
            &incentive_asset_id,
            tech_account_id,
            fees_account_id,
            incentive_total,
        )?;

        let incentive_to_burn_fixed: Fixed = incentive_to_burn
            .clone()
            .get()
            .map_err(|_| Error::<T>::CalculationError)?;

        // This is needed for other pallet that will use this variables, for example this is
        // farming pallet.
        Self::deposit_event(RawEvent::IncentivesBurnedAfterExchange(
            dex_id.clone(),
            incentive_asset_id.clone(),
            incentive_total.clone(),
            incentive_to_burn_fixed.into(),
        ));

        // This is needed for farm id 0, now it is hardcoded, in future it will be resolved and
        // used in move convinient way.
        if incentive_asset_id.clone() == common::PSWAP.into() {
            let old = BurnedPswapDedicatedForOtherPallets::get();
            let new: Fixed = (old + incentive_to_burn.clone())
                .get()
                .map_err(|_| Error::<T>::CalculationError)?;
            BurnedPswapDedicatedForOtherPallets::set(new);
        }

        // Shadowing intended, re-mint decreased incentive amount and set it as new total.
        let incentive_total = Balance::from(
            incentive_to_revive
                .get()
                .map_err(|_| Error::<T>::CalculationError)?,
        );

        let incentive_to_burn_unwrapped: Balance = incentive_to_burn
            .get()
            .map_err(|_| Error::<T>::CalculationError)?
            .into();
        if !incentive_to_burn_unwrapped.is_zero() {
            T::OnPswapBurnedAggregator::on_pswap_burned(incentive_to_burn_unwrapped);
        }

        let Balance(mut claimable_incentives) =
            assets::Module::<T>::free_balance(&incentive_asset_id, &tech_account_id)?;

        // Distribute incentive to shareholders.
        let mut shareholders_num = 0u128;
        for (account_id, currency_id, data) in Accounts::<T>::iter() {
            if currency_id == into_currency!(T, marker_asset_id.clone()) && !data.free.is_zero() {
                let pool_tokens: T::CompatBalance = data.free.into();
                let share = FixedWrapper::from(pool_tokens.into())
                    / (FixedWrapper::from(marker_total) / FixedWrapper::from(incentive_total));

                let total_claimable_shares = ClaimableShares::get();
                let claimable_share = if total_claimable_shares == fixed!(0) {
                    share
                        .clone()
                        .get()
                        .map_err(|_| Error::<T>::CalculationError)?
                } else {
                    let claimable_share = share.clone()
                        / (FixedWrapper::from(claimable_incentives)
                            / FixedWrapper::from(total_claimable_shares));
                    claimable_share
                        .get()
                        .map_err(|_| Error::<T>::CalculationError)?
                };
                let claimable_share_delta =
                    if total_claimable_shares == fixed!(0) && claimable_incentives != fixed!(0) {
                        // this case is triggered when there is unowned incentives, first
                        // claim should posess it, but share needs to be corrected to avoid
                        // precision loss by following claims
                        (claimable_incentives.clone() + share)
                            .get()
                            .map_err(|_| Error::<T>::CalculationError)?
                    } else {
                        claimable_share
                    };
                ShareholderAccounts::<T>::mutate(&account_id, |current| {
                    *current = current.cadd(claimable_share_delta).unwrap()
                });
                ClaimableShares::mutate(|current| {
                    *current = current.cadd(claimable_share_delta).unwrap()
                });
                claimable_incentives = claimable_incentives
                    .cadd(claimable_share)
                    .map_err(|_| Error::<T>::CalculationError)?;
                shareholders_num += 1;
            }
        }

        assets::Module::<T>::mint_to(
            &incentive_asset_id,
            tech_account_id,
            tech_account_id,
            incentive_total,
        )?;

        // TODO: define condition on which IncentiveDistributionFailed event if applicable
        Self::deposit_event(RawEvent::IncentiveDistributed(
            dex_id.clone(),
            fees_account_id.clone(),
            incentive_asset_id,
            incentive_total,
            shareholders_num,
        ));
        Ok(())
    }

    pub fn incentive_distribution_routine(block_num: T::BlockNumber) {
        let tech_account_id = T::GetTechnicalAccountId::get();

        for (fees_account, (dex_id, pool_token, frequency, block_offset)) in
            SubscribedAccounts::<T>::iter()
        {
            if (block_num.saturating_sub(block_offset) % frequency).is_zero() {
                let _exchange_result = Self::exchange_fees_to_incentive(&fees_account, &dex_id);
                let _distribute_result = Self::distribute_incentive(
                    &fees_account,
                    &dex_id,
                    &pool_token,
                    &tech_account_id,
                );
            }
        }
    }

    fn update_burn_rate() {
        let mut burn_rate = BurnRate::get();
        let (increase_delta, max) = BurnUpdateInfo::get();
        if burn_rate < max {
            burn_rate = max.min(burn_rate.cadd(increase_delta).unwrap());
            BurnRate::mutate(|val| *val = burn_rate.clone());
            Self::deposit_event(RawEvent::BurnRateChanged(burn_rate))
        }
    }

    pub fn burn_rate_update_routine(block_num: T::BlockNumber) {
        if (block_num % BurnUpdateFrequency::<T>::get()).is_zero() {
            Self::update_burn_rate();
        }
    }
}
