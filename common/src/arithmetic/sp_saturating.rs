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

use crate::{
    arithmetic::bounds::Bounded,
    arithmetic::checked::{checked_pow, CheckedMul},
    arithmetic::identities::{One, Zero},
    arithmetic::saturating,
};

/// Saturating arithmetic operations, returning maximum or minimum values instead of overflowing.
pub trait Saturating {
    /// Saturating addition. Compute `self + rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    fn saturating_add(self, rhs: Self) -> Self;

    /// Saturating subtraction. Compute `self - rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    fn saturating_sub(self, rhs: Self) -> Self;

    /// Saturating multiply. Compute `self * rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    fn saturating_mul(self, rhs: Self) -> Self;

    /// Saturating exponentiation. Compute `self.pow(exp)`, saturating at the numeric bounds
    /// instead of overflowing.
    fn saturating_pow(self, exp: usize) -> Self;

    /// Increment self by one, saturating.
    fn saturating_inc(&mut self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_add(One::one());
    }

    /// Decrement self by one, saturating at zero.
    fn saturating_dec(&mut self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_sub(One::one());
    }

    /// Increment self by some `amount`, saturating.
    fn saturating_accrue(&mut self, amount: Self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_add(amount);
    }

    /// Decrement self by some `amount`, saturating at zero.
    fn saturating_reduce(&mut self, amount: Self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_sub(amount);
    }
}

impl<T: Clone + Zero + One + PartialOrd + CheckedMul + Bounded + saturating::Saturating> Saturating
    for T
{
    fn saturating_add(self, o: Self) -> Self {
        <Self as saturating::Saturating>::saturating_add(self, o)
    }

    fn saturating_sub(self, o: Self) -> Self {
        <Self as saturating::Saturating>::saturating_sub(self, o)
    }

    fn saturating_mul(self, o: Self) -> Self {
        self.checked_mul(&o).unwrap_or_else(|| {
            if (self < T::zero()) != (o < T::zero()) {
                Bounded::min_value()
            } else {
                Bounded::max_value()
            }
        })
    }

    fn saturating_pow(self, exp: usize) -> Self {
        let neg = self < T::zero() && exp % 2 != 0;
        checked_pow(self, exp).unwrap_or_else(|| {
            if neg {
                Bounded::min_value()
            } else {
                Bounded::max_value()
            }
        })
    }
}
