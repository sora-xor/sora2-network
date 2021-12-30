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
use frame_benchmarking::benchmarks;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

use crate::Pallet as PriceTools;

fn create_asset<T: Config>(prefix: Vec<u8>, index: u128) -> T::AssetId {
    let entropy: [u8; 32] = (prefix, index).using_encoded(blake2_256);
    T::AssetId::from(H256(entropy))
}

fn prepare_secondary_market<T: Config>(n: u32) {
    for i in 0..n {
        let asset = create_asset::<T>(b"asset".to_vec(), i.into());

        PriceTools::<T>::register_asset(&asset).unwrap();
        for m in 1..crate::AVG_BLOCK_SPAN {
            crate::PriceInfos::<T>::mutate(asset, |val| {
                let val = val.as_mut().unwrap();
                let price = balance!(m + i);
                val.spot_prices.push_back(price);
                val.needs_update = false;
                val.last_spot_price = price;
            });
        }
    }
}

benchmarks! {
    on_initialize {
        let n in 0 .. 10 => prepare_secondary_market::<T>(n);
        let mut infos_before = Vec::new();
        for i in 0..n {
            let asset = create_asset::<T>(b"asset".to_vec(), i.into());
            assert!(crate::PriceInfos::<T>::get(&asset).is_some());
            infos_before.push(crate::PriceInfos::<T>::get(&asset).unwrap().average_price);
        }
    }: {
        PriceTools::<T>::average_prices_calculation_routine();
    }
    verify {
        for i in 0..n {
            let asset = create_asset::<T>(b"asset".to_vec(), i.into());
            assert_ne!(infos_before.get(i as usize).unwrap(), &crate::PriceInfos::<T>::get(&asset).unwrap().average_price);
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
            assert_ok!(test_benchmark_on_initialize::<Runtime>());
        });
    }
}
