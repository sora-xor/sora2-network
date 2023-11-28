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

use common::PredefinedAssetId::XOR;
use common::{balance, AssetInfoProvider, Balance};
use frame_support::{assert_err, assert_ok};
use frame_system::pallet_prelude::OriginFor;
use framenode_chain_spec::ext;
use framenode_runtime::kensetsu::CollateralRiskParameters;
use framenode_runtime::kensetsu::*;
use framenode_runtime::{Runtime, RuntimeOrigin};
use hex_literal::hex;
use sp_arithmetic::Perbill;
use sp_core::U256;
use sp_runtime::AccountId32;
use sp_runtime::DispatchError::BadOrigin;

type AccountId = AccountId32;
type KensetsuPallet = framenode_runtime::kensetsu::Pallet<Runtime>;
type KensetsuError = framenode_runtime::kensetsu::Error<Runtime>;

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
fn with_xor_as_collateral_type() {
    CollateralTypes::<Runtime>::set(
        <Runtime as assets::Config>::AssetId::from(XOR),
        Some(CollateralRiskParameters {
            max_supply: balance!(1000),
            liquidation_ratio: Perbill::from_float(0.5),
            stability_fee_rate: Default::default(),
        }),
    );
    MaxSupply::<Runtime>::set(balance!(1000));
}

/// Creates CDP with collateral asset id is XOR
fn with_xor_cdp_created(owner: OriginFor<Runtime>) {
    assert_ok!(KensetsuPallet::create_cdp(owner, XOR.into()),);
}

/// Deposits to CDP
fn with_xor_cdp_deposited(owner: OriginFor<Runtime>, cdp_id: U256, collateral_amount: Balance) {
    assert_ok!(assets::Pallet::<Runtime>::update_balance(
        RuntimeOrigin::root(),
        alice_account_id(),
        XOR.into(),
        collateral_amount.try_into().unwrap()
    ));
    assert_ok!(KensetsuPallet::deposit_collateral(
        owner,
        cdp_id,
        collateral_amount
    ));
}

/// Get CDP debt
fn with_cdp_debt(owner: OriginFor<Runtime>, cdp_id: U256, debt_amount: Balance) {
    assert_ok!(KensetsuPallet::borrow(owner, cdp_id, debt_amount));
}

/// Collateral Risk Parameters were not set for the AssetId by Risk Management Team,
/// is is restricted to create CDP for collateral not listed.
#[test]
fn test_create_cdp_for_asset_not_listed_must_result_in_error() {
    ext().execute_with(|| {
        assert_err!(
            KensetsuPallet::create_cdp(alice(), XOR.into()),
            KensetsuError::CollateralInfoNotFound
        );
    });
}

/// CDP might be created only by Signed Origin account.
#[test]
fn test_create_cdp_only_signed_origin() {
    ext().execute_with(|| {
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

/// If the number of cdp ids reached U256::MAX, next CDP will result in arithmetic error.
#[test]
fn test_create_cdp_overflow_error() {
    ext().execute_with(|| {
        with_xor_as_collateral_type();
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
    ext().execute_with(|| {
        with_xor_as_collateral_type();

        assert_ok!(KensetsuPallet::create_cdp(alice(), XOR.into()),);
        let cdp_id = U256::from(1);

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
    ext().execute_with(|| {
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
    ext().execute_with(|| {
        with_xor_as_collateral_type();
        // Alice is CDP owner
        with_xor_cdp_created(alice());
        let cdp_id = U256::from(1);

        assert_err!(
            KensetsuPallet::close_cdp(bob(), cdp_id),
            KensetsuError::OperationPermitted
        );
    });
}

/// If cdp doesn't exist, return error
#[test]
fn test_close_cdp_not_exists() {
    ext().execute_with(|| {
        with_xor_as_collateral_type();
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
    ext().execute_with(|| {
        with_xor_as_collateral_type();
        with_xor_cdp_created(alice());
        let cdp_id = U256::from(1);
        let deposit_amount = balance!(10);
        with_xor_cdp_deposited(alice(), cdp_id, deposit_amount);
        let debt_amount = balance!(1);
        with_cdp_debt(alice(), cdp_id, debt_amount);

        assert_err!(
            KensetsuPallet::close_cdp(alice(), cdp_id),
            KensetsuError::OutstandingDebt
        );
    });
}

/// Closes CDP and returns collateral to the owner
#[test]
fn test_close_cdp_sunny_day() {
    ext().execute_with(|| {
        with_xor_as_collateral_type();
        with_xor_cdp_created(alice());
        let cdp_id = U256::from(1);
        let deposit_amount = balance!(10);
        with_xor_cdp_deposited(alice(), cdp_id, deposit_amount);
        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice_account_id()).unwrap(),
            balance!(0)
        );

        assert_ok!(KensetsuPallet::close_cdp(alice(), cdp_id));

        assert_eq!(
            assets::Pallet::<Runtime>::free_balance(&XOR.into(), &alice_account_id()).unwrap(),
            balance!(10)
        );
        assert_eq!(KensetsuPallet::cdp(cdp_id), None);
    });
}

// TODO test deposit_collateral
//  - signed account
//  - cdp doesn't exist
//  - cdp not found
//  - not enough balance
//  - sunny day

// TODO test withdraw_collateral
//  - signed account
//  - sdp owner
//  - cdp not found
//  - collateral < withdraw
//  - cdp unsafe
//  - sunny day

// TODO test borrow
//  - signed account
//  - cdp owner
//  - cdp not found
//  - overflow
//  - unsafe
//  - collateral cap
//  - protocol cap
//  - sunny day

// TODO test repay_debt

// TODO test liquidate

// TODO test accrue

// TODO test update_collateral_risk_parameters

// TODO test update_hard_cap_total_supply

// TODO test update_liquidation_penalty

// TODO test withdraw_profit

// TODO test donate
