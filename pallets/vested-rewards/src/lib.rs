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

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

use codec::{Decode, Encode};
use common::prelude::{Balance, FixedWrapper};
use common::{
    balance, Fixed, OnPswapBurned, PswapRemintInfo, RewardReason, VestedRewardsPallet, PSWAP, VAL,
    XSTUSD,
};
use core::convert::TryFrom;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::fail;
use frame_support::traits::{Get, IsType};
use frame_support::weights::Weight;
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{UniqueSaturatedInto, Zero};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::convert::TryInto;
use sp_std::str;
use sp_std::vec::Vec;

pub mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod migrations;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"vested-rewards";
pub const TECH_ACCOUNT_MARKET_MAKERS: &[u8] = b"market-makers";
pub const TECH_ACCOUNT_CROWDLOAN: &[u8] = b"crowdloan";
pub const TECH_ACCOUNT_FARMING: &[u8] = b"farming";
pub const FARMING_REWARDS: Balance = balance!(3500000000);
pub const VAL_CROWDLOAN_REWARDS: Balance = balance!(676393);
pub const PSWAP_CROWDLOAN_REWARDS: Balance = balance!(9363480);
pub const XSTUSD_CROWDLOAN_REWARDS: Balance = balance!(77050);
pub const BLOCKS_PER_DAY: u128 = 14400;
#[cfg(not(feature = "private-net"))]
pub const LEASE_START_BLOCK: u128 = 4_397_212;
#[cfg(feature = "private-net")]
pub const LEASE_START_BLOCK: u128 = 0;
pub const LEASE_TOTAL_DAYS: u128 = 318;

type Assets<T> = assets::Pallet<T>;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// Denotes PSWAP rewards amounts of particular types available for user.
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default, scale_info::TypeInfo,
)]
pub struct RewardInfo {
    /// Reward amount vested, denotes portion of `total_avialable` which can be claimed.
    /// Reset to 0 after claim until more is vested over time.
    limit: Balance,
    /// Sum of reward amounts in `rewards`.
    total_available: Balance,
    /// Mapping between reward type represented by `RewardReason` and owned amount by user.
    pub rewards: BTreeMap<RewardReason, Balance>,
}

/// A vested reward for crowdloan.
#[derive(
    Encode, Decode, Deserialize, Serialize, Clone, Debug, Default, PartialEq, scale_info::TypeInfo,
)]
pub struct CrowdloanReward {
    /// The user id
    #[serde(with = "serde_bytes", rename = "ID")]
    pub id: Vec<u8>,
    /// The user address
    #[serde(with = "hex", rename = "Address")]
    pub address: Vec<u8>,
    /// Kusama contribution
    #[serde(rename = "Contribution")]
    pub contribution: Fixed,
    /// Reward in XOR
    #[serde(rename = "XOR Reward")]
    pub xor_reward: Fixed,
    /// Reward in VAL
    #[serde(rename = "Val Reward")]
    pub val_reward: Fixed,
    /// Reward in PSWAP
    #[serde(rename = "PSWAP Reward")]
    pub pswap_reward: Fixed,
    /// Reward in XSTUSD
    #[serde(rename = "XSTUSD Reward")]
    pub xstusd_reward: Fixed,
    /// Reward in percents of the total contribution
    #[serde(rename = "Percent")]
    pub percent: Fixed,
}

pub trait WeightInfo {
    fn claim_incentives() -> Weight;
    fn on_initialize(_n: u32) -> Weight;
    fn claim_crowdloan_rewards() -> Weight;
    fn update_rewards(n: u32) -> Weight;
}

impl<T: Config> Pallet<T> {
    pub fn add_pending_reward(
        account_id: &T::AccountId,
        reason: RewardReason,
        amount: Balance,
    ) -> DispatchResult {
        if !Rewards::<T>::contains_key(account_id) {
            frame_system::Pallet::<T>::inc_consumers(account_id)
                .map_err(|_| Error::<T>::IncRefError)?;
        }
        Rewards::<T>::mutate(account_id, |info| {
            info.total_available = info.total_available.saturating_add(amount);
            info.rewards
                .entry(reason)
                .and_modify(|e| *e = e.saturating_add(amount))
                .or_insert(amount);
        });
        TotalRewards::<T>::mutate(|balance| *balance = balance.saturating_add(amount));
        Ok(())
    }

    /// General claim function, which updates user reward status.
    pub fn claim_rewards_inner(account_id: &T::AccountId) -> DispatchResult {
        let mut remove_after_mutate = false;
        let result = Rewards::<T>::mutate(account_id, |info| {
            if info.total_available.is_zero() {
                fail!(Error::<T>::NothingToClaim);
            } else if info.limit.is_zero() {
                fail!(Error::<T>::ClaimLimitExceeded);
            } else {
                let mut total_actual_claimed: Balance = 0;
                for (&reward_reason, amount) in info.rewards.iter_mut() {
                    let claimable = (*amount).min(info.limit);
                    let actual_claimed = Self::claim_reward_by_reason(
                        account_id,
                        reward_reason,
                        &PSWAP.into(),
                        claimable,
                    )
                    .unwrap_or(balance!(0));
                    info.limit = info.limit.saturating_sub(actual_claimed);
                    total_actual_claimed = total_actual_claimed.saturating_add(actual_claimed);
                    if claimable > actual_claimed {
                        Self::deposit_event(Event::<T>::ActualDoesntMatchAvailable(reward_reason));
                    }
                    *amount = amount.saturating_sub(actual_claimed);
                }
                // clear zeroed entries
                // NOTE: .retain() is an unstable feature yet
                info.rewards = info
                    .rewards
                    .clone()
                    .into_iter()
                    .filter(|&(_, reward)| reward > balance!(0))
                    .collect();
                if total_actual_claimed.is_zero() {
                    fail!(Error::<T>::RewardsSupplyShortage);
                }
                info.total_available = info.total_available.saturating_sub(total_actual_claimed);
                TotalRewards::<T>::mutate(|total| {
                    *total = total.saturating_sub(total_actual_claimed)
                });
                remove_after_mutate = info.total_available == 0;
                Ok(())
            }
        });
        if result.is_ok() && remove_after_mutate {
            Rewards::<T>::remove(account_id);
            frame_system::Pallet::<T>::dec_consumers(account_id);
        }
        result
    }

    /// Claim rewards from account with reserves dedicated for particular reward type.
    pub fn claim_reward_by_reason(
        account_id: &T::AccountId,
        reason: RewardReason,
        asset_id: &T::AssetId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let source_account = match reason {
            RewardReason::BuyOnBondingCurve => T::GetBondingCurveRewardsAccountId::get(),
            RewardReason::LiquidityProvisionFarming => T::GetFarmingRewardsAccountId::get(),
            RewardReason::Crowdloan => T::GetCrowdloanRewardsAccountId::get(),
            _ => fail!(Error::<T>::UnhandledRewardType),
        };
        let available_rewards = Assets::<T>::free_balance(asset_id, &source_account)?;
        if available_rewards.is_zero() {
            fail!(Error::<T>::RewardsSupplyShortage);
        }
        let amount = amount.min(available_rewards);
        Assets::<T>::transfer_from(asset_id, &source_account, account_id, amount)?;
        Ok(amount)
    }

    pub fn distribute_limits(vested_amount: Balance) {
        let total_rewards = TotalRewards::<T>::get();

        // if there's no accounts to vest, then amount is not utilized nor stored
        if !total_rewards.is_zero() {
            Rewards::<T>::translate(|_key: T::AccountId, mut info: RewardInfo| {
                let share_of_the_vested_amount = FixedWrapper::from(info.total_available)
                    * FixedWrapper::from(vested_amount)
                    / FixedWrapper::from(total_rewards);

                let new_limit = (share_of_the_vested_amount + FixedWrapper::from(info.limit))
                    .try_into_balance()
                    .unwrap_or(info.limit);

                // don't vest more than available
                info.limit = new_limit.min(info.total_available);
                Some(info)
            })
        };
    }

    pub fn crowdloan_reward_for_asset(
        address: &T::AccountId,
        asset_id: &T::AssetId,
        current_block_number: u128,
    ) -> Result<Balance, DispatchError> {
        let rewards =
            CrowdloanRewards::<T>::try_get(address).map_err(|_| Error::<T>::NothingToClaim)?;
        let last_claim_block: u128 =
            CrowdloanClaimHistory::<T>::get(address, asset_id).unique_saturated_into();
        let claim_period = if last_claim_block.is_zero() {
            current_block_number.saturating_sub(LEASE_START_BLOCK)
        } else {
            current_block_number.saturating_sub(last_claim_block)
        };
        let claim_days = Fixed::try_from(claim_period / BLOCKS_PER_DAY)
            .map_err(|_| DispatchError::from(Error::<T>::NumberConversionError))?;
        let reward = if asset_id == &VAL.into() {
            rewards.val_reward
        } else if asset_id == &PSWAP.into() {
            rewards.pswap_reward
        } else if asset_id == &XSTUSD.into() {
            rewards.xstusd_reward
        } else {
            return Err(Error::<T>::NoRewardsForAsset.into());
        };
        let reward = reward
            / Fixed::try_from(LEASE_TOTAL_DAYS)
                .map_err(|_| DispatchError::from(Error::<T>::NumberConversionError))?
                .into();

        (reward * claim_days)
            .try_into_balance()
            .map_err(|_| Error::<T>::ArithmeticError.into())
    }
}

impl<T: Config> OnPswapBurned for Pallet<T> {
    /// NOTE: currently is not invoked.
    /// Invoked when pswap is burned after being exchanged from collected liquidity provider fees.
    fn on_pswap_burned(distribution: PswapRemintInfo) {
        Pallet::<T>::distribute_limits(distribution.vesting)
    }
}

impl<T: Config> VestedRewardsPallet<T::AccountId, T::AssetId> for Pallet<T> {
    fn add_tbc_reward(account_id: &T::AccountId, pswap_amount: Balance) -> DispatchResult {
        Pallet::<T>::add_pending_reward(account_id, RewardReason::BuyOnBondingCurve, pswap_amount)
    }

    fn add_farming_reward(account_id: &T::AccountId, pswap_amount: Balance) -> DispatchResult {
        Pallet::<T>::add_pending_reward(
            account_id,
            RewardReason::LiquidityProvisionFarming,
            pswap_amount,
        )
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::dispatch::DispatchResultWithPostInfo;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_support::transactional;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::UniqueSaturatedFrom;
    use sp_std::collections::btree_map::BTreeMap;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + common::Config
        + assets::Config
        + multicollateral_bonding_curve_pool::Config
    {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// Accounts holding PSWAP dedicated for rewards.
        type GetMarketMakerRewardsAccountId: Get<Self::AccountId>;
        type GetFarmingRewardsAccountId: Get<Self::AccountId>;
        type GetBondingCurveRewardsAccountId: Get<Self::AccountId>;
        type GetCrowdloanRewardsAccountId: Get<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim all available PSWAP rewards by account signing this transaction.
        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::claim_incentives())]

        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::claim_rewards_inner(&who)?;
            Ok(().into())
        }

        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::claim_crowdloan_rewards())]
        pub fn claim_crowdloan_rewards(
            origin: OriginFor<T>,
            asset_id: T::AssetId,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let current_block_number: u128 =
                <frame_system::Pallet<T>>::block_number().unique_saturated_into();
            let reward =
                Pallet::<T>::crowdloan_reward_for_asset(&who, &asset_id, current_block_number)?;

            Pallet::<T>::claim_reward_by_reason(&who, RewardReason::Crowdloan, &asset_id, reward)?;

            CrowdloanClaimHistory::<T>::mutate(who, asset_id, |value| {
                let offset = current_block_number % BLOCKS_PER_DAY;
                *value = T::BlockNumber::unique_saturated_from(
                    current_block_number.saturating_sub(offset),
                )
            });

            Ok(().into())
        }

        #[transactional]
        #[pallet::weight(<T as Config>::WeightInfo::update_rewards(rewards.len() as u32))]
        pub fn update_rewards(
            origin: OriginFor<T>,
            rewards: BTreeMap<T::AccountId, BTreeMap<RewardReason, Balance>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut total_rewards_diff = 0i128;
            for (account, reward) in rewards {
                Rewards::<T>::mutate(&account, |value| {
                    for (reason, amount) in reward {
                        let v = value.rewards.entry(reason).or_insert(0);
                        *v += amount;
                    }
                    let total: i128 = value
                        .rewards
                        .iter_mut()
                        .map(|(_, amount)| *amount as i128)
                        .sum();
                    total_rewards_diff += total - value.total_available as i128;
                });
            }
            TotalRewards::<T>::mutate(|value| {
                if total_rewards_diff < 0 {
                    *value -= total_rewards_diff.abs() as Balance;
                } else {
                    *value += total_rewards_diff as Balance;
                }
            });

            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Account has no pending rewards to claim.
        NothingToClaim,
        /// Account has pending rewards but it has not been vested yet.
        ClaimLimitExceeded,
        /// Attempt to claim rewards of type, which is not handled.
        UnhandledRewardType,
        /// Account holding dedicated reward reserves is empty. This likely means that some of
        /// reward programmes have finished.
        RewardsSupplyShortage,
        /// Increment account reference error.
        IncRefError,
        /// Attempt to subtract more via snapshot than assigned to user.
        CantSubtractSnapshot,
        /// Failed to perform reward calculation.
        CantCalculateReward,
        /// There are no rewards for the asset ID.
        NoRewardsForAsset,
        /// Something is wrong with arithmetic - overflow happened, for example.
        ArithmeticError,
        /// This error appears on wrong conversion of a number into another type.
        NumberConversionError,
        /// Unable to get base asset price in XOR. XOR-base asset pair should exist on Polkaswap DEX.
        UnableToGetBaseAssetPrice,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Rewards vested, limits were raised. [vested amount]
        RewardsVested(Balance),
        /// Attempted to claim reward, but actual claimed amount is less than expected. [reason for reward]
        ActualDoesntMatchAvailable(RewardReason),
        /// Saving reward for account has failed in a distribution series. [account]
        FailedToSaveCalculatedReward(AccountIdOf<T>),
    }

    /// Reserved for future use
    /// Mapping between users and their owned rewards of different kinds, which are vested.
    #[pallet::storage]
    #[pallet::getter(fn rewards)]
    pub type Rewards<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, RewardInfo, ValueQuery>;

    /// Reserved for future use
    /// Total amount of PSWAP pending rewards.
    #[pallet::storage]
    #[pallet::getter(fn total_rewards)]
    pub type TotalRewards<T: Config> = StorageValue<_, Balance, ValueQuery>;

    /// Crowdloan vested rewards storage.
    #[pallet::storage]
    #[pallet::getter(fn crowdloan_rewards)]
    pub type CrowdloanRewards<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CrowdloanReward, ValueQuery>;

    /// This storage keeps the last block number, when the user (the first) claimed a reward for
    /// asset (the second key). The block is rounded to days.
    #[pallet::storage]
    #[pallet::getter(fn crowdloan_claim_history)]
    pub type CrowdloanClaimHistory<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        T::AssetId,
        T::BlockNumber,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig {
        pub test_crowdloan_rewards: Vec<CrowdloanReward>,
    }

    #[cfg(feature = "std")]
    impl Default for GenesisConfig {
        fn default() -> Self {
            Self {
                test_crowdloan_rewards: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            use frame_support::log;
            use traits::MultiCurrency;

            self.test_crowdloan_rewards.iter().for_each(|reward| {
                CrowdloanRewards::<T>::insert(
                    T::AccountId::decode(&mut &reward.address[..])
                        .expect("Can't decode contributor address."),
                    reward.clone(),
                )
            });

            if let Err(e) = T::Currency::deposit(
                VAL.into(),
                &T::GetCrowdloanRewardsAccountId::get(),
                VAL_CROWDLOAN_REWARDS,
            ) {
                log::error!(target: "runtime", "Failed to add VAL crowdloan rewards: {:?}", e);
            }

            if let Err(e) = T::Currency::deposit(
                PSWAP.into(),
                &T::GetCrowdloanRewardsAccountId::get(),
                PSWAP_CROWDLOAN_REWARDS,
            ) {
                log::error!(target: "runtime", "Failed to add PSWAP crowdloan rewards: {:?}", e);
            }

            if let Err(e) = T::Currency::deposit(
                XSTUSD.into(),
                &T::GetCrowdloanRewardsAccountId::get(),
                XSTUSD_CROWDLOAN_REWARDS,
            ) {
                log::error!(target: "runtime", "Failed to add XSTUSD crowdloan rewards: {:?}", e);
            }
        }
    }
}
