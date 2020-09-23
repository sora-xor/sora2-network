use crate::mock::*;
use common::{LiquidityRegistry, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType};

#[test]
fn test_filter_empty_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(&XOR, &DOT, LiquiditySourceFilter::empty())
            .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool)
            ]
        );
    })
}

#[test]
fn test_filter_ignore_exact_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_ignored(&[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool,
            )]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_B_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_ignored(&[LiquiditySourceId::new(
                DEX_B_ID,
                LiquiditySourceType::MockPool,
            )]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_ignored(&[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool),
            ]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(&list, &[]);
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_ignored(&[]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool)
            ]
        );
    })
}

#[test]
fn test_filter_allow_exact_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_allowed(&[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool,
            )]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_allowed(&[LiquiditySourceId::new(
                DEX_B_ID,
                LiquiditySourceType::MockPool,
            )]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_B_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_allowed(&[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool),
            ]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool)
            ]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_concrete_allowed(&[]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(&list, &[]);
    })
}

#[test]
fn test_filter_ignore_by_dexid_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_ignored_dex_ids(&[DEX_A_ID]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_B_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_ignored_dex_ids(&[DEX_B_ID]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_ignored_dex_ids(&[DEX_A_ID, DEX_B_ID]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(&list, &[]);
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_ignored_dex_ids(&[]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool)
            ]
        );
    })
}

#[test]
fn test_filter_allow_by_dexid_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_allowed_dex_ids(&[DEX_A_ID]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_A_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_allowed_dex_ids(&[DEX_B_ID]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[LiquiditySourceId::new(
                DEX_B_ID,
                LiquiditySourceType::MockPool
            )]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_allowed_dex_ids(&[DEX_A_ID, DEX_B_ID]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(
            &list,
            &[
                LiquiditySourceId::new(DEX_A_ID, LiquiditySourceType::MockPool),
                LiquiditySourceId::new(DEX_B_ID, LiquiditySourceType::MockPool)
            ]
        );
        let list = DEXAPI::list_liquidity_sources(
            &XOR,
            &DOT,
            LiquiditySourceFilter::with_allowed_dex_ids(&[]),
        )
        .expect("Failed to list available sources.");
        assert_eq!(&list, &[]);
    })
}

#[test]
#[ignore]
fn test_filter_ignore_by_index_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // TODO: add test when more liquidity sources are available
    })
}

#[test]
#[ignore]
fn test_filter_allow_by_index_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // TODO: add test when more liquidity sources are available
    })
}
