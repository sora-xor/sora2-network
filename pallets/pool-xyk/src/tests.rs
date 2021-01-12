use common::{
    fixed,
    prelude::{SwapAmount, SwapOutcome},
    AssetSymbol, LiquiditySource, LiquiditySourceType, ToFeeAccount,
};
use frame_support::{assert_noop, assert_ok};

use crate::mock::*;

impl crate::Module<Testtime> {
    fn preset01(
        tests: Vec<
            fn(
                crate::mock::DEXId,
                AssetId,
                AssetId,
                common::TradingPair<crate::mock::TechAssetId>,
                crate::mock::TechAccountId,
                crate::mock::TechAccountId,
                AccountId,
                AccountId,
            ) -> (),
        >,
    ) {
        let mut ext = ExtBuilder::default().build();
        let dex_id = 220;
        let gt: crate::mock::AssetId = GoldenTicket.into();
        let bp: crate::mock::AssetId = BlackPepper.into();

        ext.execute_with(|| {
            assert_ok!(assets::Module::<Testtime>::register_asset_id(
                ALICE(),
                GoldenTicket.into(),
                AssetSymbol(b"GT".to_vec()),
                18
            ));

            assert_ok!(assets::Module::<Testtime>::register_asset_id(
                ALICE(),
                BlackPepper.into(),
                AssetSymbol(b"BP".to_vec()),
                18
            ));

            assert_ok!(dex_manager::Module::<Testtime>::initialize_dex(
                Origin::signed(BOB()),
                dex_id.clone(),
                GoldenTicket.into(),
                BOB(),
                true,
            ));

            assert_ok!(trading_pair::Module::<Testtime>::register(
                Origin::signed(BOB()),
                dex_id.clone(),
                GoldenTicket.into(),
                BlackPepper.into()
            ));

            assert_ok!(crate::Module::<Testtime>::initialize_pool(
                Origin::signed(BOB()),
                dex_id.clone(),
                GoldenTicket.into(),
                BlackPepper.into(),
            ));

            assert!(
                trading_pair::Module::<Testtime>::is_source_enabled_for_trading_pair(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    LiquiditySourceType::XYKPool,
                )
                .expect("Failed to query trading pair status.")
            );

            let (tpair, tech_acc_id) =
                crate::Module::<Testtime>::tech_account_from_dex_and_asset_pair(
                    dex_id.clone(),
                    GoldenTicket.into(),
                    BlackPepper.into(),
                )
                .unwrap();

            let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
            let repr: AccountId =
                technical::Module::<Testtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
            let fee_repr: AccountId =
                technical::Module::<Testtime>::tech_account_id_to_account_id(&fee_acc).unwrap();

            assert_ok!(assets::Module::<Testtime>::mint_to(
                &gt,
                &ALICE(),
                &ALICE(),
                fixed!(900000)
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                900_000u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                2000_000u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                0u32
            );

            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                0u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                0_u32
            );

            let base_asset: AssetId = GoldenTicket.into();
            let target_asset: AssetId = BlackPepper.into();
            let tech_asset: AssetId = crate::Module::<Testtime>::get_marking_asset(&tech_acc_id)
                .expect("Failed to get marking asset")
                .into();
            assert_eq!(
                crate::Module::<Testtime>::properties(base_asset, target_asset),
                Some((repr.clone(), fee_repr.clone(), tech_asset))
            );
            assert_eq!(
                pswap_distribution::Module::<Testtime>::subscribed_accounts(&fee_repr),
                Some((
                    dex_id.clone(),
                    tech_asset,
                    GetDefaultSubscriptionFrequency::get(),
                    0
                ))
            );

            for test in &tests {
                test(
                    dex_id.clone(),
                    gt.clone(),
                    bp.clone(),
                    tpair.clone(),
                    tech_acc_id.clone(),
                    fee_acc.clone(),
                    repr.clone(),
                    fee_repr.clone(),
                );
            }
        });
    }

    fn preset02(
        tests: Vec<
            fn(
                crate::mock::DEXId,
                AssetId,
                AssetId,
                common::TradingPair<crate::mock::TechAssetId>,
                crate::mock::TechAccountId,
                crate::mock::TechAccountId,
                AccountId,
                AccountId,
            ) -> (),
        >,
    ) {
        let mut new_tests: Vec<
            fn(
                crate::mock::DEXId,
                AssetId,
                AssetId,
                common::TradingPair<crate::mock::TechAssetId>,
                crate::mock::TechAccountId,
                crate::mock::TechAccountId,
                AccountId,
                AccountId,
            ) -> (),
        > = vec![|dex_id, _, _, _, _, _, _, _| {
            assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
                Origin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                fixed!(360000),
                fixed!(144000),
                fixed!(360000),
                fixed!(144000),
            ));
        }];
        let mut tests_to_add = tests.clone();
        new_tests.append(&mut tests_to_add);
        crate::Module::<Testtime>::preset01(new_tests);
    }
}

macro_rules! simplify_swap_outcome(
     ($a: expr) => ({
         match $a {
             SwapOutcome { amount, fee } => (amount.into(), fee.into())
         }
     })
);

#[test]
fn can_exchange_all_directions() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, gt, bp, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(100000),
            fixed!(200000),
            fixed!(0),
            fixed!(0),
        ));
        assert!(crate::Module::<Testtime>::can_exchange(&dex_id, &gt, &bp));
        assert!(crate::Module::<Testtime>::can_exchange(&dex_id, &bp, &gt));
        // TODO: add tests for indirect exchange, i.e. both input and output are not base asset
    }]);
}

#[test]
fn quote_case_exact_input_for_output_base_first() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, gt, bp, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(100000),
            fixed!(200000),
            fixed!(0),
            fixed!(0),
        ));
        assert_eq!(
            simplify_swap_outcome!(crate::Module::<Testtime>::quote(
                &dex_id,
                &gt,
                &bp,
                SwapAmount::WithDesiredInput {
                    desired_amount_in: fixed!(100000),
                    min_amount_out: fixed!(50000),
                }
            )
            .unwrap()),
            (99_850_u32, 300_u32)
        );
    }]);
}

#[test]
fn quote_case_exact_input_for_output_base_second() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, gt, bp, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(100000),
            fixed!(200000),
            fixed!(0),
            fixed!(0),
        ));
        assert_eq!(
            simplify_swap_outcome!(crate::Module::<Testtime>::quote(
                &dex_id,
                &bp,
                &gt,
                SwapAmount::WithDesiredInput {
                    desired_amount_in: fixed!(100000),
                    min_amount_out: fixed!(0),
                }
            )
            .unwrap()),
            (33_233_u32, 100_u32)
        );
    }]);
}

#[test]
fn quote_case_exact_output_for_input_base_first() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, gt, bp, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(100000),
            fixed!(200000),
            fixed!(0),
            fixed!(0),
        ));
        assert_eq!(
            simplify_swap_outcome!(crate::Module::<Testtime>::quote(
                &dex_id,
                &gt,
                &bp,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(100000),
                    max_amount_in: fixed!(150000),
                }
            )
            .unwrap()),
            (100_301_u32, 301_u32)
        );
    }]);
}

#[test]
fn quote_case_exact_output_for_input_base_second() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, gt, bp, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(100000),
            fixed!(200000),
            fixed!(0),
            fixed!(0),
        ));
        assert_eq!(
            simplify_swap_outcome!(crate::Module::<Testtime>::quote(
                &dex_id,
                &bp,
                &gt,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(50000),
                    max_amount_in: fixed!(999000),
                }
            )
            .unwrap()),
            (201_057_u32, 150_u32)
        );
    }]);
}

#[test]
fn quote_case_exact_output_for_input_base_second_fail_with_out_of_bounds() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, gt, bp, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(100000),
            fixed!(200000),
            fixed!(0),
            fixed!(0),
        ));
        assert_noop!(
            crate::Module::<Testtime>::quote(
                &dex_id,
                &bp,
                &gt,
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(50000),
                    max_amount_in: fixed!(90000),
                }
            ),
            crate::Error::<Testtime>::CalculatedValueIsOutOfDesiredBounds
        );
    }]);
}

#[test]
fn depositliq_large_values() {
    crate::Module::<Testtime>::preset01(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::deposit_liquidity(
                Origin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                fixed!(999360000),
                fixed!(999144000),
                fixed!(360000),
                fixed!(144000),
            ),
            crate::Error::<Testtime>::SourceBaseAmountIsNotLargeEnough
        );
    }]);
}

#[test]
fn depositliq_invalid_range() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::deposit_liquidity(
                Origin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                fixed!(360000),
                fixed!(999000),
                fixed!(350000),
                fixed!(145000),
            ),
            crate::Error::<Testtime>::ImposibleToDecideValidPairValuesFromRangeForThisPool
        );
    }]);
}

#[test]
fn depositliq_valid_range_but_desired_is_corrected() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_ok!(crate::Module::<Testtime>::deposit_liquidity(
            Origin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            fixed!(360000),
            fixed!(999000),
            fixed!(350000),
            fixed!(143000),
        ));
    }]);
}

#[test]
fn pool_is_already_initialized_and_other_after_depositliq() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, gt, bp, _, _, _, repr: AccountId, fee_repr: AccountId| {
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                144_000u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                360_000_u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &fee_repr.clone()).unwrap()
                ),
                0_u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                0_u32
            );

            assert_noop!(
                crate::Module::<Testtime>::initialize_pool(
                    Origin::signed(BOB()),
                    dex_id.clone(),
                    GoldenTicket.into(),
                    BlackPepper.into(),
                ),
                crate::Error::<Testtime>::PoolIsAlreadyInitialized
            );
        },
    ]);
}

#[test]
fn swap_pair_desired_output_and_withdraw_cascade() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, gt, bp, _, _, _, repr: AccountId, fee_repr: AccountId| {
            assert_ok!(crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(33000),
                    max_amount_in: fixed!(99999999),
                }
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                432651u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1889_000u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                467_027u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                111_000u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                322_u32
            );

            // a = sqrt ( 467027 * 111000 ) / 8784 = 25.92001146000573
            // b = 467_027 / a = 18018.00900900901
            // c = 111_000 / a = 4282.405514028097
            // Testing this line with noop
            // fail for each asset min, after this success.

            // First minimum is above boundaries.
            assert_noop!(
                crate::Module::<Testtime>::withdraw_liquidity(
                    Origin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    fixed!(8784),
                    fixed!(18100),
                    fixed!(4100)
                ),
                crate::Error::<Testtime>::CalculatedValueIsNotMeetsRequiredBoundaries
            );

            // Second minimum is above boundaries.
            assert_noop!(
                crate::Module::<Testtime>::withdraw_liquidity(
                    Origin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    fixed!(8784),
                    fixed!(18000),
                    fixed!(4300)
                ),
                crate::Error::<Testtime>::CalculatedValueIsNotMeetsRequiredBoundaries
            );

            // Both minimums is below.
            assert_ok!(crate::Module::<Testtime>::withdraw_liquidity(
                Origin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                fixed!(8784),
                fixed!(18000),
                fixed!(4200),
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                450669u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1893_282u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                449_009u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                106_718u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                322_u32
            );

            assert_ok!(crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(33000),
                    max_amount_in: fixed!(99999999),
                }
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                249063u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1926_282u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                650_010u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                73_718u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                927_u32
            );
        },
    ]);
}

#[test]
fn swap_pair_desired_input() {
    crate::Module::<Testtime>::preset02(vec![
        |dex_id, gt, bp, _, _, _, repr: AccountId, fee_repr: AccountId| {
            assert_ok!(crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredInput {
                    desired_amount_in: fixed!(33000),
                    min_amount_out: fixed!(0),
                }
            ));
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                507_000u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1868_058u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                392_901u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                131942u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                99_u32
            );
        },
    ]);
}

#[test]
fn swap_pair_invalid_dex_id() {
    crate::Module::<Testtime>::preset02(vec![|_, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                380,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(33000),
                    max_amount_in: fixed!(99999999),
                }
            ),
            dex_manager::Error::<Testtime>::DEXDoesNotExist //technical::Error::<Testtime>::TechAccountIdIsNotRegistered
        );
    }]);
}

#[test]
fn swap_pair_different_asset_pair() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                RedPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(33000),
                    max_amount_in: fixed!(99999999),
                }
            ),
            technical::Error::<Testtime>::TechAccountIdIsNotRegistered
        );
    }]);
}

#[test]
fn swap_pair_swap_fail_with_invalid_balance() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(BOB()),
                BOB(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: fixed!(33000),
                    max_amount_in: fixed!(999999999),
                }
            ),
            crate::Error::<Testtime>::AccountBalanceIsInvalid
        );
    }]);
}
