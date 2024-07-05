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

use crate::{Config, Pallet, ReferrerBalances, Referrers};
use codec::Decode;
use common::weights::constants::SMALL_FEE;
use common::{balance, AssetInfoProvider, XOR};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;
use traits::currency::MultiCurrency;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27f");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

benchmarks! {
    reserve {
        let caller = alice::<T>();
        T::MultiCurrency::deposit(XOR.into(), &caller, balance!(50000)).unwrap();
    }: {
        Pallet::<T>::reserve(RawOrigin::Signed(alice::<T>()).into(), SMALL_FEE).unwrap();
    }
    verify {
        assert_eq!(ReferrerBalances::<T>::get(&alice::<T>()), Some(SMALL_FEE));
    }

    unreserve {
        let caller = alice::<T>();
        // Alice could have some start balance depending on chainspec
        let start_balance = <T as Config>::AssetInfoProvider::free_balance(&XOR.into(), &alice::<T>())?;
        T::MultiCurrency::deposit(XOR.into(), &caller, balance!(50000)).unwrap();
        Pallet::<T>::reserve(RawOrigin::Signed(alice::<T>()).into(), SMALL_FEE).unwrap();
    }: {
        Pallet::<T>::unreserve(RawOrigin::Signed(alice::<T>()).into(), SMALL_FEE).unwrap();
    }
    verify {
        assert_eq!(ReferrerBalances::<T>::get(&alice::<T>()), None);
        assert_eq!( <T as Config>::AssetInfoProvider::free_balance(&XOR.into(), &alice::<T>()), Ok(balance!(50000) + start_balance));
    }

    set_referrer {
        let alice = alice::<T>();
        let bob = bob::<T>();
    }: {
        Pallet::<T>::set_referrer(RawOrigin::Signed(alice.clone()).into(), bob.clone()).unwrap();
    }
    verify {
        assert_eq!(Referrers::<T>::get(&alice), Some(bob));
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::test_ext(),
        crate::mock::Runtime
    );
}
