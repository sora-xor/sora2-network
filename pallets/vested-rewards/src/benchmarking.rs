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

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;
use traits::MultiCurrency;

use common::{fixed, fixed_wrapper, FromGenericPair, PSWAP, XOR};

use crate::Pallet as VestedRewards;
use technical::Pallet as Technical;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn create_account<T: Config>(prefix: Vec<u8>, index: u128) -> T::AccountId {
    let tech_account: T::TechAccountId =
        T::TechAccountId::from_generic_pair(prefix, index.encode());
    Technical::<T>::tech_account_id_to_account_id(&tech_account).unwrap()
}

fn prepare_crowdloan_rewards<T: Config>(n: u128) {
    for i in 0..n {
        let user_account = create_account::<T>(b"user".to_vec(), i);
        let reward = CrowdloanReward {
            id: user_account.encode(),
            address: user_account.encode(),
            contribution: Default::default(),
            xor_reward: Default::default(),
            val_reward: Default::default(),
            pswap_reward: fixed!(1),
            xstusd_reward: Default::default(),
            percent: Default::default(),
        };
        CrowdloanRewards::<T>::insert(&user_account, reward);
        T::Currency::deposit(XOR.into(), &user_account, balance!(1))
            .expect("Failed to deposit XOR"); // to prevent inc ref error
        T::Currency::deposit(
            PSWAP.into(),
            &T::GetCrowdloanRewardsAccountId::get(),
            balance!(1),
        )
        .expect("Failed to deposit PSWAP to CrowdloanRewards tech acc");
    }
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

benchmarks! {
    claim_rewards {
        let caller = alice::<T>();

        T::Currency::deposit(PSWAP.into(), &T::GetBondingCurveRewardsAccountId::get(), balance!(100)).unwrap();
        T::Currency::deposit(PSWAP.into(), &T::GetMarketMakerRewardsAccountId::get(), balance!(200)).unwrap();
        T::Currency::deposit(XOR.into(), &caller, balance!(1)).unwrap(); // to prevent inc ref error

        VestedRewards::<T>::add_tbc_reward(&caller, balance!(100)).expect("Failed to add reward.");
        VestedRewards::<T>::distribute_limits(balance!(100));
    }: _(
        RawOrigin::Signed(caller.clone())
    )
    verify {
        assert_eq!(
            T::Currency::free_balance(PSWAP.into(), &caller),
            balance!(100)
        );
    }

    distribute_limits {
        let n in 0 .. 100 => prepare_pending_accounts::<T>(n.into());
    }: {
        Pallet::<T>::distribute_limits(balance!(n))
    }
    verify {
        let amount = fixed_wrapper!(1) / Fixed::try_from(LEASE_TOTAL_DAYS).expect("Failed to convert to fixed");
        assert_eq!(
            T::Currency::free_balance(T::AssetId::from(PSWAP), &caller),
            amount.try_into_balance().unwrap()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Pallet::<Runtime>::test_benchmark_claim_rewards());
            assert_ok!(Pallet::<Runtime>::test_benchmark_distribute_limits());
            assert_ok!(Pallet::<Runtime>::test_benchmark_update_rewards());
        });
    }
}
