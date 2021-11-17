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
use common::{balance, OnPswapBurned, PswapRemintInfo, RewardReason, VestedRewardsPallet, PSWAP};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::traits::{Get, IsType};
use frame_support::weights::Weight;
use frame_support::{fail, transactional};
use sp_runtime::traits::Zero;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::convert::TryInto;
use sp_std::vec::Vec;

mod migration;
pub mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"vested-rewards";
pub const TECH_ACCOUNT_MARKET_MAKERS: &[u8] = b"market-makers";
pub const MARKET_MAKER_ELIGIBILITY_TX_COUNT: u32 = 500;
pub const SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT: Balance = balance!(20000000);
pub const MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY: u32 = 432000;

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

/// Denotes information about users who make transactions counted for market makers strategic rewards
/// programme. To participate in rewards distribution account needs to get 500+ tx's over 1 XOR in volume each.
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default, scale_info::TypeInfo,
)]
pub struct MarketMakerInfo {
    /// Number of eligible transactions - namely those with individual volume over 1 XOR.
    count: u32,
    /// Cumulative volume of eligible transactions.
    volume: Balance,
}

pub trait WeightInfo {
    fn claim_incentives() -> Weight;
    fn on_initialize(_n: u32) -> Weight;
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
                    let claimable = amount.clone().min(info.limit);
                    let actual_claimed =
                        Self::claim_reward_by_reason(account_id, reward_reason, claimable)
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
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        let source_account = match reason {
            RewardReason::BuyOnBondingCurve => T::GetBondingCurveRewardsAccountId::get(),
            // RewardReason::LiquidityProvisionFarming => T::GetFarmingRewardsAccountId::get(), // TODO: handle with farming rewards
            RewardReason::MarketMakerVolume => T::GetMarketMakerRewardsAccountId::get(),
            _ => fail!(Error::<T>::UnhandledRewardType),
        };
        let available_rewards = Assets::<T>::free_balance(&PSWAP.into(), &source_account)?;
        if available_rewards.is_zero() {
            fail!(Error::<T>::RewardsSupplyShortage);
        }
        let amount = amount.min(available_rewards);
        Assets::<T>::transfer_from(&PSWAP.into(), &source_account, account_id, amount)?;
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

    /// Returns number of accounts who received rewards.
    pub fn market_maker_rewards_distribution_routine() -> u32 {
        // collect list of accounts with volume info
        let mut eligible_accounts = Vec::new();
        let mut total_eligible_volume = balance!(0);
        for (account, info) in MarketMakersRegistry::<T>::drain() {
            if info.count >= MARKET_MAKER_ELIGIBILITY_TX_COUNT {
                eligible_accounts.push((account, info.volume));
                total_eligible_volume = total_eligible_volume.saturating_add(info.volume);
            }
        }
        let eligible_accounts_count = eligible_accounts.len();
        if total_eligible_volume > 0 {
            for (account, volume) in eligible_accounts {
                let reward = (FixedWrapper::from(volume)
                    * FixedWrapper::from(SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT)
                    / FixedWrapper::from(total_eligible_volume))
                .try_into_balance()
                .unwrap_or(0);
                if reward > 0 {
                    let res =
                        Self::add_pending_reward(&account, RewardReason::MarketMakerVolume, reward);
                    if res.is_err() {
                        Self::deposit_event(Event::<T>::FailedToSaveCalculatedReward(account))
                    }
                } else {
                    Self::deposit_event(Event::<T>::AddingZeroMarketMakerReward(account));
                }
            }
        } else {
            Self::deposit_event(Event::<T>::NoEligibleMarketMakers);
        }
        eligible_accounts_count.try_into().unwrap_or(u32::MAX)
    }
}

impl<T: Config> OnPswapBurned for Module<T> {
    /// NOTE: currently is not invoked.
    /// Invoked when pswap is burned after being exchanged from collected liquidity provider fees.
    fn on_pswap_burned(distribution: PswapRemintInfo) {
        Pallet::<T>::distribute_limits(distribution.vesting)
    }
}

impl<T: Config> VestedRewardsPallet<T::AccountId> for Module<T> {
    /// Check if volume is eligible to be counted for market maker rewards and add it to registry.
    /// `count` is used as a multiplier if multiple times same volume is transferred inside transaction.
    fn update_market_maker_records(
        account_id: &T::AccountId,
        xor_volume: Balance,
        count: u32,
    ) -> DispatchResult {
        if xor_volume >= balance!(1) {
            MarketMakersRegistry::<T>::mutate(account_id, |info| {
                info.count = info.count.saturating_add(count);
                info.volume = info
                    .volume
                    .saturating_add(xor_volume.saturating_mul(count as Balance));
            });
        }
        Ok(())
    }

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

    fn add_market_maker_reward(account_id: &T::AccountId, pswap_amount: Balance) -> DispatchResult {
        Pallet::<T>::add_pending_reward(account_id, RewardReason::MarketMakerVolume, pswap_amount)
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + common::Config
        + assets::Config
        + multicollateral_bonding_curve_pool::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        /// Accounts holding PSWAP dedicated for rewards.
        type GetMarketMakerRewardsAccountId: Get<Self::AccountId>;
        // type GetFarmingRewardsAccountId: Get<Self::AccountId>; // TODO: implement with farming rewards
        type GetBondingCurveRewardsAccountId: Get<Self::AccountId>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            migration::migrate::<T>()
        }

        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            if (block_number % MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY.into()).is_zero() {
                let elems = Module::<T>::market_maker_rewards_distribution_routine();
                <T as Config>::WeightInfo::on_initialize(elems)
            } else {
                <T as Config>::WeightInfo::on_initialize(0)
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Claim all available PSWAP rewards by account signing this transaction.
        #[pallet::weight(<T as Config>::WeightInfo::claim_incentives())]
        #[transactional]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            Self::claim_rewards_inner(&who)?;
            Ok(().into())
        }

        /// Inject market makers snapshot into storage.
        #[pallet::weight(0)]
        #[transactional]
        pub fn inject_market_makers(
            origin: OriginFor<T>,
            snapshot: Vec<(T::AccountId, u32, Balance)>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let weight = crate::migration::inject_market_makers_first_month_rewards::<T>(snapshot)?;
            Ok(Some(weight).into())
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
        /// Account holding dedicated reward reserves is empty. This likely means that some of reward programmes have finished.
        RewardsSupplyShortage,
        /// Increment account reference error.
        IncRefError,
        /// Attempt to subtract more via snapshot than assigned to user.
        CantSubtractSnapshot,
        /// Failed to perform reward calculation.
        CantCalculateReward,
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
        /// Account was chosen as eligible for market maker rewards, however calculated reward turned into 0. [account]
        AddingZeroMarketMakerReward(AccountIdOf<T>),
        /// Couldn't find any account with enough transactions to count market maker rewards.
        NoEligibleMarketMakers,
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

    /// Registry of market makers with large transaction volumes (>1 XOR per transaction).
    #[pallet::storage]
    #[pallet::getter(fn market_makers_registry)]
    pub type MarketMakersRegistry<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, MarketMakerInfo, ValueQuery>;
}
