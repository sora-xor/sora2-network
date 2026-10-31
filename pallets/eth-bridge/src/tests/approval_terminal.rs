use super::mock::*;
use super::{last_outgoing_request, Assets, Error, ETH_NETWORK_ID};
use crate::offchain::SignatureParams;
use crate::requests::{OutgoingRequest, RequestStatus};
use common::{eth, XOR};
use frame_support::{assert_noop, assert_ok};
use sp_core::{ecdsa, sr25519, H256};
use std::collections::BTreeSet;

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

#[test]
fn ready_legacy_and_current_approvals_are_immutable_under_late_peer_submission() {
    for legacy_without_authors in [true, false] {
        let (mut ext, state) = ExtBuilder::default().build();
        ext.execute_with(|| {
            let net = ETH_NETWORK_ID;
            let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
            Assets::mint_to(&XOR.into(), &alice, &alice, 1000).unwrap();
            assert_ok!(EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(alice),
                XOR,
                crate::EthAddress::repeat_byte(42),
                100,
                net,
            ));
            let (request, hash) = last_outgoing_request(net).unwrap();
            let peers = &state.networks[&net].ocw_keypairs;
            // Keep an externally usable quorum and a distinct late current peer.
            let mut retained = BTreeSet::new();
            let mut authors = BTreeSet::new();
            for (_, account, seed) in peers.iter().take(3) {
                let (_, proof) = signature(&request, hash, seed);
                let mut proof = proof.canonical_recovery_id().unwrap();
                if legacy_without_authors {
                    // Historical representation must remain byte-for-byte intact.
                    proof.v += 27;
                }
                retained.insert(proof);
                authors.insert(account.clone());
            }
            crate::RequestApprovals::<Runtime>::insert(net, hash, &retained);
            if legacy_without_authors {
                crate::RequestApprovers::<Runtime>::remove(net, hash);
            } else {
                crate::RequestApprovers::<Runtime>::insert(net, hash, &authors);
            }
            crate::RequestStatuses::<Runtime>::insert(net, hash, RequestStatus::ApprovalsReady);
            crate::RequestsQueue::<Runtime>::mutate(net, |queue| queue.retain(|id| *id != hash));
            let rpc_before = EthBridge::get_approved_requests(&[hash], Some(net)).unwrap();
            assert_eq!(rpc_before.len(), 1);
            let exported: Vec<_> = retained
                .iter()
                .cloned()
                .map(|proof| proof.canonical_recovery_id().unwrap())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            assert_eq!(rpc_before[0].1, exported);
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            let (_, late_author, seed) = &peers[3];
            let (public, proof) = signature(&request, hash, seed);
            assert_noop!(
                EthBridge::validate_outgoing_approval(&public, &hash, &proof, late_author, net),
                Error::RequestIsNotReady
            );
            let call = crate::Call::<Runtime>::approve_request {
                ocw_public: public.clone(),
                hash,
                signature_params: proof.clone(),
                network_id: net,
            };
            // This is the runtime fee-exemption validation entry point.
            assert_noop!(
                EthBridge::validate_peer_protocol_call(late_author, &call),
                Error::RequestIsNotReady
            );
            assert_noop!(
                EthBridge::inner_approve_request(
                    public.clone(),
                    hash,
                    proof.clone(),
                    late_author.clone(),
                    net,
                ),
                Error::RequestIsNotReady
            );
            assert_noop!(
                EthBridge::approve_request(
                    RuntimeOrigin::signed(late_author.clone()),
                    public,
                    hash,
                    proof,
                    net,
                ),
                Error::RequestIsNotReady
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            assert_eq!(crate::RequestApprovals::<Runtime>::get(net, hash), retained);
            assert_eq!(
                crate::RequestApprovers::<Runtime>::get(net, hash),
                if legacy_without_authors {
                    BTreeSet::new()
                } else {
                    authors
                }
            );
            assert_eq!(
                crate::RequestStatuses::<Runtime>::get(net, hash),
                Some(RequestStatus::ApprovalsReady)
            );
            assert_eq!(
                EthBridge::get_approved_requests(&[hash], Some(net)).unwrap(),
                rpc_before
            );
        });
    }
}
