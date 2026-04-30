// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// TODO #167: fix clippy warnings
#![allow(clippy::all)]
#![allow(deprecated)] // TODO: migrate SignedExtension-based tests to TransactionExtension.

use crate::extension::ChargeTransactionPayment;
#[cfg(feature = "wip")] // Xorless fee
use crate::WeightInfo;
use crate::{mock::*, Error, LiquidityInfo, XorToVal};
#[cfg(feature = "wip")] // Dynamic fee
use crate::{CalculateMultiplier, Multiplier, UpdatePeriod};
use common::mock::{alice, bob};
#[cfg(feature = "wip")] // Dynamic fee
use common::prelude::FixedWrapper;
#[cfg(feature = "wip")] // Dynamic fee
use common::weights::constants::SMALL_FEE;
use common::{balance, Balance};
#[cfg(feature = "wip")] // Xorless fee
use common::{KUSD, TBCD, VAL};
use frame_support::assert_err;
#[cfg(feature = "wip")] // Xorless fee
use frame_support::dispatch::GetDispatchInfo;
#[cfg(feature = "wip")] // Dynamic fee
use frame_support::dispatch::{DispatchErrorWithPostInfo, Pays};
use frame_support::error::BadOrigin;
use frame_support::traits::Currency;
use frame_support::weights::{Weight, WeightToFee};
use frame_support::{assert_noop, assert_ok};
use sp_arithmetic::traits::Zero;
use sp_runtime::traits::SignedExtension;
use sp_runtime::transaction_validity::{InvalidTransaction, TransactionValidityError};
use sp_runtime::{FixedPointNumber, FixedU128};

fn set_weight_to_fee_multiplier(mul: u64) {
    // Set WeightToFee multiplier to one to not affect the test
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(mul)
    ));
}

fn free_custom_fee_call() -> RuntimeCall {
    remark_call(FREE_CUSTOM_FEE_REMARK)
}

fn remark_call(remark: &[u8]) -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark {
        remark: remark.to_vec(),
    })
}

#[test]
fn weight_to_fee_works() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(1);
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(100_000_000_000, 0)),
            balance!(0.7)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(500_000_000, 0)),
            balance!(0.0035)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(72_000_000, 0)),
            balance!(0.000504)
        );
        assert_eq!(
            XorFee::weight_to_fee(&Weight::from_parts(210_200_000_000, 0)),
            balance!(1.4714)
        );
    });
}

#[test]
fn weight_to_fee_does_not_underflow() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        assert_eq!(XorFee::weight_to_fee(&Weight::zero()), 0);
    });
}

#[test]
fn weight_to_fee_does_not_overflow() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(1);
        assert_eq!(
            XorFee::weight_to_fee(&Weight::MAX),
            129127208515966861305000000
        );
    });
}

#[test]
fn simple_update_works() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // Update from root
        set_weight_to_fee_multiplier(3);
        assert_eq!(XorFee::multiplier(), FixedU128::saturating_from_integer(3));
    });
}

#[test]
fn non_root_update_fails() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // We allow only root

        assert_noop!(
            XorFee::update_multiplier(RuntimeOrigin::signed(alice()), FixedU128::from(3)),
            BadOrigin
        );

        assert_noop!(
            XorFee::update_multiplier(RuntimeOrigin::none(), FixedU128::from(3)),
            BadOrigin
        );
    });
}

#[test]
fn fees_remain_stable_on_codesub_block_without_forced_override() {
    // This block matches mainnet codeSubstitute activation height.
    const CODESUB_BLOCK: u64 = 23_234_813;

    ExtBuilder::build().execute_with(|| {
        set_weight_to_fee_multiplier(2);
        System::set_block_number(CODESUB_BLOCK - 1);

        let tracked_call = RuntimeCall::Assets(assets::Call::transfer {
            to: alice(),
            asset_id: common::VAL,
            amount: 10,
        });
        let tracked_info = info_from_weight(100.into());
        let tracked_len = 100;

        let (fee_before, _) = XorFee::compute_fee(tracked_len, &tracked_call, &tracked_info, 0);
        assert_eq!(fee_before, balance!(0.0014));

        let multiplier_events_before = System::events()
            .iter()
            .filter(|record| {
                matches!(
                    &record.event,
                    RuntimeEvent::XorFee(crate::Event::WeightToFeeMultiplierUpdated(_))
                )
            })
            .count();

        run_to_block(CODESUB_BLOCK);

        assert_eq!(XorFee::multiplier(), FixedU128::saturating_from_integer(2));
        let (fee_after, _) = XorFee::compute_fee(tracked_len, &tracked_call, &tracked_info, 0);
        assert_eq!(fee_after, fee_before);

        let multiplier_events_after = System::events()
            .iter()
            .filter(|record| {
                matches!(
                    &record.event,
                    RuntimeEvent::XorFee(crate::Event::WeightToFeeMultiplierUpdated(_))
                )
            })
            .count();
        assert_eq!(multiplier_events_before, multiplier_events_after);
    });
}

#[test]
fn default_custom_fee_impl_is_noop() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = bob();
        let call = RuntimeCall::System(frame_system::Call::remark {
            remark: vec![1, 2, 3],
        });
        let info = info_from_weight(100.into());
        let post_info = default_post_info();
        let result = Ok(());

        assert!(
            <() as crate::ApplyCustomFees<RuntimeCall, AccountId>>::should_be_paid(&who, &call)
        );
        assert!(
            !<() as crate::ApplyCustomFees<RuntimeCall, AccountId>>::should_be_postponed(
                &who,
                &fee_source,
                &call,
                balance!(1)
            )
        );
        assert_eq!(
            <() as crate::ApplyCustomFees<RuntimeCall, AccountId>>::get_fee_source(
                &who,
                &call,
                balance!(1)
            ),
            who
        );
        assert_eq!(
            <() as crate::ApplyCustomFees<RuntimeCall, AccountId>>::compute_fee(&call),
            None
        );
        assert_eq!(
            <() as crate::ApplyCustomFees<RuntimeCall, AccountId>>::compute_actual_fee(
                &post_info, &info, &result, None
            ),
            None
        );
    });
}

#[test]
fn zero_custom_fee_details_keep_kind_without_inclusion_fee() {
    ExtBuilder::build().execute_with(|| {
        let multiplier = 3;
        set_weight_to_fee_multiplier(multiplier);

        let call = free_custom_fee_call();
        let info = info_from_weight(100.into());
        let tip = balance!(0.000001);

        let (fee_details, custom_fee_details) =
            XorFee::compute_fee_details(10_000, &call, &info, tip);
        let multiplier = Balance::from(multiplier);

        assert_eq!(custom_fee_details, Some(0));
        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.tip, multiplier * tip);
        assert_eq!(fee_details.final_fee(), multiplier * tip);

        let (fee, custom_fee_details) = XorFee::compute_fee(10_000, &call, &info, tip);
        assert_eq!(custom_fee_details, Some(0));
        assert_eq!(fee, fee_details.final_fee());
    });
}

#[test]
fn zero_custom_actual_fee_details_keep_tip_without_inclusion_fee() {
    ExtBuilder::build().execute_with(|| {
        let multiplier = 4;
        set_weight_to_fee_multiplier(multiplier);

        let len = 10_000;
        let info = info_from_weight(100.into());
        let post_info = post_info_from_weight(50.into());
        let result = Ok(());
        let tip = balance!(0.000002);

        let fee_details =
            XorFee::compute_actual_fee_details(len, &info, &post_info, &result, tip, Some(0));
        let fee = XorFee::compute_actual_fee(len, &info, &post_info, &result, tip, Some(0));
        let multiplier = Balance::from(multiplier);

        assert!(fee_details.inclusion_fee.is_none());
        assert_eq!(fee_details.tip, multiplier * tip);
        assert_eq!(fee_details.final_fee(), multiplier * tip);
        assert_eq!(fee, fee_details.final_fee());
    });
}

#[test]
fn zero_custom_fee_pre_dispatch_does_not_withdraw() {
    ExtBuilder::build().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let who = bob();
        let call = free_custom_fee_call();
        let info = info_from_weight(100.into());
        let len = 100;

        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&who, &call, &info, len)
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(who.clone(), None, None),
                Some(0),
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);

        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info,
            &default_post_info(),
            len,
            &Ok(()),
        )
        .unwrap();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn multiplied_fee_scales_each_inclusion_component_and_tip() {
    ExtBuilder::build().execute_with(|| {
        let multiplier = 7;
        set_weight_to_fee_multiplier(multiplier);

        let fee_details = pallet_transaction_payment::FeeDetails {
            inclusion_fee: Some(pallet_transaction_payment::InclusionFee {
                base_fee: 11,
                len_fee: 17,
                adjusted_weight_fee: 23,
            }),
            tip: 29,
        };

        let multiplied = XorFee::multiplied_fee(fee_details);
        let inclusion_fee = multiplied.inclusion_fee.clone().unwrap();
        let multiplier = Balance::from(multiplier);

        assert_eq!(inclusion_fee.base_fee, multiplier * 11);
        assert_eq!(inclusion_fee.len_fee, multiplier * 17);
        assert_eq!(inclusion_fee.adjusted_weight_fee, multiplier * 23);
        assert_eq!(multiplied.tip, multiplier * 29);
        assert_eq!(multiplied.final_fee(), multiplier * (11 + 17 + 23 + 29));
    });
}

#[test]
fn multiplied_fee_scales_tip_without_inclusion_fee() {
    ExtBuilder::build().execute_with(|| {
        let multiplier = 5;
        set_weight_to_fee_multiplier(multiplier);

        let fee_details = pallet_transaction_payment::FeeDetails {
            inclusion_fee: None,
            tip: balance!(0.000003),
        };

        let multiplied = XorFee::multiplied_fee(fee_details);

        assert!(multiplied.inclusion_fee.is_none());
        assert_eq!(
            multiplied.tip,
            Balance::from(multiplier) * balance!(0.000003)
        );
        assert_eq!(multiplied.final_fee(), multiplied.tip);
    });
}

#[test]
fn validate_zero_custom_fee_succeeds_without_balance() {
    ExtBuilder::build().execute_with(|| {
        set_weight_to_fee_multiplier(1);

        let who = bob();
        let call = free_custom_fee_call();
        let info = info_from_weight(100.into());
        let len = 100;

        let valid = ChargeTransactionPayment::<Runtime>::new()
            .validate(&who, &call, &info, len)
            .unwrap();

        assert_eq!(valid.priority, 0);
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
    });
}

#[test]
fn can_withdraw_fee_short_circuits_zero_fee_and_unpaid_accounts() {
    ExtBuilder::build().execute_with(|| {
        let info = info_from_weight(100.into());
        let call = RuntimeCall::Assets(assets::Call::transfer {
            to: alice(),
            asset_id: common::VAL,
            amount: 10,
        });

        assert_ok!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::can_withdraw_fee(
                &bob(),
                &call,
                &info,
                0,
                0,
            )
        );

        let unpaid_account = GetPaysNoAccountId::get();
        assert_ok!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::can_withdraw_fee(
                &unpaid_account,
                &call,
                &info,
                balance!(1),
                0,
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&unpaid_account), 0);
    });
}

#[test]
fn can_withdraw_fee_uses_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        let fee = XorFee::compute_fee(len as u32, &call, &info, 0).0;

        assert_eq!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::can_withdraw_fee(
                &who, &call, &info, fee, 0,
            ),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );

        let _ = Balances::deposit_creating(&fee_source, fee + Balances::minimum_balance());
        assert_ok!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::can_withdraw_fee(
                &who, &call, &info, fee, 0,
            )
        );
    });
}

#[test]
fn can_withdraw_fee_postponed_account_skips_balance_check() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        let call = RuntimeCall::Assets(assets::Call::transfer {
            to: alice(),
            asset_id: common::VAL,
            amount: 10,
        });
        let info = info_from_weight(100.into());

        assert_ok!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::can_withdraw_fee(
                &who,
                &call,
                &info,
                balance!(1),
                0,
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
    });
}

#[test]
fn can_withdraw_fee_maps_withdraw_errors() {
    ExtBuilder::build().execute_with(|| {
        let info = info_from_weight(100.into());

        for (remark, expected_error) in [
            (
                ASSET_NOT_FOUND_REMARK,
                TransactionValidityError::Invalid(InvalidTransaction::Custom(2)),
            ),
            (
                FEE_CALC_FAILED_REMARK,
                TransactionValidityError::Invalid(InvalidTransaction::Payment),
            ),
            (
                OTHER_WITHDRAW_ERROR_REMARK,
                TransactionValidityError::Invalid(InvalidTransaction::Payment),
            ),
        ] {
            let call = remark_call(remark);
            assert_eq!(
                <XorFee as pallet_transaction_payment::OnChargeTransaction<
                    Runtime,
                >>::can_withdraw_fee(&alice(), &call, &info, balance!(1), 0),
                Err(expected_error)
            );
        }
    });
}

#[test]
fn withdraw_fee_postponed_account_returns_postponed_without_balance() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        let call = RuntimeCall::Assets(assets::Call::transfer {
            to: alice(),
            asset_id: common::VAL,
            amount: 10,
        });
        let info = info_from_weight(100.into());

        let liquidity_info =
            <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::withdraw_fee(
                &who,
                &call,
                &info,
                balance!(1),
                0,
            )
            .unwrap();

        assert_eq!(
            liquidity_info,
            LiquidityInfo::<Runtime>::Postponed(who.clone())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
    });
}

#[test]
fn withdraw_fee_maps_withdraw_errors() {
    ExtBuilder::build().execute_with(|| {
        let info = info_from_weight(100.into());

        for (remark, expected_error) in [
            (
                ASSET_NOT_FOUND_REMARK,
                TransactionValidityError::Invalid(InvalidTransaction::Custom(2)),
            ),
            (
                FEE_CALC_FAILED_REMARK,
                TransactionValidityError::Invalid(InvalidTransaction::Payment),
            ),
            (
                OTHER_WITHDRAW_ERROR_REMARK,
                TransactionValidityError::Invalid(InvalidTransaction::Payment),
            ),
        ] {
            let call = remark_call(remark);
            assert_eq!(
                <XorFee as pallet_transaction_payment::OnChargeTransaction<Runtime>>::withdraw_fee(
                    &alice(),
                    &call,
                    &info,
                    balance!(1),
                    0,
                ),
                Err(expected_error)
            );
        }
    });
}

#[test]
fn correct_and_deposit_fee_not_paid_is_noop() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let info = info_from_weight(100.into());
        let post_info = default_post_info();

        assert_ok!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<
                Runtime,
            >>::correct_and_deposit_fee(
                &who,
                &info,
                &post_info,
                balance!(1),
                0,
                LiquidityInfo::<Runtime>::NotPaid,
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn correct_and_deposit_fee_postponed_with_tip_withdraws_and_distributes() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = GetPostponeAccountId::get();
        let starting_balance = balance!(1000);
        let corrected_fee = balance!(0.0008);
        let tip = balance!(0.000001);
        let info = info_from_weight(100.into());
        let post_info = default_post_info();

        let _ =
            Balances::deposit_creating(&fee_source, starting_balance + Balances::minimum_balance());

        assert_ok!(
            <XorFee as pallet_transaction_payment::OnChargeTransaction<
                Runtime,
            >>::correct_and_deposit_fee(
                &who,
                &info,
                &post_info,
                corrected_fee,
                tip,
                LiquidityInfo::<Runtime>::Postponed(fee_source.clone()),
            )
        );
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            starting_balance - corrected_fee
        );
        assert_eq!(XorToVal::<Runtime>::get(), corrected_fee / 2);
    });
}

#[test]
fn post_dispatch_without_pre_is_noop() {
    ExtBuilder::build().execute_with(|| {
        let info = info_from_weight(100.into());

        assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            None,
            &info,
            &default_post_info(),
            100,
            &Ok(()),
        ));
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_postpone() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        set_weight_to_fee_multiplier(1);
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Postponed(who.clone()),
                Some(balance!(0.0007)),
            )
        );
        let _ = Balances::deposit_creating(&who, balance!(1000) + Balances::minimum_balance());
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info_from_weight(100.into()),
            &post_info_from_weight(100.into()),
            100,
            &Ok(()),
        )
        .unwrap();
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.9993));
        assert_eq!(XorToVal::<Runtime>::get(), balance!(0.00035));
    });
}

#[test]
fn it_fails_postpone() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_from_weight(100.into()),
                &post_info_from_weight(100.into()),
                100,
                &Ok(()),
            ),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_should_not_pay() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPaysNoAccountId::get();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(who.clone(), None, None),
                Some(balance!(0.0007)),
            )
        );
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_from_weight(100.into()),
                &post_info_from_weight(100.into()),
                100,
                &Ok(()),
            ),
            Ok(())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_should_pays_no() {
    ExtBuilder::build().execute_with(|| {
        let who = bob();
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_pays_no(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(who.clone(), None, None),
                None,
            )
        );
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_pays_no(100.into()),
                &post_info_from_weight(100.into()),
                100,
                &Ok(()),
            ),
            Ok(())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_should_post_info_pays_no() {
    ExtBuilder::build().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        let who = bob();
        let _ = Balances::deposit_creating(&who, balance!(1000) + Balances::minimum_balance());
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(
                    who.clone(),
                    Some(pallet_balances::NegativeImbalance::new(balance!(0.0007))),
                    None
                ),
                Some(balance!(0.0007)),
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.9993));
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::post_dispatch(
                Some(pre),
                &info_from_weight(100.into()),
                &post_info_pays_no(),
                100,
                &Ok(()),
            ),
            Ok(())
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        assert_eq!(XorToVal::<Runtime>::get(), 0);
    });
}

#[test]
fn it_works_postpone_with_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = GetPostponeAccountId::get();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100usize;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        let post_info = post_info_from_weight(100.into());
        let result = Ok(());
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        assert_eq!(Balances::usable_balance_for_fees(&fee_source), 0);
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&who, &call, &info, len)
            .unwrap();
        let fee = XorFee::compute_fee(len as u32, &call, &info, 0).0;
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Postponed(fee_source.clone()),
                None,
            )
        );
        let _ =
            Balances::deposit_creating(&fee_source, balance!(1000) + Balances::minimum_balance());
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000)
        );
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info,
            &post_info,
            len,
            &result,
        )
        .unwrap();
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000) - fee
        );
        assert_eq!(XorToVal::<Runtime>::get(), fee / 2);
    });
}

#[test]
fn it_works_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100usize;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        let post_info = post_info_from_weight(100.into());
        let result = Ok(());
        assert_eq!(Balances::usable_balance_for_fees(&who), 0);
        let _ =
            Balances::deposit_creating(&fee_source, balance!(1000) + Balances::minimum_balance());
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000)
        );
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(&who, &call, &info, len)
            .unwrap();
        let fee = XorFee::compute_fee(len as u32, &call, &info, 0).0;
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(
                    fee_source.clone(),
                    Some(pallet_balances::NegativeImbalance::new(fee)),
                    None
                ),
                None,
            )
        );
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000) - fee
        );
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info,
            &post_info,
            len,
            &result,
        )
        .unwrap();
        assert_eq!(
            Balances::usable_balance_for_fees(&fee_source),
            balance!(1000) - fee
        );
        assert_eq!(XorToVal::<Runtime>::get(), fee / 2);
    });
}

#[test]
fn it_fails_custom_fee_source() {
    ExtBuilder::build().execute_with(|| {
        let who = alice();
        let fee_source = GetFeeSourceAccountId::get();
        let len = 100usize;
        let call = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
        let info = info_from_weight(100.into());
        assert_eq!(Balances::usable_balance_for_fees(&fee_source), 0);
        let _ = Balances::deposit_creating(&who, balance!(1000) + Balances::minimum_balance());
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        assert_eq!(
            ChargeTransactionPayment::<Runtime>::new().pre_dispatch(&who, &call, &info, len),
            Err(TransactionValidityError::Invalid(
                InvalidTransaction::Payment
            ))
        );
    });
}

#[test]
fn it_works_referrer_refund() {
    ExtBuilder::build().execute_with(|| {
        set_weight_to_fee_multiplier(1);
        let who = GetReferalAccountId::get();
        let referrer = GetReferrerAccountId::get();
        let _ = Balances::deposit_creating(&who, balance!(1000) + Balances::minimum_balance());
        let _ = Balances::deposit_creating(&referrer, balance!(1000) + Balances::minimum_balance());
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(1000));
        let pre = ChargeTransactionPayment::<Runtime>::new()
            .pre_dispatch(
                &who,
                &RuntimeCall::Assets(assets::Call::transfer {
                    to: alice(),
                    asset_id: common::VAL,
                    amount: 10,
                }),
                &info_from_weight(100.into()),
                100,
            )
            .unwrap();
        assert_eq!(
            pre,
            (
                0,
                who.clone(),
                LiquidityInfo::<Runtime>::Paid(
                    who.clone(),
                    Some(pallet_balances::NegativeImbalance::new(balance!(0.0007))),
                    None
                ),
                Some(balance!(0.0007)),
            )
        );
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.9993));
        ChargeTransactionPayment::<Runtime>::post_dispatch(
            Some(pre),
            &info_from_weight(100.into()),
            &post_info_from_weight(100.into()),
            100,
            &Ok(()),
        )
        .unwrap();
        assert_eq!(Balances::usable_balance_for_fees(&who), balance!(999.9993));
        assert_eq!(
            Balances::usable_balance_for_fees(&referrer),
            balance!(1000.00007)
        );
        assert_eq!(XorToVal::<Runtime>::get(), balance!(0.00035));
    });
}

#[cfg(feature = "wip")] // Dynamic fee
#[test]
fn calculate_multiplier_using_ref_amount_works() {
    ExtBuilder::build().execute_with(|| {
        let input_asset = common::XOR;
        let ref_asset = common::DAI;

        let multiplier = DynamicMultiplier::calculate_multiplier(&input_asset, &ref_asset)
            .unwrap()
            .into_inner();
        let ref_amount = FixedWrapper::from(PRICE_XOR_DAI) * SMALL_FEE * multiplier;

        assert_eq!(ref_amount.into_balance(), SMALL_REFERENCE_AMOUNT);

        assert_noop!(
            DynamicMultiplier::calculate_multiplier(&input_asset, &input_asset),
            Error::<Runtime>::MultiplierCalculationFailed
        );
    });
}

#[cfg(feature = "wip")] // Dynamic fee
#[test]
fn update_multiplier_on_initialize() {
    ExtBuilder::build().execute_with(|| {
        System::set_block_number(0);

        UpdatePeriod::<Runtime>::put(10);
        run_to_block(9);
        assert_eq!(Multiplier::<Runtime>::get(), FixedU128::from(600000));

        run_to_block(15);
        assert_eq!(Multiplier::<Runtime>::get().into_inner(), balance!(1.25));
        assert_eq!(UpdatePeriod::<Runtime>::get(), 10);
    });
}

#[cfg(feature = "wip")] // Dynamic fee
#[test]
fn not_update_multiplier_on_initialize() {
    ExtBuilder::build().execute_with(|| {
        System::set_block_number(0);

        UpdatePeriod::<Runtime>::put(0);
        run_to_block(10);
        assert_eq!(Multiplier::<Runtime>::get(), FixedU128::from(600000));
    });
}

#[cfg(feature = "wip")] // Dynamic fee
#[test]
fn test_set_update_period() {
    ExtBuilder::build().execute_with(|| {
        System::set_block_number(0);

        assert_ok!(XorFee::set_fee_update_period(
            RuntimeOrigin::root(),
            BlockNumber::MAX
        ));
        assert_eq!(UpdatePeriod::<Runtime>::get(), BlockNumber::MAX);
    });
}

#[cfg(feature = "wip")] // Dynamic fee
#[test]
fn test_set_small_reference_amount() {
    ExtBuilder::build().execute_with(|| {
        System::set_block_number(0);

        assert_ok!(XorFee::set_small_reference_amount(
            RuntimeOrigin::root(),
            SMALL_REFERENCE_AMOUNT
        ));
        assert_eq!(XorFee::small_reference_amount(), SMALL_REFERENCE_AMOUNT);
        let expected_error = DispatchErrorWithPostInfo {
            post_info: Pays::Yes.into(),
            error: Error::<Runtime>::InvalidSmallReferenceAmount.into(),
        };
        assert_noop!(
            XorFee::set_small_reference_amount(RuntimeOrigin::root(), balance!(0)),
            expected_error
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn add_to_white_list_works_correct() {
    ExtBuilder::build().execute_with(|| {
        run_to_block(1);
        assert_ok!(XorFee::add_asset_to_white_list(RuntimeOrigin::root(), VAL));
        assert_eq!(XorFee::whitelist_tokens().len(), 1);
        assert_err!(
            XorFee::add_asset_to_white_list(RuntimeOrigin::root(), VAL),
            Error::<Runtime>::AssetAlreadyWhitelisted
        );
        System::assert_last_event(RuntimeEvent::XorFee(crate::Event::AssetAddedToWhiteList(
            VAL,
        )));
        assert_ok!(XorFee::add_asset_to_white_list(RuntimeOrigin::root(), KUSD));
        assert_err!(
            XorFee::add_asset_to_white_list(RuntimeOrigin::root(), TBCD),
            Error::<Runtime>::WhitelistFull
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn remove_from_white_list_works_correct() {
    ExtBuilder::build().execute_with(|| {
        run_to_block(1);
        assert_err!(
            XorFee::remove_asset_from_white_list(RuntimeOrigin::root(), VAL),
            Error::<Runtime>::AssetNotFound
        );
        assert_ok!(XorFee::add_asset_to_white_list(RuntimeOrigin::root(), VAL));
        assert_ok!(XorFee::remove_asset_from_white_list(
            RuntimeOrigin::root(),
            VAL
        ));
        assert_eq!(XorFee::whitelist_tokens().len(), 0);
        System::assert_last_event(RuntimeEvent::XorFee(
            crate::Event::AssetRemovedFromWhiteList(VAL),
        ));
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn test_xorless_call_weight() {
    ExtBuilder::build().execute_with(|| {
        let _ = Balances::deposit_creating(&bob(), balance!(1000) + Balances::minimum_balance());
        let asset_id = Some(VAL.into());

        let mock_call = RuntimeCall::Assets(assets::Call::transfer {
            asset_id: common::XOR.into(),
            to: alice(),
            amount: balance!(1),
        });

        let xorless_weight = RuntimeCall::XorFee(xor_fee::Call::xorless_call {
            call: Box::new(mock_call.clone()),
            asset_id,
        })
        .get_dispatch_info()
        .total_weight();
        let mock_call_weight = mock_call.get_dispatch_info().total_weight();

        let expected_weight =
            <Runtime as Config>::WeightInfo::xorless_call().saturating_add(mock_call_weight);

        assert_eq!(xorless_weight, expected_weight);

        let result = XorFee::xorless_call(
            RuntimeOrigin::signed(bob()),
            Box::new(mock_call.clone()),
            asset_id,
        );

        assert_ok!(result);

        let post_info = result.unwrap();
        assert_eq!(
            post_info.actual_weight.expect("Error while get weight"),
            expected_weight
        );
    });
}

#[cfg(feature = "wip")] // Xorless fee
#[test]
fn test_xorless_call_failed_inner_call() {
    ExtBuilder::build().execute_with(|| {
        let _ = Balances::deposit_creating(&bob(), balance!(1));
        let mock_call = RuntimeCall::Assets(assets::Call::transfer {
            asset_id: VAL.into(),
            to: alice(),
            amount: balance!(1),
        });
        let mock_call_weight = mock_call.get_dispatch_info().total_weight();

        let asset_id = None;

        let expected_weight =
            <Runtime as Config>::WeightInfo::xorless_call().saturating_add(mock_call_weight);

        let result = XorFee::xorless_call(
            RuntimeOrigin::signed(bob()),
            Box::new(mock_call.clone()),
            asset_id,
        );

        let err = result.unwrap_err();
        assert_eq!(err.error, tokens::Error::<Runtime>::BalanceTooLow.into());
        assert_eq!(err.post_info.actual_weight.unwrap(), expected_weight);
    });
}

#[test]
fn non_root_scale_fails() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        // We allow only root

        assert_noop!(
            XorFee::scale_multiplier(RuntimeOrigin::signed(alice()), FixedU128::from(3)),
            BadOrigin
        );

        assert_noop!(
            XorFee::scale_multiplier(RuntimeOrigin::none(), FixedU128::from(3)),
            BadOrigin
        );
    });
}

#[test]
fn zero_scale_fails() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(2);

        assert_err!(
            XorFee::scale_multiplier(RuntimeOrigin::root(), FixedU128::zero()),
            Error::<Runtime>::MultiplierCalculationFailed
        );

        XorFee::update_multiplier(RuntimeOrigin::root(), FixedU128::from_inner(1)).unwrap();
        assert_err!(
            XorFee::scale_multiplier(RuntimeOrigin::root(), FixedU128::from_inner(1)),
            Error::<Runtime>::MultiplierCalculationFailed
        );
    });
}

#[test]
fn scale_ok() {
    let mut ext = ExtBuilder::build();
    ext.execute_with(|| {
        set_weight_to_fee_multiplier(2);

        assert_ok!(XorFee::scale_multiplier(
            RuntimeOrigin::root(),
            FixedU128::from_float(2.25)
        ));

        assert_eq!(XorFee::multiplier(), FixedU128::from_float(4.5));

        assert_ok!(XorFee::scale_multiplier(
            RuntimeOrigin::root(),
            FixedU128::from(u128::MAX)
        ));

        assert_eq!(XorFee::multiplier(), FixedU128::from(u128::MAX));
    });
}
