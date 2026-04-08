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

//! ParachainApp pallet benchmarking

use super::*;
use bridge_types::substrate::XCMAppTransferStatus;
use bridge_types::substrate::PARENT_PARACHAIN_ASSET;
use bridge_types::traits::BridgeAssetRegistry;
use bridge_types::types::AssetKind;
use bridge_types::GenericNetworkId;
use bridge_types::SubNetworkId;
use frame_benchmarking::benchmarks;
use frame_benchmarking::whitelisted_caller;
use frame_system::RawOrigin;
use sp_std::prelude::*;
use traits::MultiCurrency;

const BASE_NETWORK_ID: SubNetworkId = SubNetworkId::Mainnet;

#[allow(unused_imports)]
use crate::Pallet as ParachainApp;
use currencies::Pallet as Currencies;

// This collection of benchmarks should include a benchmark for each
// call dispatched by the channel, i.e. each "app" pallet function
// that can be invoked by MessageDispatch. The most expensive call
// should be used in the `submit` benchmark.
//
// We rely on configuration via chain spec of the app pallets because
// we don't have access to their storage here.
benchmarks! {
    where_clause {
        where
            AssetNameOf<T>: Default,
            AssetSymbolOf<T>: Default,
            T: currencies::Config,
            Currencies<T>: MultiCurrency<T::AccountId, CurrencyId = AssetIdOf<T>>

    }
// Benchmark `submit` extrinsic under worst case conditions:
// * `submit` dispatches the DotApp::unlock call
// * `unlock` call successfully unlocks DOT
    register_thischain_asset {
        let a in 1..100;
        let asset_id = <T as Config>::AssetRegistry::register_asset(GenericNetworkId::Sub(Default::default()), Default::default(), Default::default())?;
    }: _(RawOrigin::Root, BASE_NETWORK_ID, asset_id.clone(), [0u8; 32].into(), (0..a).collect::<Vec<_>>(), 1u32.into())
    verify {
        assert_eq!(SidechainPrecision::<T>::get(BASE_NETWORK_ID, asset_id).unwrap(), 18);
    }

    register_sidechain_asset {
        let a in 1..100;
    }: _(RawOrigin::Root, BASE_NETWORK_ID, [0u8; 32].into(), Default::default(), Default::default(), 18, (0..a).collect::<Vec<_>>(), 1u32.into())
    verify {
        assert_eq!(SidechainPrecision::<T>::iter_prefix(BASE_NETWORK_ID).count(), 1);
    }

    add_assetid_paraid {
        let asset_id = <T as Config>::AssetRegistry::register_asset(GenericNetworkId::Sub(Default::default()), Default::default(), Default::default())?;
        ParachainApp::<T>::register_thischain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, asset_id.clone(), PARENT_PARACHAIN_ASSET, Default::default(), 1u32.into())?;
        ParachainApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), AssetKind::Thischain)?;
    }: _(RawOrigin::Root, BASE_NETWORK_ID, 1, asset_id.clone())
    verify {
        assert_eq!(AllowedParachainAssets::<T>::get(BASE_NETWORK_ID, 1), vec![asset_id]);
    }

    remove_assetid_paraid {
        let asset_id = <T as Config>::AssetRegistry::register_asset(GenericNetworkId::Sub(Default::default()), Default::default(), Default::default())?;
        ParachainApp::<T>::register_thischain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, asset_id.clone(), PARENT_PARACHAIN_ASSET, Default::default(), 1u32.into())?;
        ParachainApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), AssetKind::Thischain)?;
        ParachainApp::<T>::add_assetid_paraid(RawOrigin::Root.into(), BASE_NETWORK_ID, 1, asset_id.clone())?;
    }: _(RawOrigin::Root, BASE_NETWORK_ID, 1, asset_id)
    verify {
        assert_eq!(AllowedParachainAssets::<T>::get(BASE_NETWORK_ID, 1), vec![]);
    }

    update_transaction_status {
    }: {
        ParachainApp::<T>::update_transaction_status(<T as Config>::CallOrigin::try_successful_origin().unwrap(), Default::default(), XCMAppTransferStatus::Success)?;
    }

    mint {
        let who = whitelisted_caller();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        ParachainApp::<T>::register_thischain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, asset_id.clone(), PARENT_PARACHAIN_ASSET, Default::default(), 1u32.into())?;
        ParachainApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), AssetKind::Thischain)?;
        Currencies::<T>::deposit(asset_id.clone(), &who, 1000u32.into())?;
        T::BridgeAssetLocker::lock_asset(BASE_NETWORK_ID.into(), AssetKind::Thischain, &who, &asset_id, &1000u32.into())?;
    }: {
        ParachainApp::<T>::mint(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), None, who.clone(), 1000)?;
    }
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &who), 1000u32.into());
    }

    burn {
        let who = whitelisted_caller();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        ParachainApp::<T>::register_thischain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, asset_id.clone(), PARENT_PARACHAIN_ASSET, Default::default(), 1u32.into())?;
        ParachainApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), AssetKind::Thischain)?;
        Currencies::<T>::deposit(asset_id.clone(), &who, 1000u32.into())?;
    }: _(RawOrigin::Signed(
            who.clone()),
            BASE_NETWORK_ID,
            asset_id.clone(),
            ParachainAccountId::V3(staging_xcm::v3::MultiLocation::parent().pushed_with_interior([0u8; 32]).unwrap()),
            1000u32.into()
        )
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &who), 0u32.into());
    }

    finalize_asset_registration {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        ParachainApp::<T>::register_thischain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, asset_id.clone(), PARENT_PARACHAIN_ASSET, Default::default(), 1u32.into())?;
    }: {
        ParachainApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), AssetKind::Thischain)?;
    }
    verify {
        assert_eq!(AssetKinds::<T>::get(BASE_NETWORK_ID, asset_id), Some(AssetKind::Thischain));
    }

    refund {
        let who = whitelisted_caller();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        ParachainApp::<T>::register_thischain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, asset_id.clone(), PARENT_PARACHAIN_ASSET, Default::default(), 1u32.into())?;
        ParachainApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), AssetKind::Thischain)?;
        Currencies::<T>::deposit(asset_id.clone(), &who, 1000u32.into())?;
        T::BridgeAssetLocker::lock_asset(BASE_NETWORK_ID.into(), AssetKind::Thischain, &who, &asset_id, &1000u32.into())?;
    }: {
        ParachainApp::<T>::refund(BASE_NETWORK_ID.into(), Default::default(), who.clone(), asset_id.clone(), 1000u32.into())?;
    }
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &who), 1000u32.into());
    }

    impl_benchmark_test_suite!(ParachainApp, crate::mock::new_tester(), crate::mock::Test,);
}
