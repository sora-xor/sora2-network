use crate::{mock::*, *};
use common::AssetSymbol;
use frame_support::{assert_noop, assert_ok};
use sp_core::hash::H512;

#[test]
fn farm_creation_passes() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[2; 64]);
        let incenitive = Incentive::new(XOR, 1_000_u128.into());
        let parameters = Parameters::new(DateTimePeriod::new(0, 1), incenitive);
        Assets::register(Origin::signed(ALICE), XOR, AssetSymbol(b"XOR".to_vec()), 18);
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &ALICE,
            100_000_000_u128.into()
        ));
        assert_ok!(FarmsModule::create(
            Origin::signed(ALICE),
            farm_name,
            parameters
        ));
    });
}

#[test]
fn farm_create_fails_with_forbidden_error() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[3; 64]);
        let incenitive = Incentive::new(XOR, 1_000_u128.into());
        let parameters = Parameters::new(DateTimePeriod::new(0, 1), incenitive);
        let result = FarmsModule::create(Origin::signed(BOB), farm_name, parameters);
        assert_noop!(result, permissions::Error::<Test>::Forbidden);
    });
}

#[test]
fn farmer_creation_passes() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[4; 64]);
        let incenitive = Incentive::new(XOR, 20_000_u128.into());
        let parameters = Parameters::new(
            DateTimePeriod::new(0, <pallet_timestamp::Module<Test>>::get() + 10_000),
            incenitive,
        );
        Assets::register(Origin::signed(ALICE), XOR, AssetSymbol(b"XOR".to_vec()), 18);
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &ALICE,
            100_000_000_u128.into()
        ));
        assert_ok!(FarmsModule::create(
            Origin::signed(ALICE),
            farm_name,
            parameters
        ));
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, 100_000_000_u128.into()));
        assert_ok!(FarmsModule::invest(
            Origin::signed(BOB),
            farm_name,
            10_000_u128.into()
        ));
    });
}

#[test]
fn farmer_creation_fails_with_forbidden_error() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[5; 64]);
        let incenitive = Incentive::new(XOR, 20_000_u128.into());
        let parameters = Parameters::new(DateTimePeriod::new(0, 1), incenitive);
        Assets::register(Origin::signed(ALICE), XOR, AssetSymbol(b"XOR".to_vec()), 18);
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &ALICE,
            100_000_000_u128.into()
        ));
        assert_ok!(FarmsModule::create(
            Origin::signed(ALICE),
            farm_name,
            parameters
        ));
        assert_noop!(
            FarmsModule::invest(Origin::signed(ALICE), farm_name, 1_000_u128.into()),
            permissions::Error::<Test>::Forbidden
        );
    });
}

#[test]
fn farmer_creation_fails_with_farm_already_closed() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[4; 64]);
        let incenitive = Incentive::new(XOR, 20_000_u128.into());
        let parameters = Parameters::new(DateTimePeriod::new(1, 2), incenitive);
        Assets::register(Origin::signed(ALICE), XOR, AssetSymbol(b"XOR".to_vec()), 18);
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &ALICE,
            100_000_000_u128.into()
        ));
        assert_ok!(FarmsModule::create(
            Origin::signed(ALICE),
            farm_name,
            parameters
        ));
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, 100_000_000_u128.into()));
        assert_noop!(
            FarmsModule::invest(Origin::signed(BOB), farm_name, 10_000_u128.into()),
            crate::Error::<Test>::FarmAlreadyClosed
        );
    });
}

#[test]
fn farmer_claims_passes() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[6; 64]);
        let incenitive = Incentive::new(XOR, 20_000_u128.into());
        let parameters = Parameters::new(DateTimePeriod::new(0, 1), incenitive);
        Assets::register(Origin::signed(ALICE), XOR, AssetSymbol(b"XOR".to_vec()), 18);
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &ALICE,
            100_000_000_u128.into()
        ));
        assert_ok!(FarmsModule::create(
            Origin::signed(ALICE),
            farm_name,
            parameters
        ));
        assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, 100_000_000_u128.into()));
        assert_ok!(FarmsModule::invest(
            Origin::signed(BOB),
            farm_name,
            10_000_u128.into()
        ));
        assert_ok!(FarmsModule::claim(
            Origin::signed(BOB),
            farm_name,
            1_000_u128.into()
        ));
    });
}

#[test]
fn farmer_claims_fails_with_forbidden_error() {
    let mut ext = new_test_ext();
    ext.execute_with(|| {
        let farm_name = H512::from_slice(&[7; 64]);
        let incenitive = Incentive::new(XOR, 20_000_u128.into());
        let parameters = Parameters::new(DateTimePeriod::new(0, 1), incenitive);
        Assets::register(Origin::signed(ALICE), XOR, AssetSymbol(b"XOR".to_vec()), 18);
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &ALICE,
            100_000_000_u128.into()
        ));
        assert_ok!(Assets::mint_to(
            &XOR,
            &ALICE,
            &NICK,
            100_000_000_u128.into()
        ));
        assert_ok!(FarmsModule::create(
            Origin::signed(ALICE),
            farm_name,
            parameters
        ));
        assert_ok!(FarmsModule::invest(
            Origin::signed(NICK),
            farm_name,
            10_000_u128.into()
        ));
        assert_noop!(
            FarmsModule::claim(Origin::signed(NICK), farm_name, 1_000_u128.into()),
            permissions::Error::<Test>::Forbidden
        );
    });
}
