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

use crate::{UmiNftReceivers, UmiNfts};
use codec::Decode;
use common::eth::EthAddress;
use common::AssetIdOf;
use common::{balance, AssetInfoProvider, AssetManager, PSWAP, VAL};
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use crate::{
    Config, Event, Pallet, PswapFarmOwners, PswapWaifuOwners, ReservesAcc, RewardInfo, ValOwners,
};

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("f08879dab4530529153a1bdb63e27cd3be45f1574a122b7e88579b6e5e60bd43");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

// Adds `n` of unaccessible rewards and after adds 1 reward that will be claimed
fn add_rewards<T: Config>(n: u32) {
    let unaccessible_eth_addr: EthAddress = hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A635").into();
    for _i in 0..n {
        ValOwners::<T>::insert(&unaccessible_eth_addr, RewardInfo::from(1));
        PswapFarmOwners::<T>::insert(&unaccessible_eth_addr, 1);
        PswapWaifuOwners::<T>::insert(&unaccessible_eth_addr, 1);
    }
    let eth_addr: EthAddress = hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into();
    ValOwners::<T>::insert(&eth_addr, RewardInfo::from(300));
    PswapFarmOwners::<T>::insert(&eth_addr, 300);
    PswapWaifuOwners::<T>::insert(&eth_addr, 300);
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    claim {
        add_rewards::<T>(1000);
        let reserves_acc = technical::Pallet::<T>::tech_account_id_to_account_id(&ReservesAcc::<T>::get()).unwrap();

        let val_asset: AssetIdOf<T> = VAL.into();
        let val_owner = T::AssetInfoProvider::get_asset_owner(&val_asset).unwrap();
        T::AssetManager::mint_to(
            &val_asset,
            &val_owner,
            &reserves_acc,
            balance!(50000),
        ).unwrap();

        let pswap_asset: AssetIdOf<T> = PSWAP.into();
        let pswap_owner = T::AssetInfoProvider::get_asset_owner(&pswap_asset).unwrap();
        T::AssetManager::mint_to(
            &pswap_asset,
            &pswap_owner,
            &reserves_acc,
            balance!(50000),
        ).unwrap();

        let caller = alice::<T>();
        let caller_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(caller.clone()).into();
        let signature = hex!("eb7009c977888910a96d499f802e4524a939702aa6fc8ed473829bffce9289d850b97a720aa05d4a7e70e15733eeebc4fe862dcb60e018c0bf560b2de013078f1c").into();
    }: {
        Pallet::<T>::claim(caller_origin, signature).unwrap();
    }
    verify {
        assert_last_event::<T>(Event::<T>::Claimed(caller).into())
    }

    add_umi_nfts_receivers {
        let n in 1 .. 1000;
        let mut addr_vec: Vec<EthAddress> = vec![];
        for i in 0 .. n {
            let raw_addr: Vec<u8> = Vec::from(i.to_be_bytes());
            let reminder: Vec<u8> = Vec::from([0u8; 16]);
            let raw_addr: [u8; 20] = vec![raw_addr, reminder].concat().try_into().expect("Failed to cast address vector to array");
            let addr = EthAddress::from(raw_addr);
            addr_vec.push(addr);
        }
    }: {
        let _ = Pallet::<T>::add_umi_nft_receivers(RawOrigin::Root.into(), addr_vec.clone()).unwrap();
    }
    verify {
        addr_vec.iter()
            .for_each(|receiver| {
                let nft = UmiNftReceivers::<T>::get(receiver);
                assert_eq!(
                    nft,
                    vec![1; UmiNfts::<T>::get().len()]
                );
            });

    }

    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
