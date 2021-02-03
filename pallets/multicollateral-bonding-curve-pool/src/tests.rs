#[rustfmt::skip]
mod tests {
    use crate::{mock::*, DistributionAccountData, DistributionAccounts, Error};
    use common::{
        self, fixed, fixed_wrapper, Fixed, fixnum::ops::Numeric,
        prelude::{Balance, SwapAmount, SwapOutcome, QuoteAmount, FixedWrapper,},
        AssetSymbol, DEXId, LiquiditySource, TechPurpose, USDT, VAL, XOR, LiquiditySourceFilter,
    };
    use liquidity_proxy::LiquidityProxyTrait;
    use frame_support::{assert_err, assert_noop};
    use frame_support::storage::{with_transaction, TransactionOutcome};
    use sp_arithmetic::traits::{Bounded, Zero};
    use sp_runtime::DispatchError;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");

            // base case for buy
            assert_eq!(
                MBCPool::reference_buy_price_for_one_main_asset(&XOR)
                    .expect("failed to calculate buy price"),
                fixed!(536.574420344053851907)
            );
            assert_eq!(
                MBCPool::buy_price(&XOR, &VAL, QuoteAmount::with_desired_output(Balance(fixed!(100000))))
                    .expect("failed to calculate buy assets price"),
                fixed!(1151397.348365215316851554)
            );
            assert_eq!(
                MBCPool::buy_price(&XOR, &VAL, QuoteAmount::with_desired_input(Balance(fixed!(1151397.348365215316851554))))
                    .expect("failed to calculate buy assets price"),
                fixed!(99999.999999999999999671) // TODO: investigate precision error
            );

            // base case for sell with empty reserves
            assert_eq!(
                MBCPool::reference_sell_price_for_one_main_asset(&XOR)
                    .expect("failed to calculate sell price"),
                fixed!(429.259536275243081525)
            );
            assert_noop!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_output(Balance(fixed!(100000)))),
                Error::<Runtime>::NotEnoughReserves,
            );
            assert_noop!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_input(Balance(fixed!(100000)))),
                Error::<Runtime>::NotEnoughReserves,
            );

            // base case for sell with some reserves
            MBCPool::exchange(alice, alice, &DEXId::Polkaswap, &VAL, &XOR, SwapAmount::with_desired_input(100_000u32.into(), 0u32.into())).expect("Failed to buy XOR.");
            assert_eq!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_output(Balance(fixed!(50000))))
                    .expect("failed to calculate buy assets price"),
                fixed!(15287.320865181476266515)
            );
            assert_eq!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_input(Balance(fixed!(15287.320865181476266515))))
                    .expect("failed to calculate buy assets price"),
                fixed!(50000)
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
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");
            // add some reserves
            MBCPool::exchange(&alice, &alice, &DEXId::Polkaswap, &VAL, &XOR, SwapAmount::with_desired_input(1u32.into(), 0u32.into())).expect("Failed to buy XOR.");

            assert_noop!(
                MBCPool::sell_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_input(Balance::max_value()),
                ),
                Error::<Runtime>::CalculatePriceFailed,
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
                Error::<Runtime>::CalculatePriceFailed,
            );
            assert_noop!(
                MBCPool::buy_price(
                    &XOR,
                    &VAL,
                    QuoteAmount::with_desired_output(Balance::max_value()),
                ),
                Error::<Runtime>::CalculatePriceFailed,
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
                10_000u32.into(),
                AssetSymbol(b"USDT".to_vec()),
                18,
            ),
            (alice(), XOR, 0u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 205u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let _distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    SwapAmount::with_desired_output(1u32.into(), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(Balance(fixed!(5.51243108532778623)), Balance(fixed!(0)))
            );
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &VAL,
                    SwapAmount::with_desired_input(Balance(fixed!(1)), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    Balance(fixed!(2.204973934517568544)),
                    Balance(fixed!(0))
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_nearly_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, 10u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 10_000u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected = Balance(
                (FixedWrapper::from(total_issuance.0) * MBCPool::reference_sell_price_for_one_main_asset(&XOR)
                    .unwrap()).get().unwrap()
                );
            let pool_reference_amount = reserve_amount_expected
                - Balance(MBCPool::reference_buy_price_for_one_main_asset(&XOR).unwrap())
                    / Balance::from(2u32);
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
                    SwapAmount::with_desired_output(1_000u32.into(), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(Balance(fixed!(5520.075559513244296436)), Balance(fixed!(0)))
            );
            let balances: Vec<Balance> = vec![
                Balance(fixed!(247.658189977561705359)),
                Balance(fixed!(2.751757666417352281)),
                Balance(fixed!(11.007030665669409127)),
                Balance(fixed!(13.758788332086761408)),
                Balance(fixed!(247.658189977561705359)),
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
                    SwapAmount::with_desired_input(Balance(fixed!(1000)), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    Balance(fixed!(4378.339389331602962154)),
                    Balance(fixed!(0.000000000000000000))
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, 10u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 10_000u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");

            let pool_reference_amount = Balance(
                (FixedWrapper::from(total_issuance.0) * MBCPool::reference_sell_price_for_one_main_asset(&XOR)
                    .unwrap()).get().unwrap(),
            );
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
                    SwapAmount::with_desired_output(1_000u32.into(), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(Balance(fixed!(5520.075559513244296436)), Balance(fixed!(0)))
            );
            let balances: Vec<Balance> = vec![
                Balance(fixed!(247.658189977561705359)),
                Balance(fixed!(2.751757666417352281)),
                Balance(fixed!(11.007030665669409127)),
                Balance(fixed!(13.758788332086761408)),
                Balance(fixed!(247.658189977561705359)),
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
                    SwapAmount::with_desired_input(Balance(fixed!(1000)), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    Balance(fixed!(4378.339657171044553636)),
                    Balance(fixed!(0.000000000000000000))
                )
            );
        });
    }

    #[test]
    fn should_not_sell_without_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USDT, 0u32.into(), AssetSymbol(b"USDT".to_vec()), 18),
            (alice(), XOR, 1u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");
            let alice = &alice();

            assert_err!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &VAL,
                    SwapAmount::with_desired_input(1u32.into(), Balance::zero()),
                ),
                Error::<Runtime>::NotEnoughReserves
            );
        });
    }

    #[test]
    fn swaps_should_be_additive() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                0u32.into(),
                AssetSymbol(b"USDT".to_vec()),
                18,
            ),
            (alice(), XOR, 0u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 10_000u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let alice = &alice();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(XOR, VAL).expect("Failed to initialize pool.");
            let amount = 124_u32; // TODO: investigate strange precision error dependency on value
            let parts = 2;

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


            // TODO: linear additivity is inapplicable to curve used in sell, how else this can be checked?

            // let whole_outcome = with_transaction(|| {
            //     let whole_outcome = MBCPool::exchange(
            //         alice,
            //         alice,
            //         &DEXId::Polkaswap.into(),
            //         &XOR,
            //         &VAL,
            //         SwapAmount::with_desired_input(cumulative_outcome.amount, Balance::zero()),
            //     )
            //     .unwrap();
            //     TransactionOutcome::Rollback(whole_outcome)
            // });

            // let cumulative_outcome = (0..parts)
            //     .into_iter()
            //     .map(|_i| {
            //         MBCPool::exchange(
            //             alice,
            //             alice,
            //             &DEXId::Polkaswap.into(),
            //             &XOR,
            //             &VAL,
            //             SwapAmount::with_desired_input(
            //                 cumulative_outcome.amount / Balance::from(parts),
            //                 Balance::zero(),
            //             ),
            //         )
            //         .unwrap()
            //     })
            //     .fold(
            //         SwapOutcome::new(Balance::zero(), Balance::zero()),
            //         |acc, x| SwapOutcome {
            //             amount: acc.amount + x.amount,
            //             fee: acc.fee + x.fee,
            //         },
            //     );
            // assert_eq!(whole_outcome, cumulative_outcome);
        });
    }
}
