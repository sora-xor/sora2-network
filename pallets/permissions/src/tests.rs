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

use crate::mock::*;
use crate::*;
use frame_support::assert_ok;
use sp_core::hash::H512;

type Permissions = Module<Runtime>;

// The id for the user-created permission
const CUSTOM_PERMISSION: PermissionId = 10001;

#[test]
fn permission_check_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Permissions::check_permission(BOB, BURN));
    });
}

#[test]
fn permission_check_fails_with_forbidden_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| match Permissions::check_permission(BOB, MINT) {
        Err(Error::<Runtime>::Forbidden) => {}
        result => panic!("{:?}", result),
    });
}

#[test]
fn permission_check_with_scope_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Permissions::check_permission_with_scope(
            BOB,
            INIT_DEX,
            &Scope::Unlimited,
        ));
    });
}

#[test]
fn permission_check_restrictive_permission_with_scope_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        match Permissions::check_permission_with_scope(
            BOB,
            MINT,
            &Scope::Limited(H512::repeat_byte(1)),
        ) {
            Err(Error::<Runtime>::Forbidden) => {}
            result => panic!("{:?}", result),
        }
        match Permissions::check_permission_with_scope(
            ALICE,
            MINT,
            &Scope::Limited(H512::repeat_byte(1)),
        ) {
            Err(Error::<Runtime>::Forbidden) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_check_with_scope_fails_with_forbidden_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        match Permissions::check_permission_with_scope(
            BOB,
            SLASH,
            &Scope::Limited(H512::repeat_byte(1)),
        ) {
            Err(Error::<Runtime>::Forbidden) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_grant_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::grant_permission(JOHN, BOB, MINT));
        assert_ok!(Permissions::check_permission(BOB, MINT));
        // Verify existing permissions are kept
        assert_ok!(Permissions::check_permission(BOB, INIT_DEX));
    });
}

#[test]
fn permission_grant_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::grant_permission(BOB, ALICE, MANAGE_DEX) {
            Err(Error::<Runtime>::PermissionNotFound) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_grant_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::grant_permission(BOB, ALICE, BURN) {
            Err(Error::<Runtime>::PermissionNotOwned) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_grant_with_scope_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::grant_permission_with_scope(
            JOHN,
            BOB,
            MINT,
            Scope::Unlimited,
        ));
        assert_ok!(Permissions::check_permission(BOB, MINT));
        // Verify existing permissions are kept
        assert_ok!(Permissions::check_permission(BOB, INIT_DEX));
    });
}

#[test]
fn permission_grant_with_scope_multiple_times_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::grant_permission_with_scope(
            ALICE,
            JOHN,
            BURN,
            Scope::Limited(H512::repeat_byte(1))
        ));
        assert_ok!(Permissions::grant_permission_with_scope(
            ALICE,
            BOB,
            BURN,
            Scope::Limited(H512::repeat_byte(1))
        ));
        assert_ok!(Permissions::grant_permission_with_scope(
            JOHN,
            BOB,
            MINT,
            Scope::Limited(H512::repeat_byte(1))
        ));
    });
}

#[test]
fn permission_grant_with_scope_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::grant_permission_with_scope(
            BOB,
            ALICE,
            MANAGE_DEX,
            Scope::Limited(H512::repeat_byte(1)),
        ) {
            Err(Error::<Runtime>::PermissionNotFound) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_grant_with_scope_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::grant_permission_with_scope(BOB, ALICE, SLASH, Scope::Unlimited) {
            Err(Error::<Runtime>::PermissionNotOwned) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_transfer_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::transfer_permission(
            JOHN,
            BOB,
            MINT,
            Scope::Unlimited
        ));
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::transfer_permission(BOB, ALICE, MANAGE_DEX, Scope::Unlimited) {
            Err(Error::<Runtime>::PermissionNotFound) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::transfer_permission(BOB, ALICE, MINT, Scope::Unlimited) {
            Err(Error::<Runtime>::PermissionNotOwned) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_assign_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::assign_permission(
            ALICE,
            &BOB,
            SLASH,
            Scope::Unlimited
        ));
        assert_ok!(Permissions::check_permission(BOB, SLASH));
    });
}

#[test]
fn permission_assign_fails_with_permission_already_exists() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::assign_permission(ALICE, &BOB, BURN, Scope::Unlimited) {
            Err(Error::<Runtime>::PermissionAlreadyExists) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_create_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::create_permission(
            ALICE,
            BOB,
            CUSTOM_PERMISSION,
            Scope::Unlimited,
        ));
        // Verify Alice is the owner of CustomPermission
        assert_ok!(Permissions::grant_permission(
            ALICE,
            JOHN,
            CUSTOM_PERMISSION
        ));
        assert_ok!(Permissions::check_permission(BOB, CUSTOM_PERMISSION));
        // Verify existing permissions are kept
        assert_ok!(Permissions::check_permission(BOB, INIT_DEX));
    });
}

#[test]
fn permission_create_fails_with_permission_already_exists_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::create_permission(ALICE, BOB, INIT_DEX, Scope::Unlimited) {
            Err(Error::<Runtime>::PermissionAlreadyExists) => {}
            result => panic!("{:?}", result),
        }
    });
}
