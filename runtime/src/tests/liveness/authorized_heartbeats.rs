// This file is part of SORA Nexus and the Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::{Executive, RuntimeCall, UncheckedExtrinsic};
use frame_system::offchain::CreateAuthorizedTransaction;

fn general(call: RuntimeCall) -> UncheckedExtrinsic {
    <Runtime as CreateAuthorizedTransaction<RuntimeCall>>::create_authorized_transaction(call)
}

#[test]
fn authorization_rejects_forged_heartbeats_and_unapproved_calls_without_payment() {
    ext().execute_with(|| {
        let validators = Validators::new();
        validators.start_session(FIRST_SESSION + 1);
        let heartbeat = Heartbeat {
            block_number: System::block_number(),
            session_index: FIRST_SESSION + 1,
            authority_index: 0,
            validators_len: VALIDATORS,
        };
        let forged = RuntimeCall::ImOnline(pallet_im_online::Call::heartbeat {
            signature: authority_pair(1).sign(&heartbeat.encode()),
            heartbeat,
        });
        let ordinary = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
        let balances = validators.balances_and_stakes();
        for call in [forged, ordinary] {
            let tx = general(call);
            let events = System::events();
            assert!(Executive::validate_transaction(
                TransactionSource::External,
                tx.clone(),
                Default::default(),
            )
            .is_err());
            assert!(Executive::apply_extrinsic(tx).is_err());
            assert_eq!(validators.balances_and_stakes(), balances);
            assert_eq!(System::events(), events);
            assert!(!ImOnline::received_heartbeat_in_current_session(0));
        }
    });
}

#[test]
fn authorized_heartbeat_cannot_silently_ignore_a_nonzero_tip() {
    ext().execute_with(|| {
        let validators = Validators::new();
        validators.start_session(FIRST_SESSION + 1);
        let heartbeat = Heartbeat {
            block_number: System::block_number(),
            session_index: FIRST_SESSION + 1,
            authority_index: 0,
            validators_len: VALIDATORS,
        };
        let call = RuntimeCall::ImOnline(pallet_im_online::Call::heartbeat {
            signature: authority_pair(0).sign(&heartbeat.encode()),
            heartbeat,
        });
        let mut extra = <Runtime as CreateAuthorizedTransaction<RuntimeCall>>::create_extension();
        extra.7 =
            crate::ChargeTxPaymentExtension::from(xor_fee::extension::ChargeTransactionPayment::<
                Runtime,
            >::from(1));
        let tx = UncheckedExtrinsic::new_transaction(call, extra);
        let balances = validators.balances_and_stakes();
        let events = System::events();
        assert_eq!(
            Executive::validate_transaction(
                TransactionSource::External,
                tx.clone(),
                Default::default()
            ),
            InvalidTransaction::Payment.into()
        );
        assert_eq!(
            Executive::apply_extrinsic(tx),
            Err(InvalidTransaction::Payment.into())
        );
        assert_eq!(validators.balances_and_stakes(), balances);
        assert_eq!(System::events(), events);
        assert!(!ImOnline::received_heartbeat_in_current_session(0));
    });
}

#[test]
fn signed_heartbeat_still_pays_for_its_bad_origin_failure() {
    use sp_runtime::{FixedPointNumber, FixedU128};

    ext().execute_with(|| {
        let validators = Validators::new();
        validators.start_session(FIRST_SESSION + 1);
        let heartbeat = Heartbeat {
            block_number: System::block_number(),
            session_index: FIRST_SESSION + 1,
            authority_index: 0,
            validators_len: VALIDATORS,
        };
        let call = RuntimeCall::ImOnline(pallet_im_online::Call::heartbeat {
            signature: authority_pair(0).sign(&heartbeat.encode()),
            heartbeat,
        });
        let payer = sp_core::sr25519::Pair::from_seed(&[77; 32]);
        let who = AccountId::from(payer.public());
        let _ = Balances::make_free_balance_be(&who, 1_000 * UNIT);
        assert_ok!(crate::XorFee::update_multiplier(
            RuntimeOrigin::root(),
            FixedU128::saturating_from_integer(1)
        ));
        let extra = <Runtime as CreateAuthorizedTransaction<RuntimeCall>>::create_extension();
        let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
        let signature = payload.using_encoded(|bytes| payer.sign(bytes));
        let tx = UncheckedExtrinsic::new_signed(
            call,
            who.clone(),
            crate::Signature::Sr25519(signature),
            extra,
        );
        let before = Balances::free_balance(&who);
        assert_eq!(
            Executive::apply_extrinsic(tx).unwrap(),
            Err(sp_runtime::DispatchError::BadOrigin)
        );
        assert!(Balances::free_balance(&who) < before);
        assert_eq!(System::account_nonce(&who), 1);
        assert!(!ImOnline::received_heartbeat_in_current_session(0));
    });
}

#[test]
fn heartbeat_payment_adapter_preserves_legacy_encoding_and_metadata() {
    use crate::authorized_heartbeat_payment::AuthorizedHeartbeatPayment;
    use sp_runtime::traits::TransactionExtension;

    #[expect(
        deprecated,
        reason = "Bind the adapter to the reviewed signed-payment encoding and metadata"
    )]
    type LegacyPayment = sp_runtime::traits::transaction_extension::AsTransactionExtension<
        xor_fee::extension::ChargeTransactionPayment<Runtime>,
    >;

    type OriginalPayment = pallet_skip_feeless_payment::SkipCheckIfFeeless<Runtime, LegacyPayment>;

    for tip in [0, 1, 63, 64, 16_383, 16_384, u128::MAX] {
        let legacy: LegacyPayment =
            xor_fee::extension::ChargeTransactionPayment::<Runtime>::from(tip).into();
        let original = OriginalPayment::from(legacy);
        let adapter = AuthorizedHeartbeatPayment::from(
            xor_fee::extension::ChargeTransactionPayment::<Runtime>::from(tip),
        );
        assert_eq!(adapter.encode(), original.encode());

        let mut current = <Runtime as CreateAuthorizedTransaction<RuntimeCall>>::create_extension();
        current.7 = adapter;
        let original_pipeline = (
            current.1.clone(),
            current.2.clone(),
            current.3.clone(),
            current.4.clone(),
            current.5.clone(),
            current.6.clone(),
            original,
        );
        assert_eq!(current.encode(), original_pipeline.encode());
    }
    assert_eq!(
        scale_info::meta_type::<crate::ChargeTxPaymentExtension>(),
        scale_info::meta_type::<OriginalPayment>()
    );
    assert_eq!(
        scale_info::meta_type::<AuthorizedHeartbeatPayment>(),
        scale_info::meta_type::<LegacyPayment>()
    );
    let metadata = |items: Vec<sp_runtime::traits::TransactionExtensionMetadata>| {
        items
            .into_iter()
            .map(|item| (item.identifier, item.ty, item.implicit))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        metadata(<AuthorizedHeartbeatPayment as TransactionExtension<
            RuntimeCall,
        >>::metadata()),
        metadata(<LegacyPayment as TransactionExtension<RuntimeCall>>::metadata())
    );
}

#[test]
fn heartbeat_payment_prepare_rechecks_the_narrow_bypass_contract() {
    use crate::authorized_heartbeat_payment::AuthorizedHeartbeatPayment;
    use frame_support::dispatch::GetDispatchInfo;
    use sp_runtime::traits::TransactionExtension;

    ext().execute_with(|| {
        let heartbeat = Heartbeat {
            block_number: 1,
            session_index: FIRST_SESSION,
            authority_index: 0,
            validators_len: VALIDATORS,
        };
        let heartbeat_call = RuntimeCall::ImOnline(pallet_im_online::Call::heartbeat {
            signature: authority_pair(0).sign(&heartbeat.encode()),
            heartbeat,
        });
        let ordinary_call = RuntimeCall::System(frame_system::Call::remark { remark: vec![] });
        let authorized: RuntimeOrigin = frame_system::RawOrigin::Authorized.into();
        for (origin, call, tip, error) in [
            (
                RuntimeOrigin::none(),
                heartbeat_call.clone(),
                0,
                InvalidTransaction::BadSigner,
            ),
            (
                RuntimeOrigin::signed(AccountId::from([99; 32])),
                heartbeat_call.clone(),
                0,
                InvalidTransaction::BadSigner,
            ),
            (
                authorized.clone(),
                ordinary_call,
                0,
                InvalidTransaction::BadSigner,
            ),
            (authorized, heartbeat_call, 1, InvalidTransaction::Payment),
        ] {
            let payment =
                AuthorizedHeartbeatPayment::from(xor_fee::extension::ChargeTransactionPayment::<
                    Runtime,
                >::from(tip));
            // A forged absent validation result cannot turn another origin/call
            // into a free payment path during preparation.
            assert_eq!(
                payment
                    .prepare(
                        None,
                        &origin,
                        &call,
                        &call.get_dispatch_info(),
                        call.encoded_size()
                    )
                    .err(),
                Some(error.into())
            );
        }
    });
}

#[test]
fn bare_heartbeat_is_rejected_before_dispatch_and_commits_no_state() {
    use frame_support::storage::{transactional::with_transaction_opaque_err, TransactionOutcome};

    ext().execute_with(|| {
        let validators = Validators::new();
        validators.start_session(FIRST_SESSION + 1);
        let heartbeat = Heartbeat {
            block_number: System::block_number(),
            session_index: FIRST_SESSION + 1,
            authority_index: 0,
            validators_len: VALIDATORS,
        };
        let call = RuntimeCall::ImOnline(pallet_im_online::Call::heartbeat {
            signature: authority_pair(0).sign(&heartbeat.encode()),
            heartbeat,
        });
        let tx = UncheckedExtrinsic::new_bare(call);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);

        for source in [
            TransactionSource::External,
            TransactionSource::Local,
            TransactionSource::InBlock,
        ] {
            // Transaction validation runs against disposable state, as the runtime API requires.
            let validity = with_transaction_opaque_err(|| {
                TransactionOutcome::Rollback(Executive::validate_transaction(
                    source,
                    tx.clone(),
                    Default::default(),
                ))
            })
            .expect("transactional validation context is available");
            assert!(validity.is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        }

        // Match the block builder's transaction boundary: only admitted extrinsics
        // commit. Before the guard, Ok(Err(BadOrigin)) was admitted and committed.
        let applied = with_transaction_opaque_err(|| {
            let result = Executive::apply_extrinsic(tx);
            if result.is_ok() {
                TransactionOutcome::Commit(result)
            } else {
                TransactionOutcome::Rollback(result)
            }
        })
        .expect("transactional application context is available");
        assert_eq!(applied, Err(InvalidTransaction::Call.into()));
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert!(!ImOnline::received_heartbeat_in_current_session(0));
    });
}
