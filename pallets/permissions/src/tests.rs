use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok};
use sp_core::hash::H512;

#[test]
fn permission_check_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(PermissionsModule::check_permission(
            Origin::signed(ALICE),
            TRANSFER
        ));
    });
}

#[test]
fn permission_check_fails_with_permission_not_found_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            PermissionsModule::check_permission(Origin::signed(2), TRANSFER),
            Error::<Test>::PermissionNotFound
        );
    });
}

#[test]
fn permission_check_with_parameters_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(PermissionsModule::check_permission_with_parameters(
            Origin::signed(ALICE),
            TRANSFER,
            H512::zero(),
        ));
    });
}

#[test]
fn permission_check_with_parameters_fails_with_permission_not_found_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            PermissionsModule::check_permission_with_parameters(
                Origin::signed(2),
                TRANSFER,
                H512::repeat_byte(1)
            ),
            Error::<Test>::PermissionNotFound
        );
    });
}

#[test]
fn permission_grant_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::grant_permission(
            Origin::signed(ALICE),
            BOB,
            TRANSFER,
        ));
    });
}

#[test]
fn permission_grant_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            PermissionsModule::grant_permission(Origin::signed(BOB), ALICE, TRANSFER,),
            Error::<Test>::PermissionNotFound
        );
    });
}

#[test]
fn permission_grant_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            PermissionsModule::grant_permission(Origin::signed(BOB), ALICE, EXCHANGE,),
            Error::<Test>::PermissionNotOwned
        );
    });
}

#[test]
fn permission_grant_with_parameters_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::grant_permission_with_parameters(
            Origin::signed(ALICE),
            BOB,
            TRANSFER,
            H512::zero(),
        ));
    });
}

#[test]
fn permission_grant_with_parameters_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            PermissionsModule::grant_permission_with_parameters(
                Origin::signed(BOB),
                ALICE,
                TRANSFER,
                H512::repeat_byte(1),
            ),
            Error::<Test>::PermissionNotFound
        );
    });
}

#[test]
fn permission_grant_with_parameters_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            PermissionsModule::grant_permission_with_parameters(
                Origin::signed(BOB),
                ALICE,
                EXCHANGE,
                H512::zero()
            ),
            Error::<Test>::PermissionNotOwned
        );
    });
}

#[test]
fn permission_transfer_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::transfer_permission(
            Origin::signed(ALICE),
            BOB,
            TRANSFER,
        ));
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            PermissionsModule::transfer_permission(Origin::signed(BOB), ALICE, TRANSFER,),
            Error::<Test>::PermissionNotFound
        );
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        assert_noop!(
            PermissionsModule::transfer_permission(Origin::signed(BOB), ALICE, EXCHANGE,),
            Error::<Test>::PermissionNotOwned
        );
    });
}
