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
    use frame_support::pallet_prelude::Weight;
    use frame_support::traits::OnRuntimeUpgrade;
    use log::error;
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
        CollateralInfos, Config, Pallet, PegAsset, StablecoinCollateralIdentifier, StablecoinInfo,
        StablecoinInfos, StablecoinParameters,
    };
    use common::{balance, AssetIdOf, SymbolName, DAI, KARMA, KGOLD, KUSD, KXOR, TBCD, XOR};
    use core::marker::PhantomData;
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use log::error;
    use permissions::{Scope, BURN, MINT};
    use sp_core::Get;

    mod v1 {
        use crate::{CdpId, CollateralRiskParameters, Config, Pallet};
        use codec::{Decode, Encode, MaxEncodedLen};
        use common::{AccountIdOf, AssetIdOf, Balance};
        use frame_support::pallet_prelude::ValueQuery;
        use frame_support::Identity;
        use scale_info::TypeInfo;
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

    impl<T: Config + permissions::Config + technical::Config> UpgradeToV2<T> {
        fn grant_token_permission() -> Weight {
            let mut weight = Weight::zero();

            if let Ok(technical_account_id) = technical::Pallet::<T>::tech_account_id_to_account_id(
                &T::TreasuryTechAccount::get(),
            ) {
                for token in &[KXOR, KGOLD, KARMA] {
                    let scope = Scope::Limited(common::hash(token));
                    for permission in &[MINT, BURN] {
                        match permissions::Pallet::<T>::assign_permission(
                            technical_account_id.clone(),
                            &technical_account_id,
                            *permission,
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

                let scope = Scope::Limited(common::hash(&TBCD));
                match permissions::Pallet::<T>::assign_permission(
                    technical_account_id.clone(),
                    &technical_account_id,
                    BURN,
                    scope,
                ) {
                    Ok(()) => weight += <T as frame_system::Config>::DbWeight::get().writes(1),
                    Err(err) => {
                        error!(
                            "Failed to grant permission to technical account id: {:?}, error: {:?}",
                            technical_account_id, err
                        );
                        weight += <T as frame_system::Config>::DbWeight::get().reads(1);
                    }
                }
            }

            weight
        }

        fn migrate_storage() -> Weight {
            let mut weight = <T as frame_system::Config>::DbWeight::get().reads(1);
            let version = Pallet::<T>::on_chain_storage_version();
            if version <= 1 {
                let kusd_bad_debt = v1::BadDebt::<T>::take();
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                StablecoinInfos::<T>::insert(
                    AssetIdOf::<T>::from(KUSD),
                    StablecoinInfo {
                        bad_debt: kusd_bad_debt,
                        stablecoin_parameters: StablecoinParameters {
                            peg_asset: PegAsset::SoraAssetId(AssetIdOf::<T>::from(DAI)),
                            minimal_stability_fee_accrue: balance!(1),
                        },
                    },
                );
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                StablecoinInfos::<T>::insert(
                    AssetIdOf::<T>::from(KGOLD),
                    StablecoinInfo {
                        bad_debt: balance!(0),
                        stablecoin_parameters: StablecoinParameters {
                            peg_asset: PegAsset::OracleSymbol(SymbolName::xau()),
                            // approximately ~$4
                            minimal_stability_fee_accrue: balance!(0.001),
                        },
                    },
                );
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                StablecoinInfos::<T>::insert(
                    AssetIdOf::<T>::from(KXOR),
                    StablecoinInfo {
                        bad_debt: balance!(0),
                        stablecoin_parameters: StablecoinParameters {
                            peg_asset: PegAsset::SoraAssetId(AssetIdOf::<T>::from(XOR)),
                            minimal_stability_fee_accrue: balance!(100000),
                        },
                    },
                );
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                let collateral_infos: sp_std::vec::Vec<_> = v1::CollateralInfos::<T>::drain()
                    .map(|(collateral_asset_id, old_collateral_info)| {
                        weight += <T as frame_system::Config>::DbWeight::get().writes(1);

                        (
                            StablecoinCollateralIdentifier {
                                collateral_asset_id,
                                stablecoin_asset_id: AssetIdOf::<T>::from(KUSD),
                            },
                            old_collateral_info.into_v2(),
                        )
                    })
                    .collect();
                for (stablecoin_identifier, collateral_info) in collateral_infos {
                    CollateralInfos::<T>::insert(stablecoin_identifier, collateral_info);
                }

                v1::CDPDepository::<T>::translate(
                    |_, cdp: v1::CollateralizedDebtPosition<T::AccountId, AssetIdOf<T>>| {
                        weight += <T as frame_system::Config>::DbWeight::get().writes(1);
                        Some(cdp.into_v2(AssetIdOf::<T>::from(KUSD)))
                    },
                );

                StorageVersion::new(2).put::<Pallet<T>>();
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);
            }

            weight
        }
    }

    impl<T: Config + permissions::Config + technical::Config + pallet_timestamp::Config>
        OnRuntimeUpgrade for UpgradeToV2<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = Weight::zero();
            weight += Self::grant_token_permission();
            weight += Self::migrate_storage();

            weight
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::migrations::v1_to_v2::{v1, UpgradeToV2};
        use crate::mock::{new_test_ext, TestRuntime};
        use crate::{
            CollateralInfos, Pallet, PegAsset, StablecoinCollateralIdentifier, StablecoinInfos,
            StablecoinParameters,
        };
        use common::{balance, SymbolName, DAI, KGOLD, KUSD, KXOR, XOR};
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

                assert_eq!(3, StablecoinInfos::<TestRuntime>::iter().count());
                let kusd_info = StablecoinInfos::<TestRuntime>::get(KUSD).unwrap();
                assert_eq!(kusd_bad_debt, kusd_info.bad_debt);
                assert_eq!(
                    StablecoinParameters {
                        peg_asset: PegAsset::SoraAssetId(DAI),
                        minimal_stability_fee_accrue: balance!(1),
                    },
                    kusd_info.stablecoin_parameters
                );

                let kgold_info = StablecoinInfos::<TestRuntime>::get(KGOLD).unwrap();
                assert_eq!(balance!(0), kgold_info.bad_debt);
                assert_eq!(
                    StablecoinParameters {
                        peg_asset: PegAsset::OracleSymbol(SymbolName::xau()),
                        minimal_stability_fee_accrue: balance!(0.001),
                    },
                    kgold_info.stablecoin_parameters
                );

                let kxor_info = StablecoinInfos::<TestRuntime>::get(KXOR).unwrap();
                assert_eq!(balance!(0), kxor_info.bad_debt);
                assert_eq!(
                    StablecoinParameters {
                        peg_asset: PegAsset::SoraAssetId(XOR),
                        minimal_stability_fee_accrue: balance!(100000),
                    },
                    kxor_info.stablecoin_parameters
                );

                assert_eq!(2, CollateralInfos::<TestRuntime>::iter().count());
                let dai_kusd_collateral_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: DAI,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
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
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
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

/// V3 introduces depository tech account for collaterals.
pub mod v2_to_v3 {
    use crate::{CDPDepository, Config, Pallet};
    use core::marker::PhantomData;
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use sp_core::Get;

    pub struct UpgradeToV3<T>(PhantomData<T>);

    impl<T: Config + permissions::Config + technical::Config + pallet_timestamp::Config>
        OnRuntimeUpgrade for UpgradeToV3<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = Weight::zero();

            let version = Pallet::<T>::on_chain_storage_version();
            if version == 2 {
                let depository_acc = T::DepositoryTechAccount::get();
                weight += match technical::Pallet::<T>::register_tech_account_id_if_not_exist(
                    &depository_acc,
                ) {
                    Ok(()) => <T as frame_system::Config>::DbWeight::get().writes(1),
                    Err(err) => {
                        log::error!(
                            "Failed to register technical account: {:?}, error: {:?}",
                            depository_acc,
                            err
                        );
                        <T as frame_system::Config>::DbWeight::get().reads(1)
                    }
                };

                let treasury_acc = T::TreasuryTechAccount::get();
                for (_, cdp) in CDPDepository::<T>::iter() {
                    technical::Pallet::<T>::transfer(
                        &cdp.collateral_asset_id,
                        &treasury_acc,
                        &depository_acc,
                        cdp.collateral_amount,
                    )
                    .unwrap_or_else(|err| {
                        log::error!("Error while transfer to depository tech acc: {:?}", err);
                    });
                }

                StorageVersion::new(3).put::<Pallet<T>>();
                weight += <T as frame_system::Config>::DbWeight::get().writes(1);
            }

            weight
        }
    }
}

/// Registers SB as predefined stable pegged to DAI.
pub mod v3_to_v4 {
    use crate::{Config, Pallet, PegAsset, StablecoinInfo, StablecoinInfos, StablecoinParameters};
    use common::permissions::{BURN, MINT};
    use common::{balance, AssetIdOf, DAI, SB};
    use core::marker::PhantomData;
    use frame_support::dispatch::GetStorageVersion;
    use frame_support::log::error;
    use frame_support::traits::{OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use permissions::Scope;
    use sp_core::Get;

    pub struct UpgradeToV4<T>(PhantomData<T>);

    impl<T: Config + permissions::Config + technical::Config + pallet_timestamp::Config>
        OnRuntimeUpgrade for UpgradeToV4<T>
    {
        fn on_runtime_upgrade() -> Weight {
            let mut weight = Weight::zero();
            let version = Pallet::<T>::on_chain_storage_version();
            if version == 3 {
                if let Ok(technical_account_id) =
                    technical::Pallet::<T>::tech_account_id_to_account_id(
                        &T::TreasuryTechAccount::get(),
                    )
                {
                    let scope = Scope::Limited(common::hash(&SB));
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
                StablecoinInfos::<T>::insert(
                    AssetIdOf::<T>::from(SB),
                    StablecoinInfo {
                        bad_debt: balance!(0),
                        stablecoin_parameters: StablecoinParameters {
                            peg_asset: PegAsset::SoraAssetId(AssetIdOf::<T>::from(DAI)),
                            minimal_stability_fee_accrue: balance!(1),
                        },
                    },
                );

                StorageVersion::new(4).put::<Pallet<T>>();
                weight += <T as frame_system::Config>::DbWeight::get().reads_writes(3, 3)
            }

            weight
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::migrations::v3_to_v4::UpgradeToV4;
        use crate::mock::{new_test_ext, TestRuntime};
        use crate::{Pallet, PegAsset, StablecoinInfos, StablecoinParameters};
        use common::{balance, DAI, SB};
        use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};

        #[test]
        fn test() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(3).put::<Pallet<TestRuntime>>();

                UpgradeToV4::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(Pallet::<TestRuntime>::on_chain_storage_version(), 4);

                assert_eq!(1, StablecoinInfos::<TestRuntime>::iter().count());
                let sb_info = StablecoinInfos::<TestRuntime>::get(SB).unwrap();
                assert_eq!(
                    StablecoinParameters {
                        peg_asset: PegAsset::SoraAssetId(DAI),
                        minimal_stability_fee_accrue: balance!(1),
                    },
                    sb_info.stablecoin_parameters
                );
            });
        }
    }
}
