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

//! Faucet module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::{Config, Event, Module, Pallet};

use codec::Decode;
use frame_benchmarking::{benchmarks, Zero};
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use common::eth::EthereumAddress;
use common::{AssetName, AssetSymbol, Balance, XOR};
use rewards::{PswapFarmOwners, PswapWaifuOwners, ValOwners};

use assets::Pallet as Assets;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap_or_default()
}

fn add_assets<T: Config>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    for _i in 0..n {
        Assets::<T>::register(
            owner_origin.clone(),
            AssetSymbol(b"TOKEN".to_vec()),
            AssetName(b"TOKEN".to_vec()),
            Balance::zero(),
            true,
        )?;
    }

    Ok(())
}

/// Adds `n` of rewards
fn add_rewards<T: Config>(n: u32) {
    let unaccessible_eth_addr: EthereumAddress =
        hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A635").into();
    for _i in 0..n {
        ValOwners::<T>::insert(&unaccessible_eth_addr, 1);
        PswapFarmOwners::<T>::insert(&unaccessible_eth_addr, 1);
        PswapWaifuOwners::<T>::insert(&unaccessible_eth_addr, 1);
    }
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    transfer {
        let n in 1 .. 1000 => add_assets::<T>(n)?;
        let caller = alice::<T>();
        let caller_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(caller.clone()).into();
    }: {
        Pallet::<T>::transfer(
            caller_origin,
            XOR.into(),
            caller.clone(),
            100_u32.into()
        )?;
    }
    verify {
        assert_last_event::<T>(Event::Transferred(caller, 100_u32.into()).into())
    }

    reset_rewards {
        let n in 1 .. 1000 => add_rewards::<T>(n);
        let caller = alice::<T>();
        let caller_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(caller.clone()).into();
    }: {
        Pallet::<T>::reset_rewards(caller_origin)?;
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
        ExtBuilder::build().execute_with(|| {
            assert_ok!(test_benchmark_transfer::<Runtime>());
            assert_ok!(test_benchmark_reset_rewards::<Runtime>());
        });
    }
}
