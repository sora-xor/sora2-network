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
    use frame_support::__private::log::error;
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
        CollateralInfos, Config, Pallet, PegAsset, StablecoinCollateralIdentifier, StablecoinInfo,
        StablecoinInfos, StablecoinParameters,
    };
    use common::{balance, AssetIdOf, SymbolName, DAI, KARMA, KGOLD, KUSD, KXOR, TBCD, XOR};
    use core::marker::PhantomData;
    use frame_support::__private::log::error;
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use permissions::{Scope, BURN, MINT};
    use sp_core::Get;

    mod v1 {
        use crate::{CdpId, CollateralRiskParameters, Config, Pallet};
        use codec::{Decode, Encode, MaxEncodedLen};
        use common::{AccountIdOf, AssetIdOf, Balance};
        use frame_support::__private::log::error;
        use frame_support::pallet_prelude::ValueQuery;
        use frame_support::Identity;
        use scale_info::TypeInfo;
        use sp_arithmetic::traits::{AtLeast32Bit, Saturating};
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

        impl<Moment: AtLeast32Bit> CollateralInfo<Moment> {
            pub fn into_v2(self) -> crate::CollateralInfo<Moment> {
                let mut new_risk_parameters = self.risk_parameters;
                // It is rough approximation, but it is fast. Need to reset parameters after the
                // migration.
                new_risk_parameters.stability_fee_rate = new_risk_parameters
                    .stability_fee_rate
                    .saturating_mul(FixedU128::from_u32(1000));
                crate::CollateralInfo {
                    risk_parameters: new_risk_parameters,
                    total_collateral: self.total_collateral,
                    stablecoin_supply: self.kusd_supply,
                    last_fee_update_time: self
                        .last_fee_update_time
                        .checked_div(&Moment::from(1000u32))
                        .unwrap_or_else(|| {
                            error!("Math error. Div by zero.");
                            self.last_fee_update_time
                        }),
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
            if version <= StorageVersion::new(1) {
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
                let last_fee_update_time = 12345000; // in ms
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

                // ms to seconds
                let new_last_fee_update_time = last_fee_update_time / 1000;
                assert_eq!(2, crate::CollateralInfos::<TestRuntime>::iter().count());
                let dai_kusd_collateral_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: DAI,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                assert_eq!(total_collateral, dai_kusd_collateral_info.total_collateral);
                assert_eq!(kusd_supply, dai_kusd_collateral_info.stablecoin_supply);
                assert_eq!(
                    new_last_fee_update_time,
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
                    new_last_fee_update_time,
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
    use frame_support::__private::log::error;
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
            if version == StorageVersion::new(2) {
                let depository_acc = T::DepositoryTechAccount::get();
                weight += match technical::Pallet::<T>::register_tech_account_id_if_not_exist(
                    &depository_acc,
                ) {
                    Ok(()) => <T as frame_system::Config>::DbWeight::get().writes(1),
                    Err(err) => {
                        error!(
                            "Failed to register technical account: {:?}, error: {:?}",
                            depository_acc, err
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
                        error!("Error while transfer to depository tech acc: {:?}", err);
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
    use frame_support::__private::log::error;
    use frame_support::traits::GetStorageVersion;
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
            if version == StorageVersion::new(3) {
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

/// Kensetsu version 5 replaces milliseconds to seconds in parameters
pub mod v4_to_v5 {
    use crate::{CollateralInfos, Config, Pallet};
    use core::marker::PhantomData;
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use sp_core::Get;
    use sp_runtime::traits::Saturating;
    use sp_runtime::FixedU128;

    pub struct UpgradeToV5<T>(PhantomData<T>);

    impl<T: Config + pallet_timestamp::Config> OnRuntimeUpgrade for UpgradeToV5<T> {
        fn on_runtime_upgrade() -> Weight {
            if Pallet::<T>::on_chain_storage_version() == 4 {
                let mut count = 0;

                CollateralInfos::<T>::translate_values::<crate::CollateralInfo<T::Moment>, _>(
                    |mut value| {
                        value.risk_parameters.stability_fee_rate = value
                            .risk_parameters
                            .stability_fee_rate
                            .saturating_mul(FixedU128::from_u32(1000u32));
                        value.last_fee_update_time /= T::Moment::from(1000u32);
                        count += 1;
                        Some(value)
                    },
                );

                StorageVersion::new(5).put::<Pallet<T>>();
                count += 1;

                frame_support::__private::log::info!("Migration to V5 applied");
                T::DbWeight::get().reads_writes(count, count)
            } else {
                frame_support::__private::log::info!(
                    "Migration to V5 already applied, skipping..."
                );
                T::DbWeight::get().reads(1)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::migrations::v4_to_v5::UpgradeToV5;
        use crate::mock::{new_test_ext, TestRuntime};
        use crate::{
            CollateralInfo, CollateralInfos, CollateralRiskParameters, Pallet,
            StablecoinCollateralIdentifier,
        };
        use common::{balance, DAI, ETH, KUSD};
        use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
        use sp_runtime::{FixedU128, Perbill};

        #[test]
        fn test() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(4).put::<Pallet<TestRuntime>>();

                CollateralInfos::<TestRuntime>::insert(
                    StablecoinCollateralIdentifier {
                        collateral_asset_id: DAI,
                        stablecoin_asset_id: KUSD,
                    },
                    CollateralInfo {
                        risk_parameters: CollateralRiskParameters {
                            hard_cap: balance!(1000),
                            liquidation_ratio: Perbill::from_rational(50u32, 100u32),
                            max_liquidation_lot: balance!(1),
                            stability_fee_rate: FixedU128::from_inner(123_456),
                            minimal_collateral_deposit: balance!(1),
                        },
                        total_collateral: balance!(10),
                        stablecoin_supply: balance!(20),
                        last_fee_update_time: 123_456_789,
                        interest_coefficient: FixedU128::from_u32(1),
                    },
                );

                CollateralInfos::<TestRuntime>::insert(
                    StablecoinCollateralIdentifier {
                        collateral_asset_id: ETH,
                        stablecoin_asset_id: KUSD,
                    },
                    CollateralInfo {
                        risk_parameters: CollateralRiskParameters {
                            hard_cap: balance!(10000),
                            liquidation_ratio: Perbill::from_rational(75u32, 100u32),
                            max_liquidation_lot: balance!(1),
                            stability_fee_rate: FixedU128::from_inner(123_456_789),
                            minimal_collateral_deposit: balance!(1),
                        },
                        total_collateral: balance!(1),
                        stablecoin_supply: balance!(30),
                        last_fee_update_time: 123_456,
                        interest_coefficient: FixedU128::from_u32(1),
                    },
                );

                UpgradeToV5::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(CollateralInfos::<TestRuntime>::iter().count(), 2);

                assert_eq!(
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: DAI,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap(),
                    CollateralInfo {
                        risk_parameters: CollateralRiskParameters {
                            hard_cap: balance!(1000),
                            liquidation_ratio: Perbill::from_rational(50u32, 100u32),
                            max_liquidation_lot: balance!(1),
                            stability_fee_rate: FixedU128::from_inner(123_456_000),
                            minimal_collateral_deposit: balance!(1),
                        },
                        total_collateral: balance!(10),
                        stablecoin_supply: balance!(20),
                        last_fee_update_time: 123_456,
                        interest_coefficient: FixedU128::from_u32(1),
                    },
                );

                assert_eq!(
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: ETH,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap(),
                    CollateralInfo {
                        risk_parameters: CollateralRiskParameters {
                            hard_cap: balance!(10000),
                            liquidation_ratio: Perbill::from_rational(75u32, 100u32),
                            max_liquidation_lot: balance!(1),
                            stability_fee_rate: FixedU128::from_inner(123_456_789_000),
                            minimal_collateral_deposit: balance!(1),
                        },
                        total_collateral: balance!(1),
                        stablecoin_supply: balance!(30),
                        last_fee_update_time: 123,
                        interest_coefficient: FixedU128::from_u32(1),
                    },
                );

                assert_eq!(Pallet::<TestRuntime>::on_chain_storage_version(), 5);
            });
        }
    }
}

pub mod v5_to_v6 {
    use crate::{
        CDPDepository, CdpOwnerIndex, CollateralInfos, Config, Pallet,
        StablecoinCollateralIdentifier, StablecoinInfos,
    };
    #[cfg(feature = "try-runtime")]
    use codec::{Decode, Encode};
    use common::{AccountIdOf, AssetIdOf, AssetInfoProvider, AssetManager, Balance, XOR};
    use core::marker::PhantomData;
    use frame_support::__private::log::{error, info, warn};
    use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use sp_core::Get;
    use sp_runtime::traits::Zero;
    use sp_runtime::DispatchError;
    #[cfg(feature = "try-runtime")]
    use sp_runtime::TryRuntimeError;
    use sp_std::collections::btree_map::BTreeMap;
    use sp_std::vec::Vec;

    const TARGET_STORAGE_VERSION: StorageVersion = StorageVersion::new(6);

    pub struct PurgeXorCollateral<T>(PhantomData<T>);

    type AffectedCdp<T> = (crate::CdpId, AccountIdOf<T>, AssetIdOf<T>, Balance, Balance);

    #[cfg(feature = "try-runtime")]
    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    enum MigrationState<AssetId> {
        Execute {
            previous_bad_debt: Vec<(AssetId, Balance)>,
            forgiven_debt: Vec<(AssetId, Balance)>,
            affected_cdp_count: u32,
        },
        Skip(StorageVersion),
    }

    impl<T: Config + assets::Config> PurgeXorCollateral<T> {
        fn xor_asset_id() -> AssetIdOf<T> {
            XOR.into()
        }

        fn depository_account_id() -> Result<AccountIdOf<T>, DispatchError> {
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::DepositoryTechAccount::get())
        }

        fn collect_affected_cdps(xor_asset_id: &AssetIdOf<T>) -> Vec<AffectedCdp<T>> {
            CDPDepository::<T>::iter()
                .filter_map(|(cdp_id, cdp)| {
                    (cdp.collateral_asset_id == *xor_asset_id).then_some((
                        cdp_id,
                        cdp.owner,
                        cdp.stablecoin_asset_id,
                        cdp.debt,
                        cdp.collateral_amount,
                    ))
                })
                .collect()
        }

        fn accumulate_forgiven_debt(
            affected_cdps: &[AffectedCdp<T>],
        ) -> Result<BTreeMap<AssetIdOf<T>, Balance>, DispatchError> {
            let mut forgiven_debt = BTreeMap::new();

            for (_, _, stablecoin_asset_id, debt, _) in affected_cdps {
                if debt.is_zero() {
                    continue;
                }

                let total_debt = forgiven_debt
                    .entry(*stablecoin_asset_id)
                    .or_insert(Balance::zero());
                *total_debt = total_debt
                    .checked_add(*debt)
                    .ok_or(crate::Error::<T>::ArithmeticError)?;
            }

            Ok(forgiven_debt)
        }

        fn collect_xor_collateral_infos(
            xor_asset_id: &AssetIdOf<T>,
        ) -> Vec<StablecoinCollateralIdentifier<AssetIdOf<T>>> {
            CollateralInfos::<T>::iter_keys()
                .filter(|identifier| identifier.collateral_asset_id == *xor_asset_id)
                .collect()
        }

        fn burn_depository_xor(
            xor_asset_id: &AssetIdOf<T>,
            depository_account_id: &AccountIdOf<T>,
        ) -> Result<(), DispatchError> {
            let total_xor = <T as Config>::AssetInfoProvider::total_balance(
                xor_asset_id,
                depository_account_id,
            )?;
            if total_xor.is_zero() {
                return Ok(());
            }

            let xor_assets: <T as assets::Config>::AssetId = (*xor_asset_id).into();
            let free_xor = <T as Config>::AssetInfoProvider::free_balance(
                xor_asset_id,
                depository_account_id,
            )?;
            let reserved_xor = total_xor.saturating_sub(free_xor);

            if !reserved_xor.is_zero() {
                let remainder = assets::Pallet::<T>::unreserve(
                    &xor_assets,
                    depository_account_id,
                    reserved_xor,
                )?;
                if !remainder.is_zero() {
                    warn!(
                        "kensetsu xor purge migration could not unreserve {} XOR from the depository account",
                        remainder
                    );
                }
            }

            let burnable_xor = <T as Config>::AssetInfoProvider::free_balance(
                xor_asset_id,
                depository_account_id,
            )?;
            if !burnable_xor.is_zero() {
                T::AssetManager::burn_from(
                    xor_asset_id,
                    depository_account_id,
                    depository_account_id,
                    burnable_xor,
                )?;
            }

            let remaining_xor = <T as Config>::AssetInfoProvider::total_balance(
                xor_asset_id,
                depository_account_id,
            )?;
            if !remaining_xor.is_zero() {
                return Err(DispatchError::Other(
                    "kensetsu xor purge migration left non-zero XOR in the depository account",
                ));
            }

            Ok(())
        }

        #[cfg(feature = "try-runtime")]
        fn try_runtime_error(message: impl Into<String>) -> TryRuntimeError {
            TryRuntimeError::Other(Box::leak(message.into().into_boxed_str()))
        }
    }

    impl<T: Config + assets::Config> OnRuntimeUpgrade for PurgeXorCollateral<T> {
        fn on_runtime_upgrade() -> Weight {
            let on_chain = Pallet::<T>::on_chain_storage_version();
            if on_chain != StorageVersion::new(5) {
                info!(
                    "kensetsu xor purge migration skipped, on-chain storage version is {:?}",
                    on_chain
                );
                return <T as frame_system::Config>::DbWeight::get().reads(1);
            }

            let migration_result = common::with_transaction(|| -> Result<(), DispatchError> {
                let xor_asset_id = Self::xor_asset_id();
                let depository_account_id = Self::depository_account_id()?;
                let affected_cdps = Self::collect_affected_cdps(&xor_asset_id);
                let forgiven_debt = Self::accumulate_forgiven_debt(&affected_cdps)?;
                let affected_collateral_infos = Self::collect_xor_collateral_infos(&xor_asset_id);

                info!(
                    "kensetsu xor purge migration removing {} XOR-backed CDPs and zeroing {} collateral info entries",
                    affected_cdps.len(),
                    affected_collateral_infos.len()
                );

                for (cdp_id, owner, _, _, _) in &affected_cdps {
                    CDPDepository::<T>::remove(cdp_id);

                    if let Some(mut cdp_ids) = CdpOwnerIndex::<T>::take(owner) {
                        cdp_ids.retain(|existing_cdp_id| existing_cdp_id != cdp_id);
                        if !cdp_ids.is_empty() {
                            CdpOwnerIndex::<T>::insert(owner, cdp_ids);
                        }
                    }
                }

                for identifier in affected_collateral_infos {
                    CollateralInfos::<T>::mutate(identifier, |maybe_info| {
                        if let Some(info) = maybe_info {
                            info.total_collateral = Balance::zero();
                            info.stablecoin_supply = Balance::zero();
                        }
                    });
                }

                for (stablecoin_asset_id, debt) in forgiven_debt {
                    StablecoinInfos::<T>::try_mutate(stablecoin_asset_id, |maybe_info| {
                        let info = maybe_info
                            .as_mut()
                            .ok_or(crate::Error::<T>::StablecoinInfoNotFound)?;
                        info.bad_debt = info
                            .bad_debt
                            .checked_add(debt)
                            .ok_or(crate::Error::<T>::ArithmeticError)?;
                        Ok::<(), DispatchError>(())
                    })?;
                }

                Self::burn_depository_xor(&xor_asset_id, &depository_account_id)?;
                TARGET_STORAGE_VERSION.put::<Pallet<T>>();
                Ok(())
            });

            if let Err(err) = migration_result {
                error!(
                    "kensetsu xor purge migration failed and was rolled back: {:?}",
                    err
                );
                return <T as frame_system::Config>::BlockWeights::get().max_block;
            }

            <T as frame_system::Config>::BlockWeights::get().max_block
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
            let on_chain = Pallet::<T>::on_chain_storage_version();
            if on_chain != StorageVersion::new(5) {
                return Ok(MigrationState::<AssetIdOf<T>>::Skip(on_chain).encode());
            }

            let xor_asset_id = Self::xor_asset_id();
            let affected_cdps = Self::collect_affected_cdps(&xor_asset_id);
            let forgiven_debt = Self::accumulate_forgiven_debt(&affected_cdps)
                .map_err(|err| Self::try_runtime_error(format!("{err:?}")))?;
            let previous_bad_debt = forgiven_debt
                .keys()
                .map(|stablecoin_asset_id| {
                    StablecoinInfos::<T>::get(stablecoin_asset_id)
                        .map(|info| (*stablecoin_asset_id, info.bad_debt))
                        .ok_or_else(|| {
                            Self::try_runtime_error(format!(
                                "missing stablecoin info for {:?}",
                                stablecoin_asset_id
                            ))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;

            Ok(MigrationState::Execute {
                previous_bad_debt,
                forgiven_debt: forgiven_debt.into_iter().collect(),
                affected_cdp_count: affected_cdps.len() as u32,
            }
            .encode())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
            let state = MigrationState::<AssetIdOf<T>>::decode(&mut &state[..]).map_err(|_| {
                Self::try_runtime_error("failed to decode Kensetsu XOR purge migration state")
            })?;

            match state {
                MigrationState::Skip(previous) => {
                    let current = Pallet::<T>::on_chain_storage_version();
                    if current == previous {
                        return Ok(());
                    }

                    Err(Self::try_runtime_error(format!(
                        "expected Kensetsu storage version {:?}, found {:?}",
                        previous, current
                    )))
                }
                MigrationState::Execute {
                    previous_bad_debt,
                    forgiven_debt,
                    affected_cdp_count,
                } => {
                    let xor_asset_id = Self::xor_asset_id();
                    let remaining_xor_cdps = CDPDepository::<T>::iter()
                        .filter(|(_, cdp)| cdp.collateral_asset_id == xor_asset_id)
                        .count();
                    if remaining_xor_cdps != 0 {
                        return Err(Self::try_runtime_error(format!(
                            "expected no XOR-backed CDPs to remain, found {remaining_xor_cdps}"
                        )));
                    }

                    for (identifier, info) in CollateralInfos::<T>::iter() {
                        if identifier.collateral_asset_id == xor_asset_id
                            && (!info.total_collateral.is_zero()
                                || !info.stablecoin_supply.is_zero())
                        {
                            return Err(Self::try_runtime_error(format!(
                                "expected zeroed XOR collateral info for {:?}, found total_collateral={} stablecoin_supply={}",
                                identifier, info.total_collateral, info.stablecoin_supply
                            )));
                        }
                    }

                    for (stablecoin_asset_id, previous_bad_debt) in previous_bad_debt {
                        let forgiven_debt = forgiven_debt
                            .iter()
                            .find_map(|(asset_id, debt)| {
                                (*asset_id == stablecoin_asset_id).then_some(*debt)
                            })
                            .unwrap_or_else(Balance::zero);
                        let current_bad_debt = StablecoinInfos::<T>::get(stablecoin_asset_id)
                            .ok_or_else(|| {
                                Self::try_runtime_error(format!(
                                    "missing stablecoin info after migration for {:?}",
                                    stablecoin_asset_id
                                ))
                            })?
                            .bad_debt;
                        let expected_bad_debt = previous_bad_debt
                            .checked_add(forgiven_debt)
                            .ok_or_else(|| {
                                Self::try_runtime_error(format!(
                                    "bad debt overflow after migration for {:?}",
                                    stablecoin_asset_id
                                ))
                            })?;
                        if current_bad_debt != expected_bad_debt {
                            return Err(Self::try_runtime_error(format!(
                                "expected bad debt {} for {:?}, found {}",
                                expected_bad_debt, stablecoin_asset_id, current_bad_debt
                            )));
                        }
                    }

                    let depository_account_id = Self::depository_account_id()
                        .map_err(|err| Self::try_runtime_error(format!("{err:?}")))?;
                    let remaining_xor = <T as Config>::AssetInfoProvider::total_balance(
                        &xor_asset_id,
                        &depository_account_id,
                    )
                    .map_err(|err| Self::try_runtime_error(format!("{err:?}")))?;
                    if !remaining_xor.is_zero() {
                        return Err(Self::try_runtime_error(format!(
                            "expected zero XOR in Kensetsu depository after migration, found {}",
                            remaining_xor
                        )));
                    }

                    let current = Pallet::<T>::on_chain_storage_version();
                    if current != TARGET_STORAGE_VERSION {
                        return Err(Self::try_runtime_error(format!(
                            "expected Kensetsu storage version {:?} after removing {} XOR-backed CDPs, found {:?}",
                            TARGET_STORAGE_VERSION, affected_cdp_count, current
                        )));
                    }

                    Ok(())
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::PurgeXorCollateral;
        use crate::mock::{new_test_ext, RuntimeOrigin, TestRuntime};
        use crate::test_utils::{
            add_balance, alice, alice_account_id, assert_balance, bob, bob_account_id,
            configure_kensetsu_dollar_for_xor, configure_kxor_for_xor, depository_tech_account_id,
            get_account_cdp_ids, set_kensetsu_dollar_stablecoin,
        };
        use crate::{
            CdpType, CollateralInfos, CollateralRiskParameters, Pallet,
            StablecoinCollateralIdentifier, StablecoinInfos,
        };
        use common::{balance, AssetInfoProvider, Balance, DAI, KUSD, KXOR, XOR};
        use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
        use frame_support::{assert_ok, dispatch::DispatchResult, BoundedVec};
        use sp_arithmetic::{FixedU128, Perbill};
        use sp_runtime::traits::Zero;

        fn create_dai_backed_cdp(
            owner: RuntimeOrigin,
            collateral: Balance,
            debt: Balance,
        ) -> crate::CdpId {
            add_balance(alice_account_id(), collateral, DAI);
            assert_ok!(crate::Pallet::<TestRuntime>::create_cdp(
                owner,
                DAI,
                collateral,
                KUSD,
                debt,
                debt,
                CdpType::Type2
            ));

            crate::NextCDPId::<TestRuntime>::get()
        }

        fn configure_dai_as_kusd_collateral() -> DispatchResult {
            set_kensetsu_dollar_stablecoin();
            crate::Pallet::<TestRuntime>::update_collateral_risk_parameters(
                RuntimeOrigin::root(),
                DAI,
                KUSD,
                CollateralRiskParameters {
                    hard_cap: balance!(1000),
                    liquidation_ratio: Perbill::from_percent(500),
                    max_liquidation_lot: balance!(1000),
                    stability_fee_rate: FixedU128::zero(),
                    minimal_collateral_deposit: balance!(1),
                },
            )
        }

        #[test]
        fn migration_should_remove_xor_backed_cdps_and_move_debt_to_bad_debt() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(5).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                configure_kxor_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                assert_ok!(configure_dai_as_kusd_collateral());

                StablecoinInfos::<TestRuntime>::mutate(KUSD, |maybe_info| {
                    maybe_info.as_mut().unwrap().bad_debt = balance!(5);
                });
                StablecoinInfos::<TestRuntime>::mutate(KXOR, |maybe_info| {
                    maybe_info.as_mut().unwrap().bad_debt = balance!(7);
                });

                let xor_kusd_cdp_alice =
                    crate::test_utils::create_cdp_for_xor(alice(), balance!(100), balance!(20));
                add_balance(bob_account_id(), balance!(80), XOR);
                let xor_kusd_cdp_bob = crate::Pallet::<TestRuntime>::create_cdp(
                    bob(),
                    XOR,
                    balance!(30),
                    KUSD,
                    balance!(4),
                    balance!(4),
                    CdpType::Type2,
                )
                .map(|_| crate::NextCDPId::<TestRuntime>::get())
                .expect("must create XOR/KUSD CDP");
                let xor_kxor_cdp = crate::Pallet::<TestRuntime>::create_cdp(
                    bob(),
                    XOR,
                    balance!(50),
                    KXOR,
                    balance!(10),
                    balance!(10),
                    CdpType::Type2,
                )
                .map(|_| crate::NextCDPId::<TestRuntime>::get())
                .expect("must create XOR/KXOR CDP");
                let dai_cdp = create_dai_backed_cdp(alice(), balance!(30), balance!(3));

                assert!(crate::CDPDepository::<TestRuntime>::contains_key(
                    xor_kusd_cdp_alice
                ));
                assert!(crate::CDPDepository::<TestRuntime>::contains_key(
                    xor_kusd_cdp_bob
                ));
                assert!(crate::CDPDepository::<TestRuntime>::contains_key(
                    xor_kxor_cdp
                ));
                assert!(crate::CDPDepository::<TestRuntime>::contains_key(dai_cdp));
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(alice_account_id()),
                    Some(BoundedVec::try_from(vec![xor_kusd_cdp_alice, dai_cdp]).unwrap())
                );
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(bob_account_id()),
                    Some(BoundedVec::try_from(vec![xor_kusd_cdp_bob, xor_kxor_cdp]).unwrap())
                );
                assert!(
                    assets::Pallet::<TestRuntime>::free_balance(
                        &XOR,
                        &depository_tech_account_id()
                    )
                    .unwrap()
                        > balance!(0)
                );
                assert!(
                    assets::Pallet::<TestRuntime>::free_balance(
                        &DAI,
                        &depository_tech_account_id()
                    )
                    .unwrap()
                        > balance!(0)
                );

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(
                    Pallet::<TestRuntime>::on_chain_storage_version(),
                    StorageVersion::new(6)
                );
                assert!(!crate::CDPDepository::<TestRuntime>::contains_key(
                    xor_kusd_cdp_alice
                ));
                assert!(!crate::CDPDepository::<TestRuntime>::contains_key(
                    xor_kusd_cdp_bob
                ));
                assert!(!crate::CDPDepository::<TestRuntime>::contains_key(
                    xor_kxor_cdp
                ));
                assert!(crate::CDPDepository::<TestRuntime>::contains_key(dai_cdp));
                assert_eq!(get_account_cdp_ids(&alice_account_id()), vec![dai_cdp]);
                assert!(get_account_cdp_ids(&bob_account_id()).is_empty());
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(alice_account_id()),
                    Some(BoundedVec::try_from(vec![dai_cdp]).unwrap())
                );
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(bob_account_id()),
                    None
                );

                assert_eq!(
                    StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt,
                    balance!(29)
                );
                assert_eq!(
                    StablecoinInfos::<TestRuntime>::get(KXOR).unwrap().bad_debt,
                    balance!(17)
                );

                let xor_kusd_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                assert_eq!(xor_kusd_info.total_collateral, balance!(0));
                assert_eq!(xor_kusd_info.stablecoin_supply, balance!(0));

                let xor_kxor_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KXOR,
                    })
                    .unwrap();
                assert_eq!(xor_kxor_info.total_collateral, balance!(0));
                assert_eq!(xor_kxor_info.stablecoin_supply, balance!(0));

                let dai_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: DAI,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                assert_eq!(dai_info.total_collateral, balance!(30));
                assert_eq!(dai_info.stablecoin_supply, balance!(3));

                assert_balance(&depository_tech_account_id(), &XOR, balance!(0));
                assert_balance(&depository_tech_account_id(), &DAI, balance!(30));
            });
        }

        #[test]
        fn migration_should_purge_xor_even_when_totals_do_not_match() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(5).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                let cdp_id =
                    crate::test_utils::create_cdp_for_xor(alice(), balance!(100), balance!(20));

                CollateralInfos::<TestRuntime>::mutate(
                    StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    },
                    |maybe_info| {
                        let info = maybe_info.as_mut().unwrap();
                        info.total_collateral = balance!(1);
                        info.stablecoin_supply = balance!(999);
                    },
                );
                add_balance(depository_tech_account_id(), balance!(50), XOR);
                assert_ok!(assets::Pallet::<TestRuntime>::reserve(
                    &XOR,
                    &depository_tech_account_id(),
                    balance!(30)
                ));
                assert_eq!(
                    <TestRuntime as crate::Config>::AssetInfoProvider::total_balance(
                        &XOR,
                        &depository_tech_account_id()
                    )
                    .unwrap(),
                    balance!(150)
                );
                assert_eq!(
                    assets::Pallet::<TestRuntime>::free_balance(
                        &XOR,
                        &depository_tech_account_id()
                    )
                    .unwrap(),
                    balance!(120)
                );

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(
                    Pallet::<TestRuntime>::on_chain_storage_version(),
                    StorageVersion::new(6)
                );
                assert!(!crate::CDPDepository::<TestRuntime>::contains_key(cdp_id));
                assert!(get_account_cdp_ids(&alice_account_id()).is_empty());
                assert_eq!(
                    StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt,
                    balance!(20)
                );

                let xor_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                assert_eq!(xor_info.total_collateral, balance!(0));
                assert_eq!(xor_info.stablecoin_supply, balance!(0));
                assert_balance(&depository_tech_account_id(), &XOR, balance!(0));
            });
        }

        #[test]
        fn migration_should_delete_zero_debt_xor_cdps_without_creating_bad_debt() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(5).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                let cdp_id =
                    crate::test_utils::create_cdp_for_xor(alice(), balance!(100), balance!(0));

                let bad_debt_before = StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt;

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(
                    Pallet::<TestRuntime>::on_chain_storage_version(),
                    StorageVersion::new(6)
                );
                assert!(!crate::CDPDepository::<TestRuntime>::contains_key(cdp_id));
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(alice_account_id()),
                    None
                );
                assert_eq!(
                    StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt,
                    bad_debt_before
                );
                let xor_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                assert_eq!(xor_info.total_collateral, balance!(0));
                assert_eq!(xor_info.stablecoin_supply, balance!(0));
                assert_balance(&depository_tech_account_id(), &XOR, balance!(0));
            });
        }

        #[test]
        fn migration_should_clean_orphaned_xor_collateral_state_without_cdps() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(5).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                CollateralInfos::<TestRuntime>::mutate(
                    StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    },
                    |maybe_info| {
                        let info = maybe_info.as_mut().unwrap();
                        info.total_collateral = balance!(77);
                        info.stablecoin_supply = balance!(11);
                    },
                );
                add_balance(depository_tech_account_id(), balance!(77), XOR);

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(
                    Pallet::<TestRuntime>::on_chain_storage_version(),
                    StorageVersion::new(6)
                );
                assert_eq!(crate::CDPDepository::<TestRuntime>::iter().count(), 0);
                let xor_info =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                assert_eq!(xor_info.total_collateral, balance!(0));
                assert_eq!(xor_info.stablecoin_supply, balance!(0));
                assert_eq!(
                    StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt,
                    balance!(0)
                );
                assert_balance(&depository_tech_account_id(), &XOR, balance!(0));
            });
        }

        #[test]
        fn migration_should_run_only_once() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(6).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                let cdp_id =
                    crate::test_utils::create_cdp_for_xor(alice(), balance!(100), balance!(20));

                let owner_index_before = Pallet::<TestRuntime>::cdp_owner_index(alice_account_id());
                let collateral_info_before =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();
                let bad_debt_before = StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt;

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(
                    Pallet::<TestRuntime>::on_chain_storage_version(),
                    StorageVersion::new(6)
                );
                assert!(crate::CDPDepository::<TestRuntime>::contains_key(cdp_id));
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(alice_account_id()),
                    owner_index_before
                );
                assert_eq!(
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap(),
                    collateral_info_before
                );
                assert_eq!(
                    StablecoinInfos::<TestRuntime>::get(KUSD).unwrap().bad_debt,
                    bad_debt_before
                );
                assert_balance(&depository_tech_account_id(), &XOR, balance!(100));
            });
        }

        #[cfg(feature = "try-runtime")]
        #[test]
        fn try_runtime_hooks_should_validate_executed_migration() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(5).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                let _cdp_id =
                    crate::test_utils::create_cdp_for_xor(alice(), balance!(100), balance!(20));
                let state = PurgeXorCollateral::<TestRuntime>::pre_upgrade().unwrap();

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                PurgeXorCollateral::<TestRuntime>::post_upgrade(state).unwrap();
            });
        }

        #[cfg(feature = "try-runtime")]
        #[test]
        fn try_runtime_hooks_should_validate_skip_state() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(6).put::<Pallet<TestRuntime>>();
                let state = PurgeXorCollateral::<TestRuntime>::pre_upgrade().unwrap();

                PurgeXorCollateral::<TestRuntime>::post_upgrade(state).unwrap();
            });
        }

        #[test]
        fn migration_should_roll_back_if_state_update_fails() {
            new_test_ext().execute_with(|| {
                StorageVersion::new(5).put::<Pallet<TestRuntime>>();

                configure_kensetsu_dollar_for_xor(
                    balance!(1000),
                    Perbill::from_percent(500),
                    FixedU128::zero(),
                    balance!(1),
                );
                let cdp_id =
                    crate::test_utils::create_cdp_for_xor(alice(), balance!(100), balance!(20));
                let owner_index_before = Pallet::<TestRuntime>::cdp_owner_index(alice_account_id());
                let collateral_info_before =
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap();

                StablecoinInfos::<TestRuntime>::remove(KUSD);

                PurgeXorCollateral::<TestRuntime>::on_runtime_upgrade();

                assert_eq!(
                    Pallet::<TestRuntime>::on_chain_storage_version(),
                    StorageVersion::new(5)
                );
                assert!(crate::CDPDepository::<TestRuntime>::contains_key(cdp_id));
                assert_eq!(
                    Pallet::<TestRuntime>::cdp_owner_index(alice_account_id()),
                    owner_index_before
                );
                assert_eq!(
                    CollateralInfos::<TestRuntime>::get(StablecoinCollateralIdentifier {
                        collateral_asset_id: XOR,
                        stablecoin_asset_id: KUSD,
                    })
                    .unwrap(),
                    collateral_info_before
                );
                assert_balance(&depository_tech_account_id(), &XOR, balance!(100));
            });
        }
    }
}
