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

//! XST pool module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use common::{DAI, XST};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    let account = T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID");
    frame_system::Pallet::<T>::inc_providers(&account);
    account
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    initialize_pool {
        let caller = alice::<T>();
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        trading_pair::Pallet::<T>::register(
            RawOrigin::Signed(caller.clone()).into(),
            DEXId::Polkaswap.into(),
            XST.into(),
            DAI.into(),
        ).unwrap();
        permissions::Pallet::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DAI.into()
    )
    verify {
        assert_last_event::<T>(Event::<T>::PoolInitialized(common::DEXId::Polkaswap.into(), DAI.into()).into())
    }

    set_reference_asset {
    }: _(
        RawOrigin::Root,
        DAI.into()
    )
    verify {
        assert_last_event::<T>(Event::ReferenceAssetChanged(DAI.into()).into())
    }

    set_reference_asset {
    }: _(
        RawOrigin::Root,
        DAI.into()
    )
    verify {
        assert_last_event::<T>(Event::ReferenceAssetChanged(DAI.into()).into())
    }

    enable_synthetic_asset{
        let synthetic = common::AssetId32::from_bytes(hex!("0200012345000000000000000000000000000000000000000000000000000000").into());
    }: _(
        RawOrigin::Root,
        synthetic.into()
    )
    verify {
        assert_last_event::<T>(Event::SyntheticAssetEnabled(synthetic.into()).into())
    }

    set_synthetic_base_asset_floor_price {
    }: _(RawOrigin::Root, balance!(200))
    verify {
        assert_last_event::<T>(Event::SyntheticBaseAssetFloorPriceChanged(balance!(200)).into())
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
