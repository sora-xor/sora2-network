use super::{alice, QaToolsPallet};
use common::prelude::{err_pays_no, QuoteAmount};
use common::{
    assert_approx_eq, balance, DEXId, LiquiditySource, DAI, ETH, PSWAP, TBCD, VAL, XOR, XST, XSTUSD,
};
use frame_support::assert_ok;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::pallet_tools;

use pallet_tools::liquidity_proxy::liquidity_sources::initialize_xyk;
use pallet_tools::pool_xyk::AssetPairInput;

#[test]
fn should_xyk_initialize_pool() {
    ext().execute_with(|| {
        let pairs = vec![
            AssetPairInput::new(DEXId::Polkaswap.into(), XOR, VAL, balance!(0.5)),
            AssetPairInput::new(DEXId::Polkaswap.into(), XOR, ETH, balance!(0.1)),
            AssetPairInput::new(DEXId::Polkaswap.into(), XOR, PSWAP, balance!(1)),
            AssetPairInput::new(DEXId::Polkaswap.into(), XOR, DAI, balance!(10)),
            AssetPairInput::new(DEXId::Polkaswap.into(), XOR, XST, balance!(0.5)),
            AssetPairInput::new(DEXId::Polkaswap.into(), XOR, TBCD, balance!(0.5)),
            AssetPairInput::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, VAL, balance!(0.5)),
            AssetPairInput::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, PSWAP, balance!(0.5)),
            AssetPairInput::new(
                DEXId::PolkaswapXSTUSD.into(),
                XSTUSD,
                ETH,
                balance!(0.000000000000000001),
            ),
            AssetPairInput::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, DAI, balance!(0.5)),
        ];
        let prices = initialize_xyk::<Runtime>(alice(), pairs.clone()).unwrap();

        for (expected_pair, actual_pair) in pairs.into_iter().zip(prices.into_iter()) {
            let result = pool_xyk::Pallet::<Runtime>::quote_without_impact(
                &expected_pair.dex_id,
                &expected_pair.asset_a,
                &expected_pair.asset_b,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(1),
                },
                false,
            )
            .unwrap();
            // `deduce_fee` was set to false
            assert_eq!(result.fee, 0);
            let price = result.amount;
            assert_eq!(actual_pair.price, price);
            assert_approx_eq!(actual_pair.price, expected_pair.price, 10, 0);
        }
    })
}

#[test]
fn should_not_initialize_existing_xyk_pool() {
    ext().execute_with(|| {
        assert_ok!(QaToolsPallet::xyk_initialize(
            RuntimeOrigin::root(),
            alice(),
            vec![
                AssetPairInput::new(DEXId::Polkaswap.into(), XOR, VAL, balance!(0.5)),
                AssetPairInput::new(DEXId::PolkaswapXSTUSD.into(), XSTUSD, VAL, balance!(0.5))
            ],
        ));
        assert_eq!(
            QaToolsPallet::xyk_initialize(
                RuntimeOrigin::root(),
                alice(),
                vec![AssetPairInput::new(
                    DEXId::Polkaswap.into(),
                    XOR,
                    VAL,
                    balance!(0.5)
                ),],
            ),
            Err(err_pays_no(
                pool_xyk::Error::<Runtime>::PoolIsAlreadyInitialized
            ))
        );
    })
}
