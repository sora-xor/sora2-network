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

use crate::liquidity_aggregator::aggregation_result::{AggregationResult, SwapInfo};
use crate::liquidity_aggregator::liquidity_aggregator::LiquidityAggregator;
use crate::mock::Runtime;
use crate::Error;
use common::alt::{DiscreteQuotation, SideAmount, SwapChunk, SwapLimits};
use common::prelude::{OutcomeFee, SwapAmount, SwapVariant};
use common::{balance, LiquiditySourceType, XOR, XST};
use frame_support::assert_err;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::vec_deque::VecDeque;
use sp_std::vec;

fn get_liquidity_aggregator_with_desired_input_and_equal_chunks(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);
    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(120), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_output_and_equal_chunks(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(8), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(13), balance!(100.1), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_input_and_different_chunks(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);
    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(12), balance!(108), OutcomeFee::xor(balance!(1.08))),
                SwapChunk::new(balance!(14), balance!(112), OutcomeFee::xor(balance!(1.12))),
                SwapChunk::new(balance!(16), balance!(112), OutcomeFee::xor(balance!(1.12))),
                SwapChunk::new(balance!(18), balance!(108), OutcomeFee::xor(balance!(1.08))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(
                    balance!(11),
                    balance!(93.5),
                    OutcomeFee::xst(balance!(0.935)),
                ),
                SwapChunk::new(balance!(12), balance!(102), OutcomeFee::xst(balance!(1.02))),
                SwapChunk::new(
                    balance!(13),
                    balance!(110.5),
                    OutcomeFee::xst(balance!(1.105)),
                ),
                SwapChunk::new(balance!(14), balance!(119), OutcomeFee::xst(balance!(1.19))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12), balance!(144), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(14), balance!(112), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_output_and_different_chunks(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(5.5), balance!(50), Default::default()),
                SwapChunk::new(balance!(3), balance!(25), Default::default()),
                SwapChunk::new(balance!(26), balance!(200), Default::default()),
                SwapChunk::new(balance!(7), balance!(50), Default::default()),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
                SwapChunk::new(balance!(9), balance!(72), Default::default()),
                SwapChunk::new(balance!(8), balance!(64), Default::default()),
                SwapChunk::new(balance!(7), balance!(56), Default::default()),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(8), balance!(100), Default::default()),
                SwapChunk::new(balance!(9), balance!(90), Default::default()),
                SwapChunk::new(balance!(13), balance!(100.1), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_input_and_max_amount_limits(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(15))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12), balance!(144), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(14), balance!(112), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(22))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_output_and_max_amount_limits(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(12), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(150))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(8), balance!(100), Default::default()),
                SwapChunk::new(balance!(9), balance!(90), Default::default()),
                SwapChunk::new(balance!(10.5), balance!(99.75), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Output(balance!(1))),
                Some(SideAmount::Output(balance!(190))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_input_and_min_amount_limits(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
                SwapChunk::new(balance!(10), balance!(80), OutcomeFee::xor(balance!(0.8))),
                SwapChunk::new(balance!(10), balance!(70), OutcomeFee::xor(balance!(0.7))),
                SwapChunk::new(balance!(10), balance!(60), OutcomeFee::xor(balance!(0.6))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12), balance!(144), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(14), balance!(112), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(21))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_output_and_min_amount_limits(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(13), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(14), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(15), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(16), balance!(100), OutcomeFee::xor(balance!(1))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(16), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(16), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(16), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(16), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(16), balance!(100), OutcomeFee::xst(balance!(1))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(8), balance!(100), Default::default()),
                SwapChunk::new(balance!(9), balance!(90), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Output(balance!(200))),
                Some(SideAmount::Output(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_input_and_precision_limits_for_input(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(11), balance!(137.5), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
                SwapChunk::new(balance!(14), balance!(70), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.1))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_input_and_precision_limits_for_output(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredInput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(10), balance!(90), OutcomeFee::xor(balance!(0.9))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
                SwapChunk::new(balance!(10), balance!(85), OutcomeFee::xst(balance!(0.85))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Input(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(11), balance!(137.5), Default::default()),
                SwapChunk::new(balance!(14), balance!(70), Default::default()),
                SwapChunk::new(balance!(10), balance!(40), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Output(balance!(0.1))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_output_and_precision_limits_for_input(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(125), Default::default()),
                SwapChunk::new(balance!(9), balance!(90), Default::default()),
                SwapChunk::new(balance!(10), balance!(50), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.01))),
            ),
        },
    );

    aggregator
}

fn get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output(
) -> LiquidityAggregator<Runtime, LiquiditySourceType> {
    let mut aggregator = LiquidityAggregator::new(SwapVariant::WithDesiredOutput);

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), OutcomeFee::xor(balance!(1))),
                SwapChunk::new(balance!(11), balance!(100), OutcomeFee::xor(balance!(1))),
            ]),
            limits: Default::default(),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
                SwapChunk::new(balance!(12.5), balance!(100), OutcomeFee::xst(balance!(1))),
            ]),
            limits: SwapLimits::new(None, Some(SideAmount::Output(balance!(1000000))), None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(125), Default::default()),
                SwapChunk::new(balance!(14), balance!(70), Default::default()),
                SwapChunk::new(balance!(10), balance!(40), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Output(balance!(0.01))),
            ),
        },
    );

    aggregator
}

#[test]
fn check_empty_chunks() {
    let aggregator =
        LiquidityAggregator::<Runtime, LiquiditySourceType>::new(SwapVariant::WithDesiredInput);
    assert_err!(
        aggregator.aggregate_liquidity(balance!(1)),
        Error::<Runtime>::InsufficientLiquidity
    );
}

#[test]
fn check_not_enough_liquidity() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_err!(
        aggregator.aggregate_liquidity(balance!(10000)),
        Error::<Runtime>::InsufficientLiquidity
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_input_and_equal_chunks() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(10)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(10), balance!(120))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(10), balance!(120))
            )],
            balance!(10),
            balance!(120),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(20)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(20), balance!(220))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(20), balance!(220))
            )],
            balance!(20),
            balance!(220),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(30)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(220))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(10), balance!(100))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(220))
                )
            ],
            balance!(30),
            balance!(320),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(1))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(40)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(220))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(220))
                )
            ],
            balance!(40),
            balance!(410),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(1.9))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(50)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                (LiquiditySourceType::XSTPool, (balance!(10), balance!(85))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(220))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(10), balance!(85))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(220))
                )
            ],
            balance!(50),
            balance!(495),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([
                (XOR, balance!(1.9)),
                (XST, balance!(0.85))
            ]))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(60)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                (LiquiditySourceType::XSTPool, (balance!(20), balance!(170))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(20), balance!(220))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(170))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(20), balance!(220))
                )
            ],
            balance!(60),
            balance!(580),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(1.9)), (XST, balance!(1.7))]))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_output_and_equal_chunks() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(100)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_output(balance!(100), balance!(8))
            )],
            balance!(100),
            balance!(8),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(200)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(18), balance!(200))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_output(balance!(200), balance!(18))
            )],
            balance!(200),
            balance!(18),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(300)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(100), balance!(10))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18))
                )
            ],
            balance!(300),
            balance!(28),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(1))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(400)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(21), balance!(200))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(21))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18))
                )
            ],
            balance!(400),
            balance!(39),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(2))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(500)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(300), balance!(33))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18))
                )
            ],
            balance!(500),
            balance!(51),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(3))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(600)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                (
                    LiquiditySourceType::XSTPool,
                    (balance!(12.5), balance!(100))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(300), balance!(33))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(100), balance!(12.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18))
                )
            ],
            balance!(600),
            balance!(63.5),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(1))]))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_equal_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(700)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                (LiquiditySourceType::XSTPool, (balance!(25), balance!(200))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(18), balance!(200))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(300), balance!(33))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(25))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18))
                )
            ],
            balance!(700),
            balance!(76),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(2))]))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_input_and_different_chunks() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(10)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(10), balance!(120))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(10), balance!(120))
            )],
            balance!(10),
            balance!(120),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(20)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(20), balance!(224))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(20), balance!(224))
            )],
            balance!(20),
            balance!(224),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(30)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(8), balance!(80))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(30),
            balance!(324),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(0.8))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(40)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(18), balance!(172))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(18), balance!(172))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(40),
            balance!(416),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(1.719999999999999999))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(50)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(22), balance!(208))),
                (LiquiditySourceType::XSTPool, (balance!(6), balance!(51))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(22), balance!(208))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(6), balance!(51))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(50),
            balance!(503),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([
                (XOR, balance!(2.08)),
                (XST, balance!(0.51))
            ]))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(60)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(22), balance!(208))),
                (LiquiditySourceType::XSTPool, (balance!(16), balance!(136))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(22), balance!(208))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(16), balance!(136))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(60),
            balance!(588),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([
                (XOR, balance!(2.08)),
                (XST, balance!(1.359999999999999999))
            ]))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_output_and_different_chunks() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(100)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_output(balance!(100), balance!(8))
            )],
            balance!(100),
            balance!(8),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(150)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(13), balance!(150))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_output(balance!(150), balance!(13))
            )],
            balance!(150),
            balance!(13),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(250)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(6), balance!(60))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(60), balance!(6))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(250),
            balance!(23),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(340)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(15.5), balance!(150))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(150), balance!(15.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(340),
            balance!(32.5),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(405)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(18.5), balance!(175))
                ),
                (LiquiditySourceType::XSTPool, (balance!(5), balance!(40))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(175), balance!(18.5))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(40), balance!(5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(405),
            balance!(40.5),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_different_chunks();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(505)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(18.5), balance!(175))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    (balance!(17.5), balance!(140))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(175), balance!(18.5))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(140), balance!(17.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(505),
            balance!(53),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_input_and_max_amount_limits() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(10)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(10), balance!(120))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(10), balance!(120))
            )],
            balance!(10),
            balance!(120),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(20)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(20), balance!(224))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(20), balance!(224))
            )],
            balance!(20),
            balance!(224),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(30)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(8), balance!(80))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(30),
            balance!(324),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(0.8))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(50)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                (LiquiditySourceType::XSTPool, (balance!(8), balance!(68))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(8), balance!(68))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(50),
            balance!(502),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([
                (XOR, balance!(1.9)),
                (XST, balance!(0.68))
            ]))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(60)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(23), balance!(214))),
                (
                    LiquiditySourceType::XSTPool,
                    (balance!(15), balance!(127.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(23), balance!(214))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(15), balance!(127.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(60),
            balance!(585.5),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([
                (XOR, balance!(2.14)),
                (XST, balance!(1.275))
            ]))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_output_and_max_amount_limits() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(100)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::OrderBook, (balance!(8), balance!(100)))]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_output(balance!(100), balance!(8))
            )],
            balance!(100),
            balance!(8),
            SwapVariant::WithDesiredOutput,
            Default::default()
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(200)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(1), balance!(10))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(10), balance!(1))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(200),
            balance!(18),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(0.1))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(300)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(11.1), balance!(110))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(110), balance!(11.1))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(300),
            balance!(28.1),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(1.1))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(500)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                (LiquiditySourceType::XSTPool, (balance!(1.25), balance!(10))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(300), balance!(33))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(10), balance!(1.25))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(500),
            balance!(51.25),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(0.1))]))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(600)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(33), balance!(300))),
                (
                    LiquiditySourceType::XSTPool,
                    (balance!(13.75), balance!(110))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(300), balance!(33))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(110), balance!(13.75))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(600),
            balance!(63.75),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(3)), (XST, balance!(1.1))]))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_max_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(700)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(40.8), balance!(360))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    (balance!(18.75), balance!(150))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(17), balance!(190))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(360), balance!(40.8))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(150), balance!(18.75))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(190), balance!(17))
                )
            ],
            balance!(700),
            balance!(76.55),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(3.6)), (XST, balance!(1.5))]))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_input_and_min_amount_limits() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(10)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(10), balance!(100)))]),
            vec![(
                LiquiditySourceType::XYKPool,
                SwapAmount::with_desired_input(balance!(10), balance!(100))
            )],
            balance!(10),
            balance!(100),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(1))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(20)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(20), balance!(190)))]),
            vec![(
                LiquiditySourceType::XYKPool,
                SwapAmount::with_desired_input(balance!(20), balance!(190))
            )],
            balance!(20),
            balance!(190),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(1.9))
        )
    );

    // order-book appears only when it exceeds the min amount
    let aggregator = get_liquidity_aggregator_with_desired_input_and_min_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(30)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(8), balance!(80))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(22), balance!(244))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(8), balance!(80))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(22), balance!(244))
                )
            ],
            balance!(30),
            balance!(324),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(0.8))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_output_and_min_amount_limits() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(100)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(10), balance!(100)))]),
            vec![(
                LiquiditySourceType::XYKPool,
                SwapAmount::with_desired_output(balance!(100), balance!(10))
            )],
            balance!(100),
            balance!(10),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(1))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(200)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(23), balance!(200)))]),
            vec![(
                LiquiditySourceType::XYKPool,
                SwapAmount::with_desired_output(balance!(200), balance!(23))
            )],
            balance!(200),
            balance!(23),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(2))
        )
    );

    // order-book appears only when it exceeds the min amount
    let aggregator = get_liquidity_aggregator_with_desired_output_and_min_amount_limits();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(300)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(10), balance!(100))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(18.25), balance!(200))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(100), balance!(10))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(200), balance!(18.25))
                )
            ],
            balance!(300),
            balance!(28.25),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(1))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_input_and_precision_limits() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_precision_limits_for_input();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(10.65)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(0.05), balance!(0.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(10.6), balance!(132.5))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(0.05), balance!(0.5))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(10.6), balance!(132.5))
                )
            ],
            balance!(10.65),
            balance!(133),
            SwapVariant::WithDesiredInput,
            OutcomeFee::xor(balance!(0.005))
        )
    );
}

#[test]
fn check_aggregate_liquidity_with_desired_output_and_precision_limits() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(101.585)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(0.0005), balance!(0.005))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(8.1264), balance!(101.58))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(0.005), balance!(0.0005))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(101.58), balance!(8.1264))
                )
            ],
            balance!(101.585),
            balance!(8.1269),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(0.00005))
        )
    );

    let aggregator = get_liquidity_aggregator_with_desired_output_and_precision_limits_for_input();
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(101.585)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(0.0085), balance!(0.085))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(8.12), balance!(101.5))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(0.085), balance!(0.0085))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(101.5), balance!(8.12))
                )
            ],
            balance!(101.585),
            balance!(8.1285),
            SwapVariant::WithDesiredOutput,
            OutcomeFee::xor(balance!(0.00085))
        )
    );
}

#[test]
fn check_returning_back_several_chunks() {
    let mut aggregator =
        LiquidityAggregator::<Runtime, LiquiditySourceType>::new(SwapVariant::WithDesiredInput);
    aggregator.add_source(
        LiquiditySourceType::XSTPool,
        DiscreteQuotation {
            chunks: vec![SwapChunk::new(balance!(0.1), balance!(1), Default::default()); 100]
                .into(),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(2))),
                Some(SideAmount::Input(balance!(3))),
                None,
            ),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: vec![SwapChunk::new(balance!(1), balance!(8), Default::default()); 100].into(),
            limits: SwapLimits::new(None, None, None),
        },
    );

    assert_eq!(
        aggregator
            .clone()
            .aggregate_liquidity(balance!(1.5))
            .unwrap(),
        AggregationResult::new(
            SwapInfo::from([(LiquiditySourceType::XYKPool, (balance!(1.5), balance!(12))),]),
            vec![(
                LiquiditySourceType::XYKPool,
                SwapAmount::with_desired_input(balance!(1.5), balance!(12))
            ),],
            balance!(1.5),
            balance!(12),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );

    assert_eq!(
        aggregator.aggregate_liquidity(balance!(4)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(1), balance!(8))),
                (LiquiditySourceType::XSTPool, (balance!(3), balance!(30)))
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(1), balance!(8))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(3), balance!(30))
                )
            ],
            balance!(4),
            balance!(38),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );
}

#[test]
fn check_rounding_with_desired_input_amount_and_input_precision() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_precision_limits_for_input();

    assert_eq!(
        aggregator.aggregate_liquidity(balance!(52.05)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                (LiquiditySourceType::XSTPool, (balance!(20), balance!(170))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(12), balance!(145.5))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(170))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(12), balance!(145.5))
                )
            ],
            balance!(52), // rounded down
            balance!(505.5),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(1.9)), (XST, balance!(1.7))]))
        )
    );
}

#[test]
fn check_rounding_with_desired_output_amount_and_output_precision() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_precision_limits_for_output();

    assert_eq!(
        aggregator.aggregate_liquidity(balance!(525.123)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(21), balance!(200))),
                (LiquiditySourceType::XSTPool, (balance!(25), balance!(200))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(10.026), balance!(125.13))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(21))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(25))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(125.13), balance!(10.026))
                )
            ],
            balance!(525.13), // rounded up
            balance!(56.026),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(2)), (XST, balance!(2))]))
        )
    );
}

#[test]
fn check_rounding_with_desired_input_amount_and_output_precision() {
    let aggregator = get_liquidity_aggregator_with_desired_input_and_precision_limits_for_output();

    assert_eq!(
        aggregator.aggregate_liquidity(balance!(52.05)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(20), balance!(190))),
                (LiquiditySourceType::XSTPool, (balance!(20), balance!(170))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(12.04), balance!(142.7))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(190))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_input(balance!(20), balance!(170))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(12.04), balance!(142.7))
                )
            ],
            balance!(52.04), // rounded down
            balance!(502.7),
            SwapVariant::WithDesiredInput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(1.9)), (XST, balance!(1.7))]))
        )
    );
}

#[test]
fn check_rounding_with_desired_output_amount_and_input_precision() {
    let aggregator = get_liquidity_aggregator_with_desired_output_and_precision_limits_for_input();

    assert_eq!(
        aggregator.aggregate_liquidity(balance!(625.615)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (LiquiditySourceType::XYKPool, (balance!(21), balance!(200))),
                (LiquiditySourceType::XSTPool, (balance!(25), balance!(200))),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(21.13), balance!(225.65))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(21))
                ),
                (
                    LiquiditySourceType::XSTPool,
                    SwapAmount::with_desired_output(balance!(200), balance!(25))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_output(balance!(225.65), balance!(21.13))
                )
            ],
            balance!(625.65), // rounded up
            balance!(67.13),
            SwapVariant::WithDesiredOutput,
            OutcomeFee(BTreeMap::from([(XOR, balance!(2)), (XST, balance!(2))]))
        )
    );
}

#[test]
fn check_sources_with_min_amount() {
    let mut aggregator = LiquidityAggregator::<Runtime, _>::new(SwapVariant::WithDesiredInput);
    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(90), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
                SwapChunk::new(balance!(10), balance!(70), Default::default()),
                SwapChunk::new(balance!(10), balance!(60), Default::default()),
            ]),
            limits: SwapLimits::new(Some(SideAmount::Input(balance!(30))), None, None),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(125), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
                SwapChunk::new(balance!(10), balance!(50), Default::default()),
                SwapChunk::new(balance!(10), balance!(40), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(30))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.00001))),
            ),
        },
    );

    // liquidity were taken from both sources, but it didn't match the min amount requirements,
    // but the total amount is enough to exceed the min amount in one of sources.
    // Liquidity was redistributed to one source.
    assert_eq!(
        aggregator.aggregate_liquidity(balance!(40)).unwrap(),
        AggregationResult::new(
            SwapInfo::from([(
                LiquiditySourceType::OrderBook,
                (balance!(40), balance!(355))
            )]),
            vec![(
                LiquiditySourceType::OrderBook,
                SwapAmount::with_desired_input(balance!(40), balance!(355))
            )],
            balance!(40),
            balance!(355),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );
}

#[test]
fn check_sources_with_precision() {
    let mut aggregator = LiquidityAggregator::<Runtime, _>::new(SwapVariant::WithDesiredInput);
    aggregator.add_source(
        LiquiditySourceType::XYKPool,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
                SwapChunk::new(balance!(10), balance!(50), Default::default()),
                SwapChunk::new(balance!(10), balance!(40), Default::default()),
            ]),
            limits: SwapLimits::new(None, None, Some(SideAmount::Output(balance!(0.01)))),
        },
    );

    aggregator.add_source(
        LiquiditySourceType::OrderBook,
        DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::new(balance!(10), balance!(125), Default::default()),
                SwapChunk::new(balance!(10), balance!(100), Default::default()),
                SwapChunk::new(balance!(10), balance!(80), Default::default()),
                SwapChunk::new(balance!(10), balance!(50), Default::default()),
                SwapChunk::new(balance!(10), balance!(40), Default::default()),
            ]),
            limits: SwapLimits::new(
                Some(SideAmount::Input(balance!(1))),
                Some(SideAmount::Input(balance!(1000))),
                Some(SideAmount::Input(balance!(0.1))),
            ),
        },
    );

    assert_eq!(
        aggregator
            .aggregate_liquidity(balance!(19.9999999))
            .unwrap(),
        AggregationResult::new(
            SwapInfo::from([
                (
                    LiquiditySourceType::XYKPool,
                    (balance!(0.099), balance!(0.99))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    (balance!(19.9), balance!(224))
                )
            ]),
            vec![
                (
                    LiquiditySourceType::XYKPool,
                    SwapAmount::with_desired_input(balance!(0.099), balance!(0.99))
                ),
                (
                    LiquiditySourceType::OrderBook,
                    SwapAmount::with_desired_input(balance!(19.9), balance!(224))
                )
            ],
            balance!(19.999),
            balance!(224.99),
            SwapVariant::WithDesiredInput,
            Default::default()
        )
    );
}
