use crate::{mock::*, *};
use frame_support::assert_ok;
use sp_core::hash::H512;

#[test]
fn permission_check_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(PermissionsModule::check_permission(ALICE, TRANSFER));
    });
}

#[test]
fn permission_check_fails_with_permission_not_found_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        if let Err(Error::<Test>::PermissionNotFound) =
            PermissionsModule::check_permission(2, TRANSFER)
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_check_with_parameters_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(PermissionsModule::check_permission_with_parameters(
            ALICE,
            TRANSFER,
            H512::zero(),
        ));
    });
}

#[test]
fn permission_check_with_parameters_fails_with_permission_not_found_error() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        if let Err(Error::<Test>::PermissionNotFound) =
            PermissionsModule::check_permission_with_parameters(2, TRANSFER, H512::repeat_byte(1))
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_grant_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::grant_permission(ALICE, BOB, TRANSFER,));
    });
}

#[test]
fn permission_grant_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionNotFound) =
            PermissionsModule::grant_permission(BOB, ALICE, TRANSFER)
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_grant_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionNotOwned) =
            PermissionsModule::grant_permission(BOB, ALICE, EXCHANGE)
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_grant_with_parameters_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::grant_permission_with_parameters(
            ALICE,
            BOB,
            TRANSFER,
            H512::zero(),
        ));
    });
}

#[test]
fn permission_grant_with_parameters_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionNotFound) =
            PermissionsModule::grant_permission_with_parameters(
                BOB,
                ALICE,
                TRANSFER,
                H512::repeat_byte(1),
            )
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_grant_with_parameters_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionNotOwned) =
            PermissionsModule::grant_permission_with_parameters(BOB, ALICE, EXCHANGE, H512::zero())
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_transfer_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::transfer_permission(ALICE, BOB, TRANSFER,));
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_found_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionNotFound) =
            PermissionsModule::transfer_permission(BOB, ALICE, TRANSFER)
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionNotOwned) =
            PermissionsModule::transfer_permission(BOB, ALICE, EXCHANGE)
        {
        } else {
            panic!();
        }
    });
}

#[test]
fn permission_create_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(PermissionsModule::create_permission(
            ALICE,
            BOB,
            TRANSFER,
            Permission::<Test>::any(ALICE)
        ));
    });
}

#[test]
fn permission_create_fails_with_permission_already_exists_error() {
    ExtBuilder::default().build().execute_with(|| {
        if let Err(Error::<Test>::PermissionAlreadyExists) = PermissionsModule::create_permission(
            ALICE,
            ALICE,
            TRANSFER,
            Permission::<Test>::any(ALICE),
        ) {
        } else {
            panic!();
        }
    });
}
