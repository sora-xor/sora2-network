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

use crate::{Config, Pallet, RewardInfo, Weight};
use common::prelude::Balance;
use common::{fixed_wrapper, RewardReason};
use frame_support::debug;
use frame_support::traits::{Get, GetPalletVersion, PalletVersion};
use sp_runtime::traits::Zero;

pub fn migrate<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    match Pallet::<T>::storage_version() {
        // Initial version is 0.1.0 which has unutilized rewards storage
        // Version 0.2.0 converts and moves rewards from multicollateral-bonding-curve-pool
        Some(version) if version == PalletVersion::new(0, 1, 0) => {
            let migrated_weight = migrate_rewards_from_tbc::<T>().unwrap_or(100_000);
            weight = weight.saturating_add(migrated_weight)
        }
        _ => (),
    }

    weight
}

pub fn migrate_rewards_from_tbc<T: Config>() -> Option<Weight> {
    let mut weight: Weight = 0;
    let mut calculated_total_rewards = Balance::zero();
    debug::RuntimeLogger::init();
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
        debug::warn!(
            target: "runtime",
            "stored tbc rewards total doesn't match calculated total: {} != {}",
            tbc_total_rewards, calculated_total_rewards
        );
    } else {
        debug::info!(
            target: "runtime",
            "stored tbc rewards total match calculated total: {}",
            calculated_total_rewards
        );
    }

    Some(weight)
}
