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

use crate::treasury::Treasury;
use crate::{Config, MomentOf};
use codec::{Decode, Encode, MaxEncodedLen};
use common::{AccountIdOf, Balance, BoundedString};
use frame_support::traits::Time;
use sp_core::RuntimeDebug;
use sp_runtime::{DispatchError, DispatchResult};

#[derive(
    RuntimeDebug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen,
)]
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

#[derive(
    RuntimeDebug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen,
)]
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

#[derive(RuntimeDebug, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct DepositRequest<T: Config> {
    pub owner: AccountIdOf<T>,
    pub time: MomentOf<T>,
    pub amount: Balance,
    pub payment_reference: BoundedString<T::MaxRequestPaymentReferenceSize>,
    pub details: Option<BoundedString<T::MaxRequestDetailsSize>>,
    pub status: RequestStatus<T>,
}

impl<T: Config> Clone for DepositRequest<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            time: self.time,
            amount: self.amount,
            payment_reference: self.payment_reference.clone(),
            details: self.details.clone(),
            status: self.status.clone(),
        }
    }
}

impl<T: Config> PartialEq for DepositRequest<T> {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.time == other.time
            && self.amount == other.amount
            && self.payment_reference == other.payment_reference
            && self.details == other.details
            && self.status == other.status
    }
}

impl<T: Config> Eq for DepositRequest<T> {}

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

#[derive(RuntimeDebug, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct WithdrawRequest<T: Config> {
    pub owner: AccountIdOf<T>,
    pub time: MomentOf<T>,
    pub amount: Balance,
    pub payment_reference: Option<BoundedString<T::MaxRequestPaymentReferenceSize>>,
    pub details: Option<BoundedString<T::MaxRequestDetailsSize>>,
    pub status: RequestStatus<T>,
}

impl<T: Config> Clone for WithdrawRequest<T> {
    fn clone(&self) -> Self {
        Self {
            owner: self.owner.clone(),
            time: self.time,
            amount: self.amount,
            payment_reference: self.payment_reference.clone(),
            details: self.details.clone(),
            status: self.status.clone(),
        }
    }
}

impl<T: Config> PartialEq for WithdrawRequest<T> {
    fn eq(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.time == other.time
            && self.amount == other.amount
            && self.payment_reference == other.payment_reference
            && self.details == other.details
            && self.status == other.status
    }
}

impl<T: Config> Eq for WithdrawRequest<T> {}

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
