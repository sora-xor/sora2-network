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

use core::str::FromStr;

use common::alt::{DiscreteQuotation, SideAmount, SwapChunk, SwapLimits};
use common::prelude::{FixedWrapper, OutcomeFee, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, fixed, AssetInfoProvider, AssetName, AssetSymbol, Balance, LiquiditySource,
    LiquiditySourceType, Oracle, ToFeeAccount, TradingPairSourceManager, DEFAULT_BALANCE_PRECISION,
};
use frame_support::assert_ok;
use frame_support::{assert_err, assert_noop};

use crate::mock::*;
use crate::{PoolProviders, TotalIssuances};
use sp_std::collections::vec_deque::VecDeque;
use sp_std::rc::Rc;

type PresetFunction<'a> = Rc<
    dyn Fn(
            crate::mock::DEXId,
            AssetId,
            AssetId,
            AssetId,
            common::TradingPair<crate::mock::TechAssetId>,
            crate::mock::TechAccountId,
            crate::mock::TechAccountId,
            AccountId,
            AccountId,
        ) -> ()
        + 'a,
>;

#[derive(Clone)]
struct RunTestsWithSlippageBehaviors<'a> {
    initial_deposit: (Balance, Balance),
    desired_amount: Balance,
    tests: Vec<PresetFunction<'a>>,
}

impl<'a> crate::Pallet<Runtime> {
    fn preset_initial(tests: Vec<PresetFunction<'a>>) {
        let mut ext = ExtBuilder::default().build();
        let dex_id = DEX_A_ID;
        let gt: crate::mock::AssetId = GoldenTicket.into();
        let bp: crate::mock::AssetId = BlackPepper.into();
        let pt: crate::mock::AssetId = Potato.into();

        ext.execute_with(|| {
            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE(),
                GoldenTicket.into(),
                AssetSymbol(b"GT".to_vec()),
                AssetName(b"Golden Ticket".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE(),
                BlackPepper.into(),
                AssetSymbol(b"BP".to_vec()),
                AssetName(b"Black Pepper".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
                ALICE(),
                Potato.into(),
                AssetSymbol(b"PT".to_vec()),
                AssetName(b"Potato".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                Balance::from(0u32),
                true,
                None,
                None,
            ));

            assert_ok!(trading_pair::Pallet::<Runtime>::register(
                RuntimeOrigin::signed(BOB()),
                dex_id.clone(),
                GoldenTicket.into(),
                BlackPepper.into()
            ));

            assert_ok!(crate::Pallet::<Runtime>::initialize_pool(
                RuntimeOrigin::signed(BOB()),
                dex_id.clone(),
                GoldenTicket.into(),
                BlackPepper.into(),
            ));

            assert!(
                trading_pair::Pallet::<Runtime>::is_source_enabled_for_trading_pair(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    LiquiditySourceType::XYKPool,
                )
                .expect("Failed to query trading pair status.")
            );

            let (tpair, tech_acc_id) =
                crate::Pallet::<Runtime>::tech_account_from_dex_and_asset_pair(
                    dex_id.clone(),
                    GoldenTicket.into(),
                    BlackPepper.into(),
                )
                .unwrap();

            let fee_acc = tech_acc_id.clone().to_fee_account().unwrap();
            let repr: AccountId =
                technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_acc_id).unwrap();
            let fee_repr: AccountId =
                technical::Pallet::<Runtime>::tech_account_id_to_account_id(&fee_acc).unwrap();

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &gt,
                &ALICE(),
                &ALICE(),
                balance!(900000)
            ));

            assert_ok!(assets::Pallet::<Runtime>::mint_to(
                &gt,
                &ALICE(),
                &CHARLIE(),
                balance!(900000)
            ));

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                balance!(900000)
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap(),
                balance!(2000000)
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&gt, &repr.clone()).unwrap(),
                0
            );

            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&bp, &repr.clone()).unwrap(),
                0
            );
            assert_eq!(
                assets::Pallet::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                0
            );

            let base_asset: AssetId = GoldenTicket.into();
            let target_asset: AssetId = BlackPepper.into();
            assert_eq!(
                crate::Pallet::<Runtime>::properties(base_asset, target_asset),
                Some((repr.clone(), fee_repr.clone()))
            );
            assert_eq!(
                pswap_distribution::Pallet::<Runtime>::subscribed_accounts(&fee_repr),
                Some((
                    dex_id.clone(),
                    repr.clone(),
                    GetDefaultSubscriptionFrequency::get(),
                    0
                ))
            );

            for test in &tests {
                test(
                    dex_id.clone(),
                    gt.clone(),
                    bp.clone(),
                    pt.clone(),
                    tpair.clone(),
                    tech_acc_id.clone(),
                    fee_acc.clone(),
                    repr.clone(),
                    fee_repr.clone(),
                );
            }
        });
    }

    fn preset_custom_deposited_pool(
        with_chameleon: bool,
        base_amount: Balance,
        min_base_amount: Balance,
        target_amount: Balance,
        min_target_amount: Balance,
        pool_tokens: Balance,
        tests: Vec<PresetFunction<'a>>,
    ) {
        let mut new_tests: Vec<PresetFunction> = vec![Rc::new(
            |dex_id, _, _, _, _, _tech_acc_id: crate::mock::TechAccountId, _, pool_account, _| {
                assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    base_amount,
                    min_base_amount,
                    target_amount,
                    min_target_amount,
                ));

                if with_chameleon {
                    assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        Potato.into(),
                        BlackPepper.into(),
                        base_amount,
                        min_base_amount,
                        target_amount,
                        min_target_amount,
                    ));
                }

                assert_eq!(
                    PoolProviders::<Runtime>::get(pool_account, &ALICE()),
                    Some(pool_tokens),
                );
                //TODO: total supply check
            },
        )];
        let mut tests_to_add = tests.clone();
        new_tests.append(&mut tests_to_add);
        crate::Pallet::<Runtime>::preset_initial(new_tests);
    }

    fn preset_deposited_pool(with_chameleon: bool, tests: Vec<PresetFunction<'a>>) {
        Self::preset_custom_deposited_pool(
            false,
            balance!(360000),
            balance!(144000),
            balance!(360000),
            balance!(144000),
            balance!(227683.9915321233119024),
            tests.clone(),
        );
        if with_chameleon {
            Self::preset_custom_deposited_pool(
                true,
                balance!(180000),
                balance!(72000),
                balance!(180000),
                balance!(72000),
                balance!(227683.991532123311902562),
                tests,
            );
        }
    }

    fn preset_deposited_pool_2(with_chameleon: bool, tests: Vec<PresetFunction<'a>>) {
        Self::preset_custom_deposited_pool(
            false,
            balance!(100000),
            balance!(200000),
            balance!(100000),
            balance!(200000),
            balance!(141421.356237309504878507),
            tests.clone(),
        );
        if with_chameleon {
            Self::preset_custom_deposited_pool(
                true,
                balance!(50000),
                balance!(100000),
                balance!(50000),
                balance!(100000),
                balance!(141421.356237309504878190),
                tests,
            );
        }
    }

    fn run_tests_with_different_slippage_behavior(descriptor: RunTestsWithSlippageBehaviors<'a>) {
        let initial_deposit = descriptor.initial_deposit;
        let desired_amount = descriptor.desired_amount;
        let prepare: PresetFunction<'a> = Rc::new({
            move |dex_id, _, _, _, _, _, _, _, _| {
                assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    initial_deposit.0,
                    initial_deposit.1,
                    initial_deposit.0,
                    initial_deposit.1,
                ));
            }
        });

        // List of cases for different slippage behavior.
        let cases: Vec<PresetFunction<'a>> = vec![
            Rc::new(move |dex_id, _, _, _, _, _, _, _, _| {
                assert_ok!(crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: desired_amount,
                        max_amount_in: balance!(99999999),
                    }
                ));
            }),
            Rc::new(move |dex_id, _, _, _, _, _, _, _, _| {
                assert_ok!(crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    SwapAmount::WithDesiredInput {
                        desired_amount_in: desired_amount,
                        min_amount_out: balance!(0),
                    }
                ));
            }),
        ];

        // Run tests inside each behavior.
        for case in &cases {
            let mut new_tests = vec![prepare.clone(), case.clone()];
            new_tests.append(&mut descriptor.tests.clone());
            crate::Pallet::<Runtime>::preset_initial(new_tests);
        }

        // Case with original pool state, behavior is not prepended.
        let mut new_tests = vec![prepare.clone()];
        new_tests.append(&mut descriptor.tests.clone());
        crate::Pallet::<Runtime>::preset_initial(new_tests);
    }
}

macro_rules! simplify_swap_outcome(
 ($a: expr) => ({
     match $a {
         (SwapOutcome { amount, fee }, _) => (amount, fee)
     }
 })
);

#[test]
fn can_exchange_all_directions() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, pt, _, _, _, _, _| {
            assert!(crate::Pallet::<Runtime>::can_exchange(&dex_id, &gt, &bp));
            assert!(crate::Pallet::<Runtime>::can_exchange(&dex_id, &bp, &gt));
            assert!(crate::Pallet::<Runtime>::can_exchange(&dex_id, &pt, &bp));
            assert!(crate::Pallet::<Runtime>::can_exchange(&dex_id, &bp, &pt));
        })],
    );
}

#[test]
fn quote_case_exact_input_for_output_base_first() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            assert_eq!(
                simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::WithDesiredInput {
                        desired_amount_in: balance!(100000)
                    },
                    true
                )
                .unwrap()),
                (
                    99849774661992989484226,
                    OutcomeFee::from_asset(GoldenTicket.into(), balance!(300))
                )
            );
        })],
    );
}

#[test]
fn test_deducing_fee() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            let (amount_a, fee_a) = simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(100000)
                },
                true
            )
            .unwrap());
            assert_eq!(
                (amount_a, fee_a),
                (
                    99849774661992989484226,
                    OutcomeFee::from_asset(GoldenTicket.into(), balance!(300))
                )
            );
            let (_, fee_b) = simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: balance!(100000)
                },
                false
            )
            .unwrap());
            assert!(fee_b.is_zero_fee());

            let (amount_a, fee_a) = simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(100000)
                },
                true
            )
            .unwrap());
            assert_eq!(
                (amount_a, fee_a),
                (
                    100300902708124373119360,
                    OutcomeFee::from_asset(GoldenTicket.into(), balance!(300.902708124373119358))
                )
            );
            let (_, fee_b) = simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: balance!(100000)
                },
                false
            )
            .unwrap());
            assert!(fee_b.is_zero_fee());
        })],
    );
}

#[test]
fn quote_case_exact_input_for_output_base_second() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            assert_eq!(
                simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &bp,
                    &gt,
                    QuoteAmount::WithDesiredInput {
                        desired_amount_in: balance!(100000)
                    },
                    true
                )
                .unwrap()),
                (
                    balance!(33233.333333333333333333),
                    OutcomeFee::from_asset(GoldenTicket.into(), balance!(100))
                )
            );
        })],
    );
}

#[test]
fn quote_case_exact_output_for_input_base_first() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            assert_eq!(
                simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::WithDesiredOutput {
                        desired_amount_out: balance!(100000)
                    },
                    true,
                )
                .unwrap()),
                (
                    100300902708124373119360,
                    OutcomeFee::from_asset(GoldenTicket.into(), 300902708124373119358)
                )
            );
        })],
    );
}

#[test]
fn quote_case_exact_output_for_input_base_second() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            assert_eq!(
                simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &bp,
                    &gt,
                    QuoteAmount::WithDesiredOutput {
                        desired_amount_out: balance!(5000)
                    },
                    true,
                )
                .unwrap()),
                (
                    10559662090813093980992,
                    OutcomeFee::from_asset(GoldenTicket.into(), 15045135406218655967)
                )
            );
        })],
    );
}

#[test]
fn check_empty_step_quote() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            assert_eq!(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_input(balance!(0)),
                    10,
                    true
                )
                .unwrap()
                .0,
                DiscreteQuotation::new()
            );

            assert_eq!(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_output(balance!(0)),
                    10,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation::new()
            );
        })],
    );
}

#[test]
fn check_step_quote_with_zero_samples_count() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            assert_eq!(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_input(balance!(100)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(
                        balance!(100),
                        balance!(199.800199800199800199),
                        Default::default()
                    )]),
                    limits: Default::default()
                }
            );

            assert_eq!(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_input(balance!(100)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(
                        balance!(100),
                        balance!(199.800199800199800199),
                        Default::default()
                    )]),
                    limits: Default::default()
                }
            );

            assert_eq!(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_output(balance!(200)),
                    0,
                    false
                )
                .unwrap()
                .0,
                DiscreteQuotation {
                    chunks: VecDeque::from([SwapChunk::new(
                        balance!(100.100100100100100100),
                        balance!(200),
                        Default::default()
                    )]),
                    limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(198000))), None)
                }
            );
        })],
    );
}

#[test]
fn check_step_quote_without_fee() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            balance!(100000),
            balance!(200000),
            balance!(100000),
            balance!(200000),
        ));

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_input(balance!(100)),
                10,
                false
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.998000199980001999),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.994001399700061988),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.990003798700421867),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.986007396501561327),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.982012192624199695),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.978018186589295798),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.974025377918047812),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.970033766131893127),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.966043350752508194),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.962054131301808392),
                        Default::default()
                    ),
                ]),
                limits: Default::default()
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_output(balance!(200)),
                10,
                false
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(10.001000100010001000),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.003000700150031006),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.005001900650211067),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.007003701750781337),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.009006103692102153),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.011009106714654105),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.013012711059038105),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.015016916965975462),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.017021724676307957),
                        balance!(20),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(10.019027134430997908),
                        balance!(20),
                        Default::default()
                    ),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(198000))), None)
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(200)),
                10,
                false
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.999000099990000999),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.997000699850030994),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.995001899350210934),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.993003698250780663),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.991006096312099848),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.989009093294647899),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.987012688959023906),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.985016883065946563),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.983021675376254097),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.981027065650904196),
                        Default::default()
                    ),
                ]),
                limits: Default::default()
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(100)),
                10,
                false
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(20.002000200020002002),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.006001400300062012),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.010003801300422133),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.014007403501562674),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.018012207384204307),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.022018213429308210),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.026025422118076210),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.030033833931950924),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.034043449352615913),
                        balance!(10),
                        Default::default()
                    ),
                    SwapChunk::new(
                        balance!(20.038054268861995817),
                        balance!(10),
                        Default::default()
                    ),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(99000))), None)
            }
        );
    })]);
}

#[test]
fn check_step_quote_with_fee() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            balance!(100000),
            balance!(200000),
            balance!(100000),
            balance!(200000),
        ));

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_input(balance!(100)),
                10,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.938012180185635492),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.934037333141407095),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.930063674618442918),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.926091204142949627),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.922119921241369960),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.918149825440382581),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.914180916266901942),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.910213193248078135),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.906246655911296762),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                    SwapChunk::new(
                        balance!(10),
                        balance!(19.902281303784178786),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.03))
                    ),
                ]),
                limits: Default::default()
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_output(balance!(200)),
                10,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(10.031093380150452357),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030093280140451357))
                    ),
                    SwapChunk::new(
                        balance!(10.033100000150482453),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030099300000451447))
                    ),
                    SwapChunk::new(
                        balance!(10.035107222317162555),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030105321666951488))
                    ),
                    SwapChunk::new(
                        balance!(10.037115046891455704),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030111345140674367))
                    ),
                    SwapChunk::new(
                        balance!(10.039123474114445489),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030117370422343336))
                    ),
                    SwapChunk::new(
                        balance!(10.041132504227336114),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030123397512682009))
                    ),
                    SwapChunk::new(
                        balance!(10.043142137471452462),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030129426412414357))
                    ),
                    SwapChunk::new(
                        balance!(10.045152374088240182),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030135457122264720))
                    ),
                    SwapChunk::new(
                        balance!(10.047163214319265755),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030141489642957798))
                    ),
                    SwapChunk::new(
                        balance!(10.049174658406216557),
                        balance!(20),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030147523975218649))
                    ),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(198000))), None)
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(200)),
                10,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.969003099690030996),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029997000299970003))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.967009697750480901),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029991002099550093))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.965016893652160301),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029985005698050633))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.963024687156028321),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029979011094752342))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.961033078023163548),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029973018288936300))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.959042066014763955),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029967027279883944))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.957051650892146835),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029961038066877071))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.955061832416748723),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029955050649197840))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.953072610350125335),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029949065026128762))
                    ),
                    SwapChunk::new(
                        balance!(20),
                        balance!(9.951083984453951483),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.029943081196952713))
                    ),
                ]),
                limits: Default::default()
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(100)),
                10,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(20.062192797672785635),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437311))
                    ),
                    SwapChunk::new(
                        balance!(20.066218117254983225),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.070244648431316120),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.074272391688075365),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.078301347511796002),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.082331516389257222),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.086362898807482507),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.090395495253739781),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.094429306215541556),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                    SwapChunk::new(
                        balance!(20.098464332180645078),
                        balance!(10),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.030090270812437312))
                    ),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(98703))), None)
            }
        );
    })]);
}

fn sum_step_quote<AssetId: Ord + Clone>(
    step_quote_result: DiscreteQuotation<AssetId, Balance>,
) -> (Balance, Balance, OutcomeFee<AssetId, Balance>) {
    let (step_quote_input, step_quote_output, step_quote_fee) =
        step_quote_result.chunks.iter().fold(
            (balance!(0), balance!(0), OutcomeFee::default()),
            |acc, item| {
                (
                    acc.0 + item.input,
                    acc.1 + item.output,
                    acc.2.merge(item.fee.clone()),
                )
            },
        );
    (step_quote_input, step_quote_output, step_quote_fee)
}

fn compare_quotes(
    dex_id: &DEXId,
    input_asset_id: &AssetId,
    output_asset_id: &AssetId,
    amount: QuoteAmount<Balance>,
    deduce_fee: bool,
) {
    let (step_quote_input, step_quote_output, step_quote_fee) = sum_step_quote(
        crate::Pallet::<Runtime>::step_quote(
            dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            10,
            deduce_fee,
        )
        .unwrap()
        .0,
    );

    let quote_result = crate::Pallet::<Runtime>::quote(
        dex_id,
        input_asset_id,
        output_asset_id,
        amount,
        deduce_fee,
    )
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
    assert_eq!(step_quote_fee, quote_fee);
}

#[test]
fn check_step_quote_equal_with_qoute() {
    crate::Pallet::<Runtime>::preset_deposited_pool_2(
        true,
        vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
            compare_quotes(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_input(balance!(100)),
                false,
            );
            compare_quotes(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_output(balance!(100)),
                false,
            );

            compare_quotes(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(100)),
                false,
            );
            compare_quotes(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(100)),
                false,
            );

            compare_quotes(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_input(balance!(100)),
                true,
            );
            compare_quotes(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            );

            compare_quotes(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(100)),
                true,
            );
            compare_quotes(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(100)),
                true,
            );
        })],
    );
}

#[test]
fn check_exceed_reserves() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
        let gt_reserve = balance!(100000);
        let bp_reserve = balance!(200000);

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            gt_reserve,
            bp_reserve,
            gt_reserve,
            bp_reserve,
        ));

        // quote

        assert_eq!(
            simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: gt_reserve + balance!(1)
                },
                true
            )
            .unwrap()),
            (
                balance!(99850.274658368380604529),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(300.003))
            )
        );

        // error when desired output exceeds reserves
        assert_noop!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: bp_reserve + balance!(1)
                },
                true
            ),
            crate::Error::<Runtime>::FixedWrapperCalculationFailed
        );

        assert_eq!(
            simplify_swap_outcome!(crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::WithDesiredInput {
                    desired_amount_in: bp_reserve + balance!(1)
                },
                true
            )
            .unwrap()),
            (
                balance!(49850.124624688438278904),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(150.000374999062502344))
            )
        );

        // error when desired output exceeds reserves
        assert_noop!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::WithDesiredOutput {
                    desired_amount_out: gt_reserve + balance!(1)
                },
                true
            ),
            crate::Error::<Runtime>::FixedWrapperCalculationFailed
        );

        // step quote

        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::WithDesiredInput {
                        desired_amount_in: gt_reserve + balance!(1)
                    },
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (
                gt_reserve + balance!(1),
                balance!(99850.274658368380604529),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(300.003))
            )
        );

        // no error for step_quote
        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::WithDesiredOutput {
                        desired_amount_out: bp_reserve + balance!(1)
                    },
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (
                balance!(9929789.368104312938821464),
                balance!(198000),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(29789.368104312938816464))
            )
        );

        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &bp,
                    &gt,
                    QuoteAmount::WithDesiredInput {
                        desired_amount_in: bp_reserve + balance!(1)
                    },
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (
                bp_reserve + balance!(1),
                balance!(49850.124624688438278904),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(150.000374999062502344))
            )
        );

        // no error for step_quote
        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &bp,
                    &gt,
                    QuoteAmount::WithDesiredOutput {
                        desired_amount_out: gt_reserve + balance!(1)
                    },
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (
                balance!(19800000.00000000000002),
                balance!(98703),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(297))
            )
        );
    })]);
}

#[test]
fn check_empty_reserves() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, gt, bp, _, _, _, _, _, _| {
        // don't deposit any liquidity

        // error for quote

        assert_noop!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_input(balance!(1)),
                true
            ),
            crate::Error::<Runtime>::PoolIsEmpty
        );

        assert_noop!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &gt,
                &bp,
                QuoteAmount::with_desired_output(balance!(1)),
                true
            ),
            crate::Error::<Runtime>::PoolIsEmpty
        );

        assert_noop!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(1)),
                true
            ),
            crate::Error::<Runtime>::PoolIsEmpty
        );

        assert_noop!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(1)),
                true
            ),
            crate::Error::<Runtime>::PoolIsEmpty
        );

        // no error for step_quote

        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_input(balance!(1)),
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (balance!(0), balance!(0), Default::default())
        );

        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &gt,
                    &bp,
                    QuoteAmount::with_desired_output(balance!(1)),
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (balance!(0), balance!(0), Default::default())
        );

        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &bp,
                    &gt,
                    QuoteAmount::with_desired_input(balance!(1)),
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (balance!(0), balance!(0), Default::default())
        );

        assert_eq!(
            sum_step_quote(
                crate::Pallet::<Runtime>::step_quote(
                    &dex_id,
                    &bp,
                    &gt,
                    QuoteAmount::with_desired_output(balance!(1)),
                    10,
                    true,
                )
                .unwrap()
                .0,
            ),
            (balance!(0), balance!(0), Default::default())
        );
    })]);
}

#[test]
// Deposit to an empty pool
fn deposit_less_than_minimum_1() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                balance!(0.00001),
                balance!(100),
                balance!(0.00001),
                balance!(100),
            ),
            crate::Error::<Runtime>::UnableToDepositXorLessThanMinimum
        );
    })]);
}

#[test]
// Deposit to an already existing pool
fn deposit_less_than_minimum_2() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_noop!(
                crate::Pallet::<Runtime>::deposit_liquidity(
                    RuntimeOrigin::signed(CHARLIE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    balance!(0.00025),
                    balance!(0.0001),
                    balance!(0.00025),
                    balance!(0.0001),
                ),
                crate::Error::<Runtime>::UnableToDepositXorLessThanMinimum
            );
            assert_noop!(
                crate::Pallet::<Runtime>::deposit_liquidity(
                    RuntimeOrigin::signed(CHARLIE()),
                    dex_id,
                    Potato.into(),
                    BlackPepper.into(),
                    balance!(0.00025),
                    balance!(0.0001),
                    balance!(0.00025),
                    balance!(0.0001),
                ),
                crate::Error::<Runtime>::UnableToDepositXorLessThanMinimum
            );
        })],
    );
}

#[test]
// Deposit to an already existing pool, but you're in the pool already
fn deposit_less_than_minimum_3() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                balance!(0.00025),
                balance!(0.0001),
                balance!(0.00025),
                balance!(0.0001),
            ),);
        })],
    );
}

#[test]
// Deposit to an already existing pool, but you're in the pool already
fn deposit_less_than_minimum_3_chameleon() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                Potato.into(),
                BlackPepper.into(),
                balance!(0.00025),
                balance!(0.0001),
                balance!(0.00025),
                balance!(0.0001),
            ),);
        })],
    );
}

#[test]
// Deposit to an existing pool
fn multiple_providers() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(CHARLIE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                balance!(25),
                balance!(10),
                balance!(25),
                balance!(10),
            ),);
        })],
    );
}

#[test]
fn depositliq_large_values() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        assert_noop!(
            crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                balance!(999360000),
                balance!(999144000),
                balance!(360000),
                balance!(144000),
            ),
            crate::Error::<Runtime>::SourceBaseAmountIsNotLargeEnough
        );
    })]);
}

#[test]
fn depositliq_valid_range_but_desired_is_corrected() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                balance!(360000),
                balance!(999000),
                balance!(350000),
                balance!(143000),
            ));
        })],
    );
}

#[test]
fn cannot_deposit_zero_values() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            for base in [GoldenTicket, Potato] {
                assert_noop!(
                    crate::Pallet::<Runtime>::deposit_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        base.into(),
                        BlackPepper.into(),
                        balance!(0),
                        balance!(100),
                        balance!(100),
                        balance!(100),
                    ),
                    crate::Error::<Runtime>::InvalidDepositLiquidityBasicAssetAmount
                );
                assert_noop!(
                    crate::Pallet::<Runtime>::deposit_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        base.into(),
                        BlackPepper.into(),
                        balance!(100),
                        balance!(0),
                        balance!(100),
                        balance!(100),
                    ),
                    crate::Error::<Runtime>::InvalidDepositLiquidityTargetAssetAmount
                );
                assert_noop!(
                    crate::Pallet::<Runtime>::deposit_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        base.into(),
                        BlackPepper.into(),
                        balance!(100),
                        balance!(100),
                        balance!(0),
                        balance!(100),
                    ),
                    crate::Error::<Runtime>::InvalidDepositLiquidityBasicAssetAmount
                );
                assert_noop!(
                    crate::Pallet::<Runtime>::deposit_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        base.into(),
                        BlackPepper.into(),
                        balance!(1000),
                        balance!(100),
                        balance!(100),
                        balance!(0),
                    ),
                    crate::Error::<Runtime>::InvalidDepositLiquidityTargetAssetAmount
                );
            }
        })],
    );
}

#[test]
fn cannot_withdraw_zero_values() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                GoldenTicket.into(),
                BlackPepper.into(),
                balance!(360000),
                balance!(999000),
                balance!(350000),
                balance!(143000),
            ));
            assert_noop!(
                crate::Pallet::<Runtime>::withdraw_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    balance!(8784),
                    balance!(0),
                    balance!(4300)
                ),
                crate::Error::<Runtime>::InvalidWithdrawLiquidityBasicAssetAmount
            );
            assert_noop!(
                crate::Pallet::<Runtime>::withdraw_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    balance!(8784),
                    balance!(4300),
                    balance!(0)
                ),
                crate::Error::<Runtime>::InvalidWithdrawLiquidityTargetAssetAmount
            );
        })],
    );
}

#[test]
fn cannot_initialize_with_non_divisible_asset() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            GoldenTicket.into(),
            AssetSymbol(b"GT".to_vec()),
            AssetName(b"Golden Ticket".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            Flower.into(),
            AssetSymbol(b"FLOWER".to_vec()),
            AssetName(b"FLOWER".to_vec()),
            0,
            1,
            true,
            None,
            None,
        ));
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_A_ID,
            GoldenTicket.into(),
            Flower.into()
        ));
        assert_noop!(
            crate::Pallet::<Runtime>::initialize_pool(
                RuntimeOrigin::signed(BOB()),
                DEX_A_ID,
                GoldenTicket.into(),
                Flower.into(),
            ),
            crate::Error::<Runtime>::UnableToCreatePoolWithIndivisibleAssets
        );
    });
}

#[test]
fn pool_is_already_initialized_and_other_after_depositliq() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, pt, _, _, _, repr: AccountId, fee_repr: AccountId| {
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &repr.clone()).unwrap(),
                    balance!(144000)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &repr.clone()).unwrap()
                        + assets::Pallet::<Runtime>::free_balance(&pt, &repr.clone()).unwrap(),
                    balance!(360000)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &fee_repr.clone()).unwrap(),
                    0
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                    0
                );

                assert_noop!(
                    crate::Pallet::<Runtime>::initialize_pool(
                        RuntimeOrigin::signed(BOB()),
                        dex_id.clone(),
                        GoldenTicket.into(),
                        BlackPepper.into(),
                    ),
                    crate::Error::<Runtime>::PoolIsAlreadyInitialized
                );
            },
        )],
    );
}

#[test]
fn exchange_desired_output_and_withdraw_cascade() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, pt, _, _, _, repr: AccountId, fee_repr: AccountId| {
                let gt_balance = assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap();
                let bp_balance = assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap();
                assert_ok!(crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: balance!(33000),
                        max_amount_in: balance!(99999999),
                    }
                ));
                assert_eq!(
                    gt_balance - assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                    balance!(107349.074249776356095316)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap() - bp_balance,
                    balance!(33000)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &repr.clone()).unwrap()
                        + assets::Pallet::<Runtime>::free_balance(&pt, &repr.clone()).unwrap(),
                    balance!(467027.027027027027027031)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &repr.clone()).unwrap(),
                    balance!(111000)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                    balance!(322.047222749329068285)
                );

                // a = sqrt ( 467027 * 111000 ) / 8784 = 25.92001146000573
                // b = 467_027 / a = 18018.00900900901
                // c = 111_000 / a = 4282.405514028097
                // Testing this line with noop
                // fail for each asset min, after this success.

                // First minimum is above boundaries.
                assert_noop!(
                    crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(8784),
                        balance!(18100),
                        balance!(4100)
                    ),
                    crate::Error::<Runtime>::CalculatedValueIsNotMeetsRequiredBoundaries
                );

                // Second minimum is above boundaries.
                assert_noop!(
                    crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(8784),
                        balance!(18000),
                        balance!(4300)
                    ),
                    crate::Error::<Runtime>::CalculatedValueIsNotMeetsRequiredBoundaries
                );

                let gt_balance = assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap();
                let bp_balance = assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap();

                // Both minimums is below.
                assert_ok!(crate::Pallet::<Runtime>::withdraw_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    balance!(8784),
                    balance!(18000),
                    balance!(4200),
                ));

                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap() - gt_balance,
                    balance!(18017.803438001542088005),
                    balance!(0.00000001),
                );
                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap() - bp_balance,
                    balance!(4282.356407400019291402),
                    balance!(0.00000001),
                );
                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &repr.clone()).unwrap()
                        + assets::Pallet::<Runtime>::free_balance(&pt, &repr.clone()).unwrap(),
                    balance!(449009.223589025484939026),
                    balance!(0.00000001),
                );
                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &repr.clone()).unwrap(),
                    balance!(106717.643592599980708598),
                    balance!(0.00000001),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                    322047222749329068285
                );

                let gt_balance = assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap();
                let bp_balance = assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap();

                assert_ok!(crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: balance!(33000),
                        max_amount_in: balance!(99999999),
                    }
                ));

                common::assert_approx_eq_abs!(
                    gt_balance - assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                    balance!(201605.603818778020949073),
                    balance!(0.00000001),
                );
                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap() - bp_balance,
                    balance!(33000),
                    balance!(0.00000001),
                );
                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &repr.clone()).unwrap()
                        + assets::Pallet::<Runtime>::free_balance(&pt, &repr.clone()).unwrap(),
                    balance!(650010.010596347171825252),
                    balance!(0.00000001),
                );
                common::assert_approx_eq_abs!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &repr.clone()).unwrap(),
                    balance!(73717.643592599980708598),
                    balance!(0.00000001),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                    926864034205663131132
                );
            },
        )],
    );
}

#[test]
fn exchange_desired_input() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, pt, _, _, _, repr: AccountId, fee_repr: AccountId| {
                let gt_balance = assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap();
                let bp_balance = assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap();
                assert_ok!(crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredInput {
                        desired_amount_in: balance!(33000),
                        min_amount_out: 0,
                    }
                ));
                assert_eq!(
                    gt_balance - assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                    balance!(33000)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap() - bp_balance,
                    balance!(12058.365847885345163285)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &repr.clone()).unwrap()
                        + assets::Pallet::<Runtime>::free_balance(&pt, &repr.clone()).unwrap(),
                    balance!(392901)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &repr.clone()).unwrap(),
                    balance!(131941.634152114654836715)
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &fee_repr.clone()).unwrap(),
                    balance!(99)
                );
            },
        )],
    );
}

#[test]
fn exchange_invalid_dex_id() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|_, _, _, _, _, _, _, _, _| {
            assert_noop!(
                crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &380,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: balance!(33000),
                        max_amount_in: balance!(99999999),
                    }
                ),
                dex_manager::Error::<Runtime>::DEXDoesNotExist
            );
        })],
    );
}

#[test]
fn exchange_different_asset_pair() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_noop!(
                crate::Pallet::<Runtime>::exchange(
                    &ALICE(),
                    &ALICE(),
                    &dex_id,
                    &GoldenTicket.into(),
                    &RedPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: balance!(33000),
                        max_amount_in: balance!(99999999),
                    }
                ),
                technical::Error::<Runtime>::TechAccountIdIsNotRegistered
            );
        })],
    );
}

#[test]
fn exchange_swap_fail_with_invalid_balance() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
            assert_noop!(
                crate::Pallet::<Runtime>::exchange(
                    &BOB(),
                    &BOB(),
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: balance!(33000),
                        max_amount_in: balance!(999999999),
                    }
                ),
                crate::Error::<Runtime>::AccountBalanceIsInvalid
            );
        })],
    );
}

#[test]
fn exchange_outcome_should_match_actual_desired_amount_in_with_input_base() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, _, _, _, _, _repr: AccountId, _fee_repr: AccountId| {
                use sp_core::crypto::AccountId32;
                let new_account = AccountId32::from([33; 32]);
                assets::Pallet::<Runtime>::transfer(
                    RuntimeOrigin::signed(ALICE()),
                    gt.clone(),
                    new_account.clone(),
                    balance!(100000),
                )
                .expect("Failed to transfer balance");

                let (quote_outcome, _) = crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::WithDesiredInput {
                        desired_amount_in: balance!(100000),
                    },
                    true,
                )
                .expect("Failed to quote.");
                let (outcome, _) = crate::Pallet::<Runtime>::exchange(
                    &new_account,
                    &new_account,
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredInput {
                        desired_amount_in: balance!(100000),
                        min_amount_out: 0,
                    },
                )
                .expect("Failed to perform swap.");
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &new_account.clone()).unwrap(),
                    0,
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &new_account.clone()).unwrap(),
                    balance!(31230.802697411355231672),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &new_account.clone()).unwrap(),
                    outcome.amount,
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &new_account.clone()).unwrap(),
                    quote_outcome.amount,
                );
            },
        )],
    );
}

#[test]
fn exchange_outcome_should_match_actual_desired_amount_in_with_output_base() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, _, _, _, _, _repr: AccountId, _fee_repr: AccountId| {
                use sp_core::crypto::AccountId32;
                let new_account = AccountId32::from([3; 32]);
                assets::Pallet::<Runtime>::transfer(
                    RuntimeOrigin::signed(ALICE()),
                    bp.clone(),
                    new_account.clone(),
                    balance!(100000),
                )
                .expect("Failed to transfer balance");

                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap(),
                    balance!(1756000),
                );
                let (quote_outcome, _) = crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::WithDesiredInput {
                        desired_amount_in: balance!(100000),
                    },
                    true,
                )
                .expect("Failed to quote.");
                let (outcome, _) = crate::Pallet::<Runtime>::exchange(
                    &new_account,
                    &new_account,
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    SwapAmount::WithDesiredInput {
                        desired_amount_in: balance!(100000),
                        min_amount_out: 0,
                    },
                )
                .expect("Failed to perform swap.");
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &new_account.clone()).unwrap(),
                    0,
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &new_account.clone()).unwrap(),
                    balance!(147098.360655737704918032),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &new_account.clone()).unwrap(),
                    outcome.amount,
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &new_account.clone()).unwrap(),
                    quote_outcome.amount,
                );
            },
        )],
    );
}

#[test]
fn exchange_outcome_should_match_actual_desired_amount_out_with_input_base() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, _, _, _, _, _repr: AccountId, _fee_repr: AccountId| {
                use sp_core::crypto::AccountId32;
                let new_account = AccountId32::from([3; 32]);
                assets::Pallet::<Runtime>::transfer(
                    RuntimeOrigin::signed(ALICE()),
                    gt.clone(),
                    new_account.clone(),
                    balance!(100000),
                )
                .expect("Failed to transfer balance");

                let desired_out = balance!(31230.802697411355231672);
                let (quote_outcome, _) = crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::WithDesiredOutput {
                        desired_amount_out: desired_out,
                    },
                    true,
                )
                .expect("Failed to quote.");
                let (outcome, _) = crate::Pallet::<Runtime>::exchange(
                    &new_account,
                    &new_account,
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: desired_out,
                        max_amount_in: Balance::MAX,
                    },
                )
                .expect("Failed to perform swap.");
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &new_account.clone()).unwrap(),
                    0,
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &new_account.clone()).unwrap(),
                    desired_out,
                );
                assert_eq!(balance!(100000), quote_outcome.amount,);
                assert_eq!(balance!(100000), outcome.amount);
            },
        )],
    );
}

#[test]
fn exchange_outcome_should_match_actual_desired_amount_out_with_output_base() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, gt, bp, _, _, _, _, _repr: AccountId, _fee_repr: AccountId| {
                use sp_core::crypto::AccountId32;
                let new_account = AccountId32::from([3; 32]);
                assets::Pallet::<Runtime>::transfer(
                    RuntimeOrigin::signed(ALICE()),
                    bp.clone(),
                    new_account.clone(),
                    balance!(100000),
                )
                .expect("Failed to transfer balance");

                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap(),
                    balance!(1756000),
                );
                let desired_out = balance!(147098.360655737704918032);
                let (quote_outcome, _) = crate::Pallet::<Runtime>::quote(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::WithDesiredOutput {
                        desired_amount_out: desired_out,
                    },
                    true,
                )
                .expect("Failed to quote.");
                let (outcome, _) = crate::Pallet::<Runtime>::exchange(
                    &new_account,
                    &new_account,
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    SwapAmount::WithDesiredOutput {
                        desired_amount_out: desired_out,
                        max_amount_in: Balance::MAX,
                    },
                )
                .expect("Failed to perform swap.");
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &new_account.clone()).unwrap(),
                    1, // TODO: still not enough overestimation due to duducing fee from output, find workaroud to improve precision
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &new_account.clone()).unwrap(),
                    desired_out
                );
                assert_eq!(balance!(100000) - 1, quote_outcome.amount);
                assert_eq!(balance!(100000) - 1, outcome.amount);
            },
        )],
    );
}

#[test]
fn withdraw_all_liquidity() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        false,
        vec![Rc::new(
            |dex_id,
             gt,
             bp,
             _,
             _,
             _tech_acc_id: crate::mock::TechAccountId,
             _,
             repr: AccountId,
             _fee_repr: AccountId| {
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                    balance!(540000.0),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap(),
                    balance!(1856000.0),
                );

                assert_eq!(
                    PoolProviders::<Runtime>::get(&repr, &ALICE()).unwrap(),
                    balance!(227683.9915321233119024),
                );

                assert_noop!(
                    crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(227683.9915321233119025),
                        1,
                        1
                    ),
                    crate::Error::<Runtime>::SourceBalanceOfLiquidityTokensIsNotLargeEnough
                );

                assert_ok!(crate::Pallet::<Runtime>::withdraw_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    balance!(227683.9915321233119024),
                    balance!(1),
                    balance!(1),
                ));

                assert_eq!(PoolProviders::<Runtime>::get(repr, &ALICE()), None);

                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                    balance!(900000.000000000000000000),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap(),
                    balance!(2000000.000000000000000000),
                );
                // small fractions are lost due to min_liquidity locked for initial provider
                // and also rounding proportions such that user does not withdraw more thus breaking the pool
                // 900000.0 - 540000.0 = 360000.0
                // 2000000.0 - 1856000.0 = 144000.0
            },
        )],
    );
}

#[test]
fn withdraw_all_liquidity_chameleon() {
    let base_pool_tokens = balance!(113841.995766061655951781);
    let sum_pool_tokens = balance!(227683.991532123311902562);
    let chameleon_pool_tokens = sum_pool_tokens - base_pool_tokens;
    crate::Pallet::<Runtime>::preset_custom_deposited_pool(
        true,
        balance!(180000),
        balance!(72000),
        balance!(180000),
        balance!(72000),
        sum_pool_tokens,
        vec![Rc::new(
            |dex_id,
             gt,
             bp,
             _,
             _,
             _tech_acc_id: crate::mock::TechAccountId,
             _,
             repr: AccountId,
             _fee_repr: AccountId| {
                assert_noop!(
                    crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(227683.991532123311902563),
                        1,
                        1
                    ),
                    crate::Error::<Runtime>::SourceBalanceOfLiquidityTokensIsNotLargeEnough
                );

                assert_noop!(
                    crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(227683.991532123311902562),
                        1,
                        1
                    ),
                    pallet_balances::Error::<Runtime>::InsufficientBalance
                );

                assert_noop!(
                    crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        base_pool_tokens + 1,
                        1,
                        1
                    ),
                    pallet_balances::Error::<Runtime>::InsufficientBalance
                );

                assert_ok!(crate::Pallet::<Runtime>::withdraw_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    GoldenTicket.into(),
                    BlackPepper.into(),
                    base_pool_tokens,
                    balance!(1),
                    balance!(1),
                ));

                assert_eq!(
                    PoolProviders::<Runtime>::get(repr, &ALICE()),
                    Some(chameleon_pool_tokens)
                );

                assert_ok!(crate::Pallet::<Runtime>::withdraw_liquidity(
                    RuntimeOrigin::signed(ALICE()),
                    dex_id,
                    Potato.into(),
                    BlackPepper.into(),
                    chameleon_pool_tokens,
                    balance!(1),
                    balance!(1),
                ));

                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&gt, &ALICE()).unwrap(),
                    balance!(900000.000000000000000000),
                );
                assert_eq!(
                    assets::Pallet::<Runtime>::free_balance(&bp, &ALICE()).unwrap(),
                    balance!(2000000.000000000000000000),
                );
                // small fractions are lost due to min_liquidity locked for initial provider
                // and also rounding proportions such that user does not withdraw more thus breaking the pool
                // 900000.0 - 540000.0 = 360000.0
                // 2000000.0 - 1856000.0 = 144000.0
            },
        )],
    );
}

#[test]
fn deposit_liquidity_with_different_slippage_behavior() {
    crate::Pallet::<Runtime>::run_tests_with_different_slippage_behavior(
        RunTestsWithSlippageBehaviors {
            initial_deposit: (balance!(360000), balance!(144000)),
            desired_amount: balance!(2999),
            tests: vec![Rc::new(
                |dex_id,
                 _gt,
                 _bp,
                 _,
                 _,
                 _tech_acc_id: crate::mock::TechAccountId,
                 _,
                 _repr: AccountId,
                 _fee_repr: AccountId| {
                    assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(360000),
                        balance!(144000),
                        balance!(345000),
                        balance!(137000),
                    ));
                },
            )],
        },
    );
}

#[test]
fn withdraw_liquidity_with_different_slippage_behavior() {
    crate::Pallet::<Runtime>::run_tests_with_different_slippage_behavior(
        RunTestsWithSlippageBehaviors {
            initial_deposit: (balance!(360000), balance!(144000)),
            desired_amount: balance!(2999),
            tests: vec![Rc::new(
                |dex_id,
                 _gt,
                 _bp,
                 _,
                 _,
                 _tech_acc_id: crate::mock::TechAccountId,
                 _,
                 _repr: AccountId,
                 _fee_repr: AccountId| {
                    assert_ok!(crate::Pallet::<Runtime>::withdraw_liquidity(
                        RuntimeOrigin::signed(ALICE()),
                        dex_id,
                        GoldenTicket.into(),
                        BlackPepper.into(),
                        balance!(227683),
                        balance!(352000),
                        balance!(141000),
                    ));
                },
            )],
        },
    );
}

#[test]
fn variants_of_deposit_liquidity_twice() {
    let variants: Vec<Balance> = vec![1u128, 10u128, 100u128, 1000u128, 10000u128];

    for scale in variants {
        crate::Pallet::<Runtime>::run_tests_with_different_slippage_behavior(
            RunTestsWithSlippageBehaviors {
                initial_deposit: (balance!(10.13097) * scale, balance!(8.09525) * scale),
                desired_amount: balance!(0.0005) * scale,
                tests: vec![Rc::new(
                    |dex_id,
                     _gt,
                     _bp,
                     _,
                     _,
                     _tech_acc_id: crate::mock::TechAccountId,
                     _,
                     _repr: AccountId,
                     _fee_repr: AccountId| {
                        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
                            RuntimeOrigin::signed(ALICE()),
                            dex_id,
                            GoldenTicket.into(),
                            BlackPepper.into(),
                            balance!(20) * scale,
                            balance!(15.98291400432839) * scale,
                            balance!(19.9) * scale,
                            balance!(15.90299943430675) * scale,
                        ));
                    },
                )],
            },
        );
    }
}

fn distance(a: Balance, b: Balance) -> Balance {
    if a < b {
        b - a
    } else {
        a - b
    }
}

#[test]
/// WithDesiredOutput, Reserves with fractional numbers, Input is base asset
fn swapping_should_not_affect_k_1() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset_id: AssetId = GoldenTicket.into();
        let target_asset_id: AssetId = BlackPepper.into();
        let initial_reserve_base = balance!(9.000000000000000001);
        let initial_reserve_target = balance!(5.999999999999999999);
        let desired_out = balance!(4);
        let expected_in = balance!(18.054162487462387185);
        let expected_fee =
            OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.054162487462387161));

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            initial_reserve_base,
            initial_reserve_target,
            initial_reserve_base,
            initial_reserve_target,
        ));
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(reserve_base, initial_reserve_base);
        assert_eq!(reserve_target, initial_reserve_target);
        let k_before_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();

        assert_eq!(
            crate::Pallet::<Runtime>::exchange(
                &ALICE(),
                &ALICE(),
                &dex_id,
                &GoldenTicket.into(),
                &BlackPepper.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: desired_out,
                    max_amount_in: expected_in,
                }
            )
            .unwrap()
            .0,
            SwapOutcome {
                amount: expected_in,
                fee: expected_fee.clone(),
            }
        );
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(
            reserve_base,
            initial_reserve_base + (expected_in - expected_fee.get_by_asset(&GoldenTicket.into()))
        );
        assert_eq!(reserve_target, initial_reserve_target - desired_out);
        let k_after_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();
        assert!(distance(k_after_swap, k_before_swap) < balance!(0.000000000000000030));
    })]);
}

#[test]
/// WithDesiredOutput, Reserves with fractional numbers, Output is base asset
fn swapping_should_not_affect_k_2() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset_id: AssetId = GoldenTicket.into();
        let target_asset_id: AssetId = BlackPepper.into();
        let initial_reserve_base = balance!(9.000000000000000001);
        let initial_reserve_target = balance!(5.999999999999999999);
        let desired_out = balance!(4);
        let expected_in = balance!(4.826060727930826461);
        let expected_fee =
            OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.012036108324974924));

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            initial_reserve_base,
            initial_reserve_target,
            initial_reserve_base,
            initial_reserve_target,
        ));
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(reserve_base, initial_reserve_base);
        assert_eq!(reserve_target, initial_reserve_target);
        let k_before_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();

        assert_eq!(
            crate::Pallet::<Runtime>::exchange(
                &ALICE(),
                &ALICE(),
                &dex_id,
                &BlackPepper.into(),
                &GoldenTicket.into(),
                SwapAmount::WithDesiredOutput {
                    desired_amount_out: desired_out,
                    max_amount_in: expected_in,
                }
            )
            .unwrap()
            .0,
            SwapOutcome {
                amount: expected_in,
                fee: expected_fee.clone(),
            }
        );
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(
            reserve_base,
            initial_reserve_base - (desired_out + expected_fee.get_by_asset(&GoldenTicket.into()))
        );
        assert_eq!(reserve_target, initial_reserve_target + expected_in);

        let k_after_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();
        assert!(distance(k_after_swap, k_before_swap) < balance!(0.000000000000000015));
    })]);
}

#[test]
/// WithDesiredInput, Reserves with fractional numbers, Input is base asset
fn swapping_should_not_affect_k_3() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset_id: AssetId = GoldenTicket.into();
        let target_asset_id: AssetId = BlackPepper.into();
        let initial_reserve_base = balance!(9.000000000000000001);
        let initial_reserve_target = balance!(5.999999999999999999);
        let desired_in = balance!(4);
        let expected_out = balance!(1.842315983985217123);
        let expected_fee =
            OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.012000000000000000));

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            initial_reserve_base,
            initial_reserve_target,
            initial_reserve_base,
            initial_reserve_target,
        ));
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(reserve_base, initial_reserve_base);
        assert_eq!(reserve_target, initial_reserve_target);
        let k_before_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();

        assert_eq!(
            crate::Pallet::<Runtime>::exchange(
                &ALICE(),
                &ALICE(),
                &dex_id,
                &GoldenTicket.into(),
                &BlackPepper.into(),
                SwapAmount::WithDesiredInput {
                    desired_amount_in: desired_in,
                    min_amount_out: expected_out,
                }
            )
            .unwrap()
            .0,
            SwapOutcome {
                amount: expected_out,
                fee: expected_fee.clone(),
            }
        );
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(
            reserve_base,
            initial_reserve_base + (desired_in - expected_fee.get_by_asset(&GoldenTicket.into()))
        );
        assert_eq!(reserve_target, initial_reserve_target - expected_out);

        let k_after_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();
        assert!(distance(k_after_swap, k_before_swap) < balance!(0.000000000000000015));
    })]);
}

#[test]
/// WithDesiredInput, Reserves with fractional numbers, Output is base asset
fn swapping_should_not_affect_k_4() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset_id: AssetId = GoldenTicket.into();
        let target_asset_id: AssetId = BlackPepper.into();
        let initial_reserve_base = balance!(9.000000000000000001);
        let initial_reserve_target = balance!(5.999999999999999999);
        let desired_in = balance!(4);
        let expected_out = balance!(3.589200000000000000);
        let expected_fee =
            OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010800000000000000));

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            initial_reserve_base,
            initial_reserve_target,
            initial_reserve_base,
            initial_reserve_target,
        ));
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(reserve_base, initial_reserve_base);
        assert_eq!(reserve_target, initial_reserve_target);
        let k_before_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();

        assert_eq!(
            crate::Pallet::<Runtime>::exchange(
                &ALICE(),
                &ALICE(),
                &dex_id,
                &BlackPepper.into(),
                &GoldenTicket.into(),
                SwapAmount::WithDesiredInput {
                    desired_amount_in: desired_in,
                    min_amount_out: expected_out,
                }
            )
            .unwrap()
            .0,
            SwapOutcome {
                amount: expected_out,
                fee: expected_fee.clone(),
            }
        );
        let (reserve_base, reserve_target) =
            crate::Reserves::<Runtime>::get(base_asset_id, target_asset_id);
        assert_eq!(
            reserve_base,
            initial_reserve_base - (expected_out + expected_fee.get_by_asset(&GoldenTicket.into()))
        );
        assert_eq!(reserve_target, initial_reserve_target + desired_in);

        let k_after_swap =
            (FixedWrapper::from(reserve_base) * FixedWrapper::from(reserve_target)).into_balance();
        assert!(distance(k_after_swap, k_before_swap) < balance!(0.000000000000000015));
    })]);
}

#[test]
fn burn() {
    ExtBuilder::default().build().execute_with(|| {
        PoolProviders::<Runtime>::insert(ALICE(), BOB(), 10);
        TotalIssuances::<Runtime>::insert(ALICE(), 10);
        assert_ok!(crate::Pallet::<Runtime>::burn(&ALICE(), &BOB(), 10));
        assert_eq!(PoolProviders::<Runtime>::get(ALICE(), BOB()), None);
        assert_eq!(TotalIssuances::<Runtime>::get(ALICE()), Some(0));
    });

    ExtBuilder::default().build().execute_with(|| {
        TotalIssuances::<Runtime>::insert(ALICE(), 10);
        assert_noop!(
            crate::Pallet::<Runtime>::burn(&ALICE(), &BOB(), 10),
            crate::Error::<Runtime>::AccountBalanceIsInvalid
        );
        assert_eq!(PoolProviders::<Runtime>::get(ALICE(), BOB()), None);
        assert_eq!(TotalIssuances::<Runtime>::get(ALICE()), Some(10));
    });

    ExtBuilder::default().build().execute_with(|| {
        PoolProviders::<Runtime>::insert(ALICE(), BOB(), 5);
        TotalIssuances::<Runtime>::insert(ALICE(), 10);
        assert_noop!(
            crate::Pallet::<Runtime>::burn(&ALICE(), &BOB(), 10),
            crate::Error::<Runtime>::AccountBalanceIsInvalid
        );
        assert_eq!(PoolProviders::<Runtime>::get(ALICE(), BOB()), Some(5));
        assert_eq!(TotalIssuances::<Runtime>::get(ALICE()), Some(10));
    });
}

#[test]
fn mint() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(crate::Pallet::<Runtime>::mint(&ALICE(), &BOB(), 10));
        assert_eq!(PoolProviders::<Runtime>::get(ALICE(), BOB()), Some(10));
        assert_eq!(TotalIssuances::<Runtime>::get(ALICE()), Some(10));
    });
}

#[test]
fn test_get_pair_info() {
    ExtBuilder::default().build().execute_with(|| {
        let asset_base = GetBaseAssetId::get();
        let asset_target = GreenPromise.into();
        let asset_target_2 = BlackPepper.into();
        let asset_chameleon = Potato.into();

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_base, &asset_target).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_target);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, false);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_target, &asset_base).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_target);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, false);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_base, &asset_target_2).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_target_2);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, true);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_target_2, &asset_base).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_target_2);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, true);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_chameleon, &asset_target_2).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_target_2);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, true);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_target_2, &asset_chameleon).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_target_2);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, true);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_base, &asset_chameleon).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_chameleon);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, false);

        let (pair, asset_chameleon_opt, flag) =
            PoolXYK::get_pair_info(&asset_base, &asset_chameleon, &asset_base).unwrap();
        assert_eq!(pair.base_asset_id, asset_base);
        assert_eq!(pair.target_asset_id, asset_chameleon);
        assert_eq!(asset_chameleon_opt, Some(asset_chameleon));
        assert_eq!(flag, false);

        assert_noop!(
            PoolXYK::get_pair_info(&asset_base, &asset_base, &asset_base),
            crate::Error::<Runtime>::AssetsMustNotBeSame
        );
        assert_noop!(
            PoolXYK::get_pair_info(&asset_base, &asset_chameleon, &asset_chameleon),
            crate::Error::<Runtime>::AssetsMustNotBeSame
        );
        assert_noop!(
            PoolXYK::get_pair_info(&asset_base, &asset_target, &asset_target_2),
            crate::Error::<Runtime>::BaseAssetIsNotMatchedWithAnyAssetArguments
        );
        assert_noop!(
            PoolXYK::get_pair_info(&asset_base, &asset_chameleon, &asset_target),
            crate::Error::<Runtime>::RestrictedChameleonPool
        );
        assert_noop!(
            PoolXYK::get_pair_info(&asset_base, &asset_target, &asset_chameleon),
            crate::Error::<Runtime>::RestrictedChameleonPool
        );
    });
}

#[test]
fn depositing_and_withdrawing_liquidity_updates_user_pools() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset: AssetId = GoldenTicket.into();
        let target_asset_a: AssetId = BlackPepper.into();
        let target_asset_b: AssetId = BluePromise.into();
        let initial_reserve_base = balance!(10);
        let initial_reserve_target_a = balance!(20);
        let initial_reserve_target_b = balance!(20);

        assert_eq!(
            PoolXYK::account_pools(&ALICE(), &base_asset),
            Default::default()
        );

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            base_asset,
            target_asset_a,
            initial_reserve_base,
            initial_reserve_target_a,
            initial_reserve_base,
            initial_reserve_target_a,
        ));

        assert_eq!(
            PoolXYK::account_pools(&ALICE(), &base_asset),
            [target_asset_a].iter().cloned().collect()
        );

        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            base_asset,
            target_asset_a,
            initial_reserve_base,
            initial_reserve_target_a,
            initial_reserve_base,
            initial_reserve_target_a,
        ));

        assert_eq!(
            PoolXYK::account_pools(&ALICE(), &base_asset),
            [target_asset_a].iter().cloned().collect()
        );

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            target_asset_b,
            AssetSymbol(b"BP".to_vec()),
            AssetName(b"Black Pepper".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(0u32),
            true,
            None,
            None,
        ));
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(ALICE()),
            dex_id.clone(),
            base_asset,
            target_asset_b
        ));
        assert_ok!(crate::Pallet::<Runtime>::initialize_pool(
            RuntimeOrigin::signed(ALICE()),
            dex_id.clone(),
            base_asset,
            target_asset_b
        ));
        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &target_asset_b,
            &ALICE(),
            &ALICE(),
            balance!(1000)
        ));
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            base_asset,
            target_asset_b,
            initial_reserve_base,
            initial_reserve_target_b,
            initial_reserve_base,
            initial_reserve_target_b,
        ));

        assert_eq!(
            PoolXYK::account_pools(&ALICE(), &base_asset),
            [target_asset_a, target_asset_b].iter().cloned().collect()
        );

        let (_, tech_account_a) =
            PoolXYK::tech_account_from_dex_and_asset_pair(dex_id, base_asset, target_asset_a)
                .unwrap();
        let pool_account_a = Technical::tech_account_id_to_account_id(&tech_account_a).unwrap();
        let user_balance_a = PoolXYK::pool_providers(&pool_account_a, &ALICE()).unwrap();

        assert_ok!(crate::Pallet::<Runtime>::withdraw_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            base_asset,
            target_asset_a,
            user_balance_a,
            balance!(1),
            balance!(1)
        ));

        assert_eq!(
            PoolXYK::account_pools(&ALICE(), &base_asset),
            [target_asset_b].iter().cloned().collect()
        );
    })]);
}

#[test]
fn deposit_liquidity_with_non_divisible_assets() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset: AssetId = GoldenTicket.into();
        let target_asset_a: AssetId = GreenPromise.into();
        let target_asset_b: AssetId = BluePromise.into();

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            target_asset_a,
            AssetSymbol(b"GP".to_vec()),
            AssetName(b"Green Promise".to_vec()),
            0,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            target_asset_b,
            AssetSymbol(b"BP".to_vec()),
            AssetName(b"Blue Promise".to_vec()),
            0,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_noop!(
            crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                base_asset,
                target_asset_a,
                balance!(1),
                balance!(100),
                balance!(1),
                balance!(100),
            ),
            crate::Error::<Runtime>::UnableToOperateWithIndivisibleAssets
        );

        assert_noop!(
            crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                target_asset_b,
                base_asset,
                balance!(1),
                balance!(100),
                balance!(1),
                balance!(100),
            ),
            crate::Error::<Runtime>::UnableToOperateWithIndivisibleAssets
        );

        assert_noop!(
            crate::Pallet::<Runtime>::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                target_asset_a,
                target_asset_b,
                balance!(1),
                balance!(100),
                balance!(1),
                balance!(100),
            ),
            crate::Error::<Runtime>::UnableToOperateWithIndivisibleAssets
        );
    })]);
}

#[test]
fn withdraw_liquidity_with_non_divisible_assets() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, _, _, _, _, _, _, _, _| {
        let base_asset: AssetId = GoldenTicket.into();
        let target_asset_a: AssetId = GreenPromise.into();
        let target_asset_b: AssetId = BluePromise.into();

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            target_asset_a,
            AssetSymbol(b"GP".to_vec()),
            AssetName(b"Green Promise".to_vec()),
            0,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            target_asset_b,
            AssetSymbol(b"BP".to_vec()),
            AssetName(b"Blue Promise".to_vec()),
            0,
            Balance::from(0u32),
            true,
            None,
            None,
        ));

        assert_noop!(
            crate::Pallet::<Runtime>::withdraw_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                base_asset,
                target_asset_a,
                balance!(8784),
                balance!(18100),
                balance!(4100)
            ),
            crate::Error::<Runtime>::UnableToOperateWithIndivisibleAssets
        );

        assert_noop!(
            crate::Pallet::<Runtime>::withdraw_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                target_asset_b,
                base_asset,
                balance!(8784),
                balance!(18100),
                balance!(4100)
            ),
            crate::Error::<Runtime>::UnableToOperateWithIndivisibleAssets
        );

        assert_noop!(
            crate::Pallet::<Runtime>::withdraw_liquidity(
                RuntimeOrigin::signed(ALICE()),
                dex_id,
                target_asset_a,
                target_asset_b,
                balance!(8784),
                balance!(18100),
                balance!(4100)
            ),
            crate::Error::<Runtime>::UnableToOperateWithIndivisibleAssets
        );
    })]);
}

#[test]
fn price_without_impact_small_amount() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, _, _, _, _, _, _, _repr: AccountId, _fee_repr: AccountId| {
                let amount = balance!(1);
                // Buy base asset with desired input
                let (quote_outcome_a, _) = PoolXYK::quote(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_a = PoolXYK::quote_without_impact(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_a.amount, balance!(2.492482691092422969));
                assert_eq!(
                    quote_without_impact_a.amount,
                    balance!(2.492500000000000000)
                );
                assert!(quote_outcome_a.amount < quote_without_impact_a.amount);

                // Buy base asset with desired output
                let (quote_outcome_b, _) = PoolXYK::quote(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_b = PoolXYK::quote_without_impact(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_b.amount, balance!(0.401204728643510095));
                assert_eq!(
                    quote_without_impact_b.amount,
                    balance!(0.401203610832497492)
                );
                assert!(quote_outcome_b.amount > quote_without_impact_b.amount);

                // Sell base asset with desired input
                let (quote_outcome_c, _) = PoolXYK::quote(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_c = PoolXYK::quote_without_impact(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_c.amount, balance!(0.398798895548614272));
                assert_eq!(
                    quote_without_impact_c.amount,
                    balance!(0.398800000000000000)
                );
                assert!(quote_outcome_c.amount < quote_without_impact_c.amount);

                // Sell base asset with desired input
                let (quote_outcome_d, _) = PoolXYK::quote(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_d = PoolXYK::quote_without_impact(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_d.amount, balance!(2.507539981175200824));
                assert_eq!(
                    quote_without_impact_d.amount,
                    balance!(2.507522567703109327)
                );
                assert!(quote_outcome_d.amount > quote_without_impact_d.amount);
            },
        )],
    );
}

#[test]
fn price_without_impact_large_amount() {
    crate::Pallet::<Runtime>::preset_deposited_pool(
        true,
        vec![Rc::new(
            |dex_id, _, _, _, _, _, _, _repr: AccountId, _fee_repr: AccountId| {
                let amount = balance!(100000);
                // Buy base asset with desired input
                let (quote_outcome_a, _) = PoolXYK::quote(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_a = PoolXYK::quote_without_impact(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_a.amount, balance!(147098.360655737704918032));
                assert_eq!(
                    quote_without_impact_a.amount,
                    balance!(249250.000000000000000000)
                );
                assert!(quote_outcome_a.amount < quote_without_impact_a.amount);

                // Buy base asset with desired output
                let (quote_outcome_b, _) = PoolXYK::quote(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_b = PoolXYK::quote_without_impact(
                    &dex_id,
                    &BlackPepper.into(),
                    &GoldenTicket.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_b.amount, balance!(55615.634172717441680828));
                assert_eq!(
                    quote_without_impact_b.amount,
                    balance!(40120.361083249749247743)
                );
                assert!(quote_outcome_b.amount > quote_without_impact_b.amount);

                // Sell base asset with desired input
                let (quote_outcome_c, _) = PoolXYK::quote(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_c = PoolXYK::quote_without_impact(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_input(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_c.amount, balance!(31230.802697411355231672));
                assert_eq!(
                    quote_without_impact_c.amount,
                    balance!(39880.000000000000000000)
                );
                assert!(quote_outcome_c.amount < quote_without_impact_c.amount);

                // Sell base asset with desired input
                let (quote_outcome_d, _) = PoolXYK::quote(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote.");
                let quote_without_impact_d = PoolXYK::quote_without_impact(
                    &dex_id,
                    &GoldenTicket.into(),
                    &BlackPepper.into(),
                    QuoteAmount::with_desired_output(amount),
                    true,
                )
                .expect("Failed to quote without impact.");
                assert_eq!(quote_outcome_d.amount, balance!(820643.749430108507340228));
                assert_eq!(
                    quote_without_impact_d.amount,
                    balance!(250752.256770310932798395)
                );
                assert!(quote_outcome_d.amount > quote_without_impact_d.amount);
            },
        )],
    );
}

#[test]
fn initialize_pool_with_different_dex() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            AppleTree.into(),
            AssetSymbol(b"AT".to_vec()),
            AssetName(b"Apple Tree".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            GoldenTicket.into(),
            AssetSymbol(b"GT".to_vec()),
            AssetName(b"Golden Ticket".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_B_ID,
            AppleTree.into(),
            GoldenTicket.into()
        ));
        assert_ok!(PoolXYK::initialize_pool(
            RuntimeOrigin::signed(ALICE()),
            DEX_B_ID,
            AppleTree.into(),
            GoldenTicket.into()
        ));
        assert_ok!(PoolXYK::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            DEX_B_ID,
            AppleTree.into(),
            GoldenTicket.into(),
            balance!(1),
            balance!(1),
            balance!(1),
            balance!(1),
        ));
    });
}

#[test]
fn initialize_pool_with_synthetics() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            GoldenTicket.into(),
            AssetSymbol(b"GT".to_vec()),
            AssetName(b"Golden Ticket".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            Apple.into(),
            AssetSymbol(b"AP".to_vec()),
            AssetName(b"Apple".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            BlackPepper.into(),
            AssetSymbol(b"BP".to_vec()),
            AssetName(b"BlackPepper".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));

        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_A_ID,
            GoldenTicket.into(),
            Mango.into(),
        ));
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_C_ID,
            Mango.into(),
            GoldenTicket.into(),
        ));
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_C_ID,
            Mango.into(),
            BatteryForMusicPlayer.into(),
        ));
        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_C_ID,
            Mango.into(),
            BlackPepper.into(),
        ));

        let euro =
            common::SymbolName::from_str("EUR").expect("Failed to parse `EURO` as a symbol name");
        OracleProxy::enable_oracle(RuntimeOrigin::root(), Oracle::BandChainFeed)
            .expect("Failed to enable `Band` oracle");
        Band::add_relayers(RuntimeOrigin::root(), vec![ALICE()]).expect("Failed to add relayers");
        Band::relay(
            RuntimeOrigin::signed(ALICE()),
            vec![(euro.clone(), 1)].try_into().unwrap(),
            0,
            0,
        )
        .expect("Failed to relay");

        assert_ok!(xst::Pallet::<Runtime>::enable_synthetic_asset(
            RuntimeOrigin::root(),
            Apple.into(),
            euro.clone(),
            fixed!(0)
        ));

        // XOR-<Synthetic asset> pool must not be created
        assert_noop!(
            PoolXYK::initialize_pool(
                RuntimeOrigin::signed(ALICE()),
                DEX_A_ID,
                GoldenTicket.into(),
                Mango.into()
            ),
            crate::Error::<Runtime>::TargetAssetIsRestricted
        );
        // XSTUSD-XOR pool must not be created (this case also applicable to XST,
        // since it is added along with XOR to restricted assets)
        assert_noop!(
            PoolXYK::initialize_pool(
                RuntimeOrigin::signed(ALICE()),
                DEX_C_ID,
                Mango.into(),
                GoldenTicket.into()
            ),
            crate::Error::<Runtime>::TargetAssetIsRestricted
        );
        // XSTUSD-<Other synthetic asset> pool must not be created
        assert_noop!(
            PoolXYK::initialize_pool(
                RuntimeOrigin::signed(ALICE()),
                DEX_C_ID,
                Mango.into(),
                Apple.into()
            ),
            crate::Error::<Runtime>::TargetAssetIsRestricted
        );
        // XSTUSD-<Allowed asset> pool must be created
        assert_ok!(PoolXYK::initialize_pool(
            RuntimeOrigin::signed(ALICE()),
            DEX_C_ID,
            Mango.into(),
            BlackPepper.into(),
        ));
    });
}

#[test]
fn check_step_quote_chameleon_limits() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, gt, bp, pt, _, _, _, _, _| {
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            balance!(100000),
            balance!(200000),
            balance!(100000),
            balance!(200000),
        ));
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            Potato.into(),
            BlackPepper.into(),
            balance!(10),
            balance!(20),
            balance!(10),
            balance!(20),
        ));

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(99)),
                3,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(33),
                        balance!(16.447786386607548998),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.049491834663814090))
                    ),
                    SwapChunk::new(
                        balance!(33),
                        balance!(16.442360950037693547),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.049475509378247824))
                    ),
                    SwapChunk::new(
                        balance!(33),
                        balance!(16.436938197461963666),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.049459192168892569))
                    ),
                ]),
                limits: Default::default(),
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &pt,
                QuoteAmount::with_desired_input(balance!(99)),
                3,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(6.687397532149040098),
                        balance!(3.333556216843500771),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010030760933330494))
                    ),
                    SwapChunk::new(
                        balance!(6.687397532149040098),
                        balance!(3.333333325882023583),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010030090248391245))
                    ),
                    SwapChunk::new(
                        balance!(6.687397532149040099),
                        balance!(3.333110457274475645),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010029419630715573))
                    ),
                ]),
                limits: SwapLimits::new(
                    None,
                    Some(SideAmount::Input(balance!(20.062192596447120295))),
                    None
                )
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &pt,
                QuoteAmount::with_desired_output(balance!(99)),
                3,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(6.686950393907246094),
                        balance!(3.333333333333333333),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010030090270812437))
                    ),
                    SwapChunk::new(
                        balance!(6.687397517200078968),
                        balance!(3.333333333333333333),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010030090270812437))
                    ),
                    SwapChunk::new(
                        balance!(6.687844685339795233),
                        balance!(3.333333333333333334),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.010030090270812437))
                    ),
                ]),
                limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(10))), None)
            }
        );

        assert_eq!(
            crate::Pallet::<Runtime>::step_quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(99)),
                3,
                true
            )
            .unwrap()
            .0,
            DiscreteQuotation {
                chunks: VecDeque::from([
                    SwapChunk::new(
                        balance!(66.220512120302212237),
                        balance!(33),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.099297893681043129))
                    ),
                    SwapChunk::new(
                        balance!(66.264373819107869477),
                        balance!(33),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.099297893681043129))
                    ),
                    SwapChunk::new(
                        balance!(66.308279110583909913),
                        balance!(33),
                        OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.099297893681043130))
                    ),
                ]),
                limits: SwapLimits::new(
                    None,
                    Some(SideAmount::Output(balance!(98712.870300000000000000))),
                    None
                )
            }
        );
    })]);
}

#[test]
fn check_quote_chameleon_limits() {
    crate::Pallet::<Runtime>::preset_initial(vec![Rc::new(|dex_id, gt, bp, pt, _, _, _, _, _| {
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            GoldenTicket.into(),
            BlackPepper.into(),
            balance!(100000),
            balance!(200000),
            balance!(100000),
            balance!(200000),
        ));
        assert_ok!(crate::Pallet::<Runtime>::deposit_liquidity(
            RuntimeOrigin::signed(ALICE()),
            dex_id,
            Potato.into(),
            BlackPepper.into(),
            balance!(10),
            balance!(20),
            balance!(10),
            balance!(20),
        ));

        assert_eq!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_input(balance!(99)),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(
                balance!(49.327085534107206211),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.148426536210954483))
            ),
        );

        assert_err!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &pt,
                QuoteAmount::with_desired_input(balance!(99)),
                true
            ),
            crate::Error::<Runtime>::NotEnoughOutputReserves
        );

        assert_err!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &pt,
                QuoteAmount::with_desired_output(balance!(99)),
                true
            ),
            crate::Error::<Runtime>::NotEnoughOutputReserves
        );

        assert_eq!(
            crate::Pallet::<Runtime>::quote(
                &dex_id,
                &bp,
                &gt,
                QuoteAmount::with_desired_output(balance!(99)),
                true
            )
            .unwrap()
            .0,
            SwapOutcome::new(
                balance!(198.793165049993991627),
                OutcomeFee::from_asset(GoldenTicket.into(), balance!(0.297893681043129388))
            ),
        );
    })]);
}

#[test]
fn test_pool_works_with_regulated_asset() {
    ExtBuilder::default().build().execute_with(|| {
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            GoldenTicket.into(),
            AssetSymbol(b"GT".to_vec()),
            AssetName(b"Golden Ticket".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));
        assert_ok!(assets::Pallet::<Runtime>::register_asset_id(
            ALICE(),
            Apple.into(),
            AssetSymbol(b"AP".to_vec()),
            AssetName(b"Apple".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::from(balance!(10)),
            true,
            None,
            None,
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &GoldenTicket.into(),
            &ALICE(),
            &ALICE(),
            balance!(900000)
        ));

        assert_ok!(assets::Pallet::<Runtime>::mint_to(
            &Apple.into(),
            &ALICE(),
            &ALICE(),
            balance!(900000)
        ));

        assert_ok!(regulated_assets::Pallet::<Runtime>::regulate_asset(
            RuntimeOrigin::signed(ALICE()),
            Apple.into(),
        ));

        assert_ok!(trading_pair::Pallet::<Runtime>::register(
            RuntimeOrigin::signed(BOB()),
            DEX_A_ID,
            GoldenTicket.into(),
            Apple.into(),
        ));

        assert_ok!(PoolXYK::initialize_pool(
            RuntimeOrigin::signed(ALICE()),
            DEX_A_ID,
            GoldenTicket.into(),
            Apple.into(),
        ));

        assert_err!(
            PoolXYK::deposit_liquidity(
                RuntimeOrigin::signed(ALICE()),
                DEX_A_ID,
                GoldenTicket.into(),
                Apple.into(),
                balance!(144000),
                balance!(360000),
                balance!(144000),
                balance!(227683.9915321233119024),
            ),
            crate::Error::<Runtime>::AssetRegulationsCheckFailed
        );

        assert_err!(
            PoolXYK::withdraw_liquidity(
                RuntimeOrigin::signed(ALICE()),
                DEX_A_ID,
                Apple.into(),
                GoldenTicket.into(),
                balance!(144000),
                balance!(36000),
                balance!(14400),
            ),
            crate::Error::<Runtime>::AssetRegulationsCheckFailed
        );
    });
}
