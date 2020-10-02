mod tests {
    use crate::{mock::*, Error};
    use frame_support::{assert_noop, assert_ok};

    #[test]
    fn should_register_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert!(Assets::ensure_asset_exists(&XOR).is_err());
            assert_ok!(Assets::register(Origin::signed(ALICE), XOR));
            assert_ok!(Assets::ensure_asset_exists(&XOR));
        });
    }

    #[test]
    fn should_not_register_duplicated_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register(Origin::signed(ALICE), XOR));
            assert_noop!(
                Assets::register(Origin::signed(ALICE), XOR),
                Error::<Runtime>::AssetIdAlreadyExists
            );
        });
    }

    #[test]
    fn should_allow_operation() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register(Origin::signed(ALICE), XOR));
            assert_ok!(Assets::mint(&XOR, &ALICE, &ALICE, 100u32.into()));
            assert_ok!(Assets::burn(&XOR, &ALICE, &ALICE, 100u32.into()));
            assert_ok!(Assets::update_balance(&XOR, &ALICE, 100.into()));
        });
    }

    #[test]
    fn should_not_allow_operation() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(Assets::register(Origin::signed(ALICE), XOR));
            assert_noop!(
                Assets::mint(&XOR, &BOB, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::PermissionNotFound
            );
            assert_noop!(
                Assets::burn(&XOR, &BOB, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::PermissionNotFound
            );
            assert_noop!(
                Assets::update_balance(&XOR, &BOB, 100u32.into()),
                permissions::Error::<Runtime>::PermissionNotFound
            );
        });
    }
}
