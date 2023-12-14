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
use crate::mock::{RuntimeOrigin, TestRuntime};

use common::{AssetInfoProvider, Balance, XOR};
use frame_support::assert_ok;
use frame_system::pallet_prelude::OriginFor;
use hex_literal::hex;
use sp_arithmetic::Perbill;
use sp_core::U256;
use sp_runtime::AccountId32;

type AccountId = AccountId32;
type KensetsuPallet = Pallet<TestRuntime>;
type AssetId = <TestRuntime as assets::Config>::AssetId;

/// Predefined AccountId `Alice`
pub fn alice_account_id() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

/// Regular client account Alice
pub fn alice() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(alice_account_id())
}

/// Predefined AccountId `Bob`
pub fn bob_account_id() -> AccountId {
    AccountId32::from(hex!(
        "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
    ))
}

/// Regular client account Alice
pub fn bob() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(bob_account_id())
}

/// Returns Kensetsu technical treasury account id.
pub fn tech_account_id() -> AccountId {
    let tech_account = <TestRuntime as Config>::TreasuryTechAccount::get();
    technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(&tech_account)
        .expect("Must succeed")
}

/// Returns Risk Manager account
pub fn risk_manager() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(alice_account_id())
}

/// Returns Protocol Owner account id
pub fn protocol_owner_account_id() -> AccountId {
    bob_account_id()
}

/// Returns Protocol Owner account
pub fn protocol_owner() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(bob_account_id())
}

/// Sets protocol bad debt in KUSD.
pub fn set_bad_debt(bad_debt: Balance) {
    BadDebt::<TestRuntime>::set(bad_debt);
}

/// Asserts that protocol bad debt is expected amount.
pub fn assert_bad_debt(expected_amount: Balance) {
    assert_eq!(BadDebt::<TestRuntime>::get(), expected_amount);
}

/// Sets XOR asset id as collateral with default parameters
/// As if Risk Manager called `update_collateral_risk_parameters(XOR, some_info)`
pub fn set_xor_as_collateral_type(
    hard_cap: Balance,
    liquidation_ratio: Perbill,
    stability_fee_rate: FixedU128,
) {
    CollateralTypes::<TestRuntime>::set(
        XOR,
        Some(CollateralRiskParameters {
            hard_cap,
            max_liquidation_lot: balance!(1000),
            liquidation_ratio,
            stability_fee_rate,
        }),
    );
    KusdHardCap::<TestRuntime>::set(hard_cap);
}

/// Creates CDP with XOR as collateral asset id
pub fn create_cdp_for_xor(
    owner: OriginFor<TestRuntime>,
    collateral: Balance,
    debt: Balance,
) -> U256 {
    assert_ok!(KensetsuPallet::create_cdp(owner.clone(), XOR));
    let cdp_id = NextCDPId::<TestRuntime>::get();
    if collateral > 0 {
        deposit_xor_to_cdp(owner.clone(), cdp_id, collateral);
    }
    if debt > 0 {
        assert_ok!(KensetsuPallet::borrow(owner, cdp_id, debt));
    }
    cdp_id
}

/// Deposits to CDP
pub fn deposit_xor_to_cdp(owner: OriginFor<TestRuntime>, cdp_id: U256, collateral_amount: Balance) {
    set_balance(alice_account_id(), collateral_amount);
    assert_ok!(KensetsuPallet::deposit_collateral(
        owner,
        cdp_id,
        collateral_amount
    ));
}

/// Updates account balance
pub fn set_balance(account: AccountId, balance: Balance) {
    assert_ok!(assets::Pallet::<TestRuntime>::update_balance(
        RuntimeOrigin::root(),
        account,
        XOR,
        balance.try_into().unwrap()
    ));
}

/// Returns total supply for asset.
pub fn get_total_supply(asset_id: &AssetId) -> Balance {
    <TestRuntime as pallet::Config>::AssetInfoProvider::total_issuance(asset_id)
        .expect("Must succeed")
}

/// Asserts account balance is expected.
pub fn assert_balance(account: &AccountId, asset_id: &AssetId, expected: Balance) {
    assert_eq!(
        assets::Pallet::<TestRuntime>::free_balance(asset_id, account).unwrap(),
        expected
    );
}
