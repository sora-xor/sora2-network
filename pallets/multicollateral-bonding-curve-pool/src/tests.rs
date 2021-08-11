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
    use crate::{mock::*, DistributionAccountData, Module, DistributionAccounts, DistributionAccount, Error, RETRY_DISTRIBUTION_FREQUENCY};
    use common::{
        self, balance, fixed, fixed_wrapper, Fixed, fixnum::ops::One as _, fixnum::ops::Zero as _,
        prelude::{Balance, SwapAmount, SwapOutcome, QuoteAmount, FixedWrapper,},
        AssetName, AssetSymbol, DEXId, LiquiditySource, TechPurpose, USDT, VAL, XOR, PSWAP, XSTUSD, LiquiditySourceFilter,
    };
    use hex_literal::hex;
    use frame_support::traits::OnInitialize;
    use liquidity_proxy::LiquidityProxyTrait;
    use frame_support::{assert_err, assert_noop};
    use frame_support::storage::{with_transaction, TransactionOutcome};
    use sp_arithmetic::traits::{Zero};
    use sp_runtime::DispatchError;

    type MBCPool = Module<Runtime>;

    fn ensure_distribution_accounts_balances(distribution_accounts: DistributionAccounts<DistributionAccountData<DistributionAccount<AccountId, TechAccountId>>>, balances: Vec<Balance>) {
        let distribution_accounts_array = distribution_accounts.xor_distribution_accounts_as_array();
        for (account, balance) in distribution_accounts_array
            .to_vec()
            .into_iter()
            .zip(balances)
        {
            match account {
                DistributionAccount::Account(account_id) => {
                    assert_eq!(
                        Assets::total_balance(&XOR, &account_id).unwrap(),
                        balance,
                    );
                }
                DistributionAccount::TechAccount(account_id) => {
                    assert_eq!(
                        Technical::total_balance(&XOR, &account_id).unwrap(),
                        balance,
                    );
                }
            }
        }
    }

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            // base case for buy
            assert_eq!(
                MBCPool::buy_function(&XOR, Fixed::ZERO)
                    .expect("failed to calculate buy price"),
                    fixed!(536.575170537791384625)
            );
            assert_eq!(
                MBCPool::buy_price(&XOR, &VAL, QuoteAmount::with_desired_output(balance!(100000)))
                    .expect("failed to calculate buy assets price"),
                fixed!(1151398.853267396927121364)
            );
            assert_eq!(
                MBCPool::buy_price(&XOR, &VAL, QuoteAmount::with_desired_input(balance!(1151397.348365215316854563)))
                    .expect("failed to calculate buy assets price"),
                fixed!(99999.877292895570947223) // TODO: try to improve precision
            );

            // base case for sell with empty reserves
            assert_eq!(
                MBCPool::sell_function(&XOR, Fixed::ZERO)
                    .expect("failed to calculate sell price"),
                    fixed!(429.2601364302331077)
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
                fixed!(15287.882675656841911345)
            );
            assert_eq!(
                MBCPool::sell_price(&XOR, &VAL, QuoteAmount::with_desired_input(balance!(15287.903511880099065528)))
                    .expect("failed to calculate buy assets price"),
                fixed!(50000.025554804518660908) // TODO: improve precision
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
            TradingPair::register(Origin::signed(alice.clone()), DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
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
        DistributionAccounts<DistributionAccountData<DistributionAccount<
            <Runtime as frame_system::Config>::AccountId,
            <Runtime as technical::Config>::TechAccountId>>>,
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
        let initial_price: Fixed = fixed!(200);
        crate::InitialPrice::<Runtime>::put(initial_price);

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
            DistributionAccount::TechAccount(TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"xor_allocation".to_vec()),
            )),
            val_holders_xor_alloc_coeff.get().unwrap(),
        );
        let sora_citizens = DistributionAccountData::new(
            DistributionAccount::TechAccount(TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"sora_citizens".to_vec()),
            )),
            projects_sora_citizens_coeff.get().unwrap(),
        );
        let stores_and_shops = DistributionAccountData::new(
            DistributionAccount::TechAccount(TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"stores_and_shops".to_vec()),
            )),
            projects_stores_and_shops_coeff.get().unwrap(),
        );
        let parliament_and_development = DistributionAccountData::new(
            DistributionAccount::Account(hex!("881b87c9f83664b95bd13e2bb40675bfa186287da93becc0b22683334d411e4e").into()),
            projects_parliament_and_development_coeff.get().unwrap(),
        );
        let projects = DistributionAccountData::new(
            DistributionAccount::TechAccount(TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"projects".to_vec()),
            )),
            projects_other_coeff.get().unwrap(),
        );
        let val_holders = DistributionAccountData::new(
            DistributionAccount::TechAccount(TechAccountId::Pure(
                DEXId::Polkaswap,
                TechPurpose::Identifier(b"val_holders".to_vec()),
            )),
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
        for account in &accounts.xor_distribution_accounts_as_array() {
            match account {
                DistributionAccount::Account(_) => continue,
                DistributionAccount::TechAccount(account) => {
                    Technical::register_tech_account_id(account.clone())?;
                }
            }
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
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 0, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(205), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
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
                    balance!(2.100439516374830873),
                    balance!(0.093)
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_nearly_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(10), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(10000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, balance!(10000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let initial_price: Fixed = fixed!(200);
            crate::InitialPrice::<Runtime>::put(initial_price);
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            let reserve_amount_expected = FixedWrapper::from(total_issuance) * MBCPool::sell_function(&XOR, Fixed::ZERO).unwrap();
            let pool_reference_amount = reserve_amount_expected
                - FixedWrapper::from(MBCPool::buy_function(&XOR, Fixed::ZERO).unwrap())
                    / balance!(2);
            let pool_reference_amount = pool_reference_amount.into_balance();
            let pool_val_amount = MockDEXApi::quote(&USDT, &VAL, QuoteAmount::with_desired_input(pool_reference_amount), LiquiditySourceFilter::empty(DEXId::Polkaswap)).unwrap();
            let distribution_accounts =
                bonding_curve_pool_init(vec![(VAL, pool_val_amount.amount)]).unwrap();
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
                SwapOutcome::new(balance!(5538.217688292456084016), balance!(3.009027081243731193))
            );
            ensure_distribution_accounts_balances(distribution_accounts, vec![
                balance!(2.760801517613789357),
                balance!(11.043206070455157431),
                balance!(13.804007588068946789),
                balance!(248.472136585241042209),
            ]);
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
                    balance!(4366.523658759765819497),
                    balance!(3.000000000000000000)
                )
            );
        });
    }

    #[test]
    fn should_exchange_with_full_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(10), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(10000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, balance!(10000), AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let initial_price: Fixed = fixed!(200);
            crate::InitialPrice::<Runtime>::put(initial_price);
            let total_issuance = Assets::total_issuance(&XOR).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, XSTUSD).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            let pool_reference_amount =
                FixedWrapper::from(total_issuance) * MBCPool::sell_function(&XOR, Fixed::ZERO).unwrap();
            let pool_reference_amount = pool_reference_amount.into_balance();
            let pool_val_amount = MockDEXApi::quote(&USDT, &VAL, QuoteAmount::with_desired_input(pool_reference_amount), LiquiditySourceFilter::empty(DEXId::Polkaswap)).unwrap();

            let distribution_accounts =
                bonding_curve_pool_init(vec![(VAL, pool_val_amount.amount)]).unwrap();
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
                SwapOutcome::new(balance!(5538.217688292456084016), balance!(3.009027081243731193))
            );
            ensure_distribution_accounts_balances(distribution_accounts, vec![
                balance!(2.760801517613789357),
                balance!(11.043206070455157431),
                balance!(13.804007588068946789),
                balance!(248.472136585241042209),
            ]);
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
                    balance!(4366.523925066825637517),
                    balance!(3.000000000000000000)
                )
            );
        });
    }

    #[test]
    fn should_not_sell_without_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), USDT, 0, AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(1), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()), DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
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
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 0, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(10000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let alice = &alice();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
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
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(1), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(0), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            let price_a = MBCPool::quote(
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    QuoteAmount::with_desired_output(balance!(1)),
                )
                .unwrap();

            MBCPool::set_reference_asset(Origin::signed(alice()), DAI).expect("Failed to set new reference asset.");

            let price_b = MBCPool::quote(
                    &DEXId::Polkaswap.into(),
                    &VAL,
                    &XOR,
                    QuoteAmount::with_desired_output(balance!(1)),
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
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            // Buy with desired input
            let amount_a: Balance = balance!(2000);
            let quote_outcome_a = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
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
                QuoteAmount::with_desired_output(amount_b.clone()),
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
                QuoteAmount::with_desired_input(amount_c.clone()),
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
                QuoteAmount::with_desired_output(amount_d.clone()),
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
            (alice(), XOR, balance!(700000), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), DAI, balance!(200000), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), AssetName(b"Polkaswap".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
            MBCPool::initialize_pool_unchecked(DAI, false).expect("Failed to initialize pool.");

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
            assert_eq!(owned, balance!(6099.239593179625249492));
        });
    }

    #[test]
    fn should_calculate_ideal_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            // calculate buy amount from zero to total supply of XOR
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            let initial_state = MBCPool::buy_function(&XOR, (fixed_wrapper!(0) - FixedWrapper::from(xor_supply)).get().unwrap()).unwrap();
            let current_state = MBCPool::buy_function(&XOR, Fixed::ZERO).unwrap();
            let buy_amount: Balance = ((FixedWrapper::from(initial_state) + FixedWrapper::from(current_state)) / fixed_wrapper!(2) * FixedWrapper::from(xor_supply)).try_into_balance().unwrap();

            // get ideal reserves
            let ideal_reserves = MBCPool::ideal_reserves_reference_price(Fixed::ZERO).unwrap();

            // actual amount should match to 80% of buy amount
            assert_eq!(buy_amount, ideal_reserves);
        });
    }

    #[test]
    fn should_calculate_actual_reserves() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), DAI, balance!(200000), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), AssetName(b"Polkaswap".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
            MBCPool::initialize_pool_unchecked(DAI, false).expect("Failed to initialize pool.");
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
            let val_supposed_price = MockDEXApi::quote(&VAL, &DAI, QuoteAmount::with_desired_input(val_amount), LiquiditySourceFilter::empty(DEXId::Polkaswap.into())).unwrap().amount;
            let dai_supposed_price = dai_amount;

            // compare values, also deduce 20% which are distributed and not stored in reserves
            assert_eq!(val_actual_reserves, (FixedWrapper::from(val_supposed_price) * fixed_wrapper!(0.8)).into_balance());
            assert_eq!(dai_actual_reserves, (FixedWrapper::from(dai_supposed_price) * fixed_wrapper!(0.8)).into_balance());
        });
    }

    #[test]
    fn fees_for_equivalent_trades_should_match() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

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
                QuoteAmount::with_desired_input(balance!(100)),
            )
            .unwrap();
            let price_b = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(price_a.amount.clone()),
            )
            .unwrap();
            assert_eq!(price_a.fee, price_b.fee);
            assert_eq!(price_a.fee, balance!(0.054394410184082534));

            // Sell
            let price_c = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(balance!(100)),
            )
            .unwrap();
            let price_d = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(price_c.amount.clone()),
            )
            .unwrap();
            assert_eq!(price_c.fee, price_d.fee);
            assert_eq!(price_c.fee, balance!(2.655958896716961113));
        });
    }

    #[test]
    fn check_sell_penalty_based_on_collateralized_fraction() {
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0)), fixed!(0.09));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.03)), fixed!(0.09));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.05)), fixed!(0.06));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.075)), fixed!(0.06));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.1)), fixed!(0.03));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.15)), fixed!(0.03));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.2)), fixed!(0.01));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.25)), fixed!(0.01));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.3)), fixed!(0));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.35)), fixed!(0));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(0.5)), fixed!(0));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(1)), fixed!(0));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(2)), fixed!(0));
        assert_eq!(MBCPool::map_collateralized_fraction_to_penalty(fixed!(10)), fixed!(0));
    }

    #[test]
    fn fee_penalties_should_be_applied() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), DAI, balance!(20000000), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), AssetName(b"Polkaswap".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(DAI, false).expect("Failed to initialize pool.");
            MBCPool::set_reference_asset(Origin::signed(alice()), DAI).unwrap();

            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            assert_eq!(xor_supply, balance!(100000));

            // Depositing collateral #1: under 5% collateralized
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(200000), Balance::zero()),
            )
            .unwrap();
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            assert_eq!(xor_supply, balance!(100724.916324262414175551));

            let sell_price = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &DAI,
                QuoteAmount::with_desired_input(balance!(100)),
            )
            .unwrap();
            assert_eq!(sell_price.fee, balance!(9.3));

            // Depositing collateral #2: under 10% collateralized
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(2000000), Balance::zero()),
            )
            .unwrap();
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            assert_eq!(xor_supply, balance!(107896.889465954935413179));

            let sell_price = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &DAI,
                QuoteAmount::with_desired_input(balance!(100)),
            )
            .unwrap();
            assert_eq!(sell_price.fee, balance!(6.3));

            // Depositing collateral #3: under 20% collateralized
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(2000000), Balance::zero()),
            )
            .unwrap();
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            assert_eq!(xor_supply, balance!(114934.359190755661046424));

            let sell_price = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &DAI,
                QuoteAmount::with_desired_input(balance!(100)),
            )
            .unwrap();
            assert_eq!(sell_price.fee, balance!(3.3));

            // Depositing collateral #4: under 30% collateralized
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(4000000), Balance::zero()),
            )
            .unwrap();
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            assert_eq!(xor_supply, balance!(128633.975165230400026502));

            let sell_price = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &DAI,
                QuoteAmount::with_desired_input(balance!(100)),
            )
            .unwrap();
            assert_eq!(sell_price.fee, balance!(1.3));

            // Depositing collateral #5: over 30% collateralized
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(7000000), Balance::zero()),
            )
            .unwrap();
            let xor_supply = Assets::total_issuance(&XOR).unwrap();
            assert_eq!(xor_supply, balance!(151530.994236602104652386));

            let sell_price = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &DAI,
                QuoteAmount::with_desired_input(balance!(100)),
            )
            .unwrap();
            assert_eq!(sell_price.fee, balance!(0.3));
        });
    }

    #[test]
    fn sequential_rewards_adequacy_check() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), XOR, balance!(250000), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(2000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), DAI, balance!(2000000), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), PSWAP, balance!(0), AssetSymbol(b"PSWAP".to_vec()), AssetName(b"Polkaswap".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, DAI).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");
            MBCPool::initialize_pool_unchecked(DAI, false).expect("Failed to initialize pool.");

            // XOR total supply in network is 350000
            let xor_total_supply: FixedWrapper = Assets::total_issuance(&XOR).unwrap().into();
            assert_eq!(xor_total_supply.clone().into_balance(), balance!(350000));
            // initial XOR price is $264
            let xor_ideal_reserves: FixedWrapper = MBCPool::ideal_reserves_reference_price(Default::default()).unwrap().into();
            assert_eq!((xor_ideal_reserves / xor_total_supply).into_balance(), balance!(330.890052356020942408));
            // pswap price is $10 on mock secondary market
            assert_eq!(
                MockDEXApi::quote(&PSWAP, &DAI, QuoteAmount::with_desired_input(balance!(1)), MBCPool::self_excluding_filter()).unwrap().amount,
                balance!(10.173469387755102041)
            );

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(1000), Balance::zero()),
            )
            .unwrap();

            let (limit, owned_1) = MBCPool::rewards(&alice());
            assert!(limit.is_zero());
            assert_eq!(owned_1, balance!(21036.472370353787480367));

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(10000), Balance::zero()),
            )
            .unwrap();

            let (limit, owned_2) = MBCPool::rewards(&alice());
            assert!(limit.is_zero());
            assert_eq!(owned_2 - owned_1, balance!(210336.209418679523304856));

            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &DAI,
                &XOR,
                SwapAmount::with_desired_input(balance!(1000000), Balance::zero()),
            )
            .unwrap();

            let (limit, owned_3) = MBCPool::rewards(&alice());
            assert!(limit.is_zero());
            assert_eq!(owned_3 - owned_2, balance!(20769070.485987076318293437));
        });
    }

    #[test]
    fn distribution_passes_on_first_attempt() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(10000),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 0, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            // secondary market is initialized with enough funds
            MockDEXApi::init().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");
            
            // check pending list and reserves before trade
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance, balance!(0));

            ensure_distribution_accounts_balances(distribution_accounts.clone(), vec![
                balance!(0),
                balance!(0),
                balance!(0),
                balance!(0),
            ]);

            // perform buy on tbc
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(275.621555395065931189), balance!(0.003009027081243731))
            );

            // check pending list and free reserves account
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance, balance!(0));

            ensure_distribution_accounts_balances(distribution_accounts, vec![
                balance!(0.002747946907288807),
                balance!(0.010991787629155229),
                balance!(0.013739734536444036),
                balance!(0.247315221655992660),
            ]);
        })
    }

    #[test]
    fn distribution_fails_on_first_attempt() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(10000),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 350000, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            // secondary market is initialized without funds
            MockDEXApi::init_without_reserves().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");
            
            // check pending list and reserves before trade
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance, balance!(0));

            // perform buy on tbc
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(200.602181641794149028), balance!(0.003009027081243731))
            );

            // check pending list and free reserves account
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance.clone())]);
            assert_eq!(free_reserves_balance, balance!(40.120436328358829805));
           
            // attempt for distribution, still not enough reserves
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance_2 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance.clone())]);
            assert_eq!(free_reserves_balance_2, free_reserves_balance);

            // exchange becomes possible for stored free reserves
            MockDEXApi::add_reserves(vec![(XOR, balance!(100000)), (VAL, balance!(100000)), (USDT, balance!(1000000))]).unwrap();

            // actual accounts check before distribution
            ensure_distribution_accounts_balances(distribution_accounts.clone(), vec![
                balance!(0),
                balance!(0),
                balance!(0),
                balance!(0),
            ]);

            // attempt for distribution before retry period
            MBCPool::on_initialize((RETRY_DISTRIBUTION_FREQUENCY - 1).into());
            let free_reserves_balance_3 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance.clone())]);
            assert_eq!(free_reserves_balance_3, free_reserves_balance);

            // successful attempt for distribution
            MBCPool::on_initialize((RETRY_DISTRIBUTION_FREQUENCY * 2).into());
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance, balance!(0));

            // actual accounts check after distribution
            ensure_distribution_accounts_balances(distribution_accounts, vec![
                balance!(0.002000003750968687),
                balance!(0.008000015003874750),
                balance!(0.010000018754843438),
                balance!(0.180000337587181890),
            ]);
        })
    }

    #[test]
    fn multiple_pending_distributions_are_executed() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(10000),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 350000, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            // secondary market is initialized without funds
            MockDEXApi::init_without_reserves().unwrap();
            let _distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");
            
            // check pending list and reserves before trade
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance, balance!(0));

            // perform buy on tbc multiple times
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(200.602181641794149028), balance!(0.003009027081243731))
            );
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(200.602931835531681746), balance!(0.003009027081243731))
            );
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(200.603682029269214463), balance!(0.003009027081243731))
            );

            // check pending list and reserves after trade
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            let expected_balances = [balance!(40.120436328358829805), balance!(40.120586367106336349), balance!(40.120736405853842892)];
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, expected_balances[0]), (USDT, expected_balances[1]), (USDT, expected_balances[2])]);
            assert_eq!(free_reserves_balance, expected_balances.iter().fold(balance!(0), |a, b| a + b));

            // exchange becomes available
            MockDEXApi::add_reserves(vec![(XOR, balance!(100000)), (VAL, balance!(100000)), (USDT, balance!(1000000))]).unwrap();
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance, balance!(0));
        })    
    }

    #[test]
    fn large_pending_amount_dont_interfere_new_trades() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(999999999999999),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 0, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");           

            // perform large buy on tbc
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(100000000), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(3789817571942.618173119057163101), balance!(300902.708124373119358074))
            );

            // check that failed distribution was postponed
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance)]);
            assert_eq!(free_reserves_balance, balance!(757963514388.523634623811432620));

            // attempt for distribution
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance_2 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance_2)]);
            assert_eq!(free_reserves_balance_2, free_reserves_balance);

            // another exchange with reasonable amount
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(100), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(7529503.255499584322288265), balance!(0.300902708124373119))
            );

            // second distribution was successful, pending list didn't change
            let free_reserves_balance_3 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance_3)]);
            assert_eq!(free_reserves_balance_3, free_reserves_balance_2);
        })
    }

    #[test]
    fn multiple_pending_distributions_with_large_request_dont_interfere_when_exchange_becomes_available() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(999999999999999),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 350000, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            // secondary market is initialized without funds
            MockDEXApi::init_without_reserves().unwrap();
            let _distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");           

            // perform large buy on tbc
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(100000000), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(3782315634567.290994901504505143), balance!(300902.708124373119358074))
            );

            // another exchange with reasonable amount, still current market can't handle it
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(100), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(7522001.318124257144070739), balance!(0.300902708124373119))
            );

            // attempt for distribution
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            let expected_balances = [balance!(756463126913.458198980300901028), balance!(1504400.263624851428814147)];
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, expected_balances[0]), (USDT, expected_balances[1])]);
            assert_eq!(free_reserves_balance, expected_balances.iter().fold(balance!(0), |a, b| a + b));

            // funds are added and one of exchanges becomes available, unavailable is left as pending
            MockDEXApi::add_reserves(vec![(XOR, balance!(100000)), (VAL, balance!(100000)), (USDT, balance!(1000000))]).unwrap();
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, expected_balances[0])]);
            assert_eq!(free_reserves_balance, expected_balances[0]);
        })
    }

    #[test]
    fn xor_exchange_passes_but_val_exchange_fails_reserves_are_reverted() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(10000),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, 350000, AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            // secondary market is initialized without funds
            MockDEXApi::init_without_reserves().unwrap();
            let distribution_accounts = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");
            
            // perform buy on tbc
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(200.602181641794149028), balance!(0.003009027081243731))
            );

            // check pending list and free reserves account
            let free_reserves_balance = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance.clone())]);
            assert_eq!(free_reserves_balance, balance!(40.120436328358829805));

            // exchange becomes possible, but not for val, so second part of distribution fails
            MockDEXApi::add_reserves(vec![(XOR, balance!(100000)), (VAL, balance!(0)), (USDT, balance!(1000000))]).unwrap();
            
            // check pending list
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance_2 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance.clone())]);
            
            // val buy back and burn failed so exchanged xor is reverted
            assert_eq!(free_reserves_balance_2, free_reserves_balance);
            ensure_distribution_accounts_balances(distribution_accounts.clone(), vec![
                balance!(0),
                balance!(0),
                balance!(0),
                balance!(0),
            ]);
            
            // another buy is performed
            assert_eq!(
                MBCPool::exchange(
                    alice,
                    alice,
                    &DEXId::Polkaswap.into(),
                    &USDT,
                    &XOR,
                    SwapAmount::with_desired_output(balance!(1), Balance::max_value()),
                )
                .unwrap(),
                SwapOutcome::new(balance!(275.622305588803464169), balance!(0.003009027081243731))
            );

            // there are two pending distributions
            let free_reserves_balance_3 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            let second_pending_balance = balance!(55.124461117760692833);
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, free_reserves_balance), (USDT, second_pending_balance)]);
            assert_eq!(free_reserves_balance_3, free_reserves_balance + second_pending_balance);

            // exchange becomes possible, but val is enough only to fulfill one of pending distributions
            MockDEXApi::add_reserves(vec![(VAL, balance!(0.4))]).unwrap();

            // val is not enough for one of distributions, it's still present
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance_4 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![(USDT, second_pending_balance)]);
            assert_eq!(free_reserves_balance_4, second_pending_balance);
            
            // check distribution accounts
            ensure_distribution_accounts_balances(distribution_accounts.clone(), vec![
                balance!(0.002000003750968687),
                balance!(0.008000015003874750),
                balance!(0.010000018754843438),
                balance!(0.180000337587181890),
            ]);

            // enough val is added to fulfill second exchange
            MockDEXApi::add_reserves(vec![(VAL, balance!(1))]).unwrap();

            // second pending distribution is performed
            MBCPool::on_initialize(RETRY_DISTRIBUTION_FREQUENCY.into());
            let free_reserves_balance_5 = Assets::free_balance(&USDT, &MBCPool::free_reserves_account_id()).unwrap();
            assert_eq!(MBCPool::pending_free_reserves(), vec![]);
            assert_eq!(free_reserves_balance_5, balance!(0));
            
            // check distribution accounts
            ensure_distribution_accounts_balances(distribution_accounts, vec![
                balance!(0.004747958137689057),
                balance!(0.018991832550756232),
                balance!(0.023739790688445290),
                balance!(0.427316232392015238),
            ]);
        })
    }

    #[test]
    fn rewards_for_small_values() {
        let mut ext = ExtBuilder::new(vec![
            (
                alice(),
                USDT,
                balance!(0),
                AssetSymbol(b"USDT".to_vec()),
                AssetName(b"Tether USD".to_vec()),
                18,
            ),
            (alice(), XOR, balance!(6000000000), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, 0, AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init_without_reserves().unwrap();
            let _ = bonding_curve_pool_init(Vec::new()).unwrap();
            let alice = &alice();
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, USDT).expect("Failed to register trading pair.");
            TradingPair::register(Origin::signed(alice.clone()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(USDT, false).expect("Failed to initialize pool.");
            let reward = MBCPool::calculate_buy_reward(alice, &USDT, balance!(0.000000002499999999), balance!(0.000000000000001));
            assert_eq!(reward.unwrap(), balance!(0.000000002499999999));
            let reward = MBCPool::calculate_buy_reward(alice, &USDT, balance!(0.000000002499999999), balance!(0.000000000000000001));
            assert_eq!(reward.unwrap(), balance!(0.000000002499999999));
        })
    }

    #[test]
    fn price_without_impact_small_amount() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(4000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            // Buy with desired input
            let amount_a: Balance = balance!(2000);
            let quote_outcome_a = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
            )
            .unwrap();
            let quote_without_impact_a = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
            )
            .unwrap();
            assert_eq!(quote_outcome_a.amount, balance!(361.549938632002697101));
            assert_eq!(quote_without_impact_a.amount, balance!(361.728370440936309235));
            assert!(quote_outcome_a.amount < quote_without_impact_a.amount);

            // Buy with desired output
            let amount_b: Balance = balance!(200);
            let quote_outcome_b = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
            )
            .unwrap();
            let quote_without_impact_b = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
            )
            .unwrap();
            assert_eq!(quote_outcome_b.amount, balance!(1107.192203724646374582));
            assert_eq!(quote_without_impact_b.amount, balance!(1106.890317630040503506));
            assert!(quote_outcome_b.amount > quote_without_impact_b.amount);

            // Sell with desired input
            let amount_c: Balance = balance!(1);
            let quote_outcome_c = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(amount_c.clone()),
            )
            .unwrap();
            let quote_without_impact_c = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(amount_c.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
            )
            .unwrap();
            assert_eq!(quote_outcome_c.amount, balance!(3.999482655569353236));
            assert_eq!(quote_without_impact_c.amount, balance!(4.005928040448516546));
            assert!(quote_outcome_c.amount < quote_without_impact_c.amount);

            // Sell with desired output
            let amount_d: Balance = balance!(1);
            let quote_outcome_d = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(amount_d.clone()),
            )
            .unwrap();
            let quote_without_impact_d = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(amount_d.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
            )
            .unwrap();
            assert_eq!(quote_outcome_d.amount, balance!(0.249731351108007183));
            assert_eq!(quote_without_impact_d.amount, balance!(0.249630724163152921));
            assert!(quote_outcome_d.amount > quote_without_impact_d.amount);
        });
    }

    #[test]
    fn price_without_impact_large_amount() {
        let mut ext = ExtBuilder::new(vec![
            (alice(), DAI, balance!(0), AssetSymbol(b"DAI".to_vec()), AssetName(b"DAI".to_vec()), 18),
            (alice(), USDT, balance!(0), AssetSymbol(b"USDT".to_vec()), AssetName(b"Tether USD".to_vec()), 18),
            (alice(), XOR, balance!(0), AssetSymbol(b"XOR".to_vec()), AssetName(b"SORA".to_vec()), 18),
            (alice(), VAL, balance!(200000), AssetSymbol(b"VAL".to_vec()), AssetName(b"SORA Validator Token".to_vec()), 18),
            (alice(), XSTUSD, 0, AssetSymbol(b"XSTUSD".to_vec()), AssetName(b"XOR Synthetic USD".to_vec()), 18),
        ])
        .build();
        ext.execute_with(|| {
            MockDEXApi::init().unwrap();
            let _ = bonding_curve_pool_init(vec![]).unwrap();
            TradingPair::register(Origin::signed(alice()),DEXId::Polkaswap.into(), XOR, VAL).expect("Failed to register trading pair.");
            MBCPool::initialize_pool_unchecked(VAL, false).expect("Failed to initialize pool.");

            // Buy with desired input
            let amount_a: Balance = balance!(70000);
            let quote_outcome_a = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
            )
            .unwrap();
            let quote_without_impact_a = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_input(amount_a.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_input(amount_a.clone(), Balance::zero()),
            )
            .unwrap();
            assert_eq!(quote_outcome_a.amount, balance!(12448.948798121038075579));
            assert_eq!(quote_without_impact_a.amount, balance!(12660.492965432770823211));
            assert!(quote_outcome_a.amount < quote_without_impact_a.amount);

            // Buy with desired output
            let amount_b: Balance = balance!(14000);
            let quote_outcome_b = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
            )
            .unwrap();
            let quote_without_impact_b = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                QuoteAmount::with_desired_output(amount_b.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &VAL,
                &XOR,
                SwapAmount::with_desired_output(amount_b.clone(), Balance::max_value()),
            )
            .unwrap();
            assert_eq!(quote_outcome_b.amount, balance!(81508.213505580992099145));
            assert_eq!(quote_without_impact_b.amount, balance!(80028.971642012224670009));
            assert!(quote_outcome_b.amount > quote_without_impact_b.amount);

            // Sell with desired input
            let amount_c: Balance = balance!(7000);
            let quote_outcome_c = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(amount_c.clone()),
            )
            .unwrap();
            let quote_without_impact_c = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_input(amount_c.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_input(amount_c.clone(), Balance::zero()),
            )
            .unwrap();
            assert_eq!(quote_outcome_c.amount, balance!(25316.104888559067751287));
            assert_eq!(quote_without_impact_c.amount, balance!(31999.826368133346115316));
            assert!(quote_outcome_c.amount < quote_without_impact_c.amount);

            // Sell with desired output
            let amount_d: Balance = balance!(7000);
            let quote_outcome_d = MBCPool::quote(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(amount_d.clone()),
            )
            .unwrap();
            let quote_without_impact_d = MBCPool::quote_without_impact(
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                QuoteAmount::with_desired_output(amount_d.clone()),
            )
            .unwrap();
            MBCPool::exchange(
                &alice(),
                &alice(),
                &DEXId::Polkaswap.into(),
                &XOR,
                &VAL,
                SwapAmount::with_desired_output(amount_d.clone(), Balance::max_value()),
            )
            .unwrap();
            assert_eq!(quote_outcome_d.amount, balance!(1681.732720328623106894));
            assert_eq!(quote_without_impact_d.amount, balance!(1558.966302104893601417));
            assert!(quote_outcome_d.amount > quote_without_impact_d.amount);
        });
    }
}
