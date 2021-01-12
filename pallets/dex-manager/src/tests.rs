use crate::{mock::*, Error};
use common::{hash, prelude::DEXInfo, EnsureDEXManager, ManagementMode, VAL, XOR};
use frame_support::{assert_noop, assert_ok};

#[test]
fn test_initialize_dex_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            true,
        ));
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_B_ID,
            VAL,
            BOB,
            false,
        ));
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
fn test_share_init_dex_permission_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            DEXModule::initialize_dex(Origin::signed(BOB), DEX_A_ID, XOR, BOB, true),
            permissions::Error::<Runtime>::Forbidden
        );
        assert_eq!(DEXModule::dex_id(DEX_A_ID), None);
        // ALICE owns INIT_DEX permission in genesis, and shares it with BOB
        permissions::Module::<Runtime>::grant_permission(ALICE, BOB, permissions::INIT_DEX)
            .expect("Failed to grant permission.");
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(BOB),
            DEX_A_ID,
            XOR,
            BOB,
            true,
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                is_public: true,
            })
        );
    })
}

#[test]
fn test_share_manage_dex_permission_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            BOB,
            false,
        ));
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
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            BOB,
            true
        ));
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_B_ID,
            XOR,
            BOB,
            true
        ));
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_ok!(result);
        let result =
            DEXModule::ensure_can_manage(&DEX_B_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_ok!(result);
    })
}

#[test]
fn test_initialize_without_init_dex_permissions_should_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            DEXModule::initialize_dex(Origin::signed(BOB), DEX_A_ID, XOR, BOB, true,),
            permissions::Error::<Runtime>::Forbidden
        );
    })
}

#[test]
fn test_can_manage_on_private_dex_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, false)
            .expect("Failed to initialize DEX.");
        // owner has full access
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private);
        assert_ok!(result);
        let result = DEXModule::ensure_can_manage(
            &DEX_A_ID,
            Origin::signed(ALICE),
            ManagementMode::PublicCreation,
        );
        assert_ok!(result);

        // another account has no access
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXModule::ensure_can_manage(
            &DEX_A_ID,
            Origin::signed(BOB),
            ManagementMode::PublicCreation,
        );
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);

        // sudo account is not handled
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::Private);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::PublicCreation);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
    })
}

#[test]
fn test_can_manage_on_public_dex_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, true)
            .expect("Failed to initialize DEX.");
        // owner has full access
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE), ManagementMode::Private);
        assert_ok!(result);
        let result = DEXModule::ensure_can_manage(
            &DEX_A_ID,
            Origin::signed(ALICE),
            ManagementMode::PublicCreation,
        );
        assert_ok!(result);

        // another account has only access in public mode
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB), ManagementMode::Private);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXModule::ensure_can_manage(
            &DEX_A_ID,
            Origin::signed(BOB),
            ManagementMode::PublicCreation,
        );
        assert_ok!(result);

        // sudo account is not handled
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::Private);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
        let result =
            DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root(), ManagementMode::PublicCreation);
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
    })
}

#[test]
fn test_ensure_dex_exists_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            BOB,
            true,
        ));
        assert_ok!(DEXModule::ensure_dex_exists(&DEX_A_ID));
        assert_noop!(
            DEXModule::ensure_dex_exists(&DEX_B_ID),
            Error::<Runtime>::DEXDoesNotExist
        );
    })
}

#[test]
fn test_list_dex_ids_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(DEXModule::list_dex_ids(), Vec::<DEXId>::new());
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            true,
        ));
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_B_ID,
            XOR,
            BOB,
            true,
        ));
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
            DEXModule::ensure_can_manage(
                &DEX_A_ID,
                Origin::signed(ALICE),
                ManagementMode::PublicCreation
            ),
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
