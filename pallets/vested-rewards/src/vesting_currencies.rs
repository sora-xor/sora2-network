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

use crate::{Config, Error};
use codec::{Decode, Encode, MaxEncodedLen};
use common::prelude::Balance;
use frame_support::dispatch::TypeInfo;
use frame_support::{ensure, RuntimeDebug};
use sp_core::Get;
use sp_runtime::traits::{AtLeast32Bit, Zero};
use sp_runtime::DispatchError;

pub trait VestingSchedule<BlockNumber, Balance, AssetId: Copy> {
    /// Returns the end of all periods, `None` if calculation overflows.
    fn end(&self) -> Option<BlockNumber>;
    /// Returns all locked amount, `None` if calculation overflows.
    fn total_amount(&self) -> Option<Balance>;
    /// Returns locked amount for a given `time`.
    fn locked_amount(&self, time: BlockNumber) -> Option<common::Balance>;
    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError>;
    /// Returns asset id, need to get from enum
    fn asset_id(&self) -> AssetId;
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum VestingScheduleVariant<BlockNumber, AssetId: Copy> {
    LinearVestingSchedule(LinearVestingSchedule<BlockNumber, AssetId>),
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId: Copy> VestingSchedule<BlockNumber, Balance, AssetId>
    for VestingScheduleVariant<BlockNumber, AssetId>
{
    fn end(&self) -> Option<BlockNumber> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.end(),
        }
    }

    fn total_amount(&self) -> Option<Balance> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.total_amount(),
        }
    }

    fn locked_amount(&self, time: BlockNumber) -> Option<Balance> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.locked_amount(time),
        }
    }

    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => {
                variant.ensure_valid_vesting_schedule::<T>()
            }
        }
    }

    fn asset_id(&self) -> AssetId {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.asset_id(),
        }
    }
}

/// The vesting schedule.
///
/// Benefits would be granted gradually, `per_period` amount every `period`
/// of blocks after `start`.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct LinearVestingSchedule<BlockNumber, AssetId: Copy> {
    /// Vesting asset id
    pub asset_id: AssetId,
    /// Vesting starting block
    pub start: BlockNumber,
    /// Number of blocks between vest
    pub period: BlockNumber,
    /// Number of vest
    pub period_count: u32,
    /// Amount of tokens to release per vest
    #[codec(compact)]
    pub per_period: Balance,
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId: Copy> VestingSchedule<BlockNumber, Balance, AssetId>
    for LinearVestingSchedule<BlockNumber, AssetId>
{
    fn end(&self) -> Option<BlockNumber> {
        // period * period_count + start
        self.period
            .checked_mul(&self.period_count.into())?
            .checked_add(&self.start)
    }

    fn total_amount(&self) -> Option<Balance> {
        self.per_period.checked_mul(self.period_count.into())
    }

    /// Note this func assumes schedule is a valid one(non-zero period and
    /// non-overflow total amount), and it should be guaranteed by callers.
    fn locked_amount(&self, time: BlockNumber) -> Option<Balance> {
        // full = (time - start) / period
        // unrealized = period_count - full
        // per_period * unrealized
        let full = time.saturating_sub(self.start).checked_div(&self.period)?;
        let unrealized = self
            .period_count
            .saturating_sub(full.unique_saturated_into());
        self.per_period.checked_mul(unrealized.into())
    }

    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError> {
        ensure!(!self.period.is_zero(), Error::<T>::ZeroVestingPeriod);
        ensure!(
            !self.period_count.is_zero(),
            Error::<T>::ZeroVestingPeriodCount
        );
        ensure!(self.end().is_some(), Error::<T>::ArithmeticError);

        let total = self.total_amount().ok_or(Error::<T>::ArithmeticError)?;

        ensure!(total >= T::MinVestedTransfer::get(), Error::<T>::AmountLow);

        Ok(total)
    }

    fn asset_id(&self) -> AssetId {
        self.asset_id
    }
}
