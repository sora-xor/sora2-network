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

//! Assets module benchmarking.

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_system::pallet_prelude::OriginFor;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use common::{AssetName, AssetSymbol, TradingPairSourceManager, DEFAULT_BALANCE_PRECISION, XOR};

use crate::utils;

use super::*;

// Support Functions
fn asset_owner<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn signed_origin<T: Config>(account_id: T::AccountId) -> OriginFor<T> {
    RawOrigin::Signed(account_id.clone()).into()
}

fn prepare_pools<T: Config>(count: u32) -> (Vec<T::AccountId>, Vec<T::AssetId>) {
    frame_system::Pallet::<T>::inc_providers(&asset_owner::<T>());
    let xor_asset: T::AssetId = XOR.into();
    let mut pools = Vec::new();
    let mut assets = Vec::new();
    for _i in 0..count {
        frame_system::Pallet::<T>::inc_account_nonce(&asset_owner::<T>());
        let other_asset = assets::Pallet::<T>::register_from(
            &asset_owner::<T>(),
            AssetSymbol(b"SYMBOL".to_vec()),
            AssetName(b"NAME".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        )
        .unwrap();

        assert_ok!(
            <T as pallet::Config>::TradingPairSourceManager::register_pair(
                Default::default(),
                xor_asset.clone(),
                other_asset.clone(),
            )
        );

        assert_ok!(pool_xyk::Pallet::<T>::initialize_pool(
            signed_origin::<T>(asset_owner::<T>()),
            Default::default(),
            xor_asset.clone(),
            other_asset.clone(),
        ));
        let pool = pool_xyk::Properties::<T>::get(xor_asset, other_asset.clone())
            .unwrap()
            .0;
        pools.push(pool);
        assets.push(other_asset);
    }
    (pools, assets)
}

fn prepare_good_accounts<T: Config>(count: u32, assets: &[T::AssetId]) {
    let xor_asset: T::AssetId = XOR.into();
    let xor_owner = assets::Pallet::<T>::asset_owner(&xor_asset).unwrap();
    for other_asset in assets {
        for j in 0..count {
            let account_id = utils::account::<T>(j);
            assert_ok!(assets::Pallet::<T>::mint_to(
                &XOR.into(),
                &xor_owner,
                &account_id,
                balance!(50000),
            ));

            assert_ok!(assets::Pallet::<T>::mint_to(
                &other_asset,
                &asset_owner::<T>(),
                &account_id,
                balance!(50000),
            ));

            assert_ok!(pool_xyk::Pallet::<T>::deposit_liquidity(
                signed_origin::<T>(account_id),
                Default::default(),
                XOR.into(),
                other_asset.clone(),
                balance!(1.1),
                balance!(2.2),
                balance!(1.1),
                balance!(2.2),
            ));
        }
    }
}

benchmarks! {
    refresh_pool {
        let a in 1..20;
        let (mut pools, assets) = prepare_pools::<T>(1);
        prepare_good_accounts::<T>(a, &assets);
    }: {
        Pallet::<T>::refresh_pool(pools.remove(0), T::REFRESH_FREQUENCY);
    }

    prepare_accounts_for_vesting {
        let a in 1..29;
        let b in 1..43;
        let (pools, assets) = prepare_pools::<T>(a);
        prepare_good_accounts::<T>(b, &assets);
        Pallet::<T>::refresh_pools(T::VESTING_FREQUENCY);
        let mut accounts = BTreeMap::new();
    }: {
        Pallet::<T>::prepare_accounts_for_vesting(T::VESTING_FREQUENCY, &mut accounts);
    }

    vest_account_rewards {
        let a in 1..20;
        let (mut pools, assets) = prepare_pools::<T>(1);
        prepare_good_accounts::<T>(a, &assets);
        Pallet::<T>::refresh_pools(T::VESTING_FREQUENCY);
        let pool = pools.remove(0);
        let farmers = PoolFarmers::<T>::get(&pool);
        let mut accounts = BTreeMap::new();
        Pallet::<T>::prepare_accounts_for_vesting(T::VESTING_FREQUENCY, &mut accounts);
    }: {
        Pallet::<T>::vest_account_rewards(accounts);
    }
}

#[cfg(test)]
mod tests {
    use frame_support::assert_ok;

    use crate::mock::{ExtBuilder, Runtime};

    use super::*;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Pallet::<Runtime>::test_benchmark_refresh_pool());
            assert_ok!(Pallet::<Runtime>::test_benchmark_prepare_accounts_for_vesting());
            assert_ok!(Pallet::<Runtime>::test_benchmark_vest_account_rewards());
        });
    }
}
