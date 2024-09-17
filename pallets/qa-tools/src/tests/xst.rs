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

use super::{alice, QaToolsPallet};
use common::prelude::{err_pays_no, BalanceUnit, QuoteAmount, SwapVariant};
use common::PriceToolsProvider;
use common::{
    assert_approx_eq, balance, fixed, AssetId32, AssetIdOf, AssetName, AssetSymbol, DEXId,
    LiquiditySource, PredefinedAssetId, PriceVariant, SymbolName, XOR,
};
use core::str::FromStr;
use frame_support::assert_ok;
use frame_support::traits::Get;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeOrigin};
use qa_tools::pallet_tools;
use qa_tools::{Error, InputAssetId};

use pallet_tools::liquidity_proxy::liquidity_sources::initialize_xst;
use pallet_tools::price_tools::AssetPrices;

fn test_init_xst_synthetic_base_price(
    prices: pallet_tools::xst::BaseInput,
    reference_prices: AssetPrices,
) {
    ext().execute_with(|| {
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();
        let synthetic_base_asset_id = <Runtime as xst::Config>::GetSyntheticBaseAssetId::get();

        assert_eq!(
            QaToolsPallet::xst_initialize(
                RuntimeOrigin::root(),
                Some(prices.clone()),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::ReferenceAssetPriceNotFound))
        );

        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));

        assert_ok!(QaToolsPallet::xst_initialize(
            RuntimeOrigin::root(),
            Some(prices.clone()),
            vec![],
            alice(),
        ));
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &synthetic_base_asset_id,
                &reference_asset_id,
                PriceVariant::Buy
            ),
            Ok(prices.reference_per_synthetic_base_buy)
        );
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &synthetic_base_asset_id,
                &reference_asset_id,
                PriceVariant::Sell
            ),
            Ok(prices.reference_per_synthetic_base_sell)
        );
    });
}

#[test]
fn should_init_xst_synthetic_base_price() {
    let reference_prices = AssetPrices {
        buy: balance!(1),
        sell: balance!(1),
    };
    let prices = pallet_tools::xst::BaseInput {
        reference_per_synthetic_base_buy: balance!(1),
        reference_per_synthetic_base_sell: balance!(1),
    };
    test_init_xst_synthetic_base_price(prices, reference_prices);
    let reference_prices = AssetPrices {
        buy: balance!(5),
        sell: balance!(2),
    };
    let prices = pallet_tools::xst::BaseInput {
        reference_per_synthetic_base_buy: balance!(3),
        reference_per_synthetic_base_sell: balance!(1),
    };
    test_init_xst_synthetic_base_price(prices, reference_prices);
}

#[test]
fn should_reject_incorrect_xst_base_price() {
    ext().execute_with(|| {
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();
        let reference_prices = AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        };
        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));

        assert_eq!(
            QaToolsPallet::xst_initialize(
                RuntimeOrigin::root(),
                Some(pallet_tools::xst::BaseInput {
                    reference_per_synthetic_base_buy: balance!(1),
                    reference_per_synthetic_base_sell: balance!(1.1),
                }),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::BuyLessThanSell))
        );
    })
}

#[test]
fn should_reject_deduce_only_with_uninitialized_reference_asset() {
    ext().execute_with(|| {
        // Reject when not initialized
        assert_eq!(
            QaToolsPallet::xst_initialize(
                RuntimeOrigin::root(),
                Some(pallet_tools::xst::BaseInput {
                    reference_per_synthetic_base_buy: balance!(1),
                    reference_per_synthetic_base_sell: balance!(1),
                }),
                vec![],
                alice(),
            ),
            Err(err_pays_no(Error::<Runtime>::ReferenceAssetPriceNotFound))
        );

        // Initialize the reference asset
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();
        let reference_prices = AssetPrices {
            buy: balance!(5),
            sell: balance!(2),
        };
        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));

        // Now it should work fine
        assert_ok!(QaToolsPallet::xst_initialize(
            RuntimeOrigin::root(),
            Some(pallet_tools::xst::BaseInput {
                reference_per_synthetic_base_buy: balance!(3),
                reference_per_synthetic_base_sell: balance!(1),
            }),
            vec![],
            alice(),
        ));

        let (reference_per_synthetic_base_buy, reference_per_synthetic_base_sell) =
            (balance!(21), balance!(7));
        assert_ok!(QaToolsPallet::xst_initialize(
            RuntimeOrigin::root(),
            Some(pallet_tools::xst::BaseInput {
                reference_per_synthetic_base_buy,
                reference_per_synthetic_base_sell,
            }),
            vec![],
            alice(),
        ));
        // check prices
        let reference_per_xor_buy = price_tools::Pallet::<Runtime>::get_average_price(
            &XOR,
            &xst::ReferenceAssetId::<Runtime>::get(),
            PriceVariant::Buy,
        )
        .unwrap();
        let reference_per_xor_sell = price_tools::Pallet::<Runtime>::get_average_price(
            &XOR,
            &xst::ReferenceAssetId::<Runtime>::get(),
            PriceVariant::Sell,
        )
        .unwrap();
        let synthetic_base_per_xor_buy = BalanceUnit::divisible(reference_per_xor_sell)
            / BalanceUnit::divisible(reference_per_synthetic_base_sell);
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR,
                &<Runtime as xst::Config>::GetSyntheticBaseAssetId::get(),
                PriceVariant::Buy,
            )
            .unwrap(),
            *synthetic_base_per_xor_buy.balance()
        );
        let synthetic_base_per_xor_sell = BalanceUnit::divisible(reference_per_xor_buy)
            / BalanceUnit::divisible(reference_per_synthetic_base_buy);
        assert_eq!(
            price_tools::Pallet::<Runtime>::get_average_price(
                &XOR,
                &<Runtime as xst::Config>::GetSyntheticBaseAssetId::get(),
                PriceVariant::Sell,
            )
            .unwrap(),
            *synthetic_base_per_xor_sell.balance()
        );
    })
}

fn euro_init_input<T: qa_tools::Config>(
    expected_quote: pallet_tools::xst::SyntheticQuote,
) -> pallet_tools::xst::SyntheticInput<AssetIdOf<T>, <T as qa_tools::Config>::Symbol> {
    let symbol_name =
        SymbolName::from_str("EURO").expect("Failed to parse `symbol_name` as a symbol name");
    let asset_id = AssetId32::<PredefinedAssetId>::from_synthetic_reference_symbol(&symbol_name);
    let symbol = AssetSymbol("XSTEUR".into());
    let name = AssetName("XST Euro".into());
    let fee_ratio = fixed!(0);
    pallet_tools::xst::SyntheticInput {
        asset_id: asset_id.into(),
        expected_quote,
        existence: pallet_tools::xst::SyntheticExistence::RegisterNewAsset {
            symbol,
            name,
            reference_symbol: symbol_name.into(),
            fee_ratio,
        },
    }
}

/// Returns results of initialization
fn test_synthetic_price_set<T: qa_tools::Config>(
    synthetic_input: pallet_tools::xst::SyntheticInput<
        AssetIdOf<T>,
        <T as qa_tools::Config>::Symbol,
    >,
    base_input: Option<pallet_tools::xst::BaseInput>,
    relayer: T::AccountId,
) -> Vec<pallet_tools::xst::SyntheticOutput<AssetIdOf<T>>> {
    let synthetic_base_asset_id = <T as xst::Config>::GetSyntheticBaseAssetId::get();
    let init_result =
        initialize_xst::<T>(base_input, vec![synthetic_input.clone()], relayer).unwrap();
    assert_approx_eq!(
        synthetic_input.expected_quote.result,
        init_result[0].quote_achieved.result,
        10,
        0.0001f64
    );

    let (input_asset_id, output_asset_id) = match synthetic_input.expected_quote.direction {
        pallet_tools::xst::SyntheticQuoteDirection::SyntheticBaseToSynthetic => {
            (synthetic_base_asset_id, synthetic_input.asset_id)
        }
        pallet_tools::xst::SyntheticQuoteDirection::SyntheticToSyntheticBase => {
            (synthetic_input.asset_id, synthetic_base_asset_id)
        }
    };
    let (quote_result, _) = xst::Pallet::<T>::quote(
        &DEXId::Polkaswap.into(),
        &input_asset_id,
        &output_asset_id,
        synthetic_input.expected_quote.amount,
        false,
    )
    .unwrap();
    assert_eq!(quote_result.amount, init_result[0].quote_achieved.result);
    assert_eq!(quote_result.fee, Default::default());
    init_result
}

fn test_init_xst_synthetic_price_unit_prices(forward: bool, variant: SwapVariant) {
    ext().execute_with(|| {
        let synthetic_base_asset_id = <Runtime as xst::Config>::GetSyntheticBaseAssetId::get();
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();

        // simple for calculations, even though quite unrealistic
        let reference_prices = AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        };
        let prices = pallet_tools::xst::BaseInput {
            reference_per_synthetic_base_buy: balance!(1),
            reference_per_synthetic_base_sell: balance!(1),
        };
        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));

        let direction = if forward {
            pallet_tools::xst::SyntheticQuoteDirection::SyntheticBaseToSynthetic
        } else {
            pallet_tools::xst::SyntheticQuoteDirection::SyntheticToSyntheticBase
        };
        let amount = match variant {
            SwapVariant::WithDesiredOutput => QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
            SwapVariant::WithDesiredInput => QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
        };
        let euro_init = euro_init_input::<Runtime>(pallet_tools::xst::SyntheticQuote {
            direction,
            amount,
            result: balance!(1),
        });
        test_synthetic_price_set::<Runtime>(euro_init.clone(), Some(prices), alice());
        // additionally check other directions/variants
        let (quote_result, _) = xst::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &synthetic_base_asset_id,
            &euro_init.asset_id,
            QuoteAmount::WithDesiredInput {
                desired_amount_in: balance!(1),
            },
            false,
        )
        .unwrap();
        assert_eq!(quote_result.amount, balance!(1));
        assert_eq!(quote_result.fee, Default::default());
        let (quote_result, _) = xst::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &euro_init.asset_id,
            &synthetic_base_asset_id,
            QuoteAmount::WithDesiredInput {
                desired_amount_in: balance!(1),
            },
            false,
        )
        .unwrap();
        assert_eq!(quote_result.amount, balance!(1));
        assert_eq!(quote_result.fee, Default::default());
        let (quote_result, _) = xst::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &euro_init.asset_id,
            &synthetic_base_asset_id,
            QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
            false,
        )
        .unwrap();
        assert_eq!(quote_result.amount, balance!(1));
        assert_eq!(quote_result.fee, Default::default());
    })
}

#[test]
fn should_init_xst_synthetic_unit_prices_forward_out() {
    test_init_xst_synthetic_price_unit_prices(true, SwapVariant::WithDesiredOutput);
}

#[test]
fn should_init_xst_synthetic_unit_prices_forward_in() {
    test_init_xst_synthetic_price_unit_prices(true, SwapVariant::WithDesiredInput);
}

#[test]
fn should_init_xst_synthetic_unit_prices_reverse_out() {
    test_init_xst_synthetic_price_unit_prices(false, SwapVariant::WithDesiredOutput);
}

#[test]
fn should_init_xst_synthetic_unit_prices_reverse_in() {
    test_init_xst_synthetic_price_unit_prices(false, SwapVariant::WithDesiredInput);
}

fn test_init_xst_synthetic_price_various_prices(forward: bool, variant: SwapVariant) {
    ext().execute_with(|| {
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();

        let reference_prices = AssetPrices {
            buy: balance!(13),
            sell: balance!(5),
        };
        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));

        let prices = pallet_tools::xst::BaseInput {
            reference_per_synthetic_base_buy: balance!(53),
            reference_per_synthetic_base_sell: balance!(3),
        };
        let direction = if forward {
            pallet_tools::xst::SyntheticQuoteDirection::SyntheticBaseToSynthetic
        } else {
            pallet_tools::xst::SyntheticQuoteDirection::SyntheticToSyntheticBase
        };
        let amount = match variant {
            SwapVariant::WithDesiredOutput => QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(137),
            },
            SwapVariant::WithDesiredInput => QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(137),
            },
        };
        let euro_init = euro_init_input::<Runtime>(pallet_tools::xst::SyntheticQuote {
            direction,
            amount,
            result: balance!(37),
        });
        test_synthetic_price_set::<Runtime>(euro_init, Some(prices), alice());
    })
}

#[test]
fn should_init_xst_synthetic_price_various_prices_forward_out() {
    test_init_xst_synthetic_price_various_prices(true, SwapVariant::WithDesiredOutput);
}

#[test]
fn should_init_xst_synthetic_price_various_prices_forward_in() {
    test_init_xst_synthetic_price_various_prices(true, SwapVariant::WithDesiredInput);
}

#[test]
fn should_init_xst_synthetic_price_various_prices_reverse_out() {
    test_init_xst_synthetic_price_various_prices(false, SwapVariant::WithDesiredOutput);
}

#[test]
fn should_init_xst_synthetic_price_various_prices_reverse_in() {
    test_init_xst_synthetic_price_various_prices(false, SwapVariant::WithDesiredInput);
}

#[test]
fn should_update_xst_synthetic_price() {
    ext().execute_with(|| {
        let synthetic_base_asset_id = <Runtime as xst::Config>::GetSyntheticBaseAssetId::get();
        let reference_asset_id = xst::ReferenceAssetId::<Runtime>::get();

        let reference_prices = AssetPrices {
            buy: balance!(5),
            sell: balance!(2),
        };
        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));

        // Some initial values
        let prices = pallet_tools::xst::BaseInput {
            reference_per_synthetic_base_buy: balance!(3),
            reference_per_synthetic_base_sell: balance!(1),
        };

        let euro_init = euro_init_input::<Runtime>(pallet_tools::xst::SyntheticQuote {
            direction: pallet_tools::xst::SyntheticQuoteDirection::SyntheticBaseToSynthetic,
            amount: QuoteAmount::WithDesiredOutput {
                desired_amount_out: balance!(1),
            },
            result: balance!(123),
        });
        let euro_asset_id = euro_init.asset_id;
        test_synthetic_price_set::<Runtime>(euro_init, Some(prices), alice());
        // correctly updates prices
        let reference_prices = AssetPrices {
            buy: balance!(1),
            sell: balance!(1),
        };
        assert_ok!(QaToolsPallet::price_tools_set_asset_price(
            RuntimeOrigin::root(),
            reference_prices,
            InputAssetId::Other(reference_asset_id)
        ));
        let prices = pallet_tools::xst::BaseInput {
            reference_per_synthetic_base_buy: balance!(1),
            reference_per_synthetic_base_sell: balance!(1),
        };
        let euro_init = pallet_tools::xst::SyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: pallet_tools::xst::SyntheticQuote {
                direction: pallet_tools::xst::SyntheticQuoteDirection::SyntheticBaseToSynthetic,
                amount: QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(1),
                },
                result: balance!(33),
            },
            existence: pallet_tools::xst::SyntheticExistence::AlreadyExists,
        };
        test_synthetic_price_set::<Runtime>(euro_init, Some(prices), alice());

        // other variants
        let euro_init = pallet_tools::xst::SyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: pallet_tools::xst::SyntheticQuote {
                direction: pallet_tools::xst::SyntheticQuoteDirection::SyntheticBaseToSynthetic,
                amount: QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(1),
                },
                result: balance!(33),
            },
            existence: pallet_tools::xst::SyntheticExistence::AlreadyExists,
        };
        test_synthetic_price_set::<Runtime>(euro_init, None, alice());
        let euro_init = pallet_tools::xst::SyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: pallet_tools::xst::SyntheticQuote {
                direction: pallet_tools::xst::SyntheticQuoteDirection::SyntheticToSyntheticBase,
                amount: QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(1),
                },
                result: balance!(33),
            },
            existence: pallet_tools::xst::SyntheticExistence::AlreadyExists,
        };
        test_synthetic_price_set::<Runtime>(euro_init, None, alice());
        let euro_init = pallet_tools::xst::SyntheticInput {
            asset_id: euro_asset_id,
            expected_quote: pallet_tools::xst::SyntheticQuote {
                direction: pallet_tools::xst::SyntheticQuoteDirection::SyntheticToSyntheticBase,
                amount: QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(1),
                },
                result: balance!(33),
            },
            existence: pallet_tools::xst::SyntheticExistence::AlreadyExists,
        };
        let init_result = test_synthetic_price_set::<Runtime>(euro_init.clone(), None, alice());

        // prices actually change
        let prices = pallet_tools::xst::BaseInput {
            reference_per_synthetic_base_buy: balance!(321),
            reference_per_synthetic_base_sell: balance!(123),
        };
        assert_ok!(QaToolsPallet::xst_initialize(
            RuntimeOrigin::root(),
            Some(prices),
            vec![],
            alice(),
        ));
        let (quote_result, _) = xst::Pallet::<Runtime>::quote(
            &DEXId::Polkaswap.into(),
            &synthetic_base_asset_id,
            &euro_asset_id,
            euro_init.expected_quote.amount,
            false,
        )
        .unwrap();
        assert_ne!(quote_result.amount, init_result[0].quote_achieved.result);
        assert_eq!(quote_result.fee, Default::default());

        // Synthetic prices are updated correctly after changes in base assets prices.
        test_synthetic_price_set::<Runtime>(euro_init, None, alice());
    })
}
