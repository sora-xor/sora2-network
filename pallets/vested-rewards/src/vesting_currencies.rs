use crate::{Config, Error};
use codec::{Decode, Encode, MaxEncodedLen};
use common::prelude::Balance;
use frame_support::dispatch::TypeInfo;
use frame_support::{ensure, RuntimeDebug};
use sp_core::Get;
use sp_runtime::traits::{AtLeast32Bit, Zero};
use sp_runtime::DispatchError;

pub trait VestingSchedule<BlockNumber, Balance, AssetId> {
    /// Returns the end of all periods, `None` if calculation overflows.
    fn end(&self) -> Option<BlockNumber>;
    /// Returns all locked amount, `None` if calculation overflows.
    fn total_amount(&self) -> Option<Balance>;
    /// Returns locked amount for a given `time`.
    fn locked_amount(&self, time: BlockNumber) -> Balance;
    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError>;
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub enum VestingScheduleVariant<BlockNumber, AssetId> {
    LinearVestingSchedule(LinearVestingSchedule<BlockNumber, AssetId>),
}

impl<BlockNumber: AtLeast32Bit + Copy, AssetId> VestingSchedule<BlockNumber, Balance, AssetId>
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

    fn locked_amount(&self, time: BlockNumber) -> Balance {
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
}

/// The vesting schedule.
///
/// Benefits would be granted gradually, `per_period` amount every `period`
/// of blocks after `start`.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct LinearVestingSchedule<BlockNumber, AssetId> {
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

impl<BlockNumber: AtLeast32Bit + Copy, AssetId> VestingSchedule<BlockNumber, Balance, AssetId>
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
    fn locked_amount(&self, time: BlockNumber) -> Balance {
        // full = (time - start) / period
        // unrealized = period_count - full
        // per_period * unrealized
        let full = time
            .saturating_sub(self.start)
            .checked_div(&self.period)
            .expect("ensured non-zero period; qed");
        let unrealized = self
            .period_count
            .saturating_sub(full.unique_saturated_into());
        self.per_period
            .checked_mul(unrealized.into())
            .expect("ensured non-overflow total amount; qed")
    }

    fn ensure_valid_vesting_schedule<T: Config>(&self) -> Result<Balance, DispatchError> {
        ensure!(!self.period.is_zero(), Error::<T>::ZeroVestingPeriod);
        ensure!(
            !self.period_count.is_zero(),
            Error::<T>::ZeroVestingPeriodCount
        );
        ensure!(self.end().is_some(), Error::<T>::ArithmeticError);

        let total_total = self.total_amount().ok_or(Error::<T>::ArithmeticError)?;

        ensure!(
            total_total >= T::MinVestedTransfer::get(),
            Error::<T>::AmountLow
        );

        Ok(total_total)
    }
}
