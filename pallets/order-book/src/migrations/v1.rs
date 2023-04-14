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

#![cfg(feature = "wip")] // order-book

use core::marker::PhantomData;

use frame_support::{
    sp_tracing::{error, info},
    traits::{Get, GetStorageVersion, OnRuntimeUpgrade, StorageVersion},
};

use crate::Pallet;

pub struct InitializeTechnicalAccount<T>(PhantomData<T>);

impl<T: crate::Config> OnRuntimeUpgrade for InitializeTechnicalAccount<T> {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        if Pallet::<T>::on_chain_storage_version() == 0 {
            info!("Applying migration to version 1: Initialize XST pool");

            match technical::Pallet::<T>::register_tech_account_id(T::LockTechAccountId::get()) {
                Ok(()) => StorageVersion::new(1).put::<Pallet<T>>(),
                // We can't return an error here, so we just log it
                Err(err) => error!(
                    "An error occurred during technical account registration: {:?}",
                    err
                ),
            }
            <T as frame_system::Config>::BlockWeights::get().max_block
        } else {
            error!(
                "Runtime upgrade executed with wrong storage version, expected 0, got {:?}",
                Pallet::<T>::on_chain_storage_version()
            );
            <T as frame_system::Config>::DbWeight::get().reads(1)
        }
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        let account_id = T::LockTechAccountId::get();
        frame_support::ensure!(
            technical::Pallet::<T>::ensure_tech_account_registered(&account_id).is_err(),
            "Tech account is already registered"
        );
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 0,
            "must upgrade linearly"
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        let account_id = T::LockTechAccountId::get();
        frame_support::ensure!(
            technical::Pallet::<T>::ensure_tech_account_registered(&account_id).is_ok(),
            "Tech account is not registered"
        );
        frame_support::ensure!(
            Pallet::<T>::on_chain_storage_version() == 1,
            "should be upgraded to version 1"
        );
        Ok(())
    }
}
