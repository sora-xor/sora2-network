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
use codec::Decode;

#[cfg(feature = "wip")] // Dynamic fee
use crate::pallet::UpdatePeriod;
use crate::{Config, Pallet};
use common::{balance, AssetIdOf, XOR};
use frame_benchmarking::benchmarks;
use frame_support::sp_runtime::FixedU128;
use frame_system::RawOrigin;
use hex_literal::hex;
use traits::MultiCurrency;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

benchmarks! {
    update_multiplier {
        let new_multiplier = FixedU128::from(1);
    }: _(RawOrigin::Root, new_multiplier)
    verify {
        assert_eq!(Multiplier::<T>::get(), new_multiplier);
    }

    set_fee_update_period {
        let new_block_number = 3600_u32;
    }: _(RawOrigin::Root, new_block_number.into())
    verify {
        #[cfg(feature = "wip")] // Dynamic fee
        assert_eq!(<UpdatePeriod<T>>::get(), new_block_number.into());
    }

    set_small_reference_amount {
        let new_reference_amount = balance!(0.2);
    }: _(RawOrigin::Root, new_reference_amount)
    verify {
        #[cfg(feature = "wip")] // Dynamic fee
        assert_eq!(<SmallReferenceAmount<T>>::get(), new_reference_amount);
    }

    xorless_call {
        // TODO: update with whitelisted token
        let caller = alice::<T>();
        <T as common::Config>::MultiCurrency::deposit(XOR.into(), &caller, balance!(1))?;
        let call: Box<<T as Config>::RuntimeCall> = Box::new(frame_system::Call::remark { remark: vec![] }.into());
        let asset_id: AssetIdOf<T> = XOR.into();
    }: _(RawOrigin::Signed(caller), call, asset_id)

    impl_benchmark_test_suite!(Pallet, mock::ExtBuilder::build(), mock::Runtime);
}
