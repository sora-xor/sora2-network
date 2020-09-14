use crate::{mock::*, Error};
use common::DEXInfo;
use frame_support::{assert_noop, assert_ok};

#[test]
fn test_initialize_dex_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            None,
            None
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                owner_account_id: ALICE,
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
            Some(77),
            Some(88)
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                owner_account_id: ALICE,
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
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, Some(77), None)
            .expect("Failed to initialize DEX.");
        DEXModule::set_fee(Origin::signed(ALICE), DEX_A_ID, 100).expect("Failed to set fee.");
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                owner_account_id: ALICE,
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
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, None, Some(88))
            .expect("Failed to initialize DEX.");
        DEXModule::set_protocol_fee(Origin::signed(ALICE), DEX_A_ID, 100)
            .expect("Failed to set protocol fee.");
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            DEXInfo {
                owner_account_id: ALICE,
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
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, Some(77), None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(BOB), DEX_A_ID, 100);
        assert_noop!(result, <Error<Runtime>>::WrongOwnerAccountId);
    })
}

#[test]
fn test_set_fee_should_fail_with_invalid_fee_value() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, Some(77), None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(ALICE), DEX_A_ID, 10001);
        assert_noop!(result, <Error<Runtime>>::InvalidFeeValue);
    })
}

#[test]
fn test_set_fee_should_fail_with_nonexistent_dex() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, Some(77), None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(ALICE), DEX_B_ID, 100);
        assert_noop!(result, <Error<Runtime>>::DEXDoesNotExist);
    })
}

#[test]
fn test_transfer_ownership_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, None, None)
            .expect("Failed to initialize DEX.");
        DEXModule::transfer_ownership(Origin::signed(ALICE), DEX_A_ID, BOB)
            .expect("Failed to trasfer DEX ownership.");
        assert_eq!(DEXModule::dex_id(DEX_A_ID).owner_account_id, BOB);
    })
}

// FIXME: account validity check does not work
#[test]
#[ignore]
fn test_transfer_ownership_should_fail_with_invalid_account() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(Origin::signed(ALICE), DEX_A_ID, XOR, None, None)
            .expect("Failed to initialize DEX.");
        let result = DEXModule::transfer_ownership(Origin::signed(ALICE), DEX_A_ID, 77);
        assert_noop!(result, <Error<Runtime>>::InvalidAccountId);
    })
}
