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

//! Band module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Pallet as Band;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;

fn relayer<T: Config>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode relayer ID")
}

fn symbol<T: Config>(sym: &str) -> T::Symbol {
    let bytes = sym.encode();
    T::Symbol::decode(&mut &bytes[..]).expect("Failed to decode symbol")
}

benchmarks! {
    relay {
        let relayer = relayer::<T>();
        let euro = symbol::<T>("EURO");
        Band::<T>::add_relayers(RawOrigin::Root.into(), vec![relayer.clone()])?;
    }: _(RawOrigin::Signed(relayer), vec![(euro.clone(), 2)].try_into().unwrap(), 100, 1)
    verify {
        assert_eq!(Band::<T>::rates(euro), Some(BandRate {
            value: Band::<T>::raw_rate_into_balance(2).expect("failed to convert value to Balance"),
            last_updated: 100,
            request_id: 1,
            dynamic_fee: fixed!(0),
            last_updated_block: 1u32.into(),
        }));
    }

    force_relay {
        let relayer = relayer::<T>();
        let euro = symbol::<T>("EURO");
        Band::<T>::add_relayers(RawOrigin::Root.into(), vec![relayer.clone()])?;
    }: _(RawOrigin::Signed(relayer), vec![(euro.clone(), 2)].try_into().unwrap(), 100, 1)
    verify {
        assert_eq!(Band::<T>::rates(euro), Some(BandRate {
            value: Band::<T>::raw_rate_into_balance(2).expect("failed to convert value to Balance"),
            last_updated: 100,
            request_id: 1,
            dynamic_fee: fixed!(0),
            last_updated_block: 1u32.into(),
        }));
    }

    add_relayers {
        let relayer = relayer::<T>();
    }: _(RawOrigin::Root, vec![relayer.clone()])
    verify {
        assert!(Band::<T>::trusted_relayers().unwrap().contains(&relayer));
    }

    remove_relayers {
        let relayer = relayer::<T>();
        Band::<T>::add_relayers(RawOrigin::Root.into(), vec![relayer.clone()])?;
    }: _(RawOrigin::Root, vec![relayer.clone()])
    verify {
        assert!(!Band::<T>::trusted_relayers().unwrap().contains(&relayer));
    }

    set_dynamic_fee_parameters {
        let parameters = FeeCalculationParameters::new(fixed!(0), fixed!(0), fixed!(0));
    }: _(RawOrigin::Root, parameters)
    verify {}

    impl_benchmark_test_suite!(Band, crate::mock::new_test_ext(), crate::mock::Runtime);
}
