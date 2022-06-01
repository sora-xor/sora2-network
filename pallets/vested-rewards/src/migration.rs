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
    Config, CrowdloanReward, Error, MarketMakersRegistry, Pallet, RewardInfo, Weight,
    FARMING_REWARDS, LEASE_TOTAL_DAYS, MARKET_MAKER_ELIGIBILITY_TX_COUNT, PSWAP_CROWDLOAN_REWARDS,
    SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT, VAL_CROWDLOAN_REWARDS, XSTUSD_CROWDLOAN_REWARDS,
};
use codec::Decode;
use common::prelude::{Balance, FixedWrapper};
use common::weights::constants::EXTRINSIC_FIXED_WEIGHT;
use common::{balance, fixed_wrapper, Fixed, RewardReason, PSWAP, VAL, XOR, XSTUSD};
use frame_support::debug;
use frame_support::traits::{Get, GetPalletVersion, PalletVersion};
use hex_literal::hex;
use serde_json;
use sp_core::H256;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;
use traits::MultiCurrency;

const CROWDLOAN_REWARDS: &'static str = include_str!("../crowdloan_rewards.json");

pub fn migrate<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    match Pallet::<T>::storage_version() {
        // Initial version is 0.1.0 which has unutilized rewards storage
        // Version 1.1.0 converts and moves rewards from multicollateral-bonding-curve-pool
        Some(version) if version == PalletVersion::new(0, 1, 0) => {
            let migrated_weight = migrate_rewards_from_tbc::<T>().unwrap_or(100_000);
            weight = weight.saturating_add(migrated_weight);
        }
        Some(PalletVersion {
            major: 1,
            minor: 1,
            patch: 0,
        }) => {
            weight = add_funds_to_farming_rewards_account::<T>();
        }
        // we had this, but didn't update the pallet version, so it's commented and we have it here
        // for documentating purposes
        // Some(PalletVersion {
        // major: 1,
        // minor: 2,
        // patch: 0,
        // }) => weight = weight.saturating_add(add_funds_to_crowdloan_rewards_account::<T>()),
        Some(PalletVersion {
            major: 1,
            minor: 2,
            patch: 0,
        }) => weight = weight.saturating_add(reset_claiming_for_crowdloan_errors::<T>()),
        _ => (),
    }

    weight
        .saturating_add(allow_market_making_pairs::<T>())
        .saturating_add(add_crowdloan_rewards::<T>())
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
                    debug::error!(target: "runtime", "Failed to add mm reward for account: {:?}", account);
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
        debug::error!(target: "runtime", "Failed to add farming rewards: {:?}", e);
    }
    T::DbWeight::get().writes(1)
}

pub fn allow_market_making_pairs<T: Config>() -> Weight {
    let allowed = allowed_market_making_assets::<T>();
    allowed
        .clone()
        .into_iter()
        .filter(|id| !crate::MarketMakingPairs::<T>::contains_key(&T::AssetId::from(XOR), &id))
        .for_each(|id| crate::MarketMakingPairs::<T>::insert(&T::AssetId::from(XOR), &id, ()));
    allowed
        .into_iter()
        .filter(|id| !crate::MarketMakingPairs::<T>::contains_key(&id, &T::AssetId::from(XOR)))
        .for_each(|id| crate::MarketMakingPairs::<T>::insert(&id, &T::AssetId::from(XOR), ()));
    EXTRINSIC_FIXED_WEIGHT
}

fn allowed_market_making_assets<T: Config>() -> Vec<T::AssetId> {
    [
        hex!("00019977e20516b9f7112cd8cfef1a5be2e5344d2ef1aa5bc92bbb503e81146e"), // FTT
        hex!("0004d3168f737e96b66b72fbb1949a2a23d4ef87182d1e8bf64096f1bb348e0b"), // REEF
        hex!("001da2678bc8b0ff27d17eb4c11cc8e0def6c16a141d93253f3aa51276aa7b45"), // KNC
        hex!("001f7a13792061236adfc93fa3aa8bad1dc8a8e8f889432b3d8d416b986f2c43"), // DIA
        hex!("002676c3edea5b08bc0f9b6809a91aa313b7da35e28b190222e9dc032bf1e662"), // YFI
        hex!("002c48630dcb8c75cc36162cbdbc8ff27b843973b951ba9b6e260f869d45bcdc"), // WBTC
        hex!("002ca40397c794e25dba18cf807910eeb69eb8e81b3f07bb54f7c5d1d8ab76b9"), // OCEAN
        hex!("002ead91a2de57b8855b53d4a62c25277073fd7f65f7e5e79f4936ed747fcad0"), // CRV
        hex!("003005b2417b5046455e73f7fc39779a013f1a33b4518bcd83a790900dca49ff"), // NEXO
        hex!("003252667a82d2dd70fa046eea663eaec1f2e37c20879f113b880b04c5ebd805"), // UMI
        hex!("0033271716eec64234a5324506c4558de27b7c23c42f3e3b74801f98bdfeebf7"), // PHA
        hex!("0033406b3b121dff08d2f285f1184d41a5d96eb6ca27b5171489aa797fbc860f"), // COCK
        hex!("00374b2e4a72217a919dd1711500cd78f4c6178dc08c196e6c571d8320576c21"), // COCO
        hex!("00378f1c907c65cfacf46574ec5285e91fc3ef80276f730cffc8d6f66bf5229f"), // MEOW
        hex!("004249314d526b706a2e71e76a6d81911e4e6d7fb6480051d879fdb8ef1dccc9"), // PAX
        hex!("00438aac3a91cc6cee0c8d2f14e4bf7ec4512ca708b180cc0fda47b0eb1ad538"), // RENBTC
        hex!("00449af28b82575d6ac0e8c6d20e095be0917e1b0eaa63962a1dc2c6b81c2b0d"), // MANA
        hex!("0047e323378d23116261954e67836f350c45625124bbadb35404d9109026feb5"), // RARE
        hex!("004baaeb9bf0d5210a51fab72d10c84a34f53bea4e0e102d794d531a45ec50f9"), // HOT
        hex!("004d9058620eb7aa4ea243dc6cefc4b76c0cf7ad941246066142c871b376bb7e"), // CRO
        hex!("00521ad5caeadc2e3e04be4d4ebb0b7c8c9b71ba657c2362a3953490ebc81410"), // CREAM
        hex!("005476064ff01a847b1c565ce577ad37105c3cd2a2e755da908b87f7eeb4423b"), // STAKE
        hex!("00567d096a736f33bf78cad7b01e33463923b9c933ee13ab7e3fb7b23f5f953a"), // BUSD
        hex!("005e152271f8816d76221c7a0b5c6cafcb54fdfb6954dd8812f0158bfeac900d"), // AGI
        hex!("006cfd2fb06c15cd2c464d1830c0d247e32f36f34233a6a266d6581ea5677582"), // IDEX
        hex!("006d336effe921106f7817e133686bbc4258a4e0d6fed3a9294d8a8b27312cee"), // TUSD
        hex!("007348eb8f0f3cec730fbf5eec1b6a842c54d1df8bed75a9df084d5ee013e814"), // AKRO
        hex!("0078f4e6c5113b3d8c954dff62ece8fc36a8411f86f1cbb48a52527e22e73be2"), // SUSHI
        hex!("007d9428e446cf88b532d6182658996b956149b9e63565f4efbff8bfab79bb70"), // SOSHIBA
        hex!("007d998d3d13fbb74078fb58826e3b7bc154004c9cef6f5bccb27da274f02724"), // CHSB
        hex!("007e908e399cc73f3dad9f02f9c5c83a7adcd07e78dd91676ff3c002e245d8e9"), // XFUND
        hex!("0080edc40a944d29562b2dea2de42ed27b9047d16eeea27c5bc1b2e02786abe9"), // OKB
        hex!("008146909618facff9642fc591925ef91f10263c250cbae5db504b8b0955435a"), // KOBE
        hex!("008294f7b08f568a661de2b248c34fc574e7e0012a12ef7959eb1a5c6b349e09"), // RLC
        hex!("0083d5cbb4b90163b6a003e8f771eb7c0e2b706892cd0cbadb03f55cb9e06919"), // XRT
        hex!("008484148dcf23d1b48908393e7a00d5fdc3bf81029a73eeca62a15ebfb1205a"), // LINK
        hex!("008a99c642c508f4f718598f32fa9ecbeea854e335312fecdbd298b92de26e21"), // PDEX
        hex!("008ba21aa988b21e86d5b25ed9ea690d28a6ba6c5ba9037424c215fd5b193c32"), // HUSD
        hex!("008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"), // CERES
        hex!("008efe4328cba1012cb9ad97943f09cadfbeea5e692871cd2649f0bf4e718088"), // FOTO
        hex!("008f925e3e422218604fac1cc2f06f3ef9c1e244e0d2a9a823e5bd8ce9778434"), // TEL
        hex!("009134d5c7b7fda8863985531f456f89bef5fbd76684a8acdb737b3e451d0877"), // MATIC
        hex!("0091bd8d8295b25cab5a7b8b0e44498e678cfc15d872ede3215f7d4c7635ba36"), // AAVE
        hex!("009749fbd2661866f0151e367365b7c5cc4b2c90070b4f745d0bb84f2ffb3b33"), // HT
        hex!("009be848df92a400da2f217256c88d1a9b1a0304f9b3e90991a67418e1d3b08c"), // UNI
        hex!("009e199267a6a2c8ae075bb8d4c40ee8d05c1b769085ee59ce98e50c2b2d8756"), // LEO
        hex!("00b0afb0e0762b24252dd7457dc6e3bfccfdc7bac35ad81abef31fa9944815f5"), // FANS
        hex!("00d1fb79bbd1005a678fbf2de9256b3afe260e8eead49bb07bd3a566f9fe8355"), // GRT
        hex!("00dbd45af9f2ea406746f9025110297469e9d29efc60df8d88efb9b0179d6c2c"), // COMP
        hex!("00dca673e1f57dfffbb301fb6d2b5a37779a878dc21367b20161ca1462964a47"), // TAMU
        hex!("00e16b53b05b8a7378f8f3080bef710634f387552b1d1916edc578bda89d49e5"), // BAT
        hex!("00e40bcd6ee5363d3abbb4603273aa2f6bb89e29323729e884a8ef9c991fe73e"), // UMA
        hex!("00e6df883c9844e34b354b840e3a527f5fc6bfc937138c67908b1c8f2931f3e9"), // FIS
        hex!("00e8a7823b8207e4cab2e46cd10b54d1be6b82c284037b6ee76afd52c0dceba6"), // REN
        hex!("00ec184ef0b4bd955db05eea5a8489ae72888ab6e63682a15beca1cd39344c8f"), // MKR
        hex!("00ef6658f79d8b560f77b7b20a5d7822f5bc22539c7b4056128258e5829da517"), // USDC
        hex!("00f8cfb462a824f37dcea67caae0d7e2f73ed8371e706ea8b1e1a7b0c357d5d4"), // UST
        hex!("0200040000000000000000000000000000000000000000000000000000000000"), // VAL
        hex!("0200050000000000000000000000000000000000000000000000000000000000"), // PSWAP
        hex!("0200060000000000000000000000000000000000000000000000000000000000"), // DAI
        hex!("0200070000000000000000000000000000000000000000000000000000000000"), // ETH
        hex!("0200080000000000000000000000000000000000000000000000000000000000"), // XSTUSD
    ]
    .iter()
    .map(|h| T::AssetId::from(H256::from(h)))
    .collect()
}

pub fn add_crowdloan_rewards<T: Config>() -> Weight {
    let rewards = serde_json::from_str::<Vec<CrowdloanReward>>(CROWDLOAN_REWARDS)
        .expect("Can't deserialize crowdloan contributors.");

    rewards.into_iter().for_each(|reward| {
        crate::CrowdloanRewards::<T>::insert(
            T::AccountId::decode(&mut &reward.address[..])
                .expect("Can't decode contributor address."),
            reward,
        )
    });

    EXTRINSIC_FIXED_WEIGHT
}

// this function is here for documentating purposes. It was used in migration for crowdloan rewards.
// See migrate function for more.
#[allow(dead_code)]
pub fn add_funds_to_crowdloan_rewards_account<T: Config>() -> Weight {
    if let Err(e) = T::Currency::deposit(
        VAL.into(),
        &T::GetCrowdloanRewardsAccountId::get(),
        VAL_CROWDLOAN_REWARDS,
    ) {
        debug::error!(target: "runtime", "Failed to add VAL crowdloan rewards: {:?}", e);
    }

    if let Err(e) = T::Currency::deposit(
        PSWAP.into(),
        &T::GetCrowdloanRewardsAccountId::get(),
        PSWAP_CROWDLOAN_REWARDS,
    ) {
        debug::error!(target: "runtime", "Failed to add PSWAP crowdloan rewards: {:?}", e);
    }

    if let Err(e) = T::Currency::deposit(
        XSTUSD.into(),
        &T::GetCrowdloanRewardsAccountId::get(),
        XSTUSD_CROWDLOAN_REWARDS,
    ) {
        debug::error!(target: "runtime", "Failed to add XSTUSD crowdloan rewards: {:?}", e);
    }

    T::DbWeight::get().writes(3)
}

pub fn reset_claiming_for_crowdloan_errors<T: Config>() -> Weight {
    let rewards = serde_json::from_str::<Vec<CrowdloanReward>>(CROWDLOAN_REWARDS)
        .expect("Can't deserialize crowdloan contributors.");
    let mut number_of_writes = 0;
    rewards
        .into_iter()
        .map(|reward| {
            let address = T::AccountId::decode(&mut &reward.address[..])
                .expect("Can't decode contributor address.");
            let mut assets = Vec::new();

            if should_reset_claim_history(reward.val_reward) {
                assets.push(T::AssetId::from(VAL));
            }

            if should_reset_claim_history(reward.pswap_reward) {
                assets.push(T::AssetId::from(PSWAP));
            }

            if should_reset_claim_history(reward.xstusd_reward) {
                assets.push(T::AssetId::from(XSTUSD));
            }

            (address, assets)
        })
        .for_each(|(address, assets)| {
            assets.into_iter().for_each(|asset| {
                crate::CrowdloanClaimHistory::<T>::insert(
                    &address,
                    asset,
                    T::BlockNumber::default(),
                );
                number_of_writes += 1;
            })
        });
    T::DbWeight::get().writes(number_of_writes)
}

fn should_reset_claim_history(value: Fixed) -> bool {
    (value / LEASE_TOTAL_DAYS.into()).get().is_err()
}
