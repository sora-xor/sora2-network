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

//! Market module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;

use common::{AssetManager, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION};

fn asset_owner<T: Config>() -> T::AccountId {
    frame_benchmarking::account("owner", 0, 0)
}

fn buyer<T: Config>(i: u32) -> T::AccountId {
    frame_benchmarking::account("buyer", i, 0)
}

fn add_product<T: Config>(price_asset: AssetIdOf<T>) -> AssetIdOf<T> {
    let owner = asset_owner::<T>();

    Pallet::<T>::create_new_product(
        owner,
        common::AssetName(b"PRODUCT".to_vec()),
        common::AssetSymbol(b"PRODUCT".to_vec()),
        common::Description(b"PRODUCT".to_vec()),
        common::ContentSource(b"PRODUCT".to_vec()),
        Product {
            price_asset,
            price: 100,
            extensions: vec![],
        },
    )
    .expect("Failed to register product")
}

fn add_asset<T: Config>() -> AssetIdOf<T> {
    let owner = asset_owner::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);

    T::AssetManager::register_from(
        &owner,
        AssetSymbol(b"TOKEN".to_vec()),
        AssetName(b"TOKEN".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        10000,
        true,
        AssetType::Regular,
        None,
        None,
    )
    .expect("Failed to register asset")
}

benchmarks! {
    create_product {
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner).into();
        let price_asset = add_asset::<T>();
        let required_product = add_product::<T>(price_asset);
        let disallowed_product = add_product::<T>(price_asset);
    }: {
        Pallet::<T>::create_product(owner_origin,
            common::AssetName(b"PRODUCT".to_vec()),
            common::AssetSymbol(b"PRODUCT".to_vec()),
            common::Description(b"PRODUCT".to_vec()),
            common::ContentSource(b"PRODUCT".to_vec()),
            Product {
                price_asset,
                price: 100,
                extensions: vec![
                    Extension::Expirable(10u32.into()),
                    Extension::MaxAmount(10),
                    Extension::RequiredProduct(required_product, 1),
                    Extension::DisallowedProduct(disallowed_product)
                ],
            }
        ).unwrap();
    }

    buy {
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner.clone()).into();
        let price_asset = add_asset::<T>();
        let required_product = add_product::<T>(price_asset);
        let disallowed_product = add_product::<T>(price_asset);
        let product = Pallet::<T>::create_new_product(owner.clone(),
            common::AssetName(b"PRODUCT".to_vec()),
            common::AssetSymbol(b"PRODUCT".to_vec()),
            common::Description(b"PRODUCT".to_vec()),
            common::ContentSource(b"PRODUCT".to_vec()),
            Product {
                price_asset,
                price: 100,
                extensions: vec![
                    Extension::Expirable(10u32.into()),
                    Extension::MaxAmount(10),
                    Extension::RequiredProduct(required_product, 1),
                    Extension::DisallowedProduct(disallowed_product)
                ],
            }
        ).unwrap();
        Pallet::<T>::buy(owner_origin.clone(), required_product, 1).unwrap();
    }: {
        Pallet::<T>::buy(owner_origin, product, 1).unwrap();
    }
    verify {
        assert_eq!(
            T::AssetInfoProvider::total_balance(&product, &owner).unwrap(),
            1
        );
    }

    on_initialize {
        let a in 1..50;
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner.clone()).into();
        let price_asset = add_asset::<T>();
        let product = Pallet::<T>::create_new_product(owner.clone(),
            common::AssetName(b"PRODUCT".to_vec()),
            common::AssetSymbol(b"PRODUCT".to_vec()),
            common::Description(b"PRODUCT".to_vec()),
            common::ContentSource(b"PRODUCT".to_vec()),
            Product {
                price_asset,
                price: 100,
                extensions: vec![
                    Extension::Expirable(10u32.into()),
                ],
            }
        ).unwrap();
        for i in 0..a {
            let buyer = buyer::<T>(i);
            let buyer_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(buyer.clone()).into();
            T::AssetManager::mint_unchecked(&price_asset, &buyer, 1000).unwrap();
            Pallet::<T>::buy(buyer_origin, product, 1).unwrap();
            assert_eq!(
                T::AssetInfoProvider::total_balance(&product, &buyer).unwrap(),
                1
            );
        }
        frame_system::Pallet::<T>::set_block_number(frame_system::Pallet::<T>::block_number() + 10u32.into());
    }: {
        Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
    }
    verify {
        assert_eq!(Expirations::<T>::iter().collect::<Vec<_>>(), vec![]);
        for i in 0..a {
            let buyer = buyer::<T>(i);
            assert_eq!(
                T::AssetInfoProvider::total_balance(&product, &buyer).unwrap(),
                0
            );
        }
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Runtime
    );
}
