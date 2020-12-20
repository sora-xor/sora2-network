#[rustfmt::skip]
mod tests {
    use crate::{mock::*, DistributionAccountData, DistributionAccounts, Error, SwapKind};
    use common::{
        self, fixed,
        prelude::{Balance, Fixed, SwapAmount, SwapOutcome},
        AssetSymbol, DEXId, LiquiditySource, TechPurpose, USD, VAL, XOR,
    };
    use frame_support::assert_err;
    use frame_support::storage::{with_transaction, TransactionOutcome};
    use sp_arithmetic::traits::{Bounded, Zero};
    use sp_runtime::DispatchError;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(
                BondingCurvePool::buy_price_for_one_main_asset(&XOR)
                    .expect("failed to calculate buy price"),
                Fixed::from(100)
            );
            assert_eq!(
                BondingCurvePool::price_for_main_asset(&XOR, 100_000u32.into(), SwapKind::Buy)
                    .expect("failed to calculate buy assets price"),
                Fixed::from(10_010_000)
            );
            assert_eq!(
                BondingCurvePool::sell_price_for_one_main_asset(&XOR)
                    .expect("failed to calculate sell price"),
                Fixed::from(80)
            );
            assert_eq!(
                BondingCurvePool::price_for_main_asset(&XOR, 100_000u32.into(), SwapKind::Sell)
                    .expect("failed to calculate sell assets price"),
                Fixed::from(7_992_000)
            );
            assert_eq!(
                BondingCurvePool::price_for_main_asset(&XOR, 0u32.into(), SwapKind::Sell)
                    .expect("failed to calculate sell assets price"),
                Fixed::from(0)
            );
        });
    }

    #[test]
    fn should_not_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(
                BondingCurvePool::price_for_main_asset(
                    &XOR,
                    u128::max_value().into(),
                    SwapKind::Sell
                )
                .unwrap_err(),
                Error::<Runtime>::CalculatePriceFailed.into()
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
        BondingCurvePool::set_reserves_account_id(bonding_curve_tech_account_id.clone())?;
        for (asset_id, balance) in initial_reserves {
            Technical::mint(&asset_id, &bonding_curve_tech_account_id, balance)?;
        }

        let val_holders_coefficient: Fixed = fixed!(50%);
        let val_holders_xor_alloc_coeff = val_holders_coefficient * fixed!(90%);
        let val_holders_buy_back_coefficient =
            val_holders_coefficient * (fixed!(100%) - fixed!(90%));
        let projects_coefficient = fixed!(100%) - val_holders_coefficient;
        let projects_sora_citizens_coeff = projects_coefficient * fixed!(1%);
        let projects_stores_and_shops_coeff = projects_coefficient * fixed!(4%);
        let projects_parliament_and_development_coeff = projects_coefficient * fixed!(5%);
        let projects_other_coeff = projects_coefficient * fixed!(90%);

        debug_assert_eq!(
            fixed!(100%),
            val_holders_xor_alloc_coeff
                + projects_sora_citizens_coeff
                + projects_stores_and_shops_coeff
                + projects_parliament_and_development_coeff
                + projects_other_coeff
                + val_holders_buy_back_coefficient
        );

        let xor_allocation = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"xor_allocation".to_vec()),
            ),
            val_holders_xor_alloc_coeff,
        );
        let sora_citizens = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"sora_citizens".to_vec()),
            ),
            projects_sora_citizens_coeff,
        );
        let stores_and_shops = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"stores_and_shops".to_vec()),
            ),
            projects_stores_and_shops_coeff,
        );
        let parliament_and_development = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"parliament_and_development".to_vec()),
            ),
            projects_parliament_and_development_coeff,
        );
        let projects = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"projects".to_vec()),
            ),
            projects_other_coeff,
        );
        let val_holders = DistributionAccountData::new(
            TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"val_holders".to_vec()),
            ),
            val_holders_buy_back_coefficient,
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
        BondingCurvePool::set_distribution_accounts(accounts.clone());
        Ok(accounts)
    }

    #[test]
    fn should_exchange_with_empty_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USD,
                10_000u32.into(),
                AssetSymbol(b"USD".to_vec()),
                18,
            ),
            (alice(), XOR, 0u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
            let alice = &alice();
            assert_eq!(
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USD,
                    &XOR,
                    SwapAmount::with_desired_output(1u32.into(), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(fixed!(0,999).into(), fixed!(0,001).into())
            );
            for account_id in &distribution_accounts_array {
                assert_eq!(
                    Technical::total_balance(&XOR, account_id).unwrap(),
                    Balance::zero(),
                );
            }
            assert_eq!(
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &USD,
                    SwapAmount::with_desired_input(fixed!(0,999).into(), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    fixed!(79,2827970392023992).into(),
                    fixed!(0,0793621591984008).into()
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_nearly_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USD,
                10_000u32.into(),
                AssetSymbol(b"USD".to_vec()),
                18,
            ),
            (alice(), XOR, 10u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected = Balance(
                BondingCurvePool::price_for_main_asset(&XOR, total_issuance, SwapKind::Sell)
                    .unwrap(),
            );
            let pool_usd_amount = reserve_amount_expected
                - Balance(BondingCurvePool::buy_price_for_one_main_asset(&XOR).unwrap())
                    / Balance::from(2u32);
            let distribution_accounts =
                bonding_curve_pool_init(vec![(USD, pool_usd_amount)]).unwrap();
            let distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
            let alice = &alice();
            assert_eq!(
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USD,
                    &XOR,
                    SwapAmount::with_desired_output(1u32.into(), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(fixed!(0,999).into(), fixed!(0,001).into())
            );
            let balances: Vec<Balance> = vec![
                fixed!(0,0445518521703).into(),
                fixed!(0,00049502057967).into(),
                fixed!(0,00198008231868).into(),
                fixed!(0,00247510289835).into(),
                fixed!(0,0445518521703).into(),
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
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &USD,
                    SwapAmount::with_desired_input(fixed!(0, 999).into(), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    fixed!(79,2828130072183992).into(),
                    fixed!(0,0793621751824008).into()
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USD,
                10_000u32.into(),
                AssetSymbol(b"USD".to_vec()),
                18,
            ),
            (alice(), XOR, 10u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected = Balance(
                BondingCurvePool::price_for_main_asset(&XOR, total_issuance, SwapKind::Sell)
                    .unwrap(),
            );
            let distribution_accounts =
                bonding_curve_pool_init(vec![(USD, reserve_amount_expected)]).unwrap();
            let distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
            let alice = &alice();
            assert_eq!(
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USD,
                    &XOR,
                    SwapAmount::with_desired_output(1u32.into(), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(fixed!(0, 999).into(), fixed!(0, 001).into())
            );
            let balances: Vec<Balance> = vec![
                fixed!(0,0891037034433).into(),
                fixed!(0,00099004114937).into(),
                fixed!(0,00396016459748).into(),
                fixed!(0,00495020574685).into(),
                fixed!(0,0891037034433).into(),
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
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &USD,
                    SwapAmount::with_desired_input(fixed!(0,999).into(), Balance::zero()),
                )
                .unwrap(),
                SwapOutcome::new(
                    fixed!(79,2828130072183992).into(),
                    fixed!(0,0793621751824008).into()
                )
            );
        });
    }

    #[test]
    fn should_not_sell_without_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USD, 0u32.into(), AssetSymbol(b"USD".to_vec()), 18),
            (alice(), XOR, 1u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            let alice = &alice();
            assert_err!(
                BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &USD,
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
                USD,
                10_000u32.into(),
                AssetSymbol(b"USD".to_vec()),
                18,
            ),
            (alice(), XOR, 0u32.into(), AssetSymbol(b"XOR".to_vec()), 18),
            (alice(), VAL, 0u32.into(), AssetSymbol(b"VAL".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let alice = &alice();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            let amount = 100_u32;
            let parts = 2;

            let whole_outcome = with_transaction(|| {
                let whole_outcome = BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USD,
                    &XOR,
                    SwapAmount::with_desired_output(amount.into(), Balance::max_value()),
                )
                .unwrap();
                TransactionOutcome::Rollback(whole_outcome)
            });

            let cumulative_outcome = (0..parts)
                .into_iter()
                .map(|_i| {
                    BondingCurvePool::exchange(
                        alice,
                        alice,
                        &DEXId::Polkaswap.into(),
                        &USD,
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

            let whole_outcome = with_transaction(|| {
                let whole_outcome = BondingCurvePool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &XOR,
                    &USD,
                    SwapAmount::with_desired_input(cumulative_outcome.amount, Balance::zero()),
                )
                .unwrap();
                TransactionOutcome::Rollback(whole_outcome)
            });

            let cumulative_outcome = (0..parts)
                .into_iter()
                .map(|_i| {
                    BondingCurvePool::exchange(
                        alice,
                        alice,
                        &DEXId::Polkaswap.into(),
                        &XOR,
                        &USD,
                        SwapAmount::with_desired_input(
                            cumulative_outcome.amount / Balance::from(parts),
                            Balance::zero(),
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
}
