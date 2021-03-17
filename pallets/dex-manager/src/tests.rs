use crate::{mock::*, Error, Module};
use common::{hash, prelude::DEXInfo, EnsureDEXManager, ManagementMode, VAL, XOR};
use frame_support::{assert_noop, assert_ok};
use permissions::{Scope, MANAGE_DEX};

type DEXModule = Module<Runtime>;

#[test]
fn test_initialize_dex_should_pass() {
    let mut ext = ExtBuilder {
        initial_dex_list: vec![
            (
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    is_public: true,
                },
            ),
            (
                DEX_B_ID,
                DEXInfo {
                    base_asset_id: VAL,
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
            DEXModule::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                is_public: true,
            })
        );
        assert_eq!(
            DEXModule::dex_id(DEX_B_ID),
            Some(DEXInfo {
                base_asset_id: VAL,
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
                is_public: false,
            },
        )],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![BOB])],
        initial_permissions: vec![(BOB, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_ok!(result);
        permissions::Module::<Runtime>::grant_permission_with_scope(
            BOB,
            ALICE,
            permissions::MANAGE_DEX,
            permissions::Scope::Limited(hash(&DEX_A_ID)),
        )
        .expect("Failed to transfer permission.");
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private);
        assert_ok!(result);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
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
                    is_public: true,
                },
            ),
            (
                DEX_B_ID,
                DEXInfo {
                    base_asset_id: XOR,
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
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_ok!(result);
        let result =
            DEXModule::ensure_can_manage(&DEX_B_ID, Origin::signed(BOB), ManagementMode::Private);
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
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private);
        assert_ok!(result);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Public);
        assert_ok!(result);

        // another account has no access
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Public);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);

        // sudo account is not handled
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::Private);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::Public);
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
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private);
        assert_ok!(result);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Public);
        assert_ok!(result);

        // another account has only access in public mode
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Public);
        assert_ok!(result);

        // sudo account is not handled
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::Private);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::Public);
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
                is_public: true,
            },
        )],
        initial_permission_owners: vec![(MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![ALICE])],
        initial_permissions: vec![(ALICE, Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX])],
        ..Default::default()
    }
    .build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::ensure_dex_exists(&DEX_A_ID));
        assert_noop!(
            DEXModule::ensure_dex_exists(&DEX_B_ID),
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
        assert_eq!(DEXModule::list_dex_ids(), Vec::<DEXId>::new());
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
                    is_public: true,
                },
            ),
            (
                DEX_B_ID,
                DEXInfo {
                    base_asset_id: XOR,
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
        assert_eq!(DEXModule::list_dex_ids(), vec![DEX_A_ID, DEX_B_ID]);
    })
}

#[test]
fn test_queries_for_nonexistant_dex_should_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private),
            Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Public),
            Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            DEXModule::get_dex_info(&DEX_A_ID),
            Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            DEXModule::ensure_dex_exists(&DEX_A_ID),
            Error::<Runtime>::DEXDoesNotExist
        );
    })
}
