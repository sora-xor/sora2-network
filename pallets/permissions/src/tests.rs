use crate::{mock::*, *};
use frame_support::assert_ok;
use sp_core::hash::H512;

type Permissions = Module<Runtime>;

// The id for the user-created permission
const CUSTOM_PERMISSION: PermissionId = 10001;

#[test]
fn permission_check_passes() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(Permissions::check_permission(BOB, TRANSFER));
    });
}

#[test]
fn permission_check_restrictive_permission_passes() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(Permissions::check_permission(BOB, TRANSFER));
    });
}

#[test]
fn permission_check_restrictive_permission_fails_with_forbidden_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::check_permission(ALICE, TRANSFER) {
            Err(Error::<Runtime>::Forbidden) => {}
            result => panic!("{:?}", result),
        }
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
            TRANSFER,
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
            TRANSFER,
            &Scope::Limited(H512::repeat_byte(1)),
        ) {
            Err(Error::<Runtime>::Forbidden) => {}
            result => panic!("{:?}", result),
        }
        match Permissions::check_permission_with_scope(
            ALICE,
            TRANSFER,
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
            BURN,
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
        match Permissions::grant_permission(BOB, ALICE, BURN) {
            Err(Error::<Runtime>::PermissionNotFound) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_grant_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::grant_permission(BOB, ALICE, INIT_DEX) {
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
            TRANSFER,
            Scope::Limited(H512::repeat_byte(1))
        ));
        assert_ok!(Permissions::grant_permission_with_scope(
            ALICE,
            BOB,
            TRANSFER,
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
            BURN,
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
        match Permissions::grant_permission_with_scope(BOB, ALICE, INIT_DEX, Scope::Unlimited) {
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
        match Permissions::transfer_permission(BOB, ALICE, BURN, Scope::Unlimited) {
            Err(Error::<Runtime>::PermissionNotFound) => {}
            result => panic!("{:?}", result),
        }
    });
}

#[test]
fn permission_transfer_fails_with_permission_not_owned_error() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::transfer_permission(BOB, ALICE, INIT_DEX, Scope::Unlimited) {
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
            BURN,
            Scope::Unlimited
        ));
        assert_ok!(Permissions::check_permission(BOB, BURN));
    });
}

#[test]
fn permission_assign_fails_with_permission_already_exists() {
    ExtBuilder::default().build().execute_with(|| {
        match Permissions::assign_permission(ALICE, &BOB, INIT_DEX, Scope::Unlimited) {
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
            Mode::Permit
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
        match Permissions::create_permission(ALICE, BOB, INIT_DEX, Scope::Unlimited, Mode::Permit) {
            Err(Error::<Runtime>::PermissionAlreadyExists) => {}
            result => panic!("{:?}", result),
        }
    });
}
