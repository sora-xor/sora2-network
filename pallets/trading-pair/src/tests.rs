use crate::{mock::*, Error, Module};
use common::{EnsureTradingPairExists, LiquiditySourceType, TradingPair, DOT, KSM, XOR};
use frame_support::{assert_noop, assert_ok};

type TradingPairModule = Module<Runtime>;

#[test]
fn should_register_trading_pair() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(TradingPairModule::register(
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
        assert_ok!(TradingPairModule::register(
            Origin::signed(ALICE),
            DEX_ID,
            XOR,
            DOT
        ));
        assert_noop!(
            TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, DOT),
            Error::<Runtime>::TradingPairExists
        );
    });
}

#[test]
fn should_not_register_trading_pair_with_wrong_base_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            TradingPairModule::register(Origin::signed(ALICE), DEX_ID, DOT, XOR),
            Error::<Runtime>::ForbiddenBaseAssetId
        );
    });
}

#[test]
fn should_not_register_trading_pair_with_same_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, XOR),
            Error::<Runtime>::IdenticalAssetIds
        );
    });
}

#[test]
fn should_list_registered_pairs() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(
            TradingPairModule::list_trading_pairs(&DEX_ID).expect("Failed to list trading pairs."),
            vec![]
        );
        assert!(
            !TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &DOT)
                .expect("Failed to query pair state.")
        );
        assert!(
            !TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert_noop!(
            TradingPairModule::ensure_trading_pair_exists(&DEX_ID, &XOR, &DOT),
            Error::<Runtime>::TradingPairDoesntExist
        );
        assert_noop!(
            TradingPairModule::ensure_trading_pair_exists(&DEX_ID, &XOR, &KSM),
            Error::<Runtime>::TradingPairDoesntExist
        );

        TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_eq!(
            TradingPairModule::list_trading_pairs(&DEX_ID).expect("Failed to list trading pairs."),
            vec![TradingPair {
                base_asset_id: XOR,
                target_asset_id: DOT
            }]
        );
        assert!(
            TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &DOT)
                .expect("Failed to query pair state.")
        );
        assert!(
            !TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert_ok!(TradingPairModule::ensure_trading_pair_exists(
            &DEX_ID, &XOR, &DOT
        ));
        assert_noop!(
            TradingPairModule::ensure_trading_pair_exists(&DEX_ID, &XOR, &KSM),
            Error::<Runtime>::TradingPairDoesntExist
        );

        TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, KSM)
            .expect("Failed to register pair.");
        assert_eq!(
            TradingPairModule::list_trading_pairs(&DEX_ID).expect("Failed to list trading pairs."),
            vec![
                TradingPair {
                    base_asset_id: XOR,
                    target_asset_id: KSM
                },
                TradingPair {
                    base_asset_id: XOR,
                    target_asset_id: DOT
                },
            ]
        );
        assert!(
            TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert!(
            TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert_ok!(TradingPairModule::ensure_trading_pair_exists(
            &DEX_ID, &XOR, &DOT
        ));
        assert_ok!(TradingPairModule::ensure_trading_pair_exists(
            &DEX_ID, &XOR, &KSM
        ));
    });
}

#[test]
fn should_enable_sources_for_pair_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, KSM)
            .expect("Failed to register pair.");
        // check initial states after trading pair registration
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );

        // pre check for enabled sources
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));

        // enable source on one pair and check both trading pairs
        TradingPairModule::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::BondingCurvePool]
        );
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert!(TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));

        // enable source for another pair
        TradingPairModule::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XYKPool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                LiquiditySourceType::XYKPool,
                LiquiditySourceType::BondingCurvePool
            ]
        );
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert!(TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));

        // enable another source for first trading pair
        TradingPairModule::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::BondingCurvePool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                LiquiditySourceType::XYKPool,
                LiquiditySourceType::BondingCurvePool
            ]
        );
        assert_eq!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::BondingCurvePool]
        );
        assert!(TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::BondingCurvePool
        )
        .expect("Failed to query pair state."));
        assert!(TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairModule::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XYKPool
        )
        .expect("Failed to query pair state."));
    });
}

#[test]
fn duplicate_enabled_source_should_not_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_ok!(TradingPairModule::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool,
        ));
        assert_ok!(TradingPairModule::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::BondingCurvePool,
        ));
    });
}

#[test]
fn should_not_enable_source_for_unregistered_pair() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_noop!(
            TradingPairModule::enable_source_for_trading_pair(
                &DEX_ID,
                &XOR,
                &KSM,
                LiquiditySourceType::BondingCurvePool,
            ),
            Error::<Runtime>::TradingPairDoesntExist
        );
    });
}

#[test]
fn should_fail_with_nonexistent_dex() {
    let mut ext = ExtBuilder::without_initialized_dex().build();
    ext.execute_with(|| {
        assert_noop!(
            TradingPairModule::register(Origin::signed(ALICE), DEX_ID, XOR, DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairModule::ensure_trading_pair_exists(&DEX_ID, &XOR, &DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairModule::list_trading_pairs(&DEX_ID),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairModule::is_trading_pair_enabled(&DEX_ID, &XOR, &DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairModule::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairModule::is_source_enabled_for_trading_pair(
                &DEX_ID,
                &XOR,
                &DOT,
                LiquiditySourceType::BondingCurvePool
            ),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairModule::enable_source_for_trading_pair(
                &DEX_ID,
                &XOR,
                &DOT,
                LiquiditySourceType::BondingCurvePool
            ),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
    });
}
