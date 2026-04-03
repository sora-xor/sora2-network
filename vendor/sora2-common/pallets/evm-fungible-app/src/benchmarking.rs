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

//! ERC20App pallet benchmarking

use crate::*;
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::traits::BridgeAssetRegistry;
use bridge_types::traits::EVMBridgeWithdrawFee;
use bridge_types::types::AssetKind;
use bridge_types::types::CallOriginOutput;
use bridge_types::types::GenericAdditionalInboundData;
use bridge_types::EVMChainId;
use bridge_types::GenericNetworkId;
use bridge_types::H256;
use currencies::Pallet as Currencies;
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::traits::UnfilteredDispatchable;
use frame_system::RawOrigin;
use sp_std::prelude::*;
use traits::MultiCurrency;

pub const BASE_NETWORK_ID: EVMChainId = EVMChainId::repeat_byte(1);

benchmarks! {
    where_clause {where
        <T as frame_system::Config>::RuntimeOrigin: From<dispatch::RawOrigin<CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>>>,
        AssetNameOf<T>: From<Vec<u8>>,
        AssetSymbolOf<T>: From<Vec<u8>>,
        BalanceOf<T>: From<u128>,
        T: currencies::Config,
        Currencies<T>: MultiCurrency<T::AccountId, CurrencyId = AssetIdOf<T>, Balance = BalanceOf<T>>
    }

    burn {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), asset_id.clone(), 18).unwrap();
        let caller: T::AccountId = whitelisted_caller();
        let recipient = H160::repeat_byte(2);
        let amount = 1000u128;

        Currencies::<T>::deposit(asset_id.clone(), &caller, amount.into())?;
    }: burn(RawOrigin::Signed(caller.clone()), BASE_NETWORK_ID, asset_id.clone(), recipient, amount.into())
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &caller), 0u128.into());
    }

    // Benchmark `mint` extrinsic under worst case conditions:
    // * `mint` successfully adds amount to recipient account
    mint {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), asset_id.clone(), 18).unwrap();
        let asset_kind = AssetKinds::<T>::get(BASE_NETWORK_ID, &asset_id).unwrap();
        let caller = AppAddresses::<T>::get(BASE_NETWORK_ID).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {network_id: GenericNetworkId::EVM(BASE_NETWORK_ID), additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData{source: caller}), ..Default::default()});

        let recipient: T::AccountId = account("recipient", 0, 0);
        let sender = H160::zero();
        let amount = 500u128;

        let call = Call::<T>::mint { token: H160::zero(), sender, recipient: recipient.clone(), amount: amount.into()};

    }: { call.dispatch_bypass_filter(origin.into())? }
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &recipient), amount.into());
    }

    register_network {
        let address = H160::repeat_byte(98);
        let network_id = BASE_NETWORK_ID;
        let asset_name = b"ETH".to_vec();
        let asset_symbol = b"ETH".to_vec();
        assert!(!AppAddresses::<T>::contains_key(network_id));
    }: _(RawOrigin::Root, network_id, address, asset_symbol.into(), asset_name.into(), 18)
    verify {
        assert!(AppAddresses::<T>::contains_key(network_id));
    }

    register_network_with_existing_asset {
        let address = H160::repeat_byte(98);
        let network_id = BASE_NETWORK_ID;
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        assert!(!AppAddresses::<T>::contains_key(network_id));
    }: _(RawOrigin::Root, network_id, address, asset_id, 18)
    verify {
        assert!(AppAddresses::<T>::contains_key(network_id));
    }

    claim_relayer_fees {
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), asset_id.clone(), 18).unwrap();
        let caller: T::AccountId = whitelisted_caller();
        let claimer: T::AccountId = account("claimer", 0, 0);
        let address = H160::repeat_byte(98);
        let message = crate::Pallet::<T>::get_claim_prehashed_message(BASE_NETWORK_ID, &claimer);
        let pk = sp_io::crypto::ecdsa_generate(11u32.into(), None);
        let signature = sp_io::crypto::ecdsa_sign_prehashed(11u32.into(), &pk, &message.0).unwrap();

        // We need to have full public key to get Ethereum address, but sp_core public key don't have such conversion method.
        let pk = sp_io::crypto::secp256k1_ecdsa_recover(&signature.0, &message.0).map_err(|_| "Failed to recover signature").unwrap();
        let relayer = H160::from_slice(&sp_io::hashing::keccak_256(&pk)[12..]);

        let network_id = BASE_NETWORK_ID;
        crate::Pallet::<T>::update_base_fee(BASE_NETWORK_ID, 10u64.into(), 1u64);
        Currencies::<T>::deposit(asset_id.clone(), &caller, 1_000_000_000_000_000_000u128.into())?;
        crate::Pallet::<T>::withdraw_transfer_fee(&caller, BASE_NETWORK_ID, asset_id.clone())?;
        crate::Pallet::<T>::on_fee_paid(BASE_NETWORK_ID, relayer, 100u64.into());
    }: _(RawOrigin::Signed(claimer.clone()), network_id, relayer, signature)
    verify {
        assert_eq!(Currencies::<T>::free_balance(asset_id, &claimer), 100u128.into());
    }

    register_existing_sidechain_asset {
        let base_asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), base_asset_id, 18).unwrap();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        let token = H160::repeat_byte(2);
        assert!(!AssetsByAddresses::<T>::contains_key(BASE_NETWORK_ID, token));
    }: _(RawOrigin::Root, BASE_NETWORK_ID, token, asset_id, 18)
    verify {
        assert!(AssetsByAddresses::<T>::contains_key(BASE_NETWORK_ID, token));
    }

    register_sidechain_asset {
        let base_asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), base_asset_id, 18).unwrap();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        let token = H160::repeat_byte(2);
        let asset_name = b"ETH".to_vec();
        let asset_symbol = b"ETH".to_vec();
        assert!(!AssetsByAddresses::<T>::contains_key(BASE_NETWORK_ID, token));
    }: _(RawOrigin::Root, BASE_NETWORK_ID, token, asset_symbol.into(), asset_name.into(), 18)
    verify {
        assert!(AssetsByAddresses::<T>::contains_key(BASE_NETWORK_ID, token));
    }

    register_thischain_asset {
        let base_asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), base_asset_id, 18).unwrap();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
    }: _(RawOrigin::Root, BASE_NETWORK_ID, asset_id)
    verify {
    }

    register_asset_internal {
        let base_asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"ETH".to_vec().into(), b"ETH".to_vec().into())?;
        crate::Pallet::<T>::register_network_with_existing_asset(RawOrigin::Root.into(), BASE_NETWORK_ID, H160::repeat_byte(1), base_asset_id, 18).unwrap();
        let asset_id = <T as Config>::AssetRegistry::register_asset(BASE_NETWORK_ID.into(), b"DAI".to_vec().into(), b"DAI".to_vec().into())?;
        let who = AppAddresses::<T>::get(BASE_NETWORK_ID).unwrap();
        let origin = dispatch::RawOrigin::new(CallOriginOutput {network_id: GenericNetworkId::EVM(BASE_NETWORK_ID), additional: GenericAdditionalInboundData::EVM(AdditionalEVMInboundData{source: who}), ..Default::default()});
        let address = H160::repeat_byte(98);
        assert!(!TokenAddresses::<T>::contains_key(BASE_NETWORK_ID, &asset_id));
    }: _(origin, asset_id.clone(), address)
    verify {
        assert_eq!(AssetKinds::<T>::get(BASE_NETWORK_ID, &asset_id), Some(AssetKind::Thischain));
        assert!(TokenAddresses::<T>::contains_key(BASE_NETWORK_ID, &asset_id));
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::new_tester(), crate::mock::Test,);
}
