// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{
    AccountId, Assets, Balances, BridgeMultisig, BridgeProxy, Currencies, EthBridge, Executive,
    Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, Signature, SignedExtra, System,
    UncheckedExtrinsic, XorFee,
};
use bridge_types::traits::BridgeAssetLockChecker;
use codec::Encode;
use common::{balance, PSWAP};
use frame_support::{
    assert_ok,
    dispatch::{GetDispatchInfo, Pays},
    traits::Currency,
};
use framenode_chain_spec::ext;
use sp_core::{sr25519, Pair, H160, H256};
use sp_runtime::{generic::Era, DispatchResult, FixedPointNumber, FixedU128};
use traits::MultiCurrency;

fn seed_existing_pswap_bridge_mapping() {
    use frame_support::StorageHasher;
    // Fee tests start from an existing native-asset mapping. Registering PSWAP
    // as a new foreign asset would give this fixture the wrong custody semantics.
    let token = H160::repeat_byte(2);
    let network = frame_support::Twox64Concat::hash(&0u32.encode());
    for (name, suffix, value) in [
        (
            b"RegisteredAsset".as_slice(),
            PSWAP.encode(),
            eth_bridge::requests::AssetKind::Thischain.encode(),
        ),
        (
            b"SidechainAssetPrecision".as_slice(),
            PSWAP.encode(),
            18u8.encode(),
        ),
        (
            b"RegisteredSidechainToken".as_slice(),
            frame_support::Blake2_128Concat::hash(&PSWAP.encode()),
            token.encode(),
        ),
        (
            b"RegisteredSidechainAsset".as_slice(),
            frame_support::Blake2_128Concat::hash(&token.encode()),
            PSWAP.encode(),
        ),
    ] {
        let mut key = frame_support::storage::storage_prefix(b"EthBridge", name).to_vec();
        key.extend_from_slice(&network);
        key.extend(suffix);
        sp_io::storage::set(&key, &value);
    }
    assert_eq!(
        EthBridge::registered_asset(0, PSWAP),
        Some(eth_bridge::requests::AssetKind::Thischain)
    );
    assert_eq!(EthBridge::registered_sidechain_token(0, PSWAP), Some(token));
    assert_eq!(EthBridge::registered_sidechain_asset(0, token), Some(PSWAP));
}

#[test]
fn captured_legacy_bridge_peer_pair_remains_compatible_without_queue_or_index() {
    use codec::DecodeAll;
    use eth_bridge::requests::{OffchainRequest, OutgoingRequest, RequestStatus};

    // These raw Requests rows come from the pinned public October 25 capture.
    // Their hashes/nonces are historical evidence; the statuses below exercise
    // pending and completed compatibility, and are not asserted capture statuses.
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/bridge-peer-pair-2026-10-25.json")).unwrap();
    let decode_hex = |value: &str| {
        value
            .strip_prefix("0x")
            .unwrap()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    sp_io::TestExternalities::default().execute_with(|| {
        let rows = fixture["rows"].as_array().unwrap();
        let mut decoded = Vec::new();
        for row in rows {
            let key = decode_hex(row["key"].as_str().unwrap());
            let value = decode_hex(row["value"].as_str().unwrap());
            let request = OffchainRequest::<Runtime>::decode_all(&mut &value[..]).unwrap();
            let OffchainRequest::Outgoing(outgoing, hash) = request else {
                panic!("fixture must contain outgoing peer requests")
            };
            assert_eq!(OffchainRequest::outgoing(outgoing.clone()).hash(), hash);
            assert_eq!(
                eth_bridge::Requests::<Runtime>::hashed_key_for(0, hash),
                key
            );
            sp_io::storage::set(&key, &value);
            decoded.push((outgoing, hash));
        }
        let (OutgoingRequest::AddPeer(primary), primary_hash) = &decoded[0] else {
            panic!("first fixture must be the primary add request")
        };
        let (OutgoingRequest::AddPeerCompat(compat), _) = &decoded[1] else {
            panic!("second fixture must be the compatibility request")
        };
        assert_eq!(primary.nonce, 8);
        assert_eq!(compat.nonce, 9);
        assert_eq!(primary.author, compat.author);
        assert_eq!(primary.timepoint, compat.timepoint);
        assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
        eth_bridge::RequestStatuses::<Runtime>::insert(0, primary_hash, RequestStatus::Pending);
        assert!(compat.should_be_skipped());
        for status in [RequestStatus::ApprovalsReady, RequestStatus::Done] {
            eth_bridge::RequestStatuses::<Runtime>::insert(0, primary_hash, status);
            assert!(!compat.should_be_skipped());
            assert_ok!(compat.finalize());
        }
    });
}

fn account(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public())
}
fn setup() -> (Vec<sr25519::Pair>, AccountId) {
    System::set_block_number(1);
    let pairs = vec![
        sr25519::Pair::from_seed(&[91; 32]),
        sr25519::Pair::from_seed(&[92; 32]),
    ];
    for pair in &pairs {
        System::inc_providers(&account(pair));
        assert_eq!(Balances::free_balance(&account(pair)), 0);
    }
    eth_bridge::Peers::<Runtime>::insert(
        0,
        pairs
            .iter()
            .map(account)
            .collect::<std::collections::BTreeSet<_>>(),
    );
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1)
    ));
    let id = EthBridge::bridge_account(0).unwrap();
    bridge_multisig::Accounts::<Runtime>::insert(
        &id,
        bridge_multisig::MultisigAccount::new(pairs.iter().map(account).collect()),
    );
    (pairs, id)
}
fn signed(call: RuntimeCall, pair: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = account(pair);
    let extra: SignedExtra = (
        frame_system::AuthorizeCall::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),
        frame_system::CheckWeight::<Runtime>::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|payload| pair.sign(payload));
    UncheckedExtrinsic::new_signed(call, who, Signature::Sr25519(signature), extra)
}
fn apply(call: RuntimeCall, pair: &sr25519::Pair, free: bool) -> DispatchResult {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    let extrinsic = signed(call, pair);
    assert_eq!(extrinsic.function.get_dispatch_info().pays_fee, Pays::Yes);
    assert!(
        XorFee::query_info(
            &extrinsic,
            &extrinsic.function,
            extrinsic.encoded_size() as u32
        )
        .partial_fee
            > 0
    );
    System::reset_events();
    let result = Executive::apply_extrinsic(extrinsic).expect("funded bridge submission admitted");
    let fee = System::events()
        .into_iter()
        .find_map(|record| match record.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who,
                    actual_fee,
                    tip,
                },
            ) if who == account(pair) => {
                assert_eq!(tip, 0);
                Some(actual_fee)
            }
            _ => None,
        })
        .expect("payment extension emitted fee");
    assert_eq!(fee == 0, free);
    result
}
fn incoming(id: &AccountId, backed: bool, byte: u8) -> (RuntimeCall, H256, AccountId) {
    let recipient = AccountId::from([94; 32]);
    assert_ok!(Assets::update_balance(
        RuntimeOrigin::root(),
        id.clone(),
        PSWAP,
        balance!(100) as i128
    ));
    if backed {
        assert_ok!(<BridgeProxy as BridgeAssetLockChecker<
            crate::AssetId,
            crate::Balance,
        >>::before_asset_lock(
            bridge_types::GenericNetworkId::EVMLegacy(0),
            bridge_types::types::AssetKind::Thischain,
            &PSWAP,
            &balance!(10)
        ));
    }
    let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, byte as u32);
    let remote_hash = H256::repeat_byte(byte);
    let transfer =
        eth_bridge::requests::IncomingRequest::Transfer(eth_bridge::requests::IncomingTransfer::<
            Runtime,
        > {
            from: H160::repeat_byte(1),
            to: recipient.clone(),
            asset_id: PSWAP,
            asset_kind: eth_bridge::requests::AssetKind::Thischain,
            amount: balance!(10),
            author: recipient.clone(),
            tx_hash: remote_hash,
            at_height: 10,
            timepoint,
            network_id: 0,
            should_take_fee: false,
        });
    let hash = eth_bridge::requests::OffchainRequest::incoming(transfer.clone()).hash();
    let load = eth_bridge::requests::LoadIncomingRequest::Transaction(
        eth_bridge::requests::LoadIncomingTransactionRequest::<Runtime>::new(
            recipient.clone(),
            remote_hash,
            timepoint,
            eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
            0,
        ),
    );
    (
        RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request: load,
            incoming_request_result: Ok(transfer),
        }),
        hash,
        recipient,
    )
}
fn multi(id: &AccountId, call: &RuntimeCall, byte: u8) -> RuntimeCall {
    RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
        id: id.clone(),
        maybe_timepoint: Some(bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(
            10,
            byte as u32,
        )),
        call: call.encode(),
        store_call: true,
        max_weight: call.get_dispatch_info().total_weight(),
    })
}

fn rejected(call: RuntimeCall, pair: &sr25519::Pair) {
    use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    let who = account(pair);
    let nonce = System::account_nonce(&who);
    System::reset_events();
    assert_eq!(
        Executive::apply_extrinsic(signed(call, pair)),
        Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
    );
    assert_eq!(System::account_nonce(&who), nonce);
    assert!(
        System::events().is_empty(),
        "invalid protocol must not execute or emit a payment event"
    );
}

#[test]
fn zero_xor_incoming_peers_execute_free_and_replays_are_rejected() {
    ext().execute_with(|| {
        let (pairs, id) = setup();
        let (call, hash, recipient) = incoming(&id, true, 11);
        let before = Currencies::free_balance(PSWAP, &recipient);
        assert_ok!(apply(multi(&id, &call, 11), &pairs[0], true));
        assert_ok!(apply(multi(&id, &call, 11), &pairs[1], true));
        assert_eq!(
            Currencies::free_balance(PSWAP, &recipient),
            before + balance!(10)
        );
        assert_eq!(
            eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
            Some(eth_bridge::requests::RequestStatus::Done)
        );
        for pair in &pairs {
            assert_eq!(Balances::total_balance(&account(pair)), 0);
            assert_eq!(System::account_nonce(&account(pair)), 1);
        }
        rejected(multi(&id, &call, 12), &pairs[0]);
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            0
        );
    });
}

#[test]
fn accepted_remote_failure_is_free_and_consumes_canonical_request() {
    ext().execute_with(|| {
        let (pairs, id) = setup();
        let remote = H256::repeat_byte(0x71);
        let load = eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::<Runtime>::new(
                account(&pairs[0]),
                remote,
                bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 17),
                eth_bridge::requests::IncomingTransactionRequestKind::Transfer,
                0,
            ),
        );
        let call = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request: load,
            incoming_request_result: Err(sp_runtime::DispatchError::CannotLookup),
        });
        assert_ok!(apply(multi(&id, &call, 17), &pairs[0], true));
        assert_ok!(apply(multi(&id, &call, 17), &pairs[1], true));
        assert!(matches!(
            eth_bridge::RequestStatuses::<Runtime>::get(0, remote),
            Some(eth_bridge::requests::RequestStatus::Failed(_))
        ));
        rejected(multi(&id, &call, 18), &pairs[0]);
        assert_eq!(Balances::total_balance(&account(&pairs[0])), 0);
        assert_eq!(Balances::total_balance(&account(&pairs[1])), 0);
    });
}

#[test]
fn duplicate_approval_and_mismatched_proof_are_rejected_before_dispatch() {
    ext().execute_with(|| {
        let (pairs, id) = setup();
        let (call, _, _) = incoming(&id, true, 14);
        assert_ok!(apply(multi(&id, &call, 14), &pairs[0], true));
        rejected(multi(&id, &call, 14), &pairs[0]);
        let hash = sp_io::hashing::blake2_256(&call.encode());
        assert_eq!(
            bridge_multisig::Multisigs::<Runtime>::get(&id, hash)
                .unwrap()
                .approvals,
            vec![account(&pairs[0])]
        );
        let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request,
            incoming_request_result: Ok(mut request),
        }) = call.clone()
        else {
            panic!("fixture")
        };
        if let eth_bridge::requests::IncomingRequest::Transfer(ref mut request) = request {
            request.tx_hash = H256::repeat_byte(0x72);
        }
        let invalid = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            load_incoming_request,
            incoming_request_result: Ok(request),
        });
        rejected(multi(&id, &invalid, 15), &pairs[1]);
        assert_ok!(apply(multi(&id, &call, 14), &pairs[1], true));
    });
}

#[test]
fn arbitrary_peer_wrappers_are_rejected_and_protocol_cleanup_remains_free() {
    ext().execute_with(|| {
        let (pairs, id) = setup();
        bridge_multisig::Accounts::<Runtime>::insert(
            &id,
            bridge_multisig::MultisigAccount::new(vec![account(&pairs[0])]),
        );
        let wrap = |call| {
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi_threshold_1 {
                id: id.clone(),
                call: Box::new(call),
                timepoint: BridgeMultisig::sidechain_timepoint(10, 15),
            })
        };
        let remark = wrap(RuntimeCall::System(frame_system::Call::remark {
            remark: vec![],
        }));
        assert_eq!(
            crate::xor_fee_impls::validate_bridge_fee_exemption(&account(&pairs[0]), &remark),
            Ok(false)
        );
        frame_system::BlockSize::<Runtime>::kill();
        frame_system::BlockWeight::<Runtime>::kill();
        assert_eq!(
            Executive::apply_extrinsic(signed(remark.clone(), &pairs[0])),
            Err(sp_runtime::transaction_validity::InvalidTransaction::Payment.into())
        );
        let _ = Balances::make_free_balance_be(&account(&pairs[0]), balance!(1000000));
        assert_eq!(
            apply(remark, &pairs[0], false),
            Err(frame_system::Error::<Runtime>::CallFiltered.into())
        );
        let (import, hash, _) = incoming(&id, true, 15);
        let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            incoming_request_result: Ok(request),
            ..
        }) = import
        else {
            panic!("fixture")
        };
        assert_ok!(EthBridge::register_incoming_request(
            RuntimeOrigin::signed(id.clone()),
            request
        ));
        let abort = RuntimeCall::EthBridge(eth_bridge::Call::abort_request {
            hash,
            error: sp_runtime::DispatchError::CannotLookup,
            network_id: 0,
        });
        assert_ok!(apply(wrap(abort.clone()), &pairs[0], true));
        rejected(wrap(abort), &pairs[0]);
    });
}

#[test]
fn stored_hash_protocol_approval_is_free_and_cancellation_releases_new_slot() {
    ext().execute_with(|| {
        let (pairs, id) = setup();
        let (call, _, _) = incoming(&id, true, 16);
        let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 16);
        assert_ok!(apply(multi(&id, &call, 16), &pairs[0], true));
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            1
        );
        let hash = sp_io::hashing::blake2_256(&call.encode());
        let cancel = RuntimeCall::BridgeMultisig(bridge_multisig::Call::cancel_as_multi {
            id: id.clone(),
            timepoint,
            call_hash: hash,
        });
        assert_ok!(apply(cancel.clone(), &pairs[0], true));
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            0
        );
        // A canceled proposal can be re-created; an executed external request cannot.
        assert_ok!(apply(multi(&id, &call, 16), &pairs[0], true));
        assert_ok!(apply(
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::approve_as_multi {
                id: id.clone(),
                maybe_timepoint: Some(timepoint),
                call_hash: hash,
                max_weight: call.get_dispatch_info().total_weight()
            }),
            &pairs[1],
            true
        ));
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            0
        );
    });
}

#[test]
fn user_requested_load_remains_paid_without_identity_exemption() {
    ext().execute_with(|| {
        let (pairs, _) = setup();
        let who = account(&pairs[0]);
        let _ = Balances::make_free_balance_be(&who, balance!(1000000));
        let before = Balances::total_balance(&who);
        assert_ok!(apply(
            RuntimeCall::EthBridge(eth_bridge::Call::request_from_sidechain {
                eth_tx_hash: H256::repeat_byte(22),
                kind: eth_bridge::requests::IncomingTransactionRequestKind::Transfer.into(),
                network_id: 0
            }),
            &pairs[0],
            false
        ));
        assert!(Balances::total_balance(&who) < before);
        assert_eq!(Balances::reserved_balance(&who), 0);
    });
}

#[test]
fn zero_xor_outgoing_peer_approval_checks_real_signature_and_rejects_replay() {
    use sp_runtime::{traits::IdentifyAccount, MultiSigner};
    ext().execute_with(|| {
        System::set_block_number(1);
        let peer=sp_core::ecdsa::Pair::from_seed(&[77;32]);
        let who=MultiSigner::Ecdsa(peer.public()).into_account();
        System::inc_providers(&who);
        eth_bridge::Peers::<Runtime>::insert(0,[who.clone(),AccountId::from([78;32])].into_iter().collect::<std::collections::BTreeSet<_>>());
        seed_existing_pswap_bridge_mapping();
        let request=eth_bridge::requests::OutgoingRequest::Transfer(eth_bridge::requests::OutgoingTransfer::<Runtime> {from:AccountId::from([79;32]),to:H160::repeat_byte(1),asset_id:PSWAP,amount:balance!(1),nonce:0,network_id:0,timepoint:Default::default()});
        let stored=eth_bridge::requests::OffchainRequest::outgoing(request.clone());let hash=stored.hash();
        eth_bridge::Requests::<Runtime>::insert(0,hash,stored);
        eth_bridge::RequestStatuses::<Runtime>::insert(0,hash,eth_bridge::requests::RequestStatus::Pending);
        let eth_bridge::requests::OutgoingRequestEncoded::Transfer(encoded) = request.to_eth_abi(hash).unwrap() else { panic!("transfer fixture") };
        let signature=peer.sign_prehashed(&common::eth::prepare_message(&encoded.raw).serialize());
        let mut r=[0;32];r.copy_from_slice(&signature.0[..32]);let mut s=[0;32];s.copy_from_slice(&signature.0[32..64]);
        let proof=eth_bridge::offchain::SignatureParams {r,s,v:signature.0[64]};
        let call=RuntimeCall::EthBridge(eth_bridge::Call::approve_request {ocw_public:peer.public(),hash,signature_params:proof.clone(),network_id:0});
        let transaction=|call:RuntimeCall| {
            let extra:SignedExtra=(frame_system::AuthorizeCall::new(),frame_system::CheckSpecVersion::<Runtime>::new(),frame_system::CheckTxVersion::<Runtime>::new(),frame_system::CheckGenesis::<Runtime>::new(),frame_system::CheckEra::<Runtime>::from(Era::Immortal),frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),frame_system::CheckWeight::<Runtime>::new(),crate::charge_tx_payment_extension());
            let payload=crate::SignedPayload::new(call.clone(),extra.clone()).unwrap();
            let signature=payload.using_encoded(|payload|peer.sign(payload));
            UncheckedExtrinsic::new_signed(call,who.clone(),Signature::Ecdsa(signature),extra)
        };
        let mut invalid_proof=proof;invalid_proof.r[0]^=1;
        let invalid=RuntimeCall::EthBridge(eth_bridge::Call::approve_request {ocw_public:peer.public(),hash,signature_params:invalid_proof,network_id:0});
        assert_eq!(Executive::apply_extrinsic(transaction(invalid)),Err(sp_runtime::transaction_validity::InvalidTransaction::Call.into()));
        assert_eq!(System::account_nonce(&who),0);
        frame_system::BlockWeight::<Runtime>::kill();frame_system::BlockSize::<Runtime>::kill();System::reset_events();
        assert_ok!(Executive::validate_transaction(sp_runtime::transaction_validity::TransactionSource::External,transaction(call.clone()),Default::default()));
        assert_ok!(Executive::apply_extrinsic(transaction(call.clone())).unwrap());
        assert_eq!(Balances::total_balance(&who),0);assert_eq!(System::account_nonce(&who),1);
        assert_eq!(eth_bridge::RequestApprovers::<Runtime>::get(0,hash).len(),1);
        assert!(System::events().iter().any(|record|matches!(&record.event,RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid {who:paid,actual_fee:0,tip:0}) if paid==&who)));
        frame_system::BlockWeight::<Runtime>::kill();frame_system::BlockSize::<Runtime>::kill();
        assert_eq!(Executive::apply_extrinsic(transaction(call)),Err(sp_runtime::transaction_validity::InvalidTransaction::Call.into()));
        assert_eq!(System::account_nonce(&who),1);assert_eq!(eth_bridge::RequestApprovers::<Runtime>::get(0,hash).len(),1);
    });
}

#[test]
fn outgoing_approval_retains_validation_weight_before_and_at_quorum() {
    use eth_bridge::requests::{
        AssetKind, OffchainRequest, OutgoingRequest, OutgoingTransfer, RequestStatus,
    };
    use sp_runtime::{traits::IdentifyAccount, MultiSigner};

    ext().execute_with(|| {
        System::set_block_number(1);
        let peers = [
            sp_core::ecdsa::Pair::from_seed(&[81; 32]),
            sp_core::ecdsa::Pair::from_seed(&[82; 32]),
        ];
        let accounts = peers
            .iter()
            .map(|peer| MultiSigner::Ecdsa(peer.public()).into_account())
            .collect::<Vec<AccountId>>();
        for who in &accounts {
            System::inc_providers(who);
            assert_eq!(Balances::total_balance(who), 0);
        }
        eth_bridge::Peers::<Runtime>::insert(
            0,
            accounts.iter().cloned().collect::<std::collections::BTreeSet<_>>(),
        );
        assert!(EthBridge::pending_peer(0).is_none());
        seed_existing_pswap_bridge_mapping();
        // Back a native-asset outgoing transfer so the second approval completes it.
        let mut asset_key = frame_support::storage::storage_prefix(
            b"EthBridge", b"RegisteredAsset",
        ).to_vec();
        asset_key.extend(<frame_support::Twox64Concat as frame_support::StorageHasher>::hash(
            &0u32.encode(),
        ));
        asset_key.extend(PSWAP.encode());
        sp_io::storage::set(&asset_key, &AssetKind::Thischain.encode());
        assert_eq!(EthBridge::registered_asset(0, PSWAP), Some(AssetKind::Thischain));
        let bridge = EthBridge::bridge_account(0).unwrap();
        assert_ok!(Assets::update_balance(
            RuntimeOrigin::root(), bridge.clone(), PSWAP, balance!(1) as i128
        ));
        assert_ok!(Assets::reserve(&PSWAP, &bridge, balance!(1)));
        let request = OutgoingRequest::Transfer(OutgoingTransfer::<Runtime> {
            from: AccountId::from([83; 32]),
            to: H160::repeat_byte(1),
            asset_id: PSWAP,
            amount: balance!(1),
            nonce: 0,
            network_id: 0,
            timepoint: Default::default(),
        });
        let stored = OffchainRequest::outgoing(request.clone());
        let hash = stored.hash();
        eth_bridge::Requests::<Runtime>::insert(0, hash, stored);
        eth_bridge::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
        eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![hash]);
        let eth_bridge::requests::OutgoingRequestEncoded::Transfer(encoded) =
            request.to_eth_abi(hash).unwrap()
        else {
            panic!("transfer fixture")
        };

        for (index, (peer, who)) in peers.iter().zip(&accounts).enumerate() {
            let signature = peer.sign_prehashed(
                &common::eth::prepare_message(&encoded.raw).serialize(),
            );
            let mut r = [0; 32];
            r.copy_from_slice(&signature.0[..32]);
            let mut s = [0; 32];
            s.copy_from_slice(&signature.0[32..64]);
            let call = RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
                ocw_public: peer.public(),
                hash,
                signature_params: eth_bridge::offchain::SignatureParams {
                    r, s, v: signature.0[64],
                },
                network_id: 0,
            });
            let declared_weight = call.get_dispatch_info().call_weight;
            let class = call.get_dispatch_info().class;
            let extra: SignedExtra = (
                frame_system::AuthorizeCall::new(),
                frame_system::CheckSpecVersion::<Runtime>::new(),
                frame_system::CheckTxVersion::<Runtime>::new(),
                frame_system::CheckGenesis::<Runtime>::new(),
                frame_system::CheckEra::<Runtime>::from(Era::Immortal),
                frame_system::CheckNonce::<Runtime>::from(System::account_nonce(who)),
                frame_system::CheckWeight::<Runtime>::new(),
                crate::charge_tx_payment_extension(),
            );
            let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
            let signature = payload.using_encoded(|payload| peer.sign(payload));
            let transaction = UncheckedExtrinsic::new_signed(
                call, who.clone(), Signature::Ecdsa(signature), extra,
            );
            frame_system::BlockWeight::<Runtime>::kill();
            frame_system::BlockSize::<Runtime>::kill();
            System::reset_events();
            assert_ok!(Executive::apply_extrinsic(transaction).unwrap());
            let dispatch = if index == 0 {
                <<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request()
            } else {
                <<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request_finalize()
            };
            let admission = <<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request()
                .max(<<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request_finalize())
                .saturating_mul(2)
                .saturating_add(BridgeMultisig::protocol_validation_weight())
                .saturating_add(EthBridge::approval_reconciliation_weight_for(index as u32).saturating_mul(3));
            let charged = *System::block_weight().get(class);
            assert!(
                charged.all_gte(admission.saturating_add(dispatch)),
                "actual weight must retain validation, preparation and dispatch work"
            );
            assert!(
                charged.ref_time() < declared_weight.ref_time(),
                "CheckWeight must reclaim unused worst-case signature recovery work"
            );
            assert_eq!(Balances::total_balance(who), 0);
            assert_eq!(System::account_nonce(who), 1);
            assert_eq!(eth_bridge::RequestApprovers::<Runtime>::get(0, hash).len(), index + 1);
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                Some(if index == 0 { RequestStatus::Pending } else { RequestStatus::ApprovalsReady })
            );
            assert!(System::events().iter().any(|record| matches!(
                &record.event,
                RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid {
                    who: paid, actual_fee: 0, tip: 0,
                }) if paid == who
            )));
        }
        assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
    });
}

fn setup_four_peers() -> (Vec<sr25519::Pair>, AccountId) {
    let (_, id) = setup();
    let pairs = (101..105)
        .map(|seed| sr25519::Pair::from_seed(&[seed; 32]))
        .collect::<Vec<_>>();
    for pair in &pairs {
        System::inc_providers(&account(pair));
        assert_eq!(Balances::total_balance(&account(pair)), 0);
    }
    set_bridge_members(&id, &pairs);
    assert_eq!(BridgeMultisig::accounts(&id).unwrap().threshold_num(), 3);
    (pairs, id)
}

fn set_bridge_members(id: &AccountId, pairs: &[sr25519::Pair]) {
    eth_bridge::Peers::<Runtime>::insert(
        0,
        pairs
            .iter()
            .map(account)
            .collect::<std::collections::BTreeSet<_>>(),
    );
    bridge_multisig::Accounts::<Runtime>::insert(
        id,
        bridge_multisig::MultisigAccount::new(pairs.iter().map(account).collect()),
    );
}

fn apply_zero_xor(call: RuntimeCall, pair: &sr25519::Pair) {
    let who = account(pair);
    let nonce = System::account_nonce(&who);
    assert_eq!(Balances::total_balance(&who), 0);
    assert_ok!(apply(call, pair, true));
    assert_eq!(Balances::total_balance(&who), 0);
    assert_eq!(System::account_nonce(&who), nonce + 1);
}

fn rejected_from_pool_and_block(call: RuntimeCall, pair: &sr25519::Pair) {
    rejected_with_reason(
        call,
        pair,
        sp_runtime::transaction_validity::InvalidTransaction::Call,
    );
}

fn rejected_with_reason(
    call: RuntimeCall,
    pair: &sr25519::Pair,
    reason: sp_runtime::transaction_validity::InvalidTransaction,
) {
    use sp_runtime::transaction_validity::TransactionSource;
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    // Executive initializes a validation block; the runtime API caller normally
    // discards that overlay before including an extrinsic in the actual block.
    sp_io::storage::start_transaction();
    let validity = Executive::validate_transaction(
        TransactionSource::External,
        signed(call.clone(), pair),
        Default::default(),
    );
    sp_io::storage::rollback_transaction();
    assert_eq!(validity, Err(reason.clone().into()));
    let nonce = System::account_nonce(&account(pair));
    System::reset_events();
    assert_eq!(
        Executive::apply_extrinsic(signed(call, pair)),
        Err(reason.into())
    );
    assert_eq!(System::account_nonce(&account(pair)), nonce);
    assert!(System::events().is_empty());
}

fn cancellation(
    hash: [u8; 32],
    timepoint: bridge_multisig::BridgeTimepoint<crate::BlockNumber>,
) -> RuntimeCall {
    RuntimeCall::EthBridge(eth_bridge::Call::cancel_pending_multisig {
        network_id: 0,
        call_hash: hash,
        timepoint,
    })
}

#[test]
fn one_zero_xor_peer_cannot_exhaust_other_peers_proposal_capacity() {
    ext().execute_with(|| {
        let (pairs, id) = setup_four_peers();
        let proposer = account(&pairs[0]);
        for byte in 100..132 {
            let (call, _, _) = incoming(&id, false, byte);
            apply_zero_xor(multi(&id, &call, byte), &pairs[0]);
        }
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            32
        );
        assert_eq!(
            bridge_multisig::PendingOperationsByProposer::<Runtime>::get(&id, &proposer),
            32
        );
        let (excess, _, _) = incoming(&id, false, 132);
        rejected_from_pool_and_block(multi(&id, &excess, 132), &pairs[0]);
        assert_eq!(System::account_nonce(&proposer), 32);

        let (honest, _, _) = incoming(&id, false, 200);
        apply_zero_xor(multi(&id, &honest, 200), &pairs[1]);
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            33
        );
        assert_eq!(
            bridge_multisig::PendingOperationsByProposer::<Runtime>::get(&id, account(&pairs[1])),
            1
        );

        // The existing owner cancellation remains available and releases only
        // that proposer's slot. Reopening cannot bypass the same fair quota.
        let (first, _, _) = incoming(&id, false, 100);
        let hash = sp_io::hashing::blake2_256(&first.encode());
        apply_zero_xor(
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::cancel_as_multi {
                id: id.clone(),
                timepoint: bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 100),
                call_hash: hash,
            }),
            &pairs[0],
        );
        assert_eq!(
            bridge_multisig::PendingOperationsByProposer::<Runtime>::get(&id, &proposer),
            31
        );
        apply_zero_xor(multi(&id, &first, 100), &pairs[0]);
        assert_eq!(
            bridge_multisig::PendingOperationsByProposer::<Runtime>::get(&id, &proposer),
            32
        );
        rejected_from_pool_and_block(multi(&id, &excess, 132), &pairs[0]);
    });
}

#[test]
fn current_zero_xor_quorum_cleans_orphaned_proposal_at_full_shared_capacity() {
    ext().execute_with(|| {
        let (pairs, id) = setup_four_peers();
        let proposer = account(&pairs[0]);
        let (call, incoming_hash, recipient) = incoming(&id, false, 211);
        let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 211);
        let hash = sp_io::hashing::blake2_256(&call.encode());
        apply_zero_xor(multi(&id, &call, 211), &pairs[0]);

        // Model a full backlog from the earlier global-only accounting. The
        // target is a current counted operation, so its proposer slot must also
        // be released, while the other old entries have no proposer marker.
        for byte in 0u8..127 {
            let old_hash = [byte; 32];
            assert_ne!(old_hash, hash);
            bridge_multisig::Multisigs::<Runtime>::insert(
                &id,
                old_hash,
                bridge_multisig::Multisig {
                    when: timepoint,
                    deposit: 999,
                    depositor: proposer.clone(),
                    approvals: vec![proposer.clone()],
                },
            );
            bridge_multisig::CountedNewOperations::<Runtime>::insert(&id, old_hash, ());
        }
        bridge_multisig::NewPendingOperations::<Runtime>::insert(&id, 128);
        bridge_multisig::Calls::<Runtime>::remove(hash);
        // A different multisig can leave a global dispatch marker for the same
        // payload/timepoint. It must not strand this account's pending slot.
        bridge_multisig::DispatchedCalls::<Runtime>::insert(hash, timepoint, ());
        eth_bridge::LoadToIncomingRequestHash::<Runtime>::insert(
            0,
            H256::repeat_byte(211),
            incoming_hash,
        );
        eth_bridge::RequestStatuses::<Runtime>::insert(
            0,
            incoming_hash,
            eth_bridge::requests::RequestStatus::Failed(sp_runtime::DispatchError::CannotLookup),
        );
        assert!(EthBridge::validate_peer_protocol_call(
            &id,
            match &call {
                RuntimeCall::EthBridge(protocol) => protocol,
                _ => panic!("protocol fixture"),
            }
        )
        .is_err());
        assert!(!bridge_multisig::Calls::<Runtime>::contains_key(hash));
        set_bridge_members(&id, &pairs[1..]);
        let credit_before = Currencies::free_balance(PSWAP, &recipient);

        let (honest, _, _) = incoming(&id, false, 212);
        rejected_from_pool_and_block(multi(&id, &honest, 212), &pairs[1]);
        let outsider = sr25519::Pair::from_seed(&[110; 32]);
        System::inc_providers(&account(&outsider));
        rejected_from_pool_and_block(cancellation(hash, timepoint), &outsider);
        rejected_from_pool_and_block(
            cancellation(
                hash,
                bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 212),
            ),
            &pairs[1],
        );
        rejected_from_pool_and_block(cancellation(hash, timepoint), &pairs[0]);

        for peer in &pairs[1..3] {
            apply_zero_xor(cancellation(hash, timepoint), peer);
            assert_eq!(
                bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
                128
            );
            assert_eq!(
                bridge_multisig::PendingOperationsByProposer::<Runtime>::get(&id, &proposer),
                1
            );
            assert!(bridge_multisig::Multisigs::<Runtime>::contains_key(
                &id, hash
            ));
        }
        rejected_from_pool_and_block(cancellation(hash, timepoint), &pairs[1]);
        apply_zero_xor(cancellation(hash, timepoint), &pairs[3]);
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            127
        );
        assert_eq!(
            bridge_multisig::PendingOperationsByProposer::<Runtime>::get(&id, &proposer),
            0
        );
        assert!(!bridge_multisig::Multisigs::<Runtime>::contains_key(
            &id, hash
        ));
        assert!(!bridge_multisig::CountedNewOperations::<Runtime>::contains_key(&id, hash));
        assert!(!bridge_multisig::ProposerCountedOperations::<Runtime>::contains_key(&id, hash));
        assert!(!bridge_multisig::CancellationApprovals::<Runtime>::contains_key(&id, hash));
        assert!(BridgeMultisig::was_dispatched(&id, &hash, timepoint));
        assert_eq!(Currencies::free_balance(PSWAP, &recipient), credit_before);
        assert_eq!(
            eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(0, H256::repeat_byte(211)),
            incoming_hash
        );
        assert_eq!(
            eth_bridge::RequestStatuses::<Runtime>::get(0, incoming_hash),
            Some(eth_bridge::requests::RequestStatus::Failed(
                sp_runtime::DispatchError::CannotLookup
            ))
        );
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::BridgeMultisig(bridge_multisig::Event::MultisigExecuted(..))
        )));
        rejected_from_pool_and_block(cancellation(hash, timepoint), &pairs[3]);
        apply_zero_xor(multi(&id, &honest, 212), &pairs[1]);
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            128
        );
    });
}

#[test]
fn cancellation_quorum_discards_removed_members_votes() {
    ext().execute_with(|| {
        let (mut pairs, id) = setup_four_peers();
        let (call, _, recipient) = incoming(&id, false, 213);
        let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 213);
        let hash = sp_io::hashing::blake2_256(&call.encode());
        apply_zero_xor(multi(&id, &call, 213), &pairs[0]);
        apply_zero_xor(cancellation(hash, timepoint), &pairs[1]);

        pairs.push(sr25519::Pair::from_seed(&[105; 32]));
        System::inc_providers(&account(&pairs[4]));
        let current = [
            pairs[0].clone(),
            pairs[2].clone(),
            pairs[3].clone(),
            pairs[4].clone(),
        ];
        set_bridge_members(&id, &current);
        rejected_from_pool_and_block(cancellation(hash, timepoint), &pairs[1]);
        for (index, peer) in pairs[2..4].iter().enumerate() {
            apply_zero_xor(cancellation(hash, timepoint), peer);
            let (_, votes) =
                bridge_multisig::CancellationApprovals::<Runtime>::get(&id, hash).unwrap();
            assert_eq!(votes.len(), index + 1);
            assert!(!votes.contains(&account(&pairs[1])));
            assert!(bridge_multisig::Multisigs::<Runtime>::contains_key(
                &id, hash
            ));
            assert_eq!(
                bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
                1
            );
        }
        apply_zero_xor(cancellation(hash, timepoint), &pairs[4]);
        assert!(!bridge_multisig::Multisigs::<Runtime>::contains_key(
            &id, hash
        ));
        assert!(!bridge_multisig::CancellationApprovals::<Runtime>::contains_key(&id, hash));
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            0
        );
        assert_eq!(Currencies::free_balance(PSWAP, &recipient), 0);
    });
}

#[test]
fn completed_protocol_operation_rejects_cancellation_replay() {
    ext().execute_with(|| {
        let (pairs, id) = setup_four_peers();
        let (call, _, recipient) = incoming(&id, true, 214);
        let timepoint = bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 214);
        let hash = sp_io::hashing::blake2_256(&call.encode());
        let before = Currencies::free_balance(PSWAP, &recipient);
        for peer in &pairs[..3] {
            apply_zero_xor(multi(&id, &call, 214), peer);
        }
        assert!(BridgeMultisig::was_dispatched(&id, &hash, timepoint));
        assert!(!bridge_multisig::Multisigs::<Runtime>::contains_key(
            &id, hash
        ));
        assert_eq!(
            bridge_multisig::NewPendingOperations::<Runtime>::get(&id),
            0
        );
        assert_eq!(
            Currencies::free_balance(PSWAP, &recipient),
            before + balance!(10)
        );
        rejected_from_pool_and_block(cancellation(hash, timepoint), &pairs[3]);
        assert_eq!(
            Currencies::free_balance(PSWAP, &recipient),
            before + balance!(10)
        );
    });
}

#[test]
fn wrong_timepoint_or_unknown_hash_cannot_poison_a_bridge_operation() {
    use frame_support::assert_noop;
    ext().execute_with(|| {
        let (pairs, id) = setup();
        let (call, _, recipient) = incoming(&id, true, 241);
        let hash = sp_io::hashing::blake2_256(&call.encode());
        let canonical = BridgeMultisig::sidechain_timepoint(10, 241);
        let wrong = BridgeMultisig::sidechain_timepoint(10, 242);
        let weight = call.get_dispatch_info().total_weight();
        // A paid, hash-only first vote used to install the attacker's timepoint.
        // It now needs the authenticated preimage before any storage is changed.
        assert_noop!(
            BridgeMultisig::approve_as_multi(
                RuntimeOrigin::signed(account(&pairs[0])),
                id.clone(),
                Some(wrong),
                hash,
                weight
            ),
            frame_system::Error::<Runtime>::CallFiltered
        );
        for point in [None, Some(wrong)] {
            assert_noop!(
                BridgeMultisig::as_multi(
                    RuntimeOrigin::signed(account(&pairs[0])),
                    id.clone(),
                    point,
                    call.encode(),
                    true,
                    weight
                ),
                bridge_multisig::Error::<Runtime>::WrongTimepoint
            );
        }
        rejected_from_pool_and_block(multi(&id, &call, 242), &pairs[0]);
        let before = Currencies::free_balance(PSWAP, &recipient);
        apply_zero_xor(multi(&id, &call, 241), &pairs[0]);
        assert_noop!(
            BridgeMultisig::approve_as_multi(
                RuntimeOrigin::signed(account(&pairs[1])),
                id.clone(),
                Some(wrong),
                hash,
                weight
            ),
            bridge_multisig::Error::<Runtime>::WrongTimepoint
        );
        // Historical hash-only continuation is retained when its stored body is
        // available and the original deterministic timepoint is correct.
        apply_zero_xor(
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::approve_as_multi {
                id: id.clone(),
                maybe_timepoint: Some(canonical),
                call_hash: hash,
                max_weight: weight,
            }),
            &pairs[1],
        );
        assert_eq!(
            Currencies::free_balance(PSWAP, &recipient),
            before + balance!(10)
        );
        assert!(BridgeMultisig::was_dispatched(&id, &hash, canonical));
        assert!(!bridge_multisig::DispatchedCalls::<Runtime>::contains_key(
            hash, canonical
        ));
    });
}

#[test]
fn canonical_multisig_timepoints_cover_each_incoming_protocol_operation() {
    use bridge_multisig::OperationFilter;
    type Filter = <Runtime as bridge_multisig::Config>::OperationFilter;
    ext().execute_with(|| {
        let (_, id) = setup();
        let (import, hash, _) = incoming(&id, true, 243);
        let canonical = BridgeMultisig::sidechain_timepoint(10, 243);
        let wrong = BridgeMultisig::sidechain_timepoint(10, 244);
        let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            incoming_request_result: Ok(request),
            ..
        }) = &import
        else {
            panic!("fixture")
        };
        let register = RuntimeCall::EthBridge(eth_bridge::Call::register_incoming_request {
            incoming_request: request.clone(),
        });
        for call in [&import, &register] {
            assert_ok!(Filter::validate(&id, Some(canonical), Some(call)));
            assert_eq!(
                Filter::validate(&id, Some(wrong), Some(call)),
                Err(bridge_multisig::Error::<Runtime>::WrongTimepoint.into())
            );
            assert_eq!(
                Filter::validate(&AccountId::from([244; 32]), Some(canonical), Some(call)),
                Err(frame_system::Error::<Runtime>::CallFiltered.into())
            );
        }
        assert_ok!(EthBridge::register_incoming_request(
            RuntimeOrigin::signed(id.clone()),
            request.clone()
        ));
        for call in [
            RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                hash,
                network_id: 0,
            }),
            RuntimeCall::EthBridge(eth_bridge::Call::abort_request {
                hash,
                network_id: 0,
                error: sp_runtime::DispatchError::CannotLookup,
            }),
        ] {
            assert_ok!(Filter::validate(&id, Some(canonical), Some(&call)));
            assert_eq!(
                Filter::validate(&id, Some(wrong), Some(&call)),
                Err(bridge_multisig::Error::<Runtime>::WrongTimepoint.into())
            );
        }
    });
}

#[test]
fn captured_nonbridge_multisigs_cannot_dispatch_protocol_calls_or_add_cosigners() {
    use codec::DecodeAll;
    use frame_support::assert_noop;
    ext().execute_with(|| {
        let (_, id) = setup();
        let (protocol, _, _) = incoming(&id, true, 245);
        let canonical = BridgeMultisig::sidechain_timepoint(10, 245);
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/bridge-multisig-accounts-checkpoint.json"
        ))
        .unwrap();
        let bytes = |value: &str| -> Vec<u8> {
            value
                .strip_prefix("0x")
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        };
        let mut checked = 0;
        for row in fixture["accounts"].as_array().unwrap() {
            let raw = bytes(row["value"].as_str().unwrap());
            let membership =
                bridge_multisig::MultisigAccount::<AccountId>::decode_all(&mut &raw[..]).unwrap();
            if membership.signatories.len() != 1 {
                continue;
            }
            let account_bytes: [u8; 32] =
                bytes(row["account"].as_str().unwrap()).try_into().unwrap();
            let foreign = AccountId::from(account_bytes);
            let key = bytes(row["key"].as_str().unwrap());
            assert_eq!(
                bridge_multisig::Accounts::<Runtime>::hashed_key_for(&foreign),
                key
            );
            sp_io::storage::set(&key, &raw);
            let signer = membership.signatories[0].clone();
            assert_noop!(
                BridgeMultisig::as_multi_threshold_1(
                    RuntimeOrigin::signed(signer.clone()),
                    foreign.clone(),
                    Box::new(protocol.clone()),
                    canonical
                ),
                frame_system::Error::<Runtime>::CallFiltered
            );
            let add_cosigner = RuntimeCall::BridgeMultisig(bridge_multisig::Call::add_signatory {
                new_member: AccountId::from([246; 32]),
            });
            assert_noop!(
                BridgeMultisig::as_multi_threshold_1(
                    RuntimeOrigin::signed(signer),
                    foreign.clone(),
                    Box::new(add_cosigner),
                    canonical
                ),
                frame_system::Error::<Runtime>::CallFiltered
            );
            assert_eq!(sp_io::storage::get(&key).unwrap().as_ref(), raw.as_slice());
            checked += 1;
        }
        assert_eq!(checked, 2);
    });
}

#[test]
fn cancelling_prepared_incoming_restores_accounting_when_outbound_price_is_unavailable() {
    use common::AssetInfoProvider;
    use frame_support::assert_noop;
    ext().execute_with(|| {
        let (_, id) = setup();
        let (import, _, recipient) = incoming(&id, true, 247);
        let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
            incoming_request_result: Ok(request),
            ..
        }) = import
        else {
            panic!("fixture")
        };
        let network = bridge_types::GenericNetworkId::EVMLegacy(0);
        let asset = crate::AssetId::from(PSWAP);
        // Complete native-asset registration so the genuine outgoing path
        // reaches price validation instead of failing on an unknown asset.
        assert_ok!(eth_bridge::requests::OutgoingAddAsset::<Runtime> {
            author: id.clone(),
            asset_id: asset,
            nonce: 0,
            network_id: 0,
            timepoint: BridgeMultisig::sidechain_timepoint(10, 246),
        }
        .finalize());
        assert_eq!(
            EthBridge::registered_asset(0, asset),
            Some(eth_bridge::requests::AssetKind::Thischain)
        );
        let locked_before = BridgeProxy::locked_assets(network, asset);
        let available_before = Assets::free_balance(&asset, &id).unwrap();
        assert_ok!(request.prepare());
        assert_eq!(
            BridgeProxy::locked_assets(network, asset),
            locked_before - balance!(10)
        );
        assert_eq!(
            Assets::free_balance(&asset, &id).unwrap(),
            available_before - balance!(10)
        );
        if !BridgeProxy::is_asset_limited(asset) {
            assert_ok!(BridgeProxy::add_limited_asset(RuntimeOrigin::root(), asset));
        }
        price_tools::PriceInfos::<Runtime>::remove(asset);
        let consumed = BridgeProxy::consumed_transfer_limit();
        // The genesis transfer-limit window is 600 blocks.
        let unlock_at = System::block_number() + 600;
        let scheduled = BridgeProxy::transfer_limit_unlock_schedule(unlock_at);
        let outgoing = eth_bridge::requests::OutgoingTransfer::<Runtime> {
            from: recipient,
            to: H160::repeat_byte(247),
            asset_id: asset,
            amount: balance!(10),
            nonce: 0,
            network_id: 0,
            timepoint: BridgeMultisig::sidechain_timepoint(10, 247),
        };
        assert_noop!(
            outgoing.prepare(H256::repeat_byte(247)),
            price_tools::Error::<Runtime>::UnsupportedQuotePath
        );
        assert_ok!(request.cancel());
        assert_eq!(BridgeProxy::locked_assets(network, asset), locked_before);
        assert_eq!(Assets::free_balance(&asset, &id).unwrap(), available_before);
        assert_eq!(BridgeProxy::consumed_transfer_limit(), consumed);
        assert_eq!(
            BridgeProxy::transfer_limit_unlock_schedule(unlock_at),
            scheduled
        );
    });
}

#[test]
fn actual_approval_weight_reclaims_enough_for_valid_peer_submissions_beyond_declared_block_capacity(
) {
    use common::AssetInfoProvider;
    use eth_bridge::requests::{OutgoingRequest, RequestStatus};
    use sp_runtime::{traits::IdentifyAccount, MultiSigner};

    ext().execute_with(|| {
        System::set_block_number(1);
        seed_existing_pswap_bridge_mapping();
        let peers = [
            sp_core::ecdsa::Pair::from_seed(&[85; 32]),
            sp_core::ecdsa::Pair::from_seed(&[86; 32]),
        ];
        let accounts: Vec<AccountId> = peers
            .iter()
            .map(|peer| MultiSigner::Ecdsa(peer.public()).into_account())
            .collect();
        for who in &accounts {
            System::inc_providers(who);
            assert_eq!(Balances::total_balance(who), 0);
        }
        eth_bridge::Peers::<Runtime>::insert(
            0,
            accounts.iter().cloned().collect::<std::collections::BTreeSet<_>>(),
        );
        assert!(EthBridge::pending_peer(0).is_none());
        let sender = AccountId::from([87; 32]);
        System::inc_providers(&sender);
        assert_ok!(Assets::update_balance(
            RuntimeOrigin::root(), sender.clone(), PSWAP,
            balance!(1) as i128,
        ));
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(sender.clone()), PSWAP,
            H160::repeat_byte(30), balance!(1), 0,
        ));
        let first_hash = eth_bridge::RequestsQueue::<Runtime>::get(0)[0];
        let first_info = RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
            ocw_public: peers[0].public(), hash: first_hash,
            signature_params: eth_bridge::offchain::SignatureParams { r: [0; 32], s: [0; 32], v: 0 },
            network_id: 0,
        }).get_dispatch_info();
        let normal_limit = crate::BlockWeights::get().get(first_info.class).max_total.unwrap();
        // Derive the workload from a real stored transfer's declaration. A
        // calibrated faster recovery implementation must not make this test
        // accidentally fit without post-dispatch weight reclamation.
        let required_submissions = normal_limit.ref_time() / first_info.call_weight.ref_time() + 2;
        let request_count = required_submissions.div_ceil(2).max(6) as usize;
        assert!(request_count <= 100, "calibrated workload exceeds the fixture bound");
        assert_ok!(Assets::update_balance(
            RuntimeOrigin::root(), sender.clone(), PSWAP,
            (balance!(1) * (request_count - 1) as u128) as i128,
        ));
        // Use actual preparation: custody and reservation must support every
        // quorum finalization instead of substituting synthetic request rows.
        for index in 1..request_count {
            assert_ok!(EthBridge::transfer_to_sidechain(
                RuntimeOrigin::signed(sender.clone()), PSWAP,
                H160::repeat_byte(index as u8 + 30), balance!(1), 0,
            ));
        }
        let requests = eth_bridge::RequestsQueue::<Runtime>::get(0);
        assert_eq!(requests.len(), request_count);
        assert_eq!(Assets::free_balance(&PSWAP, &sender).unwrap(), 0);
        frame_system::BlockWeight::<Runtime>::kill();
        frame_system::BlockSize::<Runtime>::kill();
        System::reset_events();
        let mut declared_sum = frame_support::weights::Weight::zero();
        for (peer_index, (peer, who)) in peers.iter().zip(&accounts).enumerate() {
            for hash in &requests {
                let request = eth_bridge::Requests::<Runtime>::get(0, hash)
                    .unwrap().into_outgoing().unwrap().0;
                let OutgoingRequest::Transfer(_) = &request else {
                    panic!("prepared withdrawal fixture")
                };
                let eth_bridge::requests::OutgoingRequestEncoded::Transfer(encoded) =
                    request.to_eth_abi(*hash).unwrap()
                else {
                    panic!("prepared withdrawal encoding")
                };
                let proof = peer.sign_prehashed(
                    &common::eth::prepare_message(&encoded.raw).serialize(),
                );
                let mut r = [0; 32];
                r.copy_from_slice(&proof.0[..32]);
                let mut s = [0; 32];
                s.copy_from_slice(&proof.0[32..64]);
                let call = RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
                    ocw_public: peer.public(), hash: *hash,
                    signature_params: eth_bridge::offchain::SignatureParams { r, s, v: proof.0[64] },
                    network_id: 0,
                });
                let info = call.get_dispatch_info();
                assert_eq!(info.pays_fee, Pays::Yes);
                assert_eq!(info.class, first_info.class);
                assert_eq!(info.call_weight, first_info.call_weight);
                declared_sum = declared_sum.saturating_add(info.call_weight);
                let extra: SignedExtra = (
                    frame_system::AuthorizeCall::new(),
                    frame_system::CheckSpecVersion::<Runtime>::new(),
                    frame_system::CheckTxVersion::<Runtime>::new(),
                    frame_system::CheckGenesis::<Runtime>::new(),
                    frame_system::CheckEra::<Runtime>::from(Era::Immortal),
                    frame_system::CheckNonce::<Runtime>::from(System::account_nonce(who)),
                    frame_system::CheckWeight::<Runtime>::new(),
                    crate::charge_tx_payment_extension(),
                );
                let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
                let signature = payload.using_encoded(|payload| peer.sign(payload));
                let transaction = UncheckedExtrinsic::new_signed(
                    call, who.clone(), Signature::Ecdsa(signature), extra,
                );
                let before = *System::block_weight().get(info.class);
                // Do not clear the weight or length counters between submissions.
                // Without real CheckWeight reclamation, declared_sum exhausts
                // the block before this sequence can complete.
                assert_ok!(Executive::apply_extrinsic(transaction).unwrap());
                let after = *System::block_weight().get(info.class);
                let charged = after.saturating_sub(before);
                let dispatch_floor = <<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request()
                    .min(<<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request_finalize());
                let admission_floor = <<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request()
                    .max(<<Runtime as eth_bridge::Config>::WeightInfo as eth_bridge::weights::WeightInfo>::approve_request_finalize())
                    .saturating_mul(2)
                    .saturating_add(BridgeMultisig::protocol_validation_weight())
                    .saturating_add(EthBridge::approval_reconciliation_weight_for(peer_index as u32).saturating_mul(3));
                assert!(charged.all_gte(admission_floor.saturating_add(dispatch_floor)));
                assert!(charged.ref_time() < info.call_weight.ref_time());
                assert!(after.all_lte(normal_limit));
                assert_eq!(eth_bridge::RequestApprovers::<Runtime>::get(0, hash).len(), peer_index + 1);
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(if peer_index == 0 { RequestStatus::Pending } else { RequestStatus::ApprovalsReady }),
                );
            }
        }
        assert!(
            declared_sum.ref_time() > normal_limit.ref_time(),
            "this sequence must exceed one block if worst-case declarations are retained"
        );
        assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
        for who in &accounts {
            assert_eq!(System::account_nonce(who), request_count as u32);
            assert_eq!(Balances::total_balance(who), 0);
            let paid = System::events().iter().filter(|record| matches!(
                &record.event,
                RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid {
                    who: account, actual_fee: 0, tip: 0,
                }) if account == who
            )).count();
            assert_eq!(paid, request_count);
        }
        println!(
            "approval block: requests={}, approvals={}, declared_ref_time={}, actual_ref_time={}, normal_limit_ref_time={}",
            request_count,
            2 * request_count,
            declared_sum.ref_time(),
            System::block_weight().get(first_info.class).ref_time(),
            normal_limit.ref_time(),
        );
    });
}
#[cfg(feature = "wip")]
mod wip_xorless_admission_regressions {
    use super::*;
    use eth_bridge::requests::{OutgoingRequestEncoded, RequestStatus};
    use sp_runtime::{
        traits::IdentifyAccount,
        transaction_validity::{InvalidTransaction, TransactionSource},
        MultiSigner,
    };

    fn wrap(mut call: RuntimeCall, depth: usize) -> RuntimeCall {
        for _ in 0..depth {
            call = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(call),
                asset_id: None,
            });
        }
        call
    }

    fn fund(who: &AccountId) {
        System::inc_providers(who);
        let _ = Balances::make_free_balance_be(who, balance!(1000000));
    }

    fn ecdsa_account(pair: &sp_core::ecdsa::Pair) -> AccountId {
        MultiSigner::Ecdsa(pair.public()).into_account()
    }

    fn ecdsa_signed(call: RuntimeCall, pair: &sp_core::ecdsa::Pair) -> UncheckedExtrinsic {
        let who = ecdsa_account(pair);
        let extra: SignedExtra = (
            frame_system::AuthorizeCall::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(Era::Immortal),
            frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),
            frame_system::CheckWeight::<Runtime>::new(),
            crate::charge_tx_payment_extension(),
        );
        let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
        let signature = payload.using_encoded(|bytes| pair.sign(bytes));
        UncheckedExtrinsic::new_signed(call, who, Signature::Ecdsa(signature), extra)
    }

    fn apply_paid_ecdsa(call: RuntimeCall, pair: &sp_core::ecdsa::Pair) {
        let who = ecdsa_account(pair);
        frame_system::BlockSize::<Runtime>::kill();
        frame_system::BlockWeight::<Runtime>::kill();
        sp_io::storage::start_transaction();
        let validity = Executive::validate_transaction(
            TransactionSource::External,
            ecdsa_signed(call.clone(), pair),
            Default::default(),
        );
        sp_io::storage::rollback_transaction();
        assert_ok!(validity);
        let balance_before = Balances::free_balance(&who);
        let nonce_before = System::account_nonce(&who);
        System::reset_events();
        assert_ok!(Executive::apply_extrinsic(ecdsa_signed(call, pair)).unwrap());
        let actual_fee = System::events()
            .iter()
            .find_map(|record| match &record.event {
                RuntimeEvent::TransactionPayment(
                    pallet_transaction_payment::Event::TransactionFeePaid {
                        who: payer,
                        actual_fee,
                        tip,
                    },
                ) if payer == &who => {
                    assert_eq!(*tip, 0);
                    Some(*actual_fee)
                }
                _ => None,
            })
            .expect("wrapper execution emits the paid fee event");
        assert!(
            actual_fee > 0,
            "a nested peer call must not exempt its wrapper"
        );
        assert!(Balances::free_balance(&who) < balance_before);
        assert_eq!(System::account_nonce(&who), nonce_before + 1);
    }

    fn prepared_outgoing() -> (Vec<sp_core::ecdsa::Pair>, H256) {
        System::set_block_number(1);
        seed_existing_pswap_bridge_mapping();
        assert_ok!(XorFee::update_multiplier(
            RuntimeOrigin::root(),
            FixedU128::saturating_from_integer(1)
        ));
        let peers = vec![
            sp_core::ecdsa::Pair::from_seed(&[201; 32]),
            sp_core::ecdsa::Pair::from_seed(&[202; 32]),
        ];
        for peer in &peers {
            fund(&ecdsa_account(peer));
        }
        eth_bridge::Peers::<Runtime>::insert(
            0,
            peers
                .iter()
                .map(ecdsa_account)
                .collect::<std::collections::BTreeSet<_>>(),
        );
        assert!(EthBridge::pending_peer(0).is_none());
        let sender = AccountId::from([203; 32]);
        System::inc_providers(&sender);
        assert_ok!(Assets::update_balance(
            RuntimeOrigin::root(),
            sender.clone(),
            PSWAP,
            balance!(1) as i128
        ));
        // Prepare actual custody/reservation, not a synthetic outgoing row.
        assert_ok!(EthBridge::transfer_to_sidechain(
            RuntimeOrigin::signed(sender),
            PSWAP,
            H160::repeat_byte(203),
            balance!(1),
            0
        ));
        let queue = eth_bridge::RequestsQueue::<Runtime>::get(0);
        assert_eq!(queue.len(), 1);
        (peers, queue[0])
    }

    fn proof(peer: &sp_core::ecdsa::Pair, hash: H256) -> eth_bridge::offchain::SignatureParams {
        let outgoing = eth_bridge::Requests::<Runtime>::get(0, hash)
            .unwrap()
            .into_outgoing()
            .unwrap()
            .0;
        let OutgoingRequestEncoded::Transfer(encoded) = outgoing.to_eth_abi(hash).unwrap() else {
            panic!("prepared transfer fixture")
        };
        let signature =
            peer.sign_prehashed(&common::eth::prepare_message(&encoded.raw).serialize());
        eth_bridge::offchain::SignatureParams {
            r: signature.0[..32].try_into().unwrap(),
            s: signature.0[32..64].try_into().unwrap(),
            v: signature.0[64],
        }
    }

    #[test]
    fn wip_xorless_unauthorized_smaller_protocol_calls_reject_pool_and_block_at_both_depths() {
        for depth in [1, 2] {
            ext().execute_with(|| {
                let (_, id) = setup();
                let outsider = sr25519::Pair::from_seed(&[204; 32]);
                let who = account(&outsider);
                fund(&who);
                assert!(!EthBridge::peers(0).contains(&who));
                assert_ne!(who, id);
                let (import, hash, recipient) = incoming(&id, true, 204);
                let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
                    incoming_request_result: Ok(request),
                    ..
                }) = &import
                else {
                    panic!("incoming fixture")
                };
                let register =
                    RuntimeCall::EthBridge(eth_bridge::Call::register_incoming_request {
                        incoming_request: request.clone(),
                    });
                let calls = [
                    cancellation([204; 32], BridgeMultisig::sidechain_timepoint(10, 204)),
                    register,
                    import,
                    RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                        hash,
                        network_id: 0,
                    }),
                    RuntimeCall::EthBridge(eth_bridge::Call::abort_request {
                        hash,
                        network_id: 0,
                        error: sp_runtime::DispatchError::CannotLookup,
                    }),
                ];
                let payer_before = Balances::free_balance(&who);
                let recipient_before = Currencies::free_balance(PSWAP, &recipient);
                let custody_before = Currencies::free_balance(PSWAP, &id);
                let locked_before = BridgeProxy::locked_assets(
                    bridge_types::GenericNetworkId::EVMLegacy(0),
                    crate::AssetId::from(PSWAP),
                );
                let queue_before = eth_bridge::RequestsQueue::<Runtime>::get(0);
                for call in calls {
                    // This assertion intentionally depends on the accompanying
                    // smaller-call caller checks; paid unauthorized calls must
                    // be invalid before the wrapper can dispatch anything.
                    assert_eq!(
                        crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call),
                        Err(InvalidTransaction::Call.into())
                    );
                    rejected_from_pool_and_block(wrap(call, depth), &outsider);
                    assert_eq!(Balances::free_balance(&who), payer_before);
                    assert_eq!(
                        Currencies::free_balance(PSWAP, &recipient),
                        recipient_before
                    );
                    assert_eq!(Currencies::free_balance(PSWAP, &id), custody_before);
                    assert_eq!(
                        BridgeProxy::locked_assets(
                            bridge_types::GenericNetworkId::EVMLegacy(0),
                            crate::AssetId::from(PSWAP),
                        ),
                        locked_before
                    );
                    assert_eq!(eth_bridge::RequestsQueue::<Runtime>::get(0), queue_before);
                    assert!(!eth_bridge::Requests::<Runtime>::contains_key(0, hash));
                    assert!(!eth_bridge::RequestStatuses::<Runtime>::contains_key(
                        0, hash
                    ));
                }
            });
        }
    }

    #[test]
    fn wip_xorless_valid_peer_approval_executes_and_pays_at_both_depths() {
        for depth in [1, 2] {
            ext().execute_with(|| {
                let (peers, hash) = prepared_outgoing();
                let who = ecdsa_account(&peers[0]);
                let call = RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
                    ocw_public: peers[0].public(),
                    hash,
                    signature_params: proof(&peers[0], hash),
                    network_id: 0,
                });
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call),
                    Ok(true)
                );
                let wrapped = wrap(call, depth);
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &wrapped),
                    Ok(false)
                );
                apply_paid_ecdsa(wrapped, &peers[0]);
                assert_eq!(
                    eth_bridge::RequestApprovers::<Runtime>::get(0, hash).len(),
                    1
                );
                assert!(eth_bridge::RequestApprovers::<Runtime>::get(0, hash).contains(&who));
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(RequestStatus::Pending)
                );
            });
        }
    }

    #[test]
    fn wip_xorless_valid_public_finalization_executes_and_pays_at_both_depths() {
        for depth in [1, 2] {
            ext().execute_with(|| {
                let (peers, hash) = prepared_outgoing();
                // Retained, current-author quorum awaiting a public retry. Both
                // proofs are real, and the original transfer prepared custody.
                eth_bridge::RequestApprovals::<Runtime>::insert(
                    0,
                    hash,
                    peers
                        .iter()
                        .map(|peer| proof(peer, hash))
                        .collect::<std::collections::BTreeSet<_>>(),
                );
                eth_bridge::RequestApprovers::<Runtime>::insert(
                    0,
                    hash,
                    peers
                        .iter()
                        .map(ecdsa_account)
                        .collect::<std::collections::BTreeSet<_>>(),
                );
                let who = ecdsa_account(&peers[0]);
                let call = RuntimeCall::EthBridge(eth_bridge::Call::finalize_approved_request {
                    network_id: 0,
                    hash,
                });
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call),
                    Ok(false)
                );
                let wrapped = wrap(call, depth);
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &wrapped),
                    Ok(false)
                );
                apply_paid_ecdsa(wrapped, &peers[0]);
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(RequestStatus::ApprovalsReady)
                );
                assert_eq!(
                    eth_bridge::RequestApprovers::<Runtime>::get(0, hash).len(),
                    2
                );
                assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
            });
        }
    }

    #[test]
    fn wip_xorless_stops_at_utility_and_multisig_terminals() {
        ext().execute_with(|| {
            let (_, id) = setup();
            let outsider = sr25519::Pair::from_seed(&[205; 32]);
            let who = account(&outsider);
            let (protocol, _, _) = incoming(&id, true, 205);
            assert_eq!(
                crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &protocol),
                Err(InvalidTransaction::Call.into())
            );
            // Each terminal owns its dispatch/origin rules. In particular a
            // derivative or multisig account need not equal this outer signer.
            let terminals = [
                RuntimeCall::Utility(pallet_utility::Call::batch {
                    calls: vec![protocol.clone()],
                }),
                RuntimeCall::Utility(pallet_utility::Call::batch_all {
                    calls: vec![protocol.clone()],
                }),
                RuntimeCall::Utility(pallet_utility::Call::force_batch {
                    calls: vec![protocol.clone()],
                }),
                RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                    index: 0,
                    call: Box::new(protocol.clone()),
                }),
                RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                    other_signatories: vec![AccountId::from([206; 32])],
                    call: Box::new(protocol.clone()),
                }),
                multi(&id, &protocol, 205),
            ];
            for terminal in terminals {
                for depth in [1, 2] {
                    assert_eq!(
                        crate::xor_fee_impls::validate_bridge_fee_exemption(
                            &who,
                            &wrap(terminal.clone(), depth)
                        ),
                        Ok(false)
                    );
                }
            }
        });
    }

    #[test]
    fn wip_xorless_preserves_valid_derivative_origin_and_paid_execution() {
        for depth in [1, 2] {
            ext().execute_with(|| {
                let _ = setup();
                let signer = sr25519::Pair::from_seed(&[207; 32]);
                let who = account(&signer);
                fund(&who);
                let derived: AccountId = pallet_utility::derivative_account_id(who.clone(), 0);
                assert_ne!(derived, who);
                eth_bridge::BridgeAccount::<Runtime>::insert(0, &derived);
                let (protocol, hash, recipient) = incoming(&derived, true, 207);
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &protocol),
                    Err(InvalidTransaction::Call.into())
                );
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&derived, &protocol),
                    Ok(true)
                );
                let wrapped = wrap(
                    RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                        index: 0,
                        call: Box::new(protocol),
                    }),
                    depth,
                );
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &wrapped),
                    Ok(false)
                );
                frame_system::BlockSize::<Runtime>::kill();
                frame_system::BlockWeight::<Runtime>::kill();
                sp_io::storage::start_transaction();
                let validity = Executive::validate_transaction(
                    TransactionSource::External,
                    signed(wrapped.clone(), &signer),
                    Default::default(),
                );
                sp_io::storage::rollback_transaction();
                assert_ok!(validity);
                let payer_before = Balances::free_balance(&who);
                let recipient_before = Currencies::free_balance(PSWAP, &recipient);
                assert_ok!(apply(wrapped, &signer, false));
                assert!(Balances::free_balance(&who) < payer_before);
                assert_eq!(
                    Currencies::free_balance(PSWAP, &recipient),
                    recipient_before + balance!(10)
                );
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(RequestStatus::Done)
                );
            });
        }
    }
}

// Keep these expectations independent of finalization_declared_weight and the
// private finalization_admission_weight helper: dropping either admission or
// the capture allowance must fail against the actual signed Executive path.
mod finalize_admission_regressions {
    use super::*;
    use eth_bridge::requests::{
        IncomingTransactionRequestKind, LoadIncomingRequest, LoadIncomingTransactionRequest,
        OffchainRequest, OutgoingMigrate, OutgoingRequest, RequestStatus,
    };
    use frame_support::{
        dispatch::DispatchClass,
        storage::{transactional::with_transaction_opaque_err, TransactionOutcome},
        weights::Weight,
    };
    use sp_runtime::{
        traits::IdentifyAccount,
        transaction_validity::{InvalidTransaction, TransactionSource},
        MultiSigner, StateVersion,
    };
    use std::collections::BTreeSet;

    type BridgeWeights = <Runtime as eth_bridge::Config>::WeightInfo;

    fn finalize_admission_metadata() -> Weight {
        <Runtime as frame_system::Config>::DbWeight::get()
            .reads(1)
            .saturating_add(Weight::from_parts(0, 4096))
    }

    fn finalize_admission_expected_extra(bound: u64) -> Weight {
        let metadata_and_message =
            finalize_admission_metadata().saturating_add(EthBridge::approval_message_weight(bound));
        let one_admission = <BridgeWeights as eth_bridge::weights::WeightInfo>::approve_request()
            .max(<BridgeWeights as eth_bridge::weights::WeightInfo>::approve_request_finalize())
            .saturating_add(metadata_and_message)
            // Each admission now independently reconciles the live-domain
            // quorum. Retain maximum bounded crypto plus another complete
            // decode/ABI/hash envelope; the early rejection model stays small.
            .saturating_add(EthBridge::approval_reconciliation_weight_for(
                eth_bridge::requests::MAX_PEERS as u32,
            ))
            .saturating_add(EthBridge::approval_message_weight(bound));
        // Two independently executed signed-extension admissions, followed by
        // one full bounded host read for the dispatch-time length/tag capture.
        one_admission
            .saturating_mul(2)
            .saturating_add(metadata_and_message)
    }

    fn finalize_admission_expected_dispatch(bound: u64, abi_bytes: u64) -> Weight {
        <BridgeWeights as eth_bridge::weights::WeightInfo>::approve_request_finalize()
            .saturating_add(EthBridge::approval_reconciliation_weight_for(2))
            .saturating_add(finalize_admission_metadata())
            .saturating_add(EthBridge::approval_message_weight(abi_bytes))
            .saturating_add(BridgeMultisig::protocol_validation_weight())
            .saturating_add(finalize_admission_expected_extra(bound))
    }

    fn finalize_admission_expected_declaration(bound: u64) -> Weight {
        <BridgeWeights as eth_bridge::weights::WeightInfo>::approve_request_finalize()
            .saturating_add(EthBridge::approval_reconciliation_weight_for(
                eth_bridge::requests::MAX_PEERS as u32,
            ))
            .saturating_add(finalize_admission_metadata())
            .saturating_add(EthBridge::approval_message_weight(bound).saturating_mul(2))
            .saturating_add(BridgeMultisig::protocol_validation_weight())
            .saturating_add(finalize_admission_expected_extra(bound))
            // This expectation is for RuntimeCall metadata, which adds the
            // generated retirement-origin guard to the pallet declaration.
            .saturating_add(crate::incident_recovery_origin::weight())
    }

    fn finalize_admission_migration(
        tokens: usize,
        status: RequestStatus,
    ) -> (sr25519::Pair, H256, u64, u64, BTreeSet<AccountId>) {
        System::set_block_number(1);
        let payer = sr25519::Pair::from_seed(&[221; 32]);
        let who = account(&payer);
        let _ = Balances::make_free_balance_be(&who, balance!(1000000));
        assert_ok!(XorFee::update_multiplier(
            RuntimeOrigin::root(),
            FixedU128::saturating_from_integer(1),
        ));
        // Deterministic public test keys only. The submitter is deliberately
        // outside the peer set: finalization remains public and fee-paying.
        let peers = [
            sp_core::ecdsa::Pair::from_seed(&[231; 32]),
            sp_core::ecdsa::Pair::from_seed(&[232; 32]),
        ];
        let authors: BTreeSet<AccountId> = peers
            .iter()
            .map(|peer| MultiSigner::Ecdsa(peer.public()).into_account())
            .collect();
        assert!(!authors.contains(&who));
        eth_bridge::Peers::<Runtime>::insert(0, &authors);
        assert!(EthBridge::pending_peer(0).is_none());
        // These pallet-private maps use the same raw genesis-fixture approach
        // as the surrounding runtime tests. Assert through their public getters.
        for (name, value) in [
            (
                b"BridgeStatuses".as_slice(),
                eth_bridge::BridgeStatus::Migrating.encode(),
            ),
            (
                b"BridgeSignatureVersions".as_slice(),
                eth_bridge::BridgeSignatureVersion::V3.encode(),
            ),
        ] {
            let mut key = frame_support::storage::storage_prefix(b"EthBridge", name).to_vec();
            key.extend(
                <frame_support::Twox64Concat as frame_support::StorageHasher>::hash(&0u32.encode()),
            );
            sp_io::storage::set(&key, &value);
        }
        assert_eq!(
            EthBridge::bridge_contract_status(0),
            Some(eth_bridge::BridgeStatus::Migrating)
        );
        assert_eq!(
            EthBridge::bridge_signature_version(0),
            eth_bridge::BridgeSignatureVersion::V3
        );
        let request = OutgoingRequest::Migrate(OutgoingMigrate::<Runtime> {
            author: who,
            new_contract_address: H160::repeat_byte(42),
            erc20_native_tokens: (0..tokens)
                .map(|index| H160::from_low_u64_be(index as u64 + 1))
                .collect(),
            nonce: 0,
            network_id: 0,
            timepoint: Default::default(),
            new_signature_version: eth_bridge::BridgeSignatureVersion::V3,
        });
        let stored = OffchainRequest::outgoing(request.clone());
        let hash = stored.hash();
        let bound = 2 * stored.encoded_size() as u64 + 1024;
        eth_bridge::Requests::<Runtime>::insert(0, hash, &stored);
        eth_bridge::RequestStatuses::<Runtime>::insert(0, hash, status);
        eth_bridge::RequestsQueue::<Runtime>::insert(0, vec![hash]);
        let eth_bridge::requests::OutgoingRequestEncoded::Migrate(encoded) =
            request.to_eth_abi(hash).unwrap()
        else {
            panic!("migration fixture")
        };
        let message = common::eth::prepare_message(&encoded.raw).serialize();
        let approvals: BTreeSet<_> = peers
            .iter()
            .map(|peer| {
                let signature = peer.sign_prehashed(&message);
                let mut r = [0; 32];
                r.copy_from_slice(&signature.0[..32]);
                let mut s = [0; 32];
                s.copy_from_slice(&signature.0[32..64]);
                eth_bridge::offchain::SignatureParams {
                    r,
                    s,
                    v: signature.0[64],
                }
            })
            .collect();
        assert_eq!(approvals.len(), 2);
        // Seed a retained signed request before retry, rather than submitting
        // new approvals (which would finalize as soon as the quorum is reached).
        eth_bridge::RequestApprovals::<Runtime>::insert(0, hash, approvals);
        eth_bridge::RequestApprovers::<Runtime>::insert(0, hash, &authors);
        assert_eq!(EthBridge::approval_message_byte_bound(0, hash), bound);
        (payer, hash, bound, encoded.raw.len() as u64, authors)
    }

    fn finalize_admission_call(hash: H256) -> RuntimeCall {
        RuntimeCall::EthBridge(eth_bridge::Call::finalize_approved_request {
            network_id: 0,
            hash,
        })
    }

    #[test]
    fn finalize_admission_migration_exec_retains_both_checks_and_capture() {
        let mut previous: Option<(Weight, Weight)> = None;
        for tokens in [0, 1, 64, 4096] {
            let measured = ext().execute_with(|| {
                let (payer, hash, bound, abi_bytes, _) =
                    finalize_admission_migration(tokens, RequestStatus::Pending);
                assert_eq!(abi_bytes, 288 + 32 * tokens as u64);
                let expected = finalize_admission_expected_dispatch(bound, abi_bytes);
                let call = finalize_admission_call(hash);
                let info = call.get_dispatch_info();
                assert_eq!(info.class, DispatchClass::Normal);
                assert_eq!(
                    info.call_weight,
                    finalize_admission_expected_declaration(bound)
                );

                // Check the successful dispatch receipt exactly in a disposable
                // overlay; the subsequent signed Executive call sees the same
                // pending state and has to execute all validation/preparation.
                let before = sp_io::storage::root(StateVersion::V1);
                sp_io::storage::start_transaction();
                let result = EthBridge::finalize_approved_request(
                    RuntimeOrigin::signed(account(&payer)),
                    0,
                    hash,
                );
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(RequestStatus::ApprovalsReady),
                );
                sp_io::storage::rollback_transaction();
                assert_eq!(sp_io::storage::root(StateVersion::V1), before);
                let post = result.unwrap();
                assert_eq!(post.pays_fee, Pays::Yes);
                assert_eq!(post.actual_weight, Some(expected));

                sp_io::storage::start_transaction();
                let valid = Executive::validate_transaction(
                    TransactionSource::External,
                    signed(call.clone(), &payer),
                    Default::default(),
                );
                sp_io::storage::rollback_transaction();
                assert_ok!(valid);
                assert_eq!(sp_io::storage::root(StateVersion::V1), before);
                let encoded_len = signed(call.clone(), &payer).encoded_size() as u64;
                let balance_before = Balances::total_balance(&account(&payer));
                assert_ok!(apply(call, &payer, false));
                let charged = *System::block_weight().get(info.class);
                let success = System::events()
                    .into_iter()
                    .find_map(|record| match record.event {
                        RuntimeEvent::System(frame_system::Event::ExtrinsicSuccess {
                            dispatch_info,
                        }) => Some(dispatch_info),
                        _ => None,
                    })
                    .expect("the signed transaction must actually execute successfully");
                assert_eq!(success.class, info.class);
                assert_eq!(success.pays_fee, Pays::Yes);
                assert_eq!(
                    charged,
                    success
                        .weight
                        .saturating_add(Weight::from_parts(0, encoded_len))
                );
                let runtime_floor = expected
                    .saturating_add(crate::incident_recovery_origin::weight())
                    .saturating_add(crate::BlockWeights::get().get(info.class).base_extrinsic)
                    .saturating_add_proof_size(encoded_len);
                assert!(charged.all_gte(runtime_floor));
                assert!(
                    charged.ref_time() < info.call_weight.ref_time(),
                    "unused worst-case recoveries must still be reclaimed"
                );
                assert_eq!(System::account_nonce(&account(&payer)), 1);
                assert!(Balances::total_balance(&account(&payer)) < balance_before);
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(RequestStatus::ApprovalsReady),
                );
                assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
                assert_eq!(
                    EthBridge::pending_bridge_signature_version(0),
                    Some(eth_bridge::BridgeSignatureVersion::V3),
                );
                (charged, expected)
            });
            if let Some((previous_charge, previous_expected)) = previous {
                // The signed call has a fixed hash-sized payload, the same two
                // recoveries and the same fee/extension path. Its observed size
                // slope must therefore retain all five bounded message allowances
                // as well as the real ABI allowance, not just a constant floor.
                assert_eq!(
                    measured.0.saturating_sub(previous_charge),
                    measured.1.saturating_sub(previous_expected),
                );
                assert!(measured.0.all_gt(previous_charge));
            }
            previous = Some(measured);
        }
    }

    fn finalize_admission_authorless_is_rejected(status: RequestStatus) {
        ext().execute_with(|| {
            let (payer, hash, _, _, authors) = finalize_admission_migration(64, status.clone());
            eth_bridge::RequestApprovers::<Runtime>::remove(0, hash);
            let call = finalize_admission_call(hash);
            frame_system::BlockSize::<Runtime>::kill();
            frame_system::BlockWeight::<Runtime>::kill();
            System::reset_events();
            let before = sp_io::storage::root(StateVersion::V1);
            let error = EthBridge::finalize_approved_request(
                RuntimeOrigin::signed(account(&payer)),
                0,
                hash,
            )
            .unwrap_err();
            assert_eq!(
                error.error,
                eth_bridge::Error::<Runtime>::InsufficientCurrentApprovals.into()
            );
            assert_eq!(
                sp_io::storage::root(StateVersion::V1),
                before,
                "direct rejection must not reconcile away the retained signatures"
            );
            for source in [
                TransactionSource::External,
                TransactionSource::Local,
                TransactionSource::InBlock,
            ] {
                let valid = with_transaction_opaque_err(|| {
                    TransactionOutcome::Rollback(Executive::validate_transaction(
                        source,
                        signed(call.clone(), &payer),
                        Default::default(),
                    ))
                })
                .unwrap();
                assert_eq!(valid, Err(InvalidTransaction::Call.into()));
                assert_eq!(sp_io::storage::root(StateVersion::V1), before);
            }
            // Match the real block-builder boundary: invalid transactions roll
            // back Executive's temporary extrinsic bookkeeping; admitted calls
            // commit, including admitted dispatch failures. This detects a gate
            // regression without asserting that Executive itself is read-only.
            let applied = with_transaction_opaque_err(|| {
                let result = Executive::apply_extrinsic(signed(call, &payer));
                if result.is_ok() {
                    TransactionOutcome::Commit(result)
                } else {
                    TransactionOutcome::Rollback(result)
                }
            })
            .unwrap();
            assert_eq!(applied, Err(InvalidTransaction::Call.into()));
            assert_eq!(sp_io::storage::root(StateVersion::V1), before);
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                Some(status)
            );
            assert_eq!(
                eth_bridge::RequestApprovals::<Runtime>::get(0, hash).len(),
                2
            );
            assert!(!eth_bridge::RequestApprovers::<Runtime>::contains_key(
                0, hash
            ));
            assert_eq!(System::account_nonce(&account(&payer)), 0);
            assert!(System::events().is_empty());

            // Control: the same retained signatures are valid and the status is
            // retryable. Supplying their author records must make finalization
            // succeed; the rejection above is not a malformed-proof fixture.
            sp_io::storage::start_transaction();
            eth_bridge::RequestApprovers::<Runtime>::insert(0, hash, authors);
            assert_ok!(EthBridge::finalize_approved_request(
                RuntimeOrigin::signed(account(&payer)),
                0,
                hash,
            ));
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                Some(RequestStatus::ApprovalsReady)
            );
            sp_io::storage::rollback_transaction();
            assert_eq!(sp_io::storage::root(StateVersion::V1), before);
        });
    }

    #[test]
    fn finalize_admission_pending_authorless_rejection_commits_no_state() {
        finalize_admission_authorless_is_rejected(RequestStatus::Pending);
    }

    #[test]
    fn finalize_admission_broken_authorless_rejection_commits_no_state() {
        finalize_admission_authorless_is_rejected(RequestStatus::Broken(
            eth_bridge::Error::<Runtime>::ContractIsNotInMigrationStage.into(),
            eth_bridge::Error::<Runtime>::PublicApprovalsRequireSettlement.into(),
        ));
    }

    #[test]
    fn finalize_admission_hint_declaration_cannot_hide_later_outgoing_payload() {
        ext().execute_with(|| {
            let (payer, hash, bound, _, _) =
                finalize_admission_migration(4096, RequestStatus::Pending);
            let stored = eth_bridge::Requests::<Runtime>::get(0, hash).unwrap();
            let outgoing = finalize_admission_call(hash)
                .get_dispatch_info()
                .call_weight;
            let hint = OffchainRequest::LoadIncoming(LoadIncomingRequest::Transaction(
                LoadIncomingTransactionRequest::new(
                    account(&payer),
                    hash,
                    Default::default(),
                    IncomingTransactionRequestKind::Transfer,
                    0,
                ),
            ));
            eth_bridge::Requests::<Runtime>::insert(0, hash, hint);
            let limits = <Runtime as frame_system::Config>::BlockLength::get();
            let fallback = u64::from(*limits.max.get(DispatchClass::Operational))
                .saturating_mul(2)
                .saturating_add(1024);
            assert!(fallback > bound);
            assert_eq!(EthBridge::approval_message_byte_bound(0, hash), fallback);
            let hinted = finalize_admission_call(hash)
                .get_dispatch_info()
                .call_weight;
            assert_eq!(hinted, finalize_admission_expected_declaration(fallback));
            assert!(hinted.all_gt(outgoing));
            eth_bridge::Requests::<Runtime>::insert(0, hash, stored);
            assert_eq!(
                finalize_admission_call(hash)
                    .get_dispatch_info()
                    .call_weight,
                finalize_admission_expected_declaration(bound),
            );
            assert_eq!(
                finalize_admission_call(hash)
                    .get_dispatch_info()
                    .call_weight,
                outgoing
            );
        });
    }

    #[test]
    #[allow(deprecated)] // Exercise the payment extension's real pre-dispatch hook.
    fn finalize_admission_stale_domain_signatures_reject_without_state_changes() {
        use sp_runtime::traits::SignedExtension;
        use xor_fee::extension::ChargeTransactionPayment;

        ext().execute_with(|| {
            let (payer, hash, _, _, authors) =
                finalize_admission_migration(64, RequestStatus::Pending);
            let who = account(&payer);
            let call = finalize_admission_call(hash);
            let info = call.get_dispatch_info();
            let len = signed(call.clone(), &payer).encoded_size();
            frame_system::BlockSize::<Runtime>::kill();
            frame_system::BlockWeight::<Runtime>::kill();
            System::reset_events();

            // A valid current-domain request is admitted before the fixture's
            // contract-domain change. Its recorded authors remain current peers.
            assert_eq!(
                crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call),
                Ok(false),
            );
            sp_io::storage::start_transaction();
            let valid = Executive::validate_transaction(
                TransactionSource::External,
                signed(call.clone(), &payer),
                Default::default(),
            );
            sp_io::storage::rollback_transaction();
            assert_ok!(valid);

            // Ordinary stale-signature fixture: retain the request, approvals
            // and authors, but change only the current contract domain. This is
            // not a peer-change/migration sequence or a load-generation test.
            let retained = eth_bridge::RequestApprovals::<Runtime>::get(0, hash);
            let mut current_contract = EthBridge::bridge_contract_address(0);
            current_contract.0[0] ^= 1;
            let mut key =
                frame_support::storage::storage_prefix(b"EthBridge", b"BridgeContractAddress")
                    .to_vec();
            key.extend(
                <frame_support::Twox64Concat as frame_support::StorageHasher>::hash(&0u32.encode()),
            );
            sp_io::storage::set(&key, &current_contract.encode());
            assert_eq!(EthBridge::bridge_contract_address(0), current_contract);
            assert_eq!(
                eth_bridge::RequestApprovers::<Runtime>::get(0, hash),
                authors
            );
            assert_eq!(retained.len(), 2);
            let before = sp_io::storage::root(StateVersion::V1);

            assert_eq!(
                crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call),
                Err(InvalidTransaction::Call.into()),
            );
            assert_eq!(sp_io::storage::root(StateVersion::V1), before);
            let error =
                EthBridge::finalize_approved_request(RuntimeOrigin::signed(who.clone()), 0, hash)
                    .unwrap_err();
            assert_eq!(
                error.error,
                eth_bridge::Error::<Runtime>::InsufficientCurrentApprovals.into()
            );
            assert_eq!(sp_io::storage::root(StateVersion::V1), before);

            // Independently exercise fee withdrawal after the earlier successful
            // admission. It must recheck the live domain and reject before any
            // fee or bridge write, rather than trust prior validation.
            assert_eq!(
                ChargeTransactionPayment::<Runtime>::new()
                    .pre_dispatch(&who, &call, &info, len)
                    .err(),
                Some(InvalidTransaction::Call.into()),
            );
            assert_eq!(sp_io::storage::root(StateVersion::V1), before);
            for source in [
                TransactionSource::External,
                TransactionSource::Local,
                TransactionSource::InBlock,
            ] {
                let valid = with_transaction_opaque_err(|| {
                    TransactionOutcome::Rollback(Executive::validate_transaction(
                        source,
                        signed(call.clone(), &payer),
                        Default::default(),
                    ))
                })
                .unwrap();
                assert_eq!(valid, Err(InvalidTransaction::Call.into()));
                assert_eq!(sp_io::storage::root(StateVersion::V1), before);
            }
            let applied = with_transaction_opaque_err(|| {
                let result = Executive::apply_extrinsic(signed(call.clone(), &payer));
                if result.is_ok() {
                    TransactionOutcome::Commit(result)
                } else {
                    TransactionOutcome::Rollback(result)
                }
            })
            .unwrap();
            assert_eq!(applied, Err(InvalidTransaction::Call.into()));
            assert_eq!(sp_io::storage::root(StateVersion::V1), before);
            assert_eq!(
                eth_bridge::RequestApprovals::<Runtime>::get(0, hash),
                retained
            );
            assert_eq!(
                eth_bridge::RequestApprovers::<Runtime>::get(0, hash),
                authors
            );
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                Some(RequestStatus::Pending)
            );
            assert_eq!(System::account_nonce(&who), 0);
            assert!(System::events().is_empty());

            // Control: refresh only the two deterministic test signatures for
            // the current contract domain. The same public caller/request now
            // passes actual signed validation, preparation and finalization.
            let outgoing = eth_bridge::Requests::<Runtime>::get(0, hash)
                .unwrap()
                .into_outgoing()
                .unwrap()
                .0;
            let eth_bridge::requests::OutgoingRequestEncoded::Migrate(encoded) =
                outgoing.to_eth_abi(hash).unwrap()
            else {
                panic!("migration fixture")
            };
            let message = common::eth::prepare_message(&encoded.raw).serialize();
            let current: BTreeSet<_> = [231u8, 232u8]
                .into_iter()
                .map(|seed| {
                    let peer = sp_core::ecdsa::Pair::from_seed(&[seed; 32]);
                    let signature = peer.sign_prehashed(&message);
                    let mut r = [0; 32];
                    r.copy_from_slice(&signature.0[..32]);
                    let mut s = [0; 32];
                    s.copy_from_slice(&signature.0[32..64]);
                    eth_bridge::offchain::SignatureParams {
                        r,
                        s,
                        v: signature.0[64],
                    }
                })
                .collect();
            assert_eq!(current.len(), 2);
            assert_ne!(current, retained);
            eth_bridge::RequestApprovals::<Runtime>::insert(0, hash, current);
            assert_eq!(
                crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call),
                Ok(false),
            );
            sp_io::storage::start_transaction();
            let valid = Executive::validate_transaction(
                TransactionSource::External,
                signed(call.clone(), &payer),
                Default::default(),
            );
            sp_io::storage::rollback_transaction();
            assert_ok!(valid);
            assert_ok!(apply(call, &payer, false));
            assert_eq!(System::account_nonce(&who), 1);
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                Some(RequestStatus::ApprovalsReady)
            );
            assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
        });
    }
}
