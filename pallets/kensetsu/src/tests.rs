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

use crate::mock::new_test_ext;
use common::PredefinedAssetId::XOR;
use common::{balance, AssetInfoProvider, Balance};
use frame_support::{assert_err, assert_ok};
use frame_system::pallet_prelude::OriginFor;
use framenode_runtime::kensetsu::CollateralRiskParameters;
use framenode_runtime::kensetsu::*;
use framenode_runtime::{Runtime, RuntimeOrigin};
use hex_literal::hex;
use sp_arithmetic::{ArithmeticError, Perbill};
use sp_core::U256;
use sp_runtime::AccountId32;
use sp_runtime::DispatchError::BadOrigin;

type AccountId = AccountId32;
type Event = framenode_runtime::kensetsu::Event<Runtime>;
type KensetsuError = framenode_runtime::kensetsu::Error<Runtime>;
type KensetsuPallet = framenode_runtime::kensetsu::Pallet<Runtime>;
type System = frame_system::Pallet<Runtime>;

/// Predefined AccountId `Alice`
pub fn alice_account_id() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}
/// Predefined AccountId `Bob`
pub fn bob_account_id() -> AccountId {
    AccountId32::from(hex!(
        "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
    ))
}

/// Returns Risk Manager account
fn risk_manager() -> OriginFor<Runtime> {
    RuntimeOrigin::signed(alice_account_id())
}

/// Regular client account Alice
fn alice() -> OriginFor<Runtime> {
    RuntimeOrigin::signed(alice_account_id())
}

/// Regular client account Alice
fn bob() -> OriginFor<Runtime> {
    RuntimeOrigin::signed(bob_account_id())
}

/// Sets XOR asset id as collateral with default parameters
/// As if Risk Manager called `update_collateral_risk_parameters(XOR, some_info)`
fn set_xor_as_collateral_type() {
    CollateralTypes::<Runtime>::set(
        <Runtime as assets::Config>::AssetId::from(XOR),
        Some(CollateralRiskParameters {
            max_supply: balance!(1000),
            liquidation_ratio: Perbill::from_float(0.5),
            stability_fee_rate: Default::default(),
        }),
    );
    KusdHardCap::<Runtime>::set(balance!(1000));
}

/// Creates CDP with XOR as collateral asset id
fn create_cdp_for_xor(owner: OriginFor<Runtime>, collateral: Balance, debt: Balance) -> U256 {
    assert_ok!(KensetsuPallet::create_cdp(owner.clone(), XOR.into()));
    let cdp_id = NextCDPId::<Runtime>::get();
    if collateral > 0 {
        deposit_xor_to_cdp(owner.clone(), cdp_id, collateral);
    }
    if debt > 0 {
        assert_ok!(KensetsuPallet::borrow(owner, cdp_id, debt));
    }
    cdp_id
}

/// Deposits to CDP
fn deposit_xor_to_cdp(owner: OriginFor<Runtime>, cdp_id: U256, collateral_amount: Balance) {
    set_balance(alice_account_id(), collateral_amount);
    assert_ok!(KensetsuPallet::deposit_collateral(
        owner,
        cdp_id,
        collateral_amount
    ));
}

/// Updates account balance
fn set_balance(account: AccountId, balance: Balance) {
    assert_ok!(assets::Pallet::<Runtime>::update_balance(
        RuntimeOrigin::root(),
        account,
        XOR.into(),
        balance.try_into().unwrap()
    ));
}

/// Asserts account balance is expected.
fn assert_balance(account: &AccountId, expected: Balance) {
    assert_eq!(
        assets::Pallet::<Runtime>::free_balance(&XOR.into(), account).unwrap(),
        expected
    );
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

/// If the number of cdp ids reached U256::MAX, next CDP will result in ArithmeticError.
#[test]
fn test_create_cdp_overflow_error() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type();
        NextCDPId::<Runtime>::set(U256::MAX);

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
        System::set_block_number(1);
        set_xor_as_collateral_type();

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
        set_xor_as_collateral_type();
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
        set_xor_as_collateral_type();
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
        set_xor_as_collateral_type();
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
        System::set_block_number(1);
        set_xor_as_collateral_type();
        let cdp_id = create_cdp_for_xor(alice(), balance!(10), balance!(0));
        assert_balance(&alice_account_id(), balance!(0));

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id));

        System::assert_last_event(
            Event::CDPClosed {
                cdp_id,
                owner: alice_account_id(),
                collateral_asset_id: XOR.into(),
            }
            .into(),
        );
        assert_balance(&alice_account_id(), balance!(10));
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
        set_xor_as_collateral_type();
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
        set_xor_as_collateral_type();
        let cdp_id = create_cdp_for_xor(alice(), balance!(0), balance!(0));

        assert_err!(
            KensetsuPallet::deposit_collateral(alice(), cdp_id, balance!(1)),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
    });
}

/// Balance::MAX deposited, increase collateral results in ArithmeticError
#[test]
fn test_deposit_collateral_overlow() {
    new_test_ext().execute_with(|| {
        set_xor_as_collateral_type();
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

/// Alice deposits `amount` collateral, balance changed, event is emitted
#[test]
fn test_deposit_collateral_sunny_day() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_xor_as_collateral_type();
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
        assert_balance(&alice_account_id(), balance!(0));
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
        set_xor_as_collateral_type();
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
        set_xor_as_collateral_type();
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
        set_xor_as_collateral_type();
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
        set_xor_as_collateral_type();
        let cdp_id = create_cdp_for_xor(alice(), balance!(10), balance!(5));

        assert_err!(
            KensetsuPallet::withdraw_collateral(alice(), cdp_id, balance!(1)),
            KensetsuError::CDPUnsafe
        );
    });
}

/// Alice withdraw `amount` collateral, balance changed, event is emitted
#[test]
fn test_withdraw_collateral_sunny_day() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_xor_as_collateral_type();
        let amount = balance!(10);
        let cdp_id = create_cdp_for_xor(alice(), amount, balance!(0));
        assert_balance(&alice_account_id(), balance!(0));

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
        assert_balance(&alice_account_id(), amount);
        let cdp = KensetsuPallet::cdp(cdp_id).expect("Must exist");
        assert_eq!(cdp.collateral_amount, balance!(0));
    });
}

// TODO test borrow
//  - signed account
//  - cdp owner
//  - cdp not found
//  - overflow
//  - unsafe
//  - collateral cap
//  - protocol cap
//  - sunny day + event, check KUSD supply

// TODO test repay_debt
//  - signed account
//  - cdp not found
//  - amount > debt, leftover not burned
//  - sunny day + event, check KUSD supply

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

// TODO test accrue
//  - cdp not found
//  - overflow
//  - sunny day, check treasury balance, KUSD supply

// TODO test update_collateral_risk_parameters
//  - signed account
//  - CollateralInfoNotFound
//  - sunny day, check all cdps accrued, check inserted, event

// TODO test update_hard_cap_total_supply
//  - signed account
//  - sunny day

// TODO test update_liquidation_penalty
//  - signed account
//  - sunny day

// TODO test withdraw_profit
//  - signed account
//  - sunny day, event

// TODO test donate
//  - signed account
//  - overflow
// with bad_debt == 0 and bad debt > 0
//  kusd_amount < bad_debt
// kusd_amount = bad_debt
// kusd_amount > bad_debt

// TODO add tests for accrue()
