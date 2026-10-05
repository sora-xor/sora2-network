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
        assert_ok!(EthBridge::register_existing_sidechain_asset(
            RuntimeOrigin::root(), PSWAP, H160::repeat_byte(2), 0
        ));
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
            assert!(
                System::block_weight().get(class).all_gte(declared_weight),
                "successful approval must retain its admission-validation allowance"
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

fn rejected_nonpeer(call: RuntimeCall, pair: &sr25519::Pair) {
    rejected_with_reason(
        call,
        pair,
        sp_runtime::transaction_validity::InvalidTransaction::Payment,
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
        rejected_nonpeer(cancellation(hash, timepoint), &outsider);
        rejected_from_pool_and_block(
            cancellation(
                hash,
                bridge_multisig::Pallet::<Runtime>::sidechain_timepoint(10, 212),
            ),
            &pairs[1],
        );
        rejected_nonpeer(cancellation(hash, timepoint), &pairs[0]);

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
        assert!(bridge_multisig::DispatchedCalls::<Runtime>::contains_key(
            hash, timepoint
        ));
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
        rejected_nonpeer(cancellation(hash, timepoint), &pairs[1]);
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
        assert!(bridge_multisig::DispatchedCalls::<Runtime>::contains_key(
            hash, timepoint
        ));
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
