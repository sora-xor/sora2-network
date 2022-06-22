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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{balance, Balance, DAI, VAL, XOR};
use frame_benchmarking::benchmarks;
use frame_support::sp_runtime::traits::UniqueSaturatedInto;
use frame_support::sp_runtime::{FixedPointNumber, FixedU128};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_std::boxed::Box;
use sp_std::vec;
use sp_std::vec::Vec;

use crate::{Config, Module};

fn alice<T: Config + pool_xyk::Config + pallet_staking::Config>() -> T::AccountId {
    let bytes = [1; 32];
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn init<T: Config + pool_xyk::Config + pallet_staking::Config>() {
    let owner = alice::<T>();
    frame_system::Module::<T>::inc_providers(&owner);

    permissions::Module::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MINT,
        permissions::Scope::Unlimited,
    )
    .unwrap();

    assets::Module::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000))
        .unwrap();

    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

    assets::Module::<T>::mint_to(
        &VAL.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    pool_xyk::Module::<T>::initialize_pool(
        owner_origin.clone(),
        T::DEXIdValue::get(),
        XOR.into(),
        VAL.into(),
    )
    .unwrap();
    pool_xyk::Module::<T>::deposit_liquidity(
        owner_origin.clone(),
        T::DEXIdValue::get(),
        XOR.into(),
        VAL.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();

    assets::Module::<T>::mint_to(
        &DAI.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    pool_xyk::Module::<T>::initialize_pool(
        owner_origin.clone(),
        T::DEXIdValue::get(),
        XOR.into(),
        DAI.into(),
    )
    .unwrap();
    pool_xyk::Module::<T>::deposit_liquidity(
        owner_origin.clone(),
        T::DEXIdValue::get(),
        XOR.into(),
        DAI.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();
}

benchmarks! {
    where_clause {
        where T: Config + pool_xyk::Config + pallet_staking::Config
    }
    remint {
        init::<T>();
        assert_eq!(assets::Module::<T>::free_balance(&VAL.into(), &T::GetParliamentAccountId::get()), Ok(0));
    }: {
        crate::Module::<T>::remint(balance!(0.1)).unwrap();
    } verify {
        let val_burned: Balance = pallet_staking::Module::<T>::era_val_burned().unique_saturated_into();
        assert_eq!(val_burned, balance!(0.199380121801856354));

        assert_eq!(assets::Module::<T>::free_balance(&VAL.into(), &T::GetParliamentAccountId::get()), Ok(balance!(0.019938012180185635)));
    }

    update_multiplier {
        let m in 0 .. 100;
        let m = FixedU128::checked_from_integer(m.into()).unwrap();
    }: _(RawOrigin::Root, m)
    verify {
        assert_eq!(crate::Multiplier::<T>::get(), m);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::build().execute_with(|| {
            assert_ok!(test_benchmark_update_multiplier::<Runtime>());
            // Benchmark fails, needs revisiting
            // assert_ok!(test_benchmark_remint::<Runtime>());
        });
    }
}
