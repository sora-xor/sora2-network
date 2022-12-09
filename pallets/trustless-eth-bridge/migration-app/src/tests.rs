use crate::mock::{new_tester, MigrationApp, RuntimeOrigin, Test, BASE_NETWORK_ID};
use crate::{Addresses, Error};
use bridge_types::H160;
use common::DAI;
use frame_support::assert_noop;
use frame_support::assert_ok;

#[test]
fn test_register_network() {
    new_tester().execute_with(|| {
        assert!(!Addresses::<Test>::contains_key(BASE_NETWORK_ID + 1));
        assert_ok!(MigrationApp::register_network(
            RuntimeOrigin::root(),
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
            MigrationApp::register_network(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                H160::repeat_byte(12)
            ),
            Error::<Test>::AppAlreadyExists
        );
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID));
    });
}

#[test]
fn test_migrate_eth() {
    new_tester().execute_with(|| {
        assert_ok!(MigrationApp::migrate_eth(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID
        ),);
    });
}

#[test]
fn test_migrate_eth_not_exists() {
    new_tester().execute_with(|| {
        assert_noop!(
            MigrationApp::migrate_eth(RuntimeOrigin::root(), BASE_NETWORK_ID + 1),
            Error::<Test>::AppIsNotRegistered
        );
    });
}

#[test]
fn test_migrate_erc20() {
    new_tester().execute_with(|| {
        assert_ok!(MigrationApp::migrate_erc20(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID,
            vec![(DAI, H160::repeat_byte(12))]
        ),);
    });
}

#[test]
fn test_migrate_erc20_not_exists() {
    new_tester().execute_with(|| {
        assert_noop!(
            MigrationApp::migrate_erc20(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID + 1,
                vec![(DAI, H160::repeat_byte(12))]
            ),
            Error::<Test>::AppIsNotRegistered
        );
    });
}

#[test]
fn test_migrate_sidechain() {
    new_tester().execute_with(|| {
        assert_ok!(MigrationApp::migrate_sidechain(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID,
            vec![(DAI, H160::repeat_byte(12))]
        ),);
    });
}

#[test]
fn test_migrate_sidechain_not_exists() {
    new_tester().execute_with(|| {
        assert_noop!(
            MigrationApp::migrate_sidechain(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID + 1,
                vec![(DAI, H160::repeat_byte(12))]
            ),
            Error::<Test>::AppIsNotRegistered
        );
    });
}
