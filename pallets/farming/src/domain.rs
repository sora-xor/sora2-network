use super::*;

/// Farm container with parameters and information about tokens and owner.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct Farm<T: Trait> {
    pub owner_id: T::AccountId,
    pub technical_account_id: T::AccountId,
    pub parameters: Parameters<T>,
}

impl<T: Trait> Farm<T> {
    pub fn new(
        owner_id: &T::AccountId,
        technical_account_id: &T::AccountId,
        parameters: Parameters<T>,
    ) -> Self {
        Farm {
            owner_id: owner_id.clone(),
            technical_account_id: technical_account_id.clone(),
            parameters,
        }
    }

    pub fn is_open(&self, time: T::Moment) -> bool {
        self.parameters.period.is_in(time)
    }
}

/// `Farm` related parameters.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug)]
pub struct Parameters<T: Trait> {
    pub period: DateTimePeriod<T>,
    pub incentive: Incentive<T>,
    pub tokens_lock_period_ms: u128,
    pub vesting: Vesting,
    pub distribution_model: DistributionModel,
}

impl<T: Trait> Parameters<T> {
    pub fn new(period: DateTimePeriod<T>, incentive: Incentive<T>) -> Self {
        Parameters {
            period,
            incentive,
            ..Default::default()
        }
    }
}

#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug)]
pub struct DateTimePeriod<T: Trait> {
    start_date_time_ms: T::Moment,
    end_date_time_ms: T::Moment,
}

impl<T: Trait> DateTimePeriod<T> {
    pub fn new(start_date_time_ms: T::Moment, end_date_time_ms: T::Moment) -> Self {
        DateTimePeriod {
            start_date_time_ms,
            end_date_time_ms,
        }
    }

    pub fn is_in(&self, time: T::Moment) -> bool {
        (time <= self.end_date_time_ms) && (time >= self.start_date_time_ms)
    }
}

#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug)]
pub struct Incentive<T: Trait> {
    pub asset_id: T::AssetId,
    pub lock_period_ms: u128,
    pub amount: Balance,
}

impl<T: Trait> Incentive<T> {
    pub fn new(asset_id: T::AssetId, amount: Balance) -> Self {
        Incentive {
            amount,
            asset_id,
            ..Default::default()
        }
    }
}

/// Vesting enumeration with additinal parameters for `Enabled` variant.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum Vesting {
    /// When enabled, additional parameter acceleration (% per day) should be provided.
    Enabled(u32),
    Disabled,
}

impl Default for Vesting {
    fn default() -> Self {
        Vesting::Disabled
    }
}

/// All possible variants of supported Distribution Models.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum DistributionModel {
    Linear,
}

impl Default for DistributionModel {
    fn default() -> Self {
        DistributionModel::Linear
    }
}

/// Farmer representation.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct Farmer<T: Trait> {
    pub technical_account_id: T::AccountId,
    pub amount: Balance,
}

impl<T: Trait> Farmer<T> {
    pub fn new(technical_account_id: &T::AccountId, amount: Balance) -> Self {
        Farmer {
            technical_account_id: technical_account_id.clone(),
            amount,
        }
    }
}
