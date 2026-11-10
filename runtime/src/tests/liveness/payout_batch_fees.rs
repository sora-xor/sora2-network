// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::xor_fee_impls::{CustomFeeDetails, CustomFees};
use crate::{
    Assets, Executive, RuntimeCall, SignedExtra, TransactionPayment, UncheckedExtrinsic, XorFee,
};
use common::{prelude::constants::MINIMAL_FEE, AssetInfoProvider, VAL};
use frame_support::{
    assert_err,
    dispatch::{GetDispatchInfo, Pays, PostDispatchInfo},
    weights::Weight,
};
use pallet_staking::EraRewardPoints;
use sp_core::sr25519;
use sp_runtime::{generic::Era, DispatchResult, FixedPointNumber, FixedU128};
use xor_fee::ApplyCustomFees;

#[derive(Clone, Copy, Debug)]
enum BatchKind {
    Batch,
    All,
    Force,
}

const KINDS: [BatchKind; 3] = [BatchKind::Batch, BatchKind::All, BatchKind::Force];

impl BatchKind {
    fn call(self, calls: Vec<RuntimeCall>) -> RuntimeCall {
        RuntimeCall::Utility(match self {
            Self::Batch => pallet_utility::Call::batch { calls },
            Self::All => pallet_utility::Call::batch_all { calls },
            Self::Force => pallet_utility::Call::force_batch { calls },
        })
    }
}

fn fixture(funded: bool) -> (sr25519::Pair, Validators) {
    let validators = Validators::with_count(2);
    pallet_staking::CurrentEra::<Runtime>::put(ERA + 1);
    pallet_staking::ErasValidatorReward::<Runtime>::insert(ERA, 200 * UNIT);
    xor_fee::ValStakingEraReward::<Runtime>::insert(ERA, 200 * UNIT);
    pallet_staking::ErasRewardPoints::<Runtime>::insert(
        ERA,
        EraRewardPoints::<Runtime> {
            total: 20,
            individual: validators
                .accounts
                .iter()
                .cloned()
                .map(|who| (who, 10))
                .collect::<std::collections::BTreeMap<_, _>>()
                .try_into()
                .expect("fixture reward points fit validator bound"),
        },
    );
    let signer = sr25519::Pair::from_seed(&[78; 32]);
    let who = AccountId::from(signer.public());
    // Retain a provider even without XOR so CheckNonce does not mask fee admission.
    System::inc_providers(&who);
    if funded {
        drop(Balances::deposit_creating(&who, 1_000_000 * UNIT));
    }
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
    (signer, validators)
}

fn payout(stash: &AccountId, by_page: bool) -> RuntimeCall {
    RuntimeCall::Staking(if by_page {
        pallet_staking::Call::payout_stakers_by_page {
            validator_stash: stash.clone(),
            era: ERA,
            page: 0,
        }
    } else {
        pallet_staking::Call::payout_stakers {
            validator_stash: stash.clone(),
            era: ERA,
        }
    })
}

fn invalid_payout(stash: &AccountId) -> RuntimeCall {
    RuntimeCall::Staking(pallet_staking::Call::payout_stakers_by_page {
        validator_stash: stash.clone(),
        era: ERA,
        page: u32::MAX,
    })
}

fn reset() {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    System::reset_events();
}

fn wire(call: RuntimeCall, signer: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = AccountId::from(signer.public());
    let extra: SignedExtra = (
        frame_system::AuthorizeCall::new(),
        frame_system::CheckSpecVersion::new(),
        frame_system::CheckTxVersion::new(),
        frame_system::CheckGenesis::new(),
        frame_system::CheckEra::from(Era::Immortal),
        frame_system::CheckNonce::from(System::account_nonce(&who)),
        frame_system::CheckWeight::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| signer.sign(bytes));
    UncheckedExtrinsic::new_signed(call, who, crate::Signature::Sr25519(signature), extra)
}

fn val_balance(who: &AccountId) -> Balance {
    Assets::free_balance(&VAL, who).unwrap()
}

fn claimed(stash: &AccountId) -> bool {
    pallet_staking::ClaimedRewards::<Runtime>::get(ERA, stash).contains(&0)
}

fn apply_paid(
    call: RuntimeCall,
    signer: &sr25519::Pair,
    submitted: usize,
) -> (DispatchResult, Balance) {
    reset();
    let extrinsic = wire(call, signer);
    let len = extrinsic.encoded_size() as u32;
    let info = extrinsic.get_dispatch_info();
    assert_eq!(info.pays_fee, Pays::Yes);
    assert!(info.extension_weight.ref_time() > 0);
    let quote = XorFee::query_info(&extrinsic, &extrinsic.function, len);
    let details = XorFee::query_fee_details(&extrinsic, &extrinsic.function, len);
    assert_eq!(quote.partial_fee, details.final_fee());
    assert_eq!(quote.weight, info.total_weight());
    let multiplier = xor_fee::Multiplier::<Runtime>::get();
    let floor = multiplier.saturating_mul_int(MINIMAL_FEE * submitted as Balance)
        + TransactionPayment::length_to_fee(len);
    assert!(quote.partial_fee >= floor);
    let who = AccountId::from(signer.public());
    let before = Balances::free_balance(&who);
    let nonce = System::account_nonce(&who);
    let result = Executive::apply_extrinsic(extrinsic).expect("funded payout batch is admitted");
    let charged = before - Balances::free_balance(&who);
    assert!(
        charged >= floor,
        "all submitted calls retain a positive fee floor"
    );
    assert!(charged <= quote.partial_fee);
    assert_eq!(System::account_nonce(&who), nonce + 1);
    let paid = System::events()
        .into_iter()
        .find_map(|record| match record.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who: payer,
                    actual_fee,
                    tip,
                },
            ) if payer == who => {
                assert_eq!(tip, 0);
                Some(actual_fee)
            }
            _ => None,
        })
        .expect("the actual XOR charge is emitted");
    assert_eq!(paid, charged);
    (result, charged)
}

#[test]
fn signed_payout_batches_pay_val_once_and_charge_replays_for_all_variants() {
    for kind in KINDS {
        for by_page in [false, true] {
            for submitted in [1, 2] {
                ext().execute_with(|| {
                    let (signer, validators) = fixture(true);
                    let native_before = validators.balances_and_stakes();
                    let call = kind.call(
                        validators
                            .accounts
                            .iter()
                            .take(submitted)
                            .map(|who| payout(who, by_page))
                            .collect(),
                    );
                    assert_ok!(apply_paid(call.clone(), &signer, submitted).0);
                    for (index, stash) in validators.accounts.iter().enumerate() {
                        assert_eq!(claimed(stash), index < submitted);
                        assert_eq!(val_balance(stash) > 0, index < submitted);
                    }
                    let rewards = validators
                        .accounts
                        .iter()
                        .chain(std::iter::once(&validators.nominator))
                        .map(val_balance)
                        .collect::<Vec<_>>();
                    assert!(val_balance(&validators.nominator) > 0);
                    assert_eq!(validators.balances_and_stakes(), native_before);
                    let replay = apply_paid(call, &signer, submitted).0;
                    if matches!(kind, BatchKind::All) {
                        assert_eq!(
                            replay.unwrap_err(),
                            pallet_staking::Error::<Runtime>::AlreadyClaimed.into()
                        );
                    } else {
                        assert_ok!(replay);
                    }
                    assert_eq!(
                        validators
                            .accounts
                            .iter()
                            .chain(std::iter::once(&validators.nominator))
                            .map(val_balance)
                            .collect::<Vec<_>>(),
                        rewards
                    );
                    assert!(!System::events().iter().any(|record| matches!(
                        record.event,
                        RuntimeEvent::Staking(pallet_staking::Event::Rewarded { .. })
                    )));
                });
            }
        }
    }
}

#[test]
fn invalid_and_interrupted_payout_batches_pay_for_every_submitted_call() {
    for kind in [BatchKind::Batch, BatchKind::All] {
        ext().execute_with(|| {
            let (signer, validators) = fixture(true);
            // A failing first call must not make the unexecuted tail free.
            let call = kind.call(vec![
                invalid_payout(&validators.accounts[0]),
                payout(&validators.accounts[1], true),
            ]);
            let result = apply_paid(call, &signer, 2).0;
            if matches!(kind, BatchKind::All) {
                assert_eq!(
                    result.unwrap_err(),
                    pallet_staking::Error::<Runtime>::InvalidPage.into()
                );
            } else {
                assert_ok!(result);
                assert!(System::events().iter().any(|record| matches!(
                    record.event,
                    RuntimeEvent::Utility(pallet_utility::Event::BatchInterrupted { index: 0, .. })
                )));
            }
            assert!(validators
                .accounts
                .iter()
                .all(|who| !claimed(who) && val_balance(who) == 0));
            assert_eq!(val_balance(&validators.nominator), 0);
        });
    }
}

#[test]
fn failed_atomic_payout_batch_rolls_back_rewards_and_claims_but_keeps_xor_fee() {
    ext().execute_with(|| {
        let (signer, validators) = fixture(true);
        let before = validators.balances_and_stakes();
        let call = BatchKind::All.call(vec![
            payout(&validators.accounts[0], false),
            invalid_payout(&validators.accounts[1]),
        ]);
        assert_eq!(
            apply_paid(call, &signer, 2).0.unwrap_err(),
            pallet_staking::Error::<Runtime>::InvalidPage.into()
        );
        assert!(validators
            .accounts
            .iter()
            .all(|who| !claimed(who) && val_balance(who) == 0));
        assert_eq!(val_balance(&validators.nominator), 0);
        assert_eq!(validators.balances_and_stakes(), before);
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Staking(pallet_staking::Event::Rewarded { .. })
        )));
    });
}

#[test]
fn force_payout_batch_continues_after_invalid_page_and_still_charges_all_items() {
    ext().execute_with(|| {
        let (signer, validators) = fixture(true);
        let call = BatchKind::Force.call(vec![
            invalid_payout(&validators.accounts[0]),
            payout(&validators.accounts[1], true),
        ]);
        assert_ok!(apply_paid(call, &signer, 2).0);
        assert!(!claimed(&validators.accounts[0]));
        assert_eq!(val_balance(&validators.accounts[0]), 0);
        assert!(claimed(&validators.accounts[1]));
        assert!(val_balance(&validators.accounts[1]) > 0);
        assert!(val_balance(&validators.nominator) > 0);
        assert!(System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Utility(pallet_utility::Event::BatchCompletedWithErrors)
        )));
    });
}

#[test]
fn payout_batches_require_xor_before_validation_and_execution() {
    for kind in KINDS {
        ext().execute_with(|| {
            let (signer, validators) = fixture(false);
            let who = AccountId::from(signer.public());
            let call = kind.call(vec![
                payout(&validators.accounts[0], false),
                payout(&validators.accounts[1], true),
            ]);
            for source in [
                TransactionSource::External,
                TransactionSource::Local,
                TransactionSource::InBlock,
            ] {
                reset();
                assert_err!(
                    Executive::validate_transaction(
                        source,
                        wire(call.clone(), &signer),
                        Default::default()
                    ),
                    InvalidTransaction::Payment
                );
            }
            reset();
            assert_err!(
                Executive::apply_extrinsic(wire(call, &signer)),
                InvalidTransaction::Payment
            );
            assert_eq!(Balances::free_balance(&who), 0);
            assert_eq!(System::account_nonce(&who), 0);
            assert!(validators
                .accounts
                .iter()
                .all(|stash| !claimed(stash) && val_balance(stash) == 0));
            assert_eq!(val_balance(&validators.nominator), 0);
            assert!(System::events().is_empty());
        });
    }
}

#[test]
fn payout_discount_excludes_empty_mixed_nested_and_other_wrapped_calls() {
    ext().execute_with(|| {
        let (signer, validators) = fixture(true);
        let direct = payout(&validators.accounts[0], false);
        let eligible = BatchKind::Batch.call(vec![direct.clone()]);
        let mut calls = Vec::new();
        for kind in KINDS {
            calls.push(kind.call(vec![]));
            calls.push(kind.call(vec![
                direct.clone(),
                RuntimeCall::System(frame_system::Call::remark { remark: vec![] }),
            ]));
            calls.push(kind.call(vec![direct.clone(), eligible.clone()]));
        }
        calls.extend([
            RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                index: 0,
                call: Box::new(eligible.clone()),
            }),
            RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                other_signatories: vec![validators.accounts[0].clone()],
                call: Box::new(eligible.clone()),
            }),
            RuntimeCall::XorFee(xor_fee::Call::xorless_call {
                call: Box::new(eligible),
                asset_id: None,
            }),
        ]);
        for call in calls {
            let extrinsic = wire(call, &signer);
            let info = extrinsic.get_dispatch_info();
            let len = extrinsic.encoded_size() as u32;
            assert_eq!(
                CustomFees::compute_fee_with_dispatch_info(&extrinsic.function, &info),
                None
            );
            assert_eq!(
                XorFee::query_info(&extrinsic, &extrinsic.function, len).partial_fee,
                TransactionPayment::compute_fee(len, &info, 0)
            );
        }
        for by_page in [false, true] {
            assert_eq!(
                CustomFees::compute_fee(&payout(&validators.accounts[0], by_page)),
                Some((MINIMAL_FEE, CustomFeeDetails::Regular(MINIMAL_FEE)))
            );
        }
    });
}

#[test]
fn payout_batch_fee_quotes_preserve_length_tips_multipliers_and_extension_weight() {
    ext().execute_with(|| {
        let (signer, validators) = fixture(true);
        let call = BatchKind::Batch.call(vec![
            payout(&validators.accounts[0], false),
            payout(&validators.accounts[1], true),
        ]);
        let extrinsic = wire(call.clone(), &signer);
        let info = extrinsic.get_dispatch_info();
        assert!(info.extension_weight.ref_time() > 0);
        let len = extrinsic.encoded_size() as u32;
        let floor = 2 * MINIMAL_FEE;
        // Distinct multipliers catch accidentally applying either one twice.
        let multiplier = FixedU128::saturating_from_integer(7);
        xor_fee::Multiplier::<Runtime>::put(multiplier);
        pallet_transaction_payment::NextFeeMultiplier::<Runtime>::put(
            FixedU128::saturating_from_integer(3),
        );
        let standard = TransactionPayment::compute_fee(0, &info, 0);
        let discounted = standard.div_ceil(100).max(floor);
        let length_fee = TransactionPayment::length_to_fee(len);
        let quote = XorFee::query_info(&extrinsic, &call, len);
        let details = XorFee::query_fee_details(&extrinsic, &call, len);
        assert_eq!(
            quote.partial_fee,
            multiplier.saturating_mul_int(discounted) + length_fee
        );
        assert_eq!(quote.partial_fee, details.final_fee());
        assert_eq!(quote.weight, info.total_weight());
        let inclusion = details.inclusion_fee.unwrap();
        assert_eq!(inclusion.base_fee, 0);
        assert_eq!(inclusion.len_fee, length_fee);
        assert_eq!(
            inclusion.adjusted_weight_fee,
            multiplier.saturating_mul_int(discounted)
        );
        assert!(
            quote.partial_fee > XorFee::compute_fee(len, &call, &call.get_dispatch_info(), 0).0,
            "extension weight participates in both quote and admission price"
        );
        let tip = UNIT / 10;
        let (upfront, custom) = XorFee::compute_fee(len, &call, &info, tip);
        assert_eq!(
            custom,
            Some(CustomFeeDetails::PayoutBatch { minimum_fee: floor })
        );
        assert_eq!(
            upfront,
            quote.partial_fee + multiplier.saturating_mul_int(tip)
        );
        assert_eq!(
            XorFee::compute_fee(len + 100, &call, &info, tip).0 - upfront,
            TransactionPayment::length_to_fee(100)
        );
        for actual_weight in [
            Weight::zero(),
            info.total_weight() / 10,
            info.total_weight(),
        ] {
            let post = PostDispatchInfo {
                actual_weight: Some(actual_weight),
                pays_fee: Pays::Yes,
            };
            let old_actual = TransactionPayment::compute_actual_fee(0, &info, &post, 0);
            let expected = multiplier.saturating_mul_int(old_actual.div_ceil(100).max(floor))
                + length_fee
                + multiplier.saturating_mul_int(tip);
            for result in [
                Ok(()),
                Err(sp_runtime::DispatchError::Other("failed payout")),
            ] {
                assert_eq!(
                    XorFee::compute_actual_fee(len, &info, &post, &result, tip, custom),
                    expected
                );
            }
            assert!(expected <= upfront);
        }
        // Even an immediate error retains a floor for every submitted child.
        // Doubling a skipped tail must double that floor, irrespective of weight refunds.
        let post = PostDispatchInfo {
            actual_weight: Some(Weight::zero()),
            pays_fee: Pays::Yes,
        };
        for count in [1, 2] {
            let call = BatchKind::Batch.call(vec![invalid_payout(&validators.accounts[0]); count]);
            let info = call.get_dispatch_info();
            let (_, custom) = XorFee::compute_fee(0, &call, &info, 0);
            assert_eq!(
                XorFee::compute_actual_fee(
                    0,
                    &info,
                    &post,
                    &Err(pallet_staking::Error::<Runtime>::InvalidPage.into()),
                    0,
                    custom
                ),
                multiplier.saturating_mul_int(MINIMAL_FEE * count as Balance)
            );
        }
    });
}

#[test]
fn payout_batch_must_fund_full_quote_even_when_execution_would_refund() {
    ext().execute_with(|| {
        let (signer, validators) = fixture(false);
        let who = AccountId::from(signer.public());
        let call = BatchKind::Batch.call(vec![
            invalid_payout(&validators.accounts[0]),
            payout(&validators.accounts[1], true),
        ]);
        let extrinsic = wire(call.clone(), &signer);
        let quote =
            XorFee::query_info(&extrinsic, &call, extrinsic.encoded_size() as u32).partial_fee;
        drop(Balances::deposit_creating(&who, quote - 1));
        reset();
        assert_err!(
            Executive::validate_transaction(
                TransactionSource::External,
                wire(call.clone(), &signer),
                Default::default()
            ),
            InvalidTransaction::Payment
        );
        assert_err!(
            Executive::apply_extrinsic(wire(call, &signer)),
            InvalidTransaction::Payment
        );
        assert_eq!(Balances::free_balance(&who), quote - 1);
        assert_eq!(System::account_nonce(&who), 0);
    });
}

#[test]
fn payout_batch_discount_preserves_extrinsic_and_block_weight_admission_limits() {
    ext().execute_with(|| {
        let (signer, validators) = fixture(true);
        let child = payout(&validators.accounts[0], false);
        let mut count = 1;
        let oversized = loop {
            let call = BatchKind::Batch.call(vec![child.clone(); count]);
            let info = call.get_dispatch_info();
            if info.total_weight().any_gt(
                crate::BlockWeights::get()
                    .get(info.class)
                    .max_extrinsic
                    .unwrap(),
            ) {
                break call;
            }
            count += 1;
            assert!(count < 1000);
        };
        let extrinsic = wire(oversized, &signer);
        let info = extrinsic.get_dispatch_info();
        let len = extrinsic.encoded_size() as u32;
        let quote = XorFee::query_info(&extrinsic, &extrinsic.function, len);
        assert!(quote.partial_fee < TransactionPayment::compute_fee(len, &info, 0));
        assert_eq!(quote.weight, info.total_weight());
        let who = AccountId::from(signer.public());
        let before = Balances::free_balance(&who);
        reset();
        assert_err!(
            Executive::validate_transaction(
                TransactionSource::External,
                extrinsic.clone(),
                Default::default()
            ),
            InvalidTransaction::ExhaustsResources
        );
        assert_err!(
            Executive::apply_extrinsic(extrinsic),
            InvalidTransaction::ExhaustsResources
        );
        assert_eq!(Balances::free_balance(&who), before);
        assert_eq!(System::account_nonce(&who), 0);
        assert!(!claimed(&validators.accounts[0]));

        let small = wire(BatchKind::Batch.call(vec![child]), &signer);
        let small_info = small.get_dispatch_info();
        reset();
        System::register_extra_weight_unchecked(
            crate::BlockWeights::get()
                .get(small_info.class)
                .max_total
                .unwrap(),
            small_info.class,
        );
        // Block building discards the candidate overlay on admission failure;
        // CheckNonce prepares before the block-weight extension in this runtime.
        let admission =
            frame_support::storage::with_transaction::<_, sp_runtime::DispatchError, _>(|| {
                frame_support::storage::TransactionOutcome::Rollback(Ok(
                    Executive::apply_extrinsic(small),
                ))
            })
            .unwrap();
        assert_err!(admission, InvalidTransaction::ExhaustsResources);
        assert_eq!(Balances::free_balance(&who), before);
        assert_eq!(System::account_nonce(&who), 0);
        assert!(!claimed(&validators.accounts[0]));
    });
}
