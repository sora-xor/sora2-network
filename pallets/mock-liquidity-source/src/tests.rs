use crate::mock::*;
use common::{fixed, prelude::*};

#[test]
fn test_provides_exchange_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(5000),
            Fixed::from(7000),
        )
        .expect("Failed to set reserve.");
        assert!(MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT
        ));
    });
}

#[test]
fn test_doesnt_provide_exchange_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT
        ));
        // check again, so they are not created via get()'s
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT
        ));
    });
}

#[test]
fn test_support_multiple_dexes_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(1000),
            Fixed::from(1000),
        )
        .expect("Failed to set reserve.");
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_B_ID,
            KSM,
            Fixed::from(1000),
            Fixed::from(1000),
        )
        .expect("Failed to set reserve.");
        assert!(MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_A_ID,
            &KSM,
            &GetBaseAssetId::get()
        ));
        assert!(!MockLiquiditySource::can_exchange(
            &DEX_B_ID,
            &DOT,
            &GetBaseAssetId::get()
        ));
        assert!(MockLiquiditySource::can_exchange(
            &DEX_B_ID,
            &KSM,
            &GetBaseAssetId::get()
        ));
    });
}

#[test]
fn test_quote_base_to_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(5000),
            Fixed::from(7000),
        )
        .expect("Failed to set reserve.");
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(Fixed::from(100), Fixed::from(0)),
        )
        .unwrap();
        assert_eq!(outcome.amount, Fixed::from_inner(136_851187324744592819));
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_output(
                Fixed::from_inner(136_851187324744592819),
                Fixed::from(100),
            ),
        )
        .unwrap();
        assert_eq!(outcome.amount, Fixed::from(100));
    });
}

#[test]
fn test_quote_target_to_base_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(5000),
            Fixed::from(7000),
        )
        .expect("Failed to set reserve.");
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_input(Fixed::from(100), Fixed::from(0)),
        )
        .unwrap();
        assert_eq!(outcome.amount, Fixed::from_inner(70_211267605633802818));
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &DOT,
            &GetBaseAssetId::get(),
            SwapAmount::with_desired_output(
                Fixed::from_inner(70_211267605633802818),
                Fixed::from(100),
            ),
        )
        .unwrap();
        assert_eq!(outcome.amount, Fixed::from(100));
    });
}

#[test]
fn test_quote_target_to_target_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(5000),
            Fixed::from(7000),
        )
        .expect("Failed to set reserve.");
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            KSM,
            Fixed::from(5500),
            Fixed::from(3000),
        )
        .expect("Failed to set reserve.");
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &KSM,
            &DOT,
            SwapAmount::with_desired_input(Fixed::from(100), Fixed::from(0)),
        )
        .unwrap();
        assert_eq!(outcome.amount, Fixed::from_inner(238_487257161165663484));
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &KSM,
            &DOT,
            SwapAmount::with_desired_output(
                Fixed::from_inner(238_487257161165663484),
                Fixed::from(100),
            ),
        )
        .unwrap();
        assert_eq!(outcome.amount, Fixed::from(100));
    });
}

#[test]
fn test_quote_different_modules_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        MockLiquiditySource::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(5000),
            Fixed::from(7000),
        )
        .expect("Failed to set reserve.");
        MockLiquiditySource2::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            DOT,
            Fixed::from(5500),
            Fixed::from(3000),
        )
        .expect("Failed to set reserve.");
        let outcome = MockLiquiditySource::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(100), fixed!(100)),
        )
        .unwrap();
        assert_eq!(outcome.amount, fixed!(136, 851187324744592819));
        let outcome = MockLiquiditySource2::quote(
            &DEX_A_ID,
            &GetBaseAssetId::get(),
            &DOT,
            SwapAmount::with_desired_input(fixed!(100), fixed!(100)),
        )
        .unwrap();
        assert_eq!(outcome.amount, fixed!(53, 413575727271103809));
    });
}
