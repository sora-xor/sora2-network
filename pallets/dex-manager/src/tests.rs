use crate::{mock::*, Error};
use common::prelude::DEXInfo;
use common::{EnsureDEXOwner, XOR};
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
            None,
            true,
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                default_fee: 30,
                default_protocol_fee: 0,
                is_public: true,
            })
        )
    })
}

#[test]
fn test_initialize_dex_with_fees_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            Some(88),
            false,
        ));
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                default_fee: 77,
                default_protocol_fee: 88,
                is_public: false,
            })
        )
    })
}

#[test]
fn test_set_fee_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            false,
        )
        .expect("Failed to initialize DEX.");
        DEXModule::set_fee(Origin::signed(ALICE), DEX_A_ID, 100).expect("Failed to set fee.");
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                default_fee: 100,
                default_protocol_fee: 0,
                is_public: false,
            })
        );
    })
}

#[test]
fn test_set_protocol_fee_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            None,
            Some(88),
            false,
        )
        .expect("Failed to initialize DEX.");
        DEXModule::set_protocol_fee(Origin::signed(ALICE), DEX_A_ID, 100)
            .expect("Failed to set protocol fee.");
        assert_eq!(
            DEXModule::dex_id(DEX_A_ID),
            Some(DEXInfo {
                base_asset_id: XOR,
                default_fee: 30,
                default_protocol_fee: 100,
                is_public: false,
            })
        );
    })
}

#[test]
fn test_set_fee_on_private_dex_should_fail_with_wrong_owner_account() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            false,
        )
        .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(BOB), DEX_A_ID, 100);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXModule::set_protocol_fee(Origin::signed(BOB), DEX_A_ID, 100);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
    })
}

#[test]
fn test_set_fee_on_public_dex_should_fail_with_wrong_owner_account() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            true,
        )
        .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(BOB), DEX_A_ID, 150);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXModule::set_protocol_fee(Origin::signed(BOB), DEX_A_ID, 150);
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
    })
}

#[test]
fn test_set_fee_should_fail_with_invalid_fee_value() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            false,
        )
        .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(ALICE), DEX_A_ID, 10001);
        assert_noop!(result, Error::<Runtime>::InvalidFeeValue);
    })
}

#[test]
fn test_set_fee_should_fail_with_nonexistent_dex() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            false,
        )
        .expect("Failed to initialize DEX.");
        let result = DEXModule::set_fee(Origin::signed(ALICE), DEX_B_ID, 100);
        assert_noop!(result, Error::<Runtime>::DEXDoesNotExist);
    })
}

#[test]
fn test_can_manage_on_private_dex_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            false,
        )
        .expect("Failed to initialize DEX.");
        let result = DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE));
        assert_ok!(result);
        let result = DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB));
        assert_noop!(result, permissions::Error::<Runtime>::Forbidden);
        let result = DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root());
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
    })
}

#[test]
fn test_can_manage_on_public_dex_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        DEXModule::initialize_dex(
            Origin::signed(ALICE),
            DEX_A_ID,
            XOR,
            ALICE,
            Some(77),
            None,
            true,
        )
        .expect("Failed to initialize DEX.");
        let result = DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(ALICE));
        assert_ok!(result);
        let result = DEXModule::ensure_can_manage(&DEX_A_ID, Origin::signed(BOB));
        assert_ok!(result);
        let result = DEXModule::ensure_can_manage(&DEX_A_ID, Origin::root());
        assert_noop!(result, Error::<Runtime>::InvalidAccountId);
    })
}
