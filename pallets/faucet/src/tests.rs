use crate::{mock::*, *};
use common::fixed;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

type Module = crate::Module<Test>;
type Assets = assets::Module<Test>;
type System = frame_system::Module<Test>;

#[test]
fn transfer_passes_native_currency() {
    ExtBuilder::build().execute_with(|| {
        // Receive 100 (Limit) in two transfers
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            fixed!(49, 91).into()
        ));
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            fixed!(50, 09).into()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            fixed!(50).into()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            fixed!(0).into()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            fixed!(100).into()
        );
    });
}

#[test]
fn transfer_passes_multiple_assets() {
    ExtBuilder::build().execute_with(|| {
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            fixed!(100).into()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            fixed!(50).into()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            fixed!(0).into()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            fixed!(100).into()
        );

        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            VAL,
            bob(),
            fixed!(50, 43).into()
        ));
        assert_eq!(
            Assets::free_balance(&VAL, &account_id()).unwrap(),
            fixed!(99, 57).into()
        );
        assert_eq!(
            Assets::free_balance(&VAL, &alice()).unwrap(),
            fixed!(0).into()
        );
        assert_eq!(
            Assets::free_balance(&VAL, &bob()).unwrap(),
            fixed!(50, 43).into()
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
            fixed!(100).into()
        ));
        System::set_block_number(14401);
        assert_ok!(Module::transfer(
            Origin::signed(alice()),
            XOR,
            bob(),
            fixed!(50).into()
        ));
        assert_eq!(
            Assets::free_balance(&XOR, &account_id()).unwrap(),
            fixed!(0).into()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &alice()).unwrap(),
            fixed!(0).into()
        );
        assert_eq!(
            Assets::free_balance(&XOR, &bob()).unwrap(),
            fixed!(150).into()
        );
    });
}

#[test]
fn transfer_fails_with_bad_origin() {
    ExtBuilder::build().execute_with(|| {
        assert_noop!(
            Module::transfer(Origin::root(), XOR, bob(), fixed!(0, 5).into()),
            DispatchError::BadOrigin
        );
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
                fixed!(0, 5).into()
            ),
            crate::Error::<Test>::AssetNotSupported
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
            fixed!(100).into()
        ));
        assert_noop!(
            Module::transfer(Origin::signed(alice()), XOR, bob(), fixed!(0, 2).into()),
            crate::Error::<Test>::AmountAboveLimit
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
            fixed!(100).into()
        ));
        assert_noop!(
            Module::transfer(Origin::signed(bob()), XOR, alice(), fixed!(100).into()),
            crate::Error::<Test>::NotEnoughReserves
        );
    });
}
