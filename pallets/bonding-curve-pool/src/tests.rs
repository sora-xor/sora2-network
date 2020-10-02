#[rustfmt::skip]
mod tests {
    use crate::{mock::*, DistributionAccounts, Error};
    use common::{
        fixed, AssetId, TechPurpose, DEXId, LiquiditySource,
        prelude::{Balance, Fixed, SwapAmount, SwapOutcome}
    };
    use frame_support::assert_err;
    use sp_arithmetic::traits::{Bounded, Zero};
    use sp_runtime::DispatchError;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(
                BondingCurvePool::buy_price(&XOR).expect("failed to calculate buy price"),
                Fixed::from(100)
            );
            assert_eq!(
                BondingCurvePool::buy_tokens_out_price(&XOR, 100_000u32.into())
                    .expect("failed to calculate buy tokens price"),
                Fixed::from(100_10_000)
            );
            assert_eq!(
                BondingCurvePool::sell_price(&XOR).expect("failed to calculate sell price"),
                Fixed::from(80)
            );
            assert_eq!(
                BondingCurvePool::sell_tokens_in_price(&XOR, 100_000u32.into())
                    .expect("failed to calculate sell tokens price"),
                Fixed::from(80_08_000)
            );
            assert_eq!(
                BondingCurvePool::sell_tokens_in_price(&XOR, 0u32.into())
                    .expect("failed to calculate sell tokens price"),
                Fixed::from(0)
            );
        });
    }

    #[test]
    fn should_not_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(
                BondingCurvePool::sell_tokens_in_price(&XOR, u128::max_value().into()).unwrap_err(),
                Error::<Runtime>::CalculatePriceFailed.into()
            );
        });
    }

    fn bonding_curve_pool_init(
        initial_reserves: Vec<(AssetId, Balance)>,
    ) -> Result<DistributionAccounts<Runtime>, DispatchError> {
        let bonding_curve_tech_account_id = TechAccountId::Pure(DEXId::Polkaswap, TechPurpose::Identifier(b"bonding_curve_tech_account_id".to_vec()));
        Technical::register_tech_account_id(bonding_curve_tech_account_id.clone())?;
        BondingCurvePool::set_reserves_account_id(bonding_curve_tech_account_id.clone())?;
        for (asset_id, balance) in initial_reserves {
            Technical::mint(&asset_id, &bonding_curve_tech_account_id, balance)?;
        }
        let xor_allocation = TechAccountId::Pure(DEXId::Polkaswap, TechPurpose::Identifier(b"xor_allocation".to_vec()));
        let sora_citizens = TechAccountId::Pure(DEXId::Polkaswap, TechPurpose::Identifier(b"sora_citizens".to_vec()));
        let stores_and_shops = TechAccountId::Pure(DEXId::Polkaswap, TechPurpose::Identifier(b"stores_and_shops".to_vec()));
        let parliament_and_development = TechAccountId::Pure(DEXId::Polkaswap, TechPurpose::Identifier(b"parliament_and_development".to_vec()));
        let projects = TechAccountId::Pure(DEXId::Polkaswap, TechPurpose::Identifier(b"projects".to_vec()));
        let accounts = DistributionAccounts::<Runtime> {
            xor_allocation,
            sora_citizens,
            stores_and_shops,
            parliament_and_development,
            projects,
        };
        for tech_account in &accounts.as_array() {
            Technical::register_tech_account_id((*tech_account).clone())?;
        }
        BondingCurvePool::set_distribution_accounts(accounts.clone());
        Ok(accounts)
    }

    #[test]
    fn should_exchange_with_empty_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USD, 10_000u32.into()),
            (alice(), XOR, 0u32.into()),
            (alice(), VAL, 0u32.into()),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let distribution_accounts_array = distribution_accounts.as_array();
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
                    fixed!(79,282798634407197600).into(),
                    fixed!(0,079362160795202400).into()
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_nearly_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USD, 10_000u32.into()),
            (alice(), XOR, 10u32.into()),
            (alice(), VAL, 0u32.into()),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected =
                Balance(BondingCurvePool::sell_tokens_in_price(&XOR, total_issuance).unwrap());
            let pool_usd_amount = reserve_amount_expected
                - Balance(BondingCurvePool::buy_price(&XOR).unwrap()) / Balance::from(2u32);
            let distribution_accounts =
                bonding_curve_pool_init(vec![(USD, pool_usd_amount)]).unwrap();
            let distribution_accounts_array = distribution_accounts.as_array();
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
                fixed!(0,044551852170300000).into(),
                fixed!(0,000495020579670000).into(),
                fixed!(0,001980082318680000).into(),
                fixed!(0,002475102898350000).into(),
                fixed!(0,044551852170300000).into(),
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
                    fixed!(79,2828146024231976).into(),
                    fixed!(0,07936217677920240).into()
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USD, 10_000u32.into()),
            (alice(), XOR, 10u32.into()),
            (alice(), VAL, 0u32.into()),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected =
                Balance(BondingCurvePool::sell_tokens_in_price(&XOR, total_issuance).unwrap());
            let distribution_accounts =
                bonding_curve_pool_init(vec![(USD, reserve_amount_expected)]).unwrap();
            let distribution_accounts_array = distribution_accounts.as_array();
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
                fixed!(0,08910370344330).into(),
                fixed!(0,00099004114937).into(),
                fixed!(0,00396016459748).into(),
                fixed!(0,00495020574685).into(),
                fixed!(0,08910370344330).into(),
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
                    fixed!(79,2828146024231976).into(),
                    fixed!(0,07936217677920240).into()
                )
            );
        });
    }

    #[test]
    fn should_not_sell_without_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USD, 0u32.into()),
            (alice(), XOR, 1u32.into()),
            (alice(), VAL, 0u32.into()),
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
}
