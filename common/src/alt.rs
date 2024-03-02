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
use crate::swap_amount::SwapVariant;
use crate::{Balance, Fixed, Price};
use sp_runtime::traits::Zero;
use sp_std::collections::vec_deque::VecDeque;
use sp_std::ops::Add;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SideAmount<AmountType> {
    Input(AmountType),
    Output(AmountType),
}

impl<AmountType> SideAmount<AmountType> {
    pub fn new(amount: AmountType, swap_variant: SwapVariant) -> Self {
        match swap_variant {
            SwapVariant::WithDesiredInput => Self::Input(amount),
            SwapVariant::WithDesiredOutput => Self::Output(amount),
        }
    }

    pub fn amount(&self) -> &AmountType {
        match self {
            Self::Input(amount) => amount,
            Self::Output(amount) => amount,
        }
    }

    pub fn set_amount(&mut self, amount: AmountType) {
        match self {
            Self::Input(..) => *self = Self::Input(amount),
            Self::Output(..) => *self = Self::Output(amount),
        }
    }

    pub fn is_same(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Input(..), Self::Input(..)) | (Self::Output(..), Self::Output(..))
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Fee<AmountType> {
    pub xor: AmountType,
    pub xst: AmountType,
    pub xstusd: AmountType,
}

impl<AmountType> Fee<AmountType> {
    pub fn new(xor: AmountType, xst: AmountType, xstusd: AmountType) -> Self {
        Self { xor, xst, xstusd }
    }
}

impl<AmountType: Zero> Fee<AmountType> {
    pub fn xor(xor: AmountType) -> Self {
        Self::new(xor, Zero::zero(), Zero::zero())
    }

    pub fn xst(xst: AmountType) -> Self {
        Self::new(Zero::zero(), xst, Zero::zero())
    }

    pub fn xstusd(xstusd: AmountType) -> Self {
        Self::new(Zero::zero(), Zero::zero(), xstusd)
    }
}

impl<AmountType: Zero> Zero for Fee<AmountType> {
    fn zero() -> Self {
        Self::new(Zero::zero(), Zero::zero(), Zero::zero())
    }

    fn is_zero(&self) -> bool {
        self.xor.is_zero() && self.xst.is_zero() && self.xstusd.is_zero()
    }
}

impl<AmountType: Zero> Default for Fee<AmountType> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<AmountType> Add for Fee<AmountType>
where
    AmountType: Add<Output = AmountType>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::Output::new(
            self.xor.add(other.xor),
            self.xst.add(other.xst),
            self.xstusd.add(other.xstusd),
        )
    }
}

impl Fee<Balance> {
    pub fn saturating_add(self, rhs: Self) -> Self {
        Self::new(
            self.xor.saturating_add(rhs.xor),
            self.xst.saturating_add(rhs.xst),
            self.xstusd.saturating_add(rhs.xstusd),
        )
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self::new(
            self.xor.saturating_sub(rhs.xor),
            self.xst.saturating_sub(rhs.xst),
            self.xstusd.saturating_sub(rhs.xstusd),
        )
    }

    pub fn rescale_by_ratio(self, ratio: FixedWrapper) -> Option<Self> {
        let xor = (FixedWrapper::from(self.xor) * ratio.clone())
            .try_into_balance()
            .ok()?;
        let xst = (FixedWrapper::from(self.xst) * ratio.clone())
            .try_into_balance()
            .ok()?;
        let xstusd = (FixedWrapper::from(self.xstusd) * ratio)
            .try_into_balance()
            .ok()?;
        Some(Self::new(xor, xst, xstusd))
    }

    // Multiply all values by `n`
    pub fn mul_n(self, n: usize) -> Option<Self> {
        let xor = self.xor.checked_mul(n as Balance)?;
        let xst = self.xst.checked_mul(n as Balance)?;
        let xstusd = self.xstusd.checked_mul(n as Balance)?;
        Some(Self::new(xor, xst, xstusd))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SwapChunk<AmountType> {
    pub input: AmountType,
    pub output: AmountType,
    pub fee: Fee<AmountType>,
}

impl<AmountType> SwapChunk<AmountType> {
    pub fn new(input: AmountType, output: AmountType, fee: Fee<AmountType>) -> Self {
        Self { input, output, fee }
    }
}

impl<AmountType: Copy> SwapChunk<AmountType> {
    pub fn get_associated_field(&self, swap_variant: SwapVariant) -> SideAmount<AmountType> {
        match swap_variant {
            SwapVariant::WithDesiredInput => SideAmount::Input(self.input),
            SwapVariant::WithDesiredOutput => SideAmount::Output(self.output),
        }
    }
}

impl<AmountType: PartialEq> PartialEq<SideAmount<AmountType>> for SwapChunk<AmountType> {
    fn eq(&self, other: &SideAmount<AmountType>) -> bool {
        match other {
            SideAmount::Input(input) => self.input.eq(input),
            SideAmount::Output(output) => self.output.eq(output),
        }
    }
}

impl<AmountType: PartialOrd> PartialOrd<SideAmount<AmountType>> for SwapChunk<AmountType> {
    fn partial_cmp(
        &self,
        other: &SideAmount<AmountType>,
    ) -> Option<scale_info::prelude::cmp::Ordering> {
        match other {
            SideAmount::Input(input) => self.input.partial_cmp(input),
            SideAmount::Output(output) => self.output.partial_cmp(output),
        }
    }
}

impl<AmountType: Zero> Zero for SwapChunk<AmountType> {
    fn zero() -> Self {
        Self::new(Zero::zero(), Zero::zero(), Zero::zero())
    }

    fn is_zero(&self) -> bool {
        self.input.is_zero() && self.output.is_zero() && self.fee.is_zero()
    }
}

impl<AmountType: Zero> Default for SwapChunk<AmountType> {
    fn default() -> Self {
        Self::zero()
    }
}

impl<AmountType> Add for SwapChunk<AmountType>
where
    AmountType: Add<Output = AmountType>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::new(
            self.input.add(other.input),
            self.output.add(other.output),
            self.fee.add(other.fee),
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
        let fee = self.fee.rescale_by_ratio(ratio)?;
        Some(Self::new(input, output, fee))
    }

    pub fn rescale_by_output(self, output: Balance) -> Option<Self> {
        let input = self.proportional_input(output)?;
        let ratio = FixedWrapper::from(output) / FixedWrapper::from(self.output);
        let fee = self.fee.rescale_by_ratio(ratio)?;
        Some(Self::new(input, output, fee))
    }

    pub fn rescale_by_ratio(self, ratio: Fixed) -> Option<Self> {
        let input = (FixedWrapper::from(self.input) * ratio)
            .try_into_balance()
            .ok()?;
        let output = (FixedWrapper::from(self.output) * ratio)
            .try_into_balance()
            .ok()?;
        let fee = self.fee.rescale_by_ratio(ratio.into())?;
        Some(Self::new(input, output, fee))
    }

    pub fn rescale_by_side_amount(self, amount: SideAmount<Balance>) -> Option<Self> {
        match amount {
            SideAmount::Input(input) => self.rescale_by_input(input),
            SideAmount::Output(output) => self.rescale_by_output(output),
        }
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
    pub min_amount: Option<SideAmount<AmountType>>,

    /// The amount of swap cannot be more than `max_amount` if it's defined
    pub max_amount: Option<SideAmount<AmountType>>,

    /// The amount of swap must be a multiplier of `amount_precision` if it's defined
    pub amount_precision: Option<SideAmount<AmountType>>,
}

impl<AmountType> SwapLimits<AmountType> {
    pub fn new(
        min_amount: Option<SideAmount<AmountType>>,
        max_amount: Option<SideAmount<AmountType>>,
        amount_precision: Option<SideAmount<AmountType>>,
    ) -> Self {
        Self {
            min_amount,
            max_amount,
            amount_precision,
        }
    }
}

impl SwapLimits<Balance> {
    /// Aligns the `chunk` regarding to the `min_amount` limit.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk_min(
        &self,
        chunk: SwapChunk<Balance>,
    ) -> Option<(SwapChunk<Balance>, SwapChunk<Balance>)> {
        if let Some(min) = self.min_amount {
            match min {
                SideAmount::Input(min_amount) => {
                    if chunk.input < min_amount {
                        return Some((Zero::zero(), chunk));
                    }
                }
                SideAmount::Output(min_amount) => {
                    if chunk.output < min_amount {
                        return Some((Zero::zero(), chunk));
                    }
                }
            }
        }
        Some((chunk, Zero::zero()))
    }

    /// Aligns the `chunk` regarding to the `max_amount` limit.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk_max(
        &self,
        chunk: SwapChunk<Balance>,
    ) -> Option<(SwapChunk<Balance>, SwapChunk<Balance>)> {
        if let Some(max) = self.max_amount {
            match max {
                SideAmount::Input(max_amount) => {
                    if chunk.input > max_amount {
                        let rescaled = chunk.rescale_by_input(max_amount)?;
                        let remainder = chunk.saturating_sub(rescaled);
                        return Some((rescaled, remainder));
                    }
                }
                SideAmount::Output(max_amount) => {
                    if chunk.output > max_amount {
                        let rescaled = chunk.rescale_by_output(max_amount)?;
                        let remainder = chunk.saturating_sub(rescaled);
                        return Some((rescaled, remainder));
                    }
                }
            }
        }
        Some((chunk, Zero::zero()))
    }

    /// Aligns the `chunk` regarding to the `amount_precision` limit.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk_precision(
        &self,
        chunk: SwapChunk<Balance>,
    ) -> Option<(SwapChunk<Balance>, SwapChunk<Balance>)> {
        if let Some(precision) = self.amount_precision {
            match precision {
                SideAmount::Input(precision) => {
                    if chunk.input % precision != Balance::zero() {
                        let count = chunk.input.saturating_div(precision);
                        let aligned = count.saturating_mul(precision);
                        let rescaled = chunk.rescale_by_input(aligned)?;
                        let remainder = chunk.saturating_sub(rescaled);
                        return Some((rescaled, remainder));
                    }
                }
                SideAmount::Output(precision) => {
                    if chunk.output % precision != Balance::zero() {
                        let count = chunk.output.saturating_div(precision);
                        let aligned = count.saturating_mul(precision);
                        let rescaled = chunk.rescale_by_output(aligned)?;
                        let remainder = chunk.saturating_sub(rescaled);
                        return Some((rescaled, remainder));
                    }
                }
            }
        }
        Some((chunk, Zero::zero()))
    }

    /// Aligns the `chunk` regarding to the limits.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk(
        &self,
        chunk: SwapChunk<Balance>,
    ) -> Option<(SwapChunk<Balance>, SwapChunk<Balance>)> {
        let (chunk, remainder) = self.align_chunk_min(chunk)?;
        if !remainder.is_zero() {
            return Some((chunk, remainder));
        }

        let (chunk, remainder) = self.align_chunk_max(chunk)?;
        if !remainder.is_zero() {
            return Some((chunk, remainder));
        }

        let (chunk, remainder) = self.align_chunk_precision(chunk)?;
        if !remainder.is_zero() {
            return Some((chunk, remainder));
        }

        Some((chunk, Zero::zero()))
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
