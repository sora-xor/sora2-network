use crate::mock::*;
use common::fixed;

#[test]
fn test_exchange_tokens_1_should_pass() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        // reserves are setup in genesis (found in mock.rs)
        let result =
            LiquidityProxy::demo_function(DEX_A_ID, &GetBaseAssetId::get(), &DOT, fixed!(100))
                .expect("Failed to call demo function");
        // two dexes have liquidity source with this path, their prices are:
        assert_eq!(
            &result,
            &[
                fixed!(137, 254901960784313725),
                fixed!(98, 263905965671568386),
                fixed!(70, 283669962534155892),
                fixed!(49, 236391471289060089)
            ]
        );
        // only one dex for this path:
        let result =
            LiquidityProxy::demo_function(DEX_A_ID, &GetBaseAssetId::get(), &KSM, fixed!(100))
                .expect("Failed to call demo function");
        assert_eq!(
            &result,
            &[
                fixed!(71, 428571428571428571),
                fixed!(45, 409778936044485523),
                fixed!(26, 263849048659175241),
                fixed!(11, 593427677709687547)
            ]
        );
        let result = LiquidityProxy::demo_function(DEX_A_ID, &DOT, &KSM, fixed!(100))
            .expect("Failed to call demo function");
        assert_eq!(
            &result,
            &[
                fixed!(50, 568900126422250316),
                fixed!(44, 632430612106240006),
                fixed!(35, 802458193480328708),
                fixed!(22, 308951538371195104)
            ]
        );

        // alternatively use directly:
        <mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>>::set_reserve(
            Origin::signed(alice()),
            DEX_A_ID,
            KSM,
            fixed!(333),
            fixed!(555),
        )
        .expect("Failed to set reserve");
        let result =
            LiquidityProxy::demo_function(DEX_A_ID, &GetBaseAssetId::get(), &KSM, fixed!(100))
                .expect("Failed to call demo function");
        assert_eq!(
            &result,
            &[
                fixed!(128, 175519630484988453),
                fixed!(45, 409778936044485523),
                fixed!(26, 263849048659175241),
                fixed!(11, 593427677709687547)
            ]
        );
    });
}
