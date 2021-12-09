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

//! DEX-API module benchmarking.

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use codec::Decode;
use common::prelude::{Balance, SwapVariant};
use common::{
    balance, AssetName, AssetSymbol, DEXId, LiquiditySourceType, DEFAULT_BALANCE_PRECISION, DOT,
    PSWAP, USDT, VAL, XOR,
};
use frame_benchmarking::benchmarks;
use frame_support::traits::Get;
use frame_system::{EventRecord, RawOrigin};

use frame_benchmarking::Zero;
use hex_literal::hex;
use sp_std::prelude::*;

use assets::Pallet as Assets;
use multicollateral_bonding_curve_pool::Pallet as MBCPool;
use permissions::Pallet as Permissions;
use pool_xyk::Pallet as XYKPool;
use trading_pair::Pallet as TradingPair;

pub const DEX: DEXId = DEXId::Polkaswap;

#[cfg(test)]
mod mock;

pub struct Module<T: Config>(dex_api::Module<T>);
pub trait Config:
    dex_api::Config + pool_xyk::Config + technical::Config + multicollateral_bonding_curve_pool::Config
{
}

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("f43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

// Prepare Runtime for running benchmarks
fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Module::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();
    let dex_id: T::DEXId = DEX.into();

    // Grant permissions to self in case they haven't been explicitly given in genesis config
    Permissions::<T>::assign_permission(
        owner.clone(),
        &owner,
        permissions::MANAGE_DEX,
        permissions::Scope::Limited(common::hash(&dex_id)),
    )
    .unwrap();
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
        AssetName(b"XOR".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        VAL.into(),
        AssetSymbol(b"VAL".to_vec()),
        AssetName(b"VAL".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        PSWAP.into(),
        AssetSymbol(b"PSWAP".to_vec()),
        AssetName(b"PSWAP".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        USDT.into(),
        AssetSymbol(b"USDT".to_vec()),
        AssetName(b"USDT".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"DOT".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
    );
    Assets::<T>::mint_to(&XOR.into(), &owner.clone(), &owner.clone(), balance!(50000)).unwrap();
    Assets::<T>::mint_to(
        &DOT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &USDT.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &VAL.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();
    Assets::<T>::mint_to(
        &PSWAP.into(),
        &owner.clone(),
        &owner.clone(),
        balance!(50000000),
    )
    .unwrap();

    Assets::<T>::mint_to(&XOR.into(), &owner, &bob::<T>(), balance!(50000)).unwrap();

    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into());
    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into());
    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into());
    let _ = TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into());

    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), VAL.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), PSWAP.into())
        .unwrap();
    XYKPool::<T>::initialize_pool(owner_origin.clone(), DEX.into(), XOR.into(), USDT.into())
        .unwrap();

    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        VAL.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        PSWAP.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();
    XYKPool::<T>::deposit_liquidity(
        owner_origin.clone(),
        DEX.into(),
        XOR.into(),
        USDT.into(),
        balance!(1000),
        balance!(2000),
        balance!(0),
        balance!(0),
    )
    .unwrap();

    MBCPool::<T>::initialize_pool(owner_origin.clone(), USDT.into()).unwrap();

    Ok(())
}

#[allow(dead_code)]
fn assert_last_event<T: Config>(generic_event: <T as dex_api::Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    swap {
        let n in 1 .. 1000 => setup_benchmark::<T>().unwrap();

        let caller = bob::<T>();
        let base_asset: T::AssetId = <T as assets::Config>::GetBaseAssetId::get();
        let target_asset: T::AssetId = DOT.into();
    }: {
        dex_api::Module::<T>::swap(
            RawOrigin::Signed(caller.clone()).into(),
            DEX.into(),
            LiquiditySourceType::XYKPool,
            base_asset.clone(),
            target_asset.clone(),
            balance!(2),
            0,
            SwapVariant::WithDesiredInput,
            None
        ).unwrap()
    }
    verify {
        assert_eq!(assets::Module::<T>::total_balance(&target_asset, &caller), Ok(3980063752876763733));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_swap::<Runtime>());
        });
    }
}
