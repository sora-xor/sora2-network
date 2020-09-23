use crate::mock::*;
use common::Fixed;

#[test]
fn test_exchange_tokens_1_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // reserves are setup in genesis (found in mock.rs)
        let result = LiquidityProxy::demo_function(&GetBaseAssetId::get(), &DOT, Fixed::from(100))
            .expect("Failed to call demo function");
        // two dexes have liquidity source with this path, their prices are:
        assert_eq!(
            &result,
            &[
                Fixed::from_inner(136_851187324744592819),
                Fixed::from_inner(22_466199298948422634)
            ]
        );
        // only one dex for this path:
        let result = LiquidityProxy::demo_function(&GetBaseAssetId::get(), &KSM, Fixed::from(100))
            .expect("Failed to call demo function");
        assert_eq!(&result, &[Fixed::from_inner(71_218100969694805079)]);
        let result = LiquidityProxy::demo_function(&DOT, &KSM, Fixed::from(100))
            .expect("Failed to call demo function");
        assert_eq!(&result, &[Fixed::from_inner(50_269749254965695317)]);

        // alternatively use directly:
        <mock_liquidity_source::Module<Runtime>>::set_reserve(
            Origin::signed(ALICE),
            DEX_A_ID,
            KSM,
            Fixed::from(333),
            Fixed::from(555),
        )
        .expect("Failed to set reserve");
        let result = LiquidityProxy::demo_function(&GetBaseAssetId::get(), &KSM, Fixed::from(100))
            .expect("Failed to call demo function");
        assert_eq!(&result, &[Fixed::from_inner(127_879593251675525768)]);
    });
}
