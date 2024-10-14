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

use codec::Encode;
use common::{
    AssetIdOf, AssetInfoProvider, AssetManager, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION,
    XOR,
};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::prelude::*;

use crate::Pallet as PriceTools;

const UPDATE_SHIFT: usize = 1000;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn create_asset<T: Config>(prefix: Vec<u8>, index: usize) -> AssetIdOf<T> {
    let entropy: [u8; 32] = (prefix, index as u128).using_encoded(blake2_256);
    AssetIdOf::<T>::from(H256(entropy))
}

fn register_asset<T: Config>(owner: T::AccountId, asset: AssetIdOf<T>) {
    PriceTools::<T>::register_asset(&asset).unwrap();

    T::AssetManager::register_asset_id(
        owner,
        asset,
        AssetSymbol(b"ASSET".to_vec()),
        AssetName(b"Asset".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        balance!(1000000),
        true,
        common::AssetType::Regular,
        None,
        None,
    )
    .unwrap();
}

fn create_pair_with_xor<T: Config>(
    owner: T::AccountId,
    origin: T::RuntimeOrigin,
    asset: AssetIdOf<T>,
) {
    T::AssetManager::mint(origin.clone(), asset, owner, balance!(1000000)).unwrap();

    <T as Config>::TradingPairSourceManager::register_pair(
        DEXId::Polkaswap.into(),
        XOR.into(),
        asset,
    )
    .unwrap();

    pool_xyk::Pallet::<T>::initialize_pool(
        origin.clone(),
        DEXId::Polkaswap.into(),
        XOR.into(),
        asset,
    )
    .unwrap();

    pool_xyk::Pallet::<T>::deposit_liquidity(
        origin.clone(),
        DEXId::Polkaswap.into(),
        XOR.into(),
        asset,
        balance!(1000),
        balance!(2000),
        balance!(1000),
        balance!(2000),
    )
    .unwrap();
}

fn prepare_secondary_market<T: Config>(elems_active: usize, elems_updated: usize) {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);

    let xor_id = AssetIdOf::<T>::from(XOR);
    let xor_owner = <T as pool_xyk::Config>::AssetInfoProvider::get_asset_owner(&xor_id).unwrap();

    T::AssetManager::mint(
        RawOrigin::Signed(xor_owner.clone()).into(),
        XOR.into(),
        owner.clone(),
        balance!(1000000),
    )
    .unwrap();

    // Create assets don't need to be updated
    for i in 0..elems_active {
        let asset = create_asset::<T>(b"asset".to_vec(), i.into());
        register_asset::<T>(owner.clone(), asset);

        for m in 1..crate::AVG_BLOCK_SPAN {
            crate::PriceInfos::<T>::mutate(asset, |val| {
                let val = val.as_mut().unwrap();
                let price = balance!(m + i);
                val.buy.spot_prices.push_back(price);
                val.sell.spot_prices.push_back(price);

                val.buy.needs_update = false;
                val.sell.needs_update = false;

                val.buy.last_spot_price = price;
                val.sell.last_spot_price = price;
            });
            crate::FastPriceInfos::<T>::mutate(asset, |val| {
                let val = val.as_mut().unwrap();
                let price = balance!(m + i);
                val.buy.spot_prices.push_back(price);
                val.sell.spot_prices.push_back(price);

                val.buy.needs_update = false;
                val.sell.needs_update = false;

                val.buy.last_spot_price = price;
                val.sell.last_spot_price = price;
            });
        }
    }

    // Create assets need to be updated
    for i in UPDATE_SHIFT..(UPDATE_SHIFT + elems_updated) {
        let asset = create_asset::<T>(b"asset".to_vec(), i.into());
        register_asset::<T>(owner.clone(), asset);
        create_pair_with_xor::<T>(
            owner.clone(),
            RawOrigin::Signed(owner.clone()).into(),
            asset,
        );

        for m in 1..crate::AVG_BLOCK_SPAN {
            crate::PriceInfos::<T>::mutate(asset, |val| {
                let val = val.as_mut().unwrap();
                let price = balance!(m + i);
                val.buy.spot_prices.push_back(price);
                val.sell.spot_prices.push_back(price);

                val.buy.needs_update = true;
                val.sell.needs_update = true;
            });
            crate::FastPriceInfos::<T>::mutate(asset, |val| {
                let val = val.as_mut().unwrap();
                let price = balance!(m + i);
                val.buy.spot_prices.push_back(price);
                val.sell.spot_prices.push_back(price);

                val.buy.needs_update = true;
                val.sell.needs_update = true;
            });
        }
    }
}

benchmarks! {
    on_initialize {
        let a in 0..10;
        let b in 0..10;
        prepare_secondary_market::<T>(a as usize, b as usize);
        let mut infos_before = BTreeMap::new();

        let mut range = (0..a as usize).collect::<Vec<_>>();
        let mut to_update = (UPDATE_SHIFT..UPDATE_SHIFT + b as usize).collect::<Vec<_>>();
        range.append(&mut to_update);

        for i in range.clone() {
            let asset = create_asset::<T>(b"asset".to_vec(), i.into());
            assert!(crate::PriceInfos::<T>::get(&asset).is_some());
            infos_before.insert(
                i, (
                crate::PriceInfos::<T>::get(&asset)
                    .unwrap()
                    .buy
                    .average_price,
                crate::PriceInfos::<T>::get(&asset)
                    .unwrap()
                    .sell
                    .average_price,
                crate::FastPriceInfos::<T>::get(&asset)
                    .unwrap()
                    .buy
                    .average_price,
                crate::FastPriceInfos::<T>::get(&asset)
                    .unwrap()
                    .sell
                    .average_price,
            ));
        }
    }: {
        PriceTools::<T>::average_prices_calculation_routine();
    }
    verify {
        for i in range {
            let asset = create_asset::<T>(b"asset".to_vec(), i.into());
            assert_ne!(
                infos_before.get(&i.into()).unwrap(),
                &(
                    crate::PriceInfos::<T>::get(&asset)
                        .unwrap()
                        .buy
                        .average_price,
                    crate::PriceInfos::<T>::get(&asset)
                        .unwrap()
                        .sell
                        .average_price,
                    crate::FastPriceInfos::<T>::get(&asset)
                        .unwrap()
                        .buy
                        .average_price,
                    crate::FastPriceInfos::<T>::get(&asset)
                        .unwrap()
                        .sell
                        .average_price,
                )
            );
        }
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
            assert_ok!(Pallet::<Runtime>::test_benchmark_on_initialize());
        });
    }
}
