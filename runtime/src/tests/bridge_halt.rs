// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! The emergency halt through the real signed-transaction pipeline: guardians
//! halt for free, other signers are rejected before inclusion, and halted
//! bridges refuse work until Root resumes them.

use crate::{
    AccountId, Balances, BridgeHalt, BridgeProxy, Executive, Runtime, RuntimeCall, RuntimeEvent,
    RuntimeOrigin, Signature, SignedExtra, System, UncheckedExtrinsic, XorFee,
};
use bridge_types::{GenericAccount, GenericNetworkId, SubNetworkId};
use codec::Encode;
use common::{balance, recovery::BRIDGE_HALT_STORAGE_KEY, XOR};
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::Currency,
};
use framenode_chain_spec::ext;
use sp_core::{sr25519, Pair};
use sp_runtime::{
    generic::Era,
    transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError,
    },
    DispatchError, FixedPointNumber, FixedU128,
};

fn guardian() -> sr25519::Pair {
    sr25519::Pair::from_seed(&[61; 32])
}

fn stranger() -> sr25519::Pair {
    sr25519::Pair::from_seed(&[62; 32])
}

fn account(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public())
}

fn halt(message: &[u8]) -> RuntimeCall {
    RuntimeCall::BridgeHalt(bridge_halt::Call::halt {
        reason: message.to_vec().try_into().unwrap(),
    })
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

/// Pool validation, discarding the validation block Executive initializes.
fn validate(call: RuntimeCall, pair: &sr25519::Pair) -> TransactionValidity {
    sp_io::storage::start_transaction();
    let validity = Executive::validate_transaction(
        TransactionSource::External,
        signed(call, pair),
        Default::default(),
    );
    sp_io::storage::rollback_transaction();
    validity
}

fn fresh_block() {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    System::reset_events();
}

/// Both accounts exist and hold XOR, so a charged fee would show as a balance change.
fn setup() {
    System::set_block_number(1);
    for pair in [guardian(), stranger()] {
        let _ = Balances::make_free_balance_be(&account(&pair), balance!(10));
    }
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
    assert_ok!(BridgeHalt::add_guardian(
        RuntimeOrigin::root(),
        account(&guardian())
    ));
    sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
    fresh_block();
}

fn latch() -> Option<Vec<u8>> {
    sp_io::storage::get(BRIDGE_HALT_STORAGE_KEY).map(|value| value.to_vec())
}

/// A bridge entry point that checks the latch before anything else.
fn burn_error() -> DispatchError {
    BridgeProxy::burn(
        RuntimeOrigin::signed(account(&stranger())),
        GenericNetworkId::Sub(SubNetworkId::Liberland),
        XOR,
        GenericAccount::Sora(account(&stranger())),
        balance!(1),
    )
    .unwrap_err()
    .error
}

#[test]
fn guardian_halts_every_bridge_without_paying() {
    ext().execute_with(|| {
        setup();
        let who = account(&guardian());
        assert_eq!(
            halt(b"x").get_dispatch_info().class,
            DispatchClass::Operational
        );
        assert_eq!(
            burn_error(),
            bridge_proxy::Error::<Runtime>::WrongAccountKind.into()
        );

        assert_ok!(validate(halt(b"HASHI peer keys leaked"), &guardian()));
        let before = Balances::free_balance(&who);
        let nonce = System::account_nonce(&who);
        assert_ok!(Executive::apply_extrinsic(signed(
            halt(b"HASHI peer keys leaked"),
            &guardian()
        ))
        .unwrap());

        assert_eq!(Balances::free_balance(&who), before);
        assert_eq!(System::account_nonce(&who), nonce + 1);
        assert_eq!(latch(), Some(vec![1]));
        assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid {
                who: payer,
                actual_fee: 0,
                tip: 0,
            }) if payer == &who
        )));
        assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::BridgeHalt(bridge_halt::Event::BridgeHalted { guardian, reason })
                if guardian == &who && reason.as_slice() == b"HASHI peer keys leaked"
        )));
        assert_eq!(
            burn_error(),
            bridge_proxy::Error::<Runtime>::RecoveryHalted.into()
        );
    });
}

#[test]
fn other_signers_are_rejected_before_inclusion() {
    ext().execute_with(|| {
        setup();
        let who = account(&stranger());
        assert_eq!(
            validate(halt(b"x"), &stranger()),
            Err(InvalidTransaction::BadSigner.into())
        );
        let before = Balances::free_balance(&who);
        let nonce = System::account_nonce(&who);
        // An Err here makes the block builder drop the extrinsic and roll back its
        // overlay (which already holds System::ExtrinsicData), so it is never included.
        assert_eq!(
            Executive::apply_extrinsic(signed(halt(b"x"), &stranger())),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::BadSigner
            ))
        );
        assert_eq!(Balances::free_balance(&who), before);
        assert_eq!(System::account_nonce(&who), nonce);
        assert_eq!(latch(), None);
        assert_eq!(bridge_halt::LastHalt::<Runtime>::get(), None);
        assert!(System::events().is_empty());
    });
}

#[test]
fn a_tip_is_refused_because_the_halt_is_free() {
    ext().execute_with(|| {
        setup();
        let who = account(&guardian());
        let call = halt(b"x");
        let extra: SignedExtra = (
            frame_system::AuthorizeCall::new(),
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(Era::Immortal),
            frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),
            frame_system::CheckWeight::<Runtime>::new(),
            crate::ChargeTxPaymentExtension::from(xor_fee::extension::ChargeTransactionPayment::<
                Runtime,
            >::from(1)),
        );
        let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
        let signature = payload.using_encoded(|payload| guardian().sign(payload));
        let tipped =
            UncheckedExtrinsic::new_signed(call, who, Signature::Sr25519(signature), extra);
        assert_eq!(
            Executive::apply_extrinsic(tipped),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );
        assert_eq!(latch(), None);
    });
}

#[test]
fn repeat_halts_are_rejected_while_halted() {
    ext().execute_with(|| {
        setup();
        let who = account(&guardian());
        assert_ok!(Executive::apply_extrinsic(signed(halt(b"first"), &guardian())).unwrap());
        fresh_block();
        assert_eq!(
            validate(halt(b"second"), &guardian()),
            Err(InvalidTransaction::Stale.into())
        );
        let before = Balances::free_balance(&who);
        assert_eq!(
            Executive::apply_extrinsic(signed(halt(b"second"), &guardian())),
            Err(TransactionValidityError::Invalid(InvalidTransaction::Stale))
        );
        assert_eq!(Balances::free_balance(&who), before);
        assert_eq!(
            bridge_halt::LastHalt::<Runtime>::get()
                .unwrap()
                .reason
                .as_slice(),
            b"first"
        );
    });
}

#[test]
fn a_wrapped_halt_is_not_free_and_still_needs_a_guardian() {
    ext().execute_with(|| {
        setup();
        let who = account(&stranger());
        let batch = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![halt(b"x")],
        });
        assert_eq!(
            crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &batch),
            Ok(false)
        );
        let before = Balances::free_balance(&who);
        let _ = Executive::apply_extrinsic(signed(batch, &stranger()))
            .expect("an ordinary paid wrapper is admitted");
        assert!(Balances::free_balance(&who) < before);
        assert_eq!(latch(), None);
    });
}

#[test]
fn halted_bridges_reopen_only_through_root() {
    ext().execute_with(|| {
        setup();
        assert_ok!(BridgeHalt::halt(
            RuntimeOrigin::signed(account(&guardian())),
            b"x".to_vec().try_into().unwrap()
        ));
        assert_noop!(
            BridgeHalt::resume(RuntimeOrigin::signed(account(&guardian()))),
            DispatchError::BadOrigin
        );
        assert_ok!(BridgeHalt::resume(RuntimeOrigin::root()));
        assert_eq!(latch(), None);
        assert_eq!(
            burn_error(),
            bridge_proxy::Error::<Runtime>::WrongAccountKind.into()
        );
    });
}

#[test]
fn guardians_are_managed_by_the_bridge_manager_origin() {
    ext().execute_with(|| {
        setup();
        let who = account(&stranger());
        assert_noop!(
            BridgeHalt::add_guardian(RuntimeOrigin::signed(who.clone()), who.clone()),
            DispatchError::BadOrigin
        );
        assert_ok!(BridgeHalt::add_guardian(RuntimeOrigin::root(), who.clone()));
        assert_eq!(BridgeHalt::validate_halt(&who), Ok(()));
        assert_ok!(BridgeHalt::remove_guardian(
            RuntimeOrigin::root(),
            who.clone()
        ));
        assert_eq!(
            validate(halt(b"x"), &stranger()),
            Err(InvalidTransaction::BadSigner.into())
        );
    });
}
