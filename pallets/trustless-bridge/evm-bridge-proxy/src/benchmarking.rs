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

use super::*;

use bridge_types::types::MessageDirection;
use bridge_types::GenericAccount;
use common::{balance, AssetId32, PredefinedAssetId, XOR};
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use traits::MultiCurrency;

pub const BASE_NETWORK_ID: GenericNetworkId = GenericNetworkId::EVM(EVMChainId::zero());

#[allow(unused_imports)]
use crate::Pallet as ETHApp;

use assets::Pallet as Assets;

benchmarks! {
    where_clause {where T::AssetId: From<AssetId32<PredefinedAssetId>> }
    // Benchmark `burn` extrinsic under worst case conditions:
    // * `burn` successfully substracts amount from caller account
    // * The channel executes incentivization logic
    burn {
        let caller: T::AccountId = whitelisted_caller();
        let asset_id: T::AssetId = XOR.into();
        let asset_owner = Assets::<T>::asset_owner(asset_id).unwrap();
        let amount = balance!(20);
        let asset_id: T::AssetId = XOR.into();
        <T as assets::Config>::Currency::deposit(asset_id.clone(), &caller, amount)?;
    }: _(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, XOR.into(), GenericAccount::EVM(H160::default()), 1000)
    verify {
        let (message_id, _) = Senders::<T>::iter_prefix(BASE_NETWORK_ID).next().unwrap();
        let req = Transactions::<T>::get(&caller, (BASE_NETWORK_ID, message_id)).unwrap();
        assert!(
            req == BridgeRequest {
                source: GenericAccount::Sora(caller.clone()),
                dest: GenericAccount::EVM(H160::default()),
                asset_id: XOR.into(),
                amount: 1000,
                status: MessageStatus::InQueue,
                start_timestamp: 0u32.into(),
                end_timestamp: None,
                direction: MessageDirection::Outbound,
            }
        );
    }

    impl_benchmark_test_suite!(ETHApp, crate::mock::new_tester(), crate::mock::Test,);
}
