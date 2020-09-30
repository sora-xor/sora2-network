use crate::mock::*;
use common::{LiquidityRegistry, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType};

#[test]
fn test_filter_empty_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list =
            DEXAPI::list_liquidity_sources(&XOR, &DOT, LiquiditySourceFilter::empty(DEX_A_ID))
                .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            ),]
        );
    })
}

#[test]
fn test_filter_with_ignored_existing_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_ignored(DEX_A_ID, &[LiquiditySourceType::MockPool]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(&list, &[]);
    })
}

#[test]
fn test_filter_with_ignored_other_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_ignored(DEX_A_ID, &[LiquiditySourceType::XYKPool]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            ),]
        );
    })
}

#[test]
fn test_filter_with_allowed_existing_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_allowed(DEX_A_ID, &[LiquiditySourceType::MockPool]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            ),]
        );
    })
}

#[test]
fn test_filter_with_allowed_other_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_allowed(DEX_A_ID, &[LiquiditySourceType::XYKPool]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(&list, &[]);
    })
}
