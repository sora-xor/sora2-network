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

    /// Registers Kensetsu Treasury technical account and grant premission to [KEN, KUSD]
    impl<T: Config + permissions::Config + technical::Config> OnRuntimeUpgrade
        for RegisterTreasuryTechAccount<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let tech_account = <T>::TreasuryTechAccount::get();
            let mut weight = match technical::Pallet::<T>::register_tech_account_id_if_not_exist(
                &tech_account,
            ) {
                Ok(()) => <T as frame_system::Config>::DbWeight::get().writes(1),
                Err(err) => {
                    error!(
                        "Failed to register technical account: {:?}, error: {:?}",
                        tech_account, err
                    );
                    <T as frame_system::Config>::DbWeight::get().reads(1)
                }
            };

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

/// Kensetsu version 2 adds configurable debt asset id.
pub mod v1_to_v2 {
    use crate::{
        CollateralInfos, Config, Pallet, PegAsset, StablecoinInfo, StablecoinInfos,
        StablecoinParameters,
    };
    use common::{balance, DAI, KUSD};
    use core::marker::PhantomData;
    use frame_support::dispatch::Weight;
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
    use sp_core::Get;

    mod v1 {
        use crate::{CdpId, CollateralRiskParameters, Config, Pallet};
        use assets::AssetIdOf;
        use codec::{Decode, Encode, MaxEncodedLen};
        use common::{AccountIdOf, Balance};
        use frame_support::dispatch::TypeInfo;
        use frame_support::pallet_prelude::ValueQuery;
        use frame_support::Identity;
        use sp_arithmetic::FixedU128;

        #[derive(
            Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord,
        )]
        pub struct CollateralInfo<Moment> {
            pub risk_parameters: CollateralRiskParameters,
            pub total_collateral: Balance,
            // field was renamed to stablecoin_supply
            pub kusd_supply: Balance,
            pub last_fee_update_time: Moment,
            pub interest_coefficient: FixedU128,
        }

        impl<Moment> CollateralInfo<Moment> {
            pub fn into_v2(self) -> crate::CollateralInfo<Moment> {
                crate::CollateralInfo {
                    risk_parameters: self.risk_parameters,
                    total_collateral: self.total_collateral,
                    stablecoin_supply: self.kusd_supply,
                    last_fee_update_time: self.last_fee_update_time,
                    interest_coefficient: self.interest_coefficient,
                }
            }
        }

        #[derive(
            Debug, Clone, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, PartialOrd, Ord,
        )]
        pub struct CollateralizedDebtPosition<AccountId, AssetId> {
            pub owner: AccountId,
            pub collateral_asset_id: AssetId,
            pub collateral_amount: Balance,
            // stablecoin_asset_id was added
            pub debt: Balance,
            pub interest_coefficient: FixedU128,
        }

        impl<AccountId, AssetId> CollateralizedDebtPosition<AccountId, AssetId> {
            pub fn into_v2(
                self,
                kusd_asset_id: AssetId,
            ) -> crate::CollateralizedDebtPosition<AccountId, AssetId> {
                crate::CollateralizedDebtPosition {
                    owner: self.owner,
                    collateral_asset_id: self.collateral_asset_id,
                    collateral_amount: self.collateral_amount,
                    stablecoin_asset_id: kusd_asset_id,
                    debt: self.debt,
                    interest_coefficient: self.interest_coefficient,
                }
            }
        }

        #[frame_support::storage_alias]
        pub type BadDebt<T: Config> = StorageValue<Pallet<T>, Balance, ValueQuery>;

        #[frame_support::storage_alias]
        pub type CollateralInfos<T: Config> = StorageMap<
            Pallet<T>,
            Identity,
            AssetIdOf<T>,
            CollateralInfo<<T as pallet_timestamp::Config>::Moment>,
        >;

        #[frame_support::storage_alias]
        pub type CDPDepository<T: Config> = StorageMap<
            Pallet<T>,
            Identity,
            CdpId,
            crate::CollateralizedDebtPosition<AccountIdOf<T>, AssetIdOf<T>>,
        >;
    }

    pub struct UpgradeToV2<T>(PhantomData<T>);

    impl<T: Config + permissions::Config + technical::Config + pallet_timestamp::Config>
        OnRuntimeUpgrade for UpgradeToV2<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = <T as frame_system::Config>::DbWeight::get().reads(1);

            let version = Pallet::<T>::on_chain_storage_version();
            if version == 1 {
                let kusd_bad_debt = v1::BadDebt::<T>::take();
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                StablecoinInfos::<T>::insert(
                    T::AssetId::from(KUSD),
                    StablecoinInfo {
                        bad_debt: kusd_bad_debt,
                        stablecoin_parameters: StablecoinParameters {
                            peg_asset: PegAsset::SoraAssetId(T::AssetId::from(DAI)),
                            minimal_stability_fee_accrue: balance!(1),
                        },
                    },
                );
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                for (collateral_asset_id, old_collateral_info) in v1::CollateralInfos::<T>::drain()
                {
                    CollateralInfos::<T>::insert(
                        collateral_asset_id,
                        T::AssetId::from(KUSD),
                        old_collateral_info.into_v2(),
                    );
                    weight += <T as frame_system::Config>::DbWeight::get().writes(1);
                }

                v1::CDPDepository::<T>::translate(
                    |_, cdp: v1::CollateralizedDebtPosition<T::AccountId, T::AssetId>| {
                        weight += <T as frame_system::Config>::DbWeight::get().writes(1);
                        Some(cdp.into_v2(T::AssetId::from(KUSD)))
                    },
                );

                StorageVersion::new(2).put::<Pallet<T>>();
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);
            }

            weight
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::migrations::v1_to_v2::{v1, UpgradeToV2};
        use crate::mock::{new_test_ext, TestRuntime};
        use crate::{CollateralInfos, Pallet, PegAsset, StablecoinInfos, StablecoinParameters};
        use common::{balance, DAI, KUSD, XOR};
        use core::default::Default;
        use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
        use sp_arithmetic::FixedU128;

        #[test]
        fn test() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(1).put::<Pallet<TestRuntime>>();
                let kusd_bad_debt = balance!(2989);
                v1::BadDebt::<TestRuntime>::set(kusd_bad_debt);

                let total_collateral = balance!(500100);
                let kusd_supply = balance!(100500);
                let last_fee_update_time = 12345;
                let interest_coefficient = FixedU128::from_inner(54321);
                let old_dai_collateral_info = v1::CollateralInfo {
                    risk_parameters: Default::default(),
                    total_collateral,
                    kusd_supply,
                    last_fee_update_time,
                    interest_coefficient,
                };
                v1::CollateralInfos::<TestRuntime>::set(DAI, Some(old_dai_collateral_info));
                let old_xor_collateral_info = v1::CollateralInfo {
                    risk_parameters: Default::default(),
                    total_collateral,
                    kusd_supply,
                    last_fee_update_time,
                    interest_coefficient,
                };
                v1::CollateralInfos::<TestRuntime>::set(XOR, Some(old_xor_collateral_info));

                UpgradeToV2::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(Pallet::<TestRuntime>::on_chain_storage_version(), 2);

                assert_eq!(1, StablecoinInfos::<TestRuntime>::iter().count());
                let kusd_info = StablecoinInfos::<TestRuntime>::get(KUSD).unwrap();
                assert_eq!(kusd_bad_debt, kusd_info.bad_debt);
                assert_eq!(
                    StablecoinParameters {
                        peg_asset: PegAsset::SoraAssetId(DAI),
                        minimal_stability_fee_accrue: balance!(1),
                    },
                    kusd_info.stablecoin_parameters
                );

                assert_eq!(2, CollateralInfos::<TestRuntime>::iter().count());
                let dai_kusd_collateral_info =
                    CollateralInfos::<TestRuntime>::get(DAI, KUSD).unwrap();
                assert_eq!(total_collateral, dai_kusd_collateral_info.total_collateral);
                assert_eq!(kusd_supply, dai_kusd_collateral_info.stablecoin_supply);
                assert_eq!(
                    last_fee_update_time,
                    dai_kusd_collateral_info.last_fee_update_time
                );
                assert_eq!(
                    interest_coefficient,
                    dai_kusd_collateral_info.interest_coefficient
                );
                let xor_kusd_collateral_info =
                    CollateralInfos::<TestRuntime>::get(XOR, KUSD).unwrap();
                assert_eq!(total_collateral, xor_kusd_collateral_info.total_collateral);
                assert_eq!(kusd_supply, xor_kusd_collateral_info.stablecoin_supply);
                assert_eq!(
                    last_fee_update_time,
                    xor_kusd_collateral_info.last_fee_update_time
                );
                assert_eq!(
                    interest_coefficient,
                    xor_kusd_collateral_info.interest_coefficient
                );
            });
        }
    }
}
