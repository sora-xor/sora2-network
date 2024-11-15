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

pub mod remove_vxor_remint {
    use core::marker::PhantomData;
    use frame_support::dispatch::Weight;
    use frame_support::pallet_prelude::ValueQuery;
    use frame_support::traits::OnRuntimeUpgrade;

    use crate::*;

    #[frame_support::storage_alias]
    pub type XorToBuyBack<T: Config> = StorageValue<Pallet<T>, Balance, ValueQuery>;

    pub struct Migrate<T>(PhantomData<T>);

    impl<T> OnRuntimeUpgrade for Migrate<T>
    where
        T: Config,
    {
        fn on_runtime_upgrade() -> Weight {
            XorToBuyBack::<T>::kill();
            T::DbWeight::get().writes(1)
        }
    }
}

#[cfg(feature = "wip")] // Dynamic fee
pub mod v2 {
    use common::balance;
    use core::marker::PhantomData;
    use frame_support::dispatch::Weight;
    use frame_support::traits::OnRuntimeUpgrade;
    use frame_support::{log::info, traits::StorageVersion};

    use crate::*;

    pub struct Migrate<T>(PhantomData<T>);

    impl<T> OnRuntimeUpgrade for Migrate<T>
    where
        T: Config,
    {
        fn on_runtime_upgrade() -> Weight {
            let period =
                <T as frame_system::Config>::BlockNumber::try_from(3600_u32).unwrap_or_default();
            let small_reference_amount = balance!(0.2);
            if StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1) {
                // 1 read
                <UpdatePeriod<T>>::put(period); // 1 write
                info!("Update period initialized as {:?}", period);
                <SmallReferenceAmount<T>>::put(small_reference_amount); // 1 write
                info!(
                    "Small reference amount initialized as {:?}",
                    small_reference_amount
                );
                StorageVersion::new(2).put::<Pallet<T>>();
                return T::DbWeight::get().reads_writes(1, 2);
            }
            Weight::default()
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
            let period =
                <T as frame_system::Config>::BlockNumber::try_from(3600_u32).unwrap_or_default();
            let small_reference_amount = balance!(0.2);
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "Wrong storage version after upgrade"
            );
            frame_support::ensure!(
                <UpdatePeriod<T>>::get() == period,
                "Did not set right next update block"
            );
            frame_support::ensure!(
                <SmallReferenceAmount<T>>::get() == small_reference_amount,
                "Did not set right small reference amount"
            );
            Ok(())
        }
    }
}
