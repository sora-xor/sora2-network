use core::marker::PhantomData;

use crate::{Assets, Config, CrowdloanReward, Rewards, LEASE_TOTAL_DAYS};
use codec::Decode;
use common::{Balance, Fixed, RewardReason, PSWAP, VAL, XSTUSD};
use frame_support::dispatch::GetStorageVersion;
use frame_support::log;
use frame_support::traits::{Get, OnRuntimeUpgrade, StorageVersion};
use frame_support::weights::Weight;
use sp_io::MultiRemovalResults;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
use traits::MultiCurrency;

pub(crate) mod deprecated {
    use super::*;

    use crate::Pallet;
    use codec::Encode;
    use common::prelude::Balance;
    use frame_support::pallet_prelude::ValueQuery;
    use frame_support::Blake2_128Concat;

    /// Denotes information about users who make transactions counted for market makers strategic
    /// rewards programme. To participate in rewards distribution account needs to get 500+ tx's over 1
    /// XOR in volume each.
    #[derive(
        Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, Default, scale_info::TypeInfo,
    )]
    pub struct MarketMakerInfo {
        /// Number of eligible transactions - namely those with individual volume over 1 XOR.
        count: u32,
        /// Cumulative volume of eligible transactions.
        volume: Balance,
    }

    /// Registry of market makers with large transaction volumes (>1 XOR per transaction).
    #[frame_support::storage_alias]
    pub type MarketMakersRegistry<T: Config> = StorageMap<
        Pallet<T>,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        MarketMakerInfo,
        ValueQuery,
    >;

    /// Market making pairs storage.
    #[frame_support::storage_alias]
    pub type MarketMakingPairs<T: Config> = StorageDoubleMap<
        Pallet<T>,
        Blake2_128Concat,
        <T as assets::Config>::AssetId,
        Blake2_128Concat,
        <T as assets::Config>::AssetId,
        (),
        ValueQuery,
    >;
}

const CROWDLOAN_REWARDS: &'static str = include_str!("../crowdloan_rewards.json");

pub struct ResetClaimingForCrowdloadErrors<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for ResetClaimingForCrowdloadErrors<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut total_weight = Weight::zero();
        let version = crate::Pallet::<T>::on_chain_storage_version();
        let reset_claiming_version = StorageVersion::new(2);
        if version < reset_claiming_version {
            let weight = reset_claiming_for_crowdloan_errors::<T>();
            reset_claiming_version.put::<crate::Pallet<T>>();
            total_weight += weight;
        }
        total_weight
    }
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

pub fn add_crowdloan_rewards<T: Config>() -> Weight {
    let rewards = serde_json::from_str::<Vec<CrowdloanReward>>(CROWDLOAN_REWARDS)
        .expect("Can't deserialize crowdloan contributors.");

    let mut number_of_writes = 0;
    rewards.into_iter().for_each(|reward| {
        crate::CrowdloanRewards::<T>::insert(
            T::AccountId::decode(&mut &reward.address[..])
                .expect("Can't decode contributor address."),
            reward,
        );
        number_of_writes += 1;
    });

    T::DbWeight::get().writes(number_of_writes)
}

pub fn add_funds_to_crowdloan_rewards_account<T: Config>() -> Weight {
    if let Err(e) = T::Currency::deposit(
        VAL.into(),
        &T::GetCrowdloanRewardsAccountId::get(),
        crate::VAL_CROWDLOAN_REWARDS,
    ) {
        log::error!(target: "runtime", "Failed to add VAL crowdloan rewards: {:?}", e);
    }

    if let Err(e) = T::Currency::deposit(
        PSWAP.into(),
        &T::GetCrowdloanRewardsAccountId::get(),
        crate::PSWAP_CROWDLOAN_REWARDS,
    ) {
        log::error!(target: "runtime", "Failed to add PSWAP crowdloan rewards: {:?}", e);
    }

    if let Err(e) = T::Currency::deposit(
        XSTUSD.into(),
        &T::GetCrowdloanRewardsAccountId::get(),
        crate::XSTUSD_CROWDLOAN_REWARDS,
    ) {
        log::error!(target: "runtime", "Failed to add XSTUSD crowdloan rewards: {:?}", e);
    }

    T::DbWeight::get().writes(3)
}

pub struct MoveMarketMakerRewardPoolToLiquidityProviderPool<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for MoveMarketMakerRewardPoolToLiquidityProviderPool<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut total_weight = Weight::zero();
        let market_maker_cancel_version = StorageVersion::new(3);
        let version = crate::Pallet::<T>::on_chain_storage_version();
        if version < market_maker_cancel_version {
            let weight = move_market_making_rewards_to_liquidity_provider_rewards_pool::<T>();
            market_maker_cancel_version.put::<crate::Pallet<T>>();
            total_weight += weight;
        }
        total_weight
    }
}

pub fn move_market_making_rewards_to_liquidity_provider_rewards_pool<T: Config>() -> Weight {
    let mut weight = Weight::zero();

    let mut res: MultiRemovalResults = deprecated::MarketMakingPairs::<T>::clear(u32::MAX, None);
    weight += T::DbWeight::get().reads_writes(res.loops.into(), res.backend.into());
    while let Some(cursor) = res.maybe_cursor {
        res = deprecated::MarketMakingPairs::<T>::clear(res.backend, Some(cursor.as_slice()));
        weight += T::DbWeight::get().reads_writes(res.loops.into(), res.backend.into());
    }

    res = deprecated::MarketMakersRegistry::<T>::clear(u32::MAX, None);
    while let Some(cursor) = res.maybe_cursor {
        res = deprecated::MarketMakersRegistry::<T>::clear(res.backend, Some(cursor.as_slice()));
    }

    let market_making_reward_account = T::GetMarketMakerRewardsAccountId::get();
    let liquidity_providing_reward_account = T::GetBondingCurveRewardsAccountId::get();
    let amount = match Assets::<T>::total_balance(&PSWAP.into(), &market_making_reward_account) {
        Ok(amount) => amount,
        Err(err) => {
            log::error!(target: "runtime", "Failed to transfer tokens from market maker reward pool to liquidity provider reward pool: {:?}", err);
            return T::DbWeight::get().reads(1);
        }
    };
    if let Err(err) = Assets::<T>::transfer_from(
        &PSWAP.into(),
        &market_making_reward_account,
        &liquidity_providing_reward_account,
        amount,
    ) {
        log::error!(target: "runtime", "Failed to transfer tokens from market maker reward pool to liquidity provider reward pool: {:?}", err);
    }

    weight += T::DbWeight::get().reads_writes(2, 2);

    for id in Rewards::<T>::iter_keys() {
        Rewards::<T>::mutate(id, |reward_info| {
            let market_maker_rewards = *reward_info
                .rewards
                .get(&RewardReason::DeprecatedMarketMakerVolume)
                .unwrap_or(&Balance::zero());
            weight += T::DbWeight::get().reads(1);
            if let Some(balance) = reward_info
                .rewards
                .get_mut(&RewardReason::BuyOnBondingCurve)
            {
                *balance += market_maker_rewards;
                weight += T::DbWeight::get().writes(1);
            }
            reward_info
                .rewards
                .remove(&RewardReason::DeprecatedMarketMakerVolume);
            weight += T::DbWeight::get().writes(1);
        });
    }

    weight
}
