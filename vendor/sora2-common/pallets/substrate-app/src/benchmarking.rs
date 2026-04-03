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

//! SubstrateApp pallet benchmarking
#[allow(unused_imports)]
use crate::Pallet as SubstrateApp;
use currencies::Pallet as Currencies;

use super::*;
use bridge_types::traits::BridgeAssetRegistry;
use bridge_types::types::AssetKind;
use bridge_types::MainnetAccountId;
use bridge_types::SubNetworkId;
use frame_benchmarking::benchmarks;
use frame_benchmarking::whitelisted_caller;
use frame_system::RawOrigin;
use sp_std::prelude::*;
use traits::MultiCurrency;

const BASE_NETWORK_ID: SubNetworkId = SubNetworkId::Mainnet;

benchmarks! {
    where_clause {
        where
            AssetNameOf<T>: Default + From<Vec<u8>>,
            AssetSymbolOf<T>: Default + From<Vec<u8>>,
            T: currencies::Config,
            Currencies<T>: MultiCurrency<T::AccountId, CurrencyId = AssetIdOf<T>>

    }

    register_sidechain_asset {
    }: _(RawOrigin::Root, BASE_NETWORK_ID, GenericAssetId::Liberland(bridge_types::LiberlandAssetId::LLD), b"XOR".to_vec().into(), b"Sora".to_vec().into())

    incoming_thischain_asset_registration {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
    }: {
        SubstrateApp::<T>::incoming_thischain_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), GenericAssetId::Liberland(bridge_types::LiberlandAssetId::LLD))?
    }
    verify {
        assert_eq!(AssetKinds::<T>::get(BASE_NETWORK_ID, asset_id), Some(AssetKind::Thischain));
    }

    finalize_asset_registration {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        SubstrateApp::<T>::register_sidechain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, GenericAssetId::Sora([0; 32].into()), Default::default(), Default::default())?;
    }: {
        SubstrateApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), GenericAssetId::Sora([0; 32].into()), AssetKind::Thischain, 12)?;
    }
    verify {
        assert_eq!(AssetKinds::<T>::get(BASE_NETWORK_ID, asset_id), Some(AssetKind::Thischain));
    }

    mint {
        let who = whitelisted_caller::<T::AccountId>();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        SubstrateApp::<T>::register_sidechain_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, GenericAssetId::Sora([0; 32].into()), Default::default(), Default::default())?;
        SubstrateApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), GenericAssetId::Sora([0; 32].into()), AssetKind::Thischain, 18)?;
        Currencies::<T>::deposit(asset_id.clone(), &who, 1000u32.into())?;
        T::BridgeAssetLocker::lock_asset(BASE_NETWORK_ID.into(), AssetKind::Thischain, &who, &asset_id, &1000u32.into())?;
    }: {
        SubstrateApp::<T>::mint(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), GenericAccount::Sora(MainnetAccountId::new([0; 32])), who.clone(), GenericBalance::Substrate(1000))?;
    }
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &who), 1000u32.into());
    }

    burn {
        let who = whitelisted_caller::<T::AccountId>();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), Default::default(), Default::default())?;
        <T as Config>::AssetRegistry::manage_asset(BASE_NETWORK_ID.into(), asset_id.clone())?;
        SubstrateApp::<T>::finalize_asset_registration(<T as Config>::CallOrigin::try_successful_origin().unwrap(), asset_id.clone(), GenericAssetId::Liberland(bridge_types::LiberlandAssetId::LLD), AssetKind::Thischain, 18)?;
        Currencies::<T>::deposit(asset_id.clone(), &who, 1000u32.into())?;
    }: _(RawOrigin::Signed(
        who.clone()),
        BASE_NETWORK_ID,
        asset_id.clone(),
        GenericAccount::Sora(MainnetAccountId::new([0; 32])),
        1000u32.into()
    )
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &who), 0u32.into());
    }

    update_transaction_status {
    }: {
        SubstrateApp::<T>::update_transaction_status(<T as Config>::CallOrigin::try_successful_origin().unwrap(), Default::default(), bridge_types::types::MessageStatus::Done)?;
    }

    impl_benchmark_test_suite!(SubstrateApp, crate::mock::new_tester(), crate::mock::Test,);
}
