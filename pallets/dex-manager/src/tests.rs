use crate::{mock::*, Error};
use common::prelude::DEXInfo;
use common::XOR;
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
            None,
            None
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                base_asset_id: XOR,
                default_fee: 30,
                default_protocol_fee: 0
            }
        )
    })
}

#[test]
fn test_initialize_dex_with_fee_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            Some(88)
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                base_asset_id: XOR,
                default_fee: 77,
                default_protocol_fee: 88
            }
        )
    })
}

#[test]
fn test_set_fee_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, Some(77), None)
            .expect("Failed to initialize DEX.");
        DEXModule::set_fee(Origin::signed(ALICE), DEX_A_ID, 100).expect("Failed to set fee.");
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                base_asset_id: XOR,
                default_fee: 100,
                default_protocol_fee: 0
            }
        )
    })
}

#[test]
fn test_set_protocol_fee_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, None, Some(88))
            .expect("Failed to initialize DEX.");
        DEXModule::set_protocol_fee(Origin::signed(ALICE), DEX_A_ID, 100)
            .expect("Failed to set protocol fee.");
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                base_asset_id: XOR,
                default_fee: 30,
                default_protocol_fee: 100
            }
        )
    })
}

#[test]
fn test_set_fee_should_fail_with_wrong_owner_account() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, Some(77), None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(BOB), DEX_A_ID, 100);
        // TODO: check error more precisely
        assert!(result.is_err());
    })
}

#[test]
fn test_set_fee_should_fail_with_invalid_fee_value() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, Some(77), None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(ALICE), DEX_A_ID, 10001);
        assert_noop!(result, <Error<Runtime>>::InvalidFeeValue);
    })
}

#[test]
fn test_set_fee_should_fail_with_nonexistent_dex() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, ALICE, Some(77), None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(ALICE), DEX_B_ID, 100);
        assert_noop!(result, <Error<Runtime>>::DEXDoesNotExist);
    })
}
