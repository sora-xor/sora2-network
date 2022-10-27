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
use frame_support::traits::OriginTrait;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;
use traits::MultiCurrency;

use common::{assert_approx_eq, FromGenericPair, ETH, PSWAP, XOR};

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

fn prepare_pending_accounts<T: Config>(n: u128) {
    for i in 0..n {
        let user_account = create_account::<T>(b"user".to_vec(), i);
        T::Currency::deposit(XOR.into(), &user_account, balance!(1)).unwrap(); // to prevent inc ref error
        VestedRewards::<T>::add_tbc_reward(&user_account, balance!(1))
            .expect("Failed to add reward.");
    }
}

fn prepare_pending_market_makers<T: Config>(n: u128, m: u128) {
    MarketMakingPairs::<T>::insert(&T::AssetId::from(XOR), &T::AssetId::from(ETH), ());
    MarketMakingPairs::<T>::insert(&T::AssetId::from(PSWAP), &T::AssetId::from(XOR), ());
    for i in 0..n {
        let user_account = create_account::<T>(b"eligible mm reward".to_vec(), i);
        T::Currency::deposit(XOR.into(), &user_account, balance!(1)).unwrap(); // to prevent inc ref error
        VestedRewards::<T>::update_market_maker_records(
            &user_account,
            &XOR.into(),
            balance!(100),
            500,
            &PSWAP.into(),
            &ETH.into(),
            Some(&XOR.into()),
        )
        .unwrap();
    }
    for i in 0..m {
        let user_account = create_account::<T>(b"non eligible mm reward".to_vec(), i);
        T::Currency::deposit(XOR.into(), &user_account, balance!(1)).unwrap(); // to prevent inc ref error
        VestedRewards::<T>::update_market_maker_records(
            &user_account,
            &XOR.into(),
            balance!(100),
            100,
            &PSWAP.into(),
            &ETH.into(),
            Some(&XOR.into()),
        )
        .unwrap();
    }
}

benchmarks! {
    claim_rewards {
        let caller = alice::<T>();

        T::Currency::deposit(PSWAP.into(), &T::GetBondingCurveRewardsAccountId::get(), balance!(100)).unwrap();
        T::Currency::deposit(PSWAP.into(), &T::GetMarketMakerRewardsAccountId::get(), balance!(200)).unwrap();
        T::Currency::deposit(XOR.into(), &caller, balance!(1)).unwrap(); // to prevent inc ref error

        VestedRewards::<T>::add_tbc_reward(&caller, balance!(100)).expect("Failed to add reward.");
        VestedRewards::<T>::add_market_maker_reward(&caller, balance!(200)).expect("Failed to add reward.");
        VestedRewards::<T>::distribute_limits(balance!(300));
    }: _(
        RawOrigin::Signed(caller.clone())
    )
    verify {
        assert_eq!(
            T::Currency::free_balance(PSWAP.into(), &caller),
            balance!(300)
        );
    }

    distribute_limits {
        let n in 0 .. 10000 => prepare_pending_accounts::<T>(n.into());
    }: {
        Pallet::<T>::distribute_limits(balance!(n))
    }
    verify {
        assert_eq!(
            TotalRewards::<T>::get(),
            balance!(n)
        );
    }

    distribute_market_maker_rewards {
        let n in 0 .. 10000; // users eligible for mm rewards distribution
        let m in 0 .. 10000; // users non-eligible for mm rewards distribution
        prepare_pending_market_makers::<T>(n.into(), m.into());
    }: {
        let p = Pallet::<T>::market_maker_rewards_distribution_routine();
        assert_eq!(p, n);
    }
    verify {
        if n == 0 {
            assert_eq!(
                TotalRewards::<T>::get(),
                balance!(0)
            );
        } else {
            assert_approx_eq!(
                TotalRewards::<T>::get(),
                crate::SINGLE_MARKET_MAKER_DISTRIBUTION_AMOUNT,
                1000000
            );
        }
    }

    set_asset_pair {
        let origin = T::Origin::root();
    }: {
        Pallet::<T>::set_asset_pair(origin, XOR.into(), ETH.into(), true).unwrap();
    }
    verify {
        assert!(
            MarketMakingPairs::<T>::contains_key(&T::AssetId::from(XOR), &T::AssetId::from(ETH)));
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
            assert_ok!(Pallet::<Runtime>::test_benchmark_distribute_market_maker_rewards());
            assert_ok!(Pallet::<Runtime>::test_benchmark_set_asset_pair());
        });
    }
}
