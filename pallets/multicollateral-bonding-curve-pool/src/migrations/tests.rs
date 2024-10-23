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

use crate::migrations::v3::OldDistributionAccounts;
use crate::{mock::*, pallet::Pallet, EnabledTargets};
use crate::{DistributionAccount, DistributionAccountData};
use common::{balance, AssetInfoProvider, TBCD, XST};
use common::{fixed, FromGenericPair};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::traits::{GetStorageVersion as _, StorageVersion};

#[test]
fn test_v1() {
    ExtBuilder::default().build().execute_with(|| {
        assert!(!EnabledTargets::<Runtime>::get().contains(&XST));
        assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 0);

        super::v1::InitializeXstPool::<Runtime>::on_runtime_upgrade();

        assert!(EnabledTargets::<Runtime>::get().contains(&XST));
        assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 1);
    });
}

#[test]
fn test_v2() {
    ExtBuilder::default().build().execute_with(|| {
        super::v1::InitializeXstPool::<Runtime>::on_runtime_upgrade();
        assert!(!EnabledTargets::<Runtime>::get().contains(&TBCD));
        assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 1);

        let assets_and_permissions_tech_account_id =
            <Runtime as technical::Config>::TechAccountId::from_generic_pair(
                b"SYSTEM_ACCOUNT".to_vec(),
                b"ASSETS_PERMISSIONS".to_vec(),
            );
        let assets_and_permissions_account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(
                &assets_and_permissions_tech_account_id,
            )
            .unwrap();
        frame_system::Pallet::<Runtime>::inc_providers(&assets_and_permissions_account_id);

        super::v2::InitializeTBCD::<Runtime>::on_runtime_upgrade();

        assert!(EnabledTargets::<Runtime>::get().contains(&TBCD));
        assert_eq!(
            assets::Pallet::<Runtime>::total_balance(
                &TBCD,
                &super::v2::SORAMITSU_PAYMENT_ACCOUNT.into()
            )
            .unwrap(),
            balance!(1688406)
        );
        assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
    });
}

#[test]
fn test_v3() {
    #[frame_support::storage_alias]
    pub type DistributionAccountsEntry<T: crate::Config> = StorageValue<
        Pallet<T>,
        OldDistributionAccounts<
            DistributionAccountData<DistributionAccount<AccountId, TechAccountId>>,
        >,
    >;

    ExtBuilder::default().build().execute_with(|| {
        StorageVersion::new(2).put::<Pallet<Runtime>>();

        DistributionAccountsEntry::<Runtime>::put(OldDistributionAccounts {
            xor_allocation: DistributionAccountData {
                account: DistributionAccount::Account([1u8; 32].into()),
                coefficient: fixed!(0.1),
            },
            val_holders: DistributionAccountData {
                account: DistributionAccount::Account([2u8; 32].into()),
                coefficient: fixed!(0.2),
            },
            sora_citizens: DistributionAccountData {
                account: DistributionAccount::Account([3u8; 32].into()),
                coefficient: fixed!(0.3),
            },
            stores_and_shops: DistributionAccountData {
                account: DistributionAccount::Account([4u8; 32].into()),
                coefficient: fixed!(0.4),
            },
            parliament_and_development: DistributionAccountData {
                account: DistributionAccount::Account([5u8; 32].into()),
                coefficient: fixed!(0.5),
            },
            projects: DistributionAccountData {
                account: DistributionAccount::Account([6u8; 32].into()),
                coefficient: fixed!(0.6),
            },
        });

        super::v3::MigrateToV3::<Runtime>::on_runtime_upgrade();

        assert_eq!(
            crate::DistributionAccountsEntry::<Runtime>::get(),
            crate::DistributionAccounts {
                xor_allocation: DistributionAccountData {
                    account: DistributionAccount::Account([1u8; 32].into()),
                    coefficient: fixed!(0.1),
                },
                val_holders: DistributionAccountData {
                    account: DistributionAccount::Account([2u8; 32].into()),
                    coefficient: fixed!(0.2),
                },
                sora_citizens: DistributionAccountData {
                    account: DistributionAccount::Account([3u8; 32].into()),
                    coefficient: fixed!(0.3),
                },
                stores_and_shops: DistributionAccountData {
                    account: DistributionAccount::Account([4u8; 32].into()),
                    coefficient: fixed!(0.4),
                },
                projects: DistributionAccountData {
                    account: DistributionAccount::Account([6u8; 32].into()),
                    coefficient: fixed!(0.6),
                },
            }
        );

        assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 3);
    });
}
