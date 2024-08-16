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

use crate::pallet::{Config, Pallet, TechAccounts};
use core::marker::PhantomData;
use frame_support::pallet_prelude::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;

pub struct SetOnChainIdentity<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for SetOnChainIdentity<T> {
    fn on_runtime_upgrade() -> Weight {
        let mut reads = 1;
        let mut writes = 0;

        for (acc_id, tech_acc_id) in TechAccounts::<T>::iter() {
            let identity_info_opt = Pallet::<T>::gen_tech_account_identity_info(&tech_acc_id);
            if let Some(identity_info) = identity_info_opt {
                Pallet::<T>::set_identity(acc_id, identity_info).unwrap();
                reads += 1;
                writes += 1;
            }
        }
        T::DbWeight::get().reads_writes(reads, writes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Identity, Runtime, TechAccountId};
    use crate::TechAccounts;
    use frame_support::pallet_prelude::StorageVersion;
    use pallet_identity::Data;
    use sp_core::bounded::BoundedVec;

    #[test]
    fn test_set_onchain_identity() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            StorageVersion::new(1).put::<Pallet<Runtime>>();
            let tech_account_id = TechAccountId::Generic("Test123".into(), "Some data".into());
            let account_id =
                crate::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap();
            TechAccounts::<Runtime>::insert(account_id.clone(), tech_account_id);

            assert_eq!(Identity::identity(account_id.clone()), None);

            // migration
            SetOnChainIdentity::<Runtime>::on_runtime_upgrade();

            let registration = Identity::identity(account_id.clone()).unwrap();

            // use sp_std::if_std; // Import into scope the if_std! macro.
            // if_std! {
            //     // This code is only being compiled and executed when the `std` feature is enabled.
            //     println!("Hello native world!");
            //     println!("My value is: {:#?}", info.display.encode());
            // }

            assert_eq!(
                registration.info.display,
                Data::Raw(BoundedVec::truncate_from(b"Test123".to_vec().into()))
            );
            // storage version should not change
            assert_eq!(
                StorageVersion::get::<Pallet<Runtime>>(),
                StorageVersion::new(1)
            );
        });
    }
}
