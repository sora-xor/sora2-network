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

#[rustfmt::skip]
mod tests {
    use core::str::FromStr;

    use crate::{Error, Pallet, mock::*};
    use common::{self, AssetName, AssetSymbol, AssetInfoProvider, DEXId, LiquiditySource, USDT, VAL, XOR, XST, XSTUSD, DAI, balance, fixed, GetMarketInfo, assert_approx_eq, prelude::{Balance, SwapAmount, QuoteAmount, FixedWrapper, }, SymbolName, Oracle, PriceVariant, PredefinedAssetId, AssetId32};
    use frame_support::{assert_ok, assert_noop};
    use sp_arithmetic::traits::{Zero};

    type XSTPool = Pallet<Runtime>;
    type PriceTools = price_tools::Pallet<Runtime>;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // base case for buy
            assert_eq!(
                XSTPool::buy_price(&XST, &XSTUSD, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(15516385.30287984111) // ~ (100000.0-100000.0*0.007)*156.25
            );

            // base case for sell
            assert_eq!(
                XSTPool::sell_price(&XST, &XSTUSD, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(635.520000000000000371) // ~ (100000+100000*0.007)/156.25
            );
        });
    }

    #[test]
    fn calculate_price_for_boundary_values() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let alice = alice();
            // add some reserves
            XSTPool::exchange(&alice, &alice, &DEXId::Polkaswap, &XSTUSD, &XST, SwapAmount::with_desired_input(balance!(1), 0)).expect("Failed to buy XST.");

            assert_noop!(
                XSTPool::sell_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_noop!(
                XSTPool::sell_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_eq!(
                XSTPool::sell_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
            assert_eq!(
                XSTPool::sell_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );

            assert_noop!(
                XSTPool::buy_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_noop!(
                XSTPool::buy_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_eq!(
                XSTPool::buy_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
            assert_eq!(
                XSTPool::buy_price(
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
        });
    }

    #[test]
    fn should_set_new_reference_token() {
        let mut ext = ExtBuilder::new(
            vec![
                (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
                (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
                (alice(), XOR, balance!(1), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
                (alice(), VAL, balance!(0), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
                (alice(), XST, balance!(0), AssetSymbol(b"XST".to_vec()), AssetName(b"SORA Synthetics".to_vec()), 18),
            ],
            vec![
                (alice(), XSTUSD, balance!(0), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
            ]
        ).build();
        ext.execute_with(|| {
            let price_a = XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(1)),
                    true,
            )
                .unwrap();

            XSTPool::set_reference_asset(RuntimeOrigin::root(), DAI).expect("Failed to set new reference asset.");

            let price_b = XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(balance!(1)),
                    true,
            )
                .unwrap();

            assert_ne!(price_a, price_b);
        });
    }

    #[test]
    fn similar_returns_should_be_identical() {
        let mut ext = ExtBuilder::new(
            vec![
                (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
                (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
                (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
                (alice(), VAL, balance!(4000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
                (alice(), XST, balance!(0), AssetSymbol(b"XST".to_vec()), AssetName(b"SORA Synthetics".to_vec()), 18),
            ],
            vec![
                (alice(), XSTUSD, balance!(50000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
            ]
        )
        .build();
        ext.execute_with(|| {
            // Buy with desired input
            let amount_a: Balance = balance!(2000);
            let (quote_outcome_a, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(amount_a.clone()),
                true,
            )
            .unwrap();

            let (exchange_outcome_a, _) = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
            )
            .unwrap();

            let xstusd_balance_a = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_a = Assets::free_balance(&XST, &alice()).unwrap();

            assert_eq!(quote_outcome_a.amount, exchange_outcome_a.amount);
            assert_eq!(exchange_outcome_a.amount, xor_balance_a);
            assert_eq!(xstusd_balance_a, balance!(48000));

            // Buy with desired output
            let amount_b: Balance = balance!(200);
            let (quote_outcome_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(amount_b.clone()),
                true,
            )
            .unwrap();

            let (exchange_outcome_b, _) = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
            )
            .unwrap();

            let xstusd_balance_b = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_b = Assets::free_balance(&XST, &alice()).unwrap();

            assert_eq!(quote_outcome_b.amount, exchange_outcome_b.amount);
            assert_eq!(xor_balance_a + amount_b.clone(), xor_balance_b);
            assert_eq!(xstusd_balance_b, balance!(16759.165436044373306346));

            // Sell with desired input
            let amount_c: Balance = balance!(205);
            let (quote_outcome_c, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_input(amount_c.clone()),
                true,
            )
            .unwrap();

            let (exchange_outcome_c, _) = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
            )
            .unwrap();

            let xstusd_balance_c = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_c = Assets::free_balance(&XST, &alice()).unwrap();

            assert_eq!(quote_outcome_c.amount, exchange_outcome_c.amount);
            assert_eq!(xstusd_balance_b + exchange_outcome_c.amount, xstusd_balance_c);
            assert_eq!(xor_balance_b - amount_c.clone(), xor_balance_c.clone());

            // Sell with desired output
            let amount_d: Balance = balance!(100);
            let (quote_outcome_d, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(amount_d.clone()),
                true,
            )
            .unwrap();
            let (exchange_outcome_d, _) = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
            )
            .unwrap();
            let xstusd_balance_d = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_d = Assets::free_balance(&XST, &alice()).unwrap();
            assert_eq!(quote_outcome_d.amount, exchange_outcome_d.amount);
            assert_eq!(xstusd_balance_c - quote_outcome_d.amount, xstusd_balance_d);
            assert_eq!(xor_balance_c + amount_d.clone(), xor_balance_d);
        });
    }

    #[test]
    fn test_deducing_fee() {
        let mut ext = ExtBuilder::new(
            vec![
                (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
                (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
                (alice(), XST, balance!(0), AssetSymbol(b"XST".to_vec()), AssetName(b"SORA Synthetics".to_vec()), 18),
            ],
            vec![
                (alice(), XSTUSD, balance!(2000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
            ]
        )
        .build();
        ext.execute_with(|| {
            let (price_a, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(balance!(100)),
                true,
            )
            .unwrap();
            assert_approx_eq!(price_a.fee, balance!(0.00666), balance!(0.000001));
            assert_eq!(price_a.amount, balance!(0.640187763200000000));

            // mock uses conversion with fee
            let price_a_fee_without_fee = (
                FixedWrapper::from(price_a.fee) / balance!(0.993)
            ).into_balance();
            // convert fee back to output_asset_id (XST) for comparison
            let base_to_output: FixedWrapper = PriceTools::get_average_price(&XOR, &XST, common::PriceVariant::Buy)
                .expect("Failed to convert fee back to synthetic base asset")
                .into();
            // mock returns get_average_price with fee, we want no fee for this comparison
            let base_to_output_without_fee = base_to_output / balance!(0.993);
            let price_a_fee_in_synthetic_base_asset = (price_a_fee_without_fee * base_to_output_without_fee).into_balance();
            let (price_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(balance!(100)),
                false,
            )
            .unwrap();
            assert_eq!(price_b.fee, balance!(0));
            // more error, because more computations/roundings or larger coefficients
            assert_approx_eq!(price_b.amount, price_a_fee_in_synthetic_base_asset + price_a.amount, balance!(0.000001));

            let (price_a, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            )
            .unwrap();
            assert_approx_eq!(price_a.fee, balance!(1.04), balance!(0.001));
            assert_eq!(price_a.amount, balance!(15620.417281977813346827));

            let (price_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(balance!(100)),
                false,
            )
            .unwrap();
            assert_eq!(price_b.fee, balance!(0));
            assert_eq!(price_b.amount, balance!(15516.38530287984111));
        });
    }

    #[test]
    fn fees_for_equivalent_trades_should_match() {
        let mut ext = ExtBuilder::new(vec![
                (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
                (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
                (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
                (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
                (alice(), XST, balance!(0), AssetSymbol(b"XST".to_vec()), AssetName(b"SORA Synthetics".to_vec()), 18),
            ],
            vec![
                (alice(), XSTUSD, balance!(2000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
            ]
        )
        .build();
        ext.execute_with(|| {
            XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
            )
            .unwrap();

            // Buy
            let (price_a, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(balance!(100)),
                true,
            )
            .unwrap();
            let (price_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(price_a.amount.clone()),
                true,
            )
            .unwrap();
            assert_eq!(price_a.fee, price_b.fee);
            assert_approx_eq!(price_a.fee, balance!(0.00666), balance!(0.000000000000000002));

            // Sell
            let (price_c, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            )
            .unwrap();
            let (price_d, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_input(price_c.amount.clone()),
                true,
            )
            .unwrap();
            assert_eq!(price_c.fee, price_d.fee);
            assert_approx_eq!(price_c.fee, balance!(0.0066), balance!(0.00002));
        });
    }

    #[test]
    fn price_without_impact() {
        let mut ext = ExtBuilder::new(vec![
                (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
                (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
                (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
                (alice(), VAL, balance!(0), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
                (alice(), XST, balance!(0), AssetSymbol(b"XST".to_vec()), AssetName(b"SORA Synthetics".to_vec()), 18),
            ],
            vec![
                (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
            ]
        )
        .build();
        ext.execute_with(|| {
            // Buy with desired input
            let amount_a: Balance = balance!(200);
            let (quote_outcome_a, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(amount_a.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_a = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(amount_a.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_a.amount, quote_without_impact_a.amount);

            // Buy with desired output
            let amount_b: Balance = balance!(200);
            let (quote_outcome_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(amount_b.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_b = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(amount_b.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_b.amount, quote_without_impact_b.amount);

            // Sell with desired input
            let amount_c: Balance = balance!(1);
            let (quote_outcome_c, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_input(amount_c.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_c = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_input(amount_c.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_c.amount, quote_without_impact_c.amount);

            // Sell with desired output
            let amount_d: Balance = balance!(1);
            let (quote_outcome_d, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_output(amount_d.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_d = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_output(amount_d.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_d.amount, quote_without_impact_d.amount);
        });
    }

    #[test]
    fn exchange_synthetic_to_any_token_disallowed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let alice = alice();
            // add some reserves
            assert_noop!(XSTPool::exchange(&alice, &alice, &DEXId::Polkaswap, &XSTUSD, &DAI, SwapAmount::with_desired_input(balance!(1), 0)), Error::<Runtime>::CantExchange);
        });
    }

    #[test]
    fn set_synthetic_base_asset_floor_price_should_work() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let price_before = <XSTPool as GetMarketInfo<_>>::buy_price(&XST, &XSTUSD).expect("Failed to get buy price before setting floor price.");
            assert_eq!(price_before, fixed!(155.1638530287984111));

            XSTPool::set_synthetic_base_asset_floor_price(RuntimeOrigin::root(), balance!(200)).expect("Failed to set floor price.");
            let price_after = <XSTPool as GetMarketInfo<_>>::buy_price(&XST, &XSTUSD).expect("Failed to get buy price after setting floor price.");
            assert_eq!(price_after, fixed!(200));
        });
    }

    #[test]
    fn default_synthetic_base_asset_floor_price_should_be_greater_tha_zero() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert!(XSTPool::synthetic_base_asset_floor_price() > 0);
        });
    }

    #[test]
    fn enable_and_disable_synthetic_should_work() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let euro = SymbolName::from_str("EURO").expect("Failed to parse `EURO` as a symbol name");
            let alice = alice();

            OracleProxy::enable_oracle(RuntimeOrigin::root(), Oracle::BandChainFeed).expect("Failed to enable `Band` oracle");
            Band::add_relayers(RuntimeOrigin::root(), vec![alice.clone()])
                .expect("Failed to add relayers");
            Band::relay(RuntimeOrigin::signed(alice.clone()), vec![(euro.clone(), 1)], 0, 0)
                .expect("Failed to relay");

            let asset_id = AssetId32::<PredefinedAssetId>::from_synthetic_reference_symbol(&euro);

            XSTPool::register_synthetic_asset(
                RuntimeOrigin::root(),
                AssetSymbol("XSTEUR".into()),
                AssetName("XST Euro".into()),
                euro.clone(),
                fixed!(0),
            ).expect("Failed to register synthetic asset");

            let opt_xsteuro = XSTPool::enabled_symbols(&euro);
            assert!(opt_xsteuro.is_some());

            let xsteuro = opt_xsteuro.unwrap();
            assert_eq!(
                XSTPool::enabled_synthetics(&xsteuro).expect("Failed to get synthetic asset").reference_symbol,
                euro
            );

            XSTPool::disable_synthetic_asset(RuntimeOrigin::root(), xsteuro.clone())
                .expect("Failed to disable synthetic asset");

            assert!(XSTPool::enabled_synthetics(&xsteuro).is_none());
            assert!(XSTPool::enabled_symbols(&euro).is_none());

            XSTPool::enable_synthetic_asset(
                RuntimeOrigin::root(),
                asset_id,
                euro.clone(),
                fixed!(0),
            ).expect("Failed to enable synthetic asset");

            let opt_xsteuro = XSTPool::enabled_symbols(&euro);
            assert!(opt_xsteuro.is_some());

            let xsteuro = opt_xsteuro.unwrap();
            assert_eq!(
                XSTPool::enabled_synthetics(&xsteuro).expect("Failed to get synthetic asset").reference_symbol,
                euro
            );
        });
    }

    #[test]
    fn set_synthetic_fee_should_work() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let euro = SymbolName::from_str("EURO").expect("Failed to parse `EURO` as a symbol name");
            let alice = alice();

            OracleProxy::enable_oracle(RuntimeOrigin::root(), Oracle::BandChainFeed).expect("Failed to enable `Band` oracle");
            Band::add_relayers(RuntimeOrigin::root(), vec![alice.clone()])
                .expect("Failed to add relayers");
            Band::relay(RuntimeOrigin::signed(alice.clone()), vec![(euro.clone(), 1)], 0, 0)
                .expect("Failed to relay");

            XSTPool::register_synthetic_asset(
                RuntimeOrigin::root(),
                AssetSymbol("XSTEUR".into()),
                AssetName("XST Euro".into()),
                euro.clone(),
                fixed!(0),
            ).expect("Failed to register synthetic asset");

            let xsteuro = XSTPool::enabled_symbols(&euro).expect("Expected synthetic asset");
            let quote_amount = QuoteAmount::with_desired_input(balance!(100));

            let (swap_outcome_before, _) = XSTPool::quote(
                &DEXId::Polkaswap,
                &XST.into(),
                &xsteuro,
                quote_amount.clone(),
                true
            )
            .expect("Failed to quote XST -> XSTEURO ");
            assert_eq!(swap_outcome_before.fee, 0);


            assert_ok!(XSTPool::set_synthetic_asset_fee(
                RuntimeOrigin::root(),
                xsteuro.clone(),
                fixed!(0.5))
            );


            let (swap_outcome_after, _) = XSTPool::quote(
                &DEXId::Polkaswap,
                &XST.into(),
                &xsteuro,
                quote_amount,
                true
            )
            .expect("Failed to quote XST -> XSTEURO");

            let xst_to_xor_price = PriceTools::get_average_price(
                &XST.into(),
                &XOR.into(),
                PriceVariant::Buy,
            ).expect("Expected to calculate price XST->XOR");
            let expected_fee_amount = FixedWrapper::from(quote_amount.amount() / 2) * FixedWrapper::from(xst_to_xor_price);

            assert_eq!(swap_outcome_after.amount, swap_outcome_before.amount / 2);
            assert_eq!(swap_outcome_after.fee, expected_fee_amount.into_balance());
        });
    }

    #[test]
    fn should_disallow_invalid_fee_ratio() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let euro = SymbolName::from_str("EURO").expect("Failed to parse `EURO` as a symbol name");
            let alice = alice();

            OracleProxy::enable_oracle(RuntimeOrigin::root(), Oracle::BandChainFeed).expect("Failed to enable `Band` oracle");
            Band::add_relayers(RuntimeOrigin::root(), vec![alice.clone()])
                .expect("Failed to add relayers");
            Band::relay(RuntimeOrigin::signed(alice.clone()), vec![(euro.clone(), 1)], 0, 0)
                .expect("Failed to relay");

            assert_eq!(
                XSTPool::register_synthetic_asset(
                    RuntimeOrigin::root(),
                    AssetSymbol("XSTEUR".into()),
                    AssetName("XST Euro".into()),
                    euro.clone(),
                    fixed!(-0.1),
                ),
                Err(Error::<Runtime>::InvalidFeeRatio.into())
            );

            assert_eq!(
                XSTPool::register_synthetic_asset(
                    RuntimeOrigin::root(),
                    AssetSymbol("XSTEUR".into()),
                    AssetName("XST Euro".into()),
                    euro.clone(),
                    fixed!(1),
                ),
                Err(Error::<Runtime>::InvalidFeeRatio.into())
            )
        });
    }
}
