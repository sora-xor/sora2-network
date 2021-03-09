#[rustfmt::skip]
mod tests {
    use crate::{mock::*, DistributionAccountData, DistributionAccounts, Error};
    use common::{
        self, balance, fixed, fixed_wrapper, Fixed, fixnum::ops::One as _, fixnum::ops::Zero as _,
        prelude::{Balance, SwapAmount, SwapOutcome, QuoteAmount, FixedWrapper,},
        AssetSymbol, DEXId, LiquiditySource, TechPurpose, USDT, VAL, XOR, PSWAP, LiquiditySourceFilter,
    };
    use pswap_distribution::OnPswapBurned;
    use liquidity_proxy::LiquidityProxyTrait;
    use frame_support::{assert_err, assert_noop, assert_ok};
    use frame_support::storage::{with_transaction, TransactionOutcome};
    use sp_arithmetic::traits::{Zero};
    use sp_runtime::DispatchError;
    use orml_traits::MultiCurrency;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");

            // base case for buy
            assert_eq!(
                MBCPool::buy_function(&XOR, Fixed::ZERO)
                    .expect("failed to calculate buy price"),
                    fixed!(536.574420344053851907)
            );
            assert_eq!(
                MBCPool::buy_price(&XOR, &VAL, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(1151397.348365215316854563)
            );
            assert_eq!(
                MBCPool::buy_price(&XOR, &VAL, QuoteAmount::with_desired_input(balance!(1151397.348365215316854563)))
                    .expect("failed to calculate buy assets price"),
                fixed!(99999.999999999999999958) // TODO: try to improve precision
            );

            // base case for sell with empty reserves
            assert_eq!(
                MBCPool::sell_function(&XOR, Fixed::ZERO)
                    .expect("failed to calculate sell price"),
                    fixed!(429.259536275243081525)
            );
            assert_noop!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_output(balance!(100000))),
                Error::<Runtime>::NotEnoughReserves,
            );
            assert_noop!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_input(balance!(100000))),
                Error::<Runtime>::NotEnoughReserves,
            );

            // base case for sell with some reserves
            MBCPool::exchange(alice, alice, &DEXId::Polkaswap, &VAL, &XOR, SwapAmount::with_desired_input(balance!(100000), 0)).expect("Failed to buy XOR.");
            assert_eq!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_output(balance!(50000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(15287.903511880099065528)
            );
            assert_eq!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_input(balance!(15287.903511880099065528)))
                    .expect("failed to calculate buy assets price"),
                fixed!(49999.999999999999999999) // TODO: improve precision
            );
        });
    }

    #[test]
    fn calculate_price_for_boundary_values() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = alice();
            TradingPair::register(Origin::signed(alice.clone()) ,DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            // add some reserves
            MBCPool::exchange(&alice, &alice, &DEXId::Polkaswap, &VAL, &XOR, SwapAmount::with_desired_input(balance!(1), 0)).expect("Failed to buy XOR.");

            assert_noop!(
                MBCPool::sell_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_noop!(
                MBCPool::sell_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::NotEnoughReserves,
            );
            assert_eq!(
                MBCPool::sell_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_input(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
            assert_eq!(
                MBCPool::sell_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_output(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );

            assert_noop!(
                MBCPool::buy_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_noop!(
                MBCPool::buy_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::PriceCalculationFailed,
            );
            assert_eq!(
                MBCPool::buy_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_input(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
            assert_eq!(
                MBCPool::buy_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_output(Balance::zero()),
                ),
                Ok(fixed!(0)),
            );
        });
    }

    fn bonding_curve_pool_init(
        initial_reserves: Vec<(AssetId, Balance)>,
    ) -> Result<
        DistributionAccounts<DistributionAccountData<<Runtime as technical::Trait>::TechAccountId>>,
        DispatchError,
    > {
        let bonding_curve_tech_account_id = TechAccountId::Pure(
            DEXId::Polkaswap,
            TechPurpose::Identifier(b"bonding_curve_tech_account_id".to_vec()),
        );
        Technical::register_tech_account_id(bonding_curve_tech_account_id.clone())?;
        MBCPool::set_reserves_account_id(bonding_curve_tech_account_id.clone())?;
        for (asset_id, balance) in initial_reserves {
            Technical::mint(&asset_id, &bonding_curve_tech_account_id, balance)?;
        }

        let val_holders_coefficient = fixed_wrapper!(0.5);
        let val_holders_xor_alloc_coeff = val_holders_coefficient.clone() * fixed_wrapper!(0.9);
        let val_holders_buy_back_coefficient =
            val_holders_coefficient.clone() * fixed_wrapper!(0.1);
        let projects_coefficient: FixedWrapper = fixed_wrapper!(1) - val_holders_coefficient;
        let projects_sora_citizens_coeff: FixedWrapper = projects_coefficient.clone() * fixed_wrapper!(0.01);
        let projects_stores_and_shops_coeff: FixedWrapper = projects_coefficient.clone()* fixed_wrapper!(0.04);
        let projects_parliament_and_development_coeff: FixedWrapper = projects_coefficient.clone() * fixed_wrapper!(0.05);
        let projects_other_coeff: FixedWrapper = projects_coefficient * fixed_wrapper!(0.9);

        debug_assert_eq!(
            Fixed::ONE,
            (val_holders_xor_alloc_coeff.clone()
                + projects_sora_citizens_coeff.clone()
                + projects_stores_and_shops_coeff.clone()
                + projects_parliament_and_development_coeff.clone()
                + projects_other_coeff.clone()
                + val_holders_buy_back_coefficient.clone()).get().unwrap()
        );

        let xor_allocation = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"xor_allocation".to_vec()),
            ),
            val_holders_xor_alloc_coeff.get().unwrap(),
        );
        let sora_citizens = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"sora_citizens".to_vec()),
            ),
            projects_sora_citizens_coeff.get().unwrap(),
        );
        let stores_and_shops = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"stores_and_shops".to_vec()),
            ),
            projects_stores_and_shops_coeff.get().unwrap(),
        );
        let parliament_and_development = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"parliament_and_development".to_vec()),
            ),
            projects_parliament_and_development_coeff.get().unwrap(),
        );
        let projects = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"projects".to_vec()),
            ),
            projects_other_coeff.get().unwrap(),
        );
        let val_holders = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"val_holders".to_vec()),
            ),
            val_holders_buy_back_coefficient.get().unwrap(),
        );
        let accounts = DistributionAccounts::<_> {
            xor_allocation,
            sora_citizens,
            stores_and_shops,
            parliament_and_development,
            projects,
            val_holders,
        };
        for tech_account in &accounts.xor_distribution_accounts_as_array() {
            Technical::register_tech_account_id((*tech_account).clone())?;
        }
        MBCPool::set_distribution_accounts(accounts.clone());
        Ok(accounts)
    }

    #[test]
    fn should_exchange_with_empty_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(10000),
                AssetSymbol(b"USDT".to_vec()),
                18,
            ),
            (alice(), XOR, 0, AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(205), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let _distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(5.529018162388484076), balance!(0.003009027081243731))
            );
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &VAL,
                    SwapAmount::with_desired_input(balance!(1), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    balance!(2.204963991332086241),
                    balance!(0.003)
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_nearly_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(10), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(10000), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected = FixedWrapper::from(total_issuance) * MBCPool::sell_function(&XOR, Fixed::ZERO).unwrap();
            let pool_reference_amount = reserve_amount_expected
                - FixedWrapper::from(MBCPool::buy_function(&XOR, Fixed::ZERO).unwrap())
                    / balance!(2);
            let pool_reference_amount = pool_reference_amount.into_balance();
            let pool_val_amount = MockDEXApi::quote(&USDT, &VAL, SwapAmount::with_desired_input(pool_reference_amount, Balance::zero()), LiquiditySourceFilter::empty(DEXId::Polkaswap)).unwrap();
            let distribution_accounts =
                bonding_curve_pool_init(vec![(VAL, pool_val_amount.amount)]).unwrap();
            let distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
            let alice = &alice();
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1000), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(5536.708257819426729513), balance!(3.009027081243731193))
            );
            let balances: Vec<Balance> = vec![
                balance!(248.404415987068580219),
                balance!(2.760049066522984224),
                balance!(11.040196266091936898),
                balance!(13.800245332614921123),
                balance!(248.404415987068580219),
            ];
            for (account_id, balance) in distribution_accounts_array
                .to_vec()
                .into_iter()
                .zip(balances)
            {
                assert_eq!(
                    Technical::total_balance(&XOR, &account_id).unwrap(),
                    balance,
                );
            }
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &VAL,
                    SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    balance!(4365.335149368998667748),
                    balance!(3.000000000000000000)
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(10), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(10000), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");

            let pool_reference_amount = 
                FixedWrapper::from(total_issuance) * MBCPool::sell_function(&XOR, Fixed::ZERO).unwrap();
            let pool_reference_amount = pool_reference_amount.into_balance();
            let pool_val_amount = MockDEXApi::quote(&USDT, &VAL, SwapAmount::with_desired_input(pool_reference_amount, Balance::zero()), LiquiditySourceFilter::empty(DEXId::Polkaswap)).unwrap();

            let distribution_accounts =
                bonding_curve_pool_init(vec![(VAL, pool_val_amount.amount)]).unwrap();
            let distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();

            let alice = &alice();
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1000), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(5536.708257819426729513), balance!(3.009027081243731193))
            );
            let balances: Vec<Balance> = vec![
                balance!(248.404415987068580219),
                balance!(2.760049066522984224),
                balance!(11.040196266091936898),
                balance!(13.800245332614921123),
                balance!(248.404415987068580219),
            ];
            for (account_id, balance) in distribution_accounts_array
                .to_vec()
                .into_iter()
                .zip(balances)
            {
                assert_eq!(
                    Technical::total_balance(&XOR, &account_id).unwrap(),
                    balance,
                );
            }
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &VAL,
                    SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    balance!(4365.335415603766574971),
                    balance!(3.000000000000000000)
                )
            );
        });
    }

    #[test]
    fn should_not_sell_without_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USDT, 0, AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), XOR, balance!(1), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            let alice = &alice();

            assert_err!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &VAL,
                    SwapAmount::with_desired_input(balance!(1), Balance::zero()),
                ),
                Error::<Runtime>::NotEnoughReserves
            );
        });
    }

    #[test]
    #[ignore] // does not pass due to precision mismatch, consider optimizing precision for given cumulative case
    fn swaps_should_be_additive() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                0,
                AssetSymbol(b"USDT".to_vec()),
                18,
            ),
            (alice(), XOR, 0, AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(10000), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let alice = &alice();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            let amount = balance!(100); // TODO: investigate strange precision error dependency on value
            let parts = 5;

            let whole_outcome = with_transaction(|| {
                let whole_outcome = MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(amount.into(), Balance::max_value()),
                )
                .unwrap();
                TransactionOutcome::Rollback(whole_outcome)
            });

            let cumulative_outcome = (0..parts)
                .into_iter()
                .map(|_i| {
                    MBCPool::exchange(
                        alice,
                        alice,
                        &DEXId::Polkaswap.into(),
                        &VAL,
                        &XOR,
                        SwapAmount::with_desired_output(
                            (amount / parts).into(),
                            Balance::max_value(),
                        ),
                    )
                    .unwrap()
                })
                .fold(
                    SwapOutcome::new(Balance::zero(), Balance::zero()),
                    |acc, x| SwapOutcome {
                        amount: acc.amount + x.amount,
                        fee: acc.fee + x.fee,
                    },
                );
            assert_eq!(whole_outcome, cumulative_outcome);
        });
    }

    #[test]
    fn should_set_new_reference_token() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), XOR, balance!(1), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(0), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");

            let price_a = MBCPool::quote(
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap();

            MBCPool::set_reference_asset(Origin::signed(alice()), DAI).expect("Failed to set new reference asset.");

            let price_b = MBCPool::quote(
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap();

            assert_ne!(price_a, price_b);
        });
    }

    #[test]
    fn similar_returns_should_be_identical() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(4000), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");

            // Buy with desired input
            let amount_a: Balance = balance!(2000);
            let quote_outcome_a = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
            )
            .unwrap();
            let exchange_outcome_a = MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
            )
            .unwrap();
            let val_balance_a = Assets::free_balance(&VAL, &alice()).unwrap();
            let xor_balance_a = Assets::free_balance(&XOR, &alice()).unwrap();
            assert_eq!(quote_outcome_a.amount, exchange_outcome_a.amount);
            assert_eq!(exchange_outcome_a.amount, xor_balance_a);
            assert_eq!(val_balance_a, amount_a.clone());

            // Buy with desired output
            let amount_b: Balance = balance!(200);
            let quote_outcome_b = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
            )
            .unwrap();
            let exchange_outcome_b = MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
            )
            .unwrap();
            let val_balance_b = Assets::free_balance(&VAL, &alice()).unwrap();
            let xor_balance_b = Assets::free_balance(&XOR, &alice()).unwrap();
            assert_eq!(quote_outcome_b.amount, exchange_outcome_b.amount);
            assert_eq!(xor_balance_a + amount_b.clone(), xor_balance_b);
            assert_eq!(val_balance_b, amount_a.clone() - quote_outcome_b.amount);

            // Sell with desired input
            let amount_c: Balance = balance!(300);
            let quote_outcome_c = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
            )
            .unwrap();
            let exchange_outcome_c = MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
            )
            .unwrap();
            let val_balance_c = Assets::free_balance(&VAL, &alice()).unwrap();
            let xor_balance_c = Assets::free_balance(&XOR, &alice()).unwrap();
            assert_eq!(quote_outcome_c.amount, exchange_outcome_c.amount);
            assert_eq!(val_balance_b + exchange_outcome_c.amount, val_balance_c);
            assert_eq!(xor_balance_b - amount_c.clone(), xor_balance_c.clone());

            // Sell with desired output
            let amount_d: Balance = balance!(100);
            let quote_outcome_d = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
            )
            .unwrap();
            let exchange_outcome_d = MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
            )
            .unwrap();
            let val_balance_d = Assets::free_balance(&VAL, &alice()).unwrap();
            let xor_balance_d = Assets::free_balance(&XOR, &alice()).unwrap();
            assert_eq!(quote_outcome_d.amount, exchange_outcome_d.amount);
            assert_eq!(val_balance_c - quote_outcome_d.amount, val_balance_d);
            assert_eq!(xor_balance_c + amount_d.clone(), xor_balance_d);
        });
    }

    #[test]
    fn should_receive_pswap_reward() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(700000), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), 18),
            (alice(), DAI, balance!(200000), AssetSymbol(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            MBCPool::initialize_pool_unchecked(DAI).expect("Failed to initialize pool.");

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(2000), Balance::zero()),
            )
            .unwrap();

            // no reward for non-incentived asset - VAL
            let (limit, owned) = MBCPool::rewards(&alice());
            assert!(limit.is_zero());
            assert!(owned.is_zero());

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
            )
            .unwrap();

            let (limit, owned) = MBCPool::rewards(&alice());
            assert!(limit.is_zero());
            // FIXME: this does seem too large for a reward, considering PSWAP price $10
            assert_eq!(owned, balance!(22.911473417890552539));
        });
    }

    #[test]
    fn multiple_users_should_be_able_to_claim_rewards() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(700000), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), 18),
            (alice(), DAI, balance!(200000), AssetSymbol(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            MBCPool::initialize_pool_unchecked(DAI).expect("Failed to initialize pool.");
            Assets::transfer(Origin::signed(alice()), DAI, bob(), balance!(50000)).unwrap();
            Currencies::deposit(PSWAP, &incentives_account(), balance!(250000)).unwrap();

            // performing exchanges which are eligible for rewards
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(100000), Balance::zero()),
            )
            .unwrap();
            MBCPool::exchange(
                &bob(),
                &bob(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(50000), Balance::zero()),
            )
            .unwrap();

            // trying to claim with limit of 0
            assert!(Assets::free_balance(&PSWAP, &alice()).unwrap().is_zero());
            assert!(Assets::free_balance(&PSWAP, &bob()).unwrap().is_zero());
            assert_noop!(MBCPool::claim_incentives(Origin::signed(alice())), Error::<Runtime>::NothingToClaim);
            assert_noop!(MBCPool::claim_incentives(Origin::signed(bob())), Error::<Runtime>::NothingToClaim);
            assert!(Assets::free_balance(&PSWAP, &alice()).unwrap().is_zero());
            assert!(Assets::free_balance(&PSWAP, &bob()).unwrap().is_zero());

            // limit is updated via PSWAP burn
            let (limit_alice, owned_alice) = MBCPool::rewards(&alice());
            let (limit_bob, owned_bob) = MBCPool::rewards(&bob());
            assert!(limit_alice.is_zero());
            assert!(limit_bob.is_zero());
            assert!(!owned_alice.is_zero());
            assert!(!owned_bob.is_zero());
            let vesting_amount = (FixedWrapper::from(owned_alice + owned_bob) / MBCPool::pswap_burned_dedicated_for_rewards() / fixed_wrapper!(10)).into_balance();
            let remint_info = pswap_distribution::PswapRemintInfo {
                vesting: vesting_amount,
                ..Default::default()
            };
            MBCPool::on_pswap_burned(remint_info);
            let (limit_alice, _) = MBCPool::rewards(&alice());
            let (limit_bob, _) = MBCPool::rewards(&bob());
            assert_eq!(limit_alice, balance!(1145.147548375146487267));
            assert_eq!(limit_bob, balance!(572.536725117851865203));

            // claiming incentives partially
            assert_ok!(MBCPool::claim_incentives(Origin::signed(alice())));
            assert_ok!(MBCPool::claim_incentives(Origin::signed(bob())));
            let (limit_alice, remaining_owned_alice) = MBCPool::rewards(&alice());
            let (limit_bob, remaining_owned_bob) = MBCPool::rewards(&bob());
            assert_eq!(remaining_owned_alice, balance!(1145.147548375146487269));
            assert_eq!(remaining_owned_bob, balance!(572.536725117851865205));
            assert!(limit_alice.is_zero());
            assert!(limit_bob.is_zero());
            assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), owned_alice - remaining_owned_alice);
            assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), owned_bob - remaining_owned_bob);

            // claiming remainder
            let remint_info = pswap_distribution::PswapRemintInfo {
                vesting: vesting_amount + balance!(100),
                ..Default::default()
            };
            MBCPool::on_pswap_burned(remint_info);
            assert_ok!(MBCPool::claim_incentives(Origin::signed(alice())));
            assert_ok!(MBCPool::claim_incentives(Origin::signed(bob())));
            let (_, empty_owned_alice) = MBCPool::rewards(&alice());
            let (_, empty_owned_bob) = MBCPool::rewards(&bob());
            assert!(empty_owned_alice.is_zero());
            assert!(empty_owned_bob.is_zero());
            assert_eq!(Assets::free_balance(&PSWAP, &alice()).unwrap(), owned_alice);
            assert_eq!(Assets::free_balance(&PSWAP, &bob()).unwrap(), owned_bob);
        });
    }

    #[test]
    fn should_calculate_ideal_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");

            // calculate buy amount from zero to total supply of XOR
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            let initial_state = MBCPool::buy_function(&XOR, (fixed_wrapper!(0) - FixedWrapper::from(xor_supply)).get().unwrap()).unwrap();
            let current_state = MBCPool::buy_function(&XOR, Fixed::ZERO).unwrap();
            let buy_amount: Balance = ((FixedWrapper::from(initial_state) + FixedWrapper::from(current_state)) / fixed_wrapper!(2) * FixedWrapper::from(xor_supply)).try_into_balance().unwrap();

            // get ideal reserves
            let ideal_reserves = MBCPool::ideal_reserves_reference_price(Fixed::ZERO).unwrap();

            // actual amount should match to 80% of buy amount
            assert_eq!((FixedWrapper::from(buy_amount) * fixed_wrapper!(0.8)).into_balance(), ideal_reserves);
        });
    }

    #[test]
    fn should_calculate_actual_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), 18),
            (alice(), DAI, balance!(200000), AssetSymbol(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");
            MBCPool::initialize_pool_unchecked(DAI).expect("Failed to initialize pool.");
            MBCPool::set_reference_asset(Origin::signed(alice()), DAI).unwrap();

            let val_amount: Balance = balance!(2000);
            let dai_amount: Balance = balance!(200000);

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(val_amount.clone(), Balance::zero()),
            )
            .unwrap();

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(dai_amount.clone(), Balance::zero()),
            )
            .unwrap();

            let val_actual_reserves = MBCPool::actual_reserves_reference_price(&crate::mock::get_pool_reserves_account_id(), &VAL).unwrap();
            let dai_actual_reserves = MBCPool::actual_reserves_reference_price(&crate::mock::get_pool_reserves_account_id(), &DAI).unwrap();
            let val_supposed_price = MockDEXApi::quote(&VAL, &DAI, SwapAmount::with_desired_input(val_amount, Balance::zero()), LiquiditySourceFilter::empty(DEXId::Polkaswap.into())).unwrap().amount;
            let dai_supposed_price = dai_amount;

            // compare values, also deduce 20% which are distributed and not stored in reserves
            assert_eq!(val_actual_reserves, (FixedWrapper::from(val_supposed_price) * fixed_wrapper!(0.8)).into_balance());
            assert_eq!(dai_actual_reserves, (FixedWrapper::from(dai_supposed_price) * fixed_wrapper!(0.8)).into_balance());
        });
    }

    #[test]
    fn fees_for_equivalent_trades_should_match() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL).expect("Failed to initialize pool.");

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
            )
            .unwrap();

            // Buy
            let price_a = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(balance!(100), Balance::zero()),
            )
            .unwrap();
            let price_b = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(price_a.amount.clone(), Balance::max_value()),
            )
            .unwrap();
            assert_eq!(price_a.fee, price_b.fee);
            assert_eq!(price_a.fee, balance!(0.054394410184082534));

            // Sell
            let price_c = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(balance!(100), Balance::max_value()),
            )
            .unwrap();
            let price_d = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(price_c.amount.clone(), Balance::zero()),
            )
            .unwrap();
            assert_eq!(price_c.fee, price_d.fee);
            assert_eq!(price_c.fee, balance!(0.077942042880974657));
        });
    }
}
