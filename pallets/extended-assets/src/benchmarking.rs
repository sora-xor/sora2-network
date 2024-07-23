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

use super::test_utils::{
    register_regulated_asset as utils_register_regulated_asset, register_sbt_asset,
};
use super::*;
use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::EventRecord;
use frame_system::RawOrigin;
use hex_literal::hex;
use sp_std::prelude::*;

use common::{AssetName, AssetSymbol};

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

benchmarks! {
    register_regulated_asset {
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner.clone()).into();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let asset_id = T::AssetManager::gen_asset_id(&owner);
    }: {
        Pallet::<T>::register_regulated_asset(
            owner_origin,
            AssetSymbol(b"TOKEN".to_vec()),
            AssetName(b"TOKEN".to_vec()),
            common::Balance::from(0u32),
            true,
            true,
            None,
            None
        ).unwrap();
    }
    verify{
        assert_last_event::<T>(Event::RegulatedAssetRegistered  {
             asset_id,
            }.into()
        );
    }


    issue_sbt{
        let owner = asset_owner::<T>();
        frame_system::Pallet::<T>::inc_providers(&owner);
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner.clone()).into();
        let asset_name =  AssetName(b"Soulbound Token".to_vec());
        let asset_symbol = AssetSymbol(b"SBT".to_vec());
        let asset_id = T::AssetManager::gen_asset_id(&owner);
    }: {
        Pallet::<T>::issue_sbt(
            owner_origin,
            asset_symbol,
            asset_name.clone(),
            None,
            None,
            None,
        ).unwrap();
    }
    verify{
        assert_last_event::<T>(Event::SoulboundTokenIssued  {
             asset_id,
             owner,
             external_url: None,
             image: None,
             issued_at: pallet_timestamp::Pallet::<T>::now()
            }.into()
        );
    }

    set_sbt_expiration {
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner.clone()).into();
        let sbt_asset_id = register_sbt_asset::<T>(&owner);
    }: {
        Pallet::<T>::set_sbt_expiration(owner_origin.clone(), owner,  sbt_asset_id, Some(T::Moment::from(100_u32)))?;
    }
    verify{
        assert_last_event::<T>(Event::SBTExpirationUpdated {
             sbt_asset_id,
             old_expires_at: None,
             new_expires_at: Some(T::Moment::from(100_u32))
            }.into()
        );
    }

    bind_regulated_asset_to_sbt {
        let owner = asset_owner::<T>();
        let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner.clone()).into();
        let asset_id = utils_register_regulated_asset::<T>(&owner);
        let sbt_asset_id = register_sbt_asset::<T>(&owner);

    }: {
        Pallet::<T>::bind_regulated_asset_to_sbt(owner_origin.clone(),sbt_asset_id, asset_id).unwrap();
    }
    verify{
        assert_last_event::<T>(Event::RegulatedAssetBoundToSBT {
             sbt_asset_id,
             regulated_asset_id: asset_id
            }.into()
        );
    }


    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::TestRuntime
    );
}
