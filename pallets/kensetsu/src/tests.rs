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

use crate::mock::{new_test_ext, MockLiquidityProxy, RuntimeOrigin, TestRuntime};
use crate::test_utils::{
    add_balance, alice, alice_account_id, assert_bad_debt, assert_balance, bob, bob_account_id,
    create_cdp_for_xor, deposit_xor_to_cdp, get_total_supply, make_cdps_unsafe, set_bad_debt,
    set_borrow_tax, set_xor_as_collateral_type, tech_account_id,
};

use common::{balance, AssetId32, Balance, KEN, KUSD, XOR};
use frame_support::{assert_noop, assert_ok};
use hex_literal::hex;
use sp_arithmetic::{ArithmeticError, Percent};
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::{One, Zero};
use sp_runtime::DispatchError::BadOrigin;

type KensetsuError = Error<TestRuntime>;
type KensetsuPallet = Pallet<TestRuntime>;
type System = frame_system::Pallet<TestRuntime>;

/// CDP might be created only by Signed Origin account.
#[test]
fn test_create_cdp_only_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            KensetsuPallet::create_cdp(
                RuntimeOrigin::none(),
                XOR,
                balance!(0),
                balance!(0),
                balance!(0)
            ),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::create_cdp(
                RuntimeOrigin::root(),
                XOR,
                balance!(0),
                balance!(0),
                balance!(0)
            ),
            BadOrigin
        );
    });
}

/// Collateral Risk Parameters were not set for the AssetId by Risk Management Team,
/// is is restricted to create CDP for collateral not listed.
#[test]
fn test_create_cdp_for_asset_not_listed_must_result_in_error() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            KensetsuPallet::create_cdp(alice(), XOR, balance!(0), balance!(0), balance!(0)),
            KensetsuError::CollateralInfoNotFound
        );
    });
}

/// If the number of cdp ids reached u128::MAX, next CDP will result in ArithmeticError.
#[test]
fn test_create_cdp_overflow_error() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        NextCDPId::<TestRuntime>::set(CdpId::MAX);

        assert_noop!(
            KensetsuPallet::create_cdp(alice(), XOR, balance!(0), balance!(0), balance!(0)),
            KensetsuError::ArithmeticError
        );
    });
}

/// Create CDP should fail if collateral is under required minimal limit.
#[test]
fn test_create_cdp_collateral_below_minimal() {
    new_test_ext().execute_with(|| {
        let minimal_balance = balance!(100);
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            minimal_balance,
        );

        assert_noop!(
            KensetsuPallet::create_cdp(alice(), XOR, balance!(0), balance!(0), balance!(0)),
            KensetsuError::CollateralBelowMinimal
        );
    });
}

/// Test create_cdp call with wrong parameters: min_borrow_amount > max_borrow_amount
#[test]
fn test_create_cdp_wrong_parameters() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );

        assert_noop!(
            KensetsuPallet::create_cdp(alice(), XOR, balance!(0), balance!(100), balance!(10)),
            KensetsuError::WrongBorrowAmounts
        );
    });
}

/// Successfully creates CDP
#[test]
fn test_create_cdp_sunny_day() {
    new_test_ext().execute_with(|| {
        let collateral = balance!(10);
        let collateral_minimal_balance = collateral;
        let debt = balance!(2);
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            collateral_minimal_balance,
        );
        add_balance(alice_account_id(), collateral, XOR);

        assert_ok!(KensetsuPallet::create_cdp(
            alice(),
            XOR,
            collateral,
            debt,
            debt
        ));
        let cdp_id = 1;

        System::assert_has_event(
            Event::CDPCreated {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                debt_asset_id: KUSD,
                cdp_type: CdpType::Type2,
            }
            .into(),
        );
        System::assert_has_event(
            Event::CollateralDeposit {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                amount: collateral,
            }
            .into(),
        );
        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: debt,
            }
            .into(),
        );
        assert_eq!(
            KensetsuPallet::get_account_cdp_ids(&alice_account_id()),
            Ok(vec!(cdp_id))
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Shall create CDP");
        assert_eq!(cdp.owner, alice_account_id());
        assert_eq!(cdp.collateral_asset_id, XOR);
        assert_eq!(cdp.collateral_amount, collateral);
        assert_eq!(cdp.debt, debt);
        assert_eq!(
            KensetsuPallet::cdp_owner_index(alice_account_id()),
            Some(BoundedVec::try_from(vec![1]).unwrap())
        );
    });
}

/// CDP might be closed only by Signed Origin account.
#[test]
fn test_close_cdp_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::close_cdp(RuntimeOrigin::none(), cdp_id),
            BadOrigin
        );
        assert_noop!(
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
            balance!(0),
        );
        // Alice is CDP owner
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_noop!(
            KensetsuPallet::close_cdp(bob(), cdp_id),
            KensetsuError::OperationNotPermitted
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_close_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::close_cdp(alice(), cdp_id),
            KensetsuError::CDPNotFound
        );
    });
}

/// When CDP has outstanding debt, it should be repayed.
#[test]
fn test_close_cdp_outstanding_debt() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(10);
        let debt = balance!(1);
        let more_than_debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        assert_balance(&alice_account_id(), &XOR, balance!(0));
        add_balance(alice_account_id(), more_than_debt, KUSD);

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: debt,
            }
            .into(),
        );
        System::assert_has_event(
            Event::CDPClosed {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                collateral_amount: collateral,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR, balance!(10));
        assert_balance(&alice_account_id(), &KUSD, more_than_debt);
        assert_eq!(KensetsuPallet::cdp(cdp_id), None);
        assert_eq!(KensetsuPallet::cdp_owner_index(alice_account_id()), None);
    });
}

/// Closes CDP and returns collateral to the owner when debt == 0
#[test]
fn test_close_cdp_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, balance!(0));
        assert_balance(&alice_account_id(), &XOR, balance!(0));

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id));

        System::assert_last_event(
            Event::CDPClosed {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                collateral_amount: collateral,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR, balance!(10));
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(0));
        assert_eq!(collateral_info.total_collateral, balance!(0));
        assert_eq!(KensetsuPallet::cdp(cdp_id), None);
        assert_eq!(KensetsuPallet::cdp_owner_index(alice_account_id()), None);
    });
}

/// Multiple CDPs created by single user,then deleted
/// CDP index should return correct cdp ids by the user
#[test]
fn test_multiple_cdp_close() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let cdp_id_1 = create_cdp_for_xor(alice(), balance!(10), balance!(0));
        let cdp_id_2 = create_cdp_for_xor(alice(), balance!(10), balance!(0));

        // 2 CDPs by user Alice
        assert_eq!(
            KensetsuPallet::cdp_owner_index(alice_account_id()),
            Some(BoundedVec::try_from(vec![cdp_id_1, cdp_id_2]).unwrap())
        );

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id_1));
        assert_eq!(
            KensetsuPallet::cdp_owner_index(alice_account_id()),
            Some(BoundedVec::try_from(vec![cdp_id_2]).unwrap())
        );

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id_2));
        assert_eq!(KensetsuPallet::cdp_owner_index(alice_account_id()), None);
    });
}

/// only by Signed Origin account can deposit collateral
#[test]
fn test_deposit_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::deposit_collateral(RuntimeOrigin::none(), cdp_id, balance!(0)),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::deposit_collateral(RuntimeOrigin::root(), cdp_id, balance!(0)),
            BadOrigin
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_deposit_collateral_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
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
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_noop!(
            KensetsuPallet::deposit_collateral(alice(), cdp_id, balance!(1)),
            pallet_balances::Error::<TestRuntime>::InsufficientBalance
        );
    });
}

/// Balance::MAX deposited, increase collateral results in ArithmeticError
#[test]
fn test_deposit_collateral_overflow() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        // due to cast to i128 in update_balance() u128::MAX is done with 2 x i128::MAX
        let max_i128_amount = Balance::MAX / 2;
        let cdp_id = create_cdp_for_xor(alice(), max_i128_amount, balance!(0));
        deposit_xor_to_cdp(alice(), cdp_id, max_i128_amount);
        add_balance(alice_account_id(), max_i128_amount, XOR);

        // ArithmeticError::Overflow from pallet_balances
        assert_noop!(
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
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));
        let amount = balance!(0);

        assert_ok!(KensetsuPallet::deposit_collateral(alice(), cdp_id, amount));

        System::assert_last_event(
            Event::CollateralDeposit {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                amount,
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.total_collateral, amount);
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
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));
        let amount = balance!(10);
        add_balance(alice_account_id(), amount, XOR);

        assert_ok!(KensetsuPallet::deposit_collateral(alice(), cdp_id, amount));

        System::assert_last_event(
            Event::CollateralDeposit {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                amount,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &XOR, balance!(0));
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("Must exists");
        assert_eq!(collateral_info.total_collateral, amount);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.collateral_amount, amount);
    });
}

/// only by Signed Origin account can borrow
#[test]
fn test_borrow_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::borrow(RuntimeOrigin::none(), cdp_id, balance!(0), balance!(0)),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::borrow(RuntimeOrigin::root(), cdp_id, balance!(0), balance!(0)),
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
            balance!(0),
        );
        // Alice is CDP owner
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_noop!(
            KensetsuPallet::borrow(bob(), cdp_id, balance!(0), balance!(0)),
            KensetsuError::OperationNotPermitted
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_borrow_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::borrow(alice(), cdp_id, balance!(0), balance!(0)),
            KensetsuError::CDPNotFound
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
            balance!(0),
        );
        let amount = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), amount, balance!(0));

        assert_noop!(
            KensetsuPallet::borrow(alice(), cdp_id, amount, amount),
            KensetsuError::CDPUnsafe
        );
    });
}

/// Test borrow call with wrong parameters: min_borrow_amount > max_borrow_amount
#[test]
fn test_borrow_wrong_parameters() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );

        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(0));
        assert_noop!(
            KensetsuPallet::borrow(alice(), cdp_id, balance!(100), balance!(20)),
            KensetsuError::WrongBorrowAmounts
        );
    });
}

/// CDP with collateral exists, call borrow with 0 KUSD amount.
/// Tx must succeed, but state is unchanged.
#[test]
fn test_borrow_zero_amount() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);

        assert_ok!(KensetsuPallet::borrow(
            alice(),
            cdp_id,
            balance!(0),
            balance!(0)
        ));

        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: debt,
            }
            .into(),
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt);
        assert_balance(&alice_account_id(), &KUSD, debt);
    });
}

/// CDP with collateral exists, call borrow with some KUSD amount.
/// Tx must succeed, debt to CDP added, KUSD minted to the caller.
#[test]
fn test_borrow_sunny_day() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, balance!(0));
        let to_borrow = balance!(10);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, balance!(0));

        assert_ok!(KensetsuPallet::borrow(
            alice(),
            cdp_id,
            to_borrow,
            to_borrow
        ));

        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: to_borrow,
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("Must exists");
        assert_eq!(collateral_info.kusd_supply, to_borrow);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, to_borrow);
        assert_balance(&alice_account_id(), &KUSD, to_borrow);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + to_borrow);
    });
}

/// CDP with collateral exists, call borrow with borrow_amount_max as u128::MAX.
/// Tx must succeed, max safe debt to CDP added, KUSD minted to the caller.
#[test]
fn test_borrow_max_amount() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, balance!(0));
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, balance!(0));

        assert_ok!(KensetsuPallet::borrow(alice(), cdp_id, 0, Balance::MAX,));

        // expected debt is collateral * liquidation ratio = 100 * 50% = 50
        let expected_debt = balance!(50);
        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: expected_debt,
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("Must exists");
        assert_eq!(collateral_info.kusd_supply, expected_debt);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, expected_debt);
        assert_balance(&alice_account_id(), &KUSD, expected_debt);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + expected_debt);
    });
}

/// @given: XOR is set as collateral and borrow tax is 1%
/// @when: user borrows KUSD against XOR
/// @then: debt is increased additionally by 1% of borrow tax, this amount is used to buy back KEN
#[test]
fn borrow_with_ken_incentivization() {
    new_test_ext().execute_with(|| {
        set_borrow_tax(Percent::from_percent(1));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(1000);
        let cdp_id = create_cdp_for_xor(alice(), collateral, balance!(0));
        let to_borrow = balance!(100);
        let borrow_tax = balance!(1);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, balance!(0));
        let ken_buyback_amount = balance!(1);
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KEN, ken_buyback_amount);

        assert_ok!(KensetsuPallet::borrow(
            alice(),
            cdp_id,
            to_borrow,
            to_borrow
        ));

        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: to_borrow + borrow_tax,
            }
            .into(),
        );

        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("Must exists");
        assert_eq!(collateral_info.kusd_supply, to_borrow + borrow_tax);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, to_borrow + borrow_tax);
        assert_balance(&alice_account_id(), &KUSD, to_borrow);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(
            total_kusd_supply,
            initial_total_kusd_supply + to_borrow + borrow_tax
        );
        let remint_percent = <TestRuntime as Config>::KenIncentiveRemintPercent::get();
        let demeter_farming_amount = remint_percent * ken_buyback_amount;
        assert_balance(&tech_account_id(), &KEN, demeter_farming_amount);
    });
}

/// @given: XOR is set as collateral and collateral amount is 100 XOR and borrow tax is 1%.
/// @when: user borrows as max KUSD against XOR as possible.
/// @then: debt is 100 KUSD.
#[test]
fn borrow_max_with_ken_incentivization() {
    new_test_ext().execute_with(|| {
        set_borrow_tax(Percent::from_percent(1));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(100),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, balance!(0));
        let to_borrow_min = balance!(99);
        let to_borrow_max = balance!(100);
        // user receives
        let actual_loan = 99009900990099009900;
        let borrow_tax = 990099009900990100;
        // user debt + tax equals the value of collateral
        assert_eq!(actual_loan + borrow_tax, balance!(100));
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, balance!(0));
        let ken_buyback_amount = borrow_tax;
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KEN, ken_buyback_amount);

        assert_ok!(KensetsuPallet::borrow(
            alice(),
            cdp_id,
            to_borrow_min,
            to_borrow_max
        ));

        System::assert_has_event(
            Event::DebtIncreased {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: actual_loan + borrow_tax,
            }
            .into(),
        );

        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, actual_loan + borrow_tax);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, actual_loan + borrow_tax);
        assert_balance(&alice_account_id(), &KUSD, actual_loan);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(
            total_kusd_supply,
            initial_total_kusd_supply + actual_loan + borrow_tax
        );
        let remint_percent = <TestRuntime as Config>::KenIncentiveRemintPercent::get();
        let demeter_farming_amount = remint_percent * ken_buyback_amount;
        assert_balance(&tech_account_id(), &KEN, demeter_farming_amount);
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
            balance!(0),
        );
        let debt = balance!(10);
        let collateral = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, balance!(10));

        assert_ok!(KensetsuPallet::borrow(
            alice(),
            cdp_id,
            balance!(0),
            balance!(0)
        ));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - per millisecond rate, 1 - millisecond passed
        let interest = balance!(1);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_balance(&alice_account_id(), &KUSD, balance!(10));
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD, balance!(1));
    });
}

/// only by Signed Origin account can repay_debt
#[test]
fn test_repay_debt_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::repay_debt(RuntimeOrigin::none(), cdp_id, balance!(0)),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::repay_debt(RuntimeOrigin::root(), cdp_id, balance!(0)),
            BadOrigin
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_repay_debt_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
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
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        let to_repay = balance!(1);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, to_repay));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: to_repay,
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt - to_repay);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt - to_repay);
        let total_kusd_supply = get_total_supply(&KUSD);
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
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, debt));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: debt,
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(0));
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, balance!(0));
        let total_kusd_supply = get_total_supply(&KUSD);
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
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        // create 2nd CDP and borrow for KUSD surplus on Alice account
        let kusd_surplus = balance!(5);
        create_cdp_for_xor(alice(), collateral, kusd_surplus);
        let total_kusd_balance = debt + kusd_surplus;
        let initial_total_kusd_supply = get_total_supply(&KUSD);
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
                debt_asset_id: KUSD,
                amount: debt,
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, kusd_surplus);
        assert_eq!(collateral_info.total_collateral, 2 * collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, balance!(0));
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, kusd_surplus);
        assert_balance(&alice_account_id(), &KUSD, kusd_surplus);
    });
}

/// Repay with zero amount.
/// Success, but state is not changed.
#[test]
fn test_repay_debt_zero_amount() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, balance!(0)));

        System::assert_has_event(
            Event::DebtPayment {
                cdp_id,
                owner: alice_account_id(),
                debt_asset_id: KUSD,
                amount: balance!(0),
            }
            .into(),
        );
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_total_kusd_supply);
    });
}

/// Repay with zero amount to trigger accrue.
/// Success, debt increased and KUSD is minted to tech treasury account.
#[test]
fn test_repay_debt_accrue() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.1),
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, debt);
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_ok!(KensetsuPallet::repay_debt(alice(), cdp_id, balance!(0)));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - per millisecond rate, 1 - millisecond passed
        let interest = balance!(1);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_balance(&alice_account_id(), &KUSD, balance!(10));
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD, balance!(1));
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_liquidate_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
            KensetsuPallet::liquidate(RuntimeOrigin::none(), cdp_id),
            KensetsuError::CDPNotFound
        );
    });
}

/// If cdp safe, return error
#[test]
fn test_liquidate_cdp_safe() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(10));

        assert_noop!(
            KensetsuPallet::liquidate(RuntimeOrigin::none(), cdp_id),
            KensetsuError::CDPSafe
        );
    });
}

/// Given: CDP with collateral 10000 XOR and it is unsafe.
/// @When: Liquidation triggered that doesn't change debt.
/// Success, debt increased and KUSD is minted to tech treasury account.
#[test]
fn test_liquidate_accrue() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(10),
            FixedU128::from_float(0.1),
            balance!(0),
        );
        // the CDP will be unsafe in the next millisecond
        let collateral = balance!(10000);
        let debt = balance!(1000);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KUSD, balance!(0));
        let initial_total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(initial_total_kusd_supply, debt);

        assert_ok!(KensetsuPallet::liquidate(alice(), cdp_id));

        // interest is 1000*10%*1 = 100,
        // where 1000 - initial balance, 10% - per millisecond rate, 1 - millisecond passed
        let interest = balance!(100);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        assert_balance(&alice_account_id(), &KUSD, debt);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_total_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD, interest);
    });
}

/// CDP has debt
/// Liquidation sells only part of collateral.
/// Liquidation results with output KUSD amount > cdp.debt + liquidation penalty
/// CDP debt is repaid, corresponding amount of collateral is sold
/// Liquidation penalty is a protocol profit
/// Leftover from liquidation goes to CDP owner
#[test]
fn test_liquidate_kusd_amount_covers_cdp_debt_and_penalty() {
    new_test_ext().execute_with(|| {
        KensetsuPallet::update_liquidation_penalty(
            RuntimeOrigin::root(),
            Percent::from_percent(10),
        )
        .expect("Must set liquidation penalty");
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::zero(),
            balance!(0),
        );
        let collateral = balance!(2000);
        let debt = balance!(100);
        let collateral_liquidated = balance!(200);
        let liquidation_income = balance!(200);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        assert_balance(&alice_account_id(), &KUSD, debt);
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KUSD, collateral_liquidated);
        make_cdps_unsafe();
        // 100 KUSD debt + 200 KUSD liquidity provider
        let initial_kusd_supply = get_total_supply(&KUSD);

        // 200 XOR sold for 200 KUSD
        assert_ok!(KensetsuPallet::liquidate(alice(), cdp_id));

        let penalty = balance!(10); // debt * liquidation penalty
        System::assert_has_event(
            Event::Liquidated {
                cdp_id,
                collateral_asset_id: XOR,
                collateral_amount: collateral_liquidated,
                debt_asset_id: KUSD,
                proceeds: liquidation_income - penalty,
                penalty,
            }
            .into(),
        );
        assert_balance(&tech_account_id(), &KUSD, penalty);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(0));
        assert_eq!(
            collateral_info.total_collateral,
            collateral - collateral_liquidated
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        // initial collateral 2000 XOR, 200 XOR sold during liquidation
        assert_eq!(cdp.collateral_amount, balance!(1800));
        assert_eq!(cdp.debt, balance!(0));
        // alice balance is:
        // debt (from borrow) + liquidation leftover
        // where liquidation leftover is (liquidation_income - debt - penalty)
        assert_balance(&alice_account_id(), &KUSD, liquidation_income - penalty);
        let kusd_supply = get_total_supply(&KUSD);
        // 100 KUSD which is debt amount is burned
        assert_eq!(initial_kusd_supply - debt, kusd_supply);
        // liquidation flag was set
        assert!(LiquidatedThisBlock::<TestRuntime>::get());
    });
}

/// CDP has debt
/// Liquidation results with output KUSD amount = cdp.debt + liquidation penalty
/// CDP debt is repaid, corresponding amount of collateral is sold
/// Liquidation penalty is a protocol profit
#[test]
fn test_liquidate_kusd_amount_eq_cdp_debt_and_penalty() {
    new_test_ext().execute_with(|| {
        KensetsuPallet::update_liquidation_penalty(
            RuntimeOrigin::root(),
            Percent::from_percent(10),
        )
        .expect("Must set liquidation penalty");
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::zero(),
            balance!(0),
        );
        let collateral = balance!(2000);
        let debt = balance!(100);
        // debt + penalty = 100 + 10
        let liquidation_income = balance!(110);
        let collateral_liquidated = balance!(110);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        assert_balance(&alice_account_id(), &KUSD, debt);
        make_cdps_unsafe();
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KUSD, collateral_liquidated);
        // 100 KUSD debt + 110 KUSD liquidity provider
        let initial_kusd_supply = get_total_supply(&KUSD);

        // 110 XOR sold for 110 KUSD
        assert_ok!(KensetsuPallet::liquidate(alice(), cdp_id));

        // debt * liquidation penalty = 10 KUSD
        let penalty = balance!(10);
        System::assert_has_event(
            Event::Liquidated {
                cdp_id,
                collateral_asset_id: XOR,
                collateral_amount: collateral_liquidated,
                debt_asset_id: KUSD,
                proceeds: liquidation_income - penalty,
                penalty,
            }
            .into(),
        );
        assert_balance(&tech_account_id(), &KUSD, penalty);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(0));
        assert_eq!(
            collateral_info.total_collateral,
            collateral - collateral_liquidated
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        // initial collateral 2000 XOR, 110 XOR sold during liquidation
        assert_eq!(cdp.collateral_amount, collateral - collateral_liquidated);
        assert_eq!(cdp.debt, balance!(0));
        assert_balance(&alice_account_id(), &KUSD, debt);
        let kusd_supply = get_total_supply(&KUSD);
        // 100 KUSD which is debt amount is burned
        assert_eq!(initial_kusd_supply - debt, kusd_supply);
        // liquidation flag was set
        assert!(LiquidatedThisBlock::<TestRuntime>::get());
    });
}

/// CDP has debt and unsafe
/// Liquidation results with revenue KUSD amount where
///  - revenue KUSD amount > cdp.debt
///  - revenue KUSD amount < cdp.debt + liquidation penalty
/// CDP debt is repaid, corresponding amount of collateral is sold
/// Liquidation penalty is a protocol profit
/// CDP has outstanding debt
#[test]
fn test_liquidate_kusd_amount_covers_cdp_debt_and_partly_penalty() {
    new_test_ext().execute_with(|| {
        KensetsuPallet::update_liquidation_penalty(
            RuntimeOrigin::root(),
            Percent::from_percent(10),
        )
        .expect("Must set liquidation penalty");
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::zero(),
            balance!(0),
        );
        let collateral = balance!(2000);
        let debt = balance!(1000);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        assert_balance(&alice_account_id(), &KUSD, debt);
        make_cdps_unsafe();
        let collateral_liquidated = balance!(1050);
        let liquidation_income = balance!(1050);
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KUSD, collateral_liquidated);
        // 1000 KUSD debt + 1050 KUSD minted for liquidity provider
        let initial_kusd_supply = get_total_supply(&KUSD);

        // 1000 XOR sold for 1050 KUSD
        assert_ok!(KensetsuPallet::liquidate(alice(), cdp_id));

        // min(liquidation_income, cdp.debt) * liquidation penalty = 100 KUSD
        let penalty = balance!(100);
        System::assert_has_event(
            Event::Liquidated {
                cdp_id,
                collateral_asset_id: XOR,
                collateral_amount: collateral_liquidated,
                debt_asset_id: KUSD,
                proceeds: liquidation_income - penalty,
                penalty,
            }
            .into(),
        );
        assert_balance(&tech_account_id(), &KUSD, penalty);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(50));
        assert_eq!(
            collateral_info.total_collateral,
            collateral - collateral_liquidated
        );
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        // initial collateral 2000 XOR, 1050 XOR sold during liquidation
        assert_eq!(cdp.collateral_amount, collateral - collateral_liquidated);
        // debt = 1000 KUSD + (penalty) 100 KUSD - (liquidation_income) 1050 KUSD
        // = 50 KUSD
        assert_eq!(cdp.debt, balance!(50));
        assert_balance(&alice_account_id(), &KUSD, debt);
        let kusd_supply = get_total_supply(&KUSD);
        // were burned in liquidation (debt) - cdp.debt = 108 KUSD
        assert_eq!(initial_kusd_supply - debt + cdp.debt, kusd_supply);
        // liquidation flag was set
        assert!(LiquidatedThisBlock::<TestRuntime>::get());
    });
}

// Given: Unsafe CDP
// Liquidation of all the collateral, debt is covered.
// CDP is closed, no bad debt, liquidation penalty is a profit.
#[test]
fn test_liquidate_kusd_amount_does_not_cover_cdp_debt() {
    new_test_ext().execute_with(|| {
        KensetsuPallet::update_liquidation_penalty(
            RuntimeOrigin::root(),
            Percent::from_percent(10),
        )
        .expect("Must set liquidation penalty");
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(100),
            FixedU128::from_float(0.1),
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        assert_balance(&alice_account_id(), &KUSD, debt);
        // liquidation amount is the same, 100 XOR
        let liquidation_income = balance!(100);
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KUSD, collateral);
        // CDP debt now is 110 KUSD, it is unsafe
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        // 100 KUSD debt + 100 KUSD liquidity provider
        let initial_kusd_supply = get_total_supply(&KUSD);

        // 100 XOR sold for 100 KUSD
        assert_ok!(KensetsuPallet::liquidate(alice(), cdp_id));

        // debt * liquidation penalty = 10 KUSD
        let penalty = balance!(10);
        System::assert_has_event(
            Event::Liquidated {
                cdp_id,
                collateral_asset_id: XOR,
                collateral_amount: collateral,
                debt_asset_id: KUSD,
                proceeds: liquidation_income - penalty,
                penalty,
            }
            .into(),
        );
        System::assert_has_event(
            Event::CDPClosed {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                collateral_amount: balance!(0),
            }
            .into(),
        );
        // tech account was 10 interest + 10 penalty = 20
        // debt is 100 + 10 interest
        // liquidation revenue is 100 - 10 penalty = 90
        // bad debt = debt - liquidation = 110 - 90 = 20 - covered with protocol profit
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(0));
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(0));
        assert_eq!(collateral_info.total_collateral, balance!(0));
        assert_eq!(KensetsuPallet::cdp(cdp_id), None);
        assert_eq!(KensetsuPallet::cdp_owner_index(alice_account_id()), None);
        assert_balance(&alice_account_id(), &KUSD, debt);
        let kusd_supply = get_total_supply(&KUSD);
        // 100 KUSD which is debt amount is burned
        assert_eq!(initial_kusd_supply - debt, kusd_supply);
        // liquidation flag was set
        assert!(LiquidatedThisBlock::<TestRuntime>::get());
    });
}

/// CDP is unsafe
/// Liquidation results with revenue < debt
/// Protocol bad debt increased
/// CDP closed
#[test]
fn test_liquidate_kusd_bad_debt() {
    new_test_ext().execute_with(|| {
        KensetsuPallet::update_liquidation_penalty(
            RuntimeOrigin::root(),
            Percent::from_percent(10),
        )
        .expect("Must set liquidation penalty");
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(100),
            FixedU128::from_float(0.1),
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        assert_balance(&alice_account_id(), &KUSD, debt);
        // liquidation amount < debt
        let liquidation_income = balance!(100);
        MockLiquidityProxy::set_amounts_for_the_next_exchange(KUSD, collateral);
        // CDP debt now is 110 KUSD, it is unsafe
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));
        // withdraw 10 KUSD from interest, so the protocol can not cover bad debt
        let interest = balance!(10);
        assert_ok!(KensetsuPallet::withdraw_profit(
            RuntimeOrigin::root(),
            bob_account_id(),
            interest
        ));
        // 110 KUSD debt + 100 KUSD liquidity provider
        let initial_kusd_supply = get_total_supply(&KUSD);
        // 110 KUSD minted by the protocol
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, balance!(110));

        // 100 XOR sold for 100 KUSD
        assert_ok!(KensetsuPallet::liquidate(alice(), cdp_id));

        // liquidation_income * liquidation penalty = 10 KUSD
        let penalty = balance!(10);
        System::assert_has_event(
            Event::Liquidated {
                cdp_id,
                collateral_asset_id: XOR,
                collateral_amount: collateral,
                debt_asset_id: KUSD,
                proceeds: liquidation_income - penalty,
                penalty,
            }
            .into(),
        );
        System::assert_has_event(
            Event::CDPClosed {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR,
                collateral_amount: balance!(0),
            }
            .into(),
        );
        // tech account balance: 10 fee + 10 penalty - 10 withdrawn = 10 KUSD
        // CDP debt is 100 + 10 interest = 110 KUSD
        // liquidation sold for 100 where proceeds is 90 and 10 penalty
        // CDP bad debt = CDP debt - proceeds = 110 - 90 = 20 KUSD
        // protocol bad debt = CDP bad debt - tech account balance = 20 - 10 = 10 KUSD
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(10));
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        // 10 KUSD minted by the protocol (accounted in bad debt)
        assert_eq!(collateral_info.kusd_supply, balance!(0));
        assert_eq!(collateral_info.total_collateral, balance!(0));
        assert_eq!(KensetsuPallet::cdp(cdp_id), None);
        assert_eq!(KensetsuPallet::cdp_owner_index(alice_account_id()), None);
        assert_balance(&bob_account_id(), &KUSD, interest);
        // 10 fee on owner + 100 debt alice = 110 KUSD
        let kusd_supply = get_total_supply(&KUSD);
        // 100 KUSD which is debt amount is burned
        assert_eq!(initial_kusd_supply - debt, kusd_supply);
        // liquidation flag was set
        assert!(LiquidatedThisBlock::<TestRuntime>::get());
    });
}

/// Given: CDP is unsafe and risk parameters liquidation lot is 0
/// @When: Liquidation triggered
/// @Then: Error ZeroLiquidationLot returned
#[test]
fn test_liquidate_zero_lot() {
    new_test_ext().execute_with(|| {
        let new_parameters = CollateralRiskParameters {
            hard_cap: Balance::MAX,
            liquidation_ratio: Perbill::from_percent(100),
            max_liquidation_lot: balance!(0),
            stability_fee_rate: FixedU128::from_float(0.1),
            minimal_collateral_deposit: balance!(0),
        };
        assert_ok!(KensetsuPallet::update_collateral_risk_parameters(
            RuntimeOrigin::root(),
            XOR,
            new_parameters
        ));

        let collateral = balance!(100);
        let debt = balance!(100);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        // Make CDP unsafe in the next call
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_noop!(
            KensetsuPallet::liquidate(alice(), cdp_id),
            KensetsuError::ZeroLiquidationLot
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_accrue_cdp_does_not_exist() {
    new_test_ext().execute_with(|| {
        let cdp_id = 1;

        assert_noop!(
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
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(0));

        assert_noop!(
            KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id),
            KensetsuError::UncollectedStabilityFeeTooSmall
        );
    });
}

/// If cdp was updated, and then called with wrong time, return AccrueWrongTime
#[test]
fn test_accrue_wrong_time() {
    new_test_ext().execute_with(|| {
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(10);
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(10));
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_noop!(
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
            balance!(0),
        );
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), balance!(50));
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(9999);

        assert_noop!(
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
            // 10% per millisecond
            FixedU128::from_float(0.1),
            balance!(0),
        );
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_kusd_supply = get_total_supply(&KUSD);

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*10%*1 = 1,
        // where 10 - initial balance, 10% - per millisecond rate, 1 - millisecond passed
        let interest = balance!(1);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_kusd_supply + interest);
        assert_balance(&tech_account_id(), &KUSD, interest);
    });
}

/// Given: CDP with debt, was updated this time, protocol has no bad debt
/// When: accrue is called again with the same time
/// Then: failed, minimal threshold is not satisfied.
#[test]
fn test_accrue_profit_same_time() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 10% per millisecond
            FixedU128::from_float(0.1),
            balance!(0),
        );
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // double call should fail
        assert_noop!(
            KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id),
            KensetsuError::UncollectedStabilityFeeTooSmall
        );
    });
}

/// Given: CDP with debt, protocol has bad debt and interest accrued < bad debt.
/// When: accrue is called.
/// Then: interest covers the part of bad debt.
#[test]
fn test_accrue_interest_less_bad_debt() {
    new_test_ext().execute_with(|| {
        set_bad_debt(balance!(2));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 10% per millisecond
            FixedU128::from_float(0.1),
            balance!(0),
        );
        set_bad_debt(balance!(2));
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_kusd_supply = get_total_supply(&KUSD);

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*20%*1 = 1 KUSD,
        // where 10 - initial balance, 10% - per millisecond rate, 1 - millisecond passed
        // and 1 KUSD covers the part of bad debt
        let interest = balance!(1);
        let new_bad_debt = balance!(1);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        // fee is burned as bad debt, no KUSD minted
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_kusd_supply);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(new_bad_debt);
    });
}

/// Given: CDP with debt, protocol has bad debt and interest accrued == bad debt.
/// When: accrue is called.
/// Then: interest covers the part of bad debt.
#[test]
fn test_accrue_interest_eq_bad_debt() {
    new_test_ext().execute_with(|| {
        set_bad_debt(balance!(1));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 10% per millisecond
            FixedU128::from_float(0.1),
            balance!(0),
        );
        set_bad_debt(balance!(1));
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), collateral, debt);
        let initial_kusd_supply = get_total_supply(&KUSD);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*10%*1 = 1 KUSD,
        // where 10 - initial balance, 10% - per millisecond rate, 1 - millisecond passed
        // and 1 KUSD covers bad debt
        let interest = balance!(1);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        // supply doesn't change, fee is burned as bad debt
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_kusd_supply);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(0));
    });
}

/// Given: CDP with debt, protocol has bad debt and interest accrued > bad debt.
/// When: accrue is called.
/// Then: interest covers the bad debt and leftover goes to protocol profit.
#[test]
fn test_accrue_interest_gt_bad_debt() {
    new_test_ext().execute_with(|| {
        set_bad_debt(balance!(1));
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            // 20% per millisecond
            FixedU128::from_float(0.2),
            balance!(0),
        );
        set_bad_debt(balance!(1));
        let collateral = balance!(100);
        let debt = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), balance!(100), debt);
        // 1 sec passed
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        let initial_kusd_supply = get_total_supply(&KUSD);

        assert_ok!(KensetsuPallet::accrue(RuntimeOrigin::none(), cdp_id));

        // interest is 10*20%*1 = 2 KUSD,
        // where 10 - initial balance, 20% - per millisecond rate, 1 - millisecond passed
        // and 1 KUSD covers bad debt, 1 KUSD is a protocol profit
        let interest = balance!(2);
        let profit = balance!(1);
        let collateral_info = KensetsuPallet::collateral_infos(XOR).expect("must exists");
        // 1 KUSD goes to profit and 1 is burned as bad debt
        assert_eq!(collateral_info.kusd_supply, debt + interest);
        assert_eq!(collateral_info.total_collateral, collateral);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.debt, debt + interest);
        let total_kusd_supply = get_total_supply(&KUSD);
        assert_eq!(total_kusd_supply, initial_kusd_supply + profit);
        assert_balance(&tech_account_id(), &KUSD, profit);
        assert_bad_debt(balance!(0));
    });
}

/// Only root can update risk parameters
#[test]
fn test_update_collateral_risk_parameters_only_root() {
    new_test_ext().execute_with(|| {
        let parameters = CollateralRiskParameters {
            hard_cap: balance!(100),
            liquidation_ratio: Perbill::from_percent(50),
            max_liquidation_lot: balance!(100),
            stability_fee_rate: FixedU128::from_float(0.1),
            minimal_collateral_deposit: balance!(0),
        };

        assert_noop!(
            KensetsuPallet::update_collateral_risk_parameters(
                RuntimeOrigin::none(),
                XOR,
                parameters
            ),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::update_collateral_risk_parameters(alice(), XOR, parameters),
            BadOrigin
        );
    });
}

/// Only existing assets ids are allowed
#[test]
fn test_update_collateral_risk_parameters_wrong_asset_id() {
    new_test_ext().execute_with(|| {
        let parameters = CollateralRiskParameters {
            hard_cap: balance!(100),
            liquidation_ratio: Perbill::from_percent(50),
            max_liquidation_lot: balance!(100),
            stability_fee_rate: FixedU128::from_float(0.1),
            minimal_collateral_deposit: balance!(0),
        };
        let wrong_asset_id = AssetId32::from_bytes(hex!(
            "0000000000000000000000000000000000000000000000000000000007654321"
        ));

        assert_noop!(
            KensetsuPallet::update_collateral_risk_parameters(
                RuntimeOrigin::root(),
                wrong_asset_id,
                parameters
            ),
            KensetsuError::WrongAssetId
        );
    });
}

/// Given: risk parameters were set
/// When: update risk parameters
/// Then: risk parameters are changed, event is emitted, interest coefficient is changed
#[test]
fn test_update_collateral_risk_parameters_no_rate_change() {
    new_test_ext().execute_with(|| {
        let asset_id = XOR;
        // stability fee is 10%
        let stability_fee_rate = FixedU128::from_float(0.1);

        // parameters with stability fee 10%
        let old_parameters = CollateralRiskParameters {
            hard_cap: balance!(100),
            liquidation_ratio: Perbill::from_percent(10),
            max_liquidation_lot: balance!(100),
            stability_fee_rate,
            minimal_collateral_deposit: balance!(0),
        };
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(1);
        assert_ok!(KensetsuPallet::update_collateral_risk_parameters(
            RuntimeOrigin::root(),
            asset_id,
            old_parameters
        ));
        let old_info = CollateralInfos::<TestRuntime>::get(asset_id).expect("Must succeed");
        assert_eq!(old_info.risk_parameters, old_parameters);
        assert_eq!(old_info.last_fee_update_time, 1);
        assert_eq!(old_info.interest_coefficient, FixedU128::one());

        let new_parameters = CollateralRiskParameters {
            hard_cap: balance!(200),
            liquidation_ratio: Perbill::from_percent(10),
            max_liquidation_lot: balance!(200),
            stability_fee_rate: FixedU128::from_float(0.2),
            minimal_collateral_deposit: balance!(0),
        };
        pallet_timestamp::Pallet::<TestRuntime>::set_timestamp(2);
        assert_ok!(KensetsuPallet::update_collateral_risk_parameters(
            RuntimeOrigin::root(),
            asset_id,
            new_parameters
        ));

        System::assert_has_event(
            Event::CollateralRiskParametersUpdated {
                collateral_asset_id: XOR,
                risk_parameters: new_parameters,
            }
            .into(),
        );
        let new_info = CollateralInfos::<TestRuntime>::get(asset_id).expect("Must succeed");
        assert_eq!(new_info.risk_parameters, new_parameters);
        // interest coefficient is not changed
        assert_eq!(new_info.last_fee_update_time, 2);
        assert_eq!(
            new_info.interest_coefficient,
            FixedU128::one() * (FixedU128::one() + stability_fee_rate)
        );
    });
}

/// Only root can update borrow tax
#[test]
fn test_update_borrow_tax_only_root() {
    new_test_ext().execute_with(|| {
        let new_borrow_tax = Percent::from_percent(10);

        assert_noop!(
            KensetsuPallet::update_borrow_tax(RuntimeOrigin::none(), new_borrow_tax),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::update_borrow_tax(alice(), Percent::from_percent(10)),
            BadOrigin
        );
    });
}

/// Root can update borrow tax
#[test]
fn test_update_borrow_tax_sunny_day() {
    new_test_ext().execute_with(|| {
        let new_borrow_tax = Percent::from_percent(10);

        assert_ok!(KensetsuPallet::update_borrow_tax(
            RuntimeOrigin::root(),
            new_borrow_tax
        ));

        let old_borrow_tax = Percent::default();
        System::assert_has_event(
            Event::BorrowTaxUpdated {
                new_borrow_tax,
                old_borrow_tax,
            }
            .into(),
        );
        assert_eq!(new_borrow_tax, BorrowTax::<TestRuntime>::get());
    });
}

/// Only root can update penalty
#[test]
fn test_update_liquidation_penalty_only_root() {
    new_test_ext().execute_with(|| {
        let liquidation_penalty = Percent::from_percent(10);

        assert_noop!(
            KensetsuPallet::update_liquidation_penalty(RuntimeOrigin::none(), liquidation_penalty),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::update_liquidation_penalty(alice(), liquidation_penalty),
            BadOrigin
        );
    });
}

/// Root can update hard cap
#[test]
fn test_update_liquidation_penalty_sunny_day() {
    new_test_ext().execute_with(|| {
        let new_liquidation_penalty = Percent::from_percent(10);

        assert_ok!(KensetsuPallet::update_liquidation_penalty(
            RuntimeOrigin::root(),
            new_liquidation_penalty
        ));

        let old_liquidation_penalty = Percent::default();
        System::assert_has_event(
            Event::LiquidationPenaltyUpdated {
                new_liquidation_penalty,
                old_liquidation_penalty,
            }
            .into(),
        );
        assert_eq!(
            new_liquidation_penalty,
            LiquidationPenalty::<TestRuntime>::get()
        );
    });
}

/// Only Signed Origin account can donate to protocol
#[test]
fn test_donate_only_signed_origin() {
    new_test_ext().execute_with(|| {
        let donation = balance!(10);

        assert_noop!(
            KensetsuPallet::donate(RuntimeOrigin::none(), donation),
            BadOrigin
        );
        assert_noop!(
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
            balance!(0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));
        assert_bad_debt(balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(
            Event::Donation {
                debt_asset_id: KUSD,
                amount: donation,
            }
            .into(),
        );
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
            balance!(0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(
            Event::Donation {
                debt_asset_id: KUSD,
                amount: donation,
            }
            .into(),
        );
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
            balance!(0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(
            Event::Donation {
                debt_asset_id: KUSD,
                amount: donation,
            }
            .into(),
        );
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
            balance!(0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);
        assert_balance(&alice_account_id(), &KUSD, donation);
        assert_balance(&tech_account_id(), &KUSD, balance!(0));

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(
            Event::Donation {
                debt_asset_id: KUSD,
                amount: donation,
            }
            .into(),
        );
        assert_balance(&alice_account_id(), &KUSD, balance!(0));
        assert_balance(&tech_account_id(), &KUSD, donation - initial_bad_debt);
        assert_bad_debt(balance!(0));
    });
}

/// Donation of zero amount must succeed
#[test]
fn test_donate_zero_amount() {
    new_test_ext().execute_with(|| {
        let donation = balance!(0);
        // Alice has 10 KUSD
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        create_cdp_for_xor(alice(), balance!(100), donation);

        assert_ok!(KensetsuPallet::donate(alice(), donation));

        System::assert_has_event(
            Event::Donation {
                debt_asset_id: KUSD,
                amount: donation,
            }
            .into(),
        );
    });
}

/// Only root can withdraw profit
#[test]
fn test_withdraw_profit_only_root() {
    new_test_ext().execute_with(|| {
        let profit = balance!(10);

        assert_noop!(
            KensetsuPallet::withdraw_profit(RuntimeOrigin::none(), alice_account_id(), profit),
            BadOrigin
        );
        assert_noop!(
            KensetsuPallet::withdraw_profit(alice(), alice_account_id(), profit),
            BadOrigin
        );
    });
}

/// Error must be returned when balance too low to withdraw.
#[test]
fn test_withdraw_profit_not_enough() {
    new_test_ext().execute_with(|| {
        let profit = balance!(10);

        assert_noop!(
            KensetsuPallet::withdraw_profit(RuntimeOrigin::root(), alice_account_id(), profit),
            tokens::Error::<TestRuntime>::BalanceTooLow
        );
    });
}

/// Profit withdrawn, balances updated.
#[test]
fn test_withdraw_profit_sunny_day() {
    new_test_ext().execute_with(|| {
        let initial_protocol_profit = balance!(20);
        // Alice donates 20 KUSD to protocol, so it has profit.
        set_xor_as_collateral_type(
            Balance::MAX,
            Perbill::from_percent(50),
            FixedU128::from_float(0.0),
            balance!(0),
        );
        create_cdp_for_xor(alice(), balance!(100), initial_protocol_profit);
        assert_ok!(KensetsuPallet::donate(alice(), initial_protocol_profit));
        assert_balance(&tech_account_id(), &KUSD, initial_protocol_profit);
        assert_balance(&alice_account_id(), &KUSD, balance!(0));
        let to_withdraw = balance!(10);

        assert_ok!(KensetsuPallet::withdraw_profit(
            RuntimeOrigin::root(),
            alice_account_id(),
            to_withdraw
        ));

        System::assert_has_event(
            Event::ProfitWithdrawn {
                debt_asset_id: KUSD,
                amount: to_withdraw,
            }
            .into(),
        );
        assert_balance(
            &tech_account_id(),
            &KUSD,
            initial_protocol_profit - to_withdraw,
        );
        assert_balance(&alice_account_id(), &KUSD, to_withdraw);
    });
}

/// Withdraw 0 amount profit must succeed
#[test]
fn test_withdraw_profit_zero_amount() {
    new_test_ext().execute_with(|| {
        let to_withdraw = balance!(0);

        assert_ok!(KensetsuPallet::withdraw_profit(
            RuntimeOrigin::root(),
            alice_account_id(),
            to_withdraw
        ));

        System::assert_has_event(
            Event::ProfitWithdrawn {
                debt_asset_id: KUSD,
                amount: to_withdraw,
            }
            .into(),
        );
    });
}
