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
use common::{balance, OnPswapBurned, PswapRemintInfo, RewardReason, VestedRewardsTrait, PSWAP};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::fail;
use frame_support::traits::{Get, IsType};
use frame_support::weights::Weight;
use sp_runtime::traits::Zero;
use sp_std::collections::btree_map::BTreeMap;

pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"vested-rewards";
pub const TECH_ACCOUNT_MARKET_MAKERS: &[u8] = b"market-makers";

type Assets<T> = assets::Pallet<T>;

#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default)]
pub struct RewardInfo {
    limit: Balance,
    total_available: Balance,
    rewards: BTreeMap<RewardReason, Balance>,
}

#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default)]
pub struct MarketMakerInfo {
    count: u32,
    volume: Balance,
}

pub trait WeightInfo {
    fn claim_incentives() -> Weight;
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
            let entry = info.rewards.entry(reason).or_insert(Default::default());
            *entry = entry.saturating_add(amount);
        });
        TotalRewards::<T>::mutate(|balance| *balance = balance.saturating_add(amount));
        Ok(())
    }

    /// General claim function, which updates user reward status.
    pub fn claim_rewards_inner(account_id: &T::AccountId) -> DispatchResult {
        Rewards::<T>::mutate(account_id, |info| {
            if info.total_available.is_zero() || info.limit.is_zero() {
                fail!(Error::<T>::NothingToClaim);
            } else {
                let mut total_actual_claimed: Balance = 0;
                for (&reward_reason, amount) in info.rewards.iter_mut() {
                    let claimable = amount.clone().min(info.limit);
                    let actual_claimed =
                        Self::claim_reward_by_reason(account_id, reward_reason, claimable)
                            .unwrap_or(balance!(0));
                    total_actual_claimed = total_actual_claimed.saturating_add(actual_claimed);
                    // TODO: maybe throw event on error for better detalisation
                    info.limit = info.limit.saturating_sub(actual_claimed);
                    *amount = amount.saturating_sub(actual_claimed);
                }
                info.total_available = info.total_available.saturating_sub(total_actual_claimed);
                TotalRewards::<T>::mutate(|total| {
                    *total = total.saturating_sub(total_actual_claimed)
                });
                Ok(())
            }
        })
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
        // TODO: what to do if there's no accounts to receive vesting?
        if !total_rewards.is_zero() {
            Rewards::<T>::translate(|_key: T::AccountId, mut info: RewardInfo| {
                let limit_to_add = FixedWrapper::from(info.total_available)
                    * FixedWrapper::from(vested_amount)
                    / FixedWrapper::from(total_rewards);
                info.limit = (limit_to_add + FixedWrapper::from(info.limit))
                    .try_into_balance()
                    .unwrap_or(info.limit);
                Some(info)
            })
        };
    }
}

impl<T: Config> OnPswapBurned for Module<T> {
    /// NOTE: currently is not invoked.
    /// Invoked when pswap is burned after being exchanged from collected liquidity provider fees.
    fn on_pswap_burned(distribution: PswapRemintInfo) {
        Pallet::<T>::distribute_limits(distribution.vesting)
    }
}

impl<T: Config> VestedRewardsTrait<T::AccountId> for Module<T> {
    /// Check if volume is eligible to be counted for market maker rewards and add it to registry.
    /// `count` is used as a multiplier if multiple times single volume is transferred inside transaction.
    fn update_market_maker_records(
        account_id: &T::AccountId,
        xor_volume: Balance,
        count: u32,
    ) -> DispatchResult {
        MarketMakersRegistry::<T>::mutate(account_id, |info| {
            if xor_volume >= balance!(1) {
                info.count = info.count.saturating_add(count);
                info.volume = info
                    .volume
                    .saturating_add(xor_volume.saturating_mul(count as Balance));
            }
        });
        Ok(())
    }

    fn add_tbc_reward(account_id: &T::AccountId, pswap_amount: Balance) -> DispatchResult {
        Pallet::<T>::add_pending_reward(account_id, RewardReason::BuyOnBondingCurve, pswap_amount)
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + common::Config + assets::Config {
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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        /// Account has no pending rewards to claim.
        NothingToClaim,
        /// Attempt to claim rewards of type, which is not handled.
        UnhandledRewardType,
        /// Trying to claim PSWAP, but account with reward reserves is empty. This likely means that reward programme has finished.
        RewardsSupplyShortage,
        /// Increment account reference error.
        IncRefError,
    }

    #[pallet::event]
    #[pallet::metadata(DexIdOf<T> = "DEXId", TradingPair<T> = "TradingPair")]
    // #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Rewards vested, limits were raised. [vested amount]
        RewardsVested(Balance),
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
