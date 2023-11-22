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

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};

use crate::Pallet as DexApi;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn find_non_existing_source(sources: Vec<LiquiditySourceType>) -> Option<LiquiditySourceType> {
    let mut all_sources = Vec::from([
        LiquiditySourceType::XYKPool,
        LiquiditySourceType::MulticollateralBondingCurvePool,
        LiquiditySourceType::XSTPool,
        LiquiditySourceType::MockPool,
        LiquiditySourceType::MockPool2,
        LiquiditySourceType::MockPool3,
        LiquiditySourceType::MockPool4,
    ]);

    all_sources.retain(|x| !sources.contains(x));
    all_sources.first().copied()
}

benchmarks! {
    enable_liquidity_source {
        let source = find_non_existing_source(EnabledSourceTypes::<T>::get()).unwrap();
    }: {
        DexApi::<T>::enable_liquidity_source(RawOrigin::Root.into(), source).unwrap();
    }
    verify {
        assert!(EnabledSourceTypes::<T>::get().contains(&source));
        assert_last_event::<T>(Event::<T>::LiquiditySourceEnabled(source).into());
    }

    disable_liquidity_source {
        let source = EnabledSourceTypes::<T>::get().first().copied().unwrap();
    }: {
        DexApi::<T>::disable_liquidity_source(RawOrigin::Root.into(), source).unwrap();
    }
    verify {
        assert!(!EnabledSourceTypes::<T>::get().contains(&source));
        assert_last_event::<T>(Event::<T>::LiquiditySourceDisabled(source).into());
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
