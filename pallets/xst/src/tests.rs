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
    use crate::{Error, Pallet, mock::*};
    use common::{self, AssetName, AssetSymbol, DEXId, FromGenericPair, LiquiditySource, USDT, VAL, XOR, XSTUSD, balance, fixed, prelude::{Balance, SwapAmount, QuoteAmount,}};
    use frame_support::assert_ok;
    use sp_arithmetic::traits::{Zero};
    use sp_runtime::DispatchError;

    type XSTPool = Pallet<Runtime>;

    /// Sets up the tech account so that mint permission is enabled
    fn xst_pool_init() -> Result<TechAccountId, DispatchError> {
        let xst_tech_account_id = TechAccountId::from_generic_pair(
            crate::TECH_ACCOUNT_PREFIX.to_vec(), crate::TECH_ACCOUNT_PERMISSIONED.to_vec()
        );
        Technical::register_tech_account_id(xst_tech_account_id.clone())?;
        XSTPool::set_tech_account_id(xst_tech_account_id.clone())?;

        Ok(xst_tech_account_id)
    }

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()), DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");

            // base case for buy
            assert_eq!(
                XSTPool::buy_price(&XOR, &XSTUSD, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(10128600) // (100000.0-100000.0*0.007)*102.0
            );
            assert_eq!(
                XSTPool::buy_price(&XOR, &XSTUSD, QuoteAmount::with_desired_input(balance!(1151397.348365215316854563)))
                    .expect("failed to calculate buy assets price"),
                fixed!(11367.783784187501894186) // (1151397.348365215316854563+1151397.348365215316854563*0.007)/102
            );

            // base case for sell
            assert_ok!(
                XSTPool::sell_price(&XOR, &XSTUSD, QuoteAmount::with_desired_output(balance!(100000)))
            );
            assert_ok!(
                XSTPool::sell_price(&XOR, &XSTUSD, QuoteAmount::with_desired_input(balance!(100000)))
            );

            // base case for sell with some reserves
            XSTPool::exchange(alice, alice, &DEXId::Polkaswap, &XSTUSD, &XOR, SwapAmount::with_desired_input(balance!(100000), 0)).expect("Failed to buy XOR.");
            assert_eq!(
                XSTPool::sell_price(&XOR, &XSTUSD, QuoteAmount::with_desired_output(balance!(50000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(493.651639910747783504) // (50000+50000*0.007)/102
            );
            assert_eq!(
                XSTPool::sell_price(&XOR, &XSTUSD, QuoteAmount::with_desired_input(balance!(15287.903511880099065528)))
                    .expect("failed to calculate buy assets price"),
                fixed!(1548450.595104287713951069) // (15287.903511880099065528-15287.903511880099065528*0.007)*102
            );
        });
    }

    #[test]
    fn calculate_price_for_boundary_values() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();

            let alice = alice();
            TradingPair::register(Origin::signed(alice.clone()), DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");
            // add some reserves
            XSTPool::exchange(&alice, &alice, &DEXId::Polkaswap, &XSTUSD, &XOR, SwapAmount::with_desired_input(balance!(1), 0)).expect("Failed to buy XOR.");

            common::assert_noop_transactional!(
                XSTPool::sell_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            common::assert_noop_transactional!(
                XSTPool::sell_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_eq!(
                XSTPool::sell_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
            assert_eq!(
                XSTPool::sell_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );

            common::assert_noop_transactional!(
                XSTPool::buy_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            common::assert_noop_transactional!(
                XSTPool::buy_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_eq!(
                XSTPool::buy_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_input(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
            assert_eq!(
                XSTPool::buy_price(
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
        });
    }

    #[test]
    fn should_set_new_reference_token() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(1), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(0), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, balance!(0), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            TradingPair::register(Origin::signed(alice()), DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");

            let price_a = XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &XSTUSD,
                    QuoteAmount::with_desired_output(balance!(1)),
                    true,
            )
                .unwrap();

            XSTPool::set_reference_asset(Origin::root(), DAI).expect("Failed to set new reference asset.");

            let price_b = XSTPool::quote(
                    &DEXId::Polkaswap.into(),
                    &XSTUSD,
                    &XOR,
                    QuoteAmount::with_desired_output(balance!(1)),
                    true,
            )
                .unwrap();

            assert_ne!(price_a, price_b);
        });
    }

    #[test]
    fn similar_returns_should_be_identical() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(4000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, balance!(22600), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();
            TradingPair::register(Origin::signed(alice()), DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");

            // Buy with desired input
            let amount_a: Balance = balance!(2000);
            let quote_outcome_a = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
                true,
            )
            .unwrap();

            let exchange_outcome_a = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
            )
            .unwrap();

            let xstusd_balance_a = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_a = Assets::free_balance(&XOR, &alice()).unwrap();

            assert_eq!(quote_outcome_a.amount, exchange_outcome_a.amount);
            assert_eq!(exchange_outcome_a.amount, xor_balance_a);
            assert_eq!(xstusd_balance_a, balance!(20600));

            // Buy with desired output
            let amount_b: Balance = balance!(200);
            let quote_outcome_b = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
                true,
            )
            .unwrap();

            let exchange_outcome_b = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
            )
            .unwrap();

            let xstusd_balance_b = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_b = Assets::free_balance(&XOR, &alice()).unwrap();

            assert_eq!(quote_outcome_b.amount, exchange_outcome_b.amount);
            assert_eq!(xor_balance_a + amount_b.clone(), xor_balance_b);
            assert_eq!(xstusd_balance_b, balance!(281.845536609829488538));

            // Sell with desired input
            let amount_c: Balance = balance!(205);
            let quote_outcome_c = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_input(amount_c.clone()),
                true,
            )
            .unwrap();

            let exchange_outcome_c = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
            )
            .unwrap();

            let xstusd_balance_c = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_c = Assets::free_balance(&XOR, &alice()).unwrap();

            assert_eq!(quote_outcome_c.amount, exchange_outcome_c.amount);
            assert_eq!(xstusd_balance_b + exchange_outcome_c.amount, xstusd_balance_c);
            assert_eq!(xor_balance_b - amount_c.clone(), xor_balance_c.clone());

            // Sell with desired output
            let amount_d: Balance = balance!(100);
            let quote_outcome_d = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(amount_d.clone()),
                true,
            )
            .unwrap();
            let exchange_outcome_d = XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
            )
            .unwrap();
            let xstusd_balance_d = Assets::free_balance(&XSTUSD, &alice()).unwrap();
            let xor_balance_d = Assets::free_balance(&XOR, &alice()).unwrap();
            assert_eq!(quote_outcome_d.amount, exchange_outcome_d.amount);
            assert_eq!(xstusd_balance_c - quote_outcome_d.amount, xstusd_balance_d);
            assert_eq!(xor_balance_c + amount_d.clone(), xor_balance_d);
        });
    }

    #[test]
    fn test_deducing_fee() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), XSTUSD, balance!(2000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
        ])
            .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();
            TradingPair::register(Origin::signed(alice()), DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");

            let price_a = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_input(balance!(100)),
                true,
            )
            .unwrap();
            assert_eq!(price_a.fee, balance!(0.002961909839464486));
            assert_eq!(price_a.amount, balance!(0.984341369982031081));

            let price_b = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_input(balance!(100)),
                false,
            )
            .unwrap();
            assert_eq!(price_b.fee, balance!(0));
            assert_eq!(price_b.amount, price_a.fee + price_a.amount);

            let price_a = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            )
            .unwrap();
            assert_eq!(price_a.fee, balance!(0.300902708124373119));
            assert_eq!(price_a.amount, balance!(10159.077231695085255731));

            let price_b = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(balance!(100)),
                false,
            )
            .unwrap();
            assert_eq!(price_b.fee, balance!(0));
            assert_eq!(price_b.amount, balance!(10128.6));
        });
    }

    #[test]
    fn fees_for_equivalent_trades_should_match() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, balance!(2000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");

            XSTPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
            )
            .unwrap();

            // Buy
            let price_a = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_input(balance!(100)),
                true,
            )
            .unwrap();
            let price_b = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(price_a.amount.clone()),
                true,
            )
            .unwrap();
            assert_eq!(price_a.fee, price_b.fee);
            assert_eq!(price_a.fee, balance!(0.002961909839464486));

            // Sell
            let price_c = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            )
            .unwrap();
            let price_d = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_input(price_c.amount.clone()),
                true,
            )
            .unwrap();
            assert_eq!(price_c.fee, price_d.fee);
            assert_eq!(price_c.fee, balance!(0.002970822306383637));
        });
    }

    #[test]
    fn price_without_impact() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(0), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"SORA Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");

            // Buy with desired input
            let amount_a: Balance = balance!(200);
            let quote_outcome_a = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_a = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_a.amount, quote_without_impact_a.amount);

            // Buy with desired output
            let amount_b: Balance = balance!(200);
            let quote_outcome_b = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_b = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XSTUSD,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_b.amount, quote_without_impact_b.amount);

            // Sell with desired input
            let amount_c: Balance = balance!(1);
            let quote_outcome_c = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_input(amount_c.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_c = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_input(amount_c.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_c.amount, quote_without_impact_c.amount);

            // Sell with desired output
            let amount_d: Balance = balance!(1);
            let quote_outcome_d = XSTPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_output(amount_d.clone()),
                true,
            )
            .unwrap();
            let quote_without_impact_d = XSTPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XOR,
                &XSTUSD,
                QuoteAmount::with_desired_output(amount_d.clone()),
                true,
            )
            .unwrap();
            assert_eq!(quote_outcome_d.amount, quote_without_impact_d.amount);
        });
    }

    #[test]
    fn exchange_synthesic_to_any_token_disallowed() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = xst_pool_init().unwrap();

            let alice = alice();
            TradingPair::register(Origin::signed(alice.clone()), DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            XSTPool::initialize_pool_unchecked(XSTUSD, false).expect("Failed to initialize pool.");
            // add some reserves
            common::assert_noop_transactional!(XSTPool::exchange(&alice, &alice, &DEXId::Polkaswap, &XSTUSD, &DAI, SwapAmount::with_desired_input(balance!(1), 0)), Error::<Runtime>::CantExchange);
        });
    }

}
