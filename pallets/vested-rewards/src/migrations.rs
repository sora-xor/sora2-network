use core::marker::PhantomData;

use crate::*;
use codec::{Decode, Encode};
use common::balance;
use common::{AssetInfoProvider, AssetManager, FromGenericPair};
use common::{Balance, Fixed, PSWAP, VAL, XSTUSD};
use frame_support::pallet_prelude::GetStorageVersion;
use frame_support::traits::{Get, OnRuntimeUpgrade, StorageVersion};
use frame_support::weights::Weight;
use serde::{Deserialize, Serialize};
use sp_io::MultiRemovalResults;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;

pub mod v4 {
    use super::*;
    use common::CrowdloanTag;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::BlockNumberFor;
    use sp_runtime::traits::UniqueSaturatedInto;

    /// A vested reward for crowdloan.
    #[derive(
        Encode,
        Decode,
        Deserialize,
        Serialize,
        Clone,
        Debug,
        Default,
        PartialEq,
        scale_info::TypeInfo,
    )]
    pub struct CrowdloanReward {
        /// The user id
        #[serde(with = "serde_bytes", rename = "ID")]
        pub id: Vec<u8>,
        /// The user address
        #[serde(with = "hex", rename = "Address")]
        pub address: Vec<u8>,
        /// Kusama contribution
        #[serde(rename = "Contribution")]
        pub contribution: Fixed,
        /// Reward in XOR
        #[serde(rename = "XOR Reward")]
        pub xor_reward: Fixed,
        /// Reward in VAL
        #[serde(rename = "Val Reward")]
        pub val_reward: Fixed,
        /// Reward in PSWAP
        #[serde(rename = "PSWAP Reward")]
        pub pswap_reward: Fixed,
        /// Reward in XSTUSD
        #[serde(rename = "XSTUSD Reward")]
        pub xstusd_reward: Fixed,
        /// Reward in percents of the total contribution
        #[serde(rename = "Percent")]
        pub percent: Fixed,
    }

    /// Crowdloan vested rewards storage.
    #[frame_support::storage_alias]
    pub type CrowdloanRewards<T: crate::Config> = StorageMap<
        crate::Pallet<T>,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        CrowdloanReward,
        ValueQuery,
    >;

    /// This storage keeps the last block number, when the user (the first) claimed a reward for
    /// asset (the second key). The block is rounded to days.
    #[frame_support::storage_alias]
    pub type CrowdloanClaimHistory<T: Config> = StorageDoubleMap<
        crate::Pallet<T>,
        Blake2_128Concat,
        <T as frame_system::Config>::AccountId,
        Blake2_128Concat,
        AssetIdOf<T>,
        BlockNumberFor<T>,
        ValueQuery,
    >;

    pub const VAL_CROWDLOAN_REWARDS: Balance = balance!(676393);
    pub const PSWAP_CROWDLOAN_REWARDS: Balance = balance!(9363480);
    pub const XSTUSD_CROWDLOAN_REWARDS: Balance = balance!(77050);
    pub const BLOCKS_PER_DAY: u128 = 14400;
    pub const LEASE_START_BLOCK: u128 = 4_397_212;
    pub const LEASE_TOTAL_DAYS: u128 = 318;
    pub const CROWDLOAN_TAG: &[u8] = b"crowdloan";
    pub struct Migration<T: crate::Config>(PhantomData<T>);

    impl<T: crate::Config> OnRuntimeUpgrade for Migration<T> {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if crate::Pallet::<T>::on_chain_storage_version() == 3 {
                log::info!(
                    "Applying migration to version 3: New crowdloan rewards vesting implementation"
                );
                if let Err(err) = common::with_transaction(migrate::<T>) {
                    log::error!(
                        "Failed to migrate crowdloan rewards, state reverted: {}",
                        err
                    );
                } else {
                    StorageVersion::new(4).put::<crate::Pallet<T>>();
                }
                <T as frame_system::Config>::BlockWeights::get().max_block
            } else {
                log::error!(
                    "Runtime upgrade executed with wrong storage version, expected 3, got {:?}",
                    crate::Pallet::<T>::on_chain_storage_version()
                );
                <T as frame_system::Config>::DbWeight::get().reads(1)
            }
        }
    }

    pub fn migrate<T: Config>() -> Result<(), &'static str> {
        let tag = CrowdloanTag(
            CROWDLOAN_TAG
                .to_vec()
                .try_into()
                .expect("tag less than 128 bytes long"),
        );
        let tech_account = T::TechAccountId::from_generic_pair(
            crate::TECH_ACCOUNT_PREFIX.to_vec(),
            tag.0.clone().to_vec(),
        );
        technical::Pallet::<T>::register_tech_account_id_if_not_exist(&tech_account)?;
        let account = technical::Pallet::<T>::tech_account_id_to_account_id(&tech_account)?;

        let mut total_contribution = 0;
        for (account, reward_info) in CrowdloanRewards::<T>::drain() {
            let contribution = reward_info.contribution.into_bits() as Balance;
            total_contribution += contribution;
            let mut rewarded = vec![];
            for (asset_id, last_claim_block) in CrowdloanClaimHistory::<T>::drain_prefix(&account) {
                let last_claim_block: u128 = last_claim_block.unique_saturated_into();
                let claimed_period = if !last_claim_block.is_zero() {
                    last_claim_block.saturating_sub(LEASE_START_BLOCK)
                } else {
                    continue;
                };
                let claimed_days = Fixed::try_from(claimed_period / BLOCKS_PER_DAY)
                    .map_err(|_| "failed to calculate claim_days")?;

                let reward = if asset_id == VAL.into() {
                    reward_info.val_reward
                } else if asset_id == PSWAP.into() {
                    reward_info.pswap_reward
                } else if asset_id == XSTUSD.into() {
                    reward_info.xstusd_reward
                } else {
                    return Err("wrong asset id in CrowdloanClaimHistoryStorage");
                };

                let reward = reward
                    / Fixed::try_from(LEASE_TOTAL_DAYS)
                        .map_err(|_| "failed to calculate reward per day")?
                        .into();

                let claimed_reward = (reward * claimed_days)
                    .try_into_balance()
                    .map_err(|_| "failed to calculate claimed reward")?;
                rewarded.push((asset_id, claimed_reward));
            }
            log::debug!("Add crowdloan user info, account: {account:?}, tag: {tag:?}, rewarded: {rewarded:?}, contribution: {contribution}");
            CrowdloanUserInfos::<T>::insert(
                account,
                &tag,
                CrowdloanUserInfo {
                    rewarded,
                    contribution,
                },
            );
        }
        log::debug!(
            "Add crowdloan info, total contribution: {total_contribution}, account: {account:?}"
        );
        CrowdloanInfos::<T>::insert(
            &tag,
            CrowdloanInfo {
                total_contribution,
                rewards: vec![
                    (PSWAP.into(), PSWAP_CROWDLOAN_REWARDS),
                    (VAL.into(), VAL_CROWDLOAN_REWARDS),
                    (XSTUSD.into(), XSTUSD_CROWDLOAN_REWARDS),
                ],
                start_block: LEASE_START_BLOCK.unique_saturated_into(),
                length: (LEASE_TOTAL_DAYS * BLOCKS_PER_DAY).unique_saturated_into(),
                account,
            },
        );
        Ok(())
    }
}

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
        AssetIdOf<T>,
        Blake2_128Concat,
        AssetIdOf<T>,
        (),
        ValueQuery,
    >;
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
    let amount = match <T as Config>::AssetInfoProvider::total_balance(
        &PSWAP.into(),
        &market_making_reward_account,
    ) {
        Ok(amount) => amount,
        Err(err) => {
            log::error!(target: "runtime", "Failed to transfer tokens from market maker reward pool to liquidity provider reward pool: {:?}", err);
            return T::DbWeight::get().reads(1);
        }
    };
    if let Err(err) = T::AssetManager::transfer_from(
        &PSWAP.into(),
        &market_making_reward_account,
        &liquidity_providing_reward_account,
        amount,
    ) {
        log::error!(target: "runtime", "Failed to transfer tokens from market maker reward pool to liquidity provider reward pool: {:?}", err);
    }

    weight += T::DbWeight::get().reads_writes(2, 2);

    Rewards::<T>::translate_values(|mut reward_info: RewardInfo| {
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
        Some(reward_info)
    });

    weight
}
