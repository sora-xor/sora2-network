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

use crate::mock::{new_test_ext, RuntimeEvent, RuntimeOrigin, System, TestRuntime};
use common::{AssetInfoProvider, XOR};
use frame_support::{assert_noop, assert_ok, error::BadOrigin};
use frame_system::pallet_prelude::OriginFor;
use hex_literal::hex;
use sp_runtime::AccountId32;

type SoratopiaPallet = Pallet<TestRuntime>;

/// Predefined AccountId `Alice`
pub fn alice_account_id() -> AccountId32 {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

/// Predefined AccountId `Bob`
pub fn bob_account_id() -> AccountId32 {
    AccountId32::from(hex!(
        "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
    ))
}

/// Regular client account Alice
pub fn alice() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(alice_account_id())
}

/// Regular client account Bob
pub fn bob() -> OriginFor<TestRuntime> {
    RuntimeOrigin::signed(bob_account_id())
}

fn mint_xor(account_id: AccountId32, amount: u128) {
    assert_ok!(assets::Pallet::<TestRuntime>::update_balance(
        RuntimeOrigin::root(),
        account_id,
        XOR,
        amount.try_into().unwrap(),
    ));
}

#[test]
fn test_check_in() {
    new_test_ext().execute_with(|| {
        mint_xor(alice_account_id(), 1000);

        assert_ok!(SoratopiaPallet::check_in(alice()));

        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap_or(0),
            0
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap_or(0),
            0
        );
        assert_eq!(SoratopiaPallet::last_check_in(alice_account_id()), Some(1));
        System::assert_has_event(RuntimeEvent::Soratopia(Event::CheckIn(alice_account_id())));
    });
}

#[test]
fn check_in_fails_without_balance_and_rolls_back_events() {
    new_test_ext().execute_with(|| {
        assert!(SoratopiaPallet::check_in(alice()).is_err());

        assert_eq!(System::events().len(), 0);
        assert_eq!(SoratopiaPallet::last_check_in(alice_account_id()), None);
    });
}

#[test]
fn check_in_respects_interval() {
    new_test_ext().execute_with(|| {
        mint_xor(alice_account_id(), 3000);

        assert_ok!(SoratopiaPallet::check_in(alice()));
        let event_count = System::events().len();

        assert_noop!(
            SoratopiaPallet::check_in(alice()),
            Error::<TestRuntime>::CheckInTooSoon
        );
        assert_eq!(System::events().len(), event_count);
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap(),
            2000
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap(),
            2000
        );

        System::set_block_number(10);
        assert_noop!(
            SoratopiaPallet::check_in(alice()),
            Error::<TestRuntime>::CheckInTooSoon
        );
        assert_eq!(System::events().len(), event_count);
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap(),
            2000
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap(),
            2000
        );

        System::set_block_number(11);
        assert_ok!(SoratopiaPallet::check_in(alice()));

        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap(),
            1000
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap(),
            1000
        );
        assert_eq!(SoratopiaPallet::last_check_in(alice_account_id()), Some(11));
    });
}

#[test]
fn unsigned_check_in_is_rejected_without_side_effects() {
    new_test_ext().execute_with(|| {
        mint_xor(alice_account_id(), 1000);
        System::reset_events();

        assert_noop!(SoratopiaPallet::check_in(RuntimeOrigin::none()), BadOrigin);

        assert_eq!(System::events().len(), 0);
        assert_eq!(SoratopiaPallet::last_check_in(alice_account_id()), None);
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap(),
            1000
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap(),
            1000
        );
    });
}

#[test]
fn failed_burn_after_interval_does_not_advance_check_in() {
    new_test_ext().execute_with(|| {
        mint_xor(alice_account_id(), 1500);

        assert_ok!(SoratopiaPallet::check_in(alice()));
        let event_count = System::events().len();

        System::set_block_number(11);
        assert!(SoratopiaPallet::check_in(alice()).is_err());

        assert_eq!(System::events().len(), event_count);
        assert_eq!(SoratopiaPallet::last_check_in(alice_account_id()), Some(1));
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap(),
            500
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap(),
            500
        );
    });
}

#[test]
fn future_last_check_in_blocks_without_burning() {
    new_test_ext().execute_with(|| {
        mint_xor(alice_account_id(), 1000);
        LastCheckIn::<TestRuntime>::insert(alice_account_id(), 100);
        System::reset_events();

        assert_noop!(
            SoratopiaPallet::check_in(alice()),
            Error::<TestRuntime>::CheckInTooSoon
        );

        assert_eq!(System::events().len(), 0);
        assert_eq!(
            SoratopiaPallet::last_check_in(alice_account_id()),
            Some(100)
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap(),
            1000
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap(),
            1000
        );
    });
}

#[test]
fn cooldown_is_per_account() {
    new_test_ext().execute_with(|| {
        mint_xor(alice_account_id(), 1000);
        mint_xor(bob_account_id(), 1000);

        assert_ok!(SoratopiaPallet::check_in(alice()));
        assert_ok!(SoratopiaPallet::check_in(bob()));

        assert_eq!(SoratopiaPallet::last_check_in(alice_account_id()), Some(1));
        assert_eq!(SoratopiaPallet::last_check_in(bob_account_id()), Some(1));
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &alice_account_id()).unwrap_or(0),
            0
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::free_balance(&XOR, &bob_account_id()).unwrap_or(0),
            0
        );
        assert_eq!(
            assets::Pallet::<TestRuntime>::total_issuance(&XOR).unwrap_or(0),
            0
        );
    });
}
