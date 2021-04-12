// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use common::fixnum::ops::{CheckedAdd, CheckedSub};
use common::prelude::{Balance, FixedWrapper, SwapAmount};
use common::{
    fixed, fixed_wrapper, EnsureDEXManager, Fixed, LiquiditySourceFilter, LiquiditySourceType,
};
use frame_support::dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo, Weight};
use frame_support::traits::Get;
use frame_support::{ensure, fail, RuntimeDebug};
use frame_system::ensure_signed;
use liquidity_proxy::LiquidityProxyTrait;
use sp_arithmetic::traits::{Saturating, Zero};
use tokens::Accounts;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"pswap-distribution";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

type CurrencyIdOf<T> = <T as tokens::Config>::CurrencyId;
type DexIdOf<T> = <T as common::Config>::DEXId;
type AssetIdOf<T> = <T as assets::Config>::AssetId;
type Assets<T> = assets::Module<T>;
type System<T> = frame_system::Module<T>;

pub trait OnPswapBurned {
    fn on_pswap_burned(distribution: PswapRemintInfo);
}

impl OnPswapBurned for () {
    fn on_pswap_burned(_distribution: PswapRemintInfo) {
        // do nothing
    }
}

pub trait WeightInfo {
    fn claim_incentive() -> Weight;
    fn on_initialize(is_distributing: bool) -> Weight;
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Default)]
pub struct PswapRemintInfo {
    pub liquidity_providers: Balance,
    pub parliament: Balance,
    pub vesting: Balance,
}

macro_rules! into_currency {
    ($t:ty, $asset_id:expr) => {
        <<$t>::AssetId as Into<CurrencyIdOf<$t>>>::into($asset_id)
    };
}

impl<T: Config> Pallet<T> {
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
        frame_system::Pallet::<T>::inc_consumers(&fees_account_id)
            .map_err(|_| Error::<T>::IncRefError)?;
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
        frame_system::Pallet::<T>::dec_consumers(&fees_account_id);
        Ok(())
    }

    /// Query actual amount of PSWAP that can be claimed by account.
    pub fn claimable_amount(
        account_id: &T::AccountId,
    ) -> Result<(Balance, Balance, Fixed), DispatchError> {
        // get definitions
        let incentives_asset_id = T::GetIncentiveAssetId::get();
        let tech_account_id = T::GetTechnicalAccountId::get();
        let total_claimable =
            assets::Module::<T>::free_balance(&incentives_asset_id, &tech_account_id)?;
        let current_position = ShareholderAccounts::<T>::get(&account_id);
        if current_position == fixed!(0) {
            return Ok((Balance::zero(), total_claimable, current_position));
        }
        let shares_total = FixedWrapper::from(ClaimableShares::<T>::get());
        // perform claimed tokens transfer
        let incentives_to_claim =
            FixedWrapper::from(current_position) / (shares_total / total_claimable.clone());
        let incentives_to_claim = incentives_to_claim
            .try_into_balance()
            .map_err(|_| Error::CalculationError::<T>)?;
        Ok((incentives_to_claim, total_claimable, current_position))
    }

    /// Perform claim of PSWAP by account, desired amount is not indicated - all available will be claimed.
    fn claim_by_account(account_id: &T::AccountId) -> DispatchResult {
        let (incentives_to_claim, total_claimable, current_position) =
            Self::claimable_amount(account_id)?;
        if current_position != fixed!(0) {
            let claimable_amount_adjusted = incentives_to_claim.min(total_claimable);
            // clean up shares info
            ShareholderAccounts::<T>::mutate(&account_id, |current| *current = fixed!(0));
            ClaimableShares::<T>::mutate(|current| {
                *current = current.csub(current_position).unwrap()
            });
            let incentives_asset_id = T::GetIncentiveAssetId::get();
            let tech_account_id = T::GetTechnicalAccountId::get();
            let _result = Assets::<T>::transfer_from(
                &incentives_asset_id,
                &tech_account_id,
                &account_id,
                claimable_amount_adjusted,
            )?;
            Ok(().into())
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
        if base_total == 0 {
            Self::deposit_event(Event::<T>::NothingToExchange(
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
            SwapAmount::with_desired_input(base_total.clone(), Balance::zero()),
            LiquiditySourceFilter::with_allowed(
                dex_id.clone(),
                [LiquiditySourceType::XYKPool].into(),
            ),
        );
        match outcome {
            Ok(swap_outcome) => Self::deposit_event(Event::<T>::FeesExchanged(
                dex_id.clone(),
                fees_account_id.clone(),
                T::GetBaseAssetId::get(),
                base_total,
                T::GetIncentiveAssetId::get(),
                swap_outcome.amount,
            )),
            // TODO: put error in event
            Err(_error) => Self::deposit_event(Event::<T>::FeesExchangeFailed(
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
        if incentive_total == 0 {
            Self::deposit_event(Event::<T>::NothingToDistribute(
                dex_id.clone(),
                fees_account_id.clone(),
            ));
            return Ok(());
        }

        // Calculate actual amounts regarding their destinations to be reminted. Only liquidity providers portion is reminted here, others
        // are to be reminted in responsible pallets.
        let distribution = Self::calculate_pswap_distribution(incentive_total)?;
        // Burn all incentives.
        assets::Module::<T>::burn_from(
            &incentive_asset_id,
            tech_account_id,
            fees_account_id,
            incentive_total,
        )?;
        T::OnPswapBurnedAggregator::on_pswap_burned(distribution.clone());

        let mut claimable_incentives = FixedWrapper::from(assets::Module::<T>::free_balance(
            &incentive_asset_id,
            &tech_account_id,
        )?);

        // Distribute incentive to shareholders.
        let mut shareholders_num = 0u128;
        for (account_id, currency_id, data) in Accounts::<T>::iter() {
            if currency_id == into_currency!(T, marker_asset_id.clone()) && !data.free.is_zero() {
                let pool_tokens: T::CompatBalance = data.free.into();
                let share = FixedWrapper::from(pool_tokens.into())
                    / (FixedWrapper::from(marker_total)
                        / FixedWrapper::from(distribution.liquidity_providers));

                let total_claimable_shares = ClaimableShares::<T>::get();
                let claimable_share = if total_claimable_shares == fixed!(0) {
                    share
                        .clone()
                        .get()
                        .map_err(|_| Error::<T>::CalculationError)?
                } else {
                    let claimable_share = share.clone()
                        / (claimable_incentives.clone()
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
                ClaimableShares::<T>::mutate(|current| {
                    *current = current.cadd(claimable_share_delta).unwrap()
                });
                claimable_incentives = claimable_incentives + claimable_share;
                shareholders_num += 1;
            }
        }

        assets::Module::<T>::mint_to(
            &incentive_asset_id,
            tech_account_id,
            tech_account_id,
            distribution.liquidity_providers,
        )?;

        assets::Module::<T>::mint_to(
            &incentive_asset_id,
            tech_account_id,
            &T::GetParliamentAccountId::get(),
            distribution.parliament,
        )?;

        // TODO: define condition on which IncentiveDistributionFailed event if applicable
        Self::deposit_event(Event::<T>::IncentiveDistributed(
            dex_id.clone(),
            fees_account_id.clone(),
            incentive_asset_id,
            distribution.liquidity_providers,
            shareholders_num,
        ));
        Ok(())
    }

    fn calculate_pswap_distribution(
        amount_burned: Balance,
    ) -> Result<PswapRemintInfo, DispatchError> {
        let amount_burned = FixedWrapper::from(amount_burned);
        // Calculate amount for parliament and actual remainder after its fraction.
        let amount_parliament = (amount_burned.clone() * ParliamentPswapFraction::<T>::get())
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;
        let amount_left = (amount_burned.clone() - amount_parliament)
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;

        // Calculate amount for liquidity providers considering remaining amount.
        let fraction_lp = fixed_wrapper!(1) - BurnRate::<T>::get();
        let amount_lp = (FixedWrapper::from(amount_burned) * fraction_lp)
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;
        let amount_lp = amount_lp.min(amount_left);

        // Calculate amount for vesting from remaining amount.
        let amount_vesting = amount_left.saturating_sub(amount_lp); // guaranteed to be >= 0

        Ok(PswapRemintInfo {
            liquidity_providers: amount_lp,
            vesting: amount_vesting,
            parliament: amount_parliament,
        })
    }

    pub fn incentive_distribution_routine(block_num: T::BlockNumber) -> bool {
        let tech_account_id = T::GetTechnicalAccountId::get();

        let mut distributing_count = 0;

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
                distributing_count += 1;
            }
        }
        distributing_count > 0
    }

    fn update_burn_rate() {
        let mut burn_rate = BurnRate::<T>::get();
        let (increase_delta, max) = BurnUpdateInfo::<T>::get();
        if burn_rate < max {
            burn_rate = max.min(burn_rate.cadd(increase_delta).unwrap());
            BurnRate::<T>::mutate(|val| *val = burn_rate.clone());
            Self::deposit_event(Event::<T>::BurnRateChanged(burn_rate))
        }
    }

    pub fn burn_rate_update_routine(block_num: T::BlockNumber) {
        if (block_num % T::GetBurnUpdateFrequency::get()).is_zero() {
            Self::update_burn_rate();
        }
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::AccountIdOf;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + assets::Config + technical::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type GetIncentiveAssetId: Get<Self::AssetId>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
        type CompatBalance: From<<Self as tokens::Config>::Balance>
            + Into<Balance>
            + From<Balance>
            + Clone
            + Zero;
        type GetTechnicalAccountId: Get<Self::AccountId>;
        type GetDefaultSubscriptionFrequency: Get<Self::BlockNumber>;
        type GetBurnUpdateFrequency: Get<Self::BlockNumber>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type OnPswapBurnedAggregator: OnPswapBurned;
        type WeightInfo: WeightInfo;
        type GetParliamentAccountId: Get<Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Perform exchange and distribution routines for all substribed accounts
        /// with respect to thir configured frequencies.
        fn on_initialize(block_num: T::BlockNumber) -> Weight {
            let is_distributing = Self::incentive_distribution_routine(block_num);
            Self::burn_rate_update_routine(block_num);

            <T as Config>::WeightInfo::on_initialize(is_distributing)
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn claim_incentive(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::claim_by_account(&who)?;
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId", AssetIdOf<T> = "AssetId", DexIdOf<T> = "DEXId")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Fees successfully exchanged for appropriate amount of pool tokens.
        /// [DEX Id, Fees Account Id, Fees Asset Id, Fees Spent Amount, Incentive Asset Id, Incentive Received Amount]
        FeesExchanged(
            DexIdOf<T>,
            AccountIdOf<T>,
            AssetIdOf<T>,
            Balance,
            AssetIdOf<T>,
            Balance,
        ),
        /// Problem occurred that resulted in fees exchange not done.
        /// [DEX Id, Fees Account Id, Fees Asset Id, Available Fees Amount, Incentive Asset Id]
        FeesExchangeFailed(
            DexIdOf<T>,
            AccountIdOf<T>,
            AssetIdOf<T>,
            Balance,
            AssetIdOf<T>,
        ),
        /// Incentives successfully sent out to shareholders.
        /// [DEX Id, Fees Account Id, Incentive Asset Id, Incentive Total Distributed Amount, Number of shareholders]
        IncentiveDistributed(DexIdOf<T>, AccountIdOf<T>, AssetIdOf<T>, Balance, u128),
        /// Problem occurred that resulted in incentive distribution not done.
        /// [DEX Id, Fees Account Id, Incentive Asset Id, Available Incentive Amount]
        IncentiveDistributionFailed(DexIdOf<T>, AccountIdOf<T>, AssetIdOf<T>, Balance),
        /// Burn rate updated.
        /// [Current Burn Rate]
        BurnRateChanged(Fixed),
        /// Fees Account contains zero base tokens, thus exchange is dismissed.
        /// [DEX Id, Fees Account Id]
        NothingToExchange(DexIdOf<T>, AccountIdOf<T>),
        /// Fees Account contains zero incentive tokens, thus distribution is dismissed.
        /// [DEX Id, Fees Account Id]
        NothingToDistribute(DexIdOf<T>, AccountIdOf<T>),
        /// This is needed for other pallet that will use this variables, for example this is
        /// farming pallet.
        /// [DEX Id, Incentive Asset Id, Total exchanged incentives (Incentives burned after exchange),
        /// Incentives burned (Incentives that is not revived (to burn)]).
        IncentivesBurnedAfterExchange(DexIdOf<T>, AssetIdOf<T>, Balance, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
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
        /// Increment account reference error.
        IncRefError,
    }

    /// Store for information about accounts containing fees, that participate in incentive distribution mechanism.
    /// Fees Account Id -> (DEX Id, Pool Marker Asset Id, Distribution Frequency, Block Offset) Frequency MUST be non-zero.
    #[pallet::storage]
    #[pallet::getter(fn subscribed_accounts)]
    pub type SubscribedAccounts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        (T::DEXId, T::AssetId, T::BlockNumber, T::BlockNumber),
    >;

    /// Amount of incentive tokens to be burned on each distribution.
    #[pallet::storage]
    #[pallet::getter(fn burn_rate)]
    pub type BurnRate<T: Config> = StorageValue<_, Fixed, ValueQuery>;

    /// (Burn Rate Increase Delta, Burn Rate Max)
    #[pallet::storage]
    #[pallet::getter(fn burn_update_info)]
    pub(super) type BurnUpdateInfo<T: Config> = StorageValue<_, (Fixed, Fixed), ValueQuery>;

    /// Information about owned portion of stored incentive tokens. Shareholder -> Owned Fraction
    #[pallet::storage]
    #[pallet::getter(fn shareholder_accounts)]
    pub type ShareholderAccounts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Fixed, ValueQuery>;

    /// Sum of all shares of incentive token owners.
    #[pallet::storage]
    #[pallet::getter(fn claimable_shares)]
    pub type ClaimableShares<T: Config> = StorageValue<_, Fixed, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn DefaultForParliamentPswapFraction() -> Fixed {
        fixed!(0.1)
    }

    /// Fraction of PSWAP that could be reminted for parliament.
    #[pallet::storage]
    #[pallet::getter(fn parliament_pswap_fraction)]
    pub(super) type ParliamentPswapFraction<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForParliamentPswapFraction>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// (Fees Account, (DEX Id, Marker Token Id, Distribution Frequency, Block Offset))
        pub subscribed_accounts: Vec<(
            T::AccountId,
            (DexIdOf<T>, AssetIdOf<T>, T::BlockNumber, T::BlockNumber),
        )>,
        /// (Initial Burn Rate, Burn Rate Increase Delta, Burn Rate Max)
        pub burn_info: (Fixed, Fixed, Fixed),
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                subscribed_accounts: Default::default(),
                burn_info: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            self.subscribed_accounts.iter().for_each(
                |(fees_account, (dex_id, pool_asset, freq, block_offset))| {
                    frame_system::Pallet::<T>::inc_consumers(&fees_account).unwrap();
                    SubscribedAccounts::<T>::insert(
                        fees_account,
                        (dex_id, pool_asset, freq, block_offset),
                    );
                },
            );
            let (initial_rate, increase_delta, max) = self.burn_info;
            BurnRate::<T>::mutate(|rate| *rate = initial_rate);
            BurnUpdateInfo::<T>::mutate(|info| *info = (increase_delta, max));
        }
    }
}
