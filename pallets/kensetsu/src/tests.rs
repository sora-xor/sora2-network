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

use super::*;

use crate::mock::{new_test_ext, RuntimeOrigin, TestRuntime};
use crate::test_utils::{
    alice, alice_account_id, assert_bad_debt, assert_balance, bob, create_cdp_for_xor,
    deposit_xor_to_cdp, get_total_supply, risk_manager, set_bad_debt, set_balance,
    set_xor_as_collateral_type, tech_account_id,
};

use common::{balance, AssetId32, Balance, KUSD, XOR};
use frame_support::{assert_err, assert_ok};
use hex_literal::hex;
use sp_arithmetic::{ArithmeticError, Percent};
use sp_core::U256;
use sp_runtime::DispatchError::BadOrigin;

type KensetsuError = Error<TestRuntime>;
type KensetsuPallet = Pallet<TestRuntime>;
type System = frame_system::Pallet<TestRuntime>;

/// CDP might be created only by Signed Origin account.
#[test]
fn test_create_cdp_only_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_err!(
            KensetsuPallet::create_cdp(RuntimeOrigin::none(), XOR.into()),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::create_cdp(RuntimeOrigin::root(), XOR.into()),
            BadOrigin
        );
    });
}

/// Collateral Risk Parameters were not set for the AssetId by Risk Management Team,
/// is is restricted to create CDP for collateral not listed.
#[test]
fn test_create_cdp_for_asset_not_listed_must_result_in_error() {
    new_test_ext().execute_with(|| {
        assert_err!(
            KensetsuPallet::create_cdp(alice(), XOR.into()),
            KensetsuError::CollateralInfoNotFound
        );
    });
}

/// If the number of cdp ids reached U256::MAX, next CDP will result in ArithmeticError.
#[test]
fn test_create_cdp_overflow_error() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        NextCDPId::<TestRuntime>::set(U256::MAX);

        assert_err!(
            KensetsuPallet::create_cdp(alice(), XOR.into()),
            KensetsuError::ArithmeticError
        );
    });
}

/// Successfully creates CDP
#[test]
fn test_create_cdp_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );

        assert_ok!(KensetsuPallet::create_cdp(alice(), XOR.into()),);
        let cdp_id = U256::from(1);

        System::assert_last_event(
            Event::CDPCreated {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
            }
            .into(),
        );
        assert_eq!(
            KensetsuPallet::get_account_cdp_ids(&alice_account_id()),
            Ok(vec!(cdp_id))
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Shall create CDP");
        assert_eq!(cdp.owner, alice_account_id());
        assert_eq!(cdp.collateral_asset_id, XOR.into());
        assert_eq!(cdp.collateral_amount, balance!(0));
        assert_eq!(cdp.debt, balance!(0));
    });
}

/// CDP might be closed only by Signed Origin account.
#[test]
fn test_close_cdp_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::close_cdp(RuntimeOrigin::none(), cdp_id),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::close_cdp(RuntimeOrigin::root(), cdp_id),
            BadOrigin
        );
    });
}

/// Only owner can close CDP
#[test]
fn test_close_cdp_only_owner() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        // Alice is CDP owner
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_err!(
            KensetsuPallet::close_cdp(bob(), cdp_id),
            KensetsuError::OperationPermitted
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_close_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::close_cdp(alice(), cdp_id),
            KensetsuError::CDPNotFound
        );
    });
}

/// Doesn't allow to close CDP with outstanding debt
#[test]
fn test_close_cdp_outstanding_debt() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(10), balance!(1));

        assert_err!(
            KensetsuPallet::close_cdp(alice(), cdp_id),
            KensetsuError::OutstandingDebt
        );
    });
}

/// Closes CDP and returns collateral to the owner
#[test]
fn test_close_cdp_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(10), balance!(0));
        assert_balance(&alice_account_id(), &XOR.into(), balance!(0));

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id));

        System::assert_last_event(
            Event::CDPClosed {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR.into(), balance!(10));
        assert_eq!(KensetsuPallet::cdp(cdp_id), None);
    });
}

/// only by Signed Origin account can deposit collateral
#[test]
fn test_deposit_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::deposit_collateral(RuntimeOrigin::none(), cdp_id, balance!(0)),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::deposit_collateral(RuntimeOrigin::root(), cdp_id, balance!(0)),
            BadOrigin
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_deposit_collateral_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::deposit_collateral(alice(), cdp_id, balance!(0)),
            KensetsuError::CDPNotFound
        );
    });
}

/// Not enough balance to deposit
#[test]
fn test_deposit_collateral_not_enough_balance() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_err!(
            KensetsuPallet::deposit_collateral(alice(), cdp_id, balance!(1)),
            pallet_balances::Error::<TestRuntime>::InsufficientBalance
        );
    });
}

/// Balance::MAX deposited, increase collateral results in ArithmeticError
#[test]
fn test_deposit_collateral_overlow() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        // due to cast to i128 in update_balance() u128::MAX is done with 2 x i128::MAX
        let max_i128_amount = Balance::from(u128::MAX / 2);
        let cdp_id = create_cdp_for_xor(alice(), max_i128_amount, balance!(0));
        deposit_xor_to_cdp(alice(), cdp_id, max_i128_amount);
        set_balance(alice_account_id(), max_i128_amount);

        // ArithmeticError::Overflow from pallet_balances
        assert_err!(
            KensetsuPallet::deposit_collateral(alice(), cdp_id, max_i128_amount),
            ArithmeticError::Overflow
        );
    });
}

/// Alice deposits 0 collateral, balance not changed, event is emitted
#[test]
fn test_deposit_collateral_zero() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));
        let amount = balance!(0);

        assert_ok!(KensetsuPallet::deposit_collateral(alice(), cdp_id, amount));

        System::assert_last_event(
            Event::CollateralDeposit {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.collateral_amount, amount);
    });
}

/// Alice deposits `amount` collateral, balance changed, event is emitted
#[test]
fn test_deposit_collateral_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));
        let amount = balance!(10);
        set_balance(alice_account_id(), amount);

        assert_ok!(KensetsuPallet::deposit_collateral(alice(), cdp_id, amount));

        System::assert_last_event(
            Event::CollateralDeposit {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR.into(), balance!(0));
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.collateral_amount, amount);
    });
}

/// only by Signed Origin account can withdraw_collateral
#[test]
fn test_withdraw_collateral_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::withdraw_collateral(RuntimeOrigin::none(), cdp_id, balance!(0)),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::withdraw_collateral(RuntimeOrigin::root(), cdp_id, balance!(0)),
            BadOrigin
        );
    });
}

/// Only owner can withdraw collateral
#[test]
fn test_withdraw_collateral_only_owner() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        // Alice is CDP owner
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_err!(
            KensetsuPallet::withdraw_collateral(bob(), cdp_id, balance!(0)),
            KensetsuError::OperationPermitted
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_withdraw_collateral_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::withdraw_collateral(alice(), cdp_id, balance!(0)),
            KensetsuError::CDPNotFound
        );
    });
}

/// CDP owner withdraws more than CDP has
#[test]
fn test_withdraw_collateral_gt_amount() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(10), balance!(0));

        assert_err!(
            KensetsuPallet::withdraw_collateral(alice(), cdp_id, balance!(20)),
            KensetsuError::NotEnoughCollateral
        );
    });
}

/// CDP will be unsafe
#[test]
fn test_withdraw_collateral_unsafe() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(10), balance!(5));

        assert_err!(
            KensetsuPallet::withdraw_collateral(alice(), cdp_id, balance!(1)),
            KensetsuError::CDPUnsafe
        );
    });
}

/// Alice withdraw 0 collateral, balance not changed, event is emitted
#[test]
fn test_withdraw_collateral_zero() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let amount = balance!(0);
        let cdp_id = create_cdp_for_xor(alice(), amount, balance!(0));

        assert_ok!(KensetsuPallet::withdraw_collateral(alice(), cdp_id, amount));

        System::assert_last_event(
            Event::CollateralWithdrawn {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR.into(), amount);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.collateral_amount, balance!(0));
    });
}

/// Alice withdraw `amount` collateral, balance changed, event is emitted
#[test]
fn test_withdraw_collateral_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let amount = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), amount, balance!(0));
        assert_balance(&alice_account_id(), &XOR.into(), balance!(0));

        assert_ok!(KensetsuPallet::withdraw_collateral(alice(), cdp_id, amount));

        System::assert_last_event(
            Event::CollateralWithdrawn {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR.into(), amount);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.collateral_amount, balance!(0));
    });
}

/// only by Signed Origin account can borrow
#[test]
fn test_borrow_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::borrow(RuntimeOrigin::none(), cdp_id, balance!(0)),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::borrow(RuntimeOrigin::root(), cdp_id, balance!(0)),
            BadOrigin
        );
    });
}

/// Only owner can borrow
#[test]
fn test_borrow_only_owner() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        // Alice is CDP owner
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_err!(
            KensetsuPallet::borrow(bob(), cdp_id, balance!(0)),
            KensetsuError::OperationPermitted
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_borrow_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::borrow(alice(), cdp_id, balance!(0)),
            KensetsuError::CDPNotFound
        );
    });
}

/// CDP with debt as MAX_INT exists, borrow from CDP must result in overflow error.
#[test]
fn test_borrow_cdp_overflow() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(100),
            FixedU128::from_float(0.0),
        );
        // due to cast to i128 in update_balance() u128::MAX is done with 2 x i128::MAX
        let max_i128_amount = Balance::from(u128::MAX / 2);
        let cdp_id = create_cdp_for_xor(alice(), max_i128_amount, max_i128_amount);

        assert_err!(
            KensetsuPallet::borrow(alice(), cdp_id, u128::MAX),
            KensetsuError::ArithmeticError
        );
    });
}

/// CDP with collateral exists, try to borrow in a way that results in unsafe CDP.
#[test]
fn test_borrow_cdp_unsafe() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let amount = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), amount, balance!(0));

        assert_err!(
            KensetsuPallet::borrow(alice(), cdp_id, amount),
            KensetsuError::CDPUnsafe
        );
    });
}

/// CDP with collateral exists, hard cap is set in CDP risk parameters.
/// Borrow results with an error `HardCapSupply`
#[test]
fn test_borrow_cdp_cdp_type_hard_cap() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            balance!(10),
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(0));

        assert_err!(
            KensetsuPallet::borrow(alice(), cdp_id, balance!(20)),
            KensetsuError::HardCapSupply
        );
    });
}

/// CDP with collateral exists, hard cap is set in protocol risk parameters.
/// Borrow results with an error `HardCapSupply`
#[test]
fn test_borrow_cdp_protocol_hard_cap() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        assert_ok!(KensetsuPallet::update_hard_cap_total_supply(
            risk_manager(),
            balance!(10)
        ));
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(0));

        assert_err!(
            KensetsuPallet::borrow(alice(), cdp_id, balance!(20)),
            KensetsuError::HardCapSupply
        );
    });
}

/// CDP with collateral exists, call borrow with 0 KUSD amount.
/// Tx must succeed, but state is unchanged.
#[test]
fn test_borrow_cdp_zero_amount() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);

        assert_ok!(KensetsuPallet::borrow(alice(), cdp_id, balance!(0)));

        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount: debt,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt);
        assert_balance(&alice_account_id(), &KUSD.into(), debt);
    });
}

/// CDP with collateral exists, call borrow with some KUSD amount.
/// Tx must succeed, debt to CDP added, KUSD minted to the caller.
#[test]
fn test_borrow_cdp_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(0));
        let to_borrow = balance!(10);
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, balance!(0));

        assert_ok!(KensetsuPallet::borrow(alice(), cdp_id, to_borrow));

        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount: to_borrow,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, to_borrow);
        assert_balance(&alice_account_id(), &KUSD.into(), to_borrow);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + to_borrow);
    });
}

/// CDP with collateral and debt exists, call borrow with 0 KUSD amount to trigger accrue().
/// Tx must succeed, debt will increase on accrued interest, KUSD minted to tech account.
#[test]
fn test_borrow_cdp_accrue() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.1),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, balance!(10));

        assert_ok!(KensetsuPallet::borrow(alice(), cdp_id, balance!(0)));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - persecond rate, 1 - seconds passed
        let interest = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_balance(&alice_account_id(), &KUSD.into(), balance!(10));
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD.into(), balance!(1));
    });
}

/// only by Signed Origin account can repay_debt
#[test]
fn test_repay_debt_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::repay_debt(RuntimeOrigin::none(), cdp_id, balance!(0)),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::repay_debt(RuntimeOrigin::root(), cdp_id, balance!(0)),
            BadOrigin
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_repay_debt_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::repay_debt(alice(), cdp_id, balance!(1)),
            KensetsuError::CDPNotFound
        );
    });
}

/// Repay when amount is less than debt.
/// Debt is partially closed, tokens are burned. Event is emitted.
#[test]
fn test_repay_debt_amount_less_debt() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        let to_repay = balance!(1);
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, to_repay));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount: to_repay,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt - to_repay);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_total_kusd_supply - to_repay);
    });
}

/// Repay when amount is equal to debt.
/// Debt is closed, tokens are burned. Event is emitted.
#[test]
fn test_repay_debt_amount_eq_debt() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, debt));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount: debt,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, balance!(0));
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, balance!(0));
    });
}

/// Repay when amount is greater than debt.
/// Debt is closed, tokens are burned. Event is emitted and KUSD leftover on caller account.
#[test]
fn test_repay_debt_amount_gt_debt() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        // create 2nd CDP and borrow for KUSD surplus on Alice account
        let kusd_surplus = balance!(5);
        create_cdp_for_xor(alice(), balance!(100), kusd_surplus);
        let total_kusd_balance = debt + kusd_surplus;
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, total_kusd_balance);

        assert_ok!(KensetsuPallet::repay_debt(
            alice(),
            cdp_id,
            total_kusd_balance
        ));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount: debt,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, balance!(0));
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, kusd_surplus);
        assert_balance(&alice_account_id(), &KUSD.into(), kusd_surplus);
    });
}

/// Repay whith zero amount.
/// Success, but state is not changed.
#[test]
fn test_repay_debt_zero_amount() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, balance!(0)));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
                amount: balance!(0),
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_total_kusd_supply);
    });
}

/// Repay whith zero amount to trigger accrue.
/// Success, debt increased and KUSD is minted to tech treasury account.
#[test]
fn test_repay_debt_accrue() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.1),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        let initial_total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(initial_total_kusd_supply, debt);
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, balance!(0)));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - persecond rate, 1 - seconds passed
        let interest = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_balance(&alice_account_id(), &KUSD.into(), balance!(10));
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD.into(), balance!(1));
    });
}

// TODO test liquidate
//  - signed account
//  - cdp not found
//  - cdp safe
//  - collateral_amount > cdp.collateral_amount
// cdp_debt > kusd_amount
//   - cdp_collateral_amount < collateral_amount
//   - cdp_collateral_amount == collateral_amount
//   - cdp_collateral_amount > collateral_amount
// cdp_debt == kusd_amount
//   - cdp_collateral_amount < collateral_amount
//   - cdp_collateral_amount == collateral_amount
//   - cdp_collateral_amount > collateral_amount
// cdp_debt < kusd_amount
//   -  liquidation_penalty > leftover
//   -  liquidation_penalty == leftover
//   -  liquidation_penalty < leftover

/// If cdp doesn't exist, return error
#[test]
fn test_accrue_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id),
            KensetsuError::CDPNotFound
        );
    });
}

/// If cdp doesn't have debt, return NoDebt error
#[test]
fn test_accrue_no_debt() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(0));

        assert_err!(
            KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id),
            KensetsuError::NoDebt
        );
    });
}

/// If cdp was updated, and then called with wrong time, return AccrueWrongTime
#[test]
fn test_accrue_wrong_time() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(10));
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_err!(
            KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id),
            KensetsuError::AccrueWrongTime
        );
    });
}

/// If cdp accrue results with overflow, return ArithmeticError
#[test]
fn test_accrue_overflow() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // This big number will result with overflow
            FixedU128::from_float(9999999.0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(50));
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(9999);

        assert_err!(
            KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id),
            KensetsuError::ArithmeticError
        );
    });
}

/// Given: CDP with debt, protocol has no bad debt
/// When: accrue is called
/// Then: interest is counted as CDP debt and goes to protocol profit
#[test]
fn test_accrue_profit() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 10% per second
            FixedU128::from_float(0.1),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_kusd_supply = get_total_supply(&KUSD.into());

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - persecond rate, 1 - seconds passed
        let interest = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_eq!(cdp.last_fee_update_time, 1);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD, interest);
    });
}

/// Given: CDP with debt, was updated this time, protocol has no bad debt
/// When: accrue is called again with the same time
/// Then: success, no state changes
#[test]
fn test_accrue_profit_same_time() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 10% per second
            FixedU128::from_float(0.1),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        let initial_kusd_supply = get_total_supply(&KUSD.into());
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        // double call should not fail
        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));
        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - persecond rate, 1 - seconds passed
        let interest = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_eq!(cdp.last_fee_update_time, 1);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD, interest);
    });
}

/// Given: CDP with debt, protocol has bad debt and interest accrued < bad debt
/// When: accrue is called
/// Then: interest covers the part of bad debt
#[test]
fn test_accrue_interest_less_bad_debt() {
    new_test_ext().execute_with(|| {
        set_bad_debt(balance!(2));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 20% per second
            FixedU128::from_float(0.1),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_kusd_supply = get_total_supply(&KUSD.into());

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*20%*1 = 1 KUSD,
        // where 10 - initial balance, 20% - persecond rate, 1 - seconds passed
        // and 1 KUSD covers the part of bad debt
        let interest = balance!(1);
        let new_bad_debt = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_eq!(cdp.last_fee_update_time, 1);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_kusd_supply);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(new_bad_debt);
    });
}

/// Given: CDP with debt, protocol has bad debt and interest accrued == bad debt
/// When: accrue is called
/// Then: interest covers the part of bad debt
#[test]
fn test_accrue_interest_eq_bad_debt() {
    new_test_ext().execute_with(|| {
        set_bad_debt(balance!(1));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 20% per second
            FixedU128::from_float(0.1),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        let initial_kusd_supply = get_total_supply(&KUSD.into());
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*20%*1 = 1 KUSD,
        // where 10 - initial balance, 10% - persecond rate, 1 - seconds passed
        // and 1 KUSD covers bad debt
        let interest = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_eq!(cdp.last_fee_update_time, 1);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_kusd_supply);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(0));
    });
}

/// Given: CDP with debt, protocol has bad debt and interest accrued > bad debt
/// When: accrue is called
/// Then: interest covers the bad debt and leftover goes to protocol profit
#[test]
fn test_accrue_interest_gt_bad_debt() {
    new_test_ext().execute_with(|| {
        set_bad_debt(balance!(1));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 20% per second
            FixedU128::from_float(0.2),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_kusd_supply = get_total_supply(&KUSD.into());

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*20%*1 = 2 KUSD,
        // where 10 - initial balance, 20% - persecond rate, 1 - seconds passed
        // and 1 KUSD covers bad debt, 1 KUSD is a protocol profit
        let interest = balance!(2);
        let profit = balance!(1);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_eq!(cdp.last_fee_update_time, 1);
        let total_kusd_supply = get_total_supply(&KUSD.into());
        assert_eq!(total_kusd_supply, initial_kusd_supply + profit);
        assert_balance(&tech_account_id(), &KUSD, profit);
        assert_bad_debt(balance!(0));
    });
}

/// Only Signed Origin account can update risk parameters
#[test]
fn test_update_collateral_risk_parameters_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let parameters = CollateralRiskParameters {
            max_supply: balance!(100),
            liquidation_ratio: Perbill::from_percent(50),
            stability_fee_rate: FixedU128::from_float(0.1),
        };

        assert_err!(
            KensetsuPallet::update_collateral_risk_parameters(
                RuntimeOrigin::none(),
                XOR,
                parameters.clone()
            ),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::update_collateral_risk_parameters(
                RuntimeOrigin::root(),
                XOR,
                parameters
            ),
            BadOrigin
        );
    });
}

/// Only existing assets ids are allowed
#[test]
fn test_update_collateral_risk_wrong_asset_id() {
    new_test_ext().execute_with(|| {
        let parameters = CollateralRiskParameters {
            max_supply: balance!(100),
            liquidation_ratio: Perbill::from_percent(50),
            stability_fee_rate: FixedU128::from_float(0.1),
        };
        let wrong_asset_id = AssetId32::from_bytes(hex!(
            "0000000000000000000000000000000000000000000000000000000007654321"
        ));

        assert_err!(
            KensetsuPallet::update_collateral_risk_parameters(
                risk_manager(),
                wrong_asset_id,
                parameters.clone()
            ),
            KensetsuError::WrongAssetId
        );
    });
}

/// Given: several CDP with debt and time since last interest accrue has passed
/// When: update risk parameters with new rate
/// Then: interest is updated
#[test]
fn test_update_collateral_risk_parameters_sunny_day() {
    new_test_ext().execute_with(|| {
        let asset_id = XOR;
        // old stability fee is 10%
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.1),
        );
        let cdp_id_1 = create_cdp_for_xor(alice(), balance!(100), balance!(10));
        let cdp_id_2 = create_cdp_for_xor(alice(), balance!(100), balance!(20));
        // new parameters with stability fee 20%
        let parameters = CollateralRiskParameters {
            max_supply: balance!(100),
            liquidation_ratio: Perbill::from_percent(50),
            stability_fee_rate: FixedU128::from_float(0.2),
        };
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_ok!(KensetsuPallet::update_collateral_risk_parameters(
            risk_manager(),
            asset_id,
            parameters.clone()
        ));

        System::assert_has_event(
            Event::CollateralRiskParametersUpdated {
                collateral_asset_id: XOR,
                risk_parameters: parameters.clone(),
            }
            .into(),
        );
        let actual_parameters =
            CollateralTypes::<TestRuntime>::get(asset_id).expect("Must succeed");
        assert_eq!(actual_parameters, parameters);
        // check that accrue was called
        let cdp_1 = KensetsuPallet::cdp(cdp_id_1).expect("Must exist");
        // debt principal is 10 + 1 fee
        assert_eq!(cdp_1.debt, balance!(11));
        let cdp_2 = KensetsuPallet::cdp(cdp_id_2).expect("Must exist");
        // debt principal is 20 + 2 fee
        assert_eq!(cdp_2.debt, balance!(22));
    });
}

/// Only Signed Origin account can update hard cap
#[test]
fn test_update_hard_cap_only_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_err!(
            KensetsuPallet::update_hard_cap_total_supply(RuntimeOrigin::none(), balance!(0)),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::update_hard_cap_total_supply(RuntimeOrigin::root(), balance!(0)),
            BadOrigin
        );
    });
}

/// Risk manager can update hard cap
#[test]
fn test_update_hard_cap_sunny_day() {
    new_test_ext().execute_with(|| {
        let hard_cap = balance!(100);

        assert_ok!(KensetsuPallet::update_hard_cap_total_supply(
            risk_manager(),
            hard_cap
        ));

        System::assert_has_event(Event::KusdHardCapUpdated { hard_cap }.into());
        assert_eq!(hard_cap, KusdHardCap::<TestRuntime>::get());
    });
}

/// Only Signed Origin account can update hard cap
#[test]
fn test_update_liquidation_penalty_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let liquidation_penalty = Percent::from_percent(10);

        assert_err!(
            KensetsuPallet::update_liquidation_penalty(RuntimeOrigin::none(), liquidation_penalty),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::update_liquidation_penalty(RuntimeOrigin::root(), liquidation_penalty),
            BadOrigin
        );
    });
}

/// Risk manager can update hard cap
#[test]
fn test_update_liquidation_penalty_sunny_day() {
    new_test_ext().execute_with(|| {
        let liquidation_penalty = Percent::from_percent(10);

        assert_ok!(KensetsuPallet::update_liquidation_penalty(
            risk_manager(),
            liquidation_penalty
        ));

        System::assert_has_event(
            Event::LiquidationPenaltyUpdated {
                liquidation_penalty,
            }
            .into(),
        );
        assert_eq!(
            liquidation_penalty,
            LiquidationPenalty::<TestRuntime>::get()
        );
    });
}

/// Only Signed Origin account can donate to protocol
#[test]
fn test_donate_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let donation = balance!(10);

        assert_err!(
            KensetsuPallet::donate(RuntimeOrigin::none(), donation),
            BadOrigin
        );
        assert_err!(
            KensetsuPallet::donate(RuntimeOrigin::root(), donation),
            BadOrigin
        );
    });
}

/// Donation to protocol without bad debt goes to protocol profit.
#[test]
fn test_donate_no_bad_debt() {
    new_test_ext().execute_with(|| {
        let donation = balance!(10);
        // Alice has 10 KUSD
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(Event::Donation { amount: donation }.into());
        assert_balance(&alice_account_id(), &KUSD, balance!(0));
        assert_balance(&tech_account_id(), &KUSD, donation);
        assert_bad_debt(balance!(0));
    });
}

/// Donation to protocol with bad debt and donation < bad debt.
/// Donation partly covers bad debt.
#[test]
fn test_donate_donation_less_bad_debt() {
    new_test_ext().execute_with(|| {
        let initial_bad_debt = balance!(20);
        set_bad_debt(initial_bad_debt);
        let donation = balance!(10);
        // Alice has 10 KUSD
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(Event::Donation { amount: donation }.into());
        assert_balance(&alice_account_id(), &KUSD, balance!(0));
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(initial_bad_debt - donation);
    });
}

/// Donation to protocol with bad debt and donation == bad debt.
/// Donation covers bad debt.
#[test]
fn test_donate_donation_eq_bad_debt() {
    new_test_ext().execute_with(|| {
        let initial_bad_debt = balance!(10);
        set_bad_debt(initial_bad_debt);
        let donation = balance!(10);
        // Alice has 10 KUSD
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(Event::Donation { amount: donation }.into());
        assert_balance(&alice_account_id(), &KUSD, balance!(0));
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(0));
    });
}

/// Donation to protocol with bad debt and donation > bad debt.
/// Donation covers bad debt.
#[test]
fn test_donate_donation_gt_bad_debt() {
    new_test_ext().execute_with(|| {
        let initial_bad_debt = balance!(5);
        set_bad_debt(initial_bad_debt);
        let donation = balance!(10);
        // Alice has 10 KUSD
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(Event::Donation { amount: donation }.into());
        assert_balance(&alice_account_id(), &KUSD, balance!(0));
        assert_balance(&tech_account_id(), &KUSD, donation - initial_bad_debt);
        assert_bad_debt(balance!(0));
    });
}

// TODO test withdraw_profit
// - signed account
// - sunny day, event
// - not enough profit, check event

// TODO add tests for offchain worker accrue()
