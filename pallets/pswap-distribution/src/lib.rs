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
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use common::fixnum::ops::{CheckedAdd, CheckedSub};
use common::prelude::{Balance, FixedWrapper, SwapAmount};
use common::{
    fixed, fixed_wrapper, AccountIdOf, AssetInfoProvider, BuyBackHandler, DexInfoProvider,
    EnsureDEXManager, Fixed, LiquidityProxyTrait, LiquiditySourceFilter, LiquiditySourceType,
    OnPoolCreated, OnPswapBurned, PswapRemintInfo, XykPool,
};
use core::convert::TryInto;
use frame_support::dispatch::{DispatchError, DispatchResult, DispatchResultWithPostInfo, Weight};
use frame_support::traits::Get;
use frame_support::{ensure, fail};
use frame_system::ensure_signed;
use sp_arithmetic::traits::{Saturating, Zero};

pub mod weights;

pub mod migrations;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"pswap-distribution";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

type DexIdOf<T> = <T as common::Config>::DEXId;
type AssetIdOf<T> = <T as assets::Config>::AssetId;
type Assets<T> = assets::Pallet<T>;
type System<T> = frame_system::Pallet<T>;

pub use weights::WeightInfo;

#[derive(Default, Clone)]
pub struct DistributionWeightParams {
    pub skipped: u32,
    pub distributed: u32,
    pub shareholders: u32,
}

impl<T: Config> Pallet<T> {
    /// Check if given fees account is subscribed to incentive distribution.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
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
        pool_account: AccountIdOf<T>,
        frequency: Option<T::BlockNumber>,
    ) -> DispatchResult {
        ensure!(
            !Self::is_subscribed(&fees_account_id),
            Error::<T>::SubscriptionActive
        );
        let frequency = frequency.unwrap_or(T::GetDefaultSubscriptionFrequency::get());
        ensure!(!frequency.is_zero(), Error::<T>::InvalidFrequency);
        let current_block = System::<T>::block_number();
        frame_system::Pallet::<T>::inc_consumers(&fees_account_id)
            .map_err(|_| Error::<T>::IncRefError)?;
        SubscribedAccounts::<T>::insert(
            fees_account_id.clone(),
            (dex_id, pool_account, frequency, current_block),
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
    ///
    /// - `account_id`: Id of the account to query.
    pub fn claimable_amount(account_id: &T::AccountId) -> Result<Balance, DispatchError> {
        let current_position = ShareholderAccounts::<T>::get(&account_id);
        Ok(current_position
            .into_bits()
            .try_into()
            .map_err(|_| Error::<T>::CalculationError)?)
    }

    /// Perform claim of PSWAP by account, desired amount is not indicated - all available will be claimed.
    ///
    /// - `account_id`: Id of the account
    fn claim_by_account(account_id: &T::AccountId) -> DispatchResult {
        let current_position = ShareholderAccounts::<T>::get(&account_id);
        if current_position != fixed!(0) {
            ShareholderAccounts::<T>::mutate(&account_id, |current| *current = fixed!(0));
            ClaimableShares::<T>::mutate(|current| {
                *current = current.saturating_sub(current_position)
            });
            let incentives_asset_id = T::GetIncentiveAssetId::get();
            let tech_account_id = T::GetTechnicalAccountId::get();
            let _result = Assets::<T>::transfer_from(
                &incentives_asset_id,
                &tech_account_id,
                &account_id,
                current_position
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::CalculationError)?,
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
        dex_id: T::DEXId,
    ) -> DispatchResult {
        let dex_info = T::DexInfoProvider::get_dex_info(&dex_id)?;
        let base_chameleon_asset_id =
            <T::GetChameleonPoolBaseAssetId as traits::GetByKey<_, _>>::get(
                &dex_info.base_asset_id,
            );
        let base_total = <T as Config>::AssetInfoProvider::free_balance(
            &dex_info.base_asset_id,
            &fees_account_id,
        )?;
        let chameleon_total = if let Some(asset_id) = base_chameleon_asset_id {
            Assets::<T>::free_balance(&asset_id, &fees_account_id)?
        } else {
            0
        };
        if base_total == 0 && chameleon_total == 0 {
            Self::deposit_event(Event::<T>::NothingToExchange(
                dex_id.clone(),
                fees_account_id.clone(),
            ));
            return Ok(());
        }
        for (asset_id, balance) in Some((dex_info.base_asset_id, base_total))
            .into_iter()
            .chain(
                base_chameleon_asset_id
                    .map(|x| (x, chameleon_total))
                    .into_iter(),
            )
        {
            if balance == 0 {
                return Ok(());
            }
            let outcome = T::LiquidityProxy::exchange(
                dex_id,
                fees_account_id,
                fees_account_id,
                &asset_id,
                &T::GetIncentiveAssetId::get(),
                SwapAmount::with_desired_input(balance.clone(), Balance::zero()),
                LiquiditySourceFilter::with_allowed(
                    dex_id.clone(),
                    [LiquiditySourceType::XYKPool].into(),
                ),
            );
            match outcome {
                Ok(swap_outcome) => Self::deposit_event(Event::<T>::FeesExchanged(
                    dex_id.clone(),
                    fees_account_id.clone(),
                    asset_id,
                    balance,
                    T::GetIncentiveAssetId::get(),
                    swap_outcome.amount,
                )),
                Err(error) => Self::deposit_event(Event::<T>::FeesExchangeFailed(
                    dex_id.clone(),
                    fees_account_id.clone(),
                    asset_id,
                    balance,
                    T::GetIncentiveAssetId::get(),
                    error,
                )),
            }
        }
        Ok(())
    }

    /// Perform distribution of Incentive Asset, i.e. transfer portions of accumulated Incentive Asset
    /// to shareholders according to amount of owned marker token.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
    /// - `dex_id`: Id of DEX to which given account belongs.
    /// - `pool_account`: Pool account which stores reserves, used to identify pool and determine user liquidity share.
    /// - `tech_account_id`: Id of Account which holds permissions needed for mint/burn of arbitrary tokens, stores claimable incentives.
    fn distribute_incentive(
        fees_account_id: &T::AccountId,
        dex_id: &T::DEXId,
        pool_account: &AccountIdOf<T>,
        tech_account_id: &T::AccountId,
    ) -> Result<u32, DispatchError> {
        common::with_transaction(|| {
            // Get state of incentive availability and corresponding definitions.
            let incentive_asset_id = T::GetIncentiveAssetId::get();
            let pool_tokens_total = T::PoolXykPallet::total_issuance(&pool_account)?;
            let incentive_total = <T as Config>::AssetInfoProvider::free_balance(
                &incentive_asset_id,
                &fees_account_id,
            )?;
            if incentive_total == 0 || pool_tokens_total == 0 {
                Self::deposit_event(Event::<T>::NothingToDistribute(
                    dex_id.clone(),
                    fees_account_id.clone(),
                ));
                return Ok(0);
            }

            let mut distribution = Self::calculate_and_burn_distribution(
                fees_account_id,
                tech_account_id,
                &incentive_asset_id,
                incentive_total,
            )?;

            let mut shareholders_distributed_amount = fixed_wrapper!(0);

            // Distribute incentive to shareholders.
            let mut shareholders_num = 0u32;
            for (account_id, pool_tokens) in T::PoolXykPallet::pool_providers(pool_account) {
                let share = FixedWrapper::from(pool_tokens)
                    * FixedWrapper::from(distribution.liquidity_providers)
                    / FixedWrapper::from(pool_tokens_total);
                let share = share.get().map_err(|_| Error::<T>::CalculationError)?;

                ShareholderAccounts::<T>::mutate(&account_id, |current| {
                    *current = current.saturating_add(share)
                });
                ClaimableShares::<T>::mutate(|current| *current = current.saturating_add(share));
                shareholders_distributed_amount = shareholders_distributed_amount + share;

                shareholders_num += 1;
            }

            let undistributed_lp_amount = distribution.liquidity_providers.saturating_sub(
                shareholders_distributed_amount
                    .try_into_balance()
                    .map_err(|_| Error::<T>::CalculationError)?,
            );
            if undistributed_lp_amount > 0 {
                // utilize precision error from distribution calculation, so it won't accumulate on tech account
                distribution.liquidity_providers = distribution
                    .liquidity_providers
                    .saturating_sub(undistributed_lp_amount);
                distribution.buy_back_tbcd = distribution
                    .buy_back_tbcd
                    .saturating_add(undistributed_lp_amount);
            }

            assets::Pallet::<T>::mint_to(
                &incentive_asset_id,
                tech_account_id,
                tech_account_id,
                distribution.liquidity_providers,
            )?;

            T::BuyBackHandler::mint_buy_back_and_burn(
                &incentive_asset_id,
                &T::GetTBCDAssetId::get(),
                distribution.buy_back_tbcd,
            )?;

            Self::deposit_event(Event::<T>::IncentiveDistributed(
                dex_id.clone(),
                fees_account_id.clone(),
                incentive_asset_id,
                distribution.liquidity_providers,
                shareholders_num as u128,
            ));
            Ok(shareholders_num)
        })
    }

    /// Calculate actual incentive amounts regarding their destinations to be reminted.
    /// Only liquidity providers portion is reminted here,
    /// others are to be reminted in responsible pallets.
    ///
    /// - `fees_account_id`: Id of Account which accumulates fees from swaps.
    /// - `tech_account_id`: Id of Account which holds permissions needed for mint/burn of arbitrary tokens, stores claimable incentives.
    /// - `incentive_asset_id`: Incentive asset id.
    /// - `incentive_total`: total number of incentives to be distributed.
    fn calculate_and_burn_distribution(
        fees_account_id: &T::AccountId,
        tech_account_id: &T::AccountId,
        incentive_asset_id: &T::AssetId,
        incentive_total: Balance,
    ) -> Result<PswapRemintInfo, DispatchError> {
        let distribution = Self::calculate_pswap_distribution(incentive_total)?;
        assets::Pallet::<T>::burn_from(
            &incentive_asset_id,
            tech_account_id,
            fees_account_id,
            incentive_total,
        )?;
        T::OnPswapBurnedAggregator::on_pswap_burned(distribution.clone());
        Ok(distribution)
    }

    /// Calculates the amount of deposits to the incentive account.
    /// Used by `distribute_incentive` function.
    ///
    /// - `amount_burned`: Burned fees amount
    fn calculate_pswap_distribution(
        amount_burned: Balance,
    ) -> Result<PswapRemintInfo, DispatchError> {
        let amount_burned = FixedWrapper::from(amount_burned);
        // Calculate amount for parliament and actual remainder after its fraction.
        let amount_buy_back = (amount_burned.clone() * BuyBackTBCDFraction::<T>::get())
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;
        let mut amount_left = (amount_burned.clone() - amount_buy_back)
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;

        // Calculate amount for liquidity providers considering remaining amount.
        let fraction_lp = fixed_wrapper!(1) - BurnRate::<T>::get();
        let amount_lp = (FixedWrapper::from(amount_burned) * fraction_lp)
            .try_into_balance()
            .map_err(|_| Error::<T>::CalculationError)?;
        let amount_lp = amount_lp.min(amount_left);

        // Calculate amount for vesting from remaining amount.
        amount_left = amount_left.saturating_sub(amount_lp); // guaranteed to be >= 0
        let amount_vesting = amount_left.saturating_sub(T::PSWAP_BURN_PERCENT * amount_left); // 3% of vested PSWAP is burned without being reminted

        Ok(PswapRemintInfo {
            liquidity_providers: amount_lp,
            vesting: amount_vesting,
            buy_back_tbcd: amount_buy_back,
        })
    }

    /// Distributes incentives to all subscribed pools
    ///
    /// - `block_num`: The block number of the current chain head
    pub fn incentive_distribution_routine(block_num: T::BlockNumber) -> DistributionWeightParams {
        let tech_account_id = T::GetTechnicalAccountId::get();

        let mut weight_params = DistributionWeightParams::default();

        for (fees_account, (dex_id, pool_account, frequency, block_offset)) in
            SubscribedAccounts::<T>::iter()
        {
            if (block_num.saturating_sub(block_offset) % frequency).is_zero() {
                let _exchange_result = Self::exchange_fees_to_incentive(&fees_account, dex_id);
                // Revert storage state if distribution failed and try to distribute next time
                let distribute_result = common::with_transaction(|| {
                    Self::distribute_incentive(
                        &fees_account,
                        &dex_id,
                        &pool_account,
                        &tech_account_id,
                    )
                });
                match distribute_result {
                    Ok(shareholders) => {
                        weight_params.shareholders += shareholders;
                    }
                    Err(err) => {
                        frame_support::log::error!("Incentive distribution failed: {err:?}");
                        Self::deposit_event(Event::<T>::IncentiveDistributionFailed(
                            dex_id,
                            fees_account,
                        ));
                    }
                }
                weight_params.distributed += 1;
            } else {
                weight_params.skipped += 1;
            }
        }
        weight_params
    }

    /// Updates the fees' burn rate. Used in
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

impl<T: Config> OnPoolCreated for Pallet<T> {
    type AccountId = AccountIdOf<T>;

    type DEXId = DexIdOf<T>;

    fn on_pool_created(
        fee_account: Self::AccountId,
        dex_id: Self::DEXId,
        pool_account: Self::AccountId,
    ) -> DispatchResult {
        Self::subscribe(fee_account, dex_id, pool_account, None)
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::{
        AccountIdOf, AssetName, AssetSymbol, BalancePrecision, ContentSource, DEXInfo, Description,
        XykPool,
    };
    use frame_support::pallet_prelude::*;
    use frame_support::sp_runtime::Percent;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + common::Config + assets::Config + technical::Config
    {
        const PSWAP_BURN_PERCENT: Percent;
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type GetIncentiveAssetId: Get<Self::AssetId>;
        type GetTBCDAssetId: Get<Self::AssetId>;
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
        type PoolXykPallet: XykPool<Self::AccountId, Self::AssetId>;
        type BuyBackHandler: BuyBackHandler<Self::AccountId, Self::AssetId>;
        type DexInfoProvider: DexInfoProvider<Self::DEXId, DEXInfo<Self::AssetId>>;
        type GetChameleonPoolBaseAssetId: traits::GetByKey<Self::AssetId, Option<Self::AssetId>>;
        /// To retrieve asset info
        type AssetInfoProvider: AssetInfoProvider<
            Self::AssetId,
            Self::AccountId,
            AssetSymbol,
            AssetName,
            BalancePrecision,
            ContentSource,
            Description,
        >;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Perform exchange and distribution routines for all substribed accounts
        /// with respect to thir configured frequencies.
        fn on_initialize(block_num: T::BlockNumber) -> Weight {
            let weight_params = Self::incentive_distribution_routine(block_num);
            Self::burn_rate_update_routine(block_num);
            <T as Config>::WeightInfo::on_initialize(
                weight_params.skipped,
                weight_params.distributed,
                weight_params.shareholders,
            )
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::claim_incentive())]
        pub fn claim_incentive(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::claim_by_account(&who)?;
            Ok(().into())
        }
    }

    #[pallet::event]
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
        /// [DEX Id, Fees Account Id, Fees Asset Id, Available Fees Amount, Incentive Asset Id, Exchange error]
        FeesExchangeFailed(
            DexIdOf<T>,
            AccountIdOf<T>,
            AssetIdOf<T>,
            Balance,
            AssetIdOf<T>,
            DispatchError,
        ),
        /// Incentives successfully sent out to shareholders.
        /// [DEX Id, Fees Account Id, Incentive Asset Id, Incentive Total Distributed Amount, Number of shareholders]
        IncentiveDistributed(DexIdOf<T>, AccountIdOf<T>, AssetIdOf<T>, Balance, u128),
        /// Problem occurred that resulted in incentive distribution not done.
        /// [DEX Id, Fees Account Id]
        IncentiveDistributionFailed(DexIdOf<T>, AccountIdOf<T>),
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
        (T::DEXId, AccountIdOf<T>, T::BlockNumber, T::BlockNumber),
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
    pub(super) fn DefaultForBuyBackTBCDFraction() -> Fixed {
        fixed!(0.1)
    }

    /// Fraction of PSWAP that could be buy backed to TBCD
    #[pallet::storage]
    #[pallet::getter(fn buy_back_tbcd_fraction)]
    pub(super) type BuyBackTBCDFraction<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForBuyBackTBCDFraction>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// (Fees Account, (DEX Id, Pool Account Id, Distribution Frequency, Block Offset))
        pub subscribed_accounts: Vec<(
            T::AccountId,
            (DexIdOf<T>, AccountIdOf<T>, T::BlockNumber, T::BlockNumber),
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
                |(fees_account, (dex_id, pool_account, freq, block_offset))| {
                    frame_system::Pallet::<T>::inc_consumers(&fees_account).unwrap();
                    SubscribedAccounts::<T>::insert(
                        fees_account,
                        (dex_id, pool_account, freq, block_offset),
                    );
                },
            );
            let (initial_rate, increase_delta, max) = self.burn_info;
            BurnRate::<T>::mutate(|rate| *rate = initial_rate);
            BurnUpdateInfo::<T>::mutate(|info| *info = (increase_delta, max));
        }
    }
}
