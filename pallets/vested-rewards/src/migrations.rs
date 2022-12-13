use core::marker::PhantomData;

use crate::{Config, CrowdloanReward, LEASE_TOTAL_DAYS};
use codec::Decode;
use common::{Fixed, PSWAP, VAL, XSTUSD};
use frame_support::dispatch::GetStorageVersion;
use frame_support::log;
use frame_support::traits::{Get, OnRuntimeUpgrade, StorageVersion};
use frame_support::weights::Weight;
use sp_std::prelude::*;
use traits::MultiCurrency;

const CROWDLOAN_REWARDS: &'static str = include_str!("../crowdloan_rewards.json");

pub struct ResetClaimingForCrowdloadErrors<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for ResetClaimingForCrowdloadErrors<T> {
    fn on_runtime_upgrade() -> Weight {
        let version = crate::Pallet::<T>::on_chain_storage_version();
        let new_version = StorageVersion::new(2);
        if version < new_version {
            let weight = reset_claiming_for_crowdloan_errors::<T>();
            new_version.put::<crate::Pallet<T>>();
            weight
        } else {
            0
        }
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
