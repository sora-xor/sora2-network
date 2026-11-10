use super::mock::*;
use super::{last_outgoing_request, Assets, Error, ETH_NETWORK_ID};
use crate::offchain::SignatureParams;
use crate::requests::{
    IncomingTransactionRequestKind, LoadIncomingRequest, LoadIncomingTransactionRequest,
    OffchainRequest, OutgoingRequest, RequestStatus,
};
use crate::WeightInfo;
use common::{eth, XOR};
use frame_support::dispatch::{extract_actual_weight, GetDispatchInfo, Pays};
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_ok};
use sp_core::{ecdsa, sr25519, H256};
use sp_runtime::traits::Dispatchable;

type BridgeWeights = <Runtime as crate::Config>::WeightInfo;

fn signature(
    request: &OutgoingRequest<Runtime>,
    hash: H256,
    seed: &[u8; 32],
) -> (ecdsa::Public, SignatureParams) {
    let secret = secp256k1::SecretKey::parse(seed).unwrap();
    let encoded = request.to_eth_abi(hash).unwrap();
    let public = secp256k1::PublicKey::from_secret_key(&secret);
    let signed = secp256k1::sign(&eth::prepare_message(encoded.as_raw()), &secret);
    (
        ecdsa::Public::from_raw(public.serialize_compressed()),
        super::get_signature_params(&signed),
    )
}

fn pending_transfer() -> (OutgoingRequest<Runtime>, H256) {
    let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
    Assets::mint_to(&XOR.into(), &alice, &alice, 1000).unwrap();
    assert_ok!(EthBridge::transfer_to_sidechain(
        RuntimeOrigin::signed(alice),
        XOR,
        crate::EthAddress::repeat_byte(42),
        100,
        ETH_NETWORK_ID,
    ));
    let (request, hash) = last_outgoing_request(ETH_NETWORK_ID).unwrap();
    assert_eq!(
        crate::RequestStatuses::<Runtime>::get(ETH_NETWORK_ID, hash),
        Some(RequestStatus::Pending)
    );
    (request, hash)
}

fn approve(public: &ecdsa::Public, hash: H256, proof: &SignatureParams) -> crate::Call<Runtime> {
    crate::Call::approve_request {
        ocw_public: public.clone(),
        hash,
        signature_params: proof.clone(),
        network_id: ETH_NETWORK_ID,
    }
}

fn finalize(hash: H256) -> crate::Call<Runtime> {
    crate::Call::finalize_approved_request {
        network_id: ETH_NETWORK_ID,
        hash,
    }
}

/// Dispatch as the executive does and return the weight CheckWeight retains
/// for the failed call, and the call's declaration.
fn rejected(call: crate::Call<Runtime>, who: &AccountId, expected: Error) -> (Weight, Weight) {
    let info = call.get_dispatch_info();
    let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
    let result = RuntimeCall::EthBridge(call).dispatch(RuntimeOrigin::signed(who.clone()));
    assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    let retained = extract_actual_weight(&result, &info);
    let error = result.unwrap_err();
    assert_eq!(error.error, expected.into());
    assert_eq!(error.post_info.pays_fee, Pays::Yes);
    assert_eq!(error.post_info.actual_weight, Some(retained));
    // Neither the signature recovery allowance nor the message envelope stays.
    assert!(retained.all_lte(info.call_weight));
    assert!(
        retained.ref_time()
            < EthBridge::approval_reconciliation_weight_for(crate::MAX_PEERS as u32).ref_time()
    );
    (retained, info.call_weight)
}

#[test]
fn non_peer_approval_is_rejected_by_pool_validation() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let (request, hash) = pending_transfer();
        let (_, peer, seed) = &state.networks[&ETH_NETWORK_ID].ocw_keypairs[0];
        let (public, proof) = signature(&request, hash, seed);
        let outsider = AccountId::new([0x71; 32]);
        let absent = H256::repeat_byte(0x72);
        for target in [hash, absent] {
            // Previously Ok(false): admitted as an ordinary fee-paying call.
            let call = approve(&public, target, &proof);
            assert_err!(
                EthBridge::validate_peer_protocol_call(&outsider, &call),
                Error::Forbidden
            );
            sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
            assert_err!(
                EthBridge::validate_peer_protocol_call(&outsider, &call),
                Error::Forbidden
            );
            sp_io::storage::clear(crate::recovery::HALT_KEY);
        }
        assert_err!(
            EthBridge::validate_peer_protocol_call(peer, &approve(&public, absent, &proof)),
            Error::UnknownRequest
        );
        assert_eq!(
            EthBridge::validate_peer_protocol_call(peer, &approve(&public, hash, &proof)),
            Ok(true)
        );
    });
}

#[test]
fn finalization_is_admitted_only_for_a_request_with_a_recorded_quorum() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let anyone = AccountId::new([0x73; 32]);
        let absent = H256::repeat_byte(0x74);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(absent)),
            Error::UnknownRequest
        );
        // A public load hint occupying the hash is not an outgoing request.
        let hint = OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                anyone.clone(),
                absent,
                Default::default(),
                IncomingTransactionRequestKind::Transfer,
                net,
            ),
        ));
        crate::Requests::<Runtime>::insert(net, absent, hint);
        crate::RequestStatuses::<Runtime>::insert(net, absent, RequestStatus::Pending);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(absent)),
            Error::UnknownRequest
        );

        let (request, hash) = pending_transfer();
        assert_err!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(hash)),
            Error::InsufficientCurrentApprovals
        );
        // A pending peer requires all four peers, so three approvals stay pending.
        crate::PendingPeer::<Runtime>::insert(net, AccountId::new([0x75; 32]));
        for (_, peer, seed) in state.networks[&net].ocw_keypairs.iter().take(3) {
            let (public, proof) = signature(&request, hash, seed);
            assert_ok!(EthBridge::approve_request(
                RuntimeOrigin::signed(peer.clone()),
                public,
                hash,
                proof,
                net
            ));
        }
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::Pending)
        );
        assert_err!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(hash)),
            Error::InsufficientCurrentApprovals
        );
        crate::PendingPeer::<Runtime>::remove(net);
        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        assert_err!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(hash)),
            Error::RecoveryHalted
        );
        sp_io::storage::clear(crate::recovery::HALT_KEY);

        // Anyone may finalize the existing quorum; the call remains fee-paying.
        assert_eq!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(hash)),
            Ok(false)
        );
        let declared = finalize(hash).get_dispatch_info().call_weight;
        let post =
            EthBridge::finalize_approved_request(RuntimeOrigin::signed(anyone.clone()), net, hash)
                .unwrap();
        assert_eq!(post.pays_fee, Pays::Yes);
        assert!(post.actual_weight.unwrap().all_lte(declared));
        assert_eq!(
            crate::RequestStatuses::<Runtime>::get(net, hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert_err!(
            EthBridge::validate_peer_protocol_call(&anyone, &finalize(hash)),
            Error::RequestIsNotReady
        );
    });
}

#[test]
fn early_approval_and_finalization_failures_report_small_actual_weight() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let (request, hash) = pending_transfer();
        let (_, peer, seed) = &state.networks[&net].ocw_keypairs[0];
        let (public, proof) = signature(&request, hash, seed);
        let outsider = AccountId::new([0x76; 32]);
        let absent = H256::repeat_byte(0x77);

        // The reported attack: an absent hash declares the full envelope.
        let (retained, declared) = rejected(
            approve(&public, absent, &proof),
            &outsider,
            Error::Forbidden,
        );
        assert!(retained.ref_time().saturating_mul(50) < declared.ref_time());
        let (retained, declared) = rejected(finalize(absent), &outsider, Error::UnknownRequest);
        assert!(retained.ref_time().saturating_mul(20) < declared.ref_time());
        rejected(
            approve(&public, absent, &proof),
            peer,
            Error::UnknownRequest,
        );
        rejected(approve(&public, hash, &proof), &outsider, Error::Forbidden);

        for status in [RequestStatus::Frozen, RequestStatus::ApprovalsReady] {
            crate::RequestStatuses::<Runtime>::insert(net, hash, status);
            rejected(
                approve(&public, hash, &proof),
                peer,
                Error::RequestIsNotReady,
            );
            rejected(finalize(hash), &outsider, Error::RequestIsNotReady);
        }
        crate::RequestStatuses::<Runtime>::insert(net, hash, RequestStatus::Pending);

        sp_io::storage::set(crate::recovery::HALT_KEY, &[1]);
        for target in [hash, absent] {
            rejected(
                approve(&public, target, &proof),
                peer,
                Error::RecoveryHalted,
            );
            rejected(
                approve(&public, target, &proof),
                &outsider,
                Error::Forbidden,
            );
            rejected(finalize(target), &outsider, Error::RecoveryHalted);
        }
        sp_io::storage::clear(crate::recovery::HALT_KEY);
        assert!(crate::RequestApprovers::<Runtime>::get(net, hash).is_empty());
    });
}

#[test]
fn peer_approvals_of_a_known_request_are_admitted_and_finalize_as_before() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net = ETH_NETWORK_ID;
        let (request, hash) = pending_transfer();
        let message_bytes = request.to_eth_abi(hash).unwrap().as_raw().len() as u64;
        let peers = &state.networks[&net].ocw_keypairs;
        let quorum = crate::util::majority(peers.len());
        for (index, (_, peer, seed)) in peers.iter().take(quorum).enumerate() {
            let (public, proof) = signature(&request, hash, seed);
            let declared = approve(&public, hash, &proof)
                .get_dispatch_info()
                .call_weight;
            assert_eq!(
                EthBridge::validate_peer_protocol_call(peer, &approve(&public, hash, &proof)),
                Ok(true)
            );
            let post = EthBridge::approve_request(
                RuntimeOrigin::signed(peer.clone()),
                public,
                hash,
                proof,
                net,
            )
            .unwrap();
            let finalized = index + 1 == quorum;
            let dispatch = if finalized {
                BridgeWeights::approve_request_finalize()
            } else {
                BridgeWeights::approve_request()
            };
            assert_eq!(post.pays_fee, Pays::Yes);
            assert_eq!(
                post.actual_weight,
                Some(EthBridge::approval_actual_weight(
                    dispatch,
                    index as u32,
                    message_bytes
                ))
            );
            assert!(post.actual_weight.unwrap().all_lte(declared));
            assert_eq!(
                crate::RequestStatuses::<Runtime>::get(net, hash),
                Some(if finalized {
                    RequestStatus::ApprovalsReady
                } else {
                    RequestStatus::Pending
                })
            );
        }
        assert_eq!(
            crate::RequestApprovers::<Runtime>::get(net, hash).len(),
            quorum
        );
        assert!(!crate::RequestsQueue::<Runtime>::get(net).contains(&hash));
        assert_eq!(
            super::last_event(),
            Some(RuntimeEvent::EthBridge(crate::Event::ApprovalsCollected(
                hash
            )))
        );
    });
}
