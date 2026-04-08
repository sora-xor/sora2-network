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

//! JettonApp pallet benchmarking

use crate::*;
use bridge_types::ton::{AdditionalTONInboundData, TonAddress, TonNetworkId};
use bridge_types::traits::BridgeAssetRegistry;
use bridge_types::types::CallOriginOutput;
use bridge_types::types::GenericAdditionalInboundData;
use bridge_types::GenericNetworkId;
use bridge_types::H256;
use currencies::Pallet as Currencies;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::UnfilteredDispatchable;
use frame_system::RawOrigin;
use sp_std::prelude::*;
use traits::MultiCurrency;

pub const BASE_NETWORK_ID: TonNetworkId = TonNetworkId::Mainnet;

benchmarks! {
    where_clause {where
        <T as frame_system::Config>::RuntimeOrigin: From<dispatch::RawOrigin<CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>>>,
        AssetNameOf<T>: From<Vec<u8>>,
        AssetSymbolOf<T>: From<Vec<u8>>,
        BalanceOf<T>: From<u128>,
        T: currencies::Config,
        Currencies<T>: MultiCurrency<T::AccountId, CurrencyId = AssetIdOf<T>, Balance = BalanceOf<T>>
    }

    // Benchmark `mint` extrinsic under worst case conditions:
    // * `mint` successfully adds amount to recipient account
    mint {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"TON".to_vec().into(), b"TON".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, TonAddress::new(0, H256::repeat_byte(1)), asset_id.clone(), 18).unwrap();
        let asset_kind = AssetKinds::<T>::get(&asset_id).unwrap();
        let (_, caller) = AppInfo::<T>::get().unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {network_id: GenericNetworkId::TON(BASE_NETWORK_ID), additional:GenericAdditionalInboundData::TON(AdditionalTONInboundData{source: caller}), ..Default::default()});
        let recipient: T::AccountId = account("recipient", 0, 0);
        let sender = TonAddress::new(0, H256::repeat_byte(2));
        let amount = 500u128;

        let call = Call::<T>::mint { token: TonAddress::empty().into(), sender: sender.into(), recipient: recipient.clone(), amount: amount.into()};

    }: { call.dispatch_bypass_filter(origin.into())? }
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &recipient), amount.into());
    }

    register_network {
        let address = TonAddress::new(0, H256::repeat_byte(1));
        let network_id = BASE_NETWORK_ID;
        let asset_name = b"TON".to_vec();
        let asset_symbol = b"TON".to_vec();
        assert!(!AppInfo::<T>::exists());
    }: _(RawOrigin::Root, network_id, address, asset_symbol.into(), asset_name.into(), 18)
    verify {
        assert!(AppInfo::<T>::exists());
    }

    register_network_with_existing_asset {
        let address = TonAddress::new(0, H256::repeat_byte(1));
        let network_id = BASE_NETWORK_ID;
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"TON".to_vec().into(), b"TON".to_vec().into())?;
        assert!(!AppInfo::<T>::exists());
    }: _(RawOrigin::Root, network_id, address, asset_id, 18)
    verify {
        assert!(AppInfo::<T>::exists());
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::empty().build(), crate::mock::Test,);
}
