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
    use crate::{
        mock::*,
        test_utils::{relay_new_symbol, relay_symbol},
        Error, Pallet,
    };
    use band::FeeCalculationParameters;
    use common::alt::{DiscreteQuotation, Fee, SideAmount, SwapChunk, SwapLimits};
    use common::{
        self, assert_approx_eq, balance, fixed,
        prelude::{Balance, FixedWrapper, QuoteAmount, SwapAmount},
        AssetId32, AssetInfoProvider, AssetName, AssetSymbol, DEXId, GetMarketInfo,
        LiquiditySource, PredefinedAssetId, PriceVariant, DAI, USDT, VAL, XOR, XST, XSTUSD,
    };
    use frame_support::traits::Hooks;
    use frame_support::{assert_noop, assert_ok};
    use frame_system::pallet_prelude::BlockNumberFor;
    use sp_arithmetic::traits::Zero;
    use sp_std::collections::vec_deque::VecDeque;

    type XSTPool = Pallet<Runtime>;
    type PriceTools = price_tools::Pallet<Runtime>;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // base case for buy
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (X)
            // 1 XSTUSD = 1 DAI (S)
            // amount out = 100_000 XST (A_out)
            // amount in = (A_out * X) / S = (100_000 * 220) / 1 = 22_000_000 XSTUSD (A_in)
            assert_eq!(
                XSTPool::buy_price(&XST, &XSTUSD, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(22000000.0) 
            );

            // base case for sell
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (X)
            // 1 XSTUSD = 1 DAI (S)
            // amount out = 100_000 XSTUSD (A_out)
            // amount in = (A_out * S) / X = (100_000 * 1) / 150 = 666.(6) XST (A_in) 
            assert_eq!(
                XSTPool::sell_price(&XST, &XSTUSD, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(666.666666666666666933),
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
            // Fee ratio should be greater than 0 during this test
            assert!(XSTPool::enabled_synthetics(&XSTUSD).unwrap().fee_ratio != fixed!(0));
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
            let xst_balance_a = Assets::free_balance(&XST, &alice()).unwrap();

            assert_eq!(quote_outcome_a.amount, exchange_outcome_a.amount);
            assert_eq!(quote_outcome_a.fee, exchange_outcome_a.fee);
            assert_eq!(exchange_outcome_a.amount, xst_balance_a);
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
            let xst_balance_b = Assets::free_balance(&XST, &alice()).unwrap();

            assert_eq!(quote_outcome_b.amount, exchange_outcome_b.amount);
            assert_eq!(quote_outcome_b.fee, exchange_outcome_b.fee);
            assert_eq!(xst_balance_a + amount_b.clone(), xst_balance_b);
            assert_eq!(xstusd_balance_b, xstusd_balance_a - quote_outcome_b.amount);

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
            let xst_balance_c = Assets::free_balance(&XST, &alice()).unwrap();

            assert_eq!(quote_outcome_c.amount, exchange_outcome_c.amount);
            assert_eq!(quote_outcome_c.fee, exchange_outcome_c.fee);
            assert_eq!(xstusd_balance_b + exchange_outcome_c.amount, xstusd_balance_c);
            assert_eq!(xst_balance_b - amount_c.clone(), xst_balance_c.clone());

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
            let xst_balance_d = Assets::free_balance(&XST, &alice()).unwrap();
            assert_eq!(quote_outcome_d.amount, exchange_outcome_d.amount);
            assert_eq!(quote_outcome_d.fee, exchange_outcome_d.fee);
            assert_eq!(xstusd_balance_c - quote_outcome_d.amount, xstusd_balance_d);
            assert_eq!(xst_balance_c + amount_d.clone(), xst_balance_d);
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

            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (X)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount in = 100 XSTUSD (A_in)
            // amount out = (A_in * S) / X = (100 * 1) / 220 = 0.(45) XST (A_out)
            // deduced fee = A_out * F = 0.(45) * 0.00666 = 0.0030(27) XST (F_xst)
            // deduced fee in XOR = F_xst / X_b = 0.0030(27) / 0.6 = 0.0060(54) XOR (since we are buying XOR with XST)
            assert_approx_eq!(price_a.fee, balance!(0.006054545454545454), 2);
            // amount out with deduced fee = A_out - F_xst = 0.(45) - 0.0030(27) = 0.4515(18) XST
            assert_approx_eq!(price_a.amount, balance!(0.451518181818181818), 2);

            let (price_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(balance!(100)),
                false,
            )
            .unwrap();
            assert_eq!(price_b.fee, balance!(0));
            // we need to convert XOR fee back to XST 
            let xst_fee = (FixedWrapper::from(price_a.fee)*balance!(0.5)).into_balance();
            assert_approx_eq!(price_b.amount, xst_fee + price_a.amount, 2);

            let (price_a, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            )
            .unwrap();

            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (X)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount out = 100 XST (A_in)
            // deduced fee = A_out / (1 - F_r) - A_out = 100 / (1 - 0.00666) - 100 = 0.670465298890611 XST (F_xst)
            // amount in = ((A_out + F_xst) * X) / S = ((100 + 0.670465298890611) * 220) / 1 = 22147.5023657559344 XSTUSD (A_in)
            // deduced fee in XOR = F_xst / X_b = 0.670465298890611 / 0.5 = 1.340930597781222944 XOR (since we are buying XOR with XST)
            assert_approx_eq!(price_a.fee, balance!(1.340930597781222944), 1000);
            assert_approx_eq!(price_a.amount, balance!(22147.5023657559344), 1000_000);

            let (price_b, _) = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(balance!(100)),
                false,
            )
            .unwrap();
            assert_eq!(price_b.fee, balance!(0));
            // amount out = A_out * X / S = 100 * 220 / 1 = 22000 XSTUSD
            assert_eq!(price_b.amount, balance!(22000));
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
            
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (X)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount in = 100 XSTUSD (A_in)
            // amount out = (A_in * S) / X = (100 * 1) / 220 = 0.(45) XST (A_out)
            // deduced fee = A_out * F = 0.(45) * 0.00666 = 0.0030(27) XST (F_xst)
            // deduced fee in XOR = F_xst / X_b = 0.0030(27) / 0.5 = 0.0060(54) XOR (since we are buying XOR with XST)
            assert_approx_eq!(price_a.fee, balance!(0.006054545454545454), 2);

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

            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (X)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount out = 100 XSTUSD (A_out)
            // amount in = (A_out * S) / X = (100 * 1) / 150 = 0.(6) XST (A_in)
            // deduced fee = A_in / (1 - F) - A_in = 0.(6) / (1 - 0.00666) - A_in ~ 0.004469768659270743 XST (F_xst)
            // deduced fee in XOR = F_xst / X_b = 0.004469768659270743 / 0.5 ~ 0.008939537319 XOR
            // (since we are buying XOR with XST)
            assert_approx_eq!(price_c.fee, balance!(0.008939537318541485), 2);
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
            // 1 XOR = 0.5 XST in sell case
            // 1 XOR = 110 DAI in buy case
            // 1 XST = 110/0.5 = 220 DAI
            assert_eq!(price_before, fixed!(220.)); 

            XSTPool::set_synthetic_base_asset_floor_price(RuntimeOrigin::root(), balance!(300)).expect("Failed to set floor price.");
            let price_after = <XSTPool as GetMarketInfo<_>>::buy_price(&XST, &XSTUSD).expect("Failed to get buy price after setting floor price.");
            assert_eq!(price_after, fixed!(300));
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
            let euro = relay_new_symbol("EURO", 2_000_000_000);

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
            assert!(XSTPool::enabled_symbols(&euro).is_some());

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
            let euro = relay_new_symbol("EURO", 2_000_000_000);

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
            
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (X)
            // 1 XSTEURO = 2 DAI (S)
            // fee ratio for XSTUSD = 0. (F_r)
            // amount in = 100 XST (A_in)
            // amount out = (A_in * X) / S = (100 * 150) / 2 = 7500 XSTEURO (A_out)
            assert_approx_eq!(swap_outcome_before.amount, balance!(7500), 10000);
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
            let euro = relay_new_symbol("EURO", 2_000_000_000);
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
            );

            assert_eq!(
                XSTPool::set_synthetic_asset_fee(
                    RuntimeOrigin::root(),
                    XSTUSD.into(),
                    fixed!(-0.1),
                ),
                Err(Error::<Runtime>::InvalidFeeRatio.into())
            );

            assert_eq!(
                XSTPool::set_synthetic_asset_fee(
                    RuntimeOrigin::root(),
                    XSTUSD.into(),
                    fixed!(1),
                ),
                Err(Error::<Runtime>::InvalidFeeRatio.into())
            );
        });
    }

    #[test]
    fn should_disallow_invalid_resulting_fee_ratio() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            Band::set_dynamic_fee_parameters(
                RuntimeOrigin::root(),
                FeeCalculationParameters::new(
                    fixed!(0),
                    fixed!(0.1),
                    fixed!(0.05)
                )
            ).expect("Expected to set the dynamic fee calculation paramteres for the Band pallet");
            let euro = relay_new_symbol("EURO", 2_000_000_000);
            relay_symbol(euro.clone(), 3_000_000_000);

            XSTPool::register_synthetic_asset(
                RuntimeOrigin::root(),
                AssetSymbol("XSTEUR".into()),
                AssetName("XST Euro".into()),
                euro.clone(),
                fixed!(0.7),
            ).expect("Failed to register synthetic asset");

            let xsteuro = XSTPool::enabled_symbols(&euro).expect("Expected synthetic asset");
            let quote_amount = QuoteAmount::with_desired_input(balance!(100));

            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap,
                    &XST.into(),
                    &xsteuro,
                    quote_amount.clone(),
                    true
                ),
                Error::<Runtime>::InvalidFeeRatio
            );
        });
    }

    #[test]
    fn dynamic_fee_should_be_taken_into_account() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            Band::set_dynamic_fee_parameters(
                RuntimeOrigin::root(),
                FeeCalculationParameters::new(
                    fixed!(0),
                    fixed!(0.1),
                    fixed!(0.05)
                )
            ).expect("Expected to set the dynamic fee calculation paramteres for the Band pallet");
            let euro = relay_new_symbol("EURO", 2_000_000_000);
            relay_symbol(euro.clone(), 3_000_000_000);

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
            
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (X)
            // 1 XSTEURO = 3 DAI (S)
            // fee ratio for XSTEURO = 0.3 (F_r)
            // amount in = 100 XST (A_in)
            // amount out = (A_in * X * (1 - F_r)) / S = (100 * 150 * 0.7) / 3 = 3500 XSTEURO (A_out)
            // fee = F_xst / X_b = 0.3 * 100 / 0.5 = 60 XOR
            assert_approx_eq!(swap_outcome_before.amount, balance!(3500), 10000);
            assert_approx_eq!(swap_outcome_before.fee, balance!(60), 10000);

            assert_ok!(XSTPool::set_synthetic_asset_fee(
                RuntimeOrigin::root(),
                xsteuro.clone(),
                fixed!(0.3))
            );

            let (swap_outcome_after, _) = XSTPool::quote(
                &DEXId::Polkaswap,
                &XST.into(),
                &xsteuro,
                quote_amount,
                true
            )
            .expect("Failed to quote XST -> XSTEURO");

            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (X)
            // 1 XSTEURO = 3 DAI (S)
            // fee ratio for XSTEURO = 0.6 (F_r) <- dynamic fee + synthetic fee
            // amount in = 100 XST (A_in)
            // amount out = (A_in * X * (1 - F_r)) / S = (100 * 150 * 0.4) / 3 = 2000 XSTEURO (A_out)
            // fee = F_xst / X_b = 0.6 * 100 / 0.5 = 120 XOR
            assert_approx_eq!(swap_outcome_after.amount, balance!(2000), 10000);
            assert_approx_eq!(swap_outcome_after.fee, balance!(120), 10000);
        });
    }

    #[test]
    fn should_disallow_xst_amount_exceeding_limit() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (XST_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (XST_b)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount_out = 10_000_000 XST
            // amount in = amount_out / (1 - F_r) * XST_b / S = 10_000_000 / (1 - 0.00666) * 220 / 1 = 2214750236.57559344 (XSTUSD)
            let amount_a: Balance = balance!(2214750236.575593452663369226) + 1;
            // Buy with desired input
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_input(amount_a.clone()),
                    true,
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );

            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );

            // Buy with desired output
            let amount_b: Balance = balance!(10000000) + 1;
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(amount_b.clone()),
                    true,
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );

            // Sell with desired input
            let amount_c: Balance = balance!(10000000) + 1;
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(amount_c.clone()),
                    true,
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );

            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (XST_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (XST_b)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount_in = 10_000_000 XST
            // amount out = amount_in * (1 - F_r) * XST_s / S = 10_000_000 * (1 - 0.00666) * 150 / 1 = 1490009999.999999999818062202 (XSTUSD)
            // Sell with desired output
            let amount_d: Balance = balance!(1490009999.999999999818062202) + 1;
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(amount_d.clone()),
                    true,
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
                ),
                Error::<Runtime>::SyntheticBaseBuySellLimitExceeded
            );
        });
    }

    #[test]
    fn should_allow_xst_amount_near_limit() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {

            assets::Pallet::<Runtime>::update_balance(
                RuntimeOrigin::root(), alice(), XSTUSD.into(), balance!(13000000000) as i128)
                .expect("Expected to update Alice XSTUSD balance");
            assets::Pallet::<Runtime>::update_balance(
                RuntimeOrigin::root(), alice(), XST.into(), balance!(13000000000) as i128)
                .expect("Expected to update Alice XST balance");
            
            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (XST_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (XST_b)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount_out = 10_000_000 XST
            // amount in = amount_out / (1 - F_r) * XST_b / S = 10_000_000 / (1 - 0.00666) * 220 / 1 = 2214750236.575593452663369226 (XSTUSD)

            // precision is 10^-9
            let amount_a: Balance = balance!(2214750236.575593452663369226) - 1_000_000_000;
            // Buy with desired input
            assert_ok!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_input(amount_a.clone()),
                    true,
                )
            );
            assert_ok!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
                )
            );

            // Buy with desired output
            let amount_b: Balance = balance!(10000000);
            assert_ok!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(amount_b.clone()),
                    true,
                )
            );
            assert_ok!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
                )
            );

            // Sell with desired input
            let amount_c: Balance = balance!(10000000);
            assert_ok!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(amount_c.clone()),
                    true,
                )
            );
            assert_ok!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
                )
            );

            // 1 XOR = 0.5 XST in sell case (X_s)
            // 1 XOR = 0.6 XST in buy case (X_b)
            // 1 XOR = 110 DAI in buy case (D_b) (default reference unit in xstPool)
            // 1 XOR = 90 DAI in sell case (D_s)
            // 1 XST sell price = D_s/X_b = 90/0.6 = 150 DAI (XST_s)
            // 1 XST buy price = D_b/X_s = 110/0.5 = 220 DAI (XST_b)
            // 1 XSTUSD = 1 DAI (S)
            // fee ratio for XSTUSD = 0.00666 (F_r)
            // amount_in = 10_000_000 XST
            // amount out = amount_in * (1 - F_r) * XST_s / S = 10_000_000 * (1 - 0.00666) * 150 / 1 = 1490009999.999999999818062202 (XSTUSD)
            
            // precision is 10^-9
            let amount_d: Balance = balance!(1490009999.999999999818062202) - 1_000_000_000;
            // Sell with desired output
            assert_ok!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(amount_d.clone()),
                    true,
                )
            );
            assert_ok!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
                )
            );
        });
    }

    #[test]
    fn should_disallow_zero_amounts_in_quote_exchange() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            // Buy with desired input
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_input(0),
                    true,
                ),
                Error::<Runtime>::PriceCalculationFailed
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_input(0, Balance::zero()),
                ),
                Error::<Runtime>::PriceCalculationFailed
            );

            // Buy with desired output
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(0),
                    true,
                ),
                Error::<Runtime>::PriceCalculationFailed
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_output(0, Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed
            );

            // Sell with desired input
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(0),
                    true,
                ),
                Error::<Runtime>::PriceCalculationFailed
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    SwapAmount::with_desired_input(0, Balance::zero()),
                ),
                Error::<Runtime>::PriceCalculationFailed
            );

            // Sell with desired output
            assert_noop!(
                XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(0),
                    true,
                ),
                Error::<Runtime>::PriceCalculationFailed
            );
            assert_noop!(
                XSTPool::exchange(
                    &alice(),
                    &alice(),
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    SwapAmount::with_desired_output(0, Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed
            );
        });
    }

    #[test]
    fn remove_synthetic_should_work() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            let euro = relay_new_symbol("EURO", 2_000_000_000);

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

            XSTPool::remove_synthetic_asset(RuntimeOrigin::root(), xsteuro.clone())
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
    fn disable_symbol_and_enable_synthetic_should_work() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            System::set_block_number(1u64);

            let euro = relay_new_symbol("EURO", 2_000_000_000);
            let asset_id = AssetId32::<PredefinedAssetId>::from_synthetic_reference_symbol(&euro);
            XSTPool::register_synthetic_asset(
                RuntimeOrigin::root(),
                AssetSymbol("XSTEUR".into()),
                AssetName("XST Euro".into()),
                euro.clone(),
                fixed!(0),
            ).expect("Failed to register synthetic asset");
            assert!(XSTPool::enabled_synthetics(&asset_id).is_some());
            assert!(XSTPool::enabled_symbols(&euro).is_some());

            let new_block = 1u64 + GetBandRateStaleBlockPeriod::get();
            System::set_block_number(new_block);
            <Band as Hooks<BlockNumberFor<Runtime>>>::on_initialize(new_block);

            assert!(XSTPool::enabled_synthetics(&asset_id).is_none());
            assert!(XSTPool::enabled_symbols(&euro).is_some());

            XSTPool::enable_synthetic_asset(
                RuntimeOrigin::root(),
                asset_id,
                euro.clone(),
                fixed!(0),
            ).expect("Failed to enable synthetic asset");

            let xsteuro = XSTPool::enabled_symbols(&euro)
                .expect("Expected to get a synthetic asset linked to EURO");

            assert_eq!(
                XSTPool::enabled_synthetics(&xsteuro).expect("Failed to get synthetic asset").reference_symbol,
                euro
            );
        });
    }

    #[test]
    fn check_empty_step_quote() {
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
        assert_eq!(
            XSTPool::step_quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_input(balance!(0)),
                10,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation::new()
        );

        assert_eq!(
            XSTPool::step_quote(
                &DEXId::Polkaswap.into(),
                &XST,
                &XSTUSD,
                QuoteAmount::with_desired_output(balance!(0)),
                10,
                false
            )
            .unwrap()
            .0,
            DiscreteQuotation::new()
        );

        assert_eq!(
            XSTPool::step_quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_input(balance!(0)),
                10,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation::new()
        );

        assert_eq!(
            XSTPool::step_quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XST,
                QuoteAmount::with_desired_output(balance!(0)),
                10,
                false
            )
            .unwrap()
            .0,
            DiscreteQuotation::new()
        );
    });
    }

    #[test]
    fn check_step_quote_with_zero_samples_count() {
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
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_input(balance!(100)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(balance!(100), balance!(0.454545454545454545), Fee::zero())]),
                    limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(2200000000))), None)
                }
            );
            
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(balance!(100)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(balance!(22000), balance!(100), Fee::zero())]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(10000000))), None)
                }
            );
            
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(balance!(100)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(balance!(100), balance!(14999.999999999999994), Fee::zero())]),
                    limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(10000000))), None)
                }
            );

            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(100)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(balance!(0.666666666666666666), balance!(100), Fee::zero())]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1499999999.9999999994))), None)
                }
            );
        });
    }

    #[test]
    fn check_step_quote_without_fee() {
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
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_input(balance!(100)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545454), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(0.045454545454545459), Fee::zero()),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(2200000000))), None)
                }
            );
            
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(balance!(100)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(2200), balance!(10), Fee::zero()),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(10000000))), None)
                }
            );
            
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(balance!(100)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                        SwapChunk::new(balance!(10), balance!(1499.9999999999999994), Fee::zero()),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(10000000))), None)
                }
            );

            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(100)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666666), balance!(10), Fee::zero()),
                        SwapChunk::new(balance!(0.066666666666666672), balance!(10), Fee::zero()),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1499999999.9999999994))), None)
                }
            );
        });
    }

    #[test]
    fn check_step_quote_with_fee() {
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
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_input(balance!(100)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818181), Fee::xst(balance!(0.000605454545454545))),
                        SwapChunk::new(balance!(10), balance!(0.045151818181818189), Fee::xst(balance!(0.000605454545454549))),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(2214750236.57559345239293684))), None)
                }
            );
            
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(balance!(100)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122294))),
                        SwapChunk::new(balance!(2214.750236575593452384), balance!(10), Fee::xst(balance!(0.134093059778122298))),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(10000000))), None)
                }
            );
            
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(balance!(100)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999403), Fee::xst(balance!(0.1332))),
                        SwapChunk::new(balance!(10), balance!(1490.009999999999999412), Fee::xst(balance!(0.1332))),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(10000000))), None)
                }
            );

            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(100)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.06711364353259374), balance!(10), Fee::xst(balance!(0.000893953731854148))),
                        SwapChunk::new(balance!(0.067113643532593749), balance!(10), Fee::xst(balance!(0.000893953731854154))),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1490009999.999999999403996))), None)
                }
            );
        });
    }

    fn compare_quotes(
        dex_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) {
        let (step_quote_input, step_quote_output, step_quote_fee) = XSTPool::step_quote(
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            10,
            deduce_fee,
        )
        .unwrap()
        .0
        .chunks
        .iter()
        .fold((balance!(0), balance!(0), Fee::zero()), |acc, item| {
            (acc.0 + item.input, acc.1 + item.output, acc.2.saturating_add(item.fee))
        });

        let quote_result =
            XSTPool::quote(dex_id, input_asset_id, output_asset_id, amount, deduce_fee)
                .unwrap()
                .0;

        let (quote_input, quote_output, quote_fee) = match amount {
            QuoteAmount::WithDesiredInput { desired_amount_in } => {
                (desired_amount_in, quote_result.amount, quote_result.fee)
            }
            QuoteAmount::WithDesiredOutput { desired_amount_out } => {
                (quote_result.amount, desired_amount_out, quote_result.fee)
            }
        };

        assert_eq!(step_quote_input, quote_input);
        assert_eq!(step_quote_output, quote_output);
        assert_eq!(step_quote_fee, Fee::xst(quote_fee));
    }

    #[test]
    fn check_step_quote_equal_with_qoute() {
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
            compare_quotes(&DEXId::Polkaswap, &XSTUSD, &XST, QuoteAmount::with_desired_input(balance!(100)), false);
            compare_quotes(&DEXId::Polkaswap, &XSTUSD, &XST, QuoteAmount::with_desired_output(balance!(100)), false);
            
            compare_quotes(&DEXId::Polkaswap, &XST, &XSTUSD, QuoteAmount::with_desired_input(balance!(100)), false);
            compare_quotes(&DEXId::Polkaswap, &XST, &XSTUSD, QuoteAmount::with_desired_output(balance!(100)), false);

            compare_quotes(&DEXId::Polkaswap, &XSTUSD, &XST, QuoteAmount::with_desired_input(balance!(100)), true);
            compare_quotes(&DEXId::Polkaswap, &XSTUSD, &XST, QuoteAmount::with_desired_output(balance!(100)), true);

            compare_quotes(&DEXId::Polkaswap, &XST, &XSTUSD, QuoteAmount::with_desired_input(balance!(100)), true);
            compare_quotes(&DEXId::Polkaswap, &XST, &XSTUSD, QuoteAmount::with_desired_output(balance!(100)), true);
        });
    }

    #[test]
    fn check_step_quote_exceeds_limit_without_fee() {
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
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(balance!(123456789123456789)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                        SwapChunk::new(balance!(220000000.000000022), balance!(1000000), Fee::zero()),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(10000000))), None)
                }
            );

            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(123456789123456789)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                        SwapChunk::new(balance!(1000000), balance!(149999999.99999999994), Fee::zero()),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1499999999.9999999994))), None)
                }
            );
        });
    }

    #[test]
    fn check_step_quote_exceeds_limit_with_fee() {
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
            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XST,
                    QuoteAmount::with_desired_output(balance!(123456789123456789)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691169), balance!(1000000), Fee::xst(balance!(13409.305869196850905820))),
                        SwapChunk::new(balance!(221475023.657559354157691173), balance!(1000000), Fee::xst(balance!(13409.305869196850905827))),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(10000000))), None)
                }
            );

            assert_eq!(
                XSTPool::step_quote(
                    &DEXId::Polkaswap.into(),
                    &XST,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(123456789123456789)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522971))),
                        SwapChunk::new(balance!(1000000), balance!(149000999.99999999994), Fee::xst(balance!(13319.999999161717522976))),
                    ]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1490009999.999999999403996))), None)
                }
            );
        });
    }
}
