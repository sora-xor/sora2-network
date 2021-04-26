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

#![cfg(feature = "runtime-benchmarks")]

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_support::assert_ok;
use frame_support::traits::Hooks;
use frame_system::pallet_prelude::OriginFor;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use common::{AssetName, AssetSymbol, XOR};

use super::*;

// Support Functions
fn asset_owner<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn account<T: Config>(shift: u32) -> T::AccountId {
    let mut bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    for _ in 0..shift {
        let mut shifted = false;
        let mut byte_index = bytes.len() - 1;
        while !shifted {
            if bytes[byte_index] != 0xff {
                bytes[byte_index] -= 1;
                shifted = true;
            } else {
                byte_index -= 1;
            }
        }
    }
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn signed_origin<T: Config>(account_id: T::AccountId) -> OriginFor<T> {
    RawOrigin::Signed(account_id.clone()).into()
}

fn prepare<T: Config>(count: u32) {
    let xor_asset: T::AssetId = XOR.into();
    let xor_owner = assets::Module::<T>::asset_owner(&xor_asset).unwrap();
    for _i in 0..count {
        let asset_id = assets::Module::<T>::gen_asset_id(&asset_owner::<T>());
        assert_ok!(assets::Module::<T>::register_asset_id(
            asset_owner::<T>(),
            asset_id.clone(),
            AssetSymbol(b"SYMBOL".to_vec()),
            AssetName(b"NAME".to_vec()),
            18,
            Balance::from(0u32),
            true,
        ));

        assert_ok!(trading_pair::Module::<T>::register(
            signed_origin::<T>(asset_owner::<T>()),
            Default::default(),
            XOR.into(),
            asset_id.clone(),
        ));

        assert_ok!(pool_xyk::Module::<T>::initialize_pool(
            signed_origin::<T>(asset_owner::<T>()),
            Default::default(),
            XOR.into(),
            asset_id.clone(),
        ));

        for j in 0..count {
            let account_id = account::<T>(j);
            assert_ok!(assets::Module::<T>::mint_to(
                &XOR.into(),
                &xor_owner,
                &account_id,
                balance!(50000),
            ));

            assert_ok!(assets::Module::<T>::mint_to(
                &asset_id,
                &asset_owner::<T>(),
                &account_id,
                balance!(50000),
            ));

            assert_ok!(pool_xyk::Module::<T>::deposit_liquidity(
                signed_origin::<T>(account_id),
                Default::default(),
                XOR.into(),
                asset_id,
                balance!(1.1),
                balance!(2.2),
                balance!(1.1),
                balance!(2.2),
            ));
        }
    }
}

benchmarks! {
    on_initialize_refresh {
        let n in 50 .. 100 => prepare::<T>(n);
    }: {
        Module::<T>::on_initialize(T::REFRESH_FREQUENCY)
    }

    on_initialize_vesting {
        let n in 50 .. 100 => prepare::<T>(n);
    }: {
        Module::<T>::on_initialize(T::VESTING_FREQUENCY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    #[ignore]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_on_initialize_refresh::<Runtime>());
            assert_ok!(test_benchmark_on_initialize_vesting::<Runtime>());
        });
    }
}
