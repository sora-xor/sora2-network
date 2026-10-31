//! Disabled xorless calls must not inherit work they never execute.

use crate::{
    AccountId, Balances, Executive, Runtime, RuntimeCall, RuntimeEvent, Signature, SignedExtra,
    System, UncheckedExtrinsic,
};
use codec::Encode;
use common::balance;
use frame_support::{
    assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo, Pays},
    traits::Currency,
    weights::Weight,
};
use framenode_chain_spec::ext;
use sp_core::{ecdsa, sr25519, Pair, H256};
use sp_runtime::{generic::Era, transaction_validity::TransactionSource, DispatchError};
use xor_fee::weights::WeightInfo;

fn signer() -> sr25519::Pair {
    sr25519::Pair::from_seed(&[219; 32])
}

fn account(pair: &sr25519::Pair) -> AccountId {
    AccountId::new(pair.public().0)
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

fn inner_calls() -> [RuntimeCall; 2] {
    let hash = H256::repeat_byte(218);
    assert!(!eth_bridge::Requests::<Runtime>::contains_key(0, hash));
    [
        RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
            ocw_public: ecdsa::Public::from_raw([0; 33]),
            hash,
            signature_params: eth_bridge::offchain::SignatureParams {
                r: [0; 32],
                s: [0; 32],
                v: 0,
            },
            network_id: 0,
        }),
        RuntimeCall::EthBridge(eth_bridge::Call::finalize_approved_request {
            network_id: 0,
            hash,
        }),
    ]
}

fn disabled(inner: RuntimeCall) -> RuntimeCall {
    RuntimeCall::XorFee(xor_fee::Call::xorless_call {
        call: Box::new(inner),
        asset_id: None,
    })
}

/// Validate a real signed transaction, discard the validation overlay as the
/// runtime API does, then include it without clearing the block's counters.
fn apply_paid(call: RuntimeCall, pair: &sr25519::Pair) -> sp_runtime::DispatchResult {
    let who = account(pair);
    let before_root = sp_io::storage::root(sp_runtime::StateVersion::V1);
    sp_io::storage::start_transaction();
    let validity = Executive::validate_transaction(
        TransactionSource::External,
        signed(call.clone(), pair),
        Default::default(),
    );
    sp_io::storage::rollback_transaction();
    assert_ok!(validity);
    assert_eq!(
        sp_io::storage::root(sp_runtime::StateVersion::V1),
        before_root
    );

    let transaction = signed(call, pair);
    let info = transaction.get_dispatch_info();
    let encoded_len = transaction.encoded_size() as u64;
    let base_weight = crate::BlockWeights::get().get(info.class).base_extrinsic;
    let expected_max = info
        .total_weight()
        .saturating_add(base_weight)
        .saturating_add_proof_size(encoded_len);
    let before = *System::block_weight().get(info.class);
    let balance_before = Balances::total_balance(&who);
    let nonce_before = System::account_nonce(&who);
    System::reset_events();
    let result = Executive::apply_extrinsic(transaction).expect("paid disabled call is admitted");
    let charged = System::block_weight()
        .get(info.class)
        .saturating_sub(before);
    assert!(charged.all_lte(expected_max));
    // Refund only unused call/extension weight. CheckWeight's base extrinsic
    // charge and encoded-length proof contribution must remain in the block.
    assert!(charged.all_gte(base_weight.saturating_add_proof_size(encoded_len)));
    // The absent bridge request declared hundreds of billions of ref-time.
    // Neither it nor any tested outer wrapper may retain that phantom work.
    let normal = crate::BlockWeights::get()
        .get(DispatchClass::Normal)
        .max_total
        .unwrap();
    assert!(charged.ref_time() < normal.ref_time() / 100);
    assert_eq!(System::account_nonce(&who), nonce_before + 1);
    assert!(Balances::total_balance(&who) < balance_before);
    assert!(System::events().iter().any(|event| matches!(
        event.event,
        RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid {
            actual_fee, tip: 0, ..
        }) if actual_fee > 0
    )));
    assert!(!System::events()
        .iter()
        .any(|event| matches!(event.event, RuntimeEvent::EthBridge(_))));
    println!(
        "disabled xorless: charged_ref_time={}, declared_total_ref_time={}, charged_proof_size={}, encoded_len={}, result={:?}",
        charged.ref_time(),
        info.total_weight().ref_time(),
        charged.proof_size(),
        encoded_len,
        result
    );
    result
}

#[test]
fn disabled_xorless_bridge_calls_are_cheap_in_pool_and_executive() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let pair = signer();
        let _ = Balances::make_free_balance_be(&account(&pair), balance!(1000000));
        frame_system::BlockWeight::<Runtime>::kill();
        frame_system::BlockSize::<Runtime>::kill();
        let mut phantom_weight = 0u64;
        for _ in 0..4 {
            for inner in inner_calls() {
                phantom_weight =
                    phantom_weight.saturating_add(inner.get_dispatch_info().call_weight.ref_time());
                let call = disabled(inner);
                let info = call.get_dispatch_info();
                assert_eq!(info.class, DispatchClass::Normal);
                assert_eq!(info.pays_fee, Pays::Yes);
                assert_eq!(
                    info.call_weight,
                    <Runtime as xor_fee::Config>::WeightInfo::xorless_call()
                        .saturating_add(crate::incident_recovery_origin::weight())
                );
                assert_eq!(apply_paid(call, &pair), Err(DispatchError::Unavailable));
            }
        }
        let normal = crate::BlockWeights::get()
            .get(DispatchClass::Normal)
            .max_total
            .unwrap();
        assert!(phantom_weight > 2 * normal.ref_time());
        assert!(
            System::block_weight().get(DispatchClass::Normal).ref_time() < normal.ref_time() / 10
        );
        assert_eq!(System::account_nonce(account(&pair)), 8);
    });
}

#[test]
fn disabled_xorless_signed_admission_fits_near_full_normal_block() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let pair = signer();
        let _ = Balances::make_free_balance_be(&account(&pair), balance!(1000000));
        let limits = crate::BlockWeights::get();
        let normal = limits.get(DispatchClass::Normal).max_total.unwrap();
        let remaining_ref_time = normal.ref_time() / 100;

        for inner in inner_calls() {
            // Start a separate 99%-full block fixture for each bridge variant.
            // The prior inner declaration cannot fit, even though its execution
            // is disabled. A post-dispatch refund alone cannot fix admission.
            let inner_weight = inner.get_dispatch_info().total_weight();
            assert!(inner_weight.ref_time() > remaining_ref_time);
            frame_system::BlockWeight::<Runtime>::kill();
            frame_system::BlockSize::<Runtime>::kill();
            System::register_extra_weight_unchecked(
                Weight::from_parts(normal.ref_time() - remaining_ref_time, 0),
                DispatchClass::Normal,
            );

            let call = disabled(inner);
            let transaction = signed(call.clone(), &pair);
            let declared = transaction
                .get_dispatch_info()
                .total_weight()
                .saturating_add(limits.get(DispatchClass::Normal).base_extrinsic);
            let before = *System::block_weight().get(DispatchClass::Normal);
            assert_eq!(apply_paid(call, &pair), Err(DispatchError::Unavailable));
            // Keep this check after Executive: the postweight-only version
            // must fail real pre-dispatch admission, not just a metadata test.
            assert!(declared.ref_time() < remaining_ref_time);
            let after = *System::block_weight().get(DispatchClass::Normal);
            assert!(after.all_gt(before));
            assert!(after.all_lte(normal));
        }
        assert_eq!(System::account_nonce(account(&pair)), 2);
    });
}

#[test]
fn disabled_operational_inner_call_stays_normal_and_retains_length_proof() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let pair = signer();
        let _ = Balances::make_free_balance_be(&account(&pair), balance!(1000000));
        frame_system::BlockWeight::<Runtime>::kill();
        frame_system::BlockSize::<Runtime>::kill();
        let key = b":disabled-xorless-public-test-marker".to_vec();
        assert!(sp_io::storage::get(&key).is_none());
        let inner = RuntimeCall::System(frame_system::Call::set_storage {
            items: vec![(key.clone(), vec![0xa5; 8192])],
        });
        assert_eq!(inner.get_dispatch_info().class, DispatchClass::Operational);
        let call = disabled(inner);
        let info = call.get_dispatch_info();
        assert_eq!(info.class, DispatchClass::Normal);
        assert_eq!(info.pays_fee, Pays::Yes);
        assert_eq!(
            info.call_weight,
            <Runtime as xor_fee::Config>::WeightInfo::xorless_call()
                .saturating_add(crate::incident_recovery_origin::weight())
        );
        let encoded_len = signed(call.clone(), &pair).encoded_size() as u64;
        assert!(encoded_len > 8192);
        let operational_before = *System::block_weight().get(DispatchClass::Operational);
        assert_eq!(apply_paid(call, &pair), Err(DispatchError::Unavailable));
        assert_eq!(
            *System::block_weight().get(DispatchClass::Operational),
            operational_before
        );
        assert!(
            System::block_weight()
                .get(DispatchClass::Normal)
                .proof_size()
                >= encoded_len
        );
        assert!(sp_io::storage::get(&key).is_none());
    });
}

#[test]
fn nested_disabled_xorless_calls_do_not_reserve_inner_bridge_weight() {
    ext().execute_with(|| {
        System::set_block_number(1);
        let pair = signer();
        let _ = Balances::make_free_balance_be(&account(&pair), balance!(1000000));
        frame_system::BlockWeight::<Runtime>::kill();
        frame_system::BlockSize::<Runtime>::kill();
        for inner in inner_calls() {
            let call = disabled(inner);
            let variants = [
                (
                    RuntimeCall::Utility(pallet_utility::Call::batch {
                        calls: vec![call.clone()],
                    }),
                    true,
                ),
                (
                    RuntimeCall::Utility(pallet_utility::Call::batch_all {
                        calls: vec![call.clone()],
                    }),
                    false,
                ),
                (
                    RuntimeCall::Utility(pallet_utility::Call::force_batch {
                        calls: vec![call.clone()],
                    }),
                    true,
                ),
                (
                    RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                        index: 0,
                        call: Box::new(call.clone()),
                    }),
                    false,
                ),
                (
                    RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                        other_signatories: vec![AccountId::new([217; 32])],
                        call: Box::new(call),
                    }),
                    false,
                ),
            ];
            for (outer, succeeds) in variants {
                let result = apply_paid(outer, &pair);
                assert_eq!(
                    result,
                    if succeeds {
                        Ok(())
                    } else {
                        Err(DispatchError::Unavailable)
                    }
                );
            }
        }
        assert_eq!(System::account_nonce(account(&pair)), 10);
    });
}
