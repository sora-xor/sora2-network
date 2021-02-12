use crate::contract::RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID;
use crate::requests::encode_outgoing_request_eth_call;
use crate::{
    majority,
    mock::*,
    types,
    types::{Bytes, Log},
    Address, AssetKind, ContractEvent, IncomingRequest, IncomingRequestKind, OffchainRequest,
    OutgoingRequest, OutgoingTransfer, RequestStatus, SignatureParams,
};
use codec::{Decode, Encode};
use common::{balance::Balance, fixed, AssetId, AssetId32, AssetSymbol};
use frame_support::{
    assert_err, assert_noop, assert_ok,
    sp_runtime::{
        app_crypto::sp_core::{self, crypto::AccountId32, ecdsa, sr25519, Pair, Public},
        traits::IdentifyAccount,
    },
    StorageDoubleMap, StorageMap,
};
use hex_literal::hex;
use rustc_hex::FromHex;
use secp256k1::{PublicKey, SecretKey};
use sp_core::{H160, H256};
use sp_std::{collections::btree_set::BTreeSet, prelude::*};
use std::str::FromStr;

type Error = crate::Error<Test>;

const ETH_NETWORK_ID: u32 = 0;

fn get_signature_params(signature: &ecdsa::Signature) -> SignatureParams {
    let encoded = signature.encode();
    let mut params = SignatureParams::decode(&mut &encoded[..]).expect("Wrong signature format");
    params.v += 27;
    params
}

#[test]
fn parses_event() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let mut log = Log::default();
        log.topics = vec![types::H256(hex!("85c0fa492ded927d3acca961da52b0dda1debb06d8c27fe189315f06bb6e26c8"))];
        log.data = Bytes(hex!("111111111111111111111111111111111111111111111111111111111111111100000000000000000000000000000000000000000000000246ddf9797668000000000000000000000000000022222222222222222222222222222222222222220200040000000000000000000000000000000000000000000000000000000011").to_vec());
        assert_eq!(
            EthBridge::parse_main_event(&[log], IncomingRequestKind::Transfer).unwrap(),
            ContractEvent::Deposit(
                AccountId32::from(hex!("1111111111111111111111111111111111111111111111111111111111111111")),
                Balance::from(42u128),
                H160::from(&hex!("2222222222222222222222222222222222222222")),
                H256(hex!("0200040000000000000000000000000000000000000000000000000000000011"))
            )
        )
    });
}

#[test]
fn parses_deposit_pswap() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let mut log = Log::default();
        log.topics = vec![types::H256(hex!(
            "4eb3aea69bf61684354f60a43d355c3026751ddd0ea4e1f5afc1274b96c65505"
        ))];
        log.data = Bytes(
            hex!("00aaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaa").to_vec(),
        );
        assert_eq!(
            EthBridge::parse_main_event(&[log], IncomingRequestKind::ClaimPswap).unwrap(),
            ContractEvent::ClaimPswap(AccountId32::from(hex!(
                "00aaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaaffaa"
            )),)
        )
    });
}

#[test]
fn should_success_claim_pswap() {
    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::default();
    builder.add_reserves(net_id, (PSWAP.into(), Balance::from(0u32)));
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::ClaimPswap,
            net_id,
        )
        .unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_incoming_request_done(&state, request.clone()).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            300u32.into()
        );
    });
}

#[test]
fn should_fail_claim_pswap_already_claimed() {
    let _ = env_logger::try_init();

    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::default();
    builder.add_reserves(net_id, (PSWAP.into(), Balance::from(0u32)));
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::ClaimPswap,
            net_id,
        )
        .unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_incoming_request_done(&state, request.clone()).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            300u32.into()
        );
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2u8; 32]),
            IncomingRequestKind::ClaimPswap,
            net_id,
        )
        .unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        // Same eth_address
        assert_incoming_request_failed(&state, request.clone(), tx_hash).unwrap();
    });
}

#[test]
fn should_fail_claim_pswap_account_not_found() {
    let _ = env_logger::try_init();

    let net_id = ETH_NETWORK_ID;
    let mut builder = ExtBuilder::default();
    builder.add_reserves(net_id, (PSWAP.into(), Balance::from(0u32)));
    let (mut ext, state) = builder.build();
    let bridge_acc_id = state.networks[&ETH_NETWORK_ID]
        .config
        .bridge_account_id
        .clone();
    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::ClaimPswap,
            net_id,
        )
        .unwrap();
        let request = IncomingRequest::ClaimPswap(crate::IncomingClaimPswap {
            eth_address: Address::from_str("32fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
            account_id: alice.clone(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_ok!(EthBridge::register_incoming_request(
            Origin::signed(bridge_acc_id.clone()),
            request.clone()
        ));
        assert!(crate::PendingIncomingRequests::<Test>::get(net_id).contains(&tx_hash));
        assert_eq!(
            crate::IncomingRequests::get(net_id, &tx_hash).unwrap(),
            request
        );
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_ok!(EthBridge::finalize_incoming_request(
            Origin::signed(bridge_acc_id),
            Err((tx_hash, Error::AccountNotFound.into())),
            net_id
        ));
        assert_eq!(
            crate::RequestStatuses::<Test>::get(net_id, &tx_hash).unwrap(),
            RequestStatus::Failed
        );
        assert!(crate::PendingIncomingRequests::<Test>::get(net_id).is_empty());
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::PSWAP.into(), &alice).unwrap(),
            0u32.into()
        );
    });
}

fn last_event() -> Option<Event> {
    frame_system::Module::<Test>::events()
        .pop()
        .map(|x| x.event)
}

fn no_event() -> bool {
    frame_system::Module::<Test>::events().pop().is_none()
}

fn approve_request(state: &State, request: OutgoingRequest<Test>) -> Result<(), Option<Event>> {
    let request_hash = request.hash();
    let encoded = request.to_eth_abi(request_hash).unwrap();
    System::reset_events();
    let net_id = request.network_id();
    assert_eq!(
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .unwrap()
            .hash(),
        request.hash()
    );
    let mut approvals = BTreeSet::new();
    let keypairs = &state.networks[&net_id].ocw_keypairs;
    for (i, (_signer, account_id, seed)) in keypairs.iter().enumerate() {
        let secret = SecretKey::parse_slice(seed).unwrap();
        let public = PublicKey::from_secret_key(&secret);
        let msg = EthBridge::prepare_message(encoded.as_raw());
        let sig_pair = secp256k1::sign(&msg, &secret);
        let signature = sig_pair.into();
        let signature_params = get_signature_params(&signature);
        approvals.insert(signature_params.clone());
        let additional_sigs = if crate::PendingPeer::<Test>::get(net_id).is_some() {
            1
        } else {
            0
        };
        let sigs_needed = majority(keypairs.len()) + additional_sigs;
        let current_status = crate::RequestStatuses::<Test>::get(net_id, &request.hash()).unwrap();
        assert_ok!(EthBridge::approve_request(
            Origin::signed(account_id.clone()),
            ecdsa::Public::from_slice(&public.serialize_compressed()),
            request.clone(),
            encoded.clone(),
            signature_params
        ));
        if current_status == RequestStatus::Pending && i + 1 == sigs_needed {
            match last_event().ok_or(None)? {
                Event::eth_bridge(bridge_event) => match bridge_event {
                    crate::RawEvent::ApprovalsCollected(e, a) => {
                        assert_eq!(e, encoded);
                        assert_eq!(a, approvals);
                    }
                    e => {
                        assert_ne!(
                            crate::RequestsQueue::<Test>::get(net_id)
                                .last()
                                .map(|x| x.hash()),
                            Some(request.hash())
                        );
                        return Err(Some(Event::eth_bridge(e)));
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
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .map(|x| x.hash()),
        Some(request.hash())
    );
    Ok(())
}

fn last_outgoing_request(net_id: u32) -> Option<OutgoingRequest<Test>> {
    let request = crate::RequestsQueue::<Test>::get(net_id).last().cloned()?;
    match request {
        OffchainRequest::Outgoing(r, _) => Some(r),
        _ => panic!("Unexpected request type"),
    }
}

fn approve_last_request(
    state: &State,
    net_id: u32,
) -> Result<OutgoingRequest<Test>, Option<Event>> {
    let request = crate::RequestsQueue::<Test>::get(net_id).pop().unwrap();
    let outgoing_request = match request {
        OffchainRequest::Outgoing(r, _) => r,
        _ => panic!("Unexpected request type"),
    };
    approve_request(state, outgoing_request.clone())?;
    Ok(outgoing_request)
}

fn request_incoming(
    account_id: AccountId,
    tx_hash: H256,
    kind: IncomingRequestKind,
    net_id: u32,
) -> Result<H256, Event> {
    assert_ok!(EthBridge::request_from_sidechain(
        Origin::signed(account_id),
        tx_hash,
        kind,
        net_id
    ));
    let requests_queue = crate::RequestsQueue::get(net_id);
    let last_request: &OffchainRequest<Test> = requests_queue.last().unwrap();
    match last_request {
        OffchainRequest::Incoming(..) => (),
        _ => panic!("Invalid off-chain request"),
    }
    let hash = last_request.hash();
    assert_eq!(
        crate::RequestStatuses::<Test>::get(net_id, &hash).unwrap(),
        RequestStatus::Pending
    );
    Ok(hash)
}

fn assert_incoming_request_done(
    state: &State,
    incoming_request: IncomingRequest<Test>,
) -> Result<(), Option<Event>> {
    let net_id = incoming_request.network_id();
    let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
    let req_hash = incoming_request.hash();
    assert_eq!(
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .unwrap()
            .hash()
            .0,
        req_hash.0
    );
    assert_ok!(EthBridge::register_incoming_request(
        Origin::signed(bridge_acc_id.clone()),
        incoming_request.clone()
    ));
    assert_ne!(
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .map(|x| x.hash().0),
        Some(req_hash.0)
    );
    assert!(crate::PendingIncomingRequests::<Test>::get(net_id).contains(&req_hash));
    assert_eq!(
        crate::IncomingRequests::get(net_id, &req_hash).unwrap(),
        incoming_request
    );
    assert_ok!(EthBridge::finalize_incoming_request(
        Origin::signed(bridge_acc_id.clone()),
        Ok(incoming_request),
        net_id,
    ));
    assert_eq!(
        crate::RequestStatuses::<Test>::get(net_id, &req_hash).unwrap(),
        RequestStatus::Done
    );
    assert!(crate::PendingIncomingRequests::<Test>::get(net_id).is_empty());
    Ok(())
}

fn assert_incoming_request_registration_failed(
    state: &State,
    incoming_request: IncomingRequest<Test>,
    error: crate::Error<Test>,
) -> Result<(), Event> {
    let net_id = incoming_request.network_id();
    let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
    assert_eq!(
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .unwrap()
            .hash()
            .0,
        incoming_request.hash().0
    );
    assert_err!(
        EthBridge::register_incoming_request(
            Origin::signed(bridge_acc_id.clone()),
            incoming_request.clone()
        ),
        error
    );
    Ok(())
}

fn assert_incoming_request_failed(
    state: &State,
    incoming_request: IncomingRequest<Test>,
    tx_hash: H256,
) -> Result<(), Event> {
    let net_id = incoming_request.network_id();

    let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
    assert_eq!(
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .unwrap()
            .hash()
            .0,
        incoming_request.hash().0
    );
    assert_ok!(EthBridge::register_incoming_request(
        Origin::signed(bridge_acc_id.clone()),
        incoming_request.clone()
    ));
    assert_ne!(
        crate::RequestsQueue::<Test>::get(net_id)
            .last()
            .map(|x| x.hash().0),
        Some(incoming_request.hash().0)
    );
    assert!(crate::PendingIncomingRequests::<Test>::get(net_id).contains(&tx_hash));
    assert_eq!(
        crate::IncomingRequests::get(net_id, &tx_hash).unwrap(),
        incoming_request
    );
    assert_ok!(EthBridge::finalize_incoming_request(
        Origin::signed(bridge_acc_id.clone()),
        Ok(incoming_request),
        net_id,
    ));
    assert_eq!(
        crate::RequestStatuses::<Test>::get(net_id, &tx_hash).unwrap(),
        RequestStatus::Failed
    );
    assert!(crate::PendingIncomingRequests::<Test>::get(net_id).is_empty());
    Ok(())
}

#[test]
fn should_approve_outgoing_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100000u32.into())
            .unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            99900u32.into()
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
    });
}

#[test]
fn should_mint_and_burn_sidechain_asset() {
    let (mut ext, state) = ExtBuilder::default().build();

    #[track_caller]
    fn check_invariant(asset_id: &AssetId32<AssetId>, val: u32) {
        assert_eq!(
            assets::Module::<Test>::total_issuance(asset_id).unwrap(),
            val.into()
        );
    }

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let token_address = Address::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        EthBridge::register_sidechain_asset(
            token_address,
            18,
            AssetSymbol(b"TEST".to_vec()),
            net_id,
        )
        .unwrap();
        let (asset_id, asset_kind) =
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address, net_id)
                .unwrap()
                .unwrap();
        assert_eq!(asset_kind, AssetKind::Sidechain);
        check_invariant(&asset_id, 0);
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind,
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        check_invariant(&asset_id, 100);
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            asset_id,
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        check_invariant(&asset_id, 0);
    });
}

#[test]
fn should_not_burn_or_mint_sidechain_owned_asset() {
    let (mut ext, state) = ExtBuilder::default().build();

    #[track_caller]
    fn check_invariant() {
        assert_eq!(
            assets::Module::<Test>::total_issuance(&AssetId::XOR.into()).unwrap(),
            350000u32.into()
        );
    }

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_eq!(
            EthBridge::registered_asset(net_id, AssetId32::from(AssetId::XOR)).unwrap(),
            AssetKind::SidechainOwned
        );
        check_invariant();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id: AssetId::XOR.into(),
            asset_kind: AssetKind::SidechainOwned,
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        check_invariant();
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        check_invariant();
    });
}

#[test]
fn should_not_transfer() {
    let (mut ext, _) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_err!(
            EthBridge::transfer_to_sidechain(
                Origin::signed(alice.clone()),
                AssetId::KSM.into(),
                Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::UnsupportedToken
        );
        assert!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_000_000_u32.into(),
            net_id,
        )
        .is_err(),);
    });
}

#[test]
fn should_register_outgoing_transfer() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100000u32.into())
            .unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from([1; 20]),
            100u32.into(),
            net_id,
        ));
        let outgoing_transfer = OutgoingTransfer::<Test> {
            from: alice.clone(),
            to: Address::from([1; 20]),
            asset_id: AssetId::XOR.into(),
            amount: 100_u32.into(),
            nonce: 2,
            network_id: ETH_NETWORK_ID,
        };
        let last_request = crate::RequestsQueue::get(net_id).pop().unwrap();
        match last_request {
            OffchainRequest::Outgoing(OutgoingRequest::Transfer(r), _) => {
                assert_eq!(r, outgoing_transfer)
            }
            _ => panic!("Invalid off-chain request"),
        }
    });
}

#[test]
fn should_not_accept_duplicated_incoming_transfer() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assert_ok!(EthBridge::request_from_sidechain(
            Origin::signed(alice.clone()),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        ));
        assert_err!(
            EthBridge::request_from_sidechain(
                Origin::signed(alice.clone()),
                H256::from_slice(&[1u8; 32]),
                IncomingRequestKind::Transfer,
                net_id,
            ),
            Error::DuplicatedRequest
        );
    });
}

#[test]
fn should_not_accept_approved_incoming_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id: AssetId::XOR.into(),
            asset_kind: AssetKind::Thischain,
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        assert_err!(
            EthBridge::request_from_sidechain(
                Origin::signed(alice.clone()),
                H256::from_slice(&[1u8; 32]),
                IncomingRequestKind::Transfer,
                net_id,
            ),
            Error::DuplicatedRequest
        );
    });
}

#[test]
fn should_success_incoming_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id: AssetId::XOR.into(),
            asset_kind: AssetKind::Thischain,
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            0u32.into()
        );
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100u32.into()
        );
    });
}

#[test]
fn should_cancel_incoming_transfer() {
    let mut builder = ExtBuilder::new();
    let net_id = builder.add_network(
        vec![(
            AssetId::XOR.into(),
            Some(sp_core::H160::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap()),
            AssetKind::SidechainOwned,
        )],
        Some(vec![(XOR.into(), Balance::from(100u32))]),
        None,
    );
    let (mut ext, state) = builder.build();
    ext.execute_with(|| {
        let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100000u32.into())
            .unwrap();
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id: AssetId::XOR.into(),
            asset_kind: AssetKind::Thischain,
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_ok!(EthBridge::register_incoming_request(
            Origin::signed(bridge_acc_id.clone()),
            incoming_transfer.clone()
        ));
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assets::Module::<Test>::unreserve(AssetId::XOR.into(), &bridge_acc_id, 100u32.into())
            .unwrap();
        assets::Module::<Test>::transfer_from(
            &AssetId::XOR.into(),
            &bridge_acc_id,
            &bob,
            100u32.into(),
        )
        .unwrap();
        assert_ok!(EthBridge::finalize_incoming_request(
            Origin::signed(bridge_acc_id.clone()),
            Ok(incoming_transfer.clone()),
            net_id,
        ));
        assert_eq!(
            crate::RequestStatuses::<Test>::get(net_id, incoming_transfer.hash()).unwrap(),
            RequestStatus::Failed
        );
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
    });
}

#[test]
fn should_fail_incoming_transfer() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100000u32.into())
            .unwrap();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id: AssetId::XOR.into(),
            asset_kind: AssetKind::Thischain,
            amount: 100u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_ok!(EthBridge::register_incoming_request(
            Origin::signed(bridge_acc_id.clone()),
            incoming_transfer.clone()
        ));
        assert!(crate::PendingIncomingRequests::<Test>::get(net_id).contains(&tx_hash));
        assert_eq!(
            crate::IncomingRequests::get(net_id, &tx_hash).unwrap(),
            incoming_transfer
        );
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
        assert_ok!(EthBridge::finalize_incoming_request(
            Origin::signed(bridge_acc_id),
            Err((tx_hash, Error::Other.into())),
            net_id,
        ));
        assert_eq!(
            crate::RequestStatuses::<Test>::get(net_id, &tx_hash).unwrap(),
            RequestStatus::Failed
        );
        assert!(crate::PendingIncomingRequests::<Test>::get(net_id).is_empty());
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100000u32.into()
        );
    });
}

#[test]
fn should_register_and_find_asset_ids() {
    let (mut ext, _state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        // gets a known asset
        let (asset_id, asset_kind) = EthBridge::get_asset_by_raw_asset_id(
            H256(AssetId32::<AssetId>::from_asset_id(AssetId::XOR).code),
            &Address::zero(),
            net_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(asset_id, AssetId::XOR.into());
        assert_eq!(asset_kind, AssetKind::Thischain);
        let token_address = Address::from(hex!("7d7ff6f42e928de241282b9606c8e98ea48526e2"));
        // registers unknown token
        assert!(
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address, net_id)
                .unwrap()
                .is_none()
        );
        // gets registered asset ID, associated with the token
        EthBridge::register_sidechain_asset(
            token_address,
            18,
            AssetSymbol(b"TEST".to_vec()),
            net_id,
        )
        .unwrap();
        let (asset_id, asset_kind) =
            EthBridge::get_asset_by_raw_asset_id(H256::zero(), &token_address, net_id)
                .unwrap()
                .unwrap();
        assert_eq!(
            asset_id,
            AssetId32::from_bytes(hex!(
                "e1998577153deb622b5d7faabf23846281a8b074e1d4eebd31bca9dbe2c23006"
            ))
        );
        assert_eq!(asset_kind, AssetKind::Sidechain);
        assert_eq!(
            EthBridge::registered_sidechain_token(net_id, &asset_id).unwrap(),
            token_address
        );
        assert_eq!(
            EthBridge::registered_sidechain_asset(net_id, &token_address).unwrap(),
            asset_id
        );
    });
}

#[test]
fn should_convert_to_eth_address() {
    let (mut ext, _) = ExtBuilder::default().build();
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

#[test]
fn should_add_asset() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = assets::Module::<Test>::register_from(
            &alice,
            AssetSymbol(b"TEST".to_vec()),
            18,
            Balance::from(0u32),
            true,
        )
        .unwrap();
        assert_ok!(EthBridge::add_asset(
            Origin::signed(alice.clone()),
            asset_id,
            fixed!(100),
            net_id,
        ));
        assert!(EthBridge::registered_asset(net_id, asset_id).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            EthBridge::registered_asset(net_id, asset_id).unwrap(),
            AssetKind::Thischain
        );
    });
}

#[test]
fn should_add_token() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let token_address = Address::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = "TEST".into();
        let name = "Test Token".into();
        let decimals = 18;
        assert_ok!(EthBridge::add_sidechain_token(
            Origin::signed(state.authority_account_id.clone()),
            token_address,
            ticker,
            name,
            decimals,
            ETH_NETWORK_ID,
        ));
        assert!(EthBridge::registered_sidechain_asset(net_id, &token_address).is_none());
        approve_last_request(&state, net_id).expect("request wasn't approved");
        let asset_id_opt = EthBridge::registered_sidechain_asset(net_id, &token_address);
        assert!(asset_id_opt.is_some());
        assert_eq!(
            EthBridge::registered_asset(net_id, asset_id_opt.unwrap()).unwrap(),
            AssetKind::Sidechain
        );
    });
}

#[test]
fn should_not_add_token_if_not_bridge_account() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let token_address = Address::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        let ticker = "TEST".into();
        let name = "Test Token".into();
        let decimals = 18;
        assert_err!(
            EthBridge::add_sidechain_token(
                Origin::signed(bob),
                token_address,
                ticker,
                name,
                decimals,
                net_id,
            ),
            Error::Forbidden
        );
    });
}

#[test]
fn should_force_add_peer() {
    let (mut ext, state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bridge_acc_id = state.networks[&net_id].config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let kp = ecdsa::Pair::from_string("//OCW5", None).unwrap();
        let signer = AccountPublic::from(kp.public());
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&kp.seed()).unwrap());

        // outgoing request part
        let new_peer_id = signer.into_account();
        let new_peer_address = crate::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::add_peer(
            Origin::signed(state.authority_account_id.clone()),
            new_peer_id.clone(),
            new_peer_address,
            net_id,
        ));
        assert_eq!(
            crate::PendingPeer::<Test>::get(net_id).unwrap(),
            new_peer_id
        );
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(
            crate::PendingPeer::<Test>::get(net_id).unwrap(),
            new_peer_id
        );
        assert_eq!(
            crate::PeerAccountId::<Test>::get(&net_id, &new_peer_address),
            new_peer_id
        );
        assert_eq!(
            crate::PeerAddress::<Test>::get(net_id, &new_peer_id),
            new_peer_address
        );
        // incoming request part
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::AddPeer,
            net_id,
        )
        .unwrap();
        let incoming_request = IncomingRequest::ChangePeers(crate::IncomingChangePeers {
            peer_account_id: new_peer_id.clone(),
            peer_address: new_peer_address,
            added: true,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert!(!crate::Peers::<Test>::get(net_id).contains(&new_peer_id));
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(crate::PendingPeer::<Test>::get(net_id).is_none());
        assert!(crate::Peers::<Test>::get(net_id).contains(&new_peer_id));
        assert!(bridge_multisig::Accounts::<Test>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&new_peer_id));
    });
}

#[test]
fn should_remove_peer() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5));
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let extended_network_config = &state.networks[&net_id];
        let bridge_acc_id = extended_network_config.config.bridge_account_id.clone();
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let (_, peer_id, seed) = &extended_network_config.ocw_keypairs[4];
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&seed[..]).unwrap());

        // outgoing request part
        assert_ok!(EthBridge::remove_peer(
            Origin::signed(state.authority_account_id.clone()),
            peer_id.clone(),
            net_id,
        ));
        assert_eq!(&crate::PendingPeer::<Test>::get(net_id).unwrap(), peer_id);
        assert!(crate::Peers::<Test>::get(net_id).contains(&peer_id));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_eq!(&crate::PendingPeer::<Test>::get(net_id).unwrap(), peer_id);
        assert!(!crate::Peers::<Test>::get(net_id).contains(&peer_id));
        assert!(!bridge_multisig::Accounts::<Test>::get(&bridge_acc_id)
            .unwrap()
            .is_signatory(&peer_id));

        // incoming request part
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::RemovePeer,
            net_id,
        )
        .unwrap();
        let peer_address = crate::public_key_to_eth_address(&public);
        let incoming_request = IncomingRequest::ChangePeers(crate::IncomingChangePeers {
            peer_account_id: peer_id.clone(),
            peer_address,
            added: false,
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: ETH_NETWORK_ID,
        });
        assert_incoming_request_done(&state, incoming_request.clone()).unwrap();
        assert!(crate::PendingPeer::<Test>::get(net_id).is_none());
    });
}

#[test]
fn should_not_allow_add_and_remove_peer_only_to_authority() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5));
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let bob = get_account_id_from_seed::<sr25519::Public>("Bob");
        let (_, peer_id, _) = &state.networks[&net_id].ocw_keypairs[4];
        assert_err!(
            EthBridge::remove_peer(Origin::signed(bob.clone()), peer_id.clone(), net_id),
            Error::Forbidden
        );
        assert_err!(
            EthBridge::add_peer(
                Origin::signed(bob.clone()),
                peer_id.clone(),
                Address::from(&hex!("2222222222222222222222222222222222222222")),
                net_id,
            ),
            Error::Forbidden
        );
    });
}

#[test]
fn should_not_allow_changing_peers_simultaneously() {
    let mut builder = ExtBuilder::new();
    builder.add_network(vec![], None, Some(5));
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let (_, peer_id, seed) = &state.networks[&net_id].ocw_keypairs[4];
        let public = PublicKey::from_secret_key(&SecretKey::parse_slice(&seed[..]).unwrap());
        let address = crate::public_key_to_eth_address(&public);
        assert_ok!(EthBridge::remove_peer(
            Origin::signed(state.authority_account_id.clone()),
            peer_id.clone(),
            net_id,
        ));
        approve_last_request(&state, net_id).expect("request wasn't approved");
        assert_err!(
            EthBridge::remove_peer(
                Origin::signed(state.authority_account_id.clone()),
                peer_id.clone(),
                net_id,
            ),
            Error::UnknownPeerId
        );
        assert_err!(
            EthBridge::add_peer(
                Origin::signed(state.authority_account_id.clone()),
                peer_id.clone(),
                address,
                net_id,
            ),
            Error::TooManyPendingPeers
        );
    });
}

#[test]
fn should_cancel_ready_outgoing_request() {
    let _ = env_logger::try_init();
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Sending request part
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100u32.into())
            .unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            0u32.into()
        );
        let outgoing_req = approve_last_request(&state, net_id).expect("request wasn't approved");

        // Cancelling request part
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let request_hash = request_incoming(
            alice.clone(),
            tx_hash,
            IncomingRequestKind::CancelOutgoingRequest,
            net_id,
        )
        .unwrap();
        let tx_input = encode_outgoing_request_eth_call::<Test>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID,
            &outgoing_req,
        )
        .unwrap();
        let incoming_transfer =
            IncomingRequest::CancelOutgoingRequest(crate::IncomingCancelOutgoingRequest {
                request: outgoing_req.clone(),
                initial_request_hash: request_hash,
                tx_input: tx_input.clone(),
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: ETH_NETWORK_ID,
            });

        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100u32.into()
        );
    });
}

#[test]
fn should_fail_cancel_ready_outgoing_request_with_wrong_approvals() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Sending request part
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100u32.into())
            .unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            0u32.into()
        );
        let outgoing_req = approve_last_request(&state, net_id).expect("request wasn't approved");

        // Cancelling request part
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let request_hash = request_incoming(
            alice.clone(),
            tx_hash,
            IncomingRequestKind::CancelOutgoingRequest,
            net_id,
        )
        .unwrap();
        let tx_input = encode_outgoing_request_eth_call::<Test>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID,
            &outgoing_req,
        )
        .unwrap();
        let incoming_transfer =
            IncomingRequest::CancelOutgoingRequest(crate::IncomingCancelOutgoingRequest {
                request: outgoing_req.clone(),
                initial_request_hash: request_hash,
                tx_input: tx_input.clone(),
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: ETH_NETWORK_ID,
            });

        // Insert some signature
        crate::RequestApprovals::<Test>::mutate(net_id, outgoing_req.hash(), |v| {
            v.insert(SignatureParams {
                r: [1; 32],
                s: [1; 32],
                v: 0,
            })
        });
        assert_incoming_request_registration_failed(
            &state,
            incoming_transfer.clone(),
            Error::InvalidContractInput,
        )
        .unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            0u32.into()
        );
    });
}

#[test]
fn should_fail_cancel_unfinished_outgoing_request() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Sending request part
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100u32.into())
            .unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            100u32.into()
        );
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            0u32.into()
        );
        let outgoing_req = last_outgoing_request(net_id).expect("request wasn't found");

        // Cancelling request part
        let tx_hash = H256::from_slice(&[1u8; 32]);
        let request_hash = request_incoming(
            alice.clone(),
            tx_hash,
            IncomingRequestKind::CancelOutgoingRequest,
            net_id,
        )
        .unwrap();
        let tx_input = encode_outgoing_request_eth_call::<Test>(
            *RECEIVE_BY_ETHEREUM_ASSET_ADDRESS_ID,
            &outgoing_req,
        )
        .unwrap();
        let incoming_transfer =
            IncomingRequest::CancelOutgoingRequest(crate::IncomingCancelOutgoingRequest {
                request: outgoing_req,
                initial_request_hash: request_hash,
                tx_input,
                tx_hash,
                at_height: 1,
                timepoint: Default::default(),
                network_id: ETH_NETWORK_ID,
            });
        assert_incoming_request_registration_failed(
            &state,
            incoming_transfer.clone(),
            Error::RequestIsNotReady,
        )
        .unwrap();
        assert_eq!(
            assets::Module::<Test>::total_balance(&AssetId::XOR.into(), &alice).unwrap(),
            0u32.into()
        );
    });
}

#[test]
fn should_mark_request_as_done() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100u32.into())
            .unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        let outgoing_req = approve_last_request(&state, net_id).expect("request wasn't approved");
        let outgoing_req_hash = outgoing_req.hash();
        let _request_hash = request_incoming(
            alice.clone(),
            outgoing_req_hash,
            IncomingRequestKind::MarkAsDone,
            net_id,
        )
        .unwrap();
        assert_ok!(EthBridge::finalize_mark_as_done(
            Origin::signed(state.networks[&net_id].config.bridge_account_id.clone()),
            outgoing_req_hash,
            net_id,
        ));
        assert_eq!(
            crate::RequestStatuses::<Test>::get(net_id, outgoing_req_hash).unwrap(),
            RequestStatus::Done
        );
    });
}

#[test]
fn should_not_mark_request_as_done() {
    let (mut ext, state) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let net_id = ETH_NETWORK_ID;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        assets::Module::<Test>::mint_to(&AssetId::XOR.into(), &alice, &alice, 100u32.into())
            .unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            AssetId::XOR.into(),
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            100_u32.into(),
            net_id,
        ));
        let outgoing_req = last_outgoing_request(net_id).expect("request wasn't approved");
        let outgoing_req_hash = outgoing_req.hash();
        assert_noop!(
            EthBridge::request_from_sidechain(
                Origin::signed(alice.clone()),
                outgoing_req_hash,
                IncomingRequestKind::MarkAsDone,
                net_id
            ),
            Error::RequestIsNotReady
        );
        assert_noop!(
            EthBridge::finalize_mark_as_done(
                Origin::signed(state.networks[&net_id].config.bridge_account_id.clone()),
                outgoing_req_hash,
                net_id,
            ),
            Error::RequestIsNotReady
        );
        // incoming requests can't be made done
        let req_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id: AssetId::XOR.into(),
            asset_kind: AssetKind::Thischain,
            amount: 100u32.into(),
            tx_hash: req_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        assert_noop!(
            EthBridge::finalize_mark_as_done(
                Origin::signed(state.networks[&net_id].config.bridge_account_id.clone()),
                req_hash,
                net_id,
            ),
            Error::RequestIsNotReady
        );
    });
}

#[test]
fn should_fail_request_to_unknown_network() {
    let (mut ext, _state) = ExtBuilder::default().build();

    ext.execute_with(|| {
        let net_id = 3;
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = AssetId::XOR.into();
        assets::Module::<Test>::mint_to(&asset_id, &alice, &alice, 100u32.into()).unwrap();
        assert_noop!(
            EthBridge::transfer_to_sidechain(
                Origin::signed(alice.clone()),
                asset_id,
                Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
                100_u32.into(),
                net_id,
            ),
            Error::UnknownNetwork
        );

        assert_noop!(
            EthBridge::add_asset(Origin::signed(alice.clone()), asset_id, fixed!(100), net_id,),
            Error::UnknownNetwork
        );

        assert_noop!(
            EthBridge::request_from_sidechain(
                Origin::signed(alice),
                H256::from_slice(&[1u8; 32]),
                IncomingRequestKind::Transfer,
                net_id
            ),
            Error::UnknownNetwork
        );
    });
}

#[test]
fn should_reserve_owned_asset_on_different_networks() {
    let mut builder = ExtBuilder::default();
    let net_id_0 = ETH_NETWORK_ID;
    let net_id_1 = builder.add_network(vec![], None, None);
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        let asset_id = AssetId::XOR.into();
        assets::Module::<Test>::mint_to(&asset_id, &alice, &alice, 100u32.into()).unwrap();
        assets::Module::<Test>::mint_to(
            &asset_id,
            &alice,
            &state.networks[&net_id_0].config.bridge_account_id,
            100u32.into(),
        )
        .unwrap();
        assets::Module::<Test>::mint_to(
            &asset_id,
            &alice,
            &state.networks[&net_id_1].config.bridge_account_id,
            100u32.into(),
        )
        .unwrap();
        let supply = assets::Module::<Test>::total_issuance(&asset_id).unwrap();
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            asset_id,
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_0,
        ));
        approve_last_request(&state, net_id_0).expect("request wasn't approved");
        assert_ok!(EthBridge::add_asset(
            Origin::signed(alice.clone()),
            asset_id,
            fixed!(0),
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");
        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            asset_id,
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");
        assert_eq!(
            assets::Module::<Test>::total_issuance(&asset_id).unwrap(),
            supply
        );

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id_0,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount: 50u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_0,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2; 32]),
            IncomingRequestKind::Transfer,
            net_id_1,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount: 50u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_1,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();
        assert_eq!(
            assets::Module::<Test>::total_issuance(&asset_id).unwrap(),
            supply
        );
    });
}

#[test]
fn should_handle_sidechain_and_thischain_asset_on_different_networks() {
    let mut builder = ExtBuilder::default();
    let net_id_0 = ETH_NETWORK_ID;
    let net_id_1 = builder.add_network(vec![], None, None);
    let (mut ext, state) = builder.build();

    ext.execute_with(|| {
        let alice = get_account_id_from_seed::<sr25519::Public>("Alice");
        // Register token on the first network.
        let token_address = Address::from(hex!("e88f8313e61a97cec1871ee37fbbe2a8bf3ed1e4"));
        assert_ok!(EthBridge::add_sidechain_token(
            Origin::signed(state.authority_account_id.clone()),
            token_address,
            "TEST".into(),
            "Test Token".into(),
            18,
            net_id_0,
        ));
        approve_last_request(&state, net_id_0).expect("request wasn't approved");
        let asset_id = EthBridge::registered_sidechain_asset(net_id_0, &token_address)
            .expect("Asset wasn't found.");
        assert_eq!(
            EthBridge::registered_asset(net_id_0, asset_id).unwrap(),
            AssetKind::Sidechain
        );

        // Register the newly generated asset in the second network
        assert_ok!(EthBridge::add_asset(
            Origin::signed(alice.clone()),
            asset_id,
            fixed!(0),
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");
        assert_eq!(
            EthBridge::registered_asset(net_id_1, asset_id).unwrap(),
            AssetKind::Thischain
        );
        assets::Module::<Test>::mint_to(
            &asset_id,
            &state.networks[&net_id_0].config.bridge_account_id,
            &state.networks[&net_id_1].config.bridge_account_id,
            100u32.into(),
        )
        .unwrap();
        let supply = assets::Module::<Test>::total_issuance(&asset_id).unwrap();
        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[1u8; 32]),
            IncomingRequestKind::Transfer,
            net_id_0,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Sidechain,
            amount: 50u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_0,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();

        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            asset_id,
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_1,
        ));
        approve_last_request(&state, net_id_1).expect("request wasn't approved");

        let tx_hash = request_incoming(
            alice.clone(),
            H256::from_slice(&[2; 32]),
            IncomingRequestKind::Transfer,
            net_id_1,
        )
        .unwrap();
        let incoming_transfer = IncomingRequest::Transfer(crate::IncomingTransfer {
            from: Address::from([1; 20]),
            to: alice.clone(),
            asset_id,
            asset_kind: AssetKind::Thischain,
            amount: 50u32.into(),
            tx_hash,
            at_height: 1,
            timepoint: Default::default(),
            network_id: net_id_1,
        });
        assert_incoming_request_done(&state, incoming_transfer.clone()).unwrap();

        assert_ok!(EthBridge::transfer_to_sidechain(
            Origin::signed(alice.clone()),
            asset_id,
            Address::from_str("19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A").unwrap(),
            50_u32.into(),
            net_id_0,
        ));
        approve_last_request(&state, net_id_0).expect("request wasn't approved");
        assert_eq!(
            assets::Module::<Test>::total_issuance(&asset_id).unwrap(),
            supply
        );
    });
}
