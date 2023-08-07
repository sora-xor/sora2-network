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

pub mod v1 {
    use crate::pallet::{Config, Pallet};
    use codec::Decode;
    use common::{fixed, Balance, Fixed};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{OnRuntimeUpgrade, StorageInstance};
    use frame_support::weights::Weight;

    #[cfg(feature = "try-runtime")]
    use sp_std::prelude::Vec;

    // use crate::SymbolRates;

    #[derive(Decode, Encode, Clone, RuntimeDebug)]
    pub struct BandRateV0 {
        pub value: Balance,
        pub last_updated: u64,
        pub request_id: u64,
    }

    #[derive(Decode, Encode, RuntimeDebug)]
    pub struct BandRateV1 {
        pub value: Balance,
        pub last_updated: u64,
        pub request_id: u64,
        pub dynamic_fee: Fixed,
    }

    #[frame_support::storage_alias]
    pub type SymbolRates<T: Config> = StorageMap<
        Pallet<T>,
        Blake2_128Concat,
        <T as Config>::Symbol,
        Option<BandRateV1>,
        ValueQuery,
    >;

    // used for testing migration
    pub struct SymbolRatesV0StorageInstance<T: Config>(PhantomData<T>);

    impl<T: Config> StorageInstance for SymbolRatesV0StorageInstance<T> {
        fn pallet_prefix() -> &'static str {
            "Band"
        }
        const STORAGE_PREFIX: &'static str = "SymbolRates";
    }
    pub type SymbolRatesV0<T> = StorageMap<
        SymbolRatesV0StorageInstance<T>,
        Blake2_128Concat,
        <T as Config>::Symbol,
        Option<BandRateV0>,
        ValueQuery,
    >;

    pub struct BandUpdateV1<T>(core::marker::PhantomData<T>);

    /// Migration which migrates `XSTUSD` synthetic to the new format.
    impl<T> OnRuntimeUpgrade for BandUpdateV1<T>
    where
        T: Config,
    {
        fn on_runtime_upgrade() -> Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(0) {
                frame_support::log::error!(
                    "Expected storage version 0, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
            }
            let mut weight = Weight::zero();

            SymbolRates::<T>::translate::<Option<BandRateV0>, _>(|symbol, band_rate| {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
                match band_rate {
                    Some(band_rate) => Some(Some(BandRateV1 {
                        value: band_rate.value,
                        last_updated: band_rate.last_updated,
                        request_id: band_rate.request_id,
                        dynamic_fee: fixed!(0),
                    })),
                    None => None,
                }
            });

            StorageVersion::new(1).put::<Pallet<T>>();
            weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(0),
                "must upgrade linearly"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "should be upgraded to version 1"
            );
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{BandRateV0, SymbolRates as SymbolRatesV1, SymbolRatesV0};
        use crate::{mock::*, pallet::*};
        use common::fixed;
        use frame_support::traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion};
        #[test]
        fn test() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(0).put::<Pallet<Runtime>>();

                let sample_rate = BandRateV0 {
                    value: 0,
                    last_updated: 0,
                    request_id: 0,
                };
                let rates_vec = vec!["USD", "RUB"];
                rates_vec.iter().cloned().for_each(|symbol| {
                    SymbolRatesV0::<Runtime>::insert(symbol, Some(sample_rate.clone()));
                });

                super::BandUpdateV1::<Runtime>::on_runtime_upgrade();

                for symbol in rates_vec.into_iter() {
                    let dyn_fee_from_storage_map = SymbolRatesV1::<Runtime>::get(symbol)
                        .expect("Expected to get entry from SymbolRatesV1")
                        .dynamic_fee;
                    println!("dyn_fee_from_storage_map: {:?}", dyn_fee_from_storage_map);
                    let dyn_fee = Pallet::<Runtime>::rates(symbol)
                        .expect("Expected to get rate for the specified symbol")
                        .dynamic_fee;
                    assert_eq!(dyn_fee, fixed!(0));
                }
                assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 1);
            });
        }
    }
}

pub mod v2 {
    use crate::{
        BandRate, {Config, Pallet},
    };
    use codec::Decode;
    use common::{fixed, Balance, Fixed};
    use frame_support::pallet_prelude::*;
    use frame_support::traits::{OnRuntimeUpgrade, PalletInfoAccess, StorageInstance};
    use frame_support::weights::Weight;
    use sp_std::marker::PhantomData;

    use frame_system::pallet_prelude::BlockNumberFor;
    #[cfg(feature = "try-runtime")]
    use sp_std::prelude::Vec;

    use crate::SymbolCheckBlock;

    #[derive(Decode, Encode, Clone)]
    pub struct BandRateV1 {
        pub value: Balance,
        pub last_updated: u64,
        pub request_id: u64,
        pub dynamic_fee: Fixed,
    }

    #[frame_support::storage_alias]
    pub type SymbolRates<T: Config> = StorageMap<
        Pallet<T>,
        Blake2_128Concat,
        <T as Config>::Symbol,
        Option<BandRate<BlockNumberFor<T>>>,
        ValueQuery,
    >;

    // used for testing migration
    pub struct SymbolRatesV1StorageInstance<T: Config>(PhantomData<T>);

    impl<T: Config> StorageInstance for SymbolRatesV1StorageInstance<T> {
        fn pallet_prefix() -> &'static str {
            <Pallet<T> as PalletInfoAccess>::name()
        }
        const STORAGE_PREFIX: &'static str = "SymbolRates";
    }
    pub type SymbolRatesV1<T> = StorageMap<
        SymbolRatesV1StorageInstance<T>,
        Blake2_128Concat,
        <T as Config>::Symbol,
        Option<BandRateV1>,
        ValueQuery,
    >;

    pub struct BandUpdateV2<T>(core::marker::PhantomData<T>);

    /// Migration which migrates `XSTUSD` synthetic to the new format.
    impl<T> OnRuntimeUpgrade for BandUpdateV2<T>
    where
        T: Config,
    {
        fn on_runtime_upgrade() -> Weight {
            if StorageVersion::get::<Pallet<T>>() != StorageVersion::new(1) {
                frame_support::log::error!(
                    "Expected storage version 1, found {:?}, skipping migration",
                    StorageVersion::get::<Pallet<T>>()
                );
            }
            let mut weight = Weight::zero();
            let now = frame_system::Pallet::<T>::block_number();

            SymbolRates::<T>::translate::<Option<BandRateV1>, _>(|symbol, band_rate| {
                weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 2));
                SymbolCheckBlock::<T>::insert(
                    Pallet::<T>::calc_expiration_block(now),
                    symbol,
                    true,
                );
                match band_rate {
                    Some(band_rate) => Some(Some(BandRate {
                        value: band_rate.value,
                        last_updated: band_rate.last_updated,
                        last_updated_block: now,
                        request_id: band_rate.request_id,
                        dynamic_fee: fixed!(0),
                    })),
                    None => None,
                }
            });

            StorageVersion::new(2).put::<Pallet<T>>();
            weight.saturating_add(T::DbWeight::get().reads_writes(1, 1))
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(1),
                "must upgrade linearly"
            );
            Ok(Vec::new())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
            frame_support::ensure!(
                StorageVersion::get::<Pallet<T>>() == StorageVersion::new(2),
                "should be upgraded to version 1"
            );
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{BandRateV1, BandUpdateV2, SymbolRates, SymbolRatesV1};
        use crate::mock::*;
        use crate::pallet::{Pallet, SymbolCheckBlock};
        use common::fixed;
        use frame_support::traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion};
        #[test]
        fn test() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(1).put::<Pallet<Runtime>>();

                let sample_rate = BandRateV1 {
                    value: 0,
                    last_updated: 0,
                    request_id: 0,
                    dynamic_fee: fixed!(0),
                };
                let rates_vec = vec!["USD", "RUB"];
                rates_vec.iter().cloned().for_each(|symbol| {
                    assert_eq!(SymbolRates::<Runtime>::get(symbol), None);
                    assert_eq!(
                        SymbolCheckBlock::<Runtime>::get(
                            1 + GetRateStaleBlockPeriod::get(),
                            symbol
                        ),
                        false,
                    );
                    SymbolRatesV1::<Runtime>::insert(symbol, Some(sample_rate.clone()));
                });

                System::set_block_number(1);
                BandUpdateV2::<Runtime>::on_runtime_upgrade();

                for symbol in rates_vec.into_iter() {
                    let last_updated_block = Pallet::<Runtime>::rates(symbol)
                        .expect("Expected to get rate for the specified symbol")
                        .last_updated_block;
                    assert_eq!(last_updated_block, 1);
                    assert_eq!(
                        SymbolCheckBlock::<Runtime>::get(
                            1 + GetRateStaleBlockPeriod::get(),
                            symbol
                        ),
                        true,
                    );
                }
                assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
            });
        }
    }
}
