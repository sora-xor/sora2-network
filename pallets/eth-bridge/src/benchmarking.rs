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

//! Ethereum bridge module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use crate::Pallet;
use codec::Decode;
use common::eth::public_key_to_eth_address;
use common::{balance, XOR};
use frame_benchmarking::{benchmarks, BenchmarkError};
use frame_support::sp_runtime::traits::IdentifyAccount;
use frame_support::sp_runtime::MultiSigner;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;

type Assets<T> = assets::Pallet<T>;

fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = events.last().unwrap();
    assert_eq!(event, &system_event);
}

benchmarks! {
    transfer_to_sidechain {
        let caller = alice::<T>();
        let asset_id: T::AssetId = XOR.into();
        let net_id = 0u32.into();
        let bridge_acc_id = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        Assets::<T>::mint_to(&asset_id, &bridge_acc_id, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
    }: transfer_to_sidechain(
        RawOrigin::Signed(caller.clone()),
        asset_id,
        EthAddress::from(hex!("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A")),
        balance!(100),
        net_id
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&asset_id, &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(100)
        );
    }

    request_from_sidechain {
        let caller = alice::<T>();
        let asset_id: T::AssetId = XOR.into();
        let net_id = 0u32.into();
        let bridge_acc_id = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        Assets::<T>::mint_to(&asset_id, &bridge_acc_id, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        let req_hash = H256([1u8; 32]);
    }: request_from_sidechain(
        RawOrigin::Signed(caller.clone()),
        req_hash,
        IncomingTransactionRequestKind::Transfer.into(),
        net_id
    )
    verify {
        assert_last_event::<T>(Event::<T>::RequestRegistered(req_hash).into());
    }

    register_incoming_request {
        let net_id = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let alice = alice::<T>();
        Assets::<T>::mint_to(&asset_id, &caller, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        frame_support::assert_ok!(crate::Pallet::<T>::transfer_to_sidechain(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id,
            EthAddress::from(hex!("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A")),
            balance!(100),
            net_id
        ));
        let inc_req = IncomingRequest::Transfer(IncomingTransfer::<T> {
            from: EthAddress::from([10u8; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::SidechainOwned,
            amount: 1u32.into(),
            author: alice.clone(),
            tx_hash: H256([1u8; 32]),
            at_height: 0,
            timepoint: Default::default(),
            network_id: net_id,
            should_take_fee: false,
        });
        let req_hash = OffchainRequest::incoming(inc_req.clone()).hash();
    }: register_incoming_request(
        RawOrigin::Signed(caller.clone()),
        inc_req
    )
    verify {
        assert_eq!(crate::RequestStatuses::<T>::get(net_id, req_hash).unwrap(), RequestStatus::Pending);
    }

    finalize_incoming_request {
        let net_id = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let req_hash = H256([1u8; 32]);
        let alice = alice::<T>();
        Assets::<T>::mint_to(&asset_id, &caller, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        frame_support::assert_ok!(crate::Pallet::<T>::transfer_to_sidechain(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id,
            EthAddress::from(hex!("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A")),
            balance!(100),
            net_id
        ));
        let inc_req = IncomingRequest::Transfer(IncomingTransfer::<T> {
            from: EthAddress::from([10u8; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::SidechainOwned,
            amount: 1u32.into(),
            author: alice.clone(),
            tx_hash: req_hash,
            at_height: 0,
            timepoint: Default::default(),
            network_id: net_id,
            should_take_fee: false,
        });
        let req_hash = OffchainRequest::incoming(inc_req.clone()).hash();
        frame_support::assert_ok!(crate::Pallet::<T>::register_incoming_request(
            RawOrigin::Signed(caller.clone()).into(),
            inc_req
        ));
    }: finalize_incoming_request(
        RawOrigin::Signed(caller.clone()),
        req_hash,
        net_id
    )
    verify {
        assert_last_event::<T>(Event::<T>::IncomingRequestFinalized(req_hash).into());
    }

    approve_request {
        let net_id = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let alice = alice::<T>();
        Assets::<T>::mint_to(&asset_id, &caller, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        frame_support::assert_ok!(crate::Pallet::<T>::transfer_to_sidechain(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id,
            EthAddress::from(hex!("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A")),
            balance!(100),
            net_id
        ));
        let request = Requests::<T>::get(net_id, RequestsQueue::<T>::get(net_id).first().unwrap()).unwrap();
        let (out_req, req_hash) = request.as_outgoing().unwrap();
        let encoded_request = out_req.to_eth_abi(req_hash).map_err(|_| BenchmarkError::Stop("EthAbi encoding error"))?;
        let sk = secp256k1::SecretKey::parse(&[1; 32]).unwrap();
        let public = secp256k1::PublicKey::from_secret_key(&sk);
        let address = public_key_to_eth_address(&public);
        let public = ecdsa::Public(public.serialize_compressed());
        let account_id = T::AccountId::decode(&mut &MultiSigner::Ecdsa(public.clone()).into_account().encode()[..]).unwrap();
        Pallet::<T>::force_add_peer(RawOrigin::Root.into(), account_id.clone(), address, net_id).unwrap();
        let (signature, _) = Pallet::<T>::sign_message(encoded_request.as_raw(), &sk);
    }: approve_request(
        RawOrigin::Signed(account_id.clone()),
        public,
        req_hash,
        signature,
        net_id
    )
    verify {
        assert_eq!(RequestApprovals::<T>::get(net_id, &req_hash).len(), 1);
    }

    approve_request_finalize {
        let net_id = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let alice = alice::<T>();
        Assets::<T>::mint_to(&asset_id, &caller, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        frame_support::assert_ok!(crate::Pallet::<T>::transfer_to_sidechain(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id,
            EthAddress::from(hex!("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A")),
            balance!(100),
            net_id
        ));
        let request = Requests::<T>::get(net_id, RequestsQueue::<T>::get(net_id).first().unwrap()).unwrap();
        let (out_req, req_hash) = request.as_outgoing().unwrap();
        let encoded_request = out_req.to_eth_abi(req_hash).map_err(|_| BenchmarkError::Stop("EthAbi encoding error"))?;
        let sk = secp256k1::SecretKey::parse(&[1; 32]).unwrap();
        let public = secp256k1::PublicKey::from_secret_key(&sk);
        let address = public_key_to_eth_address(&public);
        let public = ecdsa::Public(public.serialize_compressed());
        let account_id = T::AccountId::decode(&mut &MultiSigner::Ecdsa(public.clone()).into_account().encode()[..]).unwrap();
        Pallet::<T>::force_add_peer(RawOrigin::Root.into(), account_id.clone(), address, net_id).unwrap();
        let (signature, _) = Pallet::<T>::sign_message(encoded_request.as_raw(), &sk);
        RequestApprovals::<T>::mutate(net_id, &req_hash, |v| {
            for i in 0..majority(crate::Peers::<T>::get(net_id).len()) - 1 {
                v.insert(SignatureParams {
                    v: i as u8,
                    ..Default::default()
                });
            }
        });
    }: approve_request(
        RawOrigin::Signed(account_id.clone()),
        public,
        req_hash,
        signature,
        net_id
    )
    verify {
        assert_last_event::<T>(Event::<T>::ApprovalsCollected(req_hash).into());
    }

    abort_request {
        let net_id = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let alice = alice::<T>();
        Assets::<T>::mint_to(&asset_id, &caller, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        frame_support::assert_ok!(crate::Pallet::<T>::transfer_to_sidechain(
            RawOrigin::Signed(caller.clone()).into(),
            asset_id,
            EthAddress::from(hex!("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A")),
            balance!(100),
            net_id
        ));
        let request = Requests::<T>::get(net_id, RequestsQueue::<T>::get(net_id).first().unwrap()).unwrap();
        let req_hash = request.hash();
    }: abort_request(
        RawOrigin::Signed(caller.clone()),
        req_hash,
        crate::Error::<T>::Other.into(),
        net_id
    )
    verify {
        assert_last_event::<T>(Event::<T>::RequestAborted(req_hash).into());
    }
}

#[cfg(test)]
mod bench_tests {
    use super::*;
    use crate::tests::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Pallet::<Runtime>::test_benchmark_transfer_to_sidechain());
            assert_ok!(Pallet::<Runtime>::test_benchmark_request_from_sidechain());
            assert_ok!(Pallet::<Runtime>::test_benchmark_register_incoming_request());
            assert_ok!(Pallet::<Runtime>::test_benchmark_finalize_incoming_request());
            assert_ok!(Pallet::<Runtime>::test_benchmark_approve_request());
            assert_ok!(Pallet::<Runtime>::test_benchmark_approve_request_finalize());
            assert_ok!(Pallet::<Runtime>::test_benchmark_abort_request());
        });
    }
}
