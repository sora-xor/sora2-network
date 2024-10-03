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

//! Multicollateral bonding curve pool module benchmarking.

#![cfg(feature = "runtime-benchmarks")]
use super::*;
#[cfg(feature = "wip")] // ORML multi asset vesting
use core::str::FromStr;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
#[cfg(feature = "wip")] // ORML multi asset vesting
use frame_system::EventRecord;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;
use traits::MultiCurrency;

use common::{AssetManager, AssetName, AssetSymbol, CrowdloanTag, FromGenericPair, PSWAP, XOR};

#[cfg(feature = "wip")] // ORML multi asset vesting
use crate::vesting_currencies::{LinearPendingVestingSchedule, LinearVestingSchedule};
use crate::Pallet as VestedRewards;
use technical::Pallet as Technical;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}
#[cfg(feature = "wip")] // ORML multi asset vesting
fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27c");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn create_account<T: Config>(prefix: Vec<u8>, index: u128) -> T::AccountId {
    let tech_account: T::TechAccountId =
        T::TechAccountId::from_generic_pair(prefix, index.encode());
    Technical::<T>::tech_account_id_to_account_id(&tech_account).unwrap()
}

fn create_asset<T: Config>(prefix: &str, index: u128) -> AssetIdOf<T> {
    let asset_id = T::AssetManager::gen_asset_id_from_any(&(prefix, index));
    let name = format!("{prefix}{index}").as_bytes().to_vec();
    T::AssetManager::register_asset_id(
        alice::<T>(),
        asset_id.clone(),
        AssetSymbol(name.clone()),
        AssetName(name),
        18,
        balance!(100),
        true,
        common::AssetType::Regular,
        None,
        None,
    )
    .unwrap();
    asset_id
}

fn prepare_crowdloan_rewards<T: Config>(n: u128) -> Vec<(AssetIdOf<T>, Balance)> {
    let mut rewards = vec![];
    for i in 0..n {
        let asset_id = create_asset::<T>("TEST", i.into());
        rewards.push((asset_id, balance!(10)));
    }
    rewards
}

fn prepare_crowdloan_contributions<T: Config>(n: u128) -> Vec<(T::AccountId, Balance)> {
    let mut contributions = vec![];
    for i in 0..n {
        let account_id = create_account::<T>(b"Test".to_vec(), i.into());
        contributions.push((account_id, balance!(1)));
    }
    contributions
}

fn prepare_rewards_update<T: Config>(
    n: u128,
) -> BTreeMap<T::AccountId, BTreeMap<RewardReason, Balance>> {
    let mut rewards = BTreeMap::new();
    let reward: BTreeMap<RewardReason, Balance> = vec![
        (RewardReason::BuyOnBondingCurve, balance!(1)),
        (RewardReason::Crowdloan, balance!(1)),
    ]
    .into_iter()
    .collect();
    for i in 0..n {
        let user_account = create_account::<T>(b"user".to_vec(), i);
        rewards.insert(user_account, reward.clone());
    }
    rewards
}
#[cfg(feature = "wip")] // ORML multi asset vesting
fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}
#[cfg(feature = "wip")] // ORML multi asset vesting
benchmarks! {
    claim_rewards {
        let caller = alice::<T>();

        <T as common::Config>::MultiCurrency::deposit(PSWAP.into(), &T::GetBondingCurveRewardsAccountId::get(), balance!(100)).unwrap();
        <T as common::Config>::MultiCurrency::deposit(PSWAP.into(), &T::GetMarketMakerRewardsAccountId::get(), balance!(200)).unwrap();
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &caller, balance!(1)).unwrap(); // to prevent inc ref error

        VestedRewards::<T>::add_tbc_reward(&caller, balance!(100)).expect("Failed to add reward.");
        VestedRewards::<T>::distribute_limits(balance!(100));
    }: _(
        RawOrigin::Signed(caller.clone())
    )
    verify {
        assert_eq!(
            <T as common::Config>::MultiCurrency::free_balance(PSWAP.into(), &caller),
            balance!(100)
        );
    }

    update_rewards {
        let n in 0 .. 100;
        let rewards = prepare_rewards_update::<T>(n.into());
    }: {
        Pallet::<T>::update_rewards(RawOrigin::Root.into(), rewards).unwrap()
    }
    verify {
        assert_eq!(
            TotalRewards::<T>::get(),
            balance!(n) * 2
        );
    }

    register_crowdloan {
        let m in 1 .. 1000;
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &alice::<T>(), balance!(1)).unwrap(); // to prevent inc ref error
        let tag = CrowdloanTag(b"crowdloan".to_vec().try_into().unwrap());
        let rewards = prepare_crowdloan_rewards::<T>(10);
        let contributions = prepare_crowdloan_contributions::<T>(m as u128);
    }: _(RawOrigin::Root, tag.clone(), 0u32.into(), 100u32.into(), rewards, contributions)
    verify {
        assert!(crate::CrowdloanInfos::<T>::contains_key(&tag));
    }

    claim_crowdloan_rewards {
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &alice::<T>(), balance!(1)).unwrap(); // to prevent inc ref error
        let tag = CrowdloanTag(b"crowdloan".to_vec().try_into().unwrap());
        let rewards = prepare_crowdloan_rewards::<T>(5);
        let contributions = prepare_crowdloan_contributions::<T>(100);
        let account = contributions.get(0).cloned().unwrap().0;
        let first_asset_id = rewards.get(0).cloned().unwrap().0;
        Pallet::<T>::register_crowdloan(RawOrigin::Root.into(), tag.clone(), 0u32.into(), T::BLOCKS_PER_DAY * 4u32.into(), rewards.clone(), contributions).unwrap();
        let info = crate::CrowdloanInfos::<T>::get(&tag).unwrap();
        for (asset_id, _) in rewards {
            <T as common::Config>::MultiCurrency::deposit(asset_id.clone(), &info.account, balance!(1000)).unwrap();
        }
        frame_system::Pallet::<T>::set_block_number(T::BLOCKS_PER_DAY);
    }: _(RawOrigin::Signed(account.clone()), tag.clone())
    verify {
        assert_eq!(
            <T as common::Config>::MultiCurrency::free_balance(first_asset_id, &account),
            balance!(0.025) // 10 / 100 / 4
        );
    }

    claim_unlocked {
        let caller: T::AccountId = alice::<T>();
        let asset_id: AssetIdOf<T> = create_asset::<T>("TEST", 0);
        let max_schedules = T::MaxVestingSchedules::get();
        let mut schedules: BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules> =
                    BoundedVec::default();

        let vesting_schedule = VestingScheduleOf::<T>::LinearVestingSchedule(LinearVestingSchedule {
                asset_id,
                start: T::BlockNumber::from(0_u32),
                period: T::BlockNumber::from(1_u32),
                period_count: 1,
                per_period: balance!(1),
            });
        schedules.try_push(vesting_schedule).expect("Error while push to BoundedVec");
        for i in 1..max_schedules {
            let asset_id_temp: AssetIdOf<T> = create_asset::<T>("TEST", i.into());
            let vesting_schedule = VestingScheduleOf::<T>::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: asset_id_temp,
                start: T::BlockNumber::from(0_u32),
                period: T::BlockNumber::from(1_u32),
                period_count: 1,
                per_period: balance!(1),
            });
            schedules.try_push(vesting_schedule).expect("Error while push to BoundedVec");
        }
        <VestingSchedules<T>>::insert(caller.clone(), schedules);
        frame_system::Pallet::<T>::set_block_number(T::BlockNumber::from(2_u32));
    }: _(RawOrigin::Signed(caller.clone()), asset_id)
    verify {
        assert_eq!(VestingSchedules::<T>::get(&caller).len(), (max_schedules - 1) as usize);
    }

    vested_transfer {
        let caller: T::AccountId = alice::<T>();
        let receiver = T::Lookup::unlookup(bob::<T>());
        let max_schedules = T::MaxVestingSchedules::get() - 1;
        let mut schedules: BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules> =
                    BoundedVec::default();

        for i in 1..max_schedules {
            let asset_id_temp: AssetIdOf<T> = create_asset::<T>("TEST", i.into());
            let vesting_schedule = VestingScheduleOf::<T>::LinearVestingSchedule(LinearVestingSchedule {
                asset_id: asset_id_temp,
                start: T::BlockNumber::from(0_u32),
                period: T::BlockNumber::from(1_u32),
                period_count: 1,
                per_period: balance!(1),
            });
            schedules.try_push(vesting_schedule).expect("Error while push to BoundedVec");
        }
        <VestingSchedules<T>>::insert(caller.clone(), schedules);

        let asset_id: AssetIdOf<T> = create_asset::<T>("TEST", 0);
        let schedule = VestingScheduleOf::<T>::LinearVestingSchedule(LinearVestingSchedule {
                asset_id,
                start: T::BlockNumber::from(1_u32),
                period: T::BlockNumber::from(1_u32),
                period_count: 1,
                per_period: balance!(1),
            });

    }: _(RawOrigin::Signed(caller.clone()), receiver, schedule)
    verify {
        assert!(VestingSchedules::<T>::contains_key(bob::<T>()));
    }

    update_vesting_schedules {
        let caller: T::AccountId = alice::<T>();
        let mut schedules_update: BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules> =
                    BoundedVec::default();
        let mut schedules: BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules> =
                    BoundedVec::default();

        for i in 0..T::MaxVestingSchedules::get() {
            let asset_id: AssetIdOf<T> = create_asset::<T>("TEST", i.into());
            let vesting_schedule = VestingScheduleOf::<T>::LinearVestingSchedule(LinearVestingSchedule {
                asset_id,
                start: T::BlockNumber::from(1_u32),
                period: T::BlockNumber::from(1_u32),
                period_count: 1,
                per_period: balance!(1),
            });
            schedules.try_push(vesting_schedule).expect("Error while push to BoundedVec");
            let vesting_schedule_update = VestingScheduleOf::<T>::LinearVestingSchedule(LinearVestingSchedule {
                asset_id,
                start: T::BlockNumber::from(0_u32),
                period: T::BlockNumber::from(2_u32),
                period_count: 2,
                per_period: balance!(2),
            });
            schedules_update.try_push(vesting_schedule_update).expect("Error while push to BoundedVec");
        }
        <VestingSchedules<T>>::insert(caller.clone(), schedules);

    }: _(RawOrigin::Root, T::Lookup::unlookup(caller.clone()), schedules_update)
    verify {
        assert_last_event::<T>(Event::VestingSchedulesUpdated{who: caller}.into());
    }

    unlock_pending_schedule_by_manager {
        let caller: T::AccountId = alice::<T>();
        let asset_id: AssetIdOf<T> = create_asset::<T>("TEST", 0);
        let max_schedules = T::MaxVestingSchedules::get();
        let mut schedules: BoundedVec<VestingScheduleOf<T>, T::MaxVestingSchedules> =
                    BoundedVec::default();

        for i in 1..max_schedules {
            let asset_id_temp: AssetIdOf<T> = create_asset::<T>("TEST", i.into());
            let vesting_schedule = VestingScheduleOf::<T>::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id,
                manager_id: Some(caller.clone()),
                start: None,
                period: T::BlockNumber::from(2_u32),
                period_count: 1,
                per_period: balance!(1),
            });
            schedules.try_push(vesting_schedule).expect("Error while push to BoundedVec");
        }
         let vesting_schedule_locked = VestingScheduleOf::<T>::LinearPendingVestingSchedule(LinearPendingVestingSchedule {
                asset_id,
                manager_id: Some(caller.clone()),
                start: None,
                period: T::BlockNumber::from(1_u32),
                period_count: 1,
                per_period: balance!(1),
            });
        schedules.try_push(vesting_schedule_locked.clone()).expect("Error while push to BoundedVec");
        <VestingSchedules<T>>::insert(caller.clone(), schedules);
        frame_system::Pallet::<T>::set_block_number(T::BlockNumber::from(2_u32));
    }: _(RawOrigin::Signed(caller.clone()), T::Lookup::unlookup(caller.clone()), None, vesting_schedule_locked)
    verify {
        frame_system::Pallet::<T>::set_block_number(T::BlockNumber::from(3_u32));
        assert_ok!(VestedRewards::<T>::claim_unlocked(RawOrigin::Signed(caller.clone()).into(), asset_id));
        assert_eq!(VestingSchedules::<T>::get(&caller).len(), (max_schedules - 1) as usize);
    }

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::default().build(), mock::Runtime)
}

#[cfg(not(feature = "wip"))] // ORML multi asset vesting
benchmarks! {
    claim_rewards {
        let caller = alice::<T>();

        <T as common::Config>::MultiCurrency::deposit(PSWAP.into(), &T::GetBondingCurveRewardsAccountId::get(), balance!(100)).unwrap();
        <T as common::Config>::MultiCurrency::deposit(PSWAP.into(), &T::GetMarketMakerRewardsAccountId::get(), balance!(200)).unwrap();
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &caller, balance!(1)).unwrap(); // to prevent inc ref error

        VestedRewards::<T>::add_tbc_reward(&caller, balance!(100)).expect("Failed to add reward.");
        VestedRewards::<T>::distribute_limits(balance!(100));
    }: _(
        RawOrigin::Signed(caller.clone())
    )
    verify {
        assert_eq!(
            <T as common::Config>::MultiCurrency::free_balance(PSWAP.into(), &caller),
            balance!(100)
        );
    }

    update_rewards {
        let n in 0 .. 100;
        let rewards = prepare_rewards_update::<T>(n.into());
    }: {
        Pallet::<T>::update_rewards(RawOrigin::Root.into(), rewards).unwrap()
    }
    verify {
        assert_eq!(
            TotalRewards::<T>::get(),
            balance!(n) * 2
        );
    }

    register_crowdloan {
        let m in 1 .. 1000;
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &alice::<T>(), balance!(1)).unwrap(); // to prevent inc ref error
        let tag = CrowdloanTag(b"crowdloan".to_vec().try_into().unwrap());
        let rewards = prepare_crowdloan_rewards::<T>(10);
        let contributions = prepare_crowdloan_contributions::<T>(m as u128);
    }: _(RawOrigin::Root, tag.clone(), 0u32.into(), 100u32.into(), rewards, contributions)
    verify {
        assert!(crate::CrowdloanInfos::<T>::contains_key(&tag));
    }

    claim_crowdloan_rewards {
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &alice::<T>(), balance!(1)).unwrap(); // to prevent inc ref error
        let tag = CrowdloanTag(b"crowdloan".to_vec().try_into().unwrap());
        let rewards = prepare_crowdloan_rewards::<T>(5);
        let contributions = prepare_crowdloan_contributions::<T>(100);
        let account = contributions.get(0).cloned().unwrap().0;
        let first_asset_id = rewards.get(0).cloned().unwrap().0;
        Pallet::<T>::register_crowdloan(RawOrigin::Root.into(), tag.clone(), 0u32.into(), T::BLOCKS_PER_DAY * 4u32.into(), rewards.clone(), contributions).unwrap();
        let info = crate::CrowdloanInfos::<T>::get(&tag).unwrap();
        for (asset_id, _) in rewards {
            <T as common::Config>::MultiCurrency::deposit(asset_id.clone(), &info.account, balance!(1000)).unwrap();
        }
        frame_system::Pallet::<T>::set_block_number(T::BLOCKS_PER_DAY);
    }: _(RawOrigin::Signed(account.clone()), tag.clone())
    verify {
        assert_eq!(
            <T as common::Config>::MultiCurrency::free_balance(first_asset_id, &account),
            balance!(0.025) // 10 / 100 / 4
        );
    }

    claim_unlocked {}: {}
    verify {}

    vested_transfer {}: {}
    verify {}

    update_vesting_schedules {}: {}
    verify {}

    unlock_pending_schedule_by_manager {}: {}
    verify {}

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::default().build(), mock::Runtime)
}
