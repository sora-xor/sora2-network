// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{
    AccountId, Assets, Balances, BridgeProxy, Currencies, EthBridge, Executive, Runtime,
    RuntimeCall, RuntimeEvent, RuntimeOrigin, Signature, SignedExtra, System, UncheckedExtrinsic,
    XorFee,
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
fn arbitrary_peer_wrappers_require_fees_and_protocol_cleanup_remains_free() {
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
                timepoint: Default::default(),
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
        assert_ok!(apply(remark, &pairs[0], false));
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
        assert_ok!(EthBridge::register_existing_sidechain_asset(RuntimeOrigin::root(), PSWAP, H160::repeat_byte(2), 0));
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
            let extra:SignedExtra=(frame_system::CheckSpecVersion::<Runtime>::new(),frame_system::CheckTxVersion::<Runtime>::new(),frame_system::CheckGenesis::<Runtime>::new(),frame_system::CheckEra::<Runtime>::from(Era::Immortal),frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),frame_system::CheckWeight::<Runtime>::new(),crate::charge_tx_payment_extension());
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
