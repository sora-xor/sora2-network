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
use bridge_types::traits::BridgeAssetLockChecker;
use codec::Decode;
use common::eth::public_key_to_eth_address;
use common::{balance, KUSD, XOR};
use frame_benchmarking::{account, benchmarks, BenchmarkError};
use frame_support::migrations::SteppedMigration;
use frame_support::sp_runtime::traits::{IdentifyAccount, One, Zero};
use frame_support::sp_runtime::MultiSigner;
use frame_support::traits::{GetStorageVersion, StorageVersion};
use frame_support::weights::WeightMeter;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;

type Assets<T> = assets::Pallet<T>;

fn alice<T: Config>() -> <T as frame_system::pallet::Config>::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    <T as frame_system::pallet::Config>::AccountId::decode(&mut &bytes[..])
        .expect("Failed to decode account ID")
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::pallet::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = events.last().unwrap();
    assert_eq!(event, &system_event);
}

fn benchmark_max_peer_set<T: Config>(
    network_id: T::NetworkId,
    signer: <T as frame_system::Config>::AccountId,
) -> BTreeSet<<T as frame_system::Config>::AccountId> {
    let mut peers = BTreeSet::new();
    peers.insert(signer);
    let mut index = 0u32;
    while peers.len() < MAX_PEERS {
        peers.insert(account("eth_bridge_peer", index, 0));
        index = index.saturating_add(1);
    }
    Peers::<T>::insert(network_id, &peers);
    peers
}

fn benchmark_seed_approvals<T: Config>(
    network_id: T::NetworkId,
    request_hash: H256,
    peers: &BTreeSet<<T as frame_system::Config>::AccountId>,
    signer: &<T as frame_system::Config>::AccountId,
    count: usize,
) {
    let approvers = peers
        .iter()
        .filter(|peer| *peer != signer)
        .take(count)
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(approvers.len(), count);
    let approvals = (0..count)
        .map(|index| SignatureParams {
            v: index as u8,
            ..Default::default()
        })
        .collect::<BTreeSet<_>>();
    RequestApprovals::<T>::insert(network_id, request_hash, approvals);
    RequestApprovers::<T>::insert(network_id, request_hash, approvers);
}

fn benchmark_fill_queue<T: Config>(
    network_id: T::NetworkId,
    len: usize,
    target_hash: Option<H256>,
) {
    let mut queue = (0..len)
        .map(|index| H256::from_low_u64_be(index as u64 + 1))
        .collect::<Vec<_>>();
    if let Some(target_hash) = target_hash {
        assert!(!queue.is_empty());
        queue[len - 1] = target_hash;
    }
    RequestsQueue::<T>::insert(network_id, queue);
}

fn benchmark_fill_account_requests<T: Config>(
    account_id: &<T as frame_system::Config>::AccountId,
    network_id: T::NetworkId,
    len: usize,
) {
    let requests = (0..len)
        .map(|index| (network_id, H256::from_low_u64_be(index as u64 + 1)))
        .collect::<Vec<_>>();
    AccountRequests::<T>::insert(account_id, requests);
}

fn benchmark_enable_legacy_xor_decommission<T: Config>(
    network_id: T::NetworkId,
    request_hash: H256,
) {
    let decommissioned_at = frame_system::pallet_prelude::BlockNumberFor::<T>::one();
    LegacyEthereumXorDecommissioned::<T>::put(true);
    LegacyEthereumXorDecommissionedAt::<T>::put(decommissioned_at);
    RequestSubmissionHeight::<T>::insert(
        network_id,
        request_hash,
        frame_system::pallet_prelude::BlockNumberFor::<T>::zero(),
    );
}

fn benchmark_enable_re_registered_xor<T: Config>(network_id: T::NetworkId, asset_id: T::AssetId) {
    LegacyEthereumXorDecommissioned::<T>::put(true);
    RegisteredAsset::<T>::insert(network_id, asset_id, AssetKind::Thischain);
    if let Some(token_address) = RegisteredSidechainToken::<T>::take(network_id, asset_id) {
        RegisteredSidechainAsset::<T>::remove(network_id, token_address);
    }
}

benchmarks! {
    // Release qualification benchmark for the terminal step with no account histories.
    #[extra]
    migrate_account_requests_v3_to_v4_empty {
        StorageVersion::new(3).put::<Pallet<T>>();
        let mut meter = WeightMeter::new();
    }: {
        crate::migration::AccountRequestsV3ToV4::<T>::step(None, &mut meter)
            .map_err(|_| BenchmarkError::Stop("account request migration failed"))?;
    } verify {
        assert_eq!(Pallet::<T>::on_chain_storage_version(), StorageVersion::new(4));
    }

    // Release qualification benchmark for histories which do not need rewriting.
    #[extra]
    migrate_account_requests_v3_to_v4_bounded {
        let r in 0 .. T::MaxRequestsPerAccount::get();
        let caller = alice::<T>();
        let net_id: T::NetworkId = 0u32.into();
        benchmark_fill_account_requests::<T>(&caller, net_id, r as usize);
        assert_eq!(AccountRequests::<T>::iter_keys().count(), 1);
        let expected = AccountRequests::<T>::get(&caller);
        StorageVersion::new(3).put::<Pallet<T>>();
        let mut meter = WeightMeter::new();
    }: {
        crate::migration::AccountRequestsV3ToV4::<T>::step(None, &mut meter)
            .map_err(|_| BenchmarkError::Stop("account request migration failed"))?;
    } verify {
        assert_eq!(Pallet::<T>::on_chain_storage_version(), StorageVersion::new(4));
        assert_eq!(AccountRequests::<T>::get(&caller), expected);
    }

    // Release qualification benchmark for histories which retain only their newest entries.
    #[extra]
    migrate_account_requests_v3_to_v4_oversized {
        let r in (T::MaxRequestsPerAccount::get() + 1) .. 65_536;
        let caller = alice::<T>();
        let net_id: T::NetworkId = 0u32.into();
        benchmark_fill_account_requests::<T>(&caller, net_id, r as usize);
        assert_eq!(AccountRequests::<T>::iter_keys().count(), 1);
        let original = AccountRequests::<T>::get(&caller);
        let expected = original[original.len() - T::MaxRequestsPerAccount::get() as usize..].to_vec();
        StorageVersion::new(3).put::<Pallet<T>>();
        let mut meter = WeightMeter::new();
    }: {
        crate::migration::AccountRequestsV3ToV4::<T>::step(None, &mut meter)
            .map_err(|_| BenchmarkError::Stop("account request migration failed"))?;
    } verify {
        assert_eq!(Pallet::<T>::on_chain_storage_version(), StorageVersion::new(4));
        assert_eq!(AccountRequests::<T>::get(&caller), expected);
    }

    transfer_to_sidechain {
        let caller = alice::<T>();
        let asset_id: T::AssetId = XOR.into();
        let net_id = 0u32.into();
        let bridge_acc_id = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        Assets::<T>::mint_to(&asset_id, &bridge_acc_id, &caller, balance!(100)).unwrap();
        let initial_base_balance = Assets::<T>::free_balance(&asset_id, &caller).unwrap();
        let max_queue = T::MaxRequestsPerQueue::get() as usize;
        let max_account_requests = T::MaxRequestsPerAccount::get() as usize;
        benchmark_fill_queue::<T>(net_id, max_queue - 1, None);
        benchmark_fill_account_requests::<T>(&caller, net_id, max_account_requests);
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
        let asset_id: T::AssetId = KUSD.into();
        let net_id = 0u32.into();
        let outgoing = OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<T> {
            from: caller.clone(),
            to: EthAddress::repeat_byte(1),
            asset_id,
            amount: balance!(1),
            nonce: Zero::zero(),
            network_id: net_id,
            timepoint: Default::default(),
        }));
        let req_hash = outgoing.hash();
        Requests::<T>::insert(net_id, req_hash, outgoing);
        RequestStatuses::<T>::insert(net_id, req_hash, RequestStatus::ApprovalsReady);
        benchmark_enable_legacy_xor_decommission::<T>(net_id, req_hash);
        let max_queue = T::MaxRequestsPerQueue::get() as usize;
        let max_account_requests = T::MaxRequestsPerAccount::get() as usize;
        benchmark_fill_queue::<T>(net_id, max_queue - 1, None);
        benchmark_fill_account_requests::<T>(&caller, net_id, max_account_requests);
    }: request_from_sidechain(
        RawOrigin::Signed(caller.clone()),
        req_hash,
        IncomingMetaRequestKind::MarkAsDone.into(),
        net_id
    )
    verify {
        let load_hash = *RequestsQueue::<T>::get(net_id).last().unwrap();
        assert_last_event::<T>(Event::<T>::RequestRegistered(load_hash).into());
    }

    register_incoming_request {
        let net_id: T::NetworkId = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let alice = alice::<T>();
        let amount = 1u32.into();
        benchmark_enable_re_registered_xor::<T>(net_id, asset_id);
        T::BridgeAssetLockChecker::before_asset_lock(
            GenericNetworkId::EVMLegacy(net_id.unique_saturated_into()),
            bridge_types::types::AssetKind::Thischain,
            &asset_id,
            &amount,
        ).unwrap();
        let inc_req = IncomingRequest::Transfer(IncomingTransfer::<T> {
            from: EthAddress::from([10u8; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount,
            author: alice.clone(),
            tx_hash: H256([1u8; 32]),
            at_height: 0,
            timepoint: Default::default(),
            network_id: net_id,
            should_take_fee: false,
        });
        let req_hash = OffchainRequest::incoming(inc_req.clone()).hash();
        let max_queue = T::MaxRequestsPerQueue::get() as usize;
        let max_account_requests = T::MaxRequestsPerAccount::get() as usize;
        benchmark_fill_queue::<T>(net_id, max_queue - 1, Some(H256([1u8; 32])));
        benchmark_fill_account_requests::<T>(&alice, net_id, max_account_requests);
    }: register_incoming_request(
        RawOrigin::Signed(caller.clone()),
        inc_req
    )
    verify {
        assert_eq!(crate::RequestStatuses::<T>::get(net_id, req_hash).unwrap(), RequestStatus::Pending);
    }

    finalize_incoming_request {
        let net_id: T::NetworkId = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = XOR.into();
        let req_hash = H256([1u8; 32]);
        let alice = alice::<T>();
        let amount = 1u32.into();
        benchmark_enable_re_registered_xor::<T>(net_id, asset_id);
        T::BridgeAssetLockChecker::before_asset_lock(
            GenericNetworkId::EVMLegacy(net_id.unique_saturated_into()),
            bridge_types::types::AssetKind::Thischain,
            &asset_id,
            &amount,
        ).unwrap();
        let inc_req = IncomingRequest::Transfer(IncomingTransfer::<T> {
            from: EthAddress::from([10u8; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount,
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
        let max_queue = T::MaxRequestsPerQueue::get() as usize;
        benchmark_fill_queue::<T>(net_id, max_queue, Some(req_hash));
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
        let asset_id: T::AssetId = KUSD.into();
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
        benchmark_enable_legacy_xor_decommission::<T>(net_id, req_hash);
        let encoded_request = out_req.to_eth_abi(req_hash).map_err(|_| BenchmarkError::Stop("EthAbi encoding error"))?;
        let sk = secp256k1::SecretKey::parse(&[1; 32]).unwrap();
        let public = secp256k1::PublicKey::from_secret_key(&sk);
        let address = public_key_to_eth_address(&public);
        let public = sp_core::ecdsa::Public::from_raw(public.serialize_compressed());
        let account_id = <T as frame_system::pallet::Config>::AccountId::decode(&mut &MultiSigner::Ecdsa(public.clone()).into_account().encode()[..]).unwrap();
        Pallet::<T>::force_add_peer(RawOrigin::Root.into(), account_id.clone(), address, net_id).unwrap();
        let peers = benchmark_max_peer_set::<T>(net_id, account_id.clone());
        benchmark_seed_approvals::<T>(
            net_id,
            req_hash,
            &peers,
            &account_id,
            MAX_PEERS - 1,
        );
        RequestStatuses::<T>::insert(net_id, req_hash, RequestStatus::ApprovalsReady);
        let (signature, _) = Pallet::<T>::sign_message(encoded_request.as_raw(), &sk);
    }: approve_request(
        RawOrigin::Signed(account_id.clone()),
        public,
        req_hash,
        signature,
        net_id
    )
    verify {
        assert_eq!(RequestApprovals::<T>::get(net_id, &req_hash).len(), MAX_PEERS);
        assert_eq!(RequestApprovers::<T>::get(net_id, &req_hash).len(), MAX_PEERS);
    }

    approve_request_finalize {
        let net_id = 0u32.into();
        let caller = crate::BridgeAccount::<T>::get(&net_id).unwrap();
        let asset_id: T::AssetId = KUSD.into();
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
        benchmark_enable_legacy_xor_decommission::<T>(net_id, req_hash);
        let encoded_request = out_req.to_eth_abi(req_hash).map_err(|_| BenchmarkError::Stop("EthAbi encoding error"))?;
        let sk = secp256k1::SecretKey::parse(&[1; 32]).unwrap();
        let public = secp256k1::PublicKey::from_secret_key(&sk);
        let address = public_key_to_eth_address(&public);
        let public = sp_core::ecdsa::Public::from_raw(public.serialize_compressed());
        let account_id = <T as frame_system::pallet::Config>::AccountId::decode(&mut &MultiSigner::Ecdsa(public.clone()).into_account().encode()[..]).unwrap();
        Pallet::<T>::force_add_peer(RawOrigin::Root.into(), account_id.clone(), address, net_id).unwrap();
        let peers = benchmark_max_peer_set::<T>(net_id, account_id.clone());
        let (signature, _) = Pallet::<T>::sign_message(encoded_request.as_raw(), &sk);
        benchmark_seed_approvals::<T>(
            net_id,
            req_hash,
            &peers,
            &account_id,
            majority(MAX_PEERS),
        );
        let pending_peer: <T as frame_system::Config>::AccountId =
            account("pending_peer", 0, 0);
        PendingPeer::<T>::insert(net_id, pending_peer);
        let max_queue = T::MaxRequestsPerQueue::get() as usize;
        benchmark_fill_queue::<T>(net_id, max_queue, Some(req_hash));
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
        let max_queue = T::MaxRequestsPerQueue::get() as usize;
        benchmark_fill_queue::<T>(net_id, max_queue, Some(req_hash));
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
        macro_rules! run_benchmark {
            ($benchmark:expr) => {{
                let mut builder = ExtBuilder::default();
                builder.add_currency(0, AssetConfig::Thischain { id: KUSD.into() });
                let (mut ext, _state) = builder.build();
                ext.execute_with(|| {
                    assert_ok!($benchmark);
                });
            }};
        }

        run_benchmark!(Pallet::<Runtime>::test_benchmark_transfer_to_sidechain());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_migrate_account_requests_v3_to_v4_empty());
        run_benchmark!(
            Pallet::<Runtime>::test_benchmark_migrate_account_requests_v3_to_v4_bounded()
        );
        run_benchmark!(
            Pallet::<Runtime>::test_benchmark_migrate_account_requests_v3_to_v4_oversized()
        );
        run_benchmark!(Pallet::<Runtime>::test_benchmark_request_from_sidechain());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_register_incoming_request());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_finalize_incoming_request());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_approve_request());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_approve_request_finalize());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_abort_request());
    }
}
