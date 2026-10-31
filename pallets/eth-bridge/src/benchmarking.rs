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

fn benchmark_key<T: Config>(
    bytes: &[u8; 32],
) -> (
    sp_core::ecdsa::Public,
    <T as frame_system::Config>::AccountId,
) {
    // Both the FRAME benchmark runner and ExtBuilder install a test keystore.
    // A hex SURI uses these exact secret bytes, preserving the software fixture's
    // keys while moving expensive key generation/signing out of Wasm setup.
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut seed = Vec::with_capacity(66);
    seed.extend_from_slice(b"0x");
    for byte in bytes {
        seed.push(HEX[(byte >> 4) as usize]);
        seed.push(HEX[(byte & 15) as usize]);
    }
    let public = sp_io::crypto::ecdsa_generate(crate::KEY_TYPE, Some(seed));
    let account = <T as frame_system::Config>::AccountId::decode(
        &mut &MultiSigner::Ecdsa(public.clone()).into_account().encode()[..],
    )
    .unwrap();
    (public, account)
}

fn benchmark_peer<T: Config>(
    index: u32,
) -> (
    sp_core::ecdsa::Public,
    <T as frame_system::Config>::AccountId,
) {
    // Distinct, deterministic fixture keys, separate from the dispatch signer's [1; 32].
    let mut bytes = [0u8; 32];
    bytes[28..].copy_from_slice(&(index + 1).to_be_bytes());
    benchmark_key::<T>(&bytes)
}

fn benchmark_signature(message: &[u8], public: &sp_core::ecdsa::Public) -> SignatureParams {
    let prepared = common::eth::prepare_message(message).serialize();
    let signature = sp_io::crypto::ecdsa_sign_prehashed(crate::KEY_TYPE, public, &prepared)
        .expect("benchmark key was inserted into the test keystore");
    SignatureParams {
        r: signature.0[..32].try_into().unwrap(),
        s: signature.0[32..64].try_into().unwrap(),
        v: signature.0[64],
    }
}

fn benchmark_peer_address(message: &[u8], signature: &SignatureParams) -> EthAddress {
    let prepared = common::eth::prepare_message(message).serialize();
    let mut bytes = [0; 65];
    bytes[..32].copy_from_slice(&signature.r);
    bytes[32..64].copy_from_slice(&signature.s);
    bytes[64] = signature.v;
    let public = sp_io::crypto::secp256k1_ecdsa_recover(&bytes, &prepared)
        .ok()
        .expect("benchmark signature has a valid public key");
    EthAddress::from_slice(&sp_io::hashing::keccak_256(&public)[12..])
}

fn benchmark_max_peer_set<T: Config>(
    network_id: T::NetworkId,
    signer: <T as frame_system::Config>::AccountId,
) -> BTreeSet<<T as frame_system::Config>::AccountId> {
    let mut peers = BTreeSet::new();
    peers.insert(signer);
    let mut index = 0u32;
    while peers.len() < MAX_PEERS {
        peers.insert(benchmark_peer::<T>(index).1);
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
    message: &[u8],
) {
    let mut approvers = BTreeSet::new();
    let mut approvals = BTreeSet::new();
    for index in 0..MAX_PEERS as u32 {
        let (public, account) = benchmark_peer::<T>(index);
        if approvers.len() < count && &account != signer && peers.contains(&account) {
            let signature = benchmark_signature(message, &public);
            approvers.insert(account);
            approvals.insert(signature);
        }
    }
    assert_eq!(approvers.len(), count);
    assert_eq!(approvals.len(), count);
    RequestApprovals::<T>::insert(network_id, request_hash, approvals);
    RequestApprovers::<T>::insert(network_id, request_hash, approvers);
}

fn benchmark_approval_message<T: Config>(network_id: T::NetworkId, hash: H256) -> Vec<u8> {
    // V3 add-token/add-asset encoding with the maximum valid symbol/name is the
    // largest ordinary fixed-bounds payload (480 bytes). Migration token arrays
    // are variable length; prepare_approval_message measures that hashing cost
    // separately, while reconciliation hashes only once before recovering peers.
    BridgeSignatureVersions::<T>::insert(network_id, BridgeSignatureVersion::V3);
    let request = OutgoingAddToken::<T> {
        author: alice::<T>(),
        token_address: EthAddress::repeat_byte(42),
        symbol: "A".repeat(7),
        name: "A".repeat(33),
        decimals: common::DEFAULT_BALANCE_PRECISION,
        nonce: Default::default(),
        network_id,
        timepoint: Default::default(),
    };
    request.validate().unwrap();
    let encoded = request.to_eth_abi(hash).unwrap();
    assert_eq!(encoded.raw.len(), 480);
    encoded.raw
}

#[derive(Clone, Copy)]
enum BenchmarkApprovalKind {
    Current,
    Stale,
    Invalid,
    InvalidRecoveryId,
}

fn benchmark_reconciliation_approvals<T: Config>(
    network_id: T::NetworkId,
    hash: H256,
    message: &[u8],
    count: u32,
    kind: BenchmarkApprovalKind,
) {
    let peers = (0..MAX_PEERS as u32)
        .map(|index| benchmark_peer::<T>(index).1)
        .collect::<BTreeSet<_>>();
    assert_eq!(peers.len(), MAX_PEERS);
    // Keep both account sets maximal even at n=0, so the intercept includes the
    // full intersection/account encoding/map construction and storage proofs.
    Peers::<T>::insert(network_id, &peers);
    RequestApprovers::<T>::insert(network_id, hash, &peers);
    let approvals = (0..count)
        .map(|index| match kind {
            BenchmarkApprovalKind::Current | BenchmarkApprovalKind::Stale => {
                let key_index = match kind {
                    BenchmarkApprovalKind::Stale => index + MAX_PEERS as u32,
                    _ => index,
                };
                let (public, _) = benchmark_peer::<T>(key_index);
                benchmark_signature(message, &public)
            }
            BenchmarkApprovalKind::Invalid | BenchmarkApprovalKind::InvalidRecoveryId => {
                // Zero r with canonical v exercises a failed recovery attempt;
                // invalid v instead exercises the bounded no-recovery scan.
                let mut s = [0u8; 32];
                s[28..].copy_from_slice(&(index + 1).to_be_bytes());
                SignatureParams {
                    r: [0; 32],
                    s,
                    v: match kind {
                        BenchmarkApprovalKind::InvalidRecoveryId => 255,
                        _ => 0,
                    },
                }
            }
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(approvals.len(), count as usize);
    RequestApprovals::<T>::insert(network_id, hash, approvals);
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
        // Public hints can occupy only one quarter of the authenticated queue.
        let max_queue = (T::MaxRequestsPerQueue::get() / 4) as usize;
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

    // Calibration-only components: generated measurements are combined into
    // the admission/dispatch reconciliation allowance rather than a call weight.
    #[extra]
    reconcile_approvals {
        let n in 0 .. MAX_PEERS as u32;
        let net_id = 0u32.into();
        let hash = H256::repeat_byte(42);
        let message = benchmark_approval_message::<T>(net_id, hash);
        benchmark_reconciliation_approvals::<T>(net_id, hash, &message, n, BenchmarkApprovalKind::Current);
        let mut result = None;
    }: {
        result = Some(Pallet::<T>::current_request_approvals(net_id, hash, &message)?);
    } verify {
        let (signatures, approvers, recoveries) = result.unwrap();
        assert_eq!(signatures.len(), n as usize);
        assert_eq!(approvers.len(), n as usize);
        assert_eq!(recoveries, n);
    }

    #[extra]
    reconcile_approvals_stale {
        let n in 0 .. MAX_PEERS as u32;
        let net_id = 0u32.into();
        let hash = H256::repeat_byte(42);
        let message = benchmark_approval_message::<T>(net_id, hash);
        benchmark_reconciliation_approvals::<T>(net_id, hash, &message, n, BenchmarkApprovalKind::Stale);
        let mut result = None;
    }: {
        result = Some(Pallet::<T>::current_request_approvals(net_id, hash, &message)?);
    } verify {
        let (signatures, approvers, recoveries) = result.unwrap();
        assert!(signatures.is_empty());
        assert!(approvers.is_empty());
        assert_eq!(recoveries, n);
    }

    #[extra]
    reconcile_approvals_invalid {
        let n in 0 .. MAX_PEERS as u32;
        let net_id = 0u32.into();
        let hash = H256::repeat_byte(42);
        let message = benchmark_approval_message::<T>(net_id, hash);
        benchmark_reconciliation_approvals::<T>(net_id, hash, &message, n, BenchmarkApprovalKind::Invalid);
        let mut result = None;
    }: {
        result = Some(Pallet::<T>::current_request_approvals(net_id, hash, &message)?);
    } verify {
        let (signatures, approvers, recoveries) = result.unwrap();
        assert!(signatures.is_empty());
        assert!(approvers.is_empty());
        assert_eq!(recoveries, n);
    }

    #[extra]
    reconcile_approvals_invalid_recovery_id {
        let net_id = 0u32.into();
        let hash = H256::repeat_byte(42);
        let message = benchmark_approval_message::<T>(net_id, hash);
        benchmark_reconciliation_approvals::<T>(net_id, hash, &message, MAX_PEERS as u32, BenchmarkApprovalKind::InvalidRecoveryId);
        let mut result = None;
    }: {
        result = Some(Pallet::<T>::current_request_approvals(net_id, hash, &message)?);
    } verify {
        let (signatures, approvers, recoveries) = result.unwrap();
        assert!(signatures.is_empty());
        assert!(approvers.is_empty());
        assert_eq!(recoveries, 0);
    }

    #[extra]
    prepare_approval_message {
        // A V3 migration ABI expands each SCALE address from 20 to 32 bytes.
        // 16 MiB conservatively covers that expansion from a 7 MiB block call;
        // this benchmark envelope does not impose a new request-size policy.
        let b in 0 .. 16 * 1024 * 1024;
        let message = vec![42u8; b as usize];
        let expected = common::eth::prepare_message(&message).serialize();
        let mut result = None;
    }: {
        result = Some(common::eth::prepare_message(&message));
    } verify {
        assert_eq!(result.unwrap().serialize(), expected);
    }

    #[extra]
    encode_approval_migration {
        // The full 7 MiB block envelope is conservative for the stored address
        // vector: each SCALE address costs 20 bytes and each V3 ABI word 32.
        // Include decoding, ABI allocation/encoding, hashing, and storage proof.
        let n in 0 .. (7 * 1024 * 1024) / 20;
        let net_id = 0u32.into();
        BridgeSignatureVersions::<T>::insert(net_id, BridgeSignatureVersion::V3);
        BridgeStatuses::<T>::insert(net_id, BridgeStatus::Migrating);
        let migration = OutgoingMigrate::<T> {
            author: alice::<T>(),
            new_contract_address: EthAddress::repeat_byte(42),
            erc20_native_tokens: (0..n)
                .map(|index| EthAddress::from_low_u64_be(index as u64 + 1))
                .collect(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
            new_signature_version: BridgeSignatureVersion::V3,
        };
        migration.validate().unwrap();
        let request = OffchainRequest::outgoing(OutgoingRequest::Migrate(migration));
        let hash = request.hash();
        let encoded = request.as_outgoing().unwrap().0.to_eth_abi(hash).unwrap();
        let expected_len = 32 * n as usize + 288;
        assert_eq!(encoded.as_raw().len(), expected_len);
        let expected = common::eth::prepare_message(encoded.as_raw()).serialize();
        drop(encoded);
        Requests::<T>::insert(net_id, hash, request);
        let mut result = None;
    }: {
        let request = Requests::<T>::get(net_id, hash)
            .ok_or(BenchmarkError::Stop("missing migration request"))?;
        let (outgoing, stored_hash) = request.into_outgoing()
            .ok_or(BenchmarkError::Stop("expected outgoing migration request"))?;
        let encoded = outgoing.to_eth_abi(stored_hash)
            .map_err(|_| BenchmarkError::Stop("migration ABI encoding failed"))?;
        result = Some((
            encoded.as_raw().len(),
            common::eth::prepare_message(encoded.as_raw()).serialize(),
        ));
    } verify {
        assert_eq!(result.unwrap(), (expected_len, expected));
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
        let (public, account_id) = benchmark_key::<T>(&[1; 32]);
        let signature = benchmark_signature(encoded_request.as_raw(), &public);
        let address = benchmark_peer_address(encoded_request.as_raw(), &signature);
        Pallet::<T>::force_add_peer(RawOrigin::Root.into(), account_id.clone(), address, net_id).unwrap();
        let peers = benchmark_max_peer_set::<T>(net_id, account_id.clone());
        benchmark_seed_approvals::<T>(
            net_id,
            req_hash,
            &peers,
            &account_id,
            majority(MAX_PEERS) - 2,
            encoded_request.as_raw(),
        );
    }: approve_request(
        RawOrigin::Signed(account_id.clone()),
        public,
        req_hash,
        signature,
        net_id
    )
    verify {
        assert_eq!(RequestApprovals::<T>::get(net_id, &req_hash).len(), majority(MAX_PEERS) - 1);
        assert_eq!(RequestApprovers::<T>::get(net_id, &req_hash).len(), majority(MAX_PEERS) - 1);
        assert_eq!(RequestStatuses::<T>::get(net_id, req_hash), Some(RequestStatus::Pending));
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
        let (public, account_id) = benchmark_key::<T>(&[1; 32]);
        let signature = benchmark_signature(encoded_request.as_raw(), &public);
        let address = benchmark_peer_address(encoded_request.as_raw(), &signature);
        Pallet::<T>::force_add_peer(RawOrigin::Root.into(), account_id.clone(), address, net_id).unwrap();
        let peers = benchmark_max_peer_set::<T>(net_id, account_id.clone());
        benchmark_seed_approvals::<T>(
            net_id,
            req_hash,
            &peers,
            &account_id,
            MAX_PEERS - 1,
            encoded_request.as_raw(),
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
    fn host_benchmark_keys_preserve_software_fixture_bytes() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            let message = b"ETH benchmark fixture key compatibility";
            let mut seeds = vec![[1; 32]];
            for index in [0u32, 1, 99, 100, 199] {
                let mut seed = [0u8; 32];
                seed[28..].copy_from_slice(&(index + 1).to_be_bytes());
                seeds.push(seed);
            }
            for seed in seeds {
                let (public, account) = benchmark_key::<Runtime>(&seed);
                let secret = secp256k1::SecretKey::parse(&seed).unwrap();
                let software_public = secp256k1::PublicKey::from_secret_key(&secret);
                assert_eq!(public.0, software_public.serialize_compressed());
                assert_eq!(account, MultiSigner::Ecdsa(public.clone()).into_account());
                let signature = benchmark_signature(message, &public);
                assert_eq!(
                    signature,
                    Pallet::<Runtime>::sign_message(message, &secret).0
                );
                assert_eq!(
                    benchmark_peer_address(message, &signature),
                    common::eth::public_key_to_eth_address(&software_public),
                );
            }
        });
    }

    #[test]
    fn test_encode_approval_migration_benchmark() {
        let (mut ext, _state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Pallet::<Runtime>::test_benchmark_encode_approval_migration());
        });
    }

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
        run_benchmark!(Pallet::<Runtime>::test_benchmark_reconcile_approvals());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_reconcile_approvals_stale());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_reconcile_approvals_invalid());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_reconcile_approvals_invalid_recovery_id());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_prepare_approval_message());
        run_benchmark!(Pallet::<Runtime>::test_benchmark_abort_request());
    }
}
