use crate::treasury::Treasury;
use crate::{Config, MomentOf};
use codec::{Decode, Encode, MaxEncodedLen};
use common::{AccountIdOf, Balance, BoundedString};
use frame_support::traits::Time;
use sp_runtime::{DispatchError, DispatchResult};

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum RequestStatus<T: Config> {
    Pending,
    Cancelled,
    Approved {
        by: AccountIdOf<T>,
        time: MomentOf<T>,
    },
    Declined {
        by: AccountIdOf<T>,
        time: MomentOf<T>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub enum Request<T: Config> {
    Deposit(DepositRequest<T>),
    Withdraw(WithdrawRequest<T>),
}

impl<T: Config> Request<T> {
    pub fn owner(&self) -> &AccountIdOf<T> {
        match self {
            Self::Deposit(request) => &request.owner,
            Self::Withdraw(request) => &request.owner,
        }
    }

    pub fn status(&self) -> &RequestStatus<T> {
        match self {
            Self::Deposit(request) => &request.status,
            Self::Withdraw(request) => &request.status,
        }
    }

    pub fn decline(&mut self, manager: AccountIdOf<T>) -> DispatchResult {
        match self {
            Self::Deposit(request) => request.decline(manager),
            Self::Withdraw(request) => request.decline(manager)?,
        }

        Ok(())
    }

    pub fn cancel(&mut self) -> DispatchResult {
        match self {
            Self::Deposit(request) => request.cancel(),
            Self::Withdraw(request) => request.cancel()?,
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct DepositRequest<T: Config> {
    pub owner: AccountIdOf<T>,
    pub time: MomentOf<T>,
    pub amount: Balance,
    pub payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
    pub details: Option<BoundedString<T::MaxRequestDetailsSize>>,
    pub status: RequestStatus<T>,
}

impl<T: Config> DepositRequest<T> {
    pub fn new(
        owner: AccountIdOf<T>,
        amount: Balance,
        payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
        details: Option<BoundedString<T::MaxRequestDetailsSize>>,
    ) -> Self {
        let time = T::Time::now();

        Self {
            owner,
            time,
            amount,
            payment_reference,
            details,
            status: RequestStatus::Pending,
        }
    }

    pub fn approve(&mut self, manager: AccountIdOf<T>) -> DispatchResult {
        Treasury::<T>::send_presto_usd(self.amount, &self.owner)?;

        let time = T::Time::now();
        self.status = RequestStatus::Approved { by: manager, time };

        Ok(())
    }

    pub fn decline(&mut self, manager: AccountIdOf<T>) {
        let time = T::Time::now();
        self.status = RequestStatus::Declined { by: manager, time };
    }

    pub fn cancel(&mut self) {
        self.status = RequestStatus::Cancelled;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct WithdrawRequest<T: Config> {
    pub owner: AccountIdOf<T>,
    pub time: MomentOf<T>,
    pub amount: Balance,
    pub payment_reference: Option<BoundedString<T::MaxRequestPaymentReferenceSize>>,
    pub details: Option<BoundedString<T::MaxRequestDetailsSize>>,
    pub status: RequestStatus<T>,
}

impl<T: Config> WithdrawRequest<T> {
    pub fn new(
        owner: AccountIdOf<T>,
        amount: Balance,
        details: Option<BoundedString<T::MaxRequestDetailsSize>>,
    ) -> Result<Self, DispatchError> {
        Treasury::<T>::collect_to_buffer(amount, &owner)?;

        let time = T::Time::now();
        Ok(Self {
            owner,
            time,
            amount,
            payment_reference: None,
            details,
            status: RequestStatus::Pending,
        })
    }

    pub fn approve(
        &mut self,
        manager: AccountIdOf<T>,
        payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
    ) -> DispatchResult {
        Treasury::<T>::transfer_from_buffer_to_main(self.amount)?;

        let time = T::Time::now();
        self.payment_reference = Some(payment_reference);
        self.status = RequestStatus::Approved { by: manager, time };

        Ok(())
    }

    pub fn decline(&mut self, manager: AccountIdOf<T>) -> DispatchResult {
        Treasury::<T>::return_from_buffer(self.amount, &self.owner)?;

        let time = T::Time::now();
        self.status = RequestStatus::Declined { by: manager, time };

        Ok(())
    }

    pub fn cancel(&mut self) -> DispatchResult {
        Treasury::<T>::return_from_buffer(self.amount, &self.owner)?;

        self.status = RequestStatus::Cancelled;

        Ok(())
    }
}
