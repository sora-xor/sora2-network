mod tests {
    use crate::{mock::*, Error};
    use common::{
        prelude::{AssetSymbol, Balance},
        AssetId32, DOT, VAL, XOR,
    };
    use frame_support::{assert_noop, assert_ok};
    use hex_literal::hex;
    use sp_runtime::traits::Zero;

    #[test]
    fn should_gen_and_register_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let next_asset_id = Assets::gen_asset_id(&ALICE);
            assert_eq!(
                next_asset_id,
                AssetId32::from_bytes(hex!(
                    "97770dfe3392f9bb8ab977ce23d11c92e25140c39a9d8115714168d6e484ea41"
                ))
            );
            assert!(Assets::ensure_asset_exists(&next_asset_id).is_err());
            assert_ok!(Assets::register(
                Origin::signed(ALICE),
                AssetSymbol(b"ALIC".to_vec()),
                18,
                Balance::zero(),
                true,
            ));
            assert_ok!(Assets::ensure_asset_exists(&next_asset_id));
            assert_ne!(Assets::gen_asset_id(&ALICE), next_asset_id);
        });
    }

    #[test]
    fn should_register_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert!(Assets::ensure_asset_exists(&XOR).is_err());
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::zero(),
                true,
            ));
            assert_ok!(Assets::ensure_asset_exists(&XOR));
        });
    }

    #[test]
    fn should_not_register_duplicated_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::zero(),
                true,
            ));
            assert_noop!(
                Assets::register_asset_id(
                    ALICE,
                    XOR,
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                    Balance::zero(),
                    true,
                ),
                Error::<Runtime>::AssetIdAlreadyExists
            );
        });
    }

    #[test]
    fn should_allow_operation() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::zero(),
                true,
            ));
            assert_ok!(Assets::mint_to(&XOR, &ALICE, &ALICE, 100u32.into()));
            assert_ok!(Assets::burn_from(&XOR, &ALICE, &ALICE, 100u32.into()));
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 100.into()));
        });
    }

    #[test]
    fn should_not_allow_operation() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::zero(),
                true,
            ));
            assert_noop!(
                Assets::mint_to(&XOR, &BOB, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
            assert_noop!(
                Assets::burn_from(&XOR, &BOB, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
            assert_noop!(
                Assets::update_balance(&XOR, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::Forbidden
            );
        });
    }

    #[test]
    fn should_check_symbols_correctly() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert!(crate::is_symbol_valid(&AssetSymbol(b"XOR".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"DOT".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"KSM".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"USDT".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"VAL".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"PSWAP".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"GT".to_vec())));
            assert!(crate::is_symbol_valid(&AssetSymbol(b"BP".to_vec())));
            assert!(!crate::is_symbol_valid(&AssetSymbol(b"ABCDEFGH".to_vec())));
            assert!(!crate::is_symbol_valid(&AssetSymbol(b"AB1".to_vec())));
            assert!(!crate::is_symbol_valid(&AssetSymbol(
                b"\xF0\x9F\x98\xBF".to_vec()
            )));
        })
    }

    #[test]
    fn should_mint_initial_supply_for_owner() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::from(123u32),
                true,
            ));
            assert_eq!(
                Assets::free_balance(&XOR, &ALICE).expect("Failed to query free balance."),
                Balance::from(123u32),
            );
            assert_ok!(Assets::register_asset_id(
                ALICE,
                VAL,
                AssetSymbol(b"VAL".to_vec()),
                18,
                Balance::from(321u32),
                false,
            ));
            assert_eq!(
                Assets::free_balance(&VAL, &ALICE).expect("Failed to query free balance."),
                Balance::from(321u32),
            );
            assert_ok!(Assets::register_asset_id(
                ALICE,
                DOT,
                AssetSymbol(b"DOT".to_vec()),
                18,
                Balance::from(0u32),
                false,
            ));
            assert_eq!(
                Assets::free_balance(&DOT, &ALICE).expect("Failed to query free balance."),
                Balance::zero(),
            );
        })
    }

    #[test]
    fn should_fail_with_non_mintable_asset_supply() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::from(10u32),
                false,
            ));
            assert_noop!(
                Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_noop!(
                Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_noop!(
                Assets::update_balance(&XOR, &ALICE, 1i128),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 0i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, -1i128),);
        })
    }

    #[test]
    fn should_mint_for_mintable_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::from(10u32),
                true,
            ));
            assert_ok!(Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),);
            assert_ok!(Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 1i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 0i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, -1i128),);

            assert_noop!(
                Assets::set_non_mintable_from(&XOR, &BOB),
                Error::<Runtime>::InvalidAssetOwner
            );
            assert_ok!(Assets::set_non_mintable_from(&XOR, &ALICE));

            assert_noop!(
                Assets::mint_to(&XOR, &ALICE, &ALICE, Balance::from(10u32)),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_noop!(
                Assets::mint_to(&XOR, &ALICE, &BOB, Balance::from(10u32)),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_noop!(
                Assets::update_balance(&XOR, &ALICE, 1i128),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 0i128),);
            assert_ok!(Assets::update_balance(&XOR, &ALICE, -1i128),);
        })
    }

    #[test]
    fn should_not_allow_duplicate_set_non_mintable() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register_asset_id(
                ALICE,
                XOR,
                AssetSymbol(b"XOR".to_vec()),
                18,
                Balance::from(10u32),
                true,
            ));
            assert_ok!(Assets::set_non_mintable_from(&XOR, &ALICE));
            assert_noop!(
                Assets::set_non_mintable_from(&XOR, &ALICE),
                Error::<Runtime>::AssetSupplyIsNotMintable
            );
        })
    }
}
