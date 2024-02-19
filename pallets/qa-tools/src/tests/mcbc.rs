use super::alice;
use super::QaToolsPallet;
use assets::AssetIdOf;
use common::prelude::QuoteAmount;
use common::{balance, DEXId, LiquiditySource, PriceVariant, VAL, XOR};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::pallet_tools::liquidity_proxy::liquidity_sources::initialize_mcbc;
use qa_tools::pallet_tools::mcbc as mcbc_tools;
use qa_tools::pallet_tools::price_tools::AssetPrices;

#[test]
fn should_init_mcbc() {
    ext().execute_with(|| {
        // let collateral = VAL.into();
        // let quote_result_before_mint_sell =
        //     multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
        //         &DEXId::Polkaswap.into(),
        //         &XOR.into(),
        //         &VAL.into(),
        //         QuoteAmount::WithDesiredInput {
        //             desired_amount_in: balance!(1),
        //         },
        //         true,
        //     )
        //         .unwrap();
    })
}

#[test]
fn should_init_mcbc_xor() {
    ext().execute_with(|| {
        use common::AssetInfoProvider;

        let collateral = VAL.into();
        let xor_collector = alice();

        let xor_holder = alice();
        let current_base_supply = assets::Pallet::<Runtime>::total_issuance(&XOR.into()).unwrap();
        let xor_holder_initial_balance =
            assets::Pallet::<Runtime>::total_balance(&XOR.into(), &xor_holder).unwrap();
        assert!(
            multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
                &DEXId::Polkaswap.into(),
                &collateral,
                &XOR.into(),
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(1),
                },
                true,
            )
            .is_err()
        );

        let added_supply = balance!(1000000);
        assert_ok!(initialize_mcbc::<Runtime>(
            Some(mcbc_tools::BaseSupply {
                base_supply_collector: xor_collector.clone(),
                new_base_supply: current_base_supply + added_supply,
            }),
            vec![],
            None,
        ));
        assert_eq!(
            xor_holder_initial_balance + added_supply,
            assets::Pallet::<Runtime>::total_balance(&XOR.into(), &xor_holder).unwrap()
        );

        // bring supply back to original
        assert_ok!(initialize_mcbc::<Runtime>(
            Some(mcbc_tools::BaseSupply {
                base_supply_collector: xor_collector.clone(),
                new_base_supply: current_base_supply,
            }),
            vec![],
            None,
        ));
        assert_eq!(
            xor_holder_initial_balance,
            assets::Pallet::<Runtime>::total_balance(&XOR.into(), &xor_holder).unwrap()
        );

        // cannot burn assets not owned by the holder
        assert_err!(
            initialize_mcbc::<Runtime>(
                Some(mcbc_tools::BaseSupply {
                    base_supply_collector: xor_collector,
                    new_base_supply: 0,
                }),
                vec![],
                None,
            ),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
    })
}

#[test]
fn should_init_collateral_reference_price() {
    ext().execute_with(|| {
        let collateral = VAL.into();
        let reference_asset = qa_tools::InputAssetId::<AssetIdOf<Runtime>>::McbcReference;
        let reference_asset_id = reference_asset.clone().resolve::<Runtime>();

        let refence_prices = mcbc_tools::ReferencePriceInput {
            buy: balance!(1),
            sell: balance!(1),
        };
        let input = mcbc_tools::OtherCollateralInput::<AssetIdOf<Runtime>> {
            asset: collateral,
            ref_prices: Some(refence_prices.clone()),
            reserves: None,
        };
        assert_err!(
            initialize_mcbc::<Runtime>(None, vec![input.clone()], None),
            qa_tools::Error::<Runtime>::ReferenceAssetPriceNotFound
        );

        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
            reference_asset.clone()
        ));

        assert_ok!(initialize_mcbc::<Runtime>(None, vec![input], None));
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &collateral,
                &reference_asset_id,
                PriceVariant::Buy
            ),
            Ok(refence_prices.buy)
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &collateral,
                &reference_asset_id,
                PriceVariant::Sell
            ),
            Ok(refence_prices.sell)
        );
    })
}
