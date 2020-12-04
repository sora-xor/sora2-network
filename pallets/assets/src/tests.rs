mod tests {
    use crate::{mock::*, Error};
    use common::{prelude::AssetSymbol, AssetId32, XOR};
    use frame_support::{assert_noop, assert_ok};
    use hex_literal::hex;

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
                18
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
                18
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
                18
            ));
            assert_noop!(
                Assets::register_asset_id(ALICE, XOR, AssetSymbol(b"XOR".to_vec()), 18),
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
                18
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
                18
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
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"XOR".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"DOT".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"KSM".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"USD".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"VAL".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"PSWAP".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"GT".to_vec())));
            assert!(Assets::is_symbol_valid(&AssetSymbol(b"BP".to_vec())));
            assert!(!Assets::is_symbol_valid(&AssetSymbol(b"ABCDEFGH".to_vec())));
            assert!(!Assets::is_symbol_valid(&AssetSymbol(b"AB1".to_vec())));
            assert!(!Assets::is_symbol_valid(&AssetSymbol(
                b"\xF0\x9F\x98\xBF".to_vec()
            )));
        })
    }
}
