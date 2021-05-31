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

use codec::{Decode, Encode};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use orml_traits::MultiCurrency;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::prelude::*;

use common::{AssetName, AssetSymbol, DEXId, XOR};

use crate::{Pallet as PriceTools};
use assets::Pallet as Assets;
use pool_xyk::Pallet as XYKPool;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn create_asset<T: Config>(prefix: Vec<u8>, index: u128) -> T::AssetId {
    let entropy: [u8; 32] = (prefix, index).using_encoded(blake2_256);
    T::AssetId::from(H256(entropy))
}

fn prepare_secondary_market<T: Config>(n: u128) {
    let caller = alice::<T>();
    let caller_origin: <T as frame_system::Config>::Origin =
        RawOrigin::Signed(caller.clone()).into();
    T::Currency::deposit(XOR.into(), &caller, balance!(1)).unwrap();

    for i in 0..n {
        let asset = create_asset::<T>(b"asset".to_vec(), i);
        T::Currency::deposit(XOR.into(), &caller, balance!(100)).unwrap();
        Assets::<T>::register_asset_id(
            caller.clone(),
            asset.clone(),
            AssetSymbol(b"TST".to_vec()),
            AssetName(b"TST".to_vec()),
            18,
            balance!(200),
            true,
        )
        .unwrap();
        TradingPair::<T>::register(caller_origin.clone(), DEX.into(), XOR.into(), asset).unwrap();
        XYKPool::<T>::initialize_pool(caller_origin.clone(), DEX.into(), XOR.into(), asset)
            .unwrap();
        XYKPool::<T>::deposit_liquidity(
            caller_origin.clone(),
            DEX.into(),
            XOR.into(),
            asset,
            balance!(100),
            balance!(200),
            balance!(0),
            balance!(0),
        )
        .unwrap();

        PriceTools::<T>::register_asset(&asset).unwrap();
    }
    for _ in 1..=crate::AVG_BLOCK_SPAN {
        PriceTools::<T>::average_prices_calculation_routine();
    }
}

benchmarks! {
    on_initialize {
        let caller = alice::<T>();
        prepare_secondary_market::<T>(10);
    }: {
        PriceTools::<T>::average_prices_calculation_routine();
    }
    verify {
        // different behaviour in test and runtime, execution success of routine is sufficient for check
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
