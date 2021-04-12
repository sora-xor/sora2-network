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

#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

use pool_xyk::*;

use codec::Decode;
use common::prelude::{Balance, SwapAmount};
use common::{AssetName, AssetSymbol, DEXId, DOT, XOR};
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use hex_literal::hex;
use permissions::{BURN, MINT};
use sp_std::prelude::*;

use assets::Module as Assets;
use permissions::Module as Permissions;
use pool_xyk::Module as XYKPool;
use technical::Module as Technical;
use trading_pair::Module as TradingPair;

#[cfg(test)]
mod mock;
pub struct Module<T: Config>(pool_xyk::Module<T>);
pub trait Config: pool_xyk::Config {}

pub const DEX: DEXId = DEXId::Polkaswap;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn setup_benchmark_assets_only<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

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
        18,
        Balance::from(0u32),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"Polkadot".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;

    Ok(())
}

fn setup_benchmark<T: Config>() -> Result<(), &'static str> {
    let owner = alice::<T>();
    let owner_origin: <T as frame_system::Config>::Origin = RawOrigin::Signed(owner.clone()).into();

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
        18,
        Balance::from(0u32),
        true,
    );
    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        DOT.into(),
        AssetSymbol(b"DOT".to_vec()),
        AssetName(b"Polkadot".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    TradingPair::<T>::register(owner_origin.clone(), DEX.into(), XOR.into(), DOT.into())?;

    let (_, tech_acc_id, _fee_acc_id, mark_asset) =
        XYKPool::<T>::initialize_pool_unchecked(owner.clone(), DEX.into(), XOR.into(), DOT.into())?;

    let _ = Assets::<T>::register_asset_id(
        owner.clone(),
        mark_asset.clone().into(),
        AssetSymbol(b"PSWAP".to_vec()),
        AssetName(b"Polkaswap".to_vec()),
        18,
        Balance::from(0u32),
        true,
    );

    let repr: <T>::AccountId = Technical::<T>::tech_account_id_to_account_id(&tech_acc_id).unwrap();

    Permissions::<T>::grant_permission(owner.clone(), repr.clone(), MINT)?;
    Permissions::<T>::grant_permission(owner.clone(), repr.clone(), BURN)?;

    Assets::<T>::mint(
        owner_origin.clone(),
        XOR.into(),
        owner.clone(),
        10_000_u128.into(),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        owner.clone(),
        20_000_u128.into(),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        XOR.into(),
        repr.clone(),
        1_000_000_u128.into(),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        DOT.into(),
        repr.clone(),
        1_500_000_u128.into(),
    )?;
    Assets::<T>::mint(
        owner_origin.clone(),
        mark_asset.into(),
        owner.clone(),
        1_500_000_000_000_u128.into(),
    )?;

    Ok(())
}

benchmarks! {
    swap_pair {
        let n in 1 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let amount = SwapAmount::WithDesiredInput {
            desired_amount_in: 1_000_u128.into(),
            min_amount_out: 0_u128.into(),
        };
        let initial_base_balance = Assets::<T>::free_balance(&XOR.into(), &caller).unwrap();
        let initial_target_balance = Assets::<T>::free_balance(&DOT.into(), &caller).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        caller.clone(),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        amount.clone()
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&XOR.into(), &caller).unwrap()),
            Into::<u128>::into(initial_base_balance) - 1_000_u128
        );
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DOT.into(), &caller).unwrap()),
            Into::<u128>::into(initial_target_balance) + 1_494_u128
        );
    }

    deposit_liquidity {
        let n in 1 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let initial_xor_balance = Assets::<T>::free_balance(&XOR.into(), &caller).unwrap();
        let initial_dot_balance = Assets::<T>::free_balance(&DOT.into(), &caller).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        2_000_u128.into(),
        3_000_u128.into(),
        0_u128.into(),
        0_u128.into()
    )
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&XOR.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_xor_balance) - 2_000_u128
        );
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DOT.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_dot_balance)
        );
    }

    withdraw_liquidity {
        let n in 1 .. 1000 => setup_benchmark::<T>()?;
        let caller = alice::<T>();
        let initial_xor_balance = Assets::<T>::free_balance(&XOR.into(), &caller).unwrap();
        let initial_dot_balance = Assets::<T>::free_balance(&DOT.into(), &caller).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        XOR.into(),
        DOT.into(),
        38730_u128.into(),
        0_u128.into(),
        0_u128.into()
    )
    //FIXME: Problem with mint and total supply of pool tokens.
    verify {
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&XOR.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_xor_balance) + 0_u128
        );
        assert_eq!(
            Into::<u128>::into(Assets::<T>::free_balance(&DOT.into(), &caller.clone()).unwrap()),
            Into::<u128>::into(initial_dot_balance) + 0_u128
        );
    }

    initialize_pool {
        let n in 1 .. 1000 => setup_benchmark_assets_only::<T>()?;
        let caller = alice::<T>();
    }: _(
        RawOrigin::Signed(caller.clone()),
        DEX.into(),
        XOR.into(),
        DOT.into()
    )
    verify {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(test_benchmark_swap_pair::<Runtime>());
            assert_ok!(test_benchmark_deposit_liquidity::<Runtime>());
            assert_ok!(test_benchmark_withdraw_liquidity::<Runtime>());
            assert_ok!(test_benchmark_initialize_pool::<Runtime>());
        });
    }
}
