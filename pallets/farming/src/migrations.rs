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
        G: Get<Vec<(T::AccountId, BlockNumberFor<T>)>>,
    {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
                log::error!(
                    "Expected storage version 1, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
            }
            let pools = G::get();
            for (pool_account, block) in pools {
                Pools::<T>::mutate(block % T::REFRESH_FREQUENCY, |pools| {
                    if !pools.contains(&pool_account) {
                        log::info!("Add pool {pool_account:?} at block {block:?} to farming");
                        pools.push(pool_account);
                    } else {
                        log::info!("Skip {pool_account:?} at block {block:?}, already exist");
                    }
                });
            }
            StorageVersion::new(2).put::<Pallet<T>>();
            <T as frame_system::Config>::BlockWeights::get().max_block
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "Wrong storage version before upgrade"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), DispatchError> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "Wrong storage version after upgrade"
            );
            Ok(())
        }
    }
}

pub mod v3 {
    use frame_support::traits::StorageVersion;
    use log::info;

    use super::*;

    pub struct Migrate<T, P, B>(PhantomData<(T, P, B)>);

    impl<T, P, B> OnRuntimeUpgrade for Migrate<T, P, B>
    where
        T: Config,
        P: Get<Vec<(T::AccountId, T::AccountId)>>,
        B: Get<Vec<BlockNumberFor<T>>>,
    {
        fn on_runtime_upgrade() -> frame_support::weights::Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(2) {
                log::error!(
                    "Expected storage version 2, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
                return Weight::zero();
            }

            info!("Migrating Farming to v3");

            let pools = P::get();
            let blocks = B::get();

            let pools_weight = blocks
                .iter()
                .fold(Weight::zero(), |weight_acc, block_number| {
                    Pools::<T>::mutate_exists(block_number, |pool_accounts| {
                        if let Some(pool_accounts_vec) = pool_accounts {
                            pools.iter().for_each(|(pool_account, _)| {
                                pool_accounts_vec.retain(|account| account == pool_account)
                            });
                            if pool_accounts_vec.is_empty() {
                                *pool_accounts = None
                            };
                        }
                    });
                    weight_acc.saturating_add(T::DbWeight::get().reads_writes(1, 1))
                });

            let pool_farmers_weight =
                pools
                    .iter()
                    .fold(Weight::zero(), |weight_acc, (pool_account, _)| {
                        PoolFarmers::<T>::remove(pool_account);
                        weight_acc.saturating_add(T::DbWeight::get().reads_writes(0, 1))
                    });

            StorageVersion::new(2).put::<Pallet<T>>();
            pools_weight
                .saturating_add(pool_farmers_weight)
                .saturating_add(T::DbWeight::get().reads_writes(0, 1))
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "Wrong storage version before upgrade"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), DispatchError> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(3),
                "Wrong storage version after upgrade"
            );
            let pools = &P::get();
            let blocks = B::get();

            for block_number in blocks {
                let pool_accounts_in_storage = Pools::<T>::get(block_number);
                for (pool_account, _) in pools {
                    frame_support::ensure!(
                        !pool_accounts_in_storage.contains(pool_account),
                        "Synthetic pools still referenced in Pools storage in Farming pallet"
                    );
                }
            }

            for (pool_account, _) in pools {
                frame_support::ensure!(
                    !PoolFarmers::<T>::contains_key(pool_account),
                    "Synthetic pools still referenced in PoolFarmers storage in Farming pallet"
                );
            }
            Ok(())
        }
    }
}
