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

use super::*;

use common::{AssetId32, AssetIdOf, PredefinedAssetId, XOR};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;

#[allow(unused_imports)]
use crate::Pallet as BridgeProxy;

benchmarks! {
    where_clause {where AssetIdOf<T>: From<AssetId32<PredefinedAssetId>> }

    add_limited_asset {
        let asset_id: AssetIdOf<T> = XOR.into();
    }: _(RawOrigin::Root, asset_id)
    verify {
        assert!(LimitedAssets::<T>::get(asset_id));
    }

    remove_limited_asset {
        let asset_id: AssetIdOf<T> = XOR.into();
        BridgeProxy::<T>::add_limited_asset(RawOrigin::Root.into(), asset_id)?;
    }: _(RawOrigin::Root, asset_id)
    verify {
        assert!(!LimitedAssets::<T>::get(asset_id));
    }

    update_transfer_limit {
        let settings = TransferLimitSettings {
            max_amount: 1000,
            period_blocks: 100u32.into(),
        };
    }: _(RawOrigin::Root, settings.clone())
    verify {
        assert_eq!(TransferLimit::<T>::get(), settings);
    }

    impl_benchmark_test_suite!(BridgeProxy, crate::mock::new_tester(), crate::mock::Test,);
}
