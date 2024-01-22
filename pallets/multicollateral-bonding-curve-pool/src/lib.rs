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

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

use core::convert::TryInto;

use assets::AssetIdOf;
use codec::{Decode, Encode};
use common::fixnum::ops::Zero as _;
use common::prelude::{
    Balance, EnsureDEXManager, EnsureTradingPairExists, Fixed, FixedWrapper, PriceToolsPallet,
    QuoteAmount, SwapAmount, SwapOutcome,
};
use common::{
    balance, fixed, fixed_wrapper, AssetInfoProvider, DEXId, DexIdOf, GetMarketInfo,
    LiquidityProxyTrait, LiquiditySource, LiquiditySourceFilter, LiquiditySourceQuoteError,
    LiquiditySourceType, ManagementMode, PriceVariant, RewardReason, TradingPairSourceManager,
    VestedRewardsPallet, PSWAP, TBCD, VAL, XOR, XST,
};
use common::{BuyBackHandler, LiquidityProxyError};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_support::{ensure, fail};
use permissions::{Scope, BURN, MINT};
use sp_arithmetic::traits::Zero;
use sp_runtime::{DispatchError, DispatchResult};
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec::Vec;
pub use weights::WeightInfo;
#[cfg(feature = "std")]
use {
    common::USDT,
    serde::{Deserialize, Serialize},
};

pub mod migrations;

type Assets<T> = assets::Pallet<T>;
type Technical<T> = technical::Pallet<T>;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"multicollateral-bonding-curve-pool";
pub const TECH_ACCOUNT_RESERVES: &[u8] = b"reserves";
pub const TECH_ACCOUNT_REWARDS: &[u8] = b"rewards";
pub const TECH_ACCOUNT_FREE_RESERVES: &[u8] = b"free_reserves";

pub const RETRY_DISTRIBUTION_FREQUENCY: u32 = 1000;

pub use pallet::*;

#[derive(Debug, Encode, Decode, Clone, PartialEq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum DistributionAccount<AccountId, TechAccountId> {
    Account(AccountId),
    TechAccount(TechAccountId),
}

impl<AccountId, TechAccountId: Default> Default for DistributionAccount<AccountId, TechAccountId> {
    fn default() -> Self {
        Self::TechAccount(TechAccountId::default())
    }
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DistributionAccountData<DistributionAccount> {
    pub account: DistributionAccount,
    pub coefficient: Fixed,
}

impl<DistributionAccount: Default> Default for DistributionAccountData<DistributionAccount> {
    fn default() -> Self {
        Self {
            account: Default::default(),
            coefficient: Default::default(),
        }
    }
}

impl<DistributionAccount> DistributionAccountData<DistributionAccount> {
    pub fn new(account: DistributionAccount, coefficient: Fixed) -> Self {
        DistributionAccountData {
            account,
            coefficient,
        }
    }
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DistributionAccounts<DistributionAccountData> {
    pub xor_allocation: DistributionAccountData,
    pub val_holders: DistributionAccountData,
    pub sora_citizens: DistributionAccountData,
    pub stores_and_shops: DistributionAccountData,
    pub projects: DistributionAccountData,
}

impl<AccountId, TechAccountId>
    DistributionAccounts<DistributionAccountData<DistributionAccount<AccountId, TechAccountId>>>
{
    pub fn xor_distribution_as_array(
        &self,
    ) -> [&DistributionAccountData<DistributionAccount<AccountId, TechAccountId>>; 3] {
        [&self.sora_citizens, &self.stores_and_shops, &self.projects]
    }

    pub fn xor_distribution_accounts_as_array(
        &self,
    ) -> [&DistributionAccount<AccountId, TechAccountId>; 3] {
        [
            &self.sora_citizens.account,
            &self.stores_and_shops.account,
            &self.projects.account,
        ]
    }

    pub fn accounts(&self) -> [&DistributionAccount<AccountId, TechAccountId>; 5] {
        [
            &self.xor_allocation.account,
            &self.val_holders.account,
            &self.sora_citizens.account,
            &self.stores_and_shops.account,
            &self.projects.account,
        ]
    }
}

impl<DistributionAccountData: Default> Default for DistributionAccounts<DistributionAccountData> {
    fn default() -> Self {
        Self {
            xor_allocation: Default::default(),
            val_holders: Default::default(),
            sora_citizens: Default::default(),
            stores_and_shops: Default::default(),
            projects: Default::default(),
        }
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::VestedRewardsPallet;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::ensure_root;
    use frame_system::pallet_prelude::*;

    // TODO: #395 use AssetInfoProvider instead of assets pallet
    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + common::Config
        + assets::Config
        + technical::Config
        + pool_xyk::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type LiquidityProxy: LiquidityProxyTrait<Self::DEXId, Self::AccountId, Self::AssetId>;
        type EnsureDEXManager: EnsureDEXManager<Self::DEXId, Self::AccountId, DispatchError>;
        type EnsureTradingPairExists: EnsureTradingPairExists<
            Self::DEXId,
            Self::AssetId,
            DispatchError,
        >;
        type PriceToolsPallet: PriceToolsPallet<Self::AssetId>;
        type VestedRewardsPallet: VestedRewardsPallet<Self::AccountId, Self::AssetId>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, Self::AssetId>;
        type BuyBackHandler: BuyBackHandler<Self::AccountId, Self::AssetId>;
        type BuyBackTBCDPercent: Get<Fixed>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(3);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            if (block_number % RETRY_DISTRIBUTION_FREQUENCY.into()).is_zero() {
                let elems = Pallet::<T>::free_reserves_distribution_routine().unwrap_or_default();
                <T as Config>::WeightInfo::on_initialize(elems)
            } else {
                <T as Config>::WeightInfo::on_initialize(0)
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Enable exchange path on the pool for pair BaseAsset-CollateralAsset.
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::initialize_pool())]
        pub fn initialize_pool(
            origin: OriginFor<T>,
            collateral_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let _who = <T as Config>::EnsureDEXManager::ensure_can_manage(
                &DEXId::Polkaswap.into(),
                origin,
                ManagementMode::Private,
            )?;
            Self::initialize_pool_unchecked(collateral_asset_id, true)?;
            Ok(().into())
        }

        /// Change reference asset which is used to determine collateral assets value. Inteded to be e.g. stablecoin DAI.
        #[pallet::call_index(1)]
        #[pallet::weight(<T as Config>::WeightInfo::set_reference_asset())]
        pub fn set_reference_asset(
            origin: OriginFor<T>,
            reference_asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let _who = <T as Config>::EnsureDEXManager::ensure_can_manage(
                &DEXId::Polkaswap.into(),
                origin,
                ManagementMode::Private,
            )?;
            ReferenceAssetId::<T>::put(reference_asset_id.clone());
            Self::deposit_event(Event::ReferenceAssetChanged(reference_asset_id));
            Ok(().into())
        }

        /// Set multiplier which is applied to rewarded amount when depositing particular collateral assets.
        /// `None` value indicates reward without change, same as Some(1.0).
        #[pallet::call_index(2)]
        #[pallet::weight(<T as Config>::WeightInfo::set_optional_reward_multiplier())]
        pub fn set_optional_reward_multiplier(
            origin: OriginFor<T>,
            collateral_asset_id: T::AssetId,
            multiplier: Option<Fixed>,
        ) -> DispatchResultWithPostInfo {
            let _who = <T as Config>::EnsureDEXManager::ensure_can_manage(
                &DEXId::Polkaswap.into(),
                origin,
                ManagementMode::Private,
            )?;
            ensure!(
                Self::enabled_targets().contains(&collateral_asset_id),
                Error::<T>::UnsupportedCollateralAssetId
            );
            // NOTE: not using insert() here because it unwraps Option, which is not intended
            AssetsWithOptionalRewardMultiplier::<T>::mutate(&collateral_asset_id, |opt| {
                *opt = multiplier.clone()
            });
            Self::deposit_event(Event::OptionalRewardMultiplierUpdated(
                collateral_asset_id,
                multiplier,
            ));
            Ok(().into())
        }

        /// Changes `initial_price` used as bias in XOR-DAI(reference asset) price calculation
        #[pallet::call_index(3)]
        #[pallet::weight(< T as Config >::WeightInfo::set_price_bias())]
        pub fn set_price_bias(
            origin: OriginFor<T>,
            price_bias: Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            InitialPrice::<T>::put(
                FixedWrapper::from(price_bias)
                    .get()
                    .map_err(|_| Error::<T>::ArithmeticError)?,
            );

            Self::deposit_event(Event::PriceBiasChanged(price_bias));
            Ok(().into())
        }

        /// Changes price change rate and step
        #[pallet::call_index(4)]
        #[pallet::weight(< T as Config >::WeightInfo::set_price_change_config())]
        pub fn set_price_change_config(
            origin: OriginFor<T>,
            price_change_rate: Balance,
            price_change_step: Balance,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            PriceChangeRate::<T>::put(
                FixedWrapper::from(price_change_rate)
                    .get()
                    .map_err(|_| Error::<T>::ArithmeticError)?,
            );
            PriceChangeStep::<T>::put(
                FixedWrapper::from(price_change_step)
                    .get()
                    .map_err(|_| Error::<T>::ArithmeticError)?,
            );
            Self::deposit_event(Event::PriceChangeConfigChanged(
                price_change_rate,
                price_change_step,
            ));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Pool is initialized for pair. [DEX Id, Collateral Asset Id]
        PoolInitialized(DexIdOf<T>, AssetIdOf<T>),
        /// Reference Asset has been changed for pool. [New Reference Asset Id]
        ReferenceAssetChanged(AssetIdOf<T>),
        /// Multiplier for reward has been updated on particular asset. [Asset Id, New Multiplier]
        OptionalRewardMultiplierUpdated(AssetIdOf<T>, Option<Fixed>),
        /// Price bias was changed. [New Price Bias]
        PriceBiasChanged(Balance),
        /// Price change config was changed. [New Price Change Rate, New Price Change Step]
        PriceChangeConfigChanged(Balance, Balance),
    }

    #[pallet::error]
    pub enum Error<T> {
        /// An error occurred while calculating the price.
        PriceCalculationFailed,
        /// Failure while calculating price ignoring non-linearity of liquidity source.
        FailedToCalculatePriceWithoutImpact,
        /// The pool can't perform exchange on itself.
        CannotExchangeWithSelf,
        /// It's not enough reserves in the pool to perform the operation.
        NotEnoughReserves,
        /// Attempt to initialize pool for pair that already exists.
        PoolAlreadyInitializedForPair,
        /// Attempt to get info for uninitialized pool.
        PoolNotInitialized,
        /// Indicated limits for slippage has not been met during transaction execution.
        SlippageLimitExceeded,
        /// Either user has no pending rewards or current limit is exceeded at the moment.
        NothingToClaim,
        /// User has pending reward, but rewards supply is insufficient at the moment.
        RewardsSupplyShortage,
        /// Indicated collateral asset is not enabled for pool.
        UnsupportedCollateralAssetId,
        /// Could not calculate fee including sell penalty.
        FeeCalculationFailed,
        /// Liquidity source can't exchange assets with the given IDs on the given DEXId.
        CantExchange,
        /// Increment account reference error.
        IncRefError,
        /// An error occured during balance type conversion.
        ArithmeticError,
        /// Free reserves account is not set
        FreeReservesAccountNotSet,
    }

    /// Technical account used to store collateral tokens.
    #[pallet::storage]
    #[pallet::getter(fn reserves_account_id)]
    pub type ReservesAcc<T: Config> = StorageValue<_, T::TechAccountId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn free_reserves_account_id)]
    pub type FreeReservesAccountId<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_free_reserves)]
    pub type PendingFreeReserves<T: Config> =
        StorageValue<_, Vec<(T::AssetId, Balance)>, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn DefaultForInitialPrice() -> Fixed {
        fixed!(634)
    }

    /// Buy price starting constant. This is the price users pay for new XOR.
    #[pallet::storage]
    #[pallet::getter(fn initial_price)]
    pub(super) type InitialPrice<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForInitialPrice>;

    #[pallet::type_value]
    pub(super) fn DefaultForPriceChangeStep() -> Fixed {
        fixed!(1337)
    }

    /// Cofficients in buy price function.
    #[pallet::storage]
    #[pallet::getter(fn price_change_step)]
    pub(super) type PriceChangeStep<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForPriceChangeStep>;

    #[pallet::type_value]
    pub(super) fn DefaultForPriceChangeRate() -> Fixed {
        fixed!(1)
    }

    #[pallet::storage]
    #[pallet::getter(fn price_change_rate)]
    pub(super) type PriceChangeRate<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForPriceChangeRate>;

    #[pallet::type_value]
    pub(super) fn DefaultForSellPriceCoefficient() -> Fixed {
        fixed!(0.8)
    }

    /// Sets the sell function as a fraction of the buy function, so there is margin between the two functions.
    #[pallet::storage]
    #[pallet::getter(fn sell_price_coefficient)]
    pub(super) type SellPriceCoefficient<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForSellPriceCoefficient>;

    #[pallet::type_value]
    pub(super) fn DefaultForAlwaysDistributeCoefficient() -> Fixed {
        fixed!(0.2)
    }

    /// Coefficient which determines the fraction of input collateral token to be exchanged to XOR and
    /// be distributed to predefined accounts. Relevant for the Buy function (when a user buys new XOR).
    #[pallet::storage]
    #[pallet::getter(fn always_distribute_coefficient)]
    pub(super) type AlwaysDistributeCoefficient<T: Config> =
        StorageValue<_, Fixed, ValueQuery, DefaultForAlwaysDistributeCoefficient>;

    #[pallet::type_value]
    pub(super) fn DefaultForBaseFee() -> Fixed {
        fixed!(0.003)
    }

    /// Base fee in XOR which is deducted on all trades, currently it's burned: 0.3%
    #[pallet::storage]
    #[pallet::getter(fn base_fee)]
    pub(super) type BaseFee<T: Config> = StorageValue<_, Fixed, ValueQuery, DefaultForBaseFee>;

    /// Accounts that receive 20% buy/sell margin according to predefined proportions
    #[pallet::storage]
    #[pallet::getter(fn distribution_accounts)]
    pub(super) type DistributionAccountsEntry<T: Config> = StorageValue<
        _,
        DistributionAccounts<
            DistributionAccountData<DistributionAccount<T::AccountId, T::TechAccountId>>,
        >,
        ValueQuery,
    >;

    /// Collateral Assets allowed to be sold by the token bonding curve
    #[pallet::storage]
    #[pallet::getter(fn enabled_targets)]
    pub type EnabledTargets<T: Config> = StorageValue<_, BTreeSet<T::AssetId>, ValueQuery>;

    /// Asset that is used to compare collateral assets by value, e.g., DAI
    #[pallet::storage]
    #[pallet::getter(fn reference_asset_id)]
    pub type ReferenceAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

    /// Registry to store information about rewards owned by users in PSWAP. (claim_limit, available_rewards)
    #[pallet::storage]
    #[pallet::getter(fn rewards)]
    pub type Rewards<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (Balance, Balance), ValueQuery>;

    /// Total amount of PSWAP owned by accounts
    #[pallet::storage]
    #[pallet::getter(fn total_rewards)]
    pub type TotalRewards<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// Number of reserve currencies selling which user will get rewards, namely all registered collaterals except PSWAP and VAL
    #[pallet::storage]
    #[pallet::getter(fn incentivised_currencies_num)]
    pub type IncentivisedCurrenciesNum<T: Config> = StorageValue<_, u32, ValueQuery>;

    /// Account which stores actual PSWAP intended for rewards
    #[pallet::storage]
    #[pallet::getter(fn incentives_account_id)]
    pub type IncentivesAccountId<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Reward multipliers for special assets. Asset Id => Reward Multiplier
    #[pallet::storage]
    pub type AssetsWithOptionalRewardMultiplier<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, Fixed>;

    #[pallet::type_value]
    pub(super) fn DefaultForInitialPswapRewardsSupply() -> Balance {
        balance!(2500000000)
    }

    /// Amount of PSWAP initially stored in account dedicated for TBC rewards. Actual account balance will deplete over time,
    /// however this constant is not modified
    #[pallet::storage]
    pub(super) type InitialPswapRewardsSupply<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultForInitialPswapRewardsSupply>;

    /// Current reserves balance for collateral tokens, used for client usability.
    #[pallet::storage]
    pub(super) type CollateralReserves<T: Config> =
        StorageMap<_, Twox64Concat, T::AssetId, Balance, ValueQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        /// Technical account used to store collateral tokens.
        pub reserves_account_id: T::TechAccountId,
        /// Accounts that receive 20% buy/sell margin according predefined proportions.
        pub distribution_accounts: DistributionAccounts<
            DistributionAccountData<DistributionAccount<T::AccountId, T::TechAccountId>>,
        >,
        /// Asset that is used to compare collateral assets by value, e.g., DAI.
        pub reference_asset_id: T::AssetId,
        /// Account which stores actual PSWAP intended for rewards.
        pub incentives_account_id: Option<T::AccountId>,
        /// List of tokens enabled as collaterals initially.
        pub initial_collateral_assets: Vec<T::AssetId>,
        /// Account that is used to store undistributed free reserves.
        pub free_reserves_account_id: Option<T::AccountId>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                reserves_account_id: Default::default(),
                distribution_accounts: Default::default(),
                reference_asset_id: USDT.into(),
                incentives_account_id: Default::default(),
                initial_collateral_assets: [USDT.into(), VAL.into(), PSWAP.into()].into(),
                free_reserves_account_id: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            frame_system::Pallet::<T>::inc_consumers(&self.incentives_account_id.as_ref().unwrap())
                .unwrap();
            ReservesAcc::<T>::put(&self.reserves_account_id);
            DistributionAccountsEntry::<T>::put(&self.distribution_accounts);
            ReferenceAssetId::<T>::put(&self.reference_asset_id);
            IncentivesAccountId::<T>::put(&self.incentives_account_id.as_ref().unwrap());
            FreeReservesAccountId::<T>::put(&self.free_reserves_account_id.as_ref().unwrap());
            self.initial_collateral_assets
                .iter()
                .cloned()
                .for_each(|asset_id| {
                    Pallet::<T>::initialize_pool_unchecked(asset_id, false)
                        .expect("Failed to initialize bonding curve.")
                });
        }
    }
}

/// This function is used by `exchange` function to transfer calculated `input_amount` of
/// `in_asset_id` to reserves and mint `output_amount` of `out_asset_id`.
///
/// This function buys XOR; it always distributes a portion of input tokens (see `AlwaysDistributeCoefficient`), these are
/// referred as free reserves. After collateral input portion is exchanged to XOR, it's sent out to accounts
/// specified in `DistributionAccounts` struct and buy-back and burn some amount of VAL asset.
struct BuyMainAsset<T: Config> {
    collateral_asset_id: T::AssetId,
    main_asset_id: T::AssetId,
    amount: SwapAmount<Balance>,
    from_account_id: T::AccountId,
    to_account_id: T::AccountId,
    reserves_tech_account_id: T::TechAccountId,
    reserves_account_id: T::AccountId,
}

impl<T: Config> BuyMainAsset<T> {
    pub fn new(
        collateral_asset_id: T::AssetId,
        main_asset_id: T::AssetId,
        amount: SwapAmount<Balance>,
        from_account_id: T::AccountId,
        to_account_id: T::AccountId,
    ) -> Result<Self, DispatchError> {
        let reserves_tech_account_id = ReservesAcc::<T>::get();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        Ok(BuyMainAsset {
            collateral_asset_id,
            main_asset_id,
            amount,
            from_account_id,
            to_account_id,
            reserves_tech_account_id,
            reserves_account_id,
        })
    }

    /// Make transfer from user account to collateral reserves account.
    fn deposit_input(&self, input_amount: Balance) -> Result<(), DispatchError> {
        Technical::<T>::transfer_in(
            &self.collateral_asset_id,
            &self.from_account_id,
            &self.reserves_tech_account_id,
            input_amount,
        )?;

        Ok(())
    }

    /// Assets deposition algorithm:
    ///
    /// ```nocompile
    /// free_reserves := input_amount * free_amount_coeffecient
    /// new_reserves := current_reserves + input_amount - free_reserves
    /// ```
    fn distribute_reserves(&self, input_amount: Balance) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            let free_amount = FixedWrapper::from(input_amount)
                * FixedWrapper::from(AlwaysDistributeCoefficient::<T>::get());
            let free_amount = free_amount
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
            if free_amount == Balance::zero() {
                return Ok(());
            }

            if !Pallet::<T>::attempt_free_reserves_distribution(
                &self.reserves_account_id,
                &self.collateral_asset_id,
                free_amount,
            )
            .is_ok()
            {
                Pallet::<T>::add_free_reserves_to_pending_list(
                    &self.reserves_account_id,
                    self.collateral_asset_id.clone(),
                    free_amount,
                )?;
            }
            Ok(())
        })
    }

    fn mint_output(&self, output_amount: Balance) -> Result<(), DispatchError> {
        Assets::<T>::mint_to(
            &self.main_asset_id,
            &self.reserves_account_id,
            &self.to_account_id,
            output_amount,
        )?;
        Ok(())
    }

    /// Calculate and assign PSWAP reward for buying XOR with particular assets
    fn update_reward(
        &self,
        collateral_asset_amount: Balance,
        main_asset_amount: Balance,
    ) -> Result<(), DispatchError> {
        let mut pswap_amount = Pallet::<T>::calculate_buy_reward(
            &self.reserves_account_id,
            &self.collateral_asset_id,
            collateral_asset_amount,
            main_asset_amount,
        )?;
        if let Some(multiplier) =
            AssetsWithOptionalRewardMultiplier::<T>::get(&self.collateral_asset_id)
        {
            pswap_amount = (FixedWrapper::from(pswap_amount) * multiplier)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        }
        if !pswap_amount.is_zero() {
            T::VestedRewardsPallet::add_tbc_reward(&self.from_account_id, pswap_amount)?;
        }
        Ok(())
    }

    fn swap(&self) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let (input_amount, output_amount, fee_amount) = Pallet::<T>::decide_buy_amounts(
                &self.main_asset_id,
                &self.collateral_asset_id,
                self.amount.into(),
                true,
            )?;
            let result = match self.amount {
                SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                    ensure!(
                        output_amount >= min_amount_out,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(output_amount, fee_amount)
                }
                SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                    ensure!(
                        input_amount <= max_amount_in,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(input_amount, fee_amount)
                }
            };

            // Reward needs to be updated before actual changes to reserves, because reward
            // calculations depend upon current state - actual reserves and xor supply, i.e. rewards depend
            // on values before transferring tokens from user
            self.update_reward(input_amount, output_amount)?;
            self.deposit_input(input_amount)?;
            self.distribute_reserves(input_amount)?;
            self.mint_output(output_amount.clone())?;
            Ok(result)
        })
    }
}

#[allow(non_snake_case)]
impl<T: Config> Pallet<T> {
    fn free_reserves_distribution_routine() -> Result<u32, DispatchError> {
        let free_reserves_acc =
            FreeReservesAccountId::<T>::get().ok_or(Error::<T>::FreeReservesAccountNotSet)?;
        PendingFreeReserves::<T>::mutate(|vec| {
            let len = vec.len();
            vec.retain(|(collateral_asset_id, free_amount)| {
                !Pallet::<T>::attempt_free_reserves_distribution(
                    &free_reserves_acc,
                    &collateral_asset_id,
                    *free_amount,
                )
                .is_ok()
            });
            Ok(len.try_into().unwrap_or(u32::max_value()))
        })
    }

    fn add_free_reserves_to_pending_list(
        holder: &T::AccountId,
        collateral_asset_id: T::AssetId,
        amount: Balance,
    ) -> DispatchResult {
        let free_reserves_acc =
            FreeReservesAccountId::<T>::get().ok_or(Error::<T>::FreeReservesAccountNotSet)?;
        Assets::<T>::transfer_from(&collateral_asset_id, holder, &free_reserves_acc, amount)?;
        PendingFreeReserves::<T>::mutate(|vec| vec.push((collateral_asset_id, amount)));
        Ok(())
    }

    fn attempt_free_reserves_distribution(
        holder: &T::AccountId,
        collateral_asset_id: &T::AssetId,
        free_amount: Balance,
    ) -> DispatchResult {
        common::with_transaction(|| {
            let base_asset_id = T::GetBaseAssetId::get();
            let swapped_xor_amount = T::LiquidityProxy::exchange(
                DEXId::Polkaswap.into(),
                holder,
                holder,
                &collateral_asset_id,
                &base_asset_id,
                SwapAmount::with_desired_input(free_amount, Balance::zero()).into(),
                Pallet::<T>::self_excluding_filter(),
            )
            .map_err(|error| match error {
                LiquidityProxyError::NotEnoughLiquidity => Error::<T>::CantExchange.into(),
                LiquidityProxyError::DispatchError(dispatch_error) => dispatch_error,
            })?
            .amount
            .into();
            Assets::<T>::burn_from(&base_asset_id, &holder, &holder, swapped_xor_amount)?;

            let fw_swapped_xor_amount = FixedWrapper::from(swapped_xor_amount);
            let mut undistributed_xor_amount = fw_swapped_xor_amount
                .clone()
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;

            let distribution_accounts: DistributionAccounts<
                DistributionAccountData<DistributionAccount<T::AccountId, T::TechAccountId>>,
            > = DistributionAccountsEntry::<T>::get();
            for (account, coefficient) in distribution_accounts
                .xor_distribution_as_array()
                .iter()
                .map(|x| (&x.account, x.coefficient))
            {
                let amount = fw_swapped_xor_amount.clone() * coefficient;
                let amount = amount
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let account = match account {
                    DistributionAccount::Account(account) => account.clone(),
                    DistributionAccount::TechAccount(account) => {
                        Technical::<T>::tech_account_id_to_account_id(account)?
                    }
                };
                Assets::<T>::mint_to(&base_asset_id, &holder, &account, amount)?;
                undistributed_xor_amount = undistributed_xor_amount.saturating_sub(amount);
            }

            let amount = fw_swapped_xor_amount * T::BuyBackTBCDPercent::get();
            let amount = amount
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
            undistributed_xor_amount = undistributed_xor_amount.saturating_sub(amount);
            T::BuyBackHandler::mint_buy_back_and_burn(&base_asset_id, &TBCD.into(), amount)?;

            // undistributed_xor_amount includes xor_allocation and val_holders portions
            T::BuyBackHandler::mint_buy_back_and_burn(
                &base_asset_id,
                &VAL.into(),
                undistributed_xor_amount,
            )?;
            Ok(())
        })
        .map_err(|err| {
            frame_support::log::error!("Reserves distribution failed, will try next time: {err:?}");
            err
        })
    }

    #[inline]
    fn update_collateral_reserves(
        collateral_asset: &T::AssetId,
        reserves_account: &T::AccountId,
    ) -> DispatchResult {
        let collateral_balance = Assets::<T>::free_balance(collateral_asset, reserves_account)?;
        CollateralReserves::<T>::insert(collateral_asset, collateral_balance);
        Ok(())
    }

    #[inline]
    fn self_excluding_filter() -> LiquiditySourceFilter<T::DEXId, LiquiditySourceType> {
        LiquiditySourceFilter::with_forbidden(
            DEXId::Polkaswap.into(),
            [LiquiditySourceType::MulticollateralBondingCurvePool].into(),
        )
    }

    fn initialize_pool_unchecked(
        collateral_asset_id: T::AssetId,
        transactional: bool,
    ) -> DispatchResult {
        let code = || {
            ensure!(
                !EnabledTargets::<T>::get().contains(&collateral_asset_id),
                Error::<T>::PoolAlreadyInitializedForPair
            );
            T::PriceToolsPallet::register_asset(&collateral_asset_id)?;
            <T as Config>::EnsureTradingPairExists::ensure_trading_pair_exists(
                &DEXId::Polkaswap.into(),
                &T::GetBaseAssetId::get(),
                &collateral_asset_id,
            )?;

            <T as Config>::TradingPairSourceManager::enable_source_for_trading_pair(
                &DEXId::Polkaswap.into(),
                &T::GetBaseAssetId::get(),
                &collateral_asset_id,
                LiquiditySourceType::MulticollateralBondingCurvePool,
            )?;
            if Self::collateral_is_incentivised(&collateral_asset_id) {
                IncentivisedCurrenciesNum::<T>::mutate(|num| *num += 1)
            }
            EnabledTargets::<T>::mutate(|set| set.insert(collateral_asset_id));
            Self::deposit_event(Event::PoolInitialized(
                DEXId::Polkaswap.into(),
                collateral_asset_id,
            ));
            Ok(())
        };
        if transactional {
            common::with_transaction(|| code())
        } else {
            code()
        }
    }

    /// Buy function with regards to asset total supply and its change delta. It represents the amount of
    /// input collateral required from the User to receive the requested XOR amount, i.e., the price the User buys XOR at.
    ///
    /// XOR is also referred as main asset.
    /// Value of `delta` is assumed to be either positive or negative.
    /// For every `price_change_step` tokens the price goes up by `price_change_rate`.
    ///
    /// `buy_price_usd = (xor_total_supply + xor_supply_delta) / (price_change_step * price_change_rate) + initial_price_usd`
    ///
    pub fn buy_function(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        price_variant: PriceVariant,
        delta: Fixed,
    ) -> Result<Fixed, DispatchError> {
        if collateral_asset_id == &TBCD.into() {
            // Handle TBCD
            let xp: FixedWrapper = Self::reference_price(&XOR.into(), price_variant)?.into();
            // get the XOR price in USD (DAI) and add $1 to it
            let xor_price: FixedWrapper = xp + fixed_wrapper!(1.0);
            xor_price
                .get()
                .map_err(|_| Error::<T>::PriceCalculationFailed.into())
        } else {
            // Everything other than TBCD
            let total_supply: FixedWrapper = Assets::<T>::total_issuance(main_asset_id)?.into();
            let initial_price: FixedWrapper = Self::initial_price().into();
            let price_change_step: FixedWrapper = Self::price_change_step().into();
            let price_change_rate: FixedWrapper = Self::price_change_rate().into();

            let price =
                (total_supply + delta) / (price_change_step * price_change_rate) + initial_price;
            price
                .get()
                .map_err(|_| Error::<T>::PriceCalculationFailed.into())
        }
    }

    /// Calculates and returns the current buy price, assuming that input is the collateral asset and output is the main asset.
    ///
    /// To calculate price for a specific amount of assets (with desired main asset output),
    /// one needs to calculate the area of a right trapezoid.
    ///
    /// `AB` : buy_function(xor_total_supply)
    /// `CD` : buy_function(xor_total_supply + xor_supply_delta)
    ///
    /// ```nocompile
    ///          ..  C
    ///        ..  │
    ///   B  ..    │
    ///     │   S  │
    ///     │      │
    ///   A └──────┘ D
    /// ```
    ///
    /// 1) Amount of collateral tokens needed in USD to get `xor_supply_delta`(AD) XOR tokens
    /// ```nocompile
    /// S = ((AB + CD) / 2) * AD
    ///
    /// or
    ///
    /// buy_price_usd = ((buy_function(xor_total_supply) + buy_function(xor_total_supply + xor_supply_delta)) / 2) * xor_supply_delta
    /// ```
    /// 2) Amount of XOR tokens received by depositing `S` collateral tokens in USD:
    ///
    /// Solving right trapezoid area formula with respect to `xor_supply_delta` (AD),
    /// actual square `S` is known and represents collateral amount.
    /// We have a quadratic equation:
    /// ```nocompile
    /// buy_function(x) = price_change_coefficient * x + initial_price
    /// Assume `M` = 1 / price_change_coefficient = 1 / 1337
    /// Assume `P` = price_change_coefficient = 1337
    ///
    /// M * AD² + 2 * AB * AD - 2 * S = 0
    /// equation with two solutions, taking only positive one:
    /// AD = (√((AB * 2 / M)² + 8 * S / M) - 2 * AB / M) / 2 (old formula)
    ///
    /// AD = √(P * (AB² * P + 2 * S)) - AB * P (new formula)
    ///
    /// or
    ///
    /// (old)
    /// xor_supply_delta = (√((buy_function(xor_total_supply) * 2 / price_change_coeff)²
    ///                    + 8 * buy_price_usd / price_change_coeff) - 2 * buy_function(xor_total_supply)
    ///                    / price_change_coeff) / 2
    ///
    /// (new)
    /// xor_supply_delta = √price_change_coefficient * √(buy_function(xor_total_supply)² * price_change_coefficient + 2 * buy_price_usd)
    ///                    - buy_function(xor_total_supply) * price_change_coefficient
    /// ```
    pub fn buy_price(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        let price_change_step = FixedWrapper::from(Self::price_change_step());
        let price_change_rate = Self::price_change_rate();
        let price_change_coeff = price_change_step * price_change_rate;

        let current_state: FixedWrapper = Self::buy_function(
            main_asset_id,
            collateral_asset_id,
            PriceVariant::Buy,
            Fixed::ZERO,
        )?
        .into();
        let collateral_price_per_reference_unit: FixedWrapper =
            Self::reference_price(collateral_asset_id, PriceVariant::Buy)?.into();

        match quantity {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: collateral_quantity,
            } => {
                let collateral_reference_in =
                    collateral_price_per_reference_unit * collateral_quantity;

                let main_out = if collateral_asset_id == &TBCD.into() {
                    collateral_reference_in / current_state
                } else {
                    let sqrt = (current_state.clone()
                        * current_state.clone()
                        * price_change_coeff.clone()
                        + (fixed_wrapper!(2.0) * collateral_reference_in.clone()))
                    .multiply_and_sqrt(&price_change_coeff);
                    sqrt - current_state * price_change_coeff
                };

                main_out
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: main_quantity,
            } => {
                let new_state: FixedWrapper = Self::buy_function(
                    main_asset_id,
                    collateral_asset_id,
                    PriceVariant::Buy,
                    FixedWrapper::from(main_quantity)
                        .get()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?,
                )?
                .into();
                let collateral_reference_in =
                    ((current_state + new_state) / fixed_wrapper!(2.0)) * main_quantity;
                let collateral_quantity =
                    collateral_reference_in / collateral_price_per_reference_unit;
                collateral_quantity
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
                    .map(|value| value.max(Fixed::ZERO))
            }
        }
    }

    /// Calculates and returns the current sell price, assuming that input is the main asset and output is the collateral asset.
    ///
    /// To calculate sell price for a specific amount of assets:
    /// 1. Current reserves of collateral token are taken
    /// 2. Same amount by value is assumed for main asset
    ///   2.1 Values are compared via getting prices for both main and collateral tokens with regard to another token
    ///       called reference token which is set for particular pair. This should be e.g. stablecoin DAI.
    ///   2.2 Reference price for base token is taken as 80% of current bonding curve buy price.
    ///   2.3 Reference price for collateral token is taken as current market price, i.e. price for 1 token on liquidity proxy.
    /// 3. Given known reserves for main and collateral, output collateral amount is calculated by applying x*y=k model resulting
    ///    in curve-like dependency.
    pub fn sell_price(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        quantity: QuoteAmount<Balance>,
    ) -> Result<Fixed, DispatchError> {
        let reserves_tech_account_id = ReservesAcc::<T>::get();
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        let collateral_supply: FixedWrapper =
            Assets::<T>::free_balance(collateral_asset_id, &reserves_account_id)?.into();
        // Get reference prices for base and collateral to understand token value.
        let main_price_per_reference_unit: FixedWrapper =
            Self::sell_function(main_asset_id, collateral_asset_id, Fixed::ZERO)?.into();

        let collateral_price_per_reference_unit: FixedWrapper =
            Self::reference_price(collateral_asset_id, PriceVariant::Sell)?.into();

        // Assume main token reserve is equal by reference value to collateral token reserve.
        let main_supply = collateral_supply.clone() * collateral_price_per_reference_unit
            / main_price_per_reference_unit;
        let collateral_supply_unwrapped = collateral_supply
            .clone()
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;

        match quantity {
            QuoteAmount::WithDesiredInput {
                desired_amount_in: quantity_main,
            } => {
                let output_collateral =
                    (quantity_main * collateral_supply) / (main_supply + quantity_main);
                let output_collateral_unwrapped = output_collateral
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    output_collateral_unwrapped < collateral_supply_unwrapped,
                    Error::<T>::NotEnoughReserves
                );
                Ok(output_collateral_unwrapped)
            }
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: quantity_collateral,
            } => {
                let collateral_supply_unwrapped = collateral_supply_unwrapped
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                ensure!(
                    quantity_collateral < collateral_supply_unwrapped,
                    Error::<T>::NotEnoughReserves
                );
                let output_main =
                    (main_supply * quantity_collateral) / (collateral_supply - quantity_collateral);
                output_main
                    .get()
                    .map_err(|_| Error::<T>::PriceCalculationFailed.into())
            }
        }
    }

    /// Sell function with regards to asset total supply and its change delta. It represents the amount of
    /// output collateral tokens received by User by indicating exact sold XOR amount. I.e. the price User sells at.
    ///
    /// Value of `delta` is assumed to be either positive or negative.
    /// Sell function is `sell_price_coefficient`% of buy function (see `buy_function`).
    ///
    /// `sell_price = sell_price_coefficient * buy_price`
    ///
    pub fn sell_function(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        delta: Fixed,
    ) -> Result<Fixed, DispatchError> {
        let buy_price = Self::buy_function(
            main_asset_id,
            collateral_asset_id,
            PriceVariant::Sell,
            delta,
        )?;
        let sell_price_coefficient = FixedWrapper::from(Self::sell_price_coefficient());
        let sell_price = sell_price_coefficient * buy_price;
        sell_price
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// Decompose SwapAmount into particular buy quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_buy_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        Ok(match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let mut output_amount: Balance = FixedWrapper::from(Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_input(desired_amount_in),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let fee_amount = if deduce_fee {
                    (FixedWrapper::from(BaseFee::<T>::get()) * output_amount)
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?
                } else {
                    0
                };
                output_amount = output_amount.saturating_sub(fee_amount);
                (desired_amount_in, output_amount, fee_amount)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } if deduce_fee => {
                let desired_amount_out_with_fee = (FixedWrapper::from(desired_amount_out)
                    / (fixed_wrapper!(1) - BaseFee::<T>::get()))
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                let input_amount = Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out_with_fee.clone()),
                )?;
                let input_amount = input_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                (
                    input_amount,
                    desired_amount_out,
                    desired_amount_out_with_fee.saturating_sub(desired_amount_out),
                )
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let input_amount = Self::buy_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?;
                let input_amount = input_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                (input_amount, desired_amount_out, 0)
            }
        })
    }

    /// Mapping that defines ratio of fee penalty applied for selling XOR with
    /// low collateralized reserves.
    fn map_collateralized_fraction_to_penalty(fraction: Fixed) -> Fixed {
        if fraction < fixed!(0.05) {
            fixed!(0.09)
        } else if fraction >= fixed!(0.05) && fraction < fixed!(0.1) {
            fixed!(0.06)
        } else if fraction >= fixed!(0.1) && fraction < fixed!(0.2) {
            fixed!(0.03)
        } else if fraction >= fixed!(0.2) && fraction < fixed!(0.3) {
            fixed!(0.01)
        } else {
            fixed!(0)
        }
    }

    /// Calculate ratio of fee penalty that is applied to trades when XOR is sold while
    /// reserves are low for target collateral asset.
    fn sell_penalty(collateral_asset_id: &T::AssetId) -> Result<Fixed, DispatchError> {
        let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&Self::reserves_account_id())?;
        // USD price for XOR supply on network
        let ideal_reserves_price: FixedWrapper = Self::ideal_reserves_reference_price(
            collateral_asset_id,
            PriceVariant::Sell,
            Fixed::ZERO,
        )?
        .into();
        // USD price for amount of indicated collateral asset stored in reserves
        let collateral_reserves_price = Self::actual_reserves_reference_price(
            &reserves_account_id,
            collateral_asset_id,
            PriceVariant::Sell,
        )?;
        ensure!(
            !collateral_reserves_price.is_zero(),
            Error::<T>::NotEnoughReserves
        );
        // ratio of stored reserves to ideal reserves
        let collateralized_fraction = (FixedWrapper::from(collateral_reserves_price)
            / ideal_reserves_price)
            .get()
            .map_err(|_| Error::<T>::FeeCalculationFailed)?;
        Ok(Self::map_collateralized_fraction_to_penalty(
            collateralized_fraction,
        ))
    }

    /// Decompose SwapAmount into particular sell quotation query.
    ///
    /// Returns ordered pair: (input_amount, output_amount, fee_amount).
    fn decide_sell_amounts(
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(Balance, Balance, Balance), DispatchError> {
        Ok(match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                let fee_amount = if deduce_fee {
                    let fee_ratio = FixedWrapper::from(BaseFee::<T>::get())
                        + Self::sell_penalty(collateral_asset_id)?;
                    (fee_ratio * FixedWrapper::from(desired_amount_in))
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?
                } else {
                    0
                };
                let output_amount = Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_input(
                        desired_amount_in.saturating_sub(fee_amount.clone()),
                    ),
                )?;
                let output_amount = output_amount
                    .into_bits()
                    .try_into()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                (desired_amount_in, output_amount, fee_amount)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                let input_amount: Balance = FixedWrapper::from(Self::sell_price(
                    main_asset_id,
                    collateral_asset_id,
                    QuoteAmount::with_desired_output(desired_amount_out),
                )?)
                .try_into_balance()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                if deduce_fee {
                    let fee_ratio = FixedWrapper::from(BaseFee::<T>::get())
                        + Self::sell_penalty(collateral_asset_id)?;
                    let input_amount_with_fee =
                        FixedWrapper::from(input_amount) / (fixed_wrapper!(1) - fee_ratio);
                    let input_amount_with_fee = input_amount_with_fee
                        .try_into_balance()
                        .map_err(|_| Error::<T>::PriceCalculationFailed)?;
                    (
                        input_amount_with_fee,
                        desired_amount_out,
                        input_amount_with_fee.saturating_sub(input_amount),
                    )
                } else {
                    (input_amount, desired_amount_out, 0)
                }
            }
        })
    }

    /// This function is used by `exchange` function to burn `input_amount` derived from `amount` of `main_asset_id`
    /// and transfer calculated amount of `collateral_asset_id` to the receiver from reserves.
    ///
    /// If there's not enough reserves in the pool, `NotEnoughReserves` error will be returned.
    ///
    fn sell_main_asset(
        _dex_id: &T::DEXId,
        main_asset_id: &T::AssetId,
        collateral_asset_id: &T::AssetId,
        amount: SwapAmount<Balance>,
        from_account_id: &T::AccountId,
        to_account_id: &T::AccountId,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        common::with_transaction(|| {
            let reserves_tech_account_id = Self::reserves_account_id();
            let reserves_account_id =
                Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
            let (input_amount, output_amount, fee_amount) =
                Self::decide_sell_amounts(main_asset_id, collateral_asset_id, amount.into(), true)?;
            let reserves_amount =
                Assets::<T>::total_balance(collateral_asset_id, &reserves_account_id)?;
            ensure!(
                reserves_amount >= output_amount,
                Error::<T>::NotEnoughReserves
            );
            let result = match amount {
                SwapAmount::WithDesiredInput { min_amount_out, .. } => {
                    ensure!(
                        output_amount >= min_amount_out,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(output_amount, fee_amount)
                }
                SwapAmount::WithDesiredOutput { max_amount_in, .. } => {
                    ensure!(
                        input_amount <= max_amount_in,
                        Error::<T>::SlippageLimitExceeded
                    );
                    SwapOutcome::new(input_amount, fee_amount)
                }
            };
            technical::Pallet::<T>::transfer_out(
                collateral_asset_id,
                &reserves_tech_account_id,
                &to_account_id,
                output_amount,
            )?;
            Assets::<T>::burn_from(
                main_asset_id,
                &reserves_account_id,
                from_account_id,
                input_amount,
            )?;
            Ok(result)
        })
    }

    /// Assign account id that is used to store deposited collateral tokens.
    pub fn set_reserves_account_id(account: T::TechAccountId) -> Result<(), DispatchError> {
        common::with_transaction(|| {
            ReservesAcc::<T>::set(account.clone());
            let account_id = Technical::<T>::tech_account_id_to_account_id(&account)?;
            let permissions = [BURN, MINT];
            for permission in &permissions {
                permissions::Pallet::<T>::assign_permission(
                    account_id.clone(),
                    &account_id,
                    *permission,
                    Scope::Unlimited,
                )?;
            }
            Ok(())
        })
    }

    /// Assign accounts list to be used for free reserves distribution in config.
    pub fn set_distribution_accounts(
        distribution_accounts: DistributionAccounts<
            DistributionAccountData<DistributionAccount<T::AccountId, T::TechAccountId>>,
        >,
    ) {
        DistributionAccountsEntry::<T>::set(distribution_accounts);
    }

    /// This function is used to determine particular asset price in terms of a reference asset, which is set for
    /// bonding curve (there could be only single token chosen as reference for all comparisons). Basically, the
    /// reference token is expected to be a USD-bound stablecoin, e.g. DAI.
    ///
    /// Example use: understand actual value of two tokens in terms of USD.
    fn reference_price(
        asset_id: &T::AssetId,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        let reference_asset_id = ReferenceAssetId::<T>::get();
        // always treat TBCD as being worth $1
        let price = if asset_id == &reference_asset_id || asset_id == &common::TBCD.into() {
            balance!(1)
        } else {
            <T as Config>::PriceToolsPallet::get_average_price(
                asset_id,
                &reference_asset_id,
                price_variant,
            )?
        };
        Ok(price)
    }

    /// Calculate USD price for single collateral asset that is stored in reserves account. In other words, find out how much
    /// reserves worth, considering only one asset type.
    fn actual_reserves_reference_price(
        reserves_account_id: &T::AccountId,
        collateral_asset_id: &T::AssetId,
        price_variant: PriceVariant,
    ) -> Result<Balance, DispatchError> {
        let reserve = Assets::<T>::free_balance(&collateral_asset_id, &reserves_account_id)?;
        let price = Self::reference_price(&collateral_asset_id, price_variant)?;
        (FixedWrapper::from(reserve) * price)
            .try_into_balance()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// Calculate USD price for all XOR in network, this is done by applying ideal sell function to XOR total supply.
    /// - `delta` is a XOR supply offset from current total supply.
    fn ideal_reserves_reference_price(
        collateral_asset_id: &T::AssetId,
        price_variant: PriceVariant,
        delta: Fixed,
    ) -> Result<Balance, DispatchError> {
        let base_asset_id = T::GetBaseAssetId::get();
        let base_total_supply = Assets::<T>::total_issuance(&base_asset_id)?;
        let initial_state = FixedWrapper::from(Self::initial_price());
        let current_state =
            Self::buy_function(&base_asset_id, collateral_asset_id, price_variant, delta)?;

        let price = ((initial_state + current_state) / fixed_wrapper!(2.0))
            * (FixedWrapper::from(base_total_supply) + delta);
        price
            .try_into_balance()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// Calculate amount of PSWAP rewarded for collateralizing XOR in TBC.
    ///
    /// ideal_reserves_before = sell_function(0 to xor_total_supply_before_trade)
    /// ideal_reserves_after = sell_function(0 to xor_total_supply_after_trade)
    /// actual_reserves_before = collateral_asset_reserves * collateral_asset_usd_price
    /// actual_reserves_after = actual_reserves_before + collateral_asset_input_amount * collateral_asset_usd_price
    ///
    /// unfunded_liabilities = (ideal_reserves_before - actual_reserves_before)
    /// a = unfunded_liabilities / ideal_reserves_before
    /// b = unfunded_liabilities / ideal_reserves_after
    /// P = initial_pswap_rewards
    /// N = enabled reserve currencies except PSWAP and VAL
    ///
    /// reward_pswap = ((a - b) * mean(a, b) * P) / N
    ///
    pub fn calculate_buy_reward(
        reserves_account_id: &T::AccountId,
        collateral_asset_id: &T::AssetId,
        _collateral_asset_amount: Balance,
        main_asset_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        if !Self::collateral_is_incentivised(collateral_asset_id) {
            return Ok(Balance::zero());
        }

        // Get current state values.
        let ideal_before: FixedWrapper = Self::ideal_reserves_reference_price(
            collateral_asset_id,
            PriceVariant::Buy,
            Fixed::ZERO,
        )?
        .into();
        let ideal_after: FixedWrapper = Self::ideal_reserves_reference_price(
            collateral_asset_id,
            PriceVariant::Buy,
            FixedWrapper::from(main_asset_amount)
                .get()
                .map_err(|_| Error::<T>::PriceCalculationFailed)?,
        )?
        .into();
        let actual_before: FixedWrapper = Self::actual_reserves_reference_price(
            reserves_account_id,
            collateral_asset_id,
            PriceVariant::Buy,
        )?
        .into();
        let incentivised_currencies_num: u128 = IncentivisedCurrenciesNum::<T>::get().into();
        let N: FixedWrapper = FixedWrapper::from(incentivised_currencies_num * balance!(1));
        let P: FixedWrapper = FixedWrapper::from(InitialPswapRewardsSupply::<T>::get());

        // Calculate reward.
        let unfunded_liabilities = ideal_before.clone() - actual_before;
        let a = unfunded_liabilities.clone() / ideal_before;
        let b = unfunded_liabilities / ideal_after;
        let mean_ab = (a.clone() + b.clone()) / fixed_wrapper!(2);
        let reward_pswap = ((a - b) * P * mean_ab) / N;
        reward_pswap
            .try_into_balance()
            .map_err(|_| Error::<T>::PriceCalculationFailed.into())
    }

    /// Check if particular asset is incentivesed, when depositing it as collateral,
    /// i.e. if it will result in PSWAP rewards during buy operation.
    fn collateral_is_incentivised(collateral_asset_id: &T::AssetId) -> bool {
        collateral_asset_id != &PSWAP.into()
            && collateral_asset_id != &VAL.into()
            && collateral_asset_id != &XST.into()
            && collateral_asset_id != &TBCD.into()
    }
}

impl<T: Config> LiquiditySource<T::DEXId, T::AccountId, T::AssetId, Balance, DispatchError>
    for Pallet<T>
{
    fn can_exchange(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
    ) -> bool {
        if *dex_id != DEXId::Polkaswap.into() {
            return false;
        }
        if input_asset_id == &T::GetBaseAssetId::get() {
            EnabledTargets::<T>::get().contains(&output_asset_id)
        } else if output_asset_id == &T::GetBaseAssetId::get() {
            EnabledTargets::<T>::get().contains(&input_asset_id)
        } else {
            false
        }
    }

    fn quote(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance>, Weight), LiquiditySourceQuoteError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(LiquiditySourceQuoteError::DispatchError(
                Error::<T>::CantExchange.into()
            ));
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        let (input_amount, output_amount, fee_amount) = if input_asset_id == base_asset_id {
            Self::decide_sell_amounts(&input_asset_id, &output_asset_id, amount, deduce_fee)
                .map_err(|error| LiquiditySourceQuoteError::DispatchError(error))?
        } else {
            Self::decide_buy_amounts(&output_asset_id, &input_asset_id, amount, deduce_fee)
                .map_err(|error| LiquiditySourceQuoteError::DispatchError(error))?
        };
        match amount {
            QuoteAmount::WithDesiredInput { .. } => Ok((
                SwapOutcome::new(output_amount, fee_amount),
                Self::quote_weight(),
            )),
            QuoteAmount::WithDesiredOutput { .. } => Ok((
                SwapOutcome::new(input_amount, fee_amount),
                Self::quote_weight(),
            )),
        }
    }

    fn exchange(
        sender: &T::AccountId,
        receiver: &T::AccountId,
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        desired_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }
        let reserves_account_id =
            &Technical::<T>::tech_account_id_to_account_id(&Self::reserves_account_id())?;
        // This is needed to prevent recursion calls.
        if sender == reserves_account_id && receiver == reserves_account_id {
            fail!(Error::<T>::CannotExchangeWithSelf);
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        if input_asset_id == base_asset_id {
            let outcome = Self::sell_main_asset(
                dex_id,
                input_asset_id,
                output_asset_id,
                desired_amount,
                sender,
                receiver,
            );
            Pallet::<T>::update_collateral_reserves(output_asset_id, reserves_account_id)?;
            outcome.map(|res| (res, Self::exchange_weight()))
        } else {
            let outcome = BuyMainAsset::<T>::new(
                *input_asset_id,
                *output_asset_id,
                desired_amount,
                sender.clone(),
                receiver.clone(),
            )?
            .swap();
            Pallet::<T>::update_collateral_reserves(input_asset_id, reserves_account_id)?;
            outcome.map(|res| (res, Self::exchange_weight()))
        }
    }

    fn check_rewards(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        input_amount: Balance,
        output_amount: Balance,
    ) -> Result<(Vec<(Balance, T::AssetId, RewardReason)>, Weight), DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }
        let mut weight = <T as Config>::WeightInfo::can_exchange();

        let base_asset_id = &T::GetBaseAssetId::get();
        if output_asset_id == base_asset_id {
            weight = Self::check_rewards_weight();
            let reserves_tech_account_id = ReservesAcc::<T>::get();
            let reserves_account_id =
                Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
            let mut pswap_amount = Pallet::<T>::calculate_buy_reward(
                &reserves_account_id,
                input_asset_id,
                input_amount,
                output_amount,
            )?;
            if let Some(multiplier) = AssetsWithOptionalRewardMultiplier::<T>::get(&input_asset_id)
            {
                pswap_amount = (FixedWrapper::from(pswap_amount) * multiplier)
                    .try_into_balance()
                    .map_err(|_| Error::<T>::PriceCalculationFailed)?;
            }
            if !pswap_amount.is_zero() {
                Ok((
                    [(pswap_amount, PSWAP.into(), RewardReason::BuyOnBondingCurve)].into(),
                    weight,
                ))
            } else {
                Ok((Vec::new(), weight))
            }
        } else {
            Ok((Vec::new(), weight)) // no rewards on sell
        }
    }

    fn quote_without_impact(
        dex_id: &T::DEXId,
        input_asset_id: &T::AssetId,
        output_asset_id: &T::AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        if !Self::can_exchange(dex_id, input_asset_id, output_asset_id) {
            fail!(Error::<T>::CantExchange);
        }
        let base_asset_id = &T::GetBaseAssetId::get();
        let outcome = if input_asset_id == base_asset_id {
            let base_price_wrt_collateral: FixedWrapper = <Pallet<T> as GetMarketInfo<
                T::AssetId,
            >>::sell_price(
                input_asset_id, output_asset_id
            )?
            .into();
            let fee_ratio =
                FixedWrapper::from(BaseFee::<T>::get()) + Self::sell_penalty(output_asset_id)?;
            match amount {
                QuoteAmount::WithDesiredInput { desired_amount_in } => {
                    let fee_amount = if deduce_fee {
                        (fee_ratio * FixedWrapper::from(desired_amount_in))
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?
                    } else {
                        0
                    };
                    let collateral_out =
                        (FixedWrapper::from(desired_amount_in.saturating_sub(fee_amount.clone()))
                            * base_price_wrt_collateral)
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                    SwapOutcome::new(collateral_out, fee_amount)
                }
                QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                    let base_in =
                        FixedWrapper::from(desired_amount_out) / base_price_wrt_collateral;
                    let input_amount_with_fee = base_in.clone() / (fixed_wrapper!(1) - fee_ratio);
                    let fee_amount = if deduce_fee {
                        (input_amount_with_fee.clone() - base_in)
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?
                    } else {
                        0
                    };
                    SwapOutcome::new(
                        input_amount_with_fee
                            .try_into_balance()
                            .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?,
                        fee_amount,
                    )
                }
            }
        } else {
            let base_price_wrt_collateral: FixedWrapper = <Pallet<T> as GetMarketInfo<
                T::AssetId,
            >>::buy_price(
                output_asset_id, input_asset_id
            )?
            .into();
            match amount {
                QuoteAmount::WithDesiredInput { desired_amount_in } => {
                    let base_out =
                        FixedWrapper::from(desired_amount_in) / base_price_wrt_collateral;
                    let fee_amount = (FixedWrapper::from(BaseFee::<T>::get()) * base_out.clone())
                        .try_into_balance()
                        .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                    let mut base_out_unwrapped = base_out
                        .try_into_balance()
                        .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                    base_out_unwrapped = base_out_unwrapped.saturating_sub(fee_amount);
                    SwapOutcome::new(base_out_unwrapped, fee_amount)
                }
                QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                    let desired_amount_out_with_fee = (FixedWrapper::from(desired_amount_out)
                        / (fixed_wrapper!(1) - BaseFee::<T>::get()))
                    .try_into_balance()
                    .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                    let collateral_in = (FixedWrapper::from(desired_amount_out_with_fee)
                        * base_price_wrt_collateral)
                        .try_into_balance()
                        .map_err(|_| Error::<T>::FailedToCalculatePriceWithoutImpact)?;
                    let fee_amount = desired_amount_out_with_fee.saturating_sub(desired_amount_out);
                    SwapOutcome::new(collateral_in, fee_amount)
                }
            }
        };
        Ok(outcome)
    }

    fn quote_weight() -> Weight {
        <T as Config>::WeightInfo::quote()
    }

    fn exchange_weight() -> Weight {
        <T as Config>::WeightInfo::exchange()
    }

    fn check_rewards_weight() -> Weight {
        <T as Config>::WeightInfo::check_rewards()
    }
}

impl<T: Config> GetMarketInfo<T::AssetId> for Pallet<T> {
    fn buy_price(
        base_asset: &T::AssetId,
        collateral_asset: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let base_price_wrt_ref: FixedWrapper =
            Self::buy_function(base_asset, collateral_asset, PriceVariant::Buy, fixed!(0))?.into();
        let collateral_price_per_reference_unit: FixedWrapper =
            Self::reference_price(collateral_asset, PriceVariant::Sell)?.into();
        let output = (base_price_wrt_ref / collateral_price_per_reference_unit)
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        Ok(output)
    }

    fn sell_price(
        base_asset: &T::AssetId,
        collateral_asset: &T::AssetId,
    ) -> Result<Fixed, DispatchError> {
        let base_price_wrt_ref: FixedWrapper =
            Self::sell_function(base_asset, collateral_asset, fixed!(0))?.into();
        let collateral_price_per_reference_unit: FixedWrapper =
            Self::reference_price(collateral_asset, PriceVariant::Buy)?.into();
        let output = (base_price_wrt_ref / collateral_price_per_reference_unit)
            .get()
            .map_err(|_| Error::<T>::PriceCalculationFailed)?;
        Ok(output)
    }

    fn enabled_target_assets() -> BTreeSet<T::AssetId> {
        EnabledTargets::<T>::get()
    }
}
