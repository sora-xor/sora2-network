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

//! Regulated Assets module benchmarking.
#![cfg(feature = "runtime-benchmarks")]
#![cfg(feature = "wip")] // DEFI-R

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::EventRecord;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use super::*;

use common::{AssetManager, AssetName, AssetSymbol, Balance, DEFAULT_BALANCE_PRECISION};

// Support Functions
fn asset_owner<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

fn add_asset<T: Config>() -> AssetIdOf<T> {
    let owner = asset_owner::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);

    T::AssetManager::register_from(
        &owner,
        AssetSymbol(b"TOKEN".to_vec()),
        AssetName(b"TOKEN".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .expect("Failed to register asset")
}

benchmarks! {
    regulate_asset {
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner).into();
        let asset_id = add_asset::<T>();
    }: {
        Pallet::<T>::regulate_asset(owner_origin, asset_id).unwrap();
    }
    verify{
        assert_last_event::<T>(Event::AssetRegulated{
                asset_id
            }.into()
        );
    }

    issue_sbt{
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner).into();
        let asset_id = add_asset::<T>();
        let asset_name =  AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let bounded_vec_assets = BoundedVec::try_from(vec![asset_id]).unwrap();
        Pallet::<T>::regulate_asset(owner_origin.clone(), asset_id).unwrap();
    }: {
        Pallet::<T>::issue_sbt(
            owner_origin,
            asset_symbol,
            asset_name.clone(),
            None,
            None,
            None,
            None,
            bounded_vec_assets.clone(),
        ).unwrap();
    }
    verify{
        let sbts = Pallet::<T>::sbts_by_asset(asset_id);
        let sbt_asset_id = sbts.first().ok_or("No SBT asset found").unwrap();

        assert_last_event::<T>(Event::SoulboundTokenIssued {
             asset_id: *sbt_asset_id,
             owner: asset_owner::<T>(),
             allowed_assets:  vec![asset_id]
            }.into()
        );
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::TestRuntime
    );
}
