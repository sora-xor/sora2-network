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
use frame_support::ensure;
use scale_info::TypeInfo;
use sp_core::{Get, RuntimeDebug};
use sp_runtime::traits::{AtLeast32Bit, Zero};
use sp_runtime::DispatchError;

pub trait VestingSchedule<BlockNumber, Balance, AssetId: Copy> {
    /// Returns the end of all periods, `None` if calculation overflows.
    fn end(&self) -> Option<BlockNumber>;
    /// Returns all locked amount, `None` if calculation overflows.
    fn total_amount(&self) -> Option<Balance>;
    /// Returns locked amount for a given `time`.
    fn locked_amount(&self, time: BlockNumber) -> Option<Balance>;
    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError>;
    /// Returns asset id, need to get from enum
    fn asset_id(&self) -> AssetId;
    /// Returns next block for a given `time`, where asset may be unlocked and claimed
    fn next_claim_block<T: Config>(
        &self,
        time: BlockNumber,
    ) -> Result<Option<BlockNumber>, DispatchError>;
    /// Count of claims per Vesting
    fn claims_count(&self) -> u32;
}

#[allow(unused)]
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum VestingScheduleVariant<BlockNumber, AssetId: Copy, AccountId> {
    LinearVestingSchedule(LinearVestingSchedule<BlockNumber, AssetId>),
    LinearPendingVestingSchedule(LinearPendingVestingSchedule<BlockNumber, AssetId, AccountId>),
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId: Copy, AccountId>
    VestingSchedule<BlockNumber, Balance, AssetId>
    for VestingScheduleVariant<BlockNumber, AssetId, AccountId>
{
    fn end(&self) -> Option<BlockNumber> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.end(),
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => variant.end(),
        }
    }

    fn total_amount(&self) -> Option<Balance> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.total_amount(),
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => variant.total_amount(),
        }
    }

    fn locked_amount(&self, time: BlockNumber) -> Option<Balance> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.locked_amount(time),
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => {
                variant.locked_amount(time)
            }
        }
    }

    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => {
                variant.ensure_valid_vesting_schedule::<T>()
            }
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => {
                variant.ensure_valid_vesting_schedule::<T>()
            }
        }
    }

    fn asset_id(&self) -> AssetId {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.asset_id(),
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => variant.asset_id(),
        }
    }

    fn next_claim_block<T: Config>(
        &self,
        time: BlockNumber,
    ) -> Result<Option<BlockNumber>, DispatchError> {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => {
                variant.next_claim_block::<T>(time)
            }
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => {
                variant.next_claim_block::<T>(time)
            }
        }
    }

    fn claims_count(&self) -> u32 {
        match self {
            VestingScheduleVariant::LinearVestingSchedule(variant) => variant.claims_count(),
            VestingScheduleVariant::LinearPendingVestingSchedule(variant) => variant.claims_count(),
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
    /// Amount of remainder tokens to release per last period
    #[codec(compact)]
    pub remainder_amount: Balance,
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId: Copy> LinearVestingSchedule<BlockNumber, AssetId> {
    fn amount_with_remainder(&self, periods: u32) -> Option<Balance> {
        if periods.is_zero() {
            return Some(Balance::zero());
        }
        if self.remainder_amount.is_zero() {
            self.per_period.checked_mul(periods.into())
        } else {
            self.per_period
                .checked_mul(periods.saturating_sub(1).into())?
                .checked_add(self.remainder_amount)
        }
    }
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
        self.amount_with_remainder(self.period_count)
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
        self.amount_with_remainder(unrealized)
    }

    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError> {
        ensure!(!self.period.is_zero(), Error::<T>::ZeroVestingPeriod);
        ensure!(
            !self.period_count.is_zero(),
            Error::<T>::WrongVestingPeriodCount
        );
        ensure!(
            self.period_count > 1 || self.remainder_amount.is_zero(),
            Error::<T>::WrongVestingPeriodCount
        );
        ensure!(self.end().is_some(), Error::<T>::ArithmeticError);

        let total = self.total_amount().ok_or(Error::<T>::ArithmeticError)?;

        ensure!(total >= T::MinVestedTransfer::get(), Error::<T>::AmountLow);

        Ok(total)
    }

    fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    fn next_claim_block<T: Config>(
        &self,
        time: BlockNumber,
    ) -> Result<Option<BlockNumber>, DispatchError> {
        // blocks_to_next = start + ((time - start + period) / period) * period
        if self
            .locked_amount(time)
            .ok_or(Error::<T>::ArithmeticError)?
            .is_zero()
        {
            Ok(None)
        } else {
            let to_next_period_count = time
                .saturating_sub(self.start)
                .saturating_add(self.period)
                .checked_div(&self.period)
                .ok_or(Error::<T>::ArithmeticError)?;
            if to_next_period_count <= self.period_count.into() {
                Ok(Some(
                    to_next_period_count
                        .checked_mul(&self.period)
                        .ok_or(Error::<T>::ArithmeticError)?
                        .checked_add(&self.start)
                        .ok_or(Error::<T>::ArithmeticError)?,
                ))
            } else {
                Ok(None)
            }
        }
    }

    fn claims_count(&self) -> u32 {
        self.period_count
    }
}

/// The vesting schedule.
///
/// Benefits would be granted gradually, `per_period` amount every `period`
/// of blocks after `start`.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct LinearPendingVestingSchedule<BlockNumber, AssetId: Copy, AccountId> {
    /// Vesting asset id
    pub asset_id: AssetId,
    /// Account Id of the manager, who may stop pending
    pub manager_id: Option<AccountId>,
    /// Vesting starting block
    pub start: Option<BlockNumber>,
    /// Number of blocks between vest
    pub period: BlockNumber,
    /// Number of vest
    pub period_count: u32,
    /// Amount of tokens to release per vest
    #[codec(compact)]
    pub per_period: Balance,
    /// Amount of remainder tokens to release per last period
    #[codec(compact)]
    pub remainder_amount: Balance,
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId: Copy, AccountId>
    LinearPendingVestingSchedule<BlockNumber, AssetId, AccountId>
{
    fn amount_with_remainder(&self, periods: u32) -> Option<Balance> {
        if periods.is_zero() {
            return Some(Balance::zero());
        }
        if self.remainder_amount.is_zero() {
            self.per_period.checked_mul(periods.into())
        } else {
            self.per_period
                .checked_mul(periods.saturating_sub(1).into())?
                .checked_add(self.remainder_amount)
        }
    }
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId: Copy, AccountId>
    VestingSchedule<BlockNumber, Balance, AssetId>
    for LinearPendingVestingSchedule<BlockNumber, AssetId, AccountId>
{
    fn end(&self) -> Option<BlockNumber> {
        // period * period_count + start
        self.period
            .checked_mul(&self.period_count.into())?
            .checked_add(&self.start?)
    }

    fn total_amount(&self) -> Option<Balance> {
        self.amount_with_remainder(self.period_count)
    }

    /// Note this func assumes schedule is a valid one(non-zero period and
    /// non-overflow total amount), and it should be guaranteed by callers.
    fn locked_amount(&self, time: BlockNumber) -> Option<Balance> {
        if let Some(start) = self.start {
            // full = (time - start) / period
            // unrealized = period_count - full
            // per_period * unrealized
            let full = time.saturating_sub(start).checked_div(&self.period)?;
            let unrealized = self
                .period_count
                .saturating_sub(full.unique_saturated_into());
            self.amount_with_remainder(unrealized)
        } else {
            self.amount_with_remainder(self.period_count)
        }
    }

    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError> {
        ensure!(!self.period.is_zero(), Error::<T>::ZeroVestingPeriod);
        ensure!(
            !self.period_count.is_zero(),
            Error::<T>::WrongVestingPeriodCount
        );
        ensure!(
            self.period_count > 1 || self.remainder_amount.is_zero(),
            Error::<T>::WrongVestingPeriodCount
        );
        if self.start.is_some() {
            ensure!(self.end().is_some(), Error::<T>::ArithmeticError);
        }

        let total = self.total_amount().ok_or(Error::<T>::ArithmeticError)?;

        ensure!(total >= T::MinVestedTransfer::get(), Error::<T>::AmountLow);

        Ok(total)
    }

    fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    fn next_claim_block<T: Config>(
        &self,
        time: BlockNumber,
    ) -> Result<Option<BlockNumber>, DispatchError> {
        // blocks_to_next = start + ((time - start + period) / period) * period
        // Check start for None
        if self
            .locked_amount(time)
            .ok_or(Error::<T>::ArithmeticError)?
            .is_zero()
            || self.start.is_none()
        {
            return Ok(None);
        } else {
            let to_next_period_count = time
                .saturating_sub(self.start.unwrap())
                .saturating_add(self.period)
                .checked_div(&self.period)
                .ok_or(Error::<T>::ArithmeticError)?;
            if to_next_period_count <= self.period_count.into() {
                Ok(Some(
                    to_next_period_count
                        .checked_mul(&self.period)
                        .ok_or(Error::<T>::ArithmeticError)?
                        .checked_add(&self.start.unwrap())
                        .ok_or(Error::<T>::ArithmeticError)?,
                ))
            } else {
                Ok(None)
            }
        }
    }

    fn claims_count(&self) -> u32 {
        self.period_count
    }
}
