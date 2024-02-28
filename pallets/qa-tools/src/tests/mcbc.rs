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

use super::{alice, register_custom_asset, QaToolsPallet};
use assets::AssetIdOf;
use common::prelude::{BalanceUnit, QuoteAmount};
use common::{
    assert_approx_eq, balance, AccountIdOf, AssetInfoProvider, Balance, DEXId, LiquiditySource,
    PriceVariant, CERES_ASSET_ID, ETH, TBCD, VAL, XOR,
};
use frame_support::dispatch::{DispatchError, RawOrigin};
use frame_support::{assert_err, assert_ok};
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeEvent, RuntimeOrigin};
use qa_tools::pallet_tools::liquidity_proxy::liquidity_sources::{
    initialize_mcbc_base_supply, initialize_mcbc_collateral, initialize_mcbc_tbcd_collateral,
};
use qa_tools::pallet_tools::mcbc as mcbc_tools;
use qa_tools::pallet_tools::price_tools::AssetPrices;
use sp_arithmetic::traits::One;

#[test]
fn should_init_mcbc_base_supply() {
    ext().execute_with(|| {
        let collateral_asset_id = VAL.into();

        let xor_holder = alice();
        let current_base_supply = assets::Pallet::<Runtime>::total_issuance(&XOR.into()).unwrap();
        let xor_holder_initial_balance =
            assets::Pallet::<Runtime>::total_balance(&XOR.into(), &xor_holder).unwrap();
        assert!(
            multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
                &DEXId::Polkaswap.into(),
                &collateral_asset_id,
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
                base_supply_collector: xor_holder.clone(),
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
                base_supply_collector: xor_holder.clone(),
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
                base_supply_collector: xor_holder,
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
fn should_init_val_reference_price() {
    ext().execute_with(|| {
        test_init_single_collateral_reference_price(VAL.into());
    })
}

#[test]
fn should_init_eth_reference_price() {
    ext().execute_with(|| {
        test_init_single_collateral_reference_price(ETH.into());
    })
}

#[test]
fn should_init_ceres_reference_price() {
    ext().execute_with(|| {
        test_init_single_collateral_reference_price(CERES_ASSET_ID.into());
    })
}

#[test]
fn should_init_custom_asset_reference_price() {
    ext().execute_with(|| {
        frame_system::Pallet::<Runtime>::set_block_number(1);
        let custom_asset_id = register_custom_asset();
        test_init_single_collateral_reference_price(custom_asset_id);
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
            ref_xor_prices: None,
        }
    ));
    let actual_ref_prices =
        initialize_mcbc_tbcd_collateral::<Runtime>(mcbc_tools::TbcdCollateralInput {
            ref_prices: Some(reference_prices.clone()),
            reserves: None,
            ref_xor_prices: None,
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
                ref_xor_prices: None,
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

        // new asset
        frame_system::Pallet::<Runtime>::set_block_number(1);
        let custom_asset_id = register_custom_asset();
        test_init_single_collateral_reference_price(custom_asset_id);
    })
}

fn set_and_verify_tbcd_reserves(target_reserves: Balance) {
    let input = mcbc_tools::TbcdCollateralInput {
        ref_prices: None,
        reserves: Some(target_reserves),
        ref_xor_prices: None,
    };

    assert_ok!(initialize_mcbc_tbcd_collateral::<Runtime>(input));

    let reserves_tech_account_id =
        multicollateral_bonding_curve_pool::Pallet::<Runtime>::reserves_account_id();
    let reserves_account_id =
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&reserves_tech_account_id)
            .unwrap();
    assert_eq!(
        assets::Pallet::<Runtime>::total_balance(&TBCD.into(), &reserves_account_id),
        Ok(target_reserves)
    );
}

#[test]
fn should_init_tbcd_reserves() {
    ext().execute_with(|| {
        let input = mcbc_tools::OtherCollateralInput {
            asset: TBCD.into(),
            ref_prices: None,
            reserves: None,
        };
        assert_err!(
            initialize_mcbc_collateral::<Runtime>(input),
            qa_tools::Error::<Runtime>::IncorrectCollateralAsset
        );
        set_and_verify_tbcd_reserves(balance!(1000000));
        set_and_verify_tbcd_reserves(balance!(0));
    })
}

fn set_and_verify_tbcd_ref_xor_prices(prices: AssetPrices) {
    let input = mcbc_tools::TbcdCollateralInput {
        ref_prices: None,
        reserves: None,
        ref_xor_prices: Some(prices.clone()),
    };
    let reference_asset = qa_tools::InputAssetId::<AssetIdOf<Runtime>>::McbcReference;
    let reference_asset_id = reference_asset.clone().resolve::<Runtime>();
    assert_ok!(initialize_mcbc_tbcd_collateral::<Runtime>(input));
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(
            &XOR.into(),
            &reference_asset_id,
            PriceVariant::Buy
        ),
        Ok(prices.buy)
    );
    assert_eq!(
        price_tools::Pallet::<Runtime>::get_average_price(
            &XOR.into(),
            &reference_asset_id,
            PriceVariant::Sell
        ),
        Ok(prices.sell)
    );
}

#[test]
fn should_init_tbcd_ref_prices() {
    ext().execute_with(|| {
        let collateral_asset_id: AssetIdOf<Runtime> = TBCD.into();
        let input = mcbc_tools::OtherCollateralInput {
            asset: collateral_asset_id.clone(),
            ref_prices: Some(AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            }),
            reserves: None,
        };
        assert_err!(
            initialize_mcbc_collateral::<Runtime>(input),
            qa_tools::Error::<Runtime>::IncorrectCollateralAsset
        );
        set_and_verify_tbcd_ref_xor_prices(AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        });
        set_and_verify_tbcd_ref_xor_prices(AssetPrices {
            buy: balance!(124),
            sell: balance!(123),
        });
        set_and_verify_tbcd_ref_xor_prices(AssetPrices {
            buy: balance!(0.1),
            sell: balance!(0.01),
        });
    })
}

/// Returns list of events (each event = list of initialized collateral assets + actual prices)
fn get_all_mcbc_init_events() -> Vec<Vec<(AssetIdOf<Runtime>, AssetPrices)>> {
    assert!(
        frame_system::Pallet::<Runtime>::block_number() >= 1,
        "events are not dispatched at block 0"
    );
    let events = frame_system::Pallet::<Runtime>::events()
        .into_iter()
        .map(|e| e.event);
    let mut result = vec![];
    for e in events {
        let RuntimeEvent::QaTools(qa_tools_event) = e else {
            continue
        };
        let qa_tools::Event::<Runtime>::McbcInitialized{ collateral_ref_prices } = qa_tools_event else {
            continue
        };
        result.push(collateral_ref_prices)
    }
    result
}

#[test]
fn should_extrinsic_produce_correct_events() {
    ext().execute_with(|| {
        // events are omitted on block 0
        frame_system::Pallet::<Runtime>::set_block_number(1);

        let collateral_asset_id: AssetIdOf<Runtime> = VAL.into();
        let xor_holder = alice();
        let current_base_supply = assets::Pallet::<Runtime>::total_issuance(&XOR.into()).unwrap();
        let new_supply = current_base_supply + balance!(10000);
        let collateral_reference_prices = AssetPrices {
            buy: balance!(2),
            sell: balance!(1),
        };
        let collateral_reserves = balance!(1000000);
        let tbcd_reference_prices = AssetPrices {
            buy: balance!(4),
            sell: balance!(3),
        };
        let tbcd_reserves = balance!(1000000);
        let ref_xor_prices = AssetPrices {
            buy: balance!(6),
            sell: balance!(5),
        };
        assert_ok!(qa_tools::Pallet::<Runtime>::mcbc_initialize(
            RawOrigin::Root.into(),
            Some(mcbc_tools::BaseSupply {
                base_supply_collector: xor_holder.clone(),
                new_base_supply: new_supply,
            }),
            vec![mcbc_tools::OtherCollateralInput::<AssetIdOf<Runtime>> {
                asset: collateral_asset_id.clone(),
                ref_prices: Some(collateral_reference_prices.clone()),
                reserves: Some(collateral_reserves),
            }],
            Some(mcbc_tools::TbcdCollateralInput {
                ref_prices: Some(tbcd_reference_prices.clone()),
                reserves: Some(tbcd_reserves),
                ref_xor_prices: Some(ref_xor_prices.clone()),
            }),
        ));
        let events = get_all_mcbc_init_events();
        // one init call
        assert_eq!(events.len(), 1);
        let init_collaterals = events.into_iter().next().unwrap();
        // 2 collaterals initialized in the call
        assert_eq!(init_collaterals.len(), 2);

        // check that the values are close enough to requested
        let (actual_collateral_reference_prices, actual_tbcd_reference_prices) =
            match (init_collaterals[0].clone(), init_collaterals[1].clone()) {
                ((tbcd, tbcd_prices), (collateral, collateral_prices))
                | ((collateral, collateral_prices), (tbcd, tbcd_prices))
                    if tbcd == TBCD.into() && collateral == collateral_asset_id =>
                {
                    (collateral_prices, tbcd_prices)
                }
                _ => panic!("unexpected asset ids in events: {:?}", init_collaterals),
            };
        assert_approx_eq!(
            collateral_reference_prices.buy,
            actual_collateral_reference_prices.buy,
            10,
            0.0001f64
        );
        assert_approx_eq!(
            collateral_reference_prices.sell,
            actual_collateral_reference_prices.sell,
            10,
            0.0001f64
        );
        assert_approx_eq!(
            tbcd_reference_prices.buy,
            actual_tbcd_reference_prices.buy,
            10,
            0.0001f64
        );
        assert_approx_eq!(
            tbcd_reference_prices.sell,
            actual_tbcd_reference_prices.sell,
            10,
            0.0001f64
        );
    })
}

/// formulae are taken from `multicollateral-bonding-curve-pool`
fn expected_sell_quote_collateral_amount(
    amount_in: Balance,
    collateral_asset_id: AssetIdOf<Runtime>,
    target_supply: Balance,
    collateral_reference_prices: AssetPrices,
    ref_xor_prices: AssetPrices,
) -> Result<Balance, DispatchError> {
    let target_supply = BalanceUnit::divisible(target_supply);
    let amount_in = BalanceUnit::divisible(amount_in);
    // sell (XOR -> collateral)
    let collateral_supply: BalanceUnit = {
        let reserves_tech_account_id =
            multicollateral_bonding_curve_pool::ReservesAcc::<Runtime>::get();
        let reserves_account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
        assets::Pallet::<Runtime>::free_balance(&collateral_asset_id, &reserves_account_id)?.into()
    };
    //
    // Get reference prices for base and collateral to understand token value.
    let main_price_per_reference_unit: BalanceUnit = {
        let buy_price = {
            if collateral_asset_id == TBCD.into() {
                BalanceUnit::divisible(ref_xor_prices.sell) + BalanceUnit::one()
            } else {
                // Everything other than TBCD
                let initial_price: BalanceUnit = BalanceUnit::divisible(
                    multicollateral_bonding_curve_pool::Pallet::<Runtime>::initial_price()
                        .into_bits()
                        .try_into()
                        .expect("must not be negative"),
                );
                let price_change_step: BalanceUnit = BalanceUnit::divisible(
                    multicollateral_bonding_curve_pool::Pallet::<Runtime>::price_change_step()
                        .into_bits()
                        .try_into()
                        .expect("must not be negative"),
                );
                let price_change_rate: BalanceUnit = BalanceUnit::divisible(
                    multicollateral_bonding_curve_pool::Pallet::<Runtime>::price_change_rate()
                        .into_bits()
                        .try_into()
                        .expect("must not be negative"),
                );

                target_supply / (price_change_step * price_change_rate) + initial_price
            }
        };
        let sell_price_coefficient = BalanceUnit::divisible(
            multicollateral_bonding_curve_pool::Pallet::<Runtime>::sell_price_coefficient()
                .into_bits()
                .try_into()
                .expect("must not be negative"),
        );
        sell_price_coefficient * buy_price
    };

    let collateral_price = if collateral_asset_id == TBCD.into() {
        BalanceUnit::one()
    } else {
        BalanceUnit::divisible(collateral_reference_prices.sell)
    };
    let main_supply = collateral_supply.clone() * collateral_price / main_price_per_reference_unit;
    let amount_out = (amount_in * collateral_supply) / (main_supply + amount_in);
    Ok(*amount_out.balance())
}

fn init_mcbc_and_check_quote_exchange(
    collateral_asset_id: AssetIdOf<Runtime>,
    target_supply: Balance,
    collateral_reserves: Balance,
    tbcd_reserves: Balance,
    collateral_reference_prices: AssetPrices,
    tbcd_reference_prices: AssetPrices,
    ref_xor_prices: AssetPrices,
    xor_holder: AccountIdOf<Runtime>,
) {
    let reference_asset = qa_tools::InputAssetId::<AssetIdOf<Runtime>>::McbcReference;
    let reference_asset_id = reference_asset.clone().resolve::<Runtime>();

    assert_ok!(qa_tools::Pallet::<Runtime>::mcbc_initialize(
        RawOrigin::Root.into(),
        Some(mcbc_tools::BaseSupply {
            base_supply_collector: xor_holder.clone(),
            new_base_supply: target_supply,
        }),
        vec![mcbc_tools::OtherCollateralInput::<AssetIdOf<Runtime>> {
            asset: collateral_asset_id.clone(),
            ref_prices: Some(collateral_reference_prices.clone()),
            reserves: Some(collateral_reserves),
        }],
        Some(mcbc_tools::TbcdCollateralInput {
            ref_prices: Some(tbcd_reference_prices.clone()),
            reserves: Some(tbcd_reserves),
            ref_xor_prices: Some(ref_xor_prices.clone()),
        }),
    ));
    // check the results of initialization
    let (actual_collateral_reference_prices, actual_tbcd_reference_prices) = {
        assert_eq!(
            assets::Pallet::<Runtime>::total_issuance(&XOR.into()).unwrap(),
            target_supply
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR.into(),
                &reference_asset_id,
                PriceVariant::Buy
            ),
            Ok(ref_xor_prices.buy)
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR.into(),
                &reference_asset_id,
                PriceVariant::Sell
            ),
            Ok(ref_xor_prices.sell)
        );

        let reserves_tech_account_id =
            multicollateral_bonding_curve_pool::Pallet::<Runtime>::reserves_account_id();
        let reserves_account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&reserves_tech_account_id)
                .unwrap();
        assert_eq!(
            assets::Pallet::<Runtime>::total_balance(&collateral_asset_id, &reserves_account_id),
            Ok(collateral_reserves)
        );
        assert_eq!(
            assets::Pallet::<Runtime>::total_balance(&TBCD.into(), &reserves_account_id),
            Ok(tbcd_reserves)
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR.into(),
                &reference_asset_id,
                PriceVariant::Buy
            ),
            Ok(ref_xor_prices.buy)
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR.into(),
                &reference_asset_id,
                PriceVariant::Sell
            ),
            Ok(ref_xor_prices.sell)
        );
        let events = get_all_mcbc_init_events();
        // one init call
        assert_eq!(events.len(), 1);
        let init_collaterals = events.into_iter().next().unwrap();
        // 2 collaterals initialized in the call
        assert_eq!(init_collaterals.len(), 2);

        // check that the values are close enough to requested
        let (actual_collateral_reference_prices, actual_tbcd_reference_prices) =
            match (init_collaterals[0].clone(), init_collaterals[1].clone()) {
                ((tbcd, tbcd_prices), (collateral, collateral_prices))
                | ((collateral, collateral_prices), (tbcd, tbcd_prices))
                    if tbcd == TBCD.into() && collateral == collateral_asset_id =>
                {
                    (collateral_prices, tbcd_prices)
                }
                _ => panic!("unexpected asset ids in events: {:?}", init_collaterals),
            };
        assert_approx_eq!(
            collateral_reference_prices.buy,
            actual_collateral_reference_prices.buy,
            10,
            0.0001f64
        );
        assert_approx_eq!(
            collateral_reference_prices.sell,
            actual_collateral_reference_prices.sell,
            10,
            0.0001f64
        );
        assert_approx_eq!(
            tbcd_reference_prices.buy,
            actual_tbcd_reference_prices.buy,
            10,
            0.0001f64
        );
        assert_approx_eq!(
            tbcd_reference_prices.sell,
            actual_tbcd_reference_prices.sell,
            10,
            0.0001f64
        );
        (
            actual_collateral_reference_prices,
            actual_tbcd_reference_prices,
        )
    };

    let quote_amount_in = balance!(127);
    let (expected_collateral_quote_amount_out, expected_tbcd_quote_amount_out) = (
        expected_sell_quote_collateral_amount(
            quote_amount_in,
            collateral_asset_id,
            target_supply,
            actual_collateral_reference_prices,
            ref_xor_prices.clone(),
        )
        .unwrap(),
        expected_sell_quote_collateral_amount(
            quote_amount_in,
            TBCD.into(),
            target_supply,
            actual_tbcd_reference_prices,
            ref_xor_prices,
        )
        .unwrap(),
    );
    let (collateral_quote_result, _) =
        multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &XOR.into(),
            &collateral_asset_id.into(),
            QuoteAmount::WithDesiredInput {
                desired_amount_in: quote_amount_in,
            },
            false,
        )
        .unwrap();
    assert_eq!(
        collateral_quote_result.amount,
        expected_collateral_quote_amount_out
    );
    let (tbcd_quote_result, _) = multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
        &DEXId::Polkaswap.into(),
        &XOR.into(),
        &TBCD.into(),
        QuoteAmount::WithDesiredInput {
            desired_amount_in: quote_amount_in,
        },
        false,
    )
    .unwrap();
    assert_eq!(tbcd_quote_result.amount, expected_tbcd_quote_amount_out);
}

fn test_quote(collateral_asset_id: AssetIdOf<Runtime>) {
    assert!(
        multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &collateral_asset_id,
            &XOR.into(),
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
            true,
        )
        .is_err()
    );

    let xor_holder = alice();
    let current_base_supply = assets::Pallet::<Runtime>::total_issuance(&XOR.into()).unwrap();
    let new_supply = current_base_supply + balance!(10000);
    let collateral_reference_prices = AssetPrices {
        buy: balance!(3),
        sell: balance!(2),
    };
    let collateral_reserves = balance!(1000000);
    let tbcd_reference_prices = AssetPrices {
        buy: balance!(7),
        sell: balance!(5),
    };
    let tbcd_reserves = balance!(1000000);
    let ref_xor_prices = AssetPrices {
        buy: balance!(13),
        sell: balance!(11),
    };
    init_mcbc_and_check_quote_exchange(
        collateral_asset_id,
        new_supply,
        collateral_reserves,
        tbcd_reserves,
        collateral_reference_prices,
        tbcd_reference_prices,
        ref_xor_prices,
        xor_holder,
    )
}

#[test]
fn should_init_correctly_val() {
    ext().execute_with(|| {
        frame_system::Pallet::<Runtime>::set_block_number(1);
        test_quote(VAL.into());
    })
}

#[test]
fn should_init_correctly_custom() {
    ext().execute_with(|| {
        frame_system::Pallet::<Runtime>::set_block_number(1);
        test_quote(register_custom_asset());
    })
}

#[test]
fn ref_xor_price_update_changes_quote() {
    ext().execute_with(|| {
        let collateral_asset_id = VAL.into();
        assert!(
            multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
                &DEXId::Polkaswap.into(),
                &collateral_asset_id,
                &XOR.into(),
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(1),
                },
                true,
            )
            .is_err()
        );

        let xor_holder = alice();
        let current_base_supply = assets::Pallet::<Runtime>::total_issuance(&XOR.into()).unwrap();
        let new_supply = current_base_supply + balance!(10000);
        let collateral_reference_prices = AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        };
        let collateral_reserves = balance!(1000000);
        let tbcd_reference_prices = AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        };
        let tbcd_reserves = balance!(1000000);
        let ref_xor_prices = AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        };
        let ref_xor_prices_2 = AssetPrices {
            buy: balance!(2),
            sell: balance!(2),
        };
        assert_ok!(qa_tools::Pallet::<Runtime>::mcbc_initialize(
            RawOrigin::Root.into(),
            Some(mcbc_tools::BaseSupply {
                base_supply_collector: xor_holder.clone(),
                new_base_supply: new_supply,
            }),
            vec![mcbc_tools::OtherCollateralInput::<AssetIdOf<Runtime>> {
                asset: collateral_asset_id.clone(),
                ref_prices: Some(collateral_reference_prices.clone()),
                reserves: Some(collateral_reserves),
            }],
            Some(mcbc_tools::TbcdCollateralInput {
                ref_prices: Some(tbcd_reference_prices.clone()),
                reserves: Some(tbcd_reserves),
                ref_xor_prices: Some(ref_xor_prices.clone()),
            }),
        ));
        let (quote_result, _) = multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &XOR.into(),
            &collateral_asset_id.into(),
            QuoteAmount::WithDesiredInput {
                desired_amount_in: balance!(1),
            },
            true,
        )
        .unwrap();

        assert_ok!(qa_tools::Pallet::<Runtime>::mcbc_initialize(
            RawOrigin::Root.into(),
            None,
            vec![],
            Some(mcbc_tools::TbcdCollateralInput {
                ref_prices: None,
                reserves: None,
                ref_xor_prices: Some(ref_xor_prices_2.clone()),
            }),
        ));

        let (quote_result_2, _) = multicollateral_bonding_curve_pool::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &XOR.into(),
            &collateral_asset_id.into(),
            QuoteAmount::WithDesiredInput {
                desired_amount_in: balance!(1),
            },
            true,
        )
        .unwrap();
        // the prices differ
        assert_ne!(quote_result, quote_result_2);
    })
}
