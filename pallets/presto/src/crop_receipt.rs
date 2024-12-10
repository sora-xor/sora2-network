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

use crate::{Config, Error, MomentOf};

use codec::{Decode, Encode, MaxEncodedLen};
use common::{AccountIdOf, Balance, BoundedString};
use derivative::Derivative;
use frame_support::ensure;
use frame_support::traits::Time;
use sp_runtime::DispatchResult;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum Status {
    Rating,
    Decision,
    Declined,
    Published,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum Rating {
    AAA,
    AA,
    A,
    BBB,
    BB,
    B,
    CCC,
    CC,
    C,
    D,
    NR,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum Country {
    Brazil,
    Indonesia,
    Nigeria,
    Ukraine,
    Usa,
    Other,
}

#[allow(unused)] // TODO remove
impl Country {
    pub fn symbol(&self) -> &[u8] {
        match self {
            Self::Brazil => b"BR",
            Self::Indonesia => b"ID",
            Self::Nigeria => b"NG",
            Self::Ukraine => b"UA",
            Self::Usa => b"US",
            Self::Other => b"RR",
        }
    }

    pub fn name(&self) -> &[u8] {
        match self {
            Self::Brazil => b"Brazil",
            Self::Indonesia => b"Indonesia",
            Self::Nigeria => b"Nigeria",
            Self::Ukraine => b"Ukraine",
            Self::Usa => b"USA",
            Self::Other => b"Other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(T))]
pub struct Score<T: Config> {
    pub rating: Rating,
    pub by_auditor: AccountIdOf<T>,
}

#[derive(Debug, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen, Derivative)]
#[derivative(Clone, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub struct CropReceiptContent<T: Config> {
    pub json: BoundedString<T::MaxCropReceiptContentSize>,
}

#[derive(Debug, Encode, Decode, scale_info::TypeInfo, MaxEncodedLen, Derivative)]
#[derivative(Clone, PartialEq, Eq)]
#[scale_info(skip_type_params(T))]
pub struct CropReceipt<T: Config> {
    pub owner: AccountIdOf<T>,
    pub time: MomentOf<T>,
    pub status: Status,
    pub amount: Balance,
    pub country: Country,
    pub score: Option<Score<T>>,
    pub close_initial_period: MomentOf<T>,
    pub date_of_issue: MomentOf<T>,
    pub place_of_issue: BoundedString<T::MaxPlaceOfIssueSize>,
    pub debtor: BoundedString<T::MaxDebtorSize>,
    pub creditor: BoundedString<T::MaxCreditorSize>,
    pub perfomance_time: MomentOf<T>,
}

impl<T: Config> CropReceipt<T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: AccountIdOf<T>,
        amount: Balance,
        country: Country,
        close_initial_period: MomentOf<T>,
        date_of_issue: MomentOf<T>,
        place_of_issue: BoundedString<T::MaxPlaceOfIssueSize>,
        debtor: BoundedString<T::MaxDebtorSize>,
        creditor: BoundedString<T::MaxCreditorSize>,
        perfomance_time: MomentOf<T>,
    ) -> Self {
        let time = T::Time::now();

        Self {
            owner,
            time,
            status: Status::Rating,
            amount,
            country,
            score: None,
            close_initial_period,
            date_of_issue,
            place_of_issue,
            debtor,
            creditor,
            perfomance_time,
        }
    }

    pub fn ensure_is_owner(&self, who: &AccountIdOf<T>) -> DispatchResult {
        ensure!(&self.owner == who, Error::<T>::CallerIsNotCropReceiptOwner);
        Ok(())
    }

    pub fn rate(&mut self, rating: Rating, auditor: AccountIdOf<T>) -> DispatchResult {
        ensure!(
            self.status == Status::Rating,
            Error::<T>::CropReceiptAlreadyRated
        );

        self.score = Some(Score {
            rating,
            by_auditor: auditor,
        });
        self.status = Status::Decision;

        Ok(())
    }

    pub fn decline(&mut self) -> DispatchResult {
        if self.status == Status::Rating {
            return Err(Error::<T>::CropReceiptWaitingForRate.into());
        }

        ensure!(
            self.status == Status::Decision,
            Error::<T>::CropReceiptAlreadyHasDecision
        );

        self.status = Status::Declined;

        Ok(())
    }

    pub fn publish(&mut self) -> DispatchResult {
        if self.status == Status::Rating {
            return Err(Error::<T>::CropReceiptWaitingForRate.into());
        }

        ensure!(
            self.status == Status::Decision,
            Error::<T>::CropReceiptAlreadyHasDecision
        );

        self.status = Status::Published;

        Ok(())
    }
}
