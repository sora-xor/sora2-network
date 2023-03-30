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
use common::prelude::SwapAmount;
use common::{DAI, XST};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::Pallet as XSTPool;

#[cfg(not(test))]
use price_tools::AVG_BLOCK_SPAN;

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
    where_clause {
        where T: price_tools::Config
    }

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

    quote {
        let caller = alice::<T>();
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        permissions::Pallet::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        trading_pair::Pallet::<T>::register(
            RawOrigin::Signed(caller.clone()).into(),
            dex_id,
            XST.into(),
            DAI.into(),
        ).unwrap();
        XSTPool::<T>::initialize_pool(RawOrigin::Signed(caller.clone()).into(), DAI.into()).unwrap();
        XSTPool::<T>::set_reference_asset(RawOrigin::Root.into(), DAI.into()).unwrap();

        #[cfg(not(test))]
        for _ in 1..=AVG_BLOCK_SPAN {
            price_tools::Pallet::<T>::incoming_spot_price(&DAI.into(), balance!(1), PriceVariant::Buy).unwrap();
            price_tools::Pallet::<T>::incoming_spot_price(&DAI.into(), balance!(1), PriceVariant::Sell).unwrap();
            price_tools::Pallet::<T>::incoming_spot_price(&XST.into(), balance!(0.5), PriceVariant::Buy).unwrap();
            price_tools::Pallet::<T>::incoming_spot_price(&XST.into(), balance!(0.5), PriceVariant::Sell).unwrap();
        }

        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(1),
            min_amount_out: balance!(0),
        };
    }: {
        XSTPool::<T>::quote(&dex_id, &DAI.into(), &XST.into(), amount.into(), true).unwrap();
    }
    verify {
        // can't check, nothing is changed
    }

    exchange {
        let caller = alice::<T>();
        let dex_id: T::DEXId = common::DEXId::Polkaswap.into();
        permissions::Pallet::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(common::hash(&dex_id)),
        ).unwrap();
        permissions::Pallet::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::MINT,
            permissions::Scope::Unlimited,
        )
        .unwrap();
        permissions::Pallet::<T>::assign_permission(
            caller.clone(),
            &caller,
            permissions::BURN,
            permissions::Scope::Unlimited,
        )
        .unwrap();
        trading_pair::Pallet::<T>::register(
            RawOrigin::Signed(caller.clone()).into(),
            dex_id,
            XST.into(),
            DAI.into(),
        ).unwrap();
        XSTPool::<T>::initialize_pool(RawOrigin::Signed(caller.clone()).into(), DAI.into()).unwrap();
        XSTPool::<T>::set_reference_asset(RawOrigin::Root.into(), DAI.into()).unwrap();

        assets::Pallet::<T>::mint_to(
            &DAI.into(),
            &caller,
            &caller,
            balance!(50000000),
        )
        .unwrap();

        #[cfg(not(test))]
        for _ in 1..=AVG_BLOCK_SPAN {
            price_tools::Pallet::<T>::incoming_spot_price(&DAI.into(), balance!(1), PriceVariant::Buy).unwrap();
            price_tools::Pallet::<T>::incoming_spot_price(&DAI.into(), balance!(1), PriceVariant::Sell).unwrap();
            price_tools::Pallet::<T>::incoming_spot_price(&XST.into(), balance!(0.5), PriceVariant::Buy).unwrap();
            price_tools::Pallet::<T>::incoming_spot_price(&XST.into(), balance!(0.5), PriceVariant::Sell).unwrap();
        }

        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(100),
            min_amount_out: balance!(0),
        };
        let initial_base_balance = Assets::<T>::free_balance(&DAI.into(), &caller).unwrap();
    }: {
        // run only for benchmarks, not for tests
        // TODO: remake when unit tests use chainspec
        #[cfg(not(test))]
        XSTPool::<T>::exchange(&caller, &caller, &dex_id, &DAI.into(), &XST.into(), amount.into()).unwrap();
    }
    verify {
        #[cfg(not(test))]
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DAI.into(), &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(100)
        );
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
