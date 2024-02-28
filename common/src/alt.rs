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

// This file contains ALT types

use crate::fixed_wrapper::FixedWrapper;
use crate::{Balance, Fixed, Price};
use sp_runtime::traits::Zero;
use sp_std::collections::vec_deque::VecDeque;
use sp_std::ops::Add;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SwapChunk<AmountType> {
    pub input: AmountType,
    pub output: AmountType,
    pub fee: AmountType,
}

impl<AmountType> SwapChunk<AmountType> {
    pub fn new(input: AmountType, output: AmountType, fee: AmountType) -> Self {
        Self { input, output, fee }
    }
}

impl Zero for SwapChunk<Balance> {
    fn zero() -> Self {
        Self::new(Balance::zero(), Balance::zero(), Balance::zero())
    }

    fn is_zero(&self) -> bool {
        self.input.is_zero() && self.output.is_zero() && self.fee.is_zero()
    }
}

impl Default for SwapChunk<Balance> {
    fn default() -> Self {
        Self::zero()
    }
}

impl Add for SwapChunk<Balance> {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::new(
            self.input + other.input,
            self.output + other.output,
            self.fee + other.fee,
        )
    }
}

impl SwapChunk<Balance> {
    /// Calculates a price of the chunk
    pub fn price(&self) -> Option<Price> {
        (FixedWrapper::from(self.output) / FixedWrapper::from(self.input))
            .get()
            .ok()
    }

    /// Calculates a linearly proportional input amount depending on the price and an output amount.
    /// `output` attribute must be less than or equal to `self.output`
    pub fn proportional_input(&self, output: Balance) -> Option<Balance> {
        if output.is_zero() {
            return Some(Balance::zero());
        }

        let price = self.price()?;

        (FixedWrapper::from(output) / price).try_into_balance().ok()
    }

    /// Calculates a linearly proportional output amount depending on the price and an input amount.
    /// `input` attribute must be less than or equal to `self.input`
    pub fn proportional_output(&self, input: Balance) -> Option<Balance> {
        if input.is_zero() {
            return Some(Balance::zero());
        }

        let price = self.price()?;

        (FixedWrapper::from(input) * price).try_into_balance().ok()
    }

    pub fn rescale_by_input(self, input: Balance) -> Option<Self> {
        let output = self.proportional_output(input)?;
        let ratio = FixedWrapper::from(input) / FixedWrapper::from(self.input);
        let fee = (FixedWrapper::from(self.fee) * ratio)
            .try_into_balance()
            .ok()?;
        Some(Self::new(input, output, fee))
    }

    pub fn rescale_by_output(self, output: Balance) -> Option<Self> {
        let input = self.proportional_input(output)?;
        let ratio = FixedWrapper::from(output) / FixedWrapper::from(self.output);
        let fee = (FixedWrapper::from(self.fee) * ratio)
            .try_into_balance()
            .ok()?;
        Some(Self::new(input, output, fee))
    }

    pub fn rescale_by_ratio(self, ratio: Fixed) -> Option<Self> {
        let input = (FixedWrapper::from(self.input) * ratio)
            .try_into_balance()
            .ok()?;
        let output = (FixedWrapper::from(self.output) * ratio)
            .try_into_balance()
            .ok()?;
        let fee = (FixedWrapper::from(self.fee) * ratio)
            .try_into_balance()
            .ok()?;
        Some(Self::new(input, output, fee))
    }

    pub fn saturating_add(self, rhs: Self) -> Self {
        Self::new(
            self.input.saturating_add(rhs.input),
            self.output.saturating_add(rhs.output),
            self.fee.saturating_add(rhs.fee),
        )
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self::new(
            self.input.saturating_sub(rhs.input),
            self.output.saturating_sub(rhs.output),
            self.fee.saturating_sub(rhs.fee),
        )
    }
}

/// Limitations that could have a liquidity source for the amount of swap
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct SwapLimits<AmountType> {
    /// The amount of swap cannot be less than `min_amount` if it's defined
    pub min_amount: Option<AmountType>,

    /// The amount of swap cannot be more than `max_amount` if it's defined
    pub max_amount: Option<AmountType>,

    /// The amount of swap must be a multiplier of `amount_precision` if it's defined
    pub amount_precision: Option<AmountType>,
}

impl<AmountType> SwapLimits<AmountType> {
    pub fn new(
        min_amount: Option<AmountType>,
        max_amount: Option<AmountType>,
        amount_precision: Option<AmountType>,
    ) -> Self {
        Self {
            min_amount,
            max_amount,
            amount_precision,
        }
    }
}

impl SwapLimits<Balance> {
    /// Aligns the `amount` regarding to the `max_amount` limit.
    /// Returns the aligned amount and the remainder
    pub fn align_max(&self, amount: Balance) -> (Balance, Balance) {
        if let Some(max) = self.max_amount {
            if amount > max {
                return (max, amount.saturating_sub(max));
            }
        }
        (amount, Balance::zero())
    }

    /// Aligns the `amount` regarding to limits.
    /// Returns the aligned amount and the remainder
    pub fn align(&self, amount: Balance) -> (Balance, Balance) {
        if let Some(min) = self.min_amount {
            if amount < min {
                return (Balance::zero(), amount);
            }
        }

        if let Some(max) = self.max_amount {
            if amount > max {
                return (max, amount.saturating_sub(max));
            }
        }

        if let Some(precision) = self.amount_precision {
            if amount % precision != Balance::zero() {
                let count = amount.saturating_div(precision);
                let aligned = count.saturating_mul(precision);
                return (aligned, amount.saturating_sub(aligned));
            }
        }

        (amount, Balance::zero())
    }
}

/// Discrete result of quotation
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct DiscreteQuotation<AmountType> {
    pub chunks: VecDeque<SwapChunk<AmountType>>,
    pub limits: SwapLimits<AmountType>,
}

impl<AmountType> DiscreteQuotation<AmountType> {
    pub fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            limits: SwapLimits::new(None, None, None),
        }
    }
}
