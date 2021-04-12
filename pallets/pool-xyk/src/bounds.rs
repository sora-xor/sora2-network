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

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

/// Bounds enum, used for cases than min max limits is used. Also used for cases than values is
/// Desired by used or Calculated by forumula. Dummy is used to abstract checking.
#[derive(Clone, Copy, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub enum Bounds<Balance> {
    /// This is consequence of computations, and not sed by used.
    Calculated(Balance),
    /// This values set by used as fixed and determed value.
    Desired(Balance),
    /// This is undetermined value, bounded by some logic or ranges.
    Min(Balance),
    Max(Balance),
    /// This is determined value than pool is emply, then pool is not empty this works like range.
    RangeFromDesiredToMin(Balance, Balance),
    /// This is just unknown value that must be calulated and filled.
    Decide,
    /// This is used in some checks tests and predicates, than value is not needed.
    Dummy,
}

impl<Balance: Ord + Eq + Clone> Bounds<Balance> {
    /// Unwrap only known values, min and max is not known for final value.
    pub fn unwrap(self) -> Balance {
        match self {
            Bounds::Calculated(a) => a,
            Bounds::Desired(a) => a,
            Bounds::RangeFromDesiredToMin(a, _) => a,
            _ => unreachable!("Must not happen, every uncalculated bound must be set in prepare_and_validate function"),
        }
    }

    pub fn meets_the_boundaries(&self, rhs: &Self) -> bool {
        use Bounds::*;
        match (
            self,
            Option::<&Balance>::from(self),
            Option::<&Balance>::from(rhs),
        ) {
            (Min(a), _, Some(b)) => a <= b,
            (Max(a), _, Some(b)) => a >= b,
            (RangeFromDesiredToMin(a, b), _, Some(c)) => a >= c && c <= b,
            (_, Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn meets_the_boundaries_mutally(&self, rhs: &Self) -> bool {
        self.meets_the_boundaries(rhs) || rhs.meets_the_boundaries(self)
    }
}

impl<Balance> From<Bounds<Balance>> for Option<Balance> {
    fn from(bounds: Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
            Bounds::RangeFromDesiredToMin(a, _) => Some(a),
            _ => None,
        }
    }
}

impl<'a, Balance> From<&'a Bounds<Balance>> for Option<&'a Balance> {
    fn from(bounds: &'a Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
            Bounds::RangeFromDesiredToMin(a, _) => Some(a),
            _ => None,
        }
    }
}
