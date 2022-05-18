use crate::mock::{new_tester, MigrationApp, Origin, Test, BASE_NETWORK_ID};
use crate::{Addresses, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::H160;

#[test]
fn test_register_network() {
    new_tester().execute_with(|| {
        assert!(!Addresses::<Test>::contains_key(BASE_NETWORK_ID + 1));
        assert_ok!(MigrationApp::register_network(
            Origin::root(),
            BASE_NETWORK_ID + 1,
            H160::repeat_byte(12)
        ));
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID + 1));
    });
}

#[test]
fn test_existing_register_network() {
    new_tester().execute_with(|| {
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID));
        assert_noop!(
            MigrationApp::register_network(Origin::root(), BASE_NETWORK_ID, H160::repeat_byte(12)),
            Error::<Test>::AppAlreadyExists
        );
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID));
    });
}
