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

pub mod init {
    use crate::*;
    use common::{KEN, KUSD};
    use core::marker::PhantomData;
    use frame_support::log::error;
    use frame_support::pallet_prelude::Weight;
    use frame_support::traits::OnRuntimeUpgrade;
    use permissions::{Scope, BURN, MINT};
    use sp_core::Get;

    pub struct RegisterTreasuryTechAccount<T>(PhantomData<T>);

    /// Registers Kensetsu Treasury technical account
    impl<T: Config + permissions::Config + technical::Config> OnRuntimeUpgrade
        for RegisterTreasuryTechAccount<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let tech_account = <T>::TreasuryTechAccount::get();
            match technical::Pallet::<T>::register_tech_account_id_if_not_exist(&tech_account) {
                Ok(()) => <T as frame_system::Config>::DbWeight::get().writes(1),
                Err(err) => {
                    error!(
                        "Failed to register technical account: {:?}, error: {:?}",
                        tech_account, err
                    );
                    <T as frame_system::Config>::DbWeight::get().reads(1)
                }
            }
        }
    }

    pub struct GrantPermissionsTreasuryTechAccount<T>(PhantomData<T>);

    impl<T: Config + permissions::Config + technical::Config> OnRuntimeUpgrade
        for GrantPermissionsTreasuryTechAccount<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = <T as frame_system::Config>::DbWeight::get().reads(1);
            if let Ok(technical_account_id) = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            ) {
                for token in &[KEN, KUSD] {
                    let scope = Scope::Limited(common::hash(token));
                    for permission_id in &[MINT, BURN] {
                        match permissions::Pallet::<T>::assign_permission(
                            technical_account_id.clone(),
                            &technical_account_id,
                            *permission_id,
                            scope,
                        ) {
                            Ok(()) => {
                                weight += <T as frame_system::Config>::DbWeight::get().writes(1)
                            }
                            Err(err) => {
                                error!(
                                "Failed to grant permission to technical account id: {:?}, error: {:?}",
                                technical_account_id, err
                            );
                                weight += <T as frame_system::Config>::DbWeight::get().reads(1);
                            }
                        }
                    }
                }
            }

            weight
        }
    }
}

/// Due to bug in stability fee update some extra KUSD were minted, this migration burns and sets
/// correct amounts.
pub mod stage_correction {
    use crate::{CDPDepository, CollateralInfos, Config, Error};
    use common::AssetInfoProvider;
    use common::Balance;
    use core::marker::PhantomData;
    use frame_support::dispatch::Weight;
    use frame_support::log::error;
    use frame_support::traits::OnRuntimeUpgrade;
    use sp_arithmetic::traits::Zero;
    use sp_core::Get;
    use sp_runtime::DispatchResult;

    pub struct CorrectKusdBalances<T>(PhantomData<T>);

    impl<T: Config + permissions::Config + technical::Config> CorrectKusdBalances<T> {
        fn runtime_upgrade_internal(weight: &mut Weight) -> DispatchResult {
            let mut total_debt = Balance::zero();

            for asset_id in CollateralInfos::<T>::iter_keys() {
                let accumulated_debt_for_collateral = CDPDepository::<T>::iter()
                    .filter(|(_, cdp)| {
                        *weight += <T as frame_system::Config>::DbWeight::get().reads(1);
                        cdp.collateral_asset_id == asset_id
                    })
                    .fold(
                        Balance::zero(),
                        |accumulated_debt_for_collateral, (_, cdp)| {
                            accumulated_debt_for_collateral + cdp.debt
                        },
                    );

                CollateralInfos::<T>::try_mutate(asset_id, |collateral_info| {
                    let collateral_info =
                        collateral_info.as_mut().ok_or(Error::<T>::CDPNotFound)?;
                    collateral_info.kusd_supply = accumulated_debt_for_collateral;
                    DispatchResult::Ok(())
                })?;
                *weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                total_debt += accumulated_debt_for_collateral;
            }

            // burn KUSD on tech account
            let treasury_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            )?;
            let balance =
                T::AssetInfoProvider::free_balance(&T::KusdAssetId::get(), &treasury_account_id)?;
            let to_burn = balance - total_debt;
            assets::Pallet::<T>::burn_from(
                &T::KusdAssetId::get(),
                &treasury_account_id,
                &treasury_account_id,
                to_burn,
            )?;

            *weight += <T as frame_system::Config>::DbWeight::get().writes(1);

            Ok(())
        }
    }

    impl<T: Config + permissions::Config + technical::Config> OnRuntimeUpgrade
        for CorrectKusdBalances<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = Weight::zero();
            Self::runtime_upgrade_internal(&mut weight).unwrap_or_else(|err| {
                error!("Runtime upgrade error {:?}", err);
            });
            weight
        }
    }
}

pub mod storage_add_total_collateral {
    use crate::{CDPDepository, CollateralInfo, CollateralInfos, Config, Error, Timestamp};
    use common::Balance;
    use frame_support::dispatch::{TypeInfo, Weight};
    use frame_support::log::error;
    use frame_support::traits::OnRuntimeUpgrade;
    use sp_arithmetic::traits::Zero;
    use sp_arithmetic::FixedU128;
    use sp_core::Get;
    use sp_runtime::DispatchResult;
    use std::marker::PhantomData;

    mod old {
        use crate::{pallet, CollateralRiskParameters, Pallet};
        use assets::AssetIdOf;
        use codec::{Decode, Encode, MaxEncodedLen};
        use common::{Balance, Config};
        use frame_support::dispatch::{TypeInfo, Weight};
        use frame_support::pallet_prelude::StorageMap;
        use frame_support::Identity;
        use sp_arithmetic::FixedU128;

        /// Old format without `total_collateral` field.
        #[derive(
            Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord,
        )]
        pub struct CollateralInfo<Moment> {
            /// Collateral Risk parameters set by risk management
            pub risk_parameters: CollateralRiskParameters,

            /// Amount of KUSD issued for the collateral
            pub kusd_supply: Balance,

            /// the last timestamp when stability fee was accrued
            pub last_fee_update_time: Moment,

            /// Interest accrued for collateral for all time
            pub interest_coefficient: FixedU128,
        }

        impl<Moment> CollateralInfo<Moment> {
            // Returns new format with provided `total_collateral`.
            pub fn to_new(self, total_collateral: Balance) -> crate::CollateralInfo<Moment> {
                crate::CollateralInfo {
                    risk_parameters: self.risk_parameters,
                    total_collateral,
                    kusd_supply: self.kusd_supply,
                    last_fee_update_time: self.last_fee_update_time,
                    interest_coefficient: self.interest_coefficient,
                }
            }
        }
    }

    pub struct StorageAddTotalCollateral<T>(PhantomData<T>);

    impl<T: Config + permissions::Config + technical::Config> StorageAddTotalCollateral<T> {
        fn runtime_upgrade_internal(weight: &mut Weight) -> DispatchResult {
            CollateralInfos::<T>::translate::<old::CollateralInfo<T::Moment>, _>(
                |collateral_asset_id, old_collateral_info| {
                    let accumulated_collateral = CDPDepository::<T>::iter()
                        .filter(|(_, cdp)| {
                            *weight += <T as frame_system::Config>::DbWeight::get().reads(1);
                            cdp.collateral_asset_id == collateral_asset_id
                        })
                        .fold(Balance::zero(), |accumulated_collateral, (_, cdp)| {
                            accumulated_collateral + cdp.collateral_amount
                        });
                    *weight += <T as frame_system::Config>::DbWeight::get().writes(1);
                    Some(old_collateral_info.to_new(accumulated_collateral))
                },
            );

            Ok(())
        }
    }

    impl<T: Config> OnRuntimeUpgrade for StorageAddTotalCollateral<T> {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = Weight::zero();
            Self::runtime_upgrade_internal(&mut weight).unwrap_or_else(|err| {
                error!("Runtime upgrade error {:?}", err);
            });
            weight
        }
    }
}
