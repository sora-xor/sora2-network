use crate::{
    majority,
    mock::*,
    types::{Address, Bytes, Log},
    AssetKind, ContractEvent, IncomingAsset, IncomingRequest, IncomingRequestKind, OffchainRequest,
    OutgoingRequest, OutgoingTransfer, RequestStatus, SignatureParams,
};
use codec::{Decode, Encode};
use common::{balance::Balance, AssetId, AssetId32, AssetSymbol};
use ethereum_types::H256;
use frame_support::{
    assert_err, assert_ok,
    sp_runtime::app_crypto::sp_core::{self, crypto::AccountId32, ecdsa, sr25519, Public},
    StorageMap, StorageValue,
};
use hex_literal::hex;
use rustc_hex::FromHex;
use secp256k1::{PublicKey, SecretKey};
use serde_json::Value;
use sp_std::{collections::btree_set::BTreeSet, prelude::*};
use std::str::FromStr;

fn get_signature_params(signature: &ecdsa::Signature) -> SignatureParams {
    let encoded = signature.encode();
    let mut params = SignatureParams::decode(&mut &encoded[..]).expect("Wrong signature format");
    params.v += 27;
    params
}

#[test]
fn should_parse_eth_string() {
    let bytes: Bytes = serde_json::from_value(Value::String("0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000003584f520000000000000000000000000000000000000000000000000000000000".into())).unwrap();
    let s = crate::parse_eth_string(&bytes.0).unwrap();
    assert_eq!(s, "XOR");
}

#[test]
fn parses_event() {
    let (mut ext, _, _, _) = ExtBuilder::new();
    ext.execute_with(|| {
        let mut log = Log::default();
        log.topics = vec![H256(hex!("85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8"))];
        log.data = Bytes(hex!("1111111111111111111111111111111111111111111111111111111111111111000000000000000000000000000000000000000000000000000000000000002a00000000000000000000000022222222222222222222222222222222222222220200040000000000000000000000000000000000000000000000000000000011").to_vec());
        assert_eq!(
            EthBridge::parse_main_event(&[log]).unwrap(),
            ContractEvent::Deposit(
                AccountId32::from(hex!("1111111111111111111111111111111111111111111111111111111111111111")),
                Balance::from(42u128),
                Address::from(&hex!("2222222222222222222222222222222222222222")),
                H256(hex!("0200040000000000000000000000000000000000000000000000000000000011"))
            )
        )
    });
}

#[test]
fn parses_deposit_pswap() {
    let (mut ext, _, _, _) = ExtBuilder::new();
    ext.execute_with(|| {
        let mut log = Log::default();
        log.topics = vec![H256(hex!(
            "4eb3aea69bf61684354f60a43d355c3026751ddd0ea4e1f5afc1274b96c65505"
        ))];
        log.data = Bytes(
            hex!("00aaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaa").to_vec(),
        );
        assert_eq!(
            EthBridge::parse_main_event(&[log]).unwrap(),
            ContractEvent::ClaimPswap(AccountId32::from(hex!(
                "00aaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaa"
            )),)
        )
    });
}

#[test]
fn should_success_claim_pswap() {
    let _ = env_logger::try_init();

    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::ClaimPswap).unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
        });
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_incoming_request_ready(&state, request.clone(), tx_hash).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            300u32.into()
        );
    });
}

#[test]
fn should_fail_claim_pswap_already_claimed() {
    let _ = env_logger::try_init();

    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::ClaimPswap).unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
        });
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_incoming_request_ready(&state, request.clone(), tx_hash).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            300u32.into()
        );
        let tx_hash = H256::from_slice(&[2u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::ClaimPswap).unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
        });
        // Same eth_address
        assert_incoming_request_failed(&state, request.clone(), tx_hash).unwrap();
    });
}

#[test]
fn should_fail_claim_pswap_account_not_found() {
    let _ = env_logger::try_init();

    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::ClaimPswap).unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("32fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
        });
        assert_ok!(EthBridge::register_incoming_request(
            Origin::signed(state.bridge_account.clone()),
            request.clone()
        ));
        assert!(crate::PendingIncomingRequests::get().contains(&tx_hash));
        assert_eq!(crate::IncomingRequests::get(&tx_hash).unwrap(), request);
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_ok!(EthBridge::finalize_incoming_request(
            Origin::signed(state.bridge_account),
            Err((tx_hash, crate::Error::<Test>::AccountNotFound.into()))
        ));
        assert_eq!(
            crate::RequestStatuses::get(&tx_hash).unwrap(),
            RequestStatus::Failed
        );
        assert!(crate::PendingIncomingRequests::get().is_empty());
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
    });
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
    assert_eq!(
        crate::RequestsQueue::<Test>::get().last().unwrap().hash(),
        request.hash()
    );
    let mut approves = BTreeSet::new();

    for (i, (_signer, account_id, seed)) in state.ocw_keypairs.iter().enumerate() {
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let msg = EthBridge::prepare_message(encoded.as_raw());
        let sig_pair = secp256k1::sign(&msg, &secret);
        let signature = sig_pair.into();
        let signature_params = get_signature_params(&signature);
        approves.insert(signature_params.clone());
        let additional_sigs = if crate::PendingPeer::<Test>::get().is_some() {
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
                    e => {
                        assert_ne!(
                            crate::RequestsQueue::<Test>::get().last().map(|x| x.hash()),
                            Some(request.hash())
                        );
                        return Err(Event::eth_bridge(e));
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
        crate::RequestsQueue::<Test>::get().last().map(|x| x.hash()),
        Some(request.hash())
    );
    Ok(())
}

fn approve_last_request(state: &State) -> Result<(), Event> {
    let request = crate::RequestsQueue::<Test>::get().pop().unwrap();
    let outgoing_request = match request {
        OffchainRequest::Outgoing(r, _) => r,
        _ => panic!("Unexpected request type"),
    };
    approve_request(state, outgoing_request)
}

fn request_incoming(
    account_id: AccountId,
    tx_hash: H256,
    kind: IncomingRequestKind,
) -> Result<sp_core::H256, Event> {
    assert_ok!(EthBridge::request_from_sidechain(
        Origin::signed(account_id),
        tx_hash,
        kind
    ));
    let last_request: OffchainRequest<Test> = crate::RequestsQueue::get().pop().unwrap();
    match last_request {
        OffchainRequest::Incoming(_, h, _) => assert_eq!(h, tx_hash),
        _ => panic!("Invalid off-chain request"),
    }
    let tx_hash = sp_core::H256(tx_hash.0);
    assert_eq!(
        crate::RequestStatuses::get(&tx_hash).unwrap(),
        RequestStatus::Pending
    );
    Ok(tx_hash)
}

fn assert_incoming_request_ready(
    state: &State,
    incoming_request: IncomingRequest<Test>,
    tx_hash: sp_core::H256,
) -> Result<(), Event> {
    assert_eq!(
        crate::RequestsQueue::<Test>::get().last().unwrap().hash().0,
        incoming_request.tx_hash().0
    );
    assert_ok!(EthBridge::register_incoming_request(
        Origin::signed(state.bridge_account.clone()),
        incoming_request.clone()
    ));
    assert_ne!(
        crate::RequestsQueue::<Test>::get()
            .last()
            .map(|x| x.hash().0),
        Some(incoming_request.tx_hash().0)
    );
    assert!(crate::PendingIncomingRequests::get().contains(&tx_hash));
    assert_eq!(
        crate::IncomingRequests::get(&tx_hash).unwrap(),
        incoming_request
    );
    assert_ok!(EthBridge::finalize_incoming_request(
        Origin::signed(state.bridge_account.clone()),
        Ok(incoming_request)
    ));
    assert_eq!(
        crate::RequestStatuses::get(&tx_hash).unwrap(),
        RequestStatus::Ready
    );
    assert!(crate::PendingIncomingRequests::get().is_empty());
    Ok(())
}

fn assert_incoming_request_failed(
    state: &State,
    incoming_request: IncomingRequest<Test>,
    tx_hash: sp_core::H256,
) -> Result<(), Event> {
    assert_eq!(
        crate::RequestsQueue::<Test>::get().last().unwrap().hash().0,
        incoming_request.tx_hash().0
    );
    assert_ok!(EthBridge::register_incoming_request(
        Origin::signed(state.bridge_account.clone()),
        incoming_request.clone()
    ));
    assert_ne!(
        crate::RequestsQueue::<Test>::get()
            .last()
            .map(|x| x.hash().0),
        Some(incoming_request.tx_hash().0)
    );
    assert!(crate::PendingIncomingRequests::get().contains(&tx_hash));
    assert_eq!(
        crate::IncomingRequests::get(&tx_hash).unwrap(),
        incoming_request
    );
    assert_ok!(EthBridge::finalize_incoming_request(
        Origin::signed(state.bridge_account.clone()),
        Ok(incoming_request)
    ));
    assert_eq!(
        crate::RequestStatuses::get(&tx_hash).unwrap(),
        RequestStatus::Failed
    );
    assert!(crate::PendingIncomingRequests::get().is_empty());
    Ok(())
}

#[test]
fn should_transfer() {
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
fn should_mint_and_burn_sidechain_asset() {
    let (mut ext, state, _, _) = ExtBuilder::new();

    #[track_caller]
    fn check_invariant(asset_id: &AssetId32<AssetId>, val: u32) {
        assert_eq!(
            assets::Module::<Test>::total_issuance(asset_id).unwrap(),
            val.into()
        );
    }

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = Address::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        EthBridge::register_sidechain_asset(token_address, 18, AssetSymbol(b"TEST".to_vec()))
            .unwrap();
        let (asset_id, asset_kind) =
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address)
                .unwrap()
                .unwrap();
        assert_eq!(asset_kind, AssetKind::Sidechain);
        check_invariant(&asset_id, 0);
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::Transfer).unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            incoming_asset: IncomingAsset::Loaded(asset_id, asset_kind),
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
        });
        assert_incoming_request_ready(&state, incoming_transfer.clone(), tx_hash).unwrap();
        check_invariant(&asset_id, 100);
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            asset_id,
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
        ));
        approve_last_request(&state).expect("request wasn't approved");
        check_invariant(&asset_id, 0);
    });
}

#[test]
fn should_not_burn_or_mint_sidechain_owned_asset() {
    let (mut ext, state, _, _) = ExtBuilder::new();

    fn check_invariant() {
        assert_eq!(
            assets::Module::<Test>::total_issuance(&AssetId::XOR.into()).unwrap(),
            450000u32.into()
        );
    }

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_eq!(
            EthBridge::registered_asset(AssetId32::from(AssetId::XOR)).unwrap(),
            AssetKind::SidechainOwned
        );
        check_invariant();
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::Transfer).unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            incoming_asset: IncomingAsset::Loaded(AssetId::XOR.into(), AssetKind::SidechainOwned),
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
        });
        assert_incoming_request_ready(&state, incoming_transfer.clone(), tx_hash).unwrap();
        check_invariant();
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
        ));
        approve_last_request(&state).expect("request wasn't approved");
        check_invariant();
    });
}

#[test]
fn should_not_transfer() {
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
            nonce: 4,
        };
        let last_request = crate::RequestsQueue::get().pop().unwrap();
        match last_request {
            OffchainRequest::Outgoing(OutgoingRequest::OutgoingTransfer(r), _) => {
                assert_eq!(r, outgoing_transfer)
            }
            _ => panic!("Invalid off-chain request"),
        }
    });
}

#[test]
fn should_not_accept_duplicated_incoming_transfer() {
    let (mut ext, _state, _pool_state, _oc_state) = ExtBuilder::new();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(EthBridge::request_from_sidechain(
            Origin::signed(alice.clone()),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer
        ));
        assert_err!(
            EthBridge::request_from_sidechain(
                Origin::signed(alice.clone()),
                H256::from_slice(&[1u8; 32]),
                IncomingRequestKind::Transfer
            ),
            crate::Error::<Test>::DuplicatedRequest
        );
    });
}

#[test]
fn should_not_accept_approved_incoming_transfer() {
    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::Transfer).unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            incoming_asset: IncomingAsset::Loaded(AssetId::XOR.into(), AssetKind::Thischain),
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
        });
        assert_incoming_request_ready(&state, incoming_transfer.clone(), tx_hash).unwrap();
        assert_err!(
            EthBridge::request_from_sidechain(
                Origin::signed(alice.clone()),
                H256::from_slice(&[1u8; 32]),
                IncomingRequestKind::Transfer
            ),
            crate::Error::<Test>::DuplicatedRequest
        );
    });
}

#[test]
fn should_success_incoming_transfer() {
    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::Transfer).unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            incoming_asset: IncomingAsset::Loaded(AssetId::XOR.into(), AssetKind::Thischain),
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
        });
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_incoming_request_ready(&state, incoming_transfer.clone(), tx_hash).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100100u32.into()
        );
    });
}

#[test]
fn should_fail_incoming_transfer() {
    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::Transfer).unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            incoming_asset: IncomingAsset::Loaded(AssetId::XOR.into(), AssetKind::Thischain),
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
        });
        assert_ok!(EthBridge::register_incoming_request(
            Origin::signed(state.bridge_account.clone()),
            incoming_transfer.clone()
        ));
        assert!(crate::PendingIncomingRequests::get().contains(&tx_hash));
        assert_eq!(
            crate::IncomingRequests::get(&tx_hash).unwrap(),
            incoming_transfer
        );
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::finalize_incoming_request(
            Origin::signed(state.bridge_account),
            Err((tx_hash, crate::Error::<Test>::Other.into()))
        ));
        assert_eq!(
            crate::RequestStatuses::get(&tx_hash).unwrap(),
            RequestStatus::Failed
        );
        assert!(crate::PendingIncomingRequests::get().is_empty());
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
    });
}

#[test]
fn should_register_and_find_asset_ids() {
    let (mut ext, _state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        // gets a known asset
        let (asset_id, asset_kind) = EthBridge::get_asset_by_raw_asset_id(
            H256(AssetId32::<AssetId>::from_asset_id(AssetId::XOR).code),
            &Address::zero(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(asset_id, AssetId::XOR.into());
        assert_eq!(asset_kind, AssetKind::Thischain);
        let token_address = Address::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        // registers unknown token
        assert!(
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address)
                .unwrap()
                .is_none()
        );
        // gets registered asset ID, associated with the token
        EthBridge::register_sidechain_asset(token_address, 18, AssetSymbol(b"TEST".to_vec()))
            .unwrap();
        let (asset_id, asset_kind) =
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address)
                .unwrap()
                .unwrap();
        assert_eq!(
            asset_id,
            AssetId32::from_bytes(hex!(
                "a308f54ca8c5b054d3180463aa4443c2c87600a1c2d21671f7dbfb39943377a9"
            ))
        );
        assert_eq!(asset_kind, AssetKind::Sidechain);
        assert_eq!(
            EthBridge::registered_sidechain_token(&asset_id).unwrap(),
            token_address
        );
        assert_eq!(
            EthBridge::registered_sidechain_asset(&token_address).unwrap(),
            asset_id
        );
    });
}

#[test]
fn should_add_new_asset_on_incoming_transfer() {
    let (mut ext, state, _pool_state, _oc_state) = ExtBuilder::new();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let tx_hash =
            request_incoming(alice.clone(), tx_hash, IncomingRequestKind::Transfer).unwrap();
        let token_address = Address::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            incoming_asset: IncomingAsset::ToRegister(
                token_address,
                18,
                AssetSymbol(b"TEST".to_vec()),
            ),
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
        });
        assert_incoming_request_ready(&state, incoming_transfer.clone(), tx_hash).unwrap();
        let asset_id = EthBridge::registered_sidechain_asset(&token_address).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&asset_id, &alice).unwrap(),
            100u32.into()
        );
    });
}

#[test]
fn should_convert_to_eth_address() {
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
