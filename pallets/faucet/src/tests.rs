use common::balance;
use frame_support::{assert_noop, assert_ok};

use crate::mock::*;
use crate::*;

type Module = crate::Module<Runtime>;
type Assets = assets::Module<Runtime>;
type System = frame_system::Module<Runtime>;

#[test]
fn transfer_passes_unsigned() {
    ExtBuilder::build().execute_with(|| {
        // Receive 100 (Limit) in two transfers
        assert_ok!(Module::transfer(
            Origin::none(),
            XOR,
            bob(),
            balance!(2999.91)
        ));
        assert_ok!(Module::transfer(
            Origin::none(),
            XOR,
            bob(),
            balance!(3000.09)
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            balance!(3000)
        );
        assert_eq!(Assets::free_balance(&XOR, &bob()).unwrap(), balance!(6000));
    });
}

#[test]
fn transfer_passes_native_currency() {
    ExtBuilder::build().execute_with(|| {
        // Receive 6000 (Limit) in two transfers
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(2999.91)
        ));
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(3000.09)
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            balance!(3000)
        );
        assert_eq!(Assets::free_balance(&XOR, &alice()).unwrap(), 0);
        assert_eq!(Assets::free_balance(&XOR, &bob()).unwrap(), balance!(6000));
    });
}

#[test]
fn transfer_passes_multiple_assets() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(6000)
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            balance!(3000)
        );
        assert_eq!(Assets::free_balance(&XOR, &alice()).unwrap(), 0);
        assert_eq!(Assets::free_balance(&XOR, &bob()).unwrap(), balance!(6000));

        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            VAL,
            bob(),
            balance!(3000.43)
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &account_id()).unwrap(),
            balance!(5999.57)
        );
        assert_eq!(Assets::free_balance(&VAL, &alice()).unwrap(), 0);
        assert_eq!(
            Assets::free_balance(&VAL, &bob()).unwrap(),
            balance!(3000.43)
        );
    });
}

#[test]
fn transfer_passes_after_limit_is_reset() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(6000)
        ));
        System::set_block_number(14401);
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(3000)
        ));
        assert_eq!(Assets::free_balance(&XOR, &account_id()).unwrap(), 0);
        assert_eq!(Assets::free_balance(&XOR, &alice()).unwrap(), 0);
        assert_eq!(Assets::free_balance(&XOR, &bob()).unwrap(), balance!(9000));
    });
}

#[test]
fn transfer_fails_with_asset_not_supported() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            Module::transfer(
                Origin::signed(alice()),
                NOT_SUPPORTED_ASSET_ID,
                bob(),
                balance!(0.5)
            ),
            crate::Error::<Runtime>::AssetNotSupported
        );
    });
}

#[test]
fn transfer_fails_with_amount_above_limit() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(6000)
        ));
        assert_noop!(
            Module::transfer(Origin::signed(alice()), XOR, bob(), balance!(0.2)),
            crate::Error::<Runtime>::AmountAboveLimit
        );
    });
}

#[test]
fn transfer_fails_with_not_enough_reserves() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            balance!(6000)
        ));
        assert_noop!(
            Module::transfer(Origin::signed(bob()), XOR, alice(), balance!(6000)),
            crate::Error::<Runtime>::NotEnoughReserves
        );
    });
}
