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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use common::{AssetId32, PredefinedAssetId, KUSD, XOR};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use sp_arithmetic::{Perbill, Percent};
use sp_core::{Get, U256};

/// Some account id
fn alice<T: Config>() -> T::AccountId {
    let bytes =
        hex_literal::hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).unwrap()
}

/// Sets XOR as collateral type with default risk parameters
fn set_xor_as_collateral_type<T: Config>() {
    CollateralTypes::<T>::set::<AssetIdOf<T>>(
        XOR.into(),
        Some(CollateralRiskParameters {
            max_supply: balance!(1000),
            liquidation_ratio: Perbill::from_percent(50),
            max_liquidation_lot: balance!(100),
            stability_fee_rate: FixedU128::from_perbill(Perbill::from_percent(10)),
        }),
    );
}

/// Creates CDP with XOR as collateral
fn create_cdp_with_xor<T: Config>() -> U256 {
    let caller: T::AccountId = alice::<T>();
    Pallet::<T>::create_cdp(RawOrigin::Signed(caller).into(), XOR.into())
        .expect("Shall create CDP");
    NextCDPId::<T>::get()
}

// /// Mints XOR and deposited as collateral to CDP
// fn deposit_xor_collateral<T: Config>(cdp_id: U256, amount: Balance) {
//     let caller: T::AccountId = alice::<T>();
//     assets::Pallet::<T>::update_balance(
//         RawOrigin::Root.into(),
//         caller.clone(),
//         XOR.into(),
//         amount.try_into().unwrap(),
//     )
//     .expect("Shall mint XOR");
//     Pallet::<T>::deposit_collateral(RawOrigin::Signed(caller).into(), cdp_id, amount)
//         .expect("Shall deposit");
// }
//
// /// Sets liquidation ratio too low, making CDPs unsafe
// fn make_cdps_unsafe<T: Config>() {
//     CollateralTypes::<T>::set::<AssetIdOf<T>>(
//         XOR.into(),
//         Some(CollateralRiskParameters {
//             max_supply: balance!(1000),
//             liquidation_ratio: Perbill::from_percent(10),
//             max_liquidation_lot: balance!(100),
//             stability_fee_rate: Default::default(),
//         }),
//     );
// }

benchmarks! {
    where_clause {
        where T::AssetId: From<AssetId32<PredefinedAssetId>>
    }

    create_cdp {
        set_xor_as_collateral_type::<T>();
        let caller: T::AccountId = alice::<T>();
    }: create_cdp(RawOrigin::Signed(caller.clone()), XOR.into())

    close_cdp {
        set_xor_as_collateral_type::<T>();
        let caller: T::AccountId = alice::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
    }: close_cdp(RawOrigin::Signed(caller.clone()), cdp_id)

    deposit_collateral {
        set_xor_as_collateral_type::<T>();
        let caller: T::AccountId = alice::<T>();
        let cdp_id = create_cdp_with_xor::<T>();
        let amount = balance!(10);
        assets::Pallet::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            XOR.into(),
            amount.try_into().unwrap()
        ).expect("Shall mint XOR");
    }: deposit_collateral(RawOrigin::Signed(caller.clone()), cdp_id, amount)

    // TODO UnavailableExchangePath
    // withdraw_collateral {
    //     set_xor_as_collateral_type::<T>();
    //     let caller: T::AccountId = alice::<T>();
    //     let cdp_id = create_cdp_with_xor::<T>();
    //     let amount = balance!(10);
    //     deposit_xor_collateral::<T>(cdp_id, amount);
    // }: withdraw_collateral(RawOrigin::Signed(caller.clone()), cdp_id, amount)

    // TODO UnavailableExchangePath
    // borrow {
    //     set_xor_as_collateral_type::<T>();
    //     let caller: T::AccountId = alice::<T>();
    //     let cdp_id = create_cdp_with_xor::<T>();
    //     let amount = balance!(10);
    //     deposit_xor_collateral::<T>(cdp_id, amount);
    //     let debt = balance!(1);
    // }: borrow(RawOrigin::Signed(caller.clone()), cdp_id, debt)

    // TODO UnavailableExchangePath
    // repay_debt {
    //     set_xor_as_collateral_type::<T>();
    //     let caller: T::AccountId = alice::<T>();
    //     let cdp_id = create_cdp_with_xor::<T>();
    //     let amount = balance!(10);
    //     deposit_xor_collateral::<T>(cdp_id, amount);
    //     let debt = balance!(1);
    //     Pallet::<T>::borrow(RawOrigin::Signed(caller.clone()).into(), cdp_id, debt)
    //         .expect("Shall borrow");
    // }: repay_debt(RawOrigin::Signed(caller.clone()), cdp_id, debt)

    // TODO UnavailableExchangePath
    // liquidate {
    //     set_xor_as_collateral_type::<T>();
    //     let caller: T::AccountId = alice::<T>();
    //     let cdp_id = create_cdp_with_xor::<T>();
    //     let amount = balance!(10);
    //     deposit_xor_collateral::<T>(cdp_id, amount);
    //     let debt = balance!(5);
    //     Pallet::<T>::borrow(RawOrigin::Signed(caller.clone()).into(), cdp_id, debt)
    //         .expect("Shall borrow");
    //     make_cdps_unsafe::<T>();
    // }: liquidate(RawOrigin::Signed(caller.clone()), cdp_id)

    // TODO UnavailableExchangePath
    // accrue {
    //     set_xor_as_collateral_type::<T>();
    //     let caller: T::AccountId = alice::<T>();
    //     let cdp_id = create_cdp_with_xor::<T>();
    //     let amount = balance!(10);
    //     deposit_xor_collateral::<T>(cdp_id, amount);
    //     let debt = balance!(1);
    //     Pallet::<T>::borrow(RawOrigin::Signed(caller.clone()).into(), cdp_id, debt)
    //         .expect("Shall borrow");
    //     pallet_timestamp::Pallet::<T>::set_timestamp(1);
    // }: accrue(RawOrigin::Signed(caller.clone()), cdp_id)

    // This benchmark doesn't count subsequent accrue() calls, assuming that risk manager will not
    // abuse this call.
    update_collateral_risk_parameters {
        let caller: T::AccountId = alice::<T>();
    }: update_collateral_risk_parameters(
        RawOrigin::Signed(caller.clone()),
        XOR.into(),
        CollateralRiskParameters {
            max_supply: balance!(1000),
            liquidation_ratio: Perbill::from_percent(50),
            max_liquidation_lot: balance!(100),
            stability_fee_rate: Default::default(),
        }
    )

    update_hard_cap_total_supply {
        let caller: T::AccountId = alice::<T>();
    }: update_hard_cap_total_supply(RawOrigin::Signed(caller.clone()), balance!(1000))

    update_liquidation_penalty {
        let caller: T::AccountId = alice::<T>();
    }: _(RawOrigin::Signed(caller.clone()), Percent::from_percent(10))

    withdraw_profit {
        let caller: T::AccountId = alice::<T>();
        let technical_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::TreasuryTechAccount::get(),
        )
        .expect("Shall resolve tech account id");
        let amount = balance!(10);
        assets::Pallet::<T>::update_balance(
            RawOrigin::Root.into(),
            technical_account_id,
            KUSD.into(),
            amount.try_into().unwrap(),
        )
        .expect("Shall mint KUSD");
    }: _(RawOrigin::Signed(caller.clone()), amount)

    donate {
        let caller: T::AccountId = alice::<T>();
        let amount = balance!(10);
        assets::Pallet::<T>::update_balance(
            RawOrigin::Root.into(),
            caller.clone(),
            KUSD.into(),
            amount.try_into().unwrap(),
        )
        .expect("Shall mint KUSD");
        BadDebt::<T>::set(balance!(5));
    }: donate(RawOrigin::Signed(caller.clone()), amount)
}
