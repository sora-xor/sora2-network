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
use crate::{Error, Pallet};
use common::prelude::DEXInfo;
use common::{hash, DexInfoProvider, EnsureDEXManager, ManagementMode, VAL, XOR, XST};
use frame_support::assert_noop;
use frame_support::assert_ok;
use permissions::{Scope, MANAGE_DEX};

type DEXPallet = Pallet<Runtime>;

#[test]
fn test_initialize_dex_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![
            (
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
            (
                DEX_B_ID,
                DEXInfo {
                    base_asset_id: VAL,
                    synthetic_base_asset_id: XST,
                    is_public: false,
                },
            ),
        ],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![ALICE])],
        initial_permissions: vec![(ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        assert_eq!(
            DEXPallet::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                synthetic_base_asset_id: XST,
                is_public: true,
            })
        );
        assert_eq!(
            DEXPallet::dex_id(DEX_B_ID),
            Some(DEXInfo {
                base_asset_id: VAL,
                synthetic_base_asset_id: XST,
                is_public: false,
            })
        );
    })
}

#[test]
fn test_share_manage_dex_permission_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![(
            DEX_A_ID,
            DEXInfo {
                base_asset_id: XOR,
                synthetic_base_asset_id: XST,
                is_public: false,
            },
        )],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![BOB])],
        initial_permissions: vec![(BOB, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(ALICE),
            ManagementMode::Private,
        );
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Private,
        );
        assert_ok!(result);
        permissions::Pallet::<Runtime>::grant_permission_with_scope(
            BOB,
            ALICE,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(hash(&DEX_A_ID)),
        )
        .expect("Failed to transfer permission.");
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(ALICE),
            ManagementMode::Private,
        );
        assert_ok!(result);
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Private,
        );
        assert_ok!(result);
    })
}

#[test]
fn test_own_multiple_dexes_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![
            (
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
            (
                DEX_B_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
        ],
        initial_permission_owners: vec![
            (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![BOB]),
            (MANAGE_DEX, Scope::Limited(hash(&DEX_B_ID)), vec![BOB]),
        ],
        initial_permissions: vec![
            (BOB, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
            (BOB, Scope::Limited(hash(&DEX_B_ID)), vec![MANAGE_DEX]),
        ],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Private,
        );
        assert_ok!(result);
        let result = DEXPallet::ensure_can_manage(
            &DEX_B_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Private,
        );
        assert_ok!(result);
    })
}

#[test]
fn test_can_manage_on_private_dex_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![(
            DEX_A_ID,
            DEXInfo {
                base_asset_id: XOR,
                synthetic_base_asset_id: XST,
                is_public: false,
            },
        )],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![ALICE])],
        initial_permissions: vec![(ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        // owner has full access
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(ALICE),
            ManagementMode::Private,
        );
        assert_ok!(result);
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(ALICE),
            ManagementMode::Public,
        );
        assert_ok!(result);

        // another account has no access
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Private,
        );
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Public,
        );
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);

        // sudo account is not handled
        let result =
            DEXPallet::ensure_can_manage(&DEX_A_ID, RuntimeOrigin::root(), ManagementMode::Private);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
        let result =
            DEXPallet::ensure_can_manage(&DEX_A_ID, RuntimeOrigin::root(), ManagementMode::Public);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
    })
}

#[test]
fn test_can_manage_on_public_dex_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![(
            DEX_A_ID,
            DEXInfo {
                base_asset_id: XOR,
                synthetic_base_asset_id: XST,
                is_public: true,
            },
        )],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![ALICE])],
        initial_permissions: vec![(ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        // owner has full access
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(ALICE),
            ManagementMode::Private,
        );
        assert_ok!(result);
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(ALICE),
            ManagementMode::Public,
        );
        assert_ok!(result);

        // another account has only access in public mode
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Private,
        );
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXPallet::ensure_can_manage(
            &DEX_A_ID,
            RuntimeOrigin::signed(BOB),
            ManagementMode::Public,
        );
        assert_ok!(result);

        // sudo account is not handled
        let result =
            DEXPallet::ensure_can_manage(&DEX_A_ID, RuntimeOrigin::root(), ManagementMode::Private);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
        let result =
            DEXPallet::ensure_can_manage(&DEX_A_ID, RuntimeOrigin::root(), ManagementMode::Public);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
    })
}

#[test]
fn test_ensure_dex_exists_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![(
            DEX_A_ID,
            DEXInfo {
                base_asset_id: XOR,
                synthetic_base_asset_id: XST,
                is_public: true,
            },
        )],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![ALICE])],
        initial_permissions: vec![(ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        assert_ok!(DEXPallet::ensure_dex_exists(&DEX_A_ID));
        assert_noop!(
            DEXPallet::ensure_dex_exists(&DEX_B_ID),
            Error::<Runtime>::DEXDoesNotExist
        );
    })
}

#[test]
fn test_list_dex_ids_empty_should_pass() {
    let mut ext = ExtBuilder {
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        assert_eq!(DEXPallet::list_dex_ids(), Vec::<common::DEXId>::new());
    })
}

#[test]
fn test_list_dex_ids_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![
            (
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
            (
                DEX_B_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
        ],
        initial_permission_owners: vec![
            (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![ALICE]),
            (MANAGE_DEX, Scope::Limited(hash(&DEX_B_ID)), vec![BOB]),
        ],
        initial_permissions: vec![
            (ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
            (ALICE, Scope::Limited(hash(&DEX_B_ID)), vec![MANAGE_DEX]),
        ],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        assert_eq!(DEXPallet::list_dex_ids(), vec![DEX_A_ID, DEX_B_ID]);
    })
}

#[test]
fn test_queries_for_nonexistant_dex_should_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            DEXPallet::ensure_can_manage(
                &DEX_A_ID,
                RuntimeOrigin::signed(ALICE),
                ManagementMode::Private
            ),
            Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            DEXPallet::ensure_can_manage(
                &DEX_A_ID,
                RuntimeOrigin::signed(ALICE),
                ManagementMode::Public
            ),
            Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            DEXPallet::get_dex_info(&DEX_A_ID),
            Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            DEXPallet::ensure_dex_exists(&DEX_A_ID),
            Error::<Runtime>::DEXDoesNotExist
        );
    })
}
