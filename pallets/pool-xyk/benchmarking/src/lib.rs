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

//! XYK Pool module benchmarking.

#![cfg(feature = "runtime-benchmarks")]
#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use codec::Decode;
use common::prelude::{Balance, SwapAmount};
use common::{
    balance, AssetInfoProvider, AssetName, AssetSymbol, DEXId, LiquiditySource,
    TradingPairSourceManager, DEFAULT_BALANCE_PRECISION, DOT, XOR,
};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use pool_xyk::Call;
use sp_std::prelude::*;

use assets::Pallet as Assets;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;

#[cfg(test)]
mod mock;
pub struct Pallet<T: Config>(pool_xyk::Pallet<T>);
pub trait Config: pool_xyk::Config {}

pub const DEX: DEXId = DEXId::Polkaswap;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn setup_benchmark_assets_only<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin =
        RawOrigin::Signed(owner.clone()).into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MINT,
        permissions::Scope::Unlimited,
    );
    let _ = Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::BURN,
        permissions::Scope::Unlimited,
    );

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        XOR.into(),
        AssetSymbol(b"XOR".to_vec()),
        AssetName(b"SORA".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"Polkadot".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::from(0u32),
        true,
        None,
        None,
    )
    .unwrap();

    T::TradingPairSourceManager::register_pair(DEX.into(), XOR.into(), DOT.into()).unwrap();

    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000))?;
    Assets::<T>::mint_to(&DOT.into(), &owner.clone(), &owner.clone(), balance!(50000))?;

    Ok(())
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin =
        RawOrigin::Signed(owner.clone()).into();

    setup_benchmark_assets_only::<T>()?;

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(2000),
        balance!(3000),
        balance!(2000),
        balance!(3000),
    )?;

    Ok(())
}
benchmarks! {
    swap_pair {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(1000),
            min_amount_out: balance!(0),
        };
        let initial_base_balance = Assets::<T>::free_balance(&XOR.into(), &caller).unwrap();
        let initial_target_balance = Assets::<T>::free_balance(&DOT.into(), &caller).unwrap();
    }: {
        pool_xyk::Pallet::<T>::exchange(&caller,
        &caller,
        &DEX.into(),
        &XOR.into(),
        &DOT.into(),
        amount.clone()
    ).unwrap();
}
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&XOR.into(), &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - balance!(1000)
        );
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DOT.into(), &caller).unwrap()),
            Into::<u128>::into(initial_target_balance) + balance!(997.997997997997997997)
        );
    }

    can_exchange {
        setup_benchmark::<T>()?;
    }: {
        assert!(XYKPool::<T>::can_exchange(
            &DEX.into(),
            &XOR.into(),
            &DOT.into(),
        ))
    }
    verify {
        // can't check, nothing is changed
    }

    quote {
        setup_benchmark::<T>()?;
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: balance!(1000),
            min_amount_out: balance!(0),
        };
    }: {
        XYKPool::<T>::quote(
            &DEX.into(),
            &XOR.into(),
            &DOT.into(),
            amount.into(),
            true,
        ).unwrap()
    }
    verify {
        // can't check, nothing is changed
    }

    deposit_liquidity {
        setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let initial_xor_balance = Assets::<T>::free_balance(&XOR.into(), &caller).unwrap();
        let initial_dot_balance = Assets::<T>::free_balance(&DOT.into(), &caller).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(2000),
        balance!(3000),
        balance!(2000),
        balance!(3000)
    )
    verify {
        // adding in proportions same as existing, thus call withdraws full deposit
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&XOR.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_xor_balance) - balance!(2000)
        );
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DOT.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_dot_balance) - balance!(3000)
        );
    }

    withdraw_liquidity {
        setup_benchmark::<T>().unwrap();
        let caller = alice::<T>();
        let initial_xor_balance = Assets::<T>::free_balance(&XOR.into(), &caller).unwrap();
        let initial_dot_balance = Assets::<T>::free_balance(&DOT.into(), &caller).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(1000),
        balance!(1),
        balance!(1)
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&XOR.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_xor_balance) + balance!(816.496580927726032746)
        );
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DOT.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_dot_balance) + balance!(1224.744871391589049119)
        );
    }

    initialize_pool {
        setup_benchmark_assets_only::<T>()?;
        let caller = alice::<T>();
        let asset_xor: T::AssetId = XOR.into();
        let asset_dot: T::AssetId = DOT.into();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        asset_xor.clone(),
        asset_dot.clone()
    )
    verify {
        assert!(XYKPool::<T>::properties(asset_xor, asset_dot).is_some())
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::ExtBuilder::default().build(),
        crate::mock::Runtime
    );
}
