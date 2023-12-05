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

use common::XOR;
use common::{AssetId32, PredefinedAssetId};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;

fn alice<T: Config>() -> T::AccountId {
    let bytes =
        hex_literal::hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

fn set_xor_as_collateral_type<T: Config>() {
    CollateralTypes::<T>::set::<AssetIdOf<T>>(
        XOR.into(),
        Some(CollateralRiskParameters {
            max_supply: balance!(1000),
            liquidation_ratio: Perbill::from_float(0.5),
            stability_fee_rate: Default::default(),
        }),
    );
    KusdHardCap::<T>::set(balance!(1000));
}

benchmarks! {
    where_clause {
        where T::AssetId: From<AssetId32<PredefinedAssetId>>
    }

    create_cdp {
        set_xor_as_collateral_type::<T>();
        let caller: T::AccountId = alice::<T>();
    }: create_cdp(RawOrigin::Signed(caller.clone()), XOR.into())
    verify {
        // TODO verify
    }
}
