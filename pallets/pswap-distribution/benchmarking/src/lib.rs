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

//! PSWAP distribution module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

#[cfg(test)]
mod mock;

use codec::{Decode, Encode};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_support::traits::{Get, OnInitialize};
use frame_system::RawOrigin;
use hex_literal::hex;
use pool_xyk::PoolProviders;
use pswap_distribution::DistributionWeightParams;
use pswap_distribution::{Call, ClaimableShares, ShareholderAccounts};
use sp_std::prelude::*;

use common::fixnum::ops::One;
use common::{
    balance, fixed, AmountOf, AssetIdOf, AssetInfoProvider, AssetManager, CurrencyIdOf, Fixed,
    FromGenericPair, PSWAP,
};
use traits::currency::MultiCurrencyExtended;

use permissions::Pallet as Permissions;
use pswap_distribution::Pallet as PSwap;
use sp_std::convert::TryFrom;
use technical::Pallet as Technical;

pub struct Pallet<T: Config>(pswap_distribution::Pallet<T>);

pub trait Config: pswap_distribution::Config + pool_xyk::Config {}

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

fn setup_benchmark_pool_xyk<T: Config + pool_xyk::Config>() {
    #[cfg(not(test))]
    {
        use common::{TBCD, XOR, XST};
        let authority = alice::<T>();
        pool_xyk::Pallet::<T>::initialize_pool(
            RawOrigin::Signed(authority.clone()).into(),
            common::DexId::Polkaswap.into(),
            XOR.into(),
            PSWAP.into(),
        )
        .unwrap();
        pool_xyk::Pallet::<T>::initialize_pool(
            RawOrigin::Signed(authority.clone()).into(),
            common::DexId::Polkaswap.into(),
            XOR.into(),
            XST.into(),
        )
        .unwrap();
        pool_xyk::Pallet::<T>::initialize_pool(
            RawOrigin::Signed(authority.clone()).into(),
            common::DexId::Polkaswap.into(),
            XOR.into(),
            TBCD.into(),
        )
        .unwrap();
        T::AssetManager::mint_to(&XOR.into(), &authority, &authority, balance!(20000)).unwrap();
        T::AssetManager::mint_to(&XST.into(), &authority, &authority, balance!(100000)).unwrap();
        T::AssetManager::mint_to(&PSWAP.into(), &authority, &authority, balance!(1000000)).unwrap();
        T::AssetManager::mint_to(&TBCD.into(), &authority, &authority, balance!(1000000)).unwrap();
        pool_xyk::Pallet::<T>::deposit_liquidity_unchecked(
            authority.clone(),
            common::DexId::Polkaswap.into(),
            XOR.into(),
            XST.into(),
            balance!(10000),
            balance!(100000),
            balance!(10000),
            balance!(100000),
        )
        .unwrap();
        pool_xyk::Pallet::<T>::deposit_liquidity_unchecked(
            authority.clone(),
            common::DexId::Polkaswap.into(),
            XOR.into(),
            PSWAP.into(),
            balance!(10000),
            balance!(1000000),
            balance!(10000),
            balance!(1000000),
        )
        .unwrap();
        pool_xyk::Pallet::<T>::deposit_liquidity_unchecked(
            authority.clone(),
            common::DexId::Polkaswap.into(),
            XOR.into(),
            TBCD.into(),
            balance!(10000),
            balance!(1000000),
            balance!(10000),
            balance!(1000000),
        )
        .unwrap();
    }
}

fn add_subscribtion<T: Config + pool_xyk::Config>(pool_index: u128, shareholders: u128) {
    let authority = alice::<T>();
    let pool_fee_account = create_account::<T>(b"pool_fee".to_vec(), pool_index);
    frame_system::Pallet::<T>::inc_providers(&pool_fee_account);
    let pool_account = create_account::<T>(b"pool".to_vec(), pool_index);
    frame_system::Pallet::<T>::inc_providers(&pool_account);
    T::AssetManager::mint_to(&PSWAP.into(), &authority, &pool_fee_account, balance!(1000)).unwrap();
    PSwap::<T>::subscribe(
        pool_fee_account,
        common::DexId::Polkaswap.into(),
        pool_account.clone(),
        None,
    )
    .unwrap();
    for j in 0u128..shareholders {
        let liquidity_provider = create_account::<T>(b"liquidity_provider".to_vec(), j);
        frame_system::Pallet::<T>::inc_providers(&liquidity_provider);
        pool_xyk::Pallet::<T>::mint(&pool_account, &liquidity_provider, balance!(100)).unwrap();
    }
}

fn prepare_for_distribution<T: Config + pool_xyk::Config>(weight_params: DistributionWeightParams) {
    let authority = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&authority);
    Permissions::<T>::assign_permission(
        authority.clone(),
        &authority,
        permissions::MINT,
        permissions::Scope::Unlimited,
    )
    .unwrap();
    setup_benchmark_pool_xyk::<T>();
    let mut pool_index = 0;
    frame_system::Pallet::<T>::set_block_number(0u32.into());
    for i in 0..weight_params.distributed {
        let shareholders = if i + 1 == weight_params.distributed {
            let remaining = weight_params.shareholders
                - (weight_params.shareholders / weight_params.distributed)
                    * (weight_params.distributed - 1);
            remaining
        } else {
            weight_params.shareholders / weight_params.distributed
        };
        add_subscribtion::<T>(pool_index, shareholders as u128);
        pool_index += 1;
    }
    frame_system::Pallet::<T>::set_block_number(1u32.into());
    for _ in 0..weight_params.skipped {
        add_subscribtion::<T>(pool_index, 10);
        pool_index += 1;
    }
}

fn validate_distribution<T: Config>(weight_params: DistributionWeightParams) {
    for i in 0..weight_params.distributed {
        let pool_account = create_account::<T>(b"pool".to_vec(), i as u128);
        let shareholders = if i + 1 == weight_params.distributed {
            let remaining = weight_params.shareholders
                - (weight_params.shareholders / weight_params.distributed)
                    * (weight_params.distributed - 1);
            remaining
        } else {
            weight_params.shareholders / weight_params.distributed
        };

        for j in 0..shareholders {
            let liquidity_provider = create_account::<T>(b"liquidity_provider".to_vec(), j as u128);
            frame_system::Pallet::<T>::inc_providers(&liquidity_provider);
            let _ =
                PSwap::<T>::claim_incentive(RawOrigin::Signed(liquidity_provider.clone()).into());
            assert_eq!(
                PoolProviders::<T>::get(&pool_account, &liquidity_provider).unwrap(),
                balance!(100)
            );
            assert!(
                <T as technical::Config>::AssetInfoProvider::free_balance(
                    &PSWAP.into(),
                    &liquidity_provider
                )
                .unwrap()
                    > balance!(0)
            );
        }
    }
}

benchmarks! {
    claim_incentive {
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        ShareholderAccounts::<T>::insert(caller.clone(), Fixed::ONE);
        ClaimableShares::<T>::put(Fixed::ONE);
        let pswap_rewards_account = T::GetTechnicalAccountId::get();
        let pswap_asset_id: AssetIdOf<T> = PSWAP.into();
        let pswap_currency: CurrencyIdOf<T> = pswap_asset_id.into();
        let pswap_amount = AmountOf::<T>::try_from(balance!(500)).map_err(|_|()).unwrap();
        T::MultiCurrency::update_balance(pswap_currency.into(), &pswap_rewards_account, pswap_amount).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone())
    )
    verify {
        assert_eq!(ClaimableShares::<T>::get(), fixed!(0));
    }

    on_initialize {
        let a in 1..50;
        let b in 1..10;
        let c in 10..100;
        let weight_params = DistributionWeightParams {
            skipped: a,
            distributed: b,
            shareholders: c
        };
        prepare_for_distribution::<T>(weight_params.clone());
        let distribution_freq = T::GetDefaultSubscriptionFrequency::get();
    }: {
        PSwap::<T>::on_initialize(distribution_freq);
    }
    verify {
        validate_distribution::<T>(weight_params);
    }
}

impl_benchmark_test_suite!(
    Pallet,
    crate::mock::ExtBuilder::default().build(),
    crate::mock::Runtime
);
