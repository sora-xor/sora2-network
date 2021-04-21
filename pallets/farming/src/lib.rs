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

use common::prelude::FixedWrapper;
use common::{balance, Balance};
use frame_support::traits::Get;
use frame_support::weights::Weight;
use pswap_distribution::SubscribedAccounts;
use sp_arithmetic::traits::UniqueSaturatedInto;
use sp_runtime::traits::Zero;
use sp_std::collections::btree_map::{BTreeMap, Entry};
use sp_std::vec::Vec;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"farming";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub trait WeightInfo {
    fn create() -> Weight;
    fn lock_to_farm() -> Weight;
    fn unlock_from_farm() -> Weight;
}

impl WeightInfo for () {
    fn create() -> Weight {
        100_000_000
    }
    fn lock_to_farm() -> Weight {
        100_000_000
    }
    fn unlock_from_farm() -> Weight {
        100_000_000
    }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

impl<T: Config> Pallet<T> {
    fn refresh_farmers(now: T::BlockNumber) -> Weight {
        let mut function_weight: Weight = 0;

        let token_ids: Vec<_> = SubscribedAccounts::<T>::iter_values()
            .map(|(_, token_id, ..)| token_id)
            .collect();
        function_weight =
            function_weight.saturating_add(T::DbWeight::get().reads(token_ids.len() as u64));

        let mut farmers = BTreeMap::new();
        for (account_id, currency_id, data) in Accounts::<T>::iter() {
            function_weight = function_weight.saturating_add(T::DbWeight::get().reads(1));

            if !token_ids.contains(&currency_id) || data.free.is_zero() {
                continue;
            }

            let farmer_weight = Self::get_farmer_weight(&currency_id, data.free);
            if farmer_weight == 0 {
                continue;
            }

            match farmers.entry(account_id.clone()) {
                Entry::Vacant(entry) => {
                    let block_number = if let Some(previous_farmer) = Farmers::<T>::get(&account_id)
                    {
                        previous_farmer.1
                    } else {
                        now
                    };
                    entry.insert((farmer_weight, block_number));

                    function_weight = function_weight.saturating_add(T::DbWeight::get().reads(1));
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().0 += farmer_weight;
                }
            }
        }
        Farmers::<T>::remove_all();
        for (account_id, data) in farmers {
            Farmers::<T>::insert(account_id, data);
            function_weight = function_weight.saturating_add(T::DbWeight::get().writes(1))
        }

        function_weight
    }

    fn get_farmer_weight(token_id: &T::AssetId, token_count: Balance) -> Balance {
        let pool_account =
            if let Ok(pool_account) = pool_xyk::Module::<T>::get_pool_account(token_id) {
                pool_account
            } else {
                return 0;
            };
        let xor = pool_xyk::Module::<T>::get_xor_part_from_pool_account(&pool_account, token_count)
            .unwrap_or(0);
        if xor < balance!(1) {
            return 0;
        }

        let pool_doubles_reward = pool_xyk::Module::<T>::get_pool_trading_pair(&pool_account)
            .map(|trading_pair| {
                T::RewardDoublingAssets::get()
                    .iter()
                    .any(|asset_id| trading_pair.consists_of(asset_id))
            })
            .unwrap_or(false);

        if pool_doubles_reward {
            xor * 2
        } else {
            xor
        }
    }

    fn vest(now: T::BlockNumber) -> Weight {
        let mut function_weight: Weight = 0;

        let mut total_weight = FixedWrapper::from(0);
        let mut weights = Vec::new();
        let farming_life_time: u32 = now.unique_saturated_into();
        let farming_life_time = FixedWrapper::from(balance!(farming_life_time));
        let reward = {
            let reward_per_day = FixedWrapper::from(T::PSWAP_PER_DAY);
            let freq: u128 = T::VESTING_FREQUENCY.unique_saturated_into();
            let blocks: u128 = T::BLOCKS_PER_DAY.unique_saturated_into();
            let reward_vesting_part =
                FixedWrapper::from(balance!(freq)) / FixedWrapper::from(balance!(blocks));
            reward_per_day * reward_vesting_part
        };
        for (account_id, (weight, block_number)) in Farmers::<T>::iter() {
            function_weight = function_weight.saturating_add(T::DbWeight::get().reads(1));

            // Ti
            let farmer_farming_time: u32 = (now - block_number).unique_saturated_into();
            let farmer_farming_time = FixedWrapper::from(balance!(farmer_farming_time));

            // Vi(t)
            let coeff = (FixedWrapper::from(balance!(1))
                + farmer_farming_time / farming_life_time.clone())
            .pow(T::VESTING_COEFF);

            let weight = coeff * weight;
            weights.push((account_id, weight.clone()));

            total_weight = total_weight + weight;
        }

        for (account_id, weight) in weights {
            let account_reward = reward.clone() * weight / total_weight.clone();
            let account_reward = account_reward.try_into_balance().unwrap_or(0);
            let mut rewards = VestedRewards::<T>::get(&account_id);
            rewards += account_reward;
            VestedRewards::<T>::insert(&account_id, rewards);
        }

        function_weight
    }
}

pub use pallet::*;
use tokens::Accounts;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use assets::AssetIdOf;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Zero;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + assets::Config
        + permissions::Config
        + technical::Config
        + tokens::Config<Balance = Balance, CurrencyId = <Self as assets::Config>::AssetId>
        + pool_xyk::Config
        + pswap_distribution::Config
    {
        const PSWAP_PER_DAY: Balance;
        const REFRESH_FREQUENCY: BlockNumberFor<Self>;
        const VESTING_COEFF: u32;
        const VESTING_FREQUENCY: BlockNumberFor<Self>;
        const BLOCKS_PER_DAY: BlockNumberFor<Self>;
        type RewardDoublingAssets: Get<Vec<AssetIdOf<Self>>>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: T::BlockNumber) -> Weight {
            if now.is_zero() {
                0
            } else if (now % T::VESTING_FREQUENCY).is_zero() {
                let w1 = Self::refresh_farmers(now);
                let w2 = Self::vest(now);
                w1.saturating_add(w2)
            } else if (now % T::REFRESH_FREQUENCY).is_zero() {
                Self::refresh_farmers(now)
            } else {
                0
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::error]
    pub enum Error<T> {
        NotEnoughPermissions,
        FarmNotFound,
        FarmerNotFound,
        ShareNotFound,
        TechAccountIsMissing,
        FarmAlreadyClosed,
        FarmLocked,
        CalculationFailed,
        CalculationOrOperationWithFarmingStateIsFailed,
        SomeValuesIsNotSet,
        AmountIsOutOfAvailableValue,
        UnableToConvertAssetIdToTechAssetId,
        UnableToGetPoolInformationFromTechAsset,
        ThisTypeOfLiquiditySourceIsNotImplementedOrSupported,
        NothingToClaim,
        CaseIsNotSupported,
        /// Increment account reference error.
        IncRefError,
    }

    #[pallet::storage]
    pub type Farmers<T: Config> =
        StorageMap<_, Identity, T::AccountId, (Balance, T::BlockNumber), OptionQuery>;

    #[pallet::storage]
    pub type VestedRewards<T: Config> = StorageMap<_, Identity, T::AccountId, Balance, ValueQuery>;
}
