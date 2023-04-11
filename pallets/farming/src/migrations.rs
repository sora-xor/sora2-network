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
    use frame_support::traits::StorageVersion;

    use super::*;

    // You need to provide list of pools with creation block number
    pub struct Migrate<T, G>(PhantomData<(T, G)>);

    impl<T, G> OnRuntimeUpgrade for Migrate<T, G>
    where
        T: Config,
        G: Get<Vec<(T::AccountId, T::BlockNumber)>>,
    {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
                frame_support::log::error!(
                    "Expected storage version 1, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
            }
            let pools = G::get();
            for (pool_account, block) in pools {
                Pools::<T>::mutate(block % T::REFRESH_FREQUENCY, |pools| {
                    if !pools.contains(&pool_account) {
                        frame_support::log::info!(
                            "Add pool {pool_account:?} at block {block:?} to farming"
                        );
                        pools.push(pool_account);
                    } else {
                        frame_support::log::info!(
                            "Skip {pool_account:?} at block {block:?}, already exist"
                        );
                    }
                });
            }
            StorageVersion::new(2).put::<Pallet<T>>();
            <T as frame_system::Config>::BlockWeights::get().max_block
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "Wrong storage version before upgrade"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "Wrong storage version after upgrade"
            );
            Ok(())
        }
    }
}
