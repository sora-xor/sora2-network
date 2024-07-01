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

// This file contains ALT (Aggregated Liquidity Technology) types

use crate::fixed_wrapper::FixedWrapper;
use crate::outcome_fee::OutcomeFee;
use crate::swap_amount::SwapVariant;
use crate::{Balance, Fixed, Price};
use fixnum::ops::Bounded;
use fixnum::ArithmeticError;
use sp_runtime::traits::{Saturating, Zero};
use sp_std::collections::vec_deque::VecDeque;
use sp_std::ops::Add;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SwapChunk<AssetId: Ord + Clone, AmountType> {
    pub input: AmountType,
    pub output: AmountType,
    pub fee: OutcomeFee<AssetId, AmountType>,
}

impl<AssetId: Ord + Clone, AmountType> SwapChunk<AssetId, AmountType> {
    pub fn new(
        input: AmountType,
        output: AmountType,
        fee: OutcomeFee<AssetId, AmountType>,
    ) -> Self {
        Self { input, output, fee }
    }
}

impl<AssetId: Ord + Clone, AmountType: Copy> SwapChunk<AssetId, AmountType> {
    pub fn get_associated_field(&self, swap_variant: SwapVariant) -> SideAmount<AmountType> {
        match swap_variant {
            SwapVariant::WithDesiredInput => SideAmount::Input(self.input),
            SwapVariant::WithDesiredOutput => SideAmount::Output(self.output),
        }
    }

    pub fn get_same_type_amount(
        &self,
        reference: &SideAmount<AmountType>,
    ) -> SideAmount<AmountType> {
        match reference {
            SideAmount::Input(..) => SideAmount::Input(self.input),
            SideAmount::Output(..) => SideAmount::Output(self.output),
        }
    }
}

impl<AssetId: Ord + Clone, AmountType: PartialEq> PartialEq<SideAmount<AmountType>>
    for SwapChunk<AssetId, AmountType>
{
    fn eq(&self, other: &SideAmount<AmountType>) -> bool {
        match other {
            SideAmount::Input(input) => self.input.eq(input),
            SideAmount::Output(output) => self.output.eq(output),
        }
    }
}

impl<AssetId: Ord + Clone, AmountType: PartialOrd> PartialOrd<SideAmount<AmountType>>
    for SwapChunk<AssetId, AmountType>
{
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

impl<AssetId, AmountType> Zero for SwapChunk<AssetId, AmountType>
where
    AssetId: Ord + Clone,
    AmountType: Zero + Copy + Saturating,
{
    fn zero() -> Self {
        Self::new(Zero::zero(), Zero::zero(), Default::default())
    }

    fn is_zero(&self) -> bool {
        self.input.is_zero() && self.output.is_zero() && self.fee.is_zero_fee()
    }
}

impl<AssetId, AmountType> Default for SwapChunk<AssetId, AmountType>
where
    AssetId: Ord + Clone,
    AmountType: Zero + Copy + Saturating,
{
    fn default() -> Self {
        Self::zero()
    }
}

impl<AssetId, AmountType> Add for SwapChunk<AssetId, AmountType>
where
    AssetId: Ord + Clone,
    AmountType: Add<Output = AmountType> + Copy + Saturating,
{
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self::new(
            self.input.add(other.input),
            self.output.add(other.output),
            self.fee.merge(other.fee),
        )
    }
}

impl<AssetId: Ord + Clone> SwapChunk<AssetId, Balance> {
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

        let result = ((FixedWrapper::from(output) * FixedWrapper::from(self.input))
            / (FixedWrapper::from(self.output)))
        .try_into_balance();

        if let Err(error) = result {
            if error == ArithmeticError::Overflow {
                // try to use another approach with the same result to avoid overflow
                let price = self.price()?;
                (FixedWrapper::from(output) / price).try_into_balance().ok()
            } else {
                None
            }
        } else {
            result.ok()
        }
    }

    /// Calculates a linearly proportional output amount depending on the price and an input amount.
    /// `input` attribute must be less than or equal to `self.input`
    pub fn proportional_output(&self, input: Balance) -> Option<Balance> {
        if input.is_zero() {
            return Some(Balance::zero());
        }

        let result = (FixedWrapper::from(input) * FixedWrapper::from(self.output)
            / FixedWrapper::from(self.input))
        .try_into_balance();

        if let Err(error) = result {
            if error == ArithmeticError::Overflow {
                // try to use another approach with the same result to avoid overflow
                let price = self.price()?;
                (FixedWrapper::from(input) * price).try_into_balance().ok()
            } else {
                None
            }
        } else {
            result.ok()
        }
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
            self.fee.merge(rhs.fee),
        )
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self::new(
            self.input.saturating_sub(rhs.input),
            self.output.saturating_sub(rhs.output),
            self.fee.subtract(rhs.fee),
        )
    }
}

/// Limitations that could have a liquidity source for the amount of swap
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
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
    pub fn get_precision_step<AssetId: Ord + Clone>(
        &self,
        chunk: &SwapChunk<AssetId, Balance>,
        variant: SwapVariant,
    ) -> Option<Balance> {
        let step = if let Some(precision) = self.amount_precision {
            match (variant, precision) {
                (SwapVariant::WithDesiredInput, SideAmount::Input(value)) => value,
                (SwapVariant::WithDesiredOutput, SideAmount::Output(value)) => value,
                (SwapVariant::WithDesiredInput, SideAmount::Output(value)) => {
                    chunk.proportional_input(value)?
                }
                (SwapVariant::WithDesiredOutput, SideAmount::Input(value)) => {
                    chunk.proportional_output(value)?
                }
            }
        } else {
            Balance::zero()
        };
        Some(step)
    }

    /// Aligns the `chunk` regarding to the `min_amount` limit.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk_min<AssetId: Ord + Clone>(
        &self,
        chunk: SwapChunk<AssetId, Balance>,
    ) -> (SwapChunk<AssetId, Balance>, SwapChunk<AssetId, Balance>) {
        if let Some(min) = self.min_amount {
            match min {
                SideAmount::Input(min_amount) => {
                    if chunk.input < min_amount {
                        return (Zero::zero(), chunk);
                    }
                }
                SideAmount::Output(min_amount) => {
                    if chunk.output < min_amount {
                        return (Zero::zero(), chunk);
                    }
                }
            }
        }
        (chunk, Zero::zero())
    }

    /// Aligns the `chunk` regarding to the `max_amount` limit.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk_max<AssetId: Ord + Clone>(
        &self,
        chunk: SwapChunk<AssetId, Balance>,
    ) -> Option<(SwapChunk<AssetId, Balance>, SwapChunk<AssetId, Balance>)> {
        if let Some(max) = self.max_amount {
            match max {
                SideAmount::Input(max_amount) => {
                    if chunk.input > max_amount {
                        let rescaled = chunk.clone().rescale_by_input(max_amount)?;
                        let remainder = chunk.saturating_sub(rescaled.clone());
                        return Some((rescaled, remainder));
                    }
                }
                SideAmount::Output(max_amount) => {
                    if chunk.output > max_amount {
                        let rescaled = chunk.clone().rescale_by_output(max_amount)?;
                        let remainder = chunk.saturating_sub(rescaled.clone());
                        return Some((rescaled, remainder));
                    }
                }
            }
        }
        Some((chunk, Zero::zero()))
    }

    /// Aligns the extra `chunk` regarding to the `max_amount` limit taking into account in calculations the accumulator `acc` values.
    /// Returns the aligned chunk and the remainder
    pub fn align_extra_chunk_max<AssetId: Ord + Clone>(
        &self,
        acc: SwapChunk<AssetId, Balance>,
        chunk: SwapChunk<AssetId, Balance>,
    ) -> Option<(SwapChunk<AssetId, Balance>, SwapChunk<AssetId, Balance>)> {
        if let Some(max) = self.max_amount {
            match max {
                SideAmount::Input(max_amount) => {
                    if acc.input.saturating_add(chunk.input) > max_amount {
                        let diff = max_amount.saturating_sub(acc.input);
                        let rescaled = chunk.clone().rescale_by_input(diff)?;
                        let remainder = chunk.saturating_sub(rescaled.clone());
                        return Some((rescaled, remainder));
                    }
                }
                SideAmount::Output(max_amount) => {
                    if acc.output.saturating_add(chunk.output) > max_amount {
                        let diff = max_amount.saturating_sub(acc.output);
                        let rescaled = chunk.clone().rescale_by_output(diff)?;
                        let remainder = chunk.saturating_sub(rescaled.clone());
                        return Some((rescaled, remainder));
                    }
                }
            }
        }
        Some((chunk, Zero::zero()))
    }

    /// Aligns the `chunk` regarding to the `amount_precision` limit.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk_precision<AssetId: Ord + Clone>(
        &self,
        chunk: SwapChunk<AssetId, Balance>,
    ) -> Option<(SwapChunk<AssetId, Balance>, SwapChunk<AssetId, Balance>)> {
        if let Some(precision) = self.amount_precision {
            match precision {
                SideAmount::Input(precision) => {
                    if chunk.input % precision != Balance::zero() {
                        let count = chunk.input.saturating_div(precision);
                        let aligned = count.saturating_mul(precision);
                        let rescaled = chunk.clone().rescale_by_input(aligned)?;
                        let remainder = chunk.saturating_sub(rescaled.clone());
                        return Some((rescaled, remainder));
                    }
                }
                SideAmount::Output(precision) => {
                    if chunk.output % precision != Balance::zero() {
                        let count = chunk.output.saturating_div(precision);
                        let aligned = count.saturating_mul(precision);
                        let rescaled = chunk.clone().rescale_by_output(aligned)?;
                        let remainder = chunk.saturating_sub(rescaled.clone());
                        return Some((rescaled, remainder));
                    }
                }
            }
        }
        Some((chunk, Zero::zero()))
    }

    /// Aligns the `chunk` regarding to the limits.
    /// Returns the aligned chunk and the remainder
    pub fn align_chunk<AssetId: Ord + Clone>(
        &self,
        chunk: SwapChunk<AssetId, Balance>,
    ) -> Option<(SwapChunk<AssetId, Balance>, SwapChunk<AssetId, Balance>)> {
        let (chunk, remainder) = self.align_chunk_min(chunk);
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
pub struct DiscreteQuotation<AssetId: Ord + Clone, AmountType> {
    pub chunks: VecDeque<SwapChunk<AssetId, AmountType>>,
    pub limits: SwapLimits<AmountType>,
}

impl<AssetId: Ord + Clone, AmountType> DiscreteQuotation<AssetId, AmountType> {
    pub fn new() -> Self {
        Self {
            chunks: VecDeque::new(),
            limits: SwapLimits::new(None, None, None),
        }
    }
}

impl<AssetId: Ord + Clone> DiscreteQuotation<AssetId, Balance> {
    pub fn verify(&self) -> bool {
        let mut prev_price = Price::MAX;

        for chunk in &self.chunks {
            // chunk should not contain zeros
            if chunk.input.is_zero() || chunk.output.is_zero() {
                return false;
            }

            // if source provides the precision limit - all chunks must match this requirement.
            if let Some(precision) = self.limits.amount_precision {
                let (input_precision, output_precision) = match precision {
                    SideAmount::Input(input_precision) => {
                        let Some(output_precision) = self
                            .limits
                            .get_precision_step(chunk, SwapVariant::WithDesiredOutput)
                        else {
                            return false;
                        };
                        (input_precision, output_precision)
                    }
                    SideAmount::Output(output_precision) => {
                        let Some(input_precision) = self
                            .limits
                            .get_precision_step(chunk, SwapVariant::WithDesiredInput)
                        else {
                            return false;
                        };
                        (input_precision, output_precision)
                    }
                };

                if chunk.input % input_precision != 0 || chunk.output % output_precision != 0 {
                    return false;
                }
            }

            let Some(price) = chunk.price() else {
                return false;
            };

            // chunks should go to reduce the price, from the best to the worst
            if price > prev_price {
                return false;
            }
            prev_price = price;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::balance;

    #[test]
    fn check_align_chunk_min() {
        let mock_fee = OutcomeFee::from_asset(1, balance!(0.01));

        let input_min_limit = SwapLimits::new(Some(SideAmount::Input(balance!(1))), None, None);
        let output_min_limit = SwapLimits::new(Some(SideAmount::Output(balance!(1))), None, None);
        let empty_min_limit = SwapLimits::new(None, None, None);

        let chunk1 = SwapChunk::new(balance!(0.1), balance!(0.2), mock_fee.clone());
        let chunk2 = SwapChunk::new(balance!(10), balance!(0.2), mock_fee.clone());
        let chunk3 = SwapChunk::new(balance!(0.1), balance!(20), mock_fee.clone());
        let chunk4 = SwapChunk::new(balance!(10), balance!(20), mock_fee.clone());
        let chunk5 = SwapChunk::new(balance!(1), balance!(1), mock_fee);

        assert_eq!(
            input_min_limit.align_chunk_min(chunk1.clone()),
            (Zero::zero(), chunk1.clone())
        );
        assert_eq!(
            input_min_limit.align_chunk_min(chunk2.clone()),
            (chunk2.clone(), Zero::zero())
        );
        assert_eq!(
            input_min_limit.align_chunk_min(chunk3.clone()),
            (Zero::zero(), chunk3.clone())
        );
        assert_eq!(
            input_min_limit.align_chunk_min(chunk4.clone()),
            (chunk4.clone(), Zero::zero())
        );
        assert_eq!(
            input_min_limit.align_chunk_min(chunk5.clone()),
            (chunk5.clone(), Zero::zero())
        );

        assert_eq!(
            output_min_limit.align_chunk_min(chunk1.clone()),
            (Zero::zero(), chunk1.clone())
        );
        assert_eq!(
            output_min_limit.align_chunk_min(chunk2.clone()),
            (Zero::zero(), chunk2.clone())
        );
        assert_eq!(
            output_min_limit.align_chunk_min(chunk3.clone()),
            (chunk3.clone(), Zero::zero())
        );
        assert_eq!(
            output_min_limit.align_chunk_min(chunk4.clone()),
            (chunk4.clone(), Zero::zero())
        );
        assert_eq!(
            output_min_limit.align_chunk_min(chunk5.clone()),
            (chunk5.clone(), Zero::zero())
        );

        assert_eq!(
            empty_min_limit.align_chunk_min(chunk1.clone()),
            (chunk1, Zero::zero())
        );
        assert_eq!(
            empty_min_limit.align_chunk_min(chunk2.clone()),
            (chunk2, Zero::zero())
        );
        assert_eq!(
            empty_min_limit.align_chunk_min(chunk3.clone()),
            (chunk3, Zero::zero())
        );
        assert_eq!(
            empty_min_limit.align_chunk_min(chunk4.clone()),
            (chunk4, Zero::zero())
        );
        assert_eq!(
            empty_min_limit.align_chunk_min(chunk5.clone()),
            (chunk5, Zero::zero())
        );
    }

    #[test]
    fn check_align_chunk_max() {
        let mock_fee = OutcomeFee::from_asset(1, balance!(10));

        let input_max_limit = SwapLimits::new(None, Some(SideAmount::Input(balance!(100))), None);
        let output_max_limit = SwapLimits::new(None, Some(SideAmount::Output(balance!(100))), None);
        let empty_max_limit = SwapLimits::new(None, None, None);

        let chunk1 = SwapChunk::new(balance!(80), balance!(90), mock_fee.clone());
        let chunk2 = SwapChunk::new(balance!(160), balance!(90), mock_fee.clone());
        let chunk3 = SwapChunk::new(balance!(80), balance!(160), mock_fee.clone());
        let chunk4 = SwapChunk::new(balance!(160), balance!(250), mock_fee.clone());
        let chunk5 = SwapChunk::new(balance!(100), balance!(100), mock_fee);

        assert_eq!(
            input_max_limit.align_chunk_max(chunk1.clone()).unwrap(),
            (chunk1.clone(), Zero::zero())
        );
        assert_eq!(
            input_max_limit.align_chunk_max(chunk2.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(100),
                    balance!(56.25),
                    OutcomeFee::from_asset(1, balance!(6.25))
                ),
                SwapChunk::new(
                    balance!(60),
                    balance!(33.75),
                    OutcomeFee::from_asset(1, balance!(3.75))
                )
            )
        );
        assert_eq!(
            input_max_limit.align_chunk_max(chunk3.clone()).unwrap(),
            (chunk3.clone(), Zero::zero())
        );
        assert_eq!(
            input_max_limit.align_chunk_max(chunk4.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(100),
                    balance!(156.25),
                    OutcomeFee::from_asset(1, balance!(6.25))
                ),
                SwapChunk::new(
                    balance!(60),
                    balance!(93.75),
                    OutcomeFee::from_asset(1, balance!(3.75))
                )
            )
        );
        assert_eq!(
            input_max_limit.align_chunk_max(chunk5.clone()).unwrap(),
            (chunk5.clone(), Zero::zero())
        );

        assert_eq!(
            output_max_limit.align_chunk_max(chunk1.clone()).unwrap(),
            (chunk1.clone(), Zero::zero())
        );
        assert_eq!(
            output_max_limit.align_chunk_max(chunk2.clone()).unwrap(),
            (chunk2.clone(), Zero::zero())
        );
        assert_eq!(
            output_max_limit.align_chunk_max(chunk3.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(50),
                    balance!(100),
                    OutcomeFee::from_asset(1, balance!(6.25))
                ),
                SwapChunk::new(
                    balance!(30),
                    balance!(60),
                    OutcomeFee::from_asset(1, balance!(3.75))
                )
            )
        );
        assert_eq!(
            output_max_limit.align_chunk_max(chunk4.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(64),
                    balance!(100),
                    OutcomeFee::from_asset(1, balance!(4))
                ),
                SwapChunk::new(
                    balance!(96),
                    balance!(150),
                    OutcomeFee::from_asset(1, balance!(6))
                )
            )
        );
        assert_eq!(
            output_max_limit.align_chunk_max(chunk5.clone()).unwrap(),
            (chunk5.clone(), Zero::zero())
        );

        assert_eq!(
            empty_max_limit.align_chunk_max(chunk1.clone()).unwrap(),
            (chunk1, Zero::zero())
        );
        assert_eq!(
            empty_max_limit.align_chunk_max(chunk2.clone()).unwrap(),
            (chunk2, Zero::zero())
        );
        assert_eq!(
            empty_max_limit.align_chunk_max(chunk3.clone()).unwrap(),
            (chunk3, Zero::zero())
        );
        assert_eq!(
            empty_max_limit.align_chunk_max(chunk4.clone()).unwrap(),
            (chunk4, Zero::zero())
        );
        assert_eq!(
            empty_max_limit.align_chunk_max(chunk5.clone()).unwrap(),
            (chunk5, Zero::zero())
        );
    }

    #[test]
    fn check_align_extra_chunk_max() {
        let mock_fee = OutcomeFee::from_asset(1, balance!(10));

        let input_max_limit = SwapLimits::new(None, Some(SideAmount::Input(balance!(200))), None);
        let output_max_limit = SwapLimits::new(None, Some(SideAmount::Output(balance!(200))), None);
        let empty_max_limit = SwapLimits::new(None, None, None);

        let acc = SwapChunk::new(balance!(100), balance!(100), mock_fee.clone());

        let chunk1 = SwapChunk::new(balance!(80), balance!(90), mock_fee.clone());
        let chunk2 = SwapChunk::new(balance!(160), balance!(90), mock_fee.clone());
        let chunk3 = SwapChunk::new(balance!(80), balance!(160), mock_fee.clone());
        let chunk4 = SwapChunk::new(balance!(160), balance!(250), mock_fee.clone());
        let chunk5 = SwapChunk::new(balance!(100), balance!(100), mock_fee);

        assert_eq!(
            input_max_limit
                .align_extra_chunk_max(acc.clone(), chunk1.clone())
                .unwrap(),
            (chunk1.clone(), Zero::zero())
        );
        assert_eq!(
            input_max_limit
                .align_extra_chunk_max(acc.clone(), chunk2.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(100),
                    balance!(56.25),
                    OutcomeFee::from_asset(1, balance!(6.25))
                ),
                SwapChunk::new(
                    balance!(60),
                    balance!(33.75),
                    OutcomeFee::from_asset(1, balance!(3.75))
                )
            )
        );
        assert_eq!(
            input_max_limit
                .align_extra_chunk_max(acc.clone(), chunk3.clone())
                .unwrap(),
            (chunk3.clone(), Zero::zero())
        );
        assert_eq!(
            input_max_limit
                .align_extra_chunk_max(acc.clone(), chunk4.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(100),
                    balance!(156.25),
                    OutcomeFee::from_asset(1, balance!(6.25))
                ),
                SwapChunk::new(
                    balance!(60),
                    balance!(93.75),
                    OutcomeFee::from_asset(1, balance!(3.75))
                )
            )
        );
        assert_eq!(
            input_max_limit
                .align_extra_chunk_max(acc.clone(), chunk5.clone())
                .unwrap(),
            (chunk5.clone(), Zero::zero())
        );

        assert_eq!(
            output_max_limit
                .align_extra_chunk_max(acc.clone(), chunk1.clone())
                .unwrap(),
            (chunk1.clone(), Zero::zero())
        );
        assert_eq!(
            output_max_limit
                .align_extra_chunk_max(acc.clone(), chunk2.clone())
                .unwrap(),
            (chunk2.clone(), Zero::zero())
        );
        assert_eq!(
            output_max_limit
                .align_extra_chunk_max(acc.clone(), chunk3.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(50),
                    balance!(100),
                    OutcomeFee::from_asset(1, balance!(6.25))
                ),
                SwapChunk::new(
                    balance!(30),
                    balance!(60),
                    OutcomeFee::from_asset(1, balance!(3.75))
                )
            )
        );
        assert_eq!(
            output_max_limit
                .align_extra_chunk_max(acc.clone(), chunk4.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(64),
                    balance!(100),
                    OutcomeFee::from_asset(1, balance!(4))
                ),
                SwapChunk::new(
                    balance!(96),
                    balance!(150),
                    OutcomeFee::from_asset(1, balance!(6))
                )
            )
        );
        assert_eq!(
            output_max_limit
                .align_extra_chunk_max(acc.clone(), chunk5.clone())
                .unwrap(),
            (chunk5.clone(), Zero::zero())
        );

        assert_eq!(
            empty_max_limit
                .align_extra_chunk_max(acc.clone(), chunk1.clone())
                .unwrap(),
            (chunk1, Zero::zero())
        );
        assert_eq!(
            empty_max_limit
                .align_extra_chunk_max(acc.clone(), chunk2.clone())
                .unwrap(),
            (chunk2, Zero::zero())
        );
        assert_eq!(
            empty_max_limit
                .align_extra_chunk_max(acc.clone(), chunk3.clone())
                .unwrap(),
            (chunk3, Zero::zero())
        );
        assert_eq!(
            empty_max_limit
                .align_extra_chunk_max(acc.clone(), chunk4.clone())
                .unwrap(),
            (chunk4, Zero::zero())
        );
        assert_eq!(
            empty_max_limit
                .align_extra_chunk_max(acc, chunk5.clone())
                .unwrap(),
            (chunk5, Zero::zero())
        );

        // todo
    }

    #[test]
    fn check_align_chunk_precision() {
        let mock_fee = OutcomeFee::from_asset(1, balance!(0.1));

        let input_precision_limit =
            SwapLimits::new(None, None, Some(SideAmount::Input(balance!(1))));
        let output_precision_limit =
            SwapLimits::new(None, None, Some(SideAmount::Output(balance!(1))));
        let empty_precision_limit = SwapLimits::new(None, None, None);

        let chunk1 = SwapChunk::new(balance!(2), balance!(1), mock_fee.clone());
        let chunk2 = SwapChunk::new(balance!(2.5), balance!(1), mock_fee.clone());
        let chunk3 = SwapChunk::new(balance!(2), balance!(1.6), mock_fee.clone());
        let chunk4 = SwapChunk::new(balance!(2.5), balance!(1.6), mock_fee);

        assert_eq!(
            input_precision_limit
                .align_chunk_precision(chunk1.clone())
                .unwrap(),
            (chunk1.clone(), Zero::zero())
        );
        assert_eq!(
            input_precision_limit
                .align_chunk_precision(chunk2.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(2),
                    balance!(0.8),
                    OutcomeFee::from_asset(1, balance!(0.08))
                ),
                SwapChunk::new(
                    balance!(0.5),
                    balance!(0.2),
                    OutcomeFee::from_asset(1, balance!(0.02))
                )
            )
        );
        assert_eq!(
            input_precision_limit
                .align_chunk_precision(chunk3.clone())
                .unwrap(),
            (chunk3.clone(), Zero::zero())
        );
        assert_eq!(
            input_precision_limit
                .align_chunk_precision(chunk4.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(2),
                    balance!(1.28),
                    OutcomeFee::from_asset(1, balance!(0.08))
                ),
                SwapChunk::new(
                    balance!(0.5),
                    balance!(0.32),
                    OutcomeFee::from_asset(1, balance!(0.02))
                )
            )
        );

        assert_eq!(
            output_precision_limit
                .align_chunk_precision(chunk1.clone())
                .unwrap(),
            (chunk1.clone(), Zero::zero())
        );
        assert_eq!(
            output_precision_limit
                .align_chunk_precision(chunk2.clone())
                .unwrap(),
            (chunk2.clone(), Zero::zero())
        );
        assert_eq!(
            output_precision_limit
                .align_chunk_precision(chunk3.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(1.25),
                    balance!(1),
                    OutcomeFee::from_asset(1, balance!(0.0625))
                ),
                SwapChunk::new(
                    balance!(0.75),
                    balance!(0.6),
                    OutcomeFee::from_asset(1, balance!(0.0375))
                )
            )
        );
        assert_eq!(
            output_precision_limit
                .align_chunk_precision(chunk4.clone())
                .unwrap(),
            (
                SwapChunk::new(
                    balance!(1.5625),
                    balance!(1),
                    OutcomeFee::from_asset(1, balance!(0.0625))
                ),
                SwapChunk::new(
                    balance!(0.9375),
                    balance!(0.6),
                    OutcomeFee::from_asset(1, balance!(0.0375))
                )
            )
        );

        assert_eq!(
            empty_precision_limit
                .align_chunk_precision(chunk1.clone())
                .unwrap(),
            (chunk1, Zero::zero())
        );
        assert_eq!(
            empty_precision_limit
                .align_chunk_precision(chunk2.clone())
                .unwrap(),
            (chunk2, Zero::zero())
        );
        assert_eq!(
            empty_precision_limit
                .align_chunk_precision(chunk3.clone())
                .unwrap(),
            (chunk3, Zero::zero())
        );
        assert_eq!(
            empty_precision_limit
                .align_chunk_precision(chunk4.clone())
                .unwrap(),
            (chunk4, Zero::zero())
        );
    }

    #[test]
    fn check_align_chunk() {
        let mock_fee = OutcomeFee::from_asset(1, balance!(1));

        let input_limit = SwapLimits::new(
            Some(SideAmount::Input(balance!(1))),
            Some(SideAmount::Input(balance!(100))),
            Some(SideAmount::Input(balance!(1))),
        );
        let output_limit = SwapLimits::new(
            Some(SideAmount::Output(balance!(1))),
            Some(SideAmount::Output(balance!(100))),
            Some(SideAmount::Output(balance!(1))),
        );
        let empty_limit = SwapLimits::new(None, None, None);

        let chunk_min = SwapChunk::new(balance!(0.1), balance!(0.2), mock_fee.clone());
        let chunk_max = SwapChunk::new(balance!(160), balance!(250), mock_fee.clone());
        let chunk_precision = SwapChunk::new(balance!(2.5), balance!(1.6), mock_fee.clone());
        let chunk_ok = SwapChunk::new(balance!(80), balance!(50), mock_fee);

        assert_eq!(
            input_limit.align_chunk(chunk_min.clone()).unwrap(),
            (Zero::zero(), chunk_min.clone())
        );
        assert_eq!(
            output_limit.align_chunk(chunk_min.clone()).unwrap(),
            (Zero::zero(), chunk_min.clone())
        );
        assert_eq!(
            empty_limit.align_chunk(chunk_min.clone()).unwrap(),
            (chunk_min, Zero::zero())
        );

        assert_eq!(
            input_limit.align_chunk(chunk_max.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(100),
                    balance!(156.25),
                    OutcomeFee::from_asset(1, balance!(0.625))
                ),
                SwapChunk::new(
                    balance!(60),
                    balance!(93.75),
                    OutcomeFee::from_asset(1, balance!(0.375))
                )
            )
        );
        assert_eq!(
            output_limit.align_chunk(chunk_max.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(64),
                    balance!(100),
                    OutcomeFee::from_asset(1, balance!(0.4))
                ),
                SwapChunk::new(
                    balance!(96),
                    balance!(150),
                    OutcomeFee::from_asset(1, balance!(0.6))
                )
            )
        );
        assert_eq!(
            empty_limit.align_chunk(chunk_max.clone()).unwrap(),
            (chunk_max, Zero::zero())
        );

        assert_eq!(
            input_limit.align_chunk(chunk_precision.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(2),
                    balance!(1.28),
                    OutcomeFee::from_asset(1, balance!(0.8))
                ),
                SwapChunk::new(
                    balance!(0.5),
                    balance!(0.32),
                    OutcomeFee::from_asset(1, balance!(0.2))
                )
            )
        );
        assert_eq!(
            output_limit.align_chunk(chunk_precision.clone()).unwrap(),
            (
                SwapChunk::new(
                    balance!(1.5625),
                    balance!(1),
                    OutcomeFee::from_asset(1, balance!(0.625))
                ),
                SwapChunk::new(
                    balance!(0.9375),
                    balance!(0.6),
                    OutcomeFee::from_asset(1, balance!(0.375))
                )
            )
        );
        assert_eq!(
            empty_limit.align_chunk(chunk_precision.clone()).unwrap(),
            (chunk_precision, Zero::zero())
        );

        assert_eq!(
            input_limit.align_chunk(chunk_ok.clone()).unwrap(),
            (chunk_ok.clone(), Zero::zero())
        );
        assert_eq!(
            output_limit.align_chunk(chunk_ok.clone()).unwrap(),
            (chunk_ok.clone(), Zero::zero())
        );
        assert_eq!(
            empty_limit.align_chunk(chunk_ok.clone()).unwrap(),
            (chunk_ok, Zero::zero())
        );
    }

    #[test]
    fn check_discrete_quotation_verification_with_zero() {
        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(0),
                balance!(0),
                Default::default(),
            )]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1),
                balance!(0),
                Default::default(),
            )]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(0),
                balance!(1),
                Default::default(),
            )]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1),
                balance!(1),
                Default::default(),
            )]),
            limits: Default::default(),
        };
        assert!(correct.verify());
    }

    #[test]
    fn check_discrete_quotation_verification_with_precision() {
        // wrong input
        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1.11),
                balance!(1),
                Default::default(),
            )]),
            limits: SwapLimits {
                min_amount: None,
                max_amount: None,
                amount_precision: Some(SideAmount::Input(balance!(0.1))),
            },
        };
        assert!(!wrong.verify());

        // wrong output
        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1),
                balance!(1.11),
                Default::default(),
            )]),
            limits: SwapLimits {
                min_amount: None,
                max_amount: None,
                amount_precision: Some(SideAmount::Output(balance!(0.1))),
            },
        };
        assert!(!wrong.verify());

        // input is ok, but output doesn't match with proportional precision
        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1.1),
                balance!(1),
                Default::default(),
            )]),
            limits: SwapLimits {
                min_amount: None,
                max_amount: None,
                amount_precision: Some(SideAmount::Input(balance!(0.1))),
            },
        };
        assert!(!wrong.verify());

        // output is ok, but input doesn't match with proportional precision
        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1),
                balance!(1.1),
                Default::default(),
            )]),
            limits: SwapLimits {
                min_amount: None,
                max_amount: None,
                amount_precision: Some(SideAmount::Output(balance!(0.1))),
            },
        };
        assert!(!wrong.verify());

        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1.1),
                balance!(1.1),
                Default::default(),
            )]),
            limits: SwapLimits {
                min_amount: None,
                max_amount: None,
                amount_precision: Some(SideAmount::Input(balance!(0.1))),
            },
        };
        assert!(correct.verify());

        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1.1),
                balance!(1.1),
                Default::default(),
            )]),
            limits: SwapLimits {
                min_amount: None,
                max_amount: None,
                amount_precision: Some(SideAmount::Output(balance!(0.1))),
            },
        };
        assert!(correct.verify());

        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([SwapChunk::<u8, _>::new(
                balance!(1.11),
                balance!(1.11),
                Default::default(),
            )]),
            limits: Default::default(),
        };
        assert!(correct.verify());
    }

    #[test]
    fn check_discrete_quotation_verification_with_price_order() {
        // wrong order
        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(1), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
            ]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(1), Default::default()),
            ]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()), // wrong
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(1), Default::default()),
            ]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        let wrong: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()), // wrong
            ]),
            limits: Default::default(),
        };
        assert!(!wrong.verify());

        //todo

        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(9), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(8), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(7), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(6), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(1), Default::default()),
            ]),
            limits: Default::default(),
        };
        assert!(correct.verify());

        // the same price in a row is ok
        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
            ]),
            limits: Default::default(),
        };
        assert!(correct.verify());

        let correct: DiscreteQuotation<_, Balance> = DiscreteQuotation {
            chunks: VecDeque::from([
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(10), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(9), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(9), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(8), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(8), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(7), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(7), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(6), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(6), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(5), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(4), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(3), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(2), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(1), Default::default()),
                SwapChunk::<u8, _>::new(balance!(1), balance!(1), Default::default()),
            ]),
            limits: Default::default(),
        };
        assert!(correct.verify());
    }
}
