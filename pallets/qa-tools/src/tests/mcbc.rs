// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use super::alice;
use super::QaToolsPallet;
use assets::AssetIdOf;
use common::prelude::QuoteAmount;
use common::{
    assert_approx_eq, balance, AssetInfoProvider, Balance, DEXId, LiquiditySource, PriceVariant,
    CERES_ASSET_ID, ETH, TBCD, VAL, XOR,
};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::pallet_tools::liquidity_proxy::liquidity_sources::{
    initialize_mcbc_base_supply, initialize_mcbc_collateral, initialize_mcbc_tbcd_collateral,
};
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
        assert_ok!(initialize_mcbc_base_supply::<Runtime>(
            mcbc_tools::BaseSupply {
                base_supply_collector: xor_collector.clone(),
                new_base_supply: current_base_supply + added_supply,
            }
        ));
        assert_eq!(
            xor_holder_initial_balance + added_supply,
            assets::Pallet::<Runtime>::total_balance(&XOR.into(), &xor_holder).unwrap()
        );

        // bring supply back to original
        assert_ok!(initialize_mcbc_base_supply::<Runtime>(
            mcbc_tools::BaseSupply {
                base_supply_collector: xor_collector.clone(),
                new_base_supply: current_base_supply,
            }
        ));
        assert_eq!(
            xor_holder_initial_balance,
            assets::Pallet::<Runtime>::total_balance(&XOR.into(), &xor_holder).unwrap()
        );

        // cannot burn assets not owned by the holder
        assert_err!(
            initialize_mcbc_base_supply::<Runtime>(mcbc_tools::BaseSupply {
                base_supply_collector: xor_collector,
                new_base_supply: 0,
            }),
            pallet_balances::Error::<Runtime>::InsufficientBalance
        );
    })
}

fn set_and_verify_reference_prices(
    reference_asset_id: &AssetIdOf<Runtime>,
    collateral_asset_id: &AssetIdOf<Runtime>,
    reference_prices: AssetPrices,
) {
    let actual_ref_prices =
        initialize_mcbc_collateral::<Runtime>(mcbc_tools::OtherCollateralInput::<
            AssetIdOf<Runtime>,
        > {
            asset: collateral_asset_id.clone(),
            ref_prices: Some(reference_prices.clone()),
            reserves: None,
        })
        .unwrap()
        .expect("Provided `ref_prices`, should be `Some`");
    assert_approx_eq!(reference_prices.buy, actual_ref_prices.buy, 10, 0.0001f64);
    assert_approx_eq!(reference_prices.sell, actual_ref_prices.sell, 10, 0.0001f64);

    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(
            &collateral_asset_id,
            &reference_asset_id,
            PriceVariant::Buy
        ),
        Ok(actual_ref_prices.buy)
    );
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(
            &collateral_asset_id,
            &reference_asset_id,
            PriceVariant::Sell
        ),
        Ok(actual_ref_prices.sell)
    );
}

fn test_init_single_collateral_reference_price(collateral_asset_id: AssetIdOf<Runtime>) {
    let reference_asset = qa_tools::InputAssetId::<AssetIdOf<Runtime>>::McbcReference;
    let reference_asset_id = reference_asset.clone().resolve::<Runtime>();
    assert_err!(
        initialize_mcbc_collateral::<Runtime>(mcbc_tools::OtherCollateralInput::<
            AssetIdOf<Runtime>,
        > {
            asset: collateral_asset_id,
            ref_prices: Some(AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            }),
            reserves: None,
        }),
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
    set_and_verify_reference_prices(
        &reference_asset_id,
        &collateral_asset_id,
        AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        },
    );
    set_and_verify_reference_prices(
        &reference_asset_id,
        &collateral_asset_id,
        AssetPrices {
            buy: balance!(124),
            sell: balance!(123),
        },
    );
    set_and_verify_reference_prices(
        &reference_asset_id,
        &collateral_asset_id,
        AssetPrices {
            buy: balance!(0.1),
            sell: balance!(0.01),
        },
    );
}

#[test]
fn should_init_collateral_reference_price() {
    ext().execute_with(|| {
        test_init_single_collateral_reference_price(VAL.into());
        test_init_single_collateral_reference_price(ETH.into());
        test_init_single_collateral_reference_price(CERES_ASSET_ID.into());
        // todo: test with newly created assets
    })
}

fn set_and_verify_tbcd_reference_prices(
    reference_asset_id: &AssetIdOf<Runtime>,
    reference_prices: AssetPrices,
) {
    let collateral_asset_id = TBCD.into();

    assert_ok!(initialize_mcbc_tbcd_collateral::<Runtime>(
        mcbc_tools::TbcdCollateralInput {
            ref_prices: Some(AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            }),
            reserves: None,
            xor_ref_prices: None,
        }
    ));
    let actual_ref_prices =
        initialize_mcbc_tbcd_collateral::<Runtime>(mcbc_tools::TbcdCollateralInput {
            ref_prices: Some(reference_prices.clone()),
            reserves: None,
            xor_ref_prices: None,
        })
        .unwrap()
        .expect("Provided `ref_prices`, should be `Some`");
    assert_approx_eq!(reference_prices.buy, actual_ref_prices.buy, 10, 0.0001f64);
    assert_approx_eq!(reference_prices.sell, actual_ref_prices.sell, 10, 0.0001f64);

    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(
            &collateral_asset_id,
            &reference_asset_id,
            PriceVariant::Buy
        ),
        Ok(actual_ref_prices.buy)
    );
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(
            &collateral_asset_id,
            &reference_asset_id,
            PriceVariant::Sell
        ),
        Ok(actual_ref_prices.sell)
    );
}

#[test]
fn should_init_tbcd_reference_price() {
    ext().execute_with(|| {
        let reference_asset = qa_tools::InputAssetId::<AssetIdOf<Runtime>>::McbcReference;
        let reference_asset_id = reference_asset.clone().resolve::<Runtime>();
        assert_err!(
            initialize_mcbc_tbcd_collateral::<Runtime>(mcbc_tools::TbcdCollateralInput {
                ref_prices: Some(AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1),
                }),
                reserves: None,
                xor_ref_prices: None,
            }),
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

        assert_err!(
            initialize_mcbc_collateral::<Runtime>(mcbc_tools::OtherCollateralInput {
                asset: TBCD.into(),
                ref_prices: Some(AssetPrices {
                    buy: balance!(1),
                    sell: balance!(1),
                }),
                reserves: None,
            }),
            qa_tools::Error::<Runtime>::IncorrectCollateralAsset
        );

        set_and_verify_tbcd_reference_prices(
            &reference_asset_id,
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
        );
        set_and_verify_tbcd_reference_prices(
            &reference_asset_id,
            AssetPrices {
                buy: balance!(124),
                sell: balance!(123),
            },
        );
        set_and_verify_tbcd_reference_prices(
            &reference_asset_id,
            AssetPrices {
                buy: balance!(0.1),
                sell: balance!(0.01),
            },
        );
    })
}

fn set_and_verify_reserves(collateral_asset_id: &AssetIdOf<Runtime>, target_reserves: Balance) {
    let input = mcbc_tools::OtherCollateralInput::<AssetIdOf<Runtime>> {
        asset: collateral_asset_id.clone(),
        ref_prices: None,
        reserves: Some(target_reserves),
    };

    assert_ok!(initialize_mcbc_collateral::<Runtime>(input));

    let reserves_tech_account_id =
        multicollateral_bonding_curve_pool::Pallet::<Runtime>::reserves_account_id();
    let reserves_account_id =
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&reserves_tech_account_id)
            .unwrap();
    assert_eq!(
        assets::Pallet::<Runtime>::total_balance(&collateral_asset_id, &reserves_account_id),
        Ok(target_reserves)
    );
}

fn test_init_single_collateral_reserves(collateral_asset_id: AssetIdOf<Runtime>) {
    set_and_verify_reserves(&collateral_asset_id, balance!(1000000));
    set_and_verify_reserves(&collateral_asset_id, balance!(0));
}

#[test]
fn should_init_collateral_reserves() {
    ext().execute_with(|| {
        test_init_single_collateral_reserves(VAL.into());
        test_init_single_collateral_reserves(ETH.into());
        test_init_single_collateral_reserves(CERES_ASSET_ID.into());
        // todo: test with newly created assets
    })
}

fn set_and_verify_tbcd_reserves(
    collateral_asset_id: &AssetIdOf<Runtime>,
    target_reserves: Balance,
) {
    let input = mcbc_tools::OtherCollateralInput::<AssetIdOf<Runtime>> {
        asset: collateral_asset_id.clone(),
        ref_prices: None,
        reserves: Some(target_reserves),
    };

    assert_ok!(initialize_mcbc_collateral::<Runtime>(input));

    let reserves_tech_account_id =
        multicollateral_bonding_curve_pool::Pallet::<Runtime>::reserves_account_id();
    let reserves_account_id =
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&reserves_tech_account_id)
            .unwrap();
    assert_eq!(
        assets::Pallet::<Runtime>::total_balance(&collateral_asset_id, &reserves_account_id),
        Ok(target_reserves)
    );
}

#[test]
fn should_init_tbcd_reserves() {
    ext().execute_with(|| {
        let collateral_asset_id = TBCD.into();
        let input = mcbc_tools::TbcdCollateralInput {
            ref_prices: None,
            reserves: Some(balance!(0)),
            xor_ref_prices: None,
        };
        assert_err!(
            initialize_mcbc_tbcd_collateral::<Runtime>(input),
            qa_tools::Error::<Runtime>::IncorrectCollateralAsset
        );
        set_and_verify_tbcd_reserves(&collateral_asset_id, balance!(1000000));
        set_and_verify_tbcd_reserves(&collateral_asset_id, balance!(0));
    })
}
