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

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg(feature = "runtime-benchmarks")]

use codec::Decode;
use common::{
    balance, AssetId32, AssetIdOf, AssetManager, Balance, DEXId, PredefinedAssetId,
    PriceToolsProvider, PriceVariant, SymbolName, DAI, KEN, KUSD, XOR,
};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use kensetsu::{
    BorrowTax, BorrowTaxes, CdpId, CdpType, CollateralInfos, CollateralRiskParameters, Event,
    KarmaBorrowTax, PegAsset, StablecoinCollateralIdentifier, StablecoinInfos,
    StablecoinParameters, TbcdBorrowTax,
};
use price_tools::AVG_BLOCK_SPAN;
use sp_arithmetic::{Perbill, Percent};
use sp_core::Get;
use sp_runtime::traits::{One, Zero};
use sp_runtime::FixedU128;
use sp_std::vec;

pub struct Pallet<T: Config>(kensetsu::Pallet<T>);
pub trait Config:
    kensetsu::Config
    + pool_xyk::Config
    + trading_pair::Config
    + pallet_timestamp::Config
    + price_tools::Config
{
}

/// Client account id
fn caller<T: Config>() -> T::AccountId {
    let bytes = hex!("92c4ff71ae7492a1e6fef5d80546ea16307c560ac1063ffaa5e0e084df1e2b7e");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

/// Configures Kensetsu Dollar stablecoin pegged to DAI.
pub fn set_kensetsu_dollar_stablecoin<T: Config>() {
    StablecoinInfos::<T>::set::<AssetIdOf<T>>(
        KUSD.into(),
        Some(kensetsu::StablecoinInfo {
            bad_debt: 0,
            stablecoin_parameters: StablecoinParameters {
                peg_asset: PegAsset::SoraAssetId(DAI.into()),
                minimal_stability_fee_accrue: balance!(0),
            },
        }),
    );
}

/// Sets XOR as collateral type with default risk parameters
fn set_xor_as_collateral_type<T: Config>() {
    set_kensetsu_dollar_stablecoin::<T>();

    CollateralInfos::<T>::set(
        StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        },
        Some(kensetsu::CollateralInfo {
            risk_parameters: CollateralRiskParameters {
                hard_cap: Balance::MAX,
                liquidation_ratio: Perbill::from_percent(50),
                max_liquidation_lot: balance!(100),
                stability_fee_rate: FixedU128::from_perbill(Perbill::from_percent(10)),
                minimal_collateral_deposit: balance!(0),
            },
            total_collateral: balance!(0),
            stablecoin_supply: balance!(0),
            last_fee_update_time: Default::default(),
            interest_coefficient: FixedU128::one(),
        }),
    );
}

/// Creates CDP with XOR as collateral
fn create_cdp_with_xor<T: Config>() -> CdpId {
    kensetsu::Pallet::<T>::create_cdp(
        RawOrigin::Signed(caller::<T>()).into(),
        XOR.into(),
        balance!(0),
        KUSD.into(),
        balance!(0),
        balance!(0),
        CdpType::Type2,
    )
    .expect("Shall create CDP");
    kensetsu::NextCDPId::<T>::get()
}

/// Mints XOR and deposited as collateral to CDP
fn deposit_xor_collateral<T: Config>(cdp_id: CdpId, amount: Balance) {
    T::AssetManager::update_balance(
        RawOrigin::Root.into(),
        caller::<T>(),
        XOR.into(),
        amount.try_into().unwrap(),
    )
    .expect("Shall mint XOR");
    kensetsu::Pallet::<T>::deposit_collateral(
        RawOrigin::Signed(caller::<T>()).into(),
        cdp_id,
        amount,
    )
    .expect("Shall deposit");
}

/// Sets liquidation ratio too low, making CDPs unsafe
fn make_cdps_unsafe<T: Config>() {
    CollateralInfos::<T>::mutate(
        StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        },
        |info| {
            if let Some(info) = info.as_mut() {
                info.risk_parameters = CollateralRiskParameters {
                    hard_cap: Balance::MAX,
                    max_liquidation_lot: balance!(100),
                    liquidation_ratio: Perbill::from_percent(1),
                    stability_fee_rate: FixedU128::zero(),
                    minimal_collateral_deposit: balance!(0),
                }
            }
        },
    );
}

/// Initializes and adds liquidity to XYK pool XOR/asset_id.
fn initialize_xyk_pool<T: Config>(asset_id: AssetIdOf<T>) {
    let amount = balance!(1000000);
    T::AssetManager::update_balance(
        RawOrigin::Root.into(),
        caller::<T>(),
        XOR.into(),
        amount.try_into().unwrap(),
    )
    .expect("Shall mint XOR");
    T::AssetManager::update_balance(
        RawOrigin::Root.into(),
        caller::<T>(),
        asset_id,
        amount.try_into().unwrap(),
    )
    .expect("Shall mint token");
    pool_xyk::Pallet::<T>::initialize_pool(
        RawOrigin::Signed(caller::<T>()).into(),
        DEXId::Polkaswap.into(),
        XOR.into(),
        asset_id,
    )
    .expect("Must init init pool");
    pool_xyk::Pallet::<T>::deposit_liquidity(
        RawOrigin::Signed(caller::<T>()).into(),
        DEXId::Polkaswap.into(),
        XOR.into(),
        asset_id,
        amount,
        amount,
        amount,
        amount,
    )
    .expect("Must deposit liquidity to pool");
}

/// Initializes pools with:
/// - XOR/DAI for collateral assessment
/// - XOR/KUSD for liquidation
/// - initializes PriceTools
fn initialize_liquidity_sources<T: Config>() {
    initialize_xyk_pool::<T>(DAI.into());
    trading_pair::Pallet::<T>::register(
        RawOrigin::Signed(caller::<T>()).into(),
        DEXId::Polkaswap.into(),
        XOR.into(),
        KEN.into(),
    )
    .expect("Must register trading pair KEN/XOR");
    initialize_xyk_pool::<T>(KEN.into());
    trading_pair::Pallet::<T>::register(
        RawOrigin::Signed(caller::<T>()).into(),
        DEXId::Polkaswap.into(),
        XOR.into(),
        KUSD.into(),
    )
    .expect("Must register trading pair KUSD/XOR");
    initialize_xyk_pool::<T>(KUSD.into());
    price_tools::Pallet::<T>::register_asset(&KUSD.into()).unwrap();
    for _ in 1..=AVG_BLOCK_SPAN {
        incoming_spot_price::<T>(DAI.into(), balance!(1));
        incoming_spot_price::<T>(KUSD.into(), balance!(1));
    }
}

fn incoming_spot_price<T: price_tools::Config>(asset_id: AssetIdOf<T>, price: Balance) {
    price_tools::FastPriceInfos::<T>::mutate(asset_id, |opt_val| {
        let val = opt_val.as_mut().unwrap();
        val.price_mut_of(PriceVariant::Buy)
            .incoming_spot_price(price, PriceVariant::Buy, &price_tools::FAST_PARAMETERS)
            .unwrap();
        val.price_mut_of(PriceVariant::Sell)
            .incoming_spot_price(price, PriceVariant::Sell, &price_tools::FAST_PARAMETERS)
            .unwrap();
    })
}

benchmarks! {
    where_clause {
        where
            AssetIdOf<T>: From<AssetId32<PredefinedAssetId>>,
            T::Moment: From<u32>,
    }

    create_cdp {
        initialize_liquidity_sources::<T>();
        set_xor_as_collateral_type::<T>();
        let collateral = balance!(10);
        let debt = balance!(1);
        T::AssetManager::update_balance(
            RawOrigin::Root.into(),
            caller::<T>(),
            XOR.into(),
            collateral.try_into().unwrap(),
        )
        .expect("Shall mint XOR");
    }: {
        kensetsu::Pallet::<T>::create_cdp(
            RawOrigin::Signed(caller::<T>()).into(),
            XOR.into(),
            collateral,
            KUSD.into(),
            debt,
            debt,
            CdpType::Type2,
        ).unwrap();
    }

    close_cdp {
        set_xor_as_collateral_type::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
    }: {
        kensetsu::Pallet::<T>::close_cdp(RawOrigin::Signed(caller::<T>()).into(), cdp_id).unwrap();
    }

    deposit_collateral {
        set_xor_as_collateral_type::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
        let amount = balance!(10);
        T::AssetManager::update_balance(
            RawOrigin::Root.into(),
            caller::<T>(),
            XOR.into(),
            amount.try_into().unwrap()
        ).expect("Shall mint XOR");
    }: {
        kensetsu::Pallet::<T>::deposit_collateral(
            RawOrigin::Signed(caller::<T>()).into(),
            cdp_id,
            amount
        ).unwrap();
    }

    borrow {
        initialize_liquidity_sources::<T>();
        set_xor_as_collateral_type::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
        let amount = balance!(10);
        deposit_xor_collateral::<T>(cdp_id, amount);
        let debt = balance!(1);
    }: {
        kensetsu::Pallet::<T>::borrow(
            RawOrigin::Signed(caller::<T>()).into(),
            cdp_id,
            debt,
            debt
        ).unwrap();
    }

    repay_debt {
        initialize_liquidity_sources::<T>();
        set_xor_as_collateral_type::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
        let amount = balance!(10);
        deposit_xor_collateral::<T>(cdp_id, amount);
        let debt = balance!(1);
        kensetsu::Pallet::<T>::borrow(RawOrigin::Signed(caller::<T>()).into(), cdp_id, debt, debt)
            .expect("Shall borrow");
    }: {
        kensetsu::Pallet::<T>::repay_debt(
            RawOrigin::Signed(caller::<T>()).into(),
            cdp_id,
            debt
        ).unwrap();
    }

    liquidate {
        initialize_liquidity_sources::<T>();
        set_xor_as_collateral_type::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
        let amount = balance!(100);
        deposit_xor_collateral::<T>(cdp_id, amount);
        let debt = balance!(50);
        kensetsu::Pallet::<T>::borrow(RawOrigin::Signed(caller::<T>()).into(), cdp_id, debt, debt)
            .expect("Shall borrow");
        make_cdps_unsafe::<T>();
    }: {
        kensetsu::Pallet::<T>::liquidate(RawOrigin::Signed(caller::<T>()).into(), cdp_id).unwrap();
    }

    accrue {
        initialize_liquidity_sources::<T>();
        set_xor_as_collateral_type::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
        let amount = balance!(1000);
        deposit_xor_collateral::<T>(cdp_id, amount);
        let debt = balance!(100);
        kensetsu::Pallet::<T>::borrow(
            RawOrigin::Signed(caller::<T>()).into(),
            cdp_id,
            debt,
            debt
        ).expect("Shall borrow");
        pallet_timestamp::Pallet::<T>::set_timestamp(1.into());
    }: {
        kensetsu::Pallet::<T>::accrue(RawOrigin::Signed(caller::<T>()).into(), cdp_id).unwrap();
    }

    update_collateral_risk_parameters {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_collateral_risk_parameters(
            RawOrigin::Root.into(),
            XOR.into(),
            KUSD.into(),
            CollateralRiskParameters {
                hard_cap: balance!(1000),
                liquidation_ratio: Perbill::from_percent(50),
                max_liquidation_lot: balance!(100),
                stability_fee_rate: Default::default(),
                minimal_collateral_deposit: balance!(0),
            }
        ).unwrap();
    }

    update_borrow_tax {
        let new_borrow_taxes = BorrowTaxes{
            ken_borrow_tax: Percent::from_percent(1),
            karma_borrow_tax: Percent::from_percent(2),
            tbcd_borrow_tax: Percent::from_percent(3),
        };
    }:{
        kensetsu::Pallet::<T>::update_borrow_tax(
            RawOrigin::Root.into(),
            new_borrow_taxes.clone()
        ).unwrap();
    }
    verify {
        let old_borrow_taxes = BorrowTaxes::default();
        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::BorrowTaxUpdated {
                    old_borrow_taxes,
                    new_borrow_taxes: new_borrow_taxes.clone(),
                }
            ).into()
        );
        assert_eq!(new_borrow_taxes.ken_borrow_tax, BorrowTax::<T>::get());
        assert_eq!(new_borrow_taxes.karma_borrow_tax, KarmaBorrowTax::<T>::get());
        assert_eq!(new_borrow_taxes.tbcd_borrow_tax, TbcdBorrowTax::<T>::get());
    }

    update_liquidation_penalty {}:{
        kensetsu::Pallet::<T>::update_liquidation_penalty(
            RawOrigin::Root.into(),
            Percent::from_percent(10)
        ).unwrap();
    }

    withdraw_profit {
        set_kensetsu_dollar_stablecoin::<T>();
        let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::TreasuryTechAccount::get(),
        ).expect("Shall resolve tech account id");
        let amount = balance!(10);
        T::AssetManager::update_balance(
            RawOrigin::Root.into(),
            technical_account_id,
            KUSD.into(),
            amount.try_into().unwrap(),
        )
        .expect("Shall mint KUSD");
    }:{
        kensetsu::Pallet::<T>::withdraw_profit(
            RawOrigin::Root.into(),
            caller::<T>(),
            KUSD.into(),
            amount
        ).unwrap();
    }

    donate {
        set_xor_as_collateral_type::<T>();
        let amount = balance!(10);
        T::AssetManager::update_balance(
            RawOrigin::Root.into(),
            caller::<T>(),
            KUSD.into(),
            amount.try_into().unwrap(),
        )
        .expect("Shall mint KUSD");
        StablecoinInfos::<T>::mutate::<AssetIdOf<T>, _, _>(KUSD.into(), |stablecoin_info| {
            stablecoin_info.as_mut().unwrap().bad_debt = balance!(5);
        });
    }: {
        kensetsu::Pallet::<T>::donate(
            RawOrigin::Signed(caller::<T>()).into(),
            KUSD.into(),
            amount
        ).unwrap();
    }

    register_stablecoin {
        let vec_symbol = SymbolName(vec![b'K', b'D', b'A', b'I']);
        let stablecoin_asset_id: AssetIdOf<T> =
            AssetId32::<PredefinedAssetId>::from_kensetsu_sora_peg_symbol(&vec_symbol).into();
        let stablecoin_parameters = StablecoinParameters::<AssetIdOf<T>> {
            peg_asset: PegAsset::SoraAssetId(DAI.into()),
            minimal_stability_fee_accrue: balance!(0.01),
        };
    }: {
        kensetsu::Pallet::<T>::register_stablecoin(
            RawOrigin::Root.into(),
            stablecoin_parameters.clone(),
        )
        .unwrap()
    }
    verify {
        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::StablecoinRegistered {
                    stablecoin_asset_id,
                    new_stablecoin_parameters: stablecoin_parameters,
                }
            ).into()
        );
    }

    update_hard_cap {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_hard_cap(
            RawOrigin::Root.into(),
            XOR.into(),
            KUSD.into(),
            balance!(42000),
        ).unwrap();
    }
    verify {
        let new_info = CollateralInfos::<T>::get(StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        })
        .expect("Must succeed");
        assert_eq!(new_info.risk_parameters.hard_cap, balance!(42000));

        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::HardCapUpdated {
                    old_hard_cap: Balance::MAX,
                    new_hard_cap: balance!(42000),
                }
            ).into()
        );
    }

    update_liquidation_ratio {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_liquidation_ratio(
            RawOrigin::Root.into(),
            XOR.into(),
            KUSD.into(),
            Perbill::from_percent(42),
        ).unwrap();
    }
    verify {
        let new_info = CollateralInfos::<T>::get(StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        })
        .expect("Must succeed");
        assert_eq!(new_info.risk_parameters.liquidation_ratio, Perbill::from_percent(42));

        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::LiquidationRatioUpdated {
                    old_liquidation_ratio: Perbill::from_percent(50),
                    new_liquidation_ratio: Perbill::from_percent(42),
                }
            ).into()
        );
    }

    update_max_liquidation_lot {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_max_liquidation_lot(
            RawOrigin::Root.into(),
            XOR.into(),
            KUSD.into(),
            balance!(42),
        ).unwrap();
    }
    verify {
        let new_info = CollateralInfos::<T>::get(StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        })
        .expect("Must succeed");
        assert_eq!(new_info.risk_parameters.max_liquidation_lot, balance!(42));

        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::MaxLiquidationLotUpdated {
                    old_max_liquidation_lot: balance!(100),
                    new_max_liquidation_lot: balance!(42),
                }
            ).into()
        );
    }

    update_stability_fee_rate {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_stability_fee_rate(
            RawOrigin::Root.into(),
            XOR.into(),
            KUSD.into(),
            FixedU128::from_perbill(Perbill::from_percent(42)),
        ).unwrap();
    }
    verify {
        let new_info = CollateralInfos::<T>::get(StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        })
        .expect("Must succeed");
        assert_eq!(new_info.risk_parameters.stability_fee_rate, FixedU128::from_perbill(Perbill::from_percent(42)));

        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::StabilityFeeRateUpdated {
                    old_stability_fee_rate: FixedU128::from_perbill(Perbill::from_percent(10)),
                    new_stability_fee_rate: FixedU128::from_perbill(Perbill::from_percent(42)),
                }
            ).into()
        );
    }

    update_minimal_collateral_deposit {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_minimal_collateral_deposit(
            RawOrigin::Root.into(),
            XOR.into(),
            KUSD.into(),
            balance!(42),
        ).unwrap();
    }
    verify {
        let new_info = CollateralInfos::<T>::get(StablecoinCollateralIdentifier {
            collateral_asset_id: XOR.into(),
            stablecoin_asset_id: KUSD.into(),
        })
        .expect("Must succeed");
        assert_eq!(new_info.risk_parameters.minimal_collateral_deposit, balance!(42));

        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::MinimalCollateralDepositUpdated {
                    old_minimal_collateral_deposit: balance!(0),
                    new_minimal_collateral_deposit: balance!(42),
                }
            ).into()
        );
    }

    update_minimal_stability_fee_accrue {
        set_xor_as_collateral_type::<T>();
    }: {
        kensetsu::Pallet::<T>::update_minimal_stability_fee_accrue(
            RawOrigin::Root.into(),
            KUSD.into(),
            balance!(42),
        ).unwrap();
    }
    verify {
        let new_info = StablecoinInfos::<T>::get::<AssetIdOf<T>>(KUSD.into()).expect("Must succeed");
        assert_eq!(new_info.stablecoin_parameters.minimal_stability_fee_accrue, balance!(42));

        frame_system::Pallet::<T>::assert_has_event(
            <T as kensetsu::Config>::RuntimeEvent::from(
                Event::<T>::MinimalStabilityFeeAccrueUpdated {
                    old_minimal_stability_fee_accrue: balance!(0),
                    new_minimal_stability_fee_accrue: balance!(42),
                }
            ).into()
        );
    }
}
