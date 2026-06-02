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

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod migrations;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(feature = "runtime-benchmarks")]
mod utils;
pub mod weights;

use codec::{Decode, Encode};
use common::AssetIdOf;
use common::{GetBaseAssetIdOf, RewardReason, TradingPair};
use frame_support::dispatch::DispatchResult;
use frame_support::traits::Get;
use frame_support::weights::Weight;
use frame_system::pallet_prelude::BlockNumberFor;
use pool_xyk::PoolProviders;
use sp_arithmetic::traits::UniqueSaturatedInto;
use sp_runtime::DispatchError;
use sp_std::collections::btree_map::{BTreeMap, Entry};
use sp_std::vec::Vec;

use common::prelude::QuoteAmount;
use common::{
    balance, AccountIdOf, Balance, DexIdOf, FixedWrapper256, LiquiditySource, OnPoolCreated,
    TradingPairSourceManager,
};

pub type WeightInfoOf<T> = <T as Config>::WeightInfo;
pub use weights::WeightInfo;

impl<T: Config> OnPoolCreated for Pallet<T> {
    type AccountId = AccountIdOf<T>;
    type DEXId = DexIdOf<T>;

    fn on_pool_created(
        _fee_account: Self::AccountId,
        _dex_id: Self::DEXId,
        pool_account: Self::AccountId,
    ) -> DispatchResult {
        Self::add_pool(pool_account, frame_system::Pallet::<T>::block_number());
        Ok(())
    }
}

impl<T: Config> Pallet<T> {
    fn add_pool(pool_account: AccountIdOf<T>, block_number: BlockNumberFor<T>) {
        Pools::<T>::mutate(block_number % T::REFRESH_FREQUENCY, |pools| {
            pools.push(pool_account)
        });
    }

    fn refresh_pools(now: BlockNumberFor<T>) -> Weight {
        let mut total_weight = Weight::zero();
        let pools = Pools::<T>::get(now % T::REFRESH_FREQUENCY);
        for pool in pools {
            let read_count = Self::refresh_pool(pool, now);
            total_weight = total_weight.saturating_add(WeightInfoOf::<T>::refresh_pool(read_count));
        }
        total_weight
    }

    fn get_multiplier(asset_id: &AssetIdOf<T>) -> Result<FixedWrapper256, DispatchError> {
        let base_asset = GetBaseAssetIdOf::<T>::get();
        if asset_id == &base_asset {
            Ok(FixedWrapper256::from(balance!(1)))
        } else {
            let (outcome, _) = pool_xyk::Pallet::<T>::quote(
                &common::DEXId::Polkaswap.into(),
                &base_asset,
                asset_id,
                QuoteAmount::with_desired_output(balance!(1)),
                false,
            )?;
            frame_support::__private::log::debug!("{outcome:?}");
            Ok(FixedWrapper256::from(outcome.amount))
        }
    }

    fn refresh_pool(pool: T::AccountId, now: BlockNumberFor<T>) -> u32 {
        let old_farmers = PoolFarmers::<T>::get(&pool);
        let trading_pair = match pool_xyk::Pallet::<T>::get_pool_trading_pair(&pool) {
            Ok(trading_pair) => trading_pair,
            Err(err) => {
                frame_support::__private::log::warn!(
                    "Failed to get trading pair for {pool:?} pool: {err:?}",
                );
                if !old_farmers.is_empty() {
                    PoolFarmers::<T>::remove(&pool);
                }
                return 0;
            }
        };
        let multiplier = match Self::get_multiplier(&trading_pair.base_asset_id) {
            Ok(multiplier) => multiplier,
            Err(err) => {
                frame_support::__private::log::warn!(
                    "Failed to get farming rewards multiplier for {:?} asset: {err:?}",
                    trading_pair.base_asset_id
                );
                if !old_farmers.is_empty() {
                    PoolFarmers::<T>::remove(&pool);
                }
                return 0;
            }
        };
        frame_support::__private::log::debug!("Multiplier for TP {trading_pair:?}: {multiplier:?}");
        let mut read_count = 0;
        let mut new_farmers = Vec::new();
        let Some(pool_total_liquidity) = pool_xyk::TotalIssuances::<T>::get(&pool) else {
            frame_support::__private::log::warn!(
                "Failed to get total issuance for pool {:?}",
                pool
            );
            if !old_farmers.is_empty() {
                PoolFarmers::<T>::remove(&pool);
            }
            return 0;
        };
        let Ok((pool_base_reserves, _, _)) = pool_xyk::Pallet::<T>::get_actual_reserves(
            &pool,
            &trading_pair.base_asset_id,
            &trading_pair.base_asset_id,
            &trading_pair.target_asset_id,
        )
        .map_err(|e| {
            frame_support::__private::log::warn!(
                "Failed to get base reserves for pool {:?}: {:?}",
                pool,
                e
            );
            e
        }) else {
            if !old_farmers.is_empty() {
                PoolFarmers::<T>::remove(&pool);
            }
            return 0;
        };
        for (account, pool_tokens) in PoolProviders::<T>::iter_prefix(&pool) {
            read_count += 1;

            let weight = match Self::get_account_weight(
                &trading_pair,
                multiplier.clone(),
                pool_base_reserves,
                pool_total_liquidity,
                pool_tokens,
            ) {
                Ok(weight) => weight,
                Err(err) => {
                    frame_support::__private::log::debug!(
                        "Failed to calculate farming weight for pool {:?}, account {:?}: {:?}",
                        pool,
                        account,
                        err
                    );
                    continue;
                }
            };
            if weight == 0 {
                continue;
            }

            let block = if let Some(farmer) = old_farmers.iter().find(|f| f.account == account) {
                farmer.block
            } else {
                // Pools are refreshed at different blocks for performance reasons.
                // However, reward calculation should not be affected.
                // 1205 becomes 1200, given REFRESH_FREQUENCY = 1200
                now - (now % T::REFRESH_FREQUENCY)
            };

            new_farmers.push(PoolFarmer {
                account,
                block,
                weight,
            });
        }

        // Either add new farmers or remove old farmers
        if !new_farmers.is_empty() || !old_farmers.is_empty() {
            PoolFarmers::<T>::insert(&pool, new_farmers);
        }

        read_count
    }

    fn get_account_weight(
        trading_pair: &TradingPair<AssetIdOf<T>>,
        multiplier: FixedWrapper256,
        base_reserves: Balance,
        total_liquidity: Balance,
        pool_tokens: Balance,
    ) -> Result<Balance, DispatchError> {
        if pool_tokens == 0 || total_liquidity == 0 || pool_tokens > total_liquidity {
            return Ok(0);
        }

        let base_asset_amt = pool_xyk::Pallet::<T>::get_base_asset_part(
            base_reserves,
            total_liquidity,
            pool_tokens,
        )?;

        let base_asset_amt = (FixedWrapper256::from(base_asset_amt) * multiplier)
            .try_into_balance()
            .map_err(|_| Error::<T>::ArithmeticError)?;

        if base_asset_amt < Self::lp_min_xor_for_bonus_reward() {
            return Ok(0);
        }

        let pool_doubles_reward = T::RewardDoublingAssets::get()
            .iter()
            .any(|asset_id| trading_pair.contains(asset_id));

        let weight = if pool_doubles_reward {
            base_asset_amt
                .checked_mul(2)
                .ok_or(Error::<T>::ArithmeticError)?
        } else {
            base_asset_amt
        };
        Ok(weight)
    }

    fn vest(now: BlockNumberFor<T>) -> Weight {
        let mut accounts = BTreeMap::new();
        let function_weight: Weight = Self::prepare_accounts_for_vesting(now, &mut accounts);
        let function_weight = function_weight.saturating_add(
            WeightInfoOf::<T>::vest_account_rewards(accounts.len() as u32),
        );
        if let Err(err) = Self::vest_account_rewards(accounts) {
            frame_support::__private::log::warn!("Failed to vest farming rewards: {:?}", err);
        }
        function_weight
    }

    fn prepare_accounts_for_vesting(
        now: BlockNumberFor<T>,
        accounts: &mut BTreeMap<T::AccountId, FixedWrapper256>,
    ) -> Weight {
        let mut pool_count = 0;
        let mut farmer_count = 0;
        for (_pool, farmers) in PoolFarmers::<T>::iter() {
            pool_count += 1;
            farmer_count += farmers.len() as u32;

            Self::prepare_pool_accounts_for_vesting(farmers, now, accounts);
        }

        WeightInfoOf::<T>::prepare_accounts_for_vesting(pool_count, farmer_count)
    }

    fn get_farmer_weight_amplified_by_time(
        farmer_weight: u128,
        farmer_block: BlockNumberFor<T>,
        now: BlockNumberFor<T>,
    ) -> FixedWrapper256 {
        // Ti
        let farmer_farming_time: u32 = (now - farmer_block).unique_saturated_into();
        let farmer_farming_time = FixedWrapper256::from(balance!(farmer_farming_time));

        // Vi(t)
        let now_u128: u128 = now.unique_saturated_into();
        let coeff = (FixedWrapper256::from(balance!(1))
            + farmer_farming_time.clone() / FixedWrapper256::from(balance!(now_u128)))
        .pow(T::VESTING_COEFF);

        coeff * FixedWrapper256::from(farmer_weight)
    }

    fn prepare_pool_accounts_for_vesting(
        farmers: Vec<PoolFarmer<T>>,
        now: BlockNumberFor<T>,
        accounts: &mut BTreeMap<T::AccountId, FixedWrapper256>,
    ) {
        if farmers.is_empty() {
            return;
        }

        for farmer in farmers {
            let weight =
                Self::get_farmer_weight_amplified_by_time(farmer.weight, farmer.block, now);

            match accounts.entry(farmer.account) {
                Entry::Vacant(entry) => {
                    entry.insert(weight);
                }
                Entry::Occupied(mut entry) => {
                    *entry.get_mut() = entry.get().clone() + weight;
                }
            }
        }
    }

    fn prepare_account_rewards(
        accounts: BTreeMap<T::AccountId, FixedWrapper256>,
    ) -> Result<BTreeMap<T::AccountId, u128>, DispatchError> {
        let total_weight = accounts
            .values()
            .fold(FixedWrapper256::from(0), |a, b| a + b.clone());

        let reward = {
            let reward_per_day = FixedWrapper256::from(T::PSWAP_PER_DAY);
            let freq: u128 = T::VESTING_FREQUENCY.unique_saturated_into();
            let blocks: u128 = <T as Config>::BLOCKS_PER_DAY.unique_saturated_into();
            let reward_vesting_part =
                FixedWrapper256::from(balance!(freq)) / FixedWrapper256::from(balance!(blocks));
            reward_per_day * reward_vesting_part
        };

        accounts
            .into_iter()
            .map(|(account, weight)| {
                let account_reward = reward.clone() * weight / total_weight.clone();
                let account_reward = account_reward
                    .try_into_balance()
                    .map_err(|_| Error::<T>::ArithmeticError)?;
                Ok((account, account_reward))
            })
            .collect()
    }

    fn vest_account_rewards(accounts: BTreeMap<T::AccountId, FixedWrapper256>) -> DispatchResult {
        let rewards = Self::prepare_account_rewards(accounts)?;

        common::with_transaction(|| {
            for (account, reward) in rewards {
                vested_rewards::Pallet::<T>::add_pending_reward(
                    &account,
                    RewardReason::LiquidityProvisionFarming,
                    reward,
                )?;
            }
            Ok(())
        })
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use common::AssetIdOf;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::schedule::v3::Anon;
    use frame_support::traits::StorageVersion;
    use frame_system::{ensure_root, pallet_prelude::*};
    use sp_runtime::traits::Zero;
    use sp_runtime::AccountId32;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        // pallet::config doesn't let to specify associated type constraints
        + frame_system::Config<AccountId = AccountId32>
        + permissions::Config
        + technical::Config
        + tokens::Config<Balance = Balance, CurrencyId = AssetIdOf<Self>>
        + pool_xyk::Config
        + vested_rewards::Config
    {
        #[allow(deprecated)]
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        const PSWAP_PER_DAY: Balance;
        const REFRESH_FREQUENCY: BlockNumberFor<Self>;
        const VESTING_COEFF: u32;
        /// How often the vesting happens. VESTING_FREQUENCY % REFRESH_FREQUENCY must be 0
        const VESTING_FREQUENCY: BlockNumberFor<Self>;
        const BLOCKS_PER_DAY: BlockNumberFor<Self>;
        type RuntimeCall: Parameter;
        type SchedulerOriginCaller: From<frame_system::RawOrigin<Self::AccountId>>;
        type Scheduler: Anon<frame_system::pallet_prelude::BlockNumberFor<Self>, <Self as Config>::RuntimeCall, Self::SchedulerOriginCaller>;
        type RewardDoublingAssets: Get<Vec<AssetIdOf<Self>>>;
        type TradingPairSourceManager: TradingPairSourceManager<Self::DEXId, AssetIdOf<Self>>;
        /// Weight information for extrinsics in this pallet.
        type WeightInfo: WeightInfo;
    }

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(now: BlockNumberFor<T>) -> Weight {
            if now.is_zero() {
                return Weight::zero();
            }

            let mut total_weight = Self::refresh_pools(now);

            if (now % T::VESTING_FREQUENCY).is_zero() {
                let weight = Self::vest(now);
                total_weight = total_weight.saturating_add(weight);
            }

            total_weight
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        /// Increment account reference error.
        IncRefError,
        /// Something is wrong with arithmetic - overflow happened, for example.
        ArithmeticError,
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// When Minimum XOR amount for Liquidity Provider Bonus Reward is updated
        LpMinXorForBonusRewardUpdated {
            new_lp_min_xor_for_bonus_reward: Balance,
            old_lp_min_xor_for_bonus_reward: Balance,
        },
    }

    #[pallet::type_value]
    pub fn DefaultLpMinXorForBonusReward<T: Config>() -> Balance {
        balance!(3000000)
    }

    /// Pools whose farmers are refreshed at the specific block. Block => Pools
    #[pallet::storage]
    pub type Pools<T: Config> =
        StorageMap<_, Identity, BlockNumberFor<T>, Vec<T::AccountId>, ValueQuery>;

    /// Farmers of the pool. Pool => Farmers
    #[pallet::storage]
    pub type PoolFarmers<T: Config> =
        StorageMap<_, Identity, T::AccountId, Vec<PoolFarmer<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn lp_min_xor_for_bonus_reward)]
    pub type LpMinXorForBonusReward<T: Config> =
        StorageValue<_, Balance, ValueQuery, DefaultLpMinXorForBonusReward<T>>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::set_lp_min_xor_for_bonus_reward())]
        pub fn set_lp_min_xor_for_bonus_reward(
            origin: OriginFor<T>,
            new_lp_min_xor_for_bonus_reward: Balance,
        ) -> DispatchResult {
            ensure_root(origin)?;
            let old_lp_min_xor_for_bonus_reward = <LpMinXorForBonusReward<T>>::get();
            <LpMinXorForBonusReward<T>>::put(new_lp_min_xor_for_bonus_reward);
            Self::deposit_event(Event::LpMinXorForBonusRewardUpdated {
                new_lp_min_xor_for_bonus_reward,
                old_lp_min_xor_for_bonus_reward,
            });
            Ok(())
        }
    }
}

pub mod rpc {
    use super::{AssetIdOf, Config, Pallet};
    use frame_support::traits::Get as _;
    use sp_std::prelude::*;

    impl<T: Config> Pallet<T> {
        pub fn reward_doubling_assets() -> Vec<AssetIdOf<T>> {
            T::RewardDoublingAssets::get()
        }
    }
}

#[derive(Debug, Encode, Decode, scale_info::TypeInfo)]
#[cfg_attr(test, derive(PartialEq))]
#[scale_info(skip_type_params(T))]
/// The specific farmer in the specific pool
pub struct PoolFarmer<T: Config> {
    /// The account of the farmer
    account: T::AccountId,
    /// The block that the farmer started farming at
    block: BlockNumberFor<T>,
    /// The weight the farmer has in the pool
    weight: Balance,
}
