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

use crate::{
    Config, Error, MarketMakersRegistry, Pallet, RewardInfo, Weight, FARMING_REWARDS,
    MARKET_MAKER_ELIGIBILITY_TX_COUNT, SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT,
};
use common::prelude::{Balance, FixedWrapper};
use common::{balance, fixed_wrapper, RewardReason, PSWAP};
use frame_support::log::{error, info, warn};
use frame_support::traits::{CrateVersion, Get, PalletInfoAccess};
use sp_runtime::runtime_logger::RuntimeLogger;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;
use traits::MultiCurrency;

pub fn migrate<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    match Pallet::<T>::crate_version() {
        // Initial version is 0.1.0 which has unutilized rewards storage
        // Version 1.1.0 converts and moves rewards from multicollateral-bonding-curve-pool
        version if version == CrateVersion::new(0, 1, 0) => {
            let migrated_weight = migrate_rewards_from_tbc::<T>().unwrap_or(100_000);
            weight = weight.saturating_add(migrated_weight);
        }
        CrateVersion {
            major: 1,
            minor: 1,
            patch: 0,
        } => {
            weight = add_funds_to_farming_rewards_account::<T>();
        }
        _ => (),
    }

    weight
}

#[allow(dead_code)]
pub fn migrate_rewards_from_tbc<T: Config>() -> Option<Weight> {
    let mut weight: Weight = 0;
    let mut calculated_total_rewards = Balance::zero();
    RuntimeLogger::init();
    // common factor for rewards difference, derived emperically
    let rewards_multiplier = fixed_wrapper!(6.8);
    for (account, (vested_amount, tbc_rewards_amount)) in
        multicollateral_bonding_curve_pool::Rewards::<T>::drain()
    {
        let updated_reward_amount = (tbc_rewards_amount * rewards_multiplier.clone())
            .try_into_balance()
            .ok()?;
        let reward_info = RewardInfo {
            limit: vested_amount,
            total_available: updated_reward_amount,
            rewards: [(RewardReason::BuyOnBondingCurve, updated_reward_amount)]
                .iter()
                .cloned()
                .collect(),
        };
        // Assuming target storage is empty before migration.
        crate::Rewards::<T>::insert(account, reward_info);
        calculated_total_rewards = calculated_total_rewards.saturating_add(updated_reward_amount);
        weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
    }

    // Set stored total rewards in tbc to zero.
    let tbc_total_rewards = multicollateral_bonding_curve_pool::TotalRewards::<T>::get();
    multicollateral_bonding_curve_pool::TotalRewards::<T>::put(Balance::zero());
    crate::TotalRewards::<T>::put(calculated_total_rewards);
    weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 2));

    if tbc_total_rewards != calculated_total_rewards {
        warn!(
            target: "runtime",
            "stored tbc rewards total doesn't match calculated total: {} != {}",
            tbc_total_rewards, calculated_total_rewards
        );
    } else {
        info!(
            target: "runtime",
            "stored tbc rewards total match calculated total: {}",
            calculated_total_rewards
        );
    }

    Some(weight)
}

pub fn inject_market_makers_first_month_rewards<T: Config>(
    snapshot: Vec<(T::AccountId, u32, Balance)>,
) -> Result<Weight, DispatchError> {
    let mut weight: Weight = 0;

    let mut eligible_accounts = Vec::new();
    let mut total_eligible_volume = balance!(0);
    for (account_id, count, volume) in snapshot {
        let account_id: T::AccountId = account_id.into();
        if count >= MARKET_MAKER_ELIGIBILITY_TX_COUNT {
            eligible_accounts.push((account_id.clone(), volume));
            total_eligible_volume = total_eligible_volume.saturating_add(volume);
        }
        let current_state = MarketMakersRegistry::<T>::get(&account_id);
        let new_count = current_state
            .count
            .checked_sub(count)
            .ok_or(Error::<T>::CantSubtractSnapshot)?;
        let new_volume = current_state
            .volume
            .checked_sub(volume)
            .ok_or(Error::<T>::CantSubtractSnapshot)?;
        MarketMakersRegistry::<T>::mutate(&account_id, |val| {
            val.count = new_count;
            val.volume = new_volume;
        });
        weight = weight.saturating_add(T::DbWeight::get().writes(1));
    }
    if total_eligible_volume > 0 {
        for (account, volume) in eligible_accounts.iter() {
            let reward = (FixedWrapper::from(*volume)
                * FixedWrapper::from(SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT)
                / FixedWrapper::from(total_eligible_volume))
            .try_into_balance()
            .map_err(|_| Error::<T>::CantCalculateReward)?;
            if reward > 0 {
                let res = Pallet::<T>::add_pending_reward(
                    account,
                    RewardReason::MarketMakerVolume,
                    reward,
                );
                if res.is_err() {
                    error!(target: "runtime", "Failed to add mm reward for account: {:?}", account);
                }
                weight = weight.saturating_add(T::DbWeight::get().writes(2));
            }
        }
    }

    Ok(weight)
}

pub fn add_funds_to_farming_rewards_account<T: Config>() -> Weight {
    if let Err(e) = T::Currency::deposit(
        PSWAP.into(),
        &T::GetFarmingRewardsAccountId::get(),
        FARMING_REWARDS,
    ) {
        error!(target: "runtime", "Failed to add farming rewards: {:?}", e);
    }
    T::DbWeight::get().writes(1)
}
