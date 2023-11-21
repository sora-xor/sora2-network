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

//! Liquidity Proxy benchmarking module.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]

use common::{
    balance, DEXId, DexInfoProvider, FilterMode, LiquidityRegistry, LiquiditySourceFilter,
    LiquiditySourceType, VAL, XOR, XSTUSD,
};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use liquidity_proxy::ExchangePath;
use sp_std::prelude::*;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

#[cfg(any(feature = "runtime-benchmarks", test, feature = "std"))]
fn assert_last_event<T: liquidity_proxy::Config>(
    generic_event: <T as liquidity_proxy::Config>::RuntimeEvent,
) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

pub struct Pallet<T: Config>(liquidity_proxy::Pallet<T>);
pub trait Config:
    liquidity_proxy::Config
    + pool_xyk::Config
    + multicollateral_bonding_curve_pool::Config
    + price_tools::Config
{
}

benchmarks! {
    enable_liquidity_source {
        liquidity_proxy::Pallet::<T>::disable_liquidity_source(
            RawOrigin::Root.into(),
            LiquiditySourceType::XSTPool
        )?;
    }: {
        liquidity_proxy::Pallet::<T>::enable_liquidity_source(
            RawOrigin::Root.into(),
            LiquiditySourceType::XSTPool
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(
            liquidity_proxy::Event::<T>::LiquiditySourceEnabled(
                LiquiditySourceType::XSTPool
            ).into()
        );
    }

    disable_liquidity_source {
    }: {
        liquidity_proxy::Pallet::<T>::disable_liquidity_source(
            RawOrigin::Root.into(),
            LiquiditySourceType::XSTPool
        ).unwrap();
    }
    verify {
        assert_last_event::<T>(
            liquidity_proxy::Event::<T>::LiquiditySourceDisabled(
                LiquiditySourceType::XSTPool
            ).into()
        );
    }

    check_indivisible_assets {
        let from_asset: T::AssetId = XOR.into();
        let to_asset: T::AssetId = VAL.into();
    }: {
        liquidity_proxy::Pallet::<T>::check_indivisible_assets(
            &from_asset,
            &to_asset
        ).unwrap();
    }
    verify {
    }

    new_trivial {
        let dex_info = <T as trading_pair::Config>::DexInfoProvider::get_dex_info(&DEX.into())?;
        let from_asset: T::AssetId = XSTUSD.into();
        let to_asset: T::AssetId = VAL.into();
    }: {
        ExchangePath::<T>::new_trivial(
            &dex_info,
            from_asset,
            to_asset
        ).unwrap();
    }
    verify {
    }

    is_forbidden_filter {
        let from_asset: T::AssetId = XOR.into();
        let to_asset: T::AssetId = VAL.into();
        let sources = vec![LiquiditySourceType::XYKPool, LiquiditySourceType::MulticollateralBondingCurvePool, LiquiditySourceType::XSTPool];
        let filter = FilterMode::Disabled;
    }: {
        liquidity_proxy::Pallet::<T>::is_forbidden_filter(&from_asset, &to_asset, &sources, &filter);
    }
    verify {
    }

    list_liquidity_sources {
        let from_asset: T::AssetId = XOR.into();
        let to_asset: T::AssetId = VAL.into();
        let filter = LiquiditySourceFilter::<T::DEXId, LiquiditySourceType>::with_allowed(
            DEX.into(),
            [
                LiquiditySourceType::XYKPool,
                LiquiditySourceType::MulticollateralBondingCurvePool,
                LiquiditySourceType::XSTPool,
            ]
            .to_vec()
        );
    }: {
        T::LiquidityRegistry::list_liquidity_sources(&from_asset, &to_asset, &filter).unwrap();
    }
    verify {
    }

    set_adar_commission_ratio {
    }: {
        liquidity_proxy::Pallet::<T>::set_adar_commission_ratio(
            RawOrigin::Root.into(),
            balance!(0.5)
        ).unwrap();
    }
    verify {
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
            assert_ok!(Pallet::<Runtime>::test_benchmark_enable_liquidity_source());
            assert_ok!(Pallet::<Runtime>::test_benchmark_disable_liquidity_source());
            assert_ok!(Pallet::<Runtime>::test_benchmark_check_indivisible_assets());
            assert_ok!(Pallet::<Runtime>::test_benchmark_new_trivial());
            assert_ok!(Pallet::<Runtime>::test_benchmark_is_forbidden_filter());
            assert_ok!(Pallet::<Runtime>::test_benchmark_list_liquidity_sources());
        });
    }
}
