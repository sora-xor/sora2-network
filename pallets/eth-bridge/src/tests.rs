use crate::{
    majority, mock::*, types::Address, OffchainRequest, OutgoingRequest, OutgoingTransfer,
    RequestStatus, SignatureParams,
};
use codec::{Decode, Encode};
use common::AssetId;
use frame_support::{
    assert_err, assert_ok,
    sp_runtime::app_crypto::sp_core::{ecdsa, sr25519, Public},
    StorageMap, StorageValue,
};
use rustc_hex::FromHex;
use secp256k1::{PublicKey, SecretKey};
use sp_std::{collections::btree_set::BTreeSet, prelude::*};
use std::str::FromStr;

fn get_signature_params(signature: &ecdsa::Signature) -> SignatureParams {
    let encoded = signature.encode();
    let mut params = SignatureParams::decode(&mut &encoded[..]).expect("Wrong signature format");
    params.v += 27;
    params
}

fn last_event() -> Event {
    frame_system::Module::<Test>::events()
        .pop()
        .expect("Event expected")
        .event
}

fn no_event() -> bool {
    frame_system::Module::<Test>::events().pop().is_none()
}

fn approve_request(state: &State, request: OutgoingRequest<Test>) -> Result<(), Event> {
    let request_hash = request.hash();
    let encoded = request.to_eth_abi(request_hash).unwrap();
    System::reset_events();
    let mut approves = BTreeSet::new();

    for (i, (_signer, account_id, seed)) in state.ocw_keypairs.iter().enumerate() {
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let msg = EthBridge::prepare_message(encoded.as_raw());
        let sig_pair = secp256k1::sign(&msg, &secret);
        let signature = sig_pair.into();
        let signature_params = get_signature_params(&signature);
        approves.insert(signature_params.clone());
        let additional_sigs = if crate::PendingAuthority::<Test>::get().is_some() {
            1
        } else {
            0
        };
        let sigs_needed = majority(state.ocw_keypairs.len()) + additional_sigs;
        let current_status = crate::RequestStatuses::get(&request.hash()).unwrap();
        assert_ok!(EthBridge::approve_request(
            Origin::signed(account_id.clone()),
            ecdsa::Public::from_slice(&public.serialize_compressed()),
            request.clone(),
            encoded.clone(),
            signature_params
        ));
        if current_status == RequestStatus::Pending && i + 1 == sigs_needed {
            match last_event() {
                Event::eth_bridge(bridge_event) => match bridge_event {
                    crate::RawEvent::ApprovesCollected(e, a) => {
                        assert_eq!(e, encoded);
                        assert_eq!(a, approves);
                    }
                    e => return Err(Event::eth_bridge(e)),
                },
                e => panic!("Unexpected event: {:?}", e),
            }
        } else {
            assert!(no_event());
        }
        System::reset_events();
    }
    Ok(())
}

fn approve_last_request(state: &State) -> Result<(), Event> {
    let request = crate::RequestsQueue::<Test>::mutate(|v| v.pop().unwrap());
    let outgoing_request = match request {
        OffchainRequest::Outgoing(r, _) => r,
    };
    approve_request(state, outgoing_request)
}

#[test]
fn should_transfer() {
    let _ = env_logger::try_init();

    let (mut ext, state, _, _) = ExtBuilder::new();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
        ));
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            99900u32.into()
        );
        approve_last_request(&state).expect("request wasn't approved");
    });
}

#[test]
fn should_not_transfer() {
    let _ = env_logger::try_init();

    let (mut ext, _, _, _) = ExtBuilder::new();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_err!(
            EthBridge::transfer_to_sidechain(
                Origin::signed(alice.clone()),
                AssetId::KSM.into(),
                Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
            ),
            crate::Error::<Test>::UnsupportedToken
        );
        assert!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_000_000_u32.into(),
        )
        .is_err(),);
    });
}

#[test]
fn should_register_outgoing_transfer() {
    let _ = env_logger::try_init();

    let (mut ext, _state, _pool_state, _oc_state) = ExtBuilder::new();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from([1; 20]),
            100u32.into(),
        ));
        let outgoing_transfer = OutgoingTransfer::<Test> {
            from: alice.clone(),
            to: Address::from([1; 20]),
            asset_id: AssetId::XOR.into(),
            amount: 100_u32.into(),
            nonce: 2,
        };
        let last_request = crate::RequestsQueue::get().pop().unwrap();
        match last_request {
            OffchainRequest::Outgoing(OutgoingRequest::OutgoingTransfer(r), _) => {
                assert_eq!(r, outgoing_transfer)
            }
        }
    });
}

#[test]
fn should_convert_to_eth_address() {
    let _ = env_logger::try_init();
    let (mut ext, _, _, _) = ExtBuilder::new();
    ext.execute_with(|| {
        let account_id = PublicKey::parse_slice(
            &"03b27380932f3750c416ba38c967c4e63a8c9778bac4d28a520e499525f170ae85"
                .from_hex::<Vec<u8>>()
                .unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(
            crate::public_key_to_eth_address(&account_id),
            Address::from_str("8589c3814C3c1d4d2f5C21B74c6A00fb15E5166E").unwrap()
        );
    });
}
