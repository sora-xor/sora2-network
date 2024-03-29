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

use crate::offchain::SignatureParams;
use crate::requests::{
    IncomingRequest, IncomingRequestKind, OffchainRequest, OutgoingRequest, RequestStatus,
};
use crate::tests::mock::*;
use crate::util::majority;
use common::eth;
use frame_support::dispatch::{Pays, PostDispatchInfo};
use frame_support::{assert_ok, ensure};

use secp256k1::{PublicKey, SecretKey};
use sp_core::{ecdsa, H256};
use std::collections::BTreeSet;

mod asset;
mod cancel;
mod ethabi;
mod incoming_transfer;
pub mod mock;
mod ocw;
mod outgoing_tranfser;
mod peer;

pub(crate) type Error = crate::Error<Runtime>;
pub(crate) type Assets = assets::Pallet<Runtime>;

pub const ETH_NETWORK_ID: u32 = 0;

pub(crate) fn assert_last_event<T: crate::Config>(
    generic_event: <T as crate::Config>::RuntimeEvent,
) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let frame_system::EventRecord { event, .. } = &events.last().expect("Event expected");
    assert_eq!(event, &system_event);
}

fn get_signature_params(
    signature: &(secp256k1::Signature, secp256k1::RecoveryId),
) -> SignatureParams {
    let mut params = SignatureParams {
        r: signature.0.r.b32(),
        s: signature.0.s.b32(),
        v: signature.1.into(),
    };
    params.v += 27;
    params
}

pub fn last_event() -> Option<RuntimeEvent> {
    frame_system::Pallet::<Runtime>::events()
        .pop()
        .map(|x| x.event)
}

pub fn no_event() -> bool {
    frame_system::Pallet::<Runtime>::events().pop().is_none()
}

#[allow(clippy::result_large_err)]
pub fn approve_request(
    state: &State,
    request: OutgoingRequest<Runtime>,
    request_hash: H256,
) -> Result<(), Option<RuntimeEvent>> {
    let encoded = request.to_eth_abi(request_hash).unwrap();
    System::reset_events();
    let net_id = request.network_id();
    let mut approvals = BTreeSet::new();
    let keypairs = &state.networks[&net_id].ocw_keypairs;
    for (i, (_signer, account_id, seed)) in keypairs.iter().enumerate() {
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let msg = eth::prepare_message(encoded.as_raw());
        let sig_pair = secp256k1::sign(&msg, &secret);
        let signature_params = get_signature_params(&sig_pair);
        approvals.insert(signature_params.clone());
        let additional_sigs = if EthBridge::is_additional_signature_needed(net_id, &request) {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional_sigs;
        let current_status = crate::RequestStatuses::<Runtime>::get(net_id, request_hash).unwrap();
        ensure!(
            EthBridge::approve_request(
                RuntimeOrigin::signed(account_id.clone()),
                ecdsa::Public::from_raw(public.serialize_compressed()),
                request_hash,
                signature_params,
                net_id
            )
            .is_ok(),
            None
        );
        if current_status == RequestStatus::Pending && i + 1 == sigs_needed {
            match last_event().ok_or(None)? {
                RuntimeEvent::EthBridge(bridge_event) => match bridge_event {
                    crate::Event::ApprovalsCollected(h) => {
                        assert_eq!(h, request_hash);
                    }
                    e => {
                        assert_ne!(
                            crate::RequestsQueue::<Runtime>::get(net_id).last(),
                            Some(&request_hash)
                        );
                        return Err(Some(RuntimeEvent::EthBridge(e)));
                    }
                },
                e => panic!("Unexpected event: {:?}", e),
            }
        } else {
            assert!(no_event());
        }
        System::reset_events();
    }
    assert_ne!(
        crate::RequestsQueue::<Runtime>::get(net_id).last(),
        Some(&request_hash)
    );
    Ok(())
}

pub fn last_request(net_id: u32) -> Option<OffchainRequest<Runtime>> {
    let request_hash = crate::RequestsQueue::<Runtime>::get(net_id)
        .last()
        .cloned()?;
    crate::Requests::<Runtime>::get(net_id, request_hash)
}

pub fn last_outgoing_request(net_id: u32) -> Option<(OutgoingRequest<Runtime>, H256)> {
    let request = last_request(net_id)?;
    match request {
        OffchainRequest::Outgoing(r, hash) => Some((r, hash)),
        _ => panic!("Unexpected request type"),
    }
}

#[allow(clippy::result_large_err)]
pub fn approve_last_request(
    state: &State,
    net_id: u32,
) -> Result<(OutgoingRequest<Runtime>, H256), Option<RuntimeEvent>> {
    let (outgoing_request, hash) = last_outgoing_request(net_id).ok_or(None)?;
    approve_request(state, outgoing_request.clone(), hash)?;
    Ok((outgoing_request, hash))
}

#[allow(clippy::result_large_err)]
pub fn approve_next_request(
    state: &State,
    net_id: u32,
) -> Result<(OutgoingRequest<Runtime>, H256), Option<RuntimeEvent>> {
    let request_hash = crate::RequestsQueue::<Runtime>::get(net_id).remove(0);
    let (outgoing_request, hash) = crate::Requests::<Runtime>::get(net_id, request_hash)
        .ok_or(None)?
        .into_outgoing()
        .unwrap();
    approve_request(state, outgoing_request.clone(), hash)?;
    Ok((outgoing_request, hash))
}

#[allow(clippy::result_large_err)]
pub fn request_incoming(
    account_id: AccountId,
    tx_hash: H256,
    kind: IncomingRequestKind,
    net_id: u32,
) -> Result<H256, RuntimeEvent> {
    assert_ok!(EthBridge::request_from_sidechain(
        RuntimeOrigin::signed(account_id),
        tx_hash,
        kind,
        net_id
    ));
    let last_request: OffchainRequest<Runtime> = last_request(net_id).unwrap();
    match last_request {
        OffchainRequest::LoadIncoming(..) => (),
        _ => panic!("Invalid off-chain request"),
    }
    let hash = last_request.hash();
    assert_eq!(
        crate::RequestStatuses::<Runtime>::get(net_id, hash).unwrap(),
        RequestStatus::Pending
    );
    Ok(hash)
}

#[allow(clippy::result_large_err)]
pub fn assert_incoming_request_done(
    state: &State,
    incoming_request: IncomingRequest<Runtime>,
) -> Result<(), Option<RuntimeEvent>> {
    let net_id = incoming_request.network_id();
    let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
    let sidechain_req_hash = incoming_request.hash();
    assert_eq!(
        crate::RequestsQueue::<Runtime>::get(net_id)
            .last()
            .unwrap()
            .0,
        sidechain_req_hash.0
    );
    assert_ok!(EthBridge::register_incoming_request(
        RuntimeOrigin::signed(bridge_acc_id.clone()),
        incoming_request.clone(),
    ));
    let req_hash = crate::LoadToIncomingRequestHash::<Runtime>::get(net_id, sidechain_req_hash);
    assert_ne!(
        crate::RequestsQueue::<Runtime>::get(net_id)
            .last()
            .map(|x| x.0),
        Some(sidechain_req_hash.0)
    );
    assert!(crate::RequestsQueue::<Runtime>::get(net_id).contains(&req_hash));
    assert_eq!(
        *crate::Requests::get(net_id, req_hash)
            .unwrap()
            .as_incoming()
            .unwrap()
            .0,
        incoming_request
    );
    assert_ok!(EthBridge::finalize_incoming_request(
        RuntimeOrigin::signed(bridge_acc_id),
        req_hash,
        net_id,
    ));
    assert_eq!(
        crate::RequestStatuses::<Runtime>::get(net_id, req_hash).unwrap(),
        RequestStatus::Done
    );
    assert!(!crate::RequestsQueue::<Runtime>::get(net_id).contains(&req_hash));
    Ok(())
}

#[allow(clippy::result_large_err)]
pub fn assert_incoming_request_registration_failed(
    state: &State,
    incoming_request: IncomingRequest<Runtime>,
    error: crate::Error<Runtime>,
) -> Result<(), RuntimeEvent> {
    let net_id = incoming_request.network_id();
    let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
    assert_eq!(
        crate::RequestsQueue::<Runtime>::get(net_id)
            .last()
            .unwrap()
            .0,
        incoming_request.hash().0
    );
    assert_ok!(
        EthBridge::register_incoming_request(
            RuntimeOrigin::signed(bridge_acc_id),
            incoming_request.clone(),
        ),
        PostDispatchInfo {
            pays_fee: Pays::No,
            actual_weight: None
        }
    );
    let req_hash =
        crate::LoadToIncomingRequestHash::<Runtime>::get(net_id, incoming_request.hash());
    assert_last_event::<Runtime>(
        crate::Event::RegisterRequestFailed(req_hash, error.into()).into(),
    );
    Ok(())
}
