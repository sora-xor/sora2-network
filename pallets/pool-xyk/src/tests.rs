use crate::mock::*;
use common::{prelude::SwapAmount, AssetSymbol, ToFeeAccount};
use frame_support::{assert_noop, assert_ok};

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
            assert_ok!(assets::Module::<Testtime>::register(
                Origin::signed(ALICE()),
                GoldenTicket.into(),
                AssetSymbol(b"GT".to_vec()),
                18
            ));

            assert_ok!(dex_manager::Module::<Testtime>::initialize_dex(
                Origin::signed(BOB()),
                dex_id.clone(),
                GoldenTicket.into(),
                BOB(),
                None,
                None
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

            assert_ok!(assets::Module::<Testtime>::register(
                Origin::signed(ALICE()),
                BlackPepper.into(),
                AssetSymbol(b"BP".to_vec()),
                18
            ));

            assert_ok!(assets::Module::<Testtime>::mint_to(
                &gt,
                &ALICE(),
                &ALICE(),
                900_000u32.into()
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
                360_000u32.into(),
                144_000u32.into(),
                360_000u32.into(),
                144_000u32.into(),
            ));
        }];
        let mut tests_to_add = tests.clone();
        new_tests.append(&mut tests_to_add);
        crate::Module::<Testtime>::preset01(new_tests);
    }
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
                999360_000u32.into(),
                999144_000u32.into(),
                360_000u32.into(),
                144_000u32.into(),
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
                360_000u32.into(),
                999_000u32.into(),
                350_000u32.into(),
                145_000u32.into(),
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
            360_000u32.into(),
            999_000u32.into(),
            350_000u32.into(),
            143_000u32.into(),
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
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                457_470u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1889_000u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                442_500u32
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
                30_u32
            );

            // a = ( 442_500 * 111_000 )  / 1999_999_000
            // b = 442_500 / a
            // c = 111_000 / a
            // Testing this line with noop
            // fail for each asset min, after this success.

            // First minimum is above boundaries.
            assert_noop!(
                crate::Module::<Testtime>::withdraw_liquidity(
                    Origin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    1999_999_000_u32.into(),
                    18_100_u32.into(),
                    4_500_u32.into()
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
                    1999_999_000_u32.into(),
                    18_000_u32.into(),
                    4_600_u32.into()
                ),
                crate::Error::<Testtime>::CalculatedValueIsNotMeetsRequiredBoundaries
            );

            // Both minimums is below.
            assert_ok!(crate::Module::<Testtime>::withdraw_liquidity(
                Origin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                1999_999_000_u32.into(),
                18_000_u32.into(),
                4_500_u32.into(),
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                475_488u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1893_519u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                424_481u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                106_480u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                30_u32
            );

            assert_ok!(crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ));

            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                343_903u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1926_519u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                556_036u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                73_480u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                60_u32
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
                    desired_amount_in: 33_000u32.into(),
                    min_amount_out: 0_u32.into(),
                }
            ));
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&gt, &ALICE()).unwrap()),
                506_970u32
            );
            assert_eq!(
                Into::<u32>::into(assets::Module::<Testtime>::free_balance(&bp, &ALICE()).unwrap()),
                1869_200u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &repr.clone()).unwrap()
                ),
                393_000u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&bp, &repr.clone()).unwrap()
                ),
                130_800u32
            );
            assert_eq!(
                Into::<u32>::into(
                    assets::Module::<Testtime>::free_balance(&gt, &fee_repr.clone()).unwrap()
                ),
                30_u32
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
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ),
            technical::Error::<Testtime>::TechAccountIdIsNotRegistered
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
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ),
            technical::Error::<Testtime>::TechAccountIdIsNotRegistered
        );
    }]);
}

#[test]
fn swap_pair_large_swap_fail_with_out_of_bounds() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 99999_000u32.into(),
                    max_amount_in: 99999999_u32.into(),
                }
            ),
            crate::Error::<Testtime>::CalculatedValueIsOutOfDesiredBounds
        );
    }]);
}

#[test]
fn swap_pair_large_swap_fail_with_source_balance_not_large_enouth() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 999999u32.into(),
                    max_amount_in: 999999999u32.into(),
                }
            ),
            crate::Error::<Testtime>::SourceBalanceIsNotLargeEnough
        );
    }]);
}

#[test]
fn swap_pair_swap_fail_with_target_balance_not_large_enoth() {
    crate::Module::<Testtime>::preset02(vec![|dex_id, gt, _, _, _, _, _, _| {
        assert_ok!(assets::Module::<Testtime>::mint_to(
            &gt,
            &ALICE(),
            &ALICE(),
            99999_000u32.into()
        ));
        assert_noop!(
            crate::Module::<Testtime>::swap_pair(
                Origin::signed(ALICE()),
                ALICE(),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: 9999_000u32.into(),
                    max_amount_in: 999999999u32.into(),
                }
            ),
            crate::Error::<Testtime>::TargetBalanceIsNotLargeEnough
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
                    desired_amount_out: 33_000u32.into(),
                    max_amount_in: 999999999u32.into(),
                }
            ),
            crate::Error::<Testtime>::AccountBalanceIsInvalid
        );
    }]);
}
