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

use crate::*;
use core::marker::PhantomData;
use frame_support::traits::OnRuntimeUpgrade;

pub mod v2 {
    use common::AccountIdOf;
    use frame_support::traits::StorageVersion;
    use log::info;
    #[cfg(feature = "try-runtime")]
    use sp_runtime::TryRuntimeError;
    use sp_std::prelude::Vec;

    use super::*;

    pub struct Migrate<T, G>(PhantomData<(T, G)>);

    impl<T, G> OnRuntimeUpgrade for Migrate<T, G>
    where
        T: Config,
        G: Get<Vec<(AccountIdOf<T>, AccountIdOf<T>)>>,
    {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
                log::error!(
                    "Expected storage version 1, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
                return frame_support::weights::Weight::zero();
            }

            info!("Migrating PswapDistribution to v2");

            let pools = G::get();
            let weight = pools
                .iter()
                .fold(Weight::zero(), |weight_acc, (_, pool_fee_account)| {
                    // using this instead of unsubscribe function, since it can return errors
                    SubscribedAccounts::<T>::remove(pool_fee_account);
                    frame_system::Pallet::<T>::dec_consumers(&pool_fee_account);
                    weight_acc.saturating_add(T::DbWeight::get().reads_writes(0, 2))
                });

            StorageVersion::new(2).put::<Pallet<T>>();
            weight.saturating_add(T::DbWeight::get().reads_writes(0, 1))
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                TryRuntimeError::Other("Wrong storage version before upgrade")
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), TryRuntimeError> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                TryRuntimeError::Other("Wrong storage version after upgrade")
            );
            let pools = &G::get();

            for (_, pool_account) in pools {
                frame_support::ensure!(
                    !SubscribedAccounts::<T>::contains_key(pool_account),
                    "Synthetic pools still referenced in SubscribedAccounts storage map in PswapDistribution pallet"
                );
            }
            Ok(())
        }
    }
}
