mod tests {
    use crate::{mock::*, Error};
    use common::{DOT, XOR};
    use frame_support::{assert_noop, assert_ok};

    #[test]
    fn should_register_trading_pair() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(TradingPair::register(
                Origin::signed(ALICE),
                DEX_ID,
                XOR,
                DOT
            ));
        });
    }

    #[test]
    fn should_not_register_duplicate_trading_pair() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_ok!(TradingPair::register(
                Origin::signed(ALICE),
                DEX_ID,
                XOR,
                DOT
            ));
            assert_noop!(
                TradingPair::register(Origin::signed(ALICE), DEX_ID, XOR, DOT),
                Error::<Runtime>::TradingPairExists
            );
        });
    }

    #[test]
    fn should_not_register_trading_pair_with_wrong_base_asset() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_noop!(
                TradingPair::register(Origin::signed(ALICE), DEX_ID, DOT, XOR),
                Error::<Runtime>::ForbiddenBaseAssetId
            );
        });
    }

    #[test]
    fn should_not_register_trading_pair_with_same_assets() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_noop!(
                TradingPair::register(Origin::signed(ALICE), DEX_ID, XOR, XOR),
                Error::<Runtime>::IdenticalAssetIds
            );
        });
    }
}
