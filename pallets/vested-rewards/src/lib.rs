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
use hex_literal::hex;
use sp_core::H256;
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
pub const TECH_ACCOUNT_FARMING: &[u8] = b"farming";
pub const MARKET_MAKER_ELIGIBILITY_TX_COUNT: u32 = 500;
pub const SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT: Balance = balance!(20000000);
pub const FARMING_REWARDS: Balance = balance!(3500000000);
pub const MARKET_MAKER_REWARDS_DISTRIBUTION_FREQUENCY: u32 = 432000;

type Assets<T> = assets::Pallet<T>;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

/// Denotes PSWAP rewards amounts of particular types available for user.
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default)]
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
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default)]
pub struct MarketMakerInfo {
    /// Number of eligible transactions - namely those with individual volume over 1 XOR.
    count: u32,
    /// Cumulative volume of eligible transactions.
    volume: Balance,
}

pub trait WeightInfo {
    fn claim_incentives() -> Weight;
    fn on_initialize(_n: u32) -> Weight;
    fn on_runtime_upgrade() -> Weight;
    fn set_asset_pair() -> Weight;
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
            RewardReason::LiquidityProvisionFarming => T::GetFarmingRewardsAccountId::get(),
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

    fn allowed_market_making_assets() -> Vec<T::AssetId> {
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
}

impl<T: Config> OnPswapBurned for Module<T> {
    /// NOTE: currently is not invoked.
    /// Invoked when pswap is burned after being exchanged from collected liquidity provider fees.
    fn on_pswap_burned(distribution: PswapRemintInfo) {
        Pallet::<T>::distribute_limits(distribution.vesting)
    }
}

impl<T: Config> VestedRewardsPallet<T::AccountId, T::AssetId> for Module<T> {
    /// Check if volume is eligible to be counted for market maker rewards and add it to registry.
    /// `count` is used as a multiplier if multiple times same volume is transferred inside transaction.
    fn update_market_maker_records(
        account_id: &T::AccountId,
        xor_volume: Balance,
        count: u32,
        from_asset_id: &T::AssetId,
        to_asset_id: &T::AssetId,
    ) -> DispatchResult {
        if MarketMakingPairs::<T>::contains_key(from_asset_id, to_asset_id)
            && xor_volume >= balance!(1)
        {
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
    use common::XOR;
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
        type GetFarmingRewardsAccountId: Get<Self::AccountId>;
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
            Self::allowed_market_making_assets()
                .into_iter()
                .filter(|id| !MarketMakingPairs::<T>::contains_key(&T::AssetId::from(XOR), &id))
                .for_each(|id| MarketMakingPairs::<T>::insert(&T::AssetId::from(XOR), &id, ()));
            Self::allowed_market_making_assets()
                .into_iter()
                .filter(|id| !MarketMakingPairs::<T>::contains_key(&id, &T::AssetId::from(XOR)))
                .for_each(|id| MarketMakingPairs::<T>::insert(&id, &T::AssetId::from(XOR), ()));
            migration::migrate::<T>() + <T as Config>::WeightInfo::on_runtime_upgrade()
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

        /// Allow/disallow a market making pair.
        #[pallet::weight(<T as Config>::WeightInfo::set_asset_pair())]
        #[transactional]
        pub fn set_asset_pair(
            origin: OriginFor<T>,
            from_asset_id: T::AssetId,
            to_asset_id: T::AssetId,
            market_making_rewards_allowed: bool,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let error = if market_making_rewards_allowed {
                Error::<T>::MarketMakingPairAlreadyAllowed
            } else {
                Error::<T>::MarketMakingPairAlreadyDisallowed
            };

            ensure!(
                MarketMakingPairs::<T>::contains_key(&from_asset_id, &to_asset_id)
                    != market_making_rewards_allowed,
                error
            );

            if market_making_rewards_allowed {
                MarketMakingPairs::<T>::insert(from_asset_id, to_asset_id, ());
            } else {
                MarketMakingPairs::<T>::remove(from_asset_id, to_asset_id);
            }

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
        /// Account holding dedicated reward reserves is empty. This likely means that some of reward programmes have finished.
        RewardsSupplyShortage,
        /// Increment account reference error.
        IncRefError,
        /// Attempt to subtract more via snapshot than assigned to user.
        CantSubtractSnapshot,
        /// Failed to perform reward calculation.
        CantCalculateReward,
        /// The market making pair already allowed.
        MarketMakingPairAlreadyAllowed,
        /// The market making pair is disallowed.
        MarketMakingPairAlreadyDisallowed,
    }

    #[pallet::event]
    #[pallet::metadata(AccountIdOf<T> = "AccountId")]
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

    /// Market making pairs storage.
    #[pallet::storage]
    #[pallet::getter(fn market_making_pairs)]
    pub type MarketMakingPairs<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AssetId,
        Blake2_128Concat,
        T::AssetId,
        (),
        ValueQuery,
    >;
}
