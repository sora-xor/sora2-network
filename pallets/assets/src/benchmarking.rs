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

//! Assets module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use codec::Decode;
use frame_benchmarking::benchmarks;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use sp_std::prelude::*;

use common::{DEFAULT_BALANCE_PRECISION, USDT, XOR};

use crate::Pallet as Assets;

// Support Functions
fn alice<T: Config>() -> T::AccountId {
    let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

fn bob<T: Config>() -> T::AccountId {
    let bytes = hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48");
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}

// Adds `n` assets to the Assets Pallet
fn add_assets<T: Config>(n: u32) -> Result<(), &'static str> {
    let owner = alice::<T>();
    frame_system::Pallet::<T>::inc_providers(&owner);
    let owner_origin: <T as frame_system::Config>::RuntimeOrigin = RawOrigin::Signed(owner).into();
    for _i in 0..n {
        Assets::<T>::register(
            owner_origin.clone(),
            AssetSymbol(b"TOKEN".to_vec()),
            AssetName(b"TOKEN".to_vec()),
            Balance::zero(),
            true,
            false,
            None,
            None,
        )?;
    }

    Ok(())
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    let events = frame_system::Pallet::<T>::events();
    let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = &events[events.len() - 1];
    assert_eq!(event, &system_event);
}

benchmarks! {
    register {
        add_assets::<T>(100)?;
        let caller = bob::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
    }: _(
        RawOrigin::Signed(caller.clone()),
        AssetSymbol(b"NEWT".to_vec()),
        AssetName(b"NEWT".to_vec()),
        Balance::zero(),
        true,
        false,
        None,
        None
    )
    verify {
        let (asset_id, _) = AssetOwners::<T>::iter().find(|(k, v)| v == &caller).unwrap();
        assert_last_event::<T>(Event::<T>::AssetRegistered(asset_id, caller).into())
    }

    transfer {
        add_assets::<T>(100)?;
        let caller = alice::<T>();
        let receiver = bob::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        let _ = Assets::<T>::register_asset_id(
            caller.clone(),
            XOR.into(),
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"XOR".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            1000_u32.into(),
            true,
            None,
            None,
        );
    }: _(
        RawOrigin::Signed(caller.clone()),
        XOR.into(),
        receiver.clone(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::<T>::Transfer(caller.clone(), receiver, XOR.into(), 100_u32.into()).into())
    }

    mint {
        add_assets::<T>(100)?;
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"USDT".to_vec()),
            AssetName(b"USDT".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        USDT.into(),
        caller.clone(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::<T>::Mint(caller.clone(), caller, USDT.into(), 100_u32.into()).into())
    }

    force_mint {
        add_assets::<T>(100)?;
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"USDT".to_vec()),
            AssetName(b"USDT".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ).unwrap();
    }: _(
        RawOrigin::Root,
        USDT.into(),
        caller,
        100_u32.into()
    )
    verify {
        let usdt_issuance = Assets::<T>::total_issuance(&USDT.into())?;
        assert_eq!(usdt_issuance, 100_u32.into());
    }

    burn {
        add_assets::<T>(100)?;
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"USDT".to_vec()),
            AssetName(b"USDT".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ).unwrap();
        Assets::<T>::mint(
            RawOrigin::Signed(caller.clone()).into(),
            USDT.into(),
            caller.clone(),
            1000_u32.into()
        ).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        USDT.into(),
        100_u32.into()
    )
    verify {
        assert_last_event::<T>(Event::<T>::Burn(caller, USDT.into(), 100_u32.into()).into())
    }

    update_balance {
        add_assets::<T>(100)?;
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"USDT".to_vec()),
            AssetName(b"USDT".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ).unwrap();
    }: _(
        RawOrigin::Root,
        caller,
        USDT.into(),
        100_i128
    )
    verify {
        let usdt_issuance = Assets::<T>::total_issuance(&USDT.into())?;
        assert_eq!(usdt_issuance, 100_u32.into());
    }

    set_non_mintable {
        add_assets::<T>(100)?;
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        Assets::<T>::register_asset_id(
            caller.clone(),
            USDT.into(),
            AssetSymbol(b"USDT".to_vec()),
            AssetName(b"USDT".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ).unwrap();
    }: _(
        RawOrigin::Signed(caller.clone()),
        USDT.into()
    )
    verify {
        assert_last_event::<T>(Event::<T>::AssetSetNonMintable(USDT.into()).into())
    }

    update_info {
        add_assets::<T>(10)?;
        let caller = alice::<T>();
        frame_system::Pallet::<T>::inc_providers(&caller);
        Assets::<T>::register_asset_id(
            caller,
            USDT.into(),
            AssetSymbol(b"USDT".to_vec()),
            AssetName(b"USDT".to_vec()),
            DEFAULT_BALANCE_PRECISION,
            Balance::zero(),
            true,
            None,
            None,
        ).unwrap();
    }: _(
        RawOrigin::Root,
        USDT.into(),
        Some(AssetSymbol(b"DAI".to_vec())),
        Some(AssetName(b"DAI stablecoin".to_vec()))
    )
    verify {
        assert_eq!(
            crate::AssetInfos::<T>::get(T::AssetId::from(USDT)),
            AssetInfo {
                symbol: AssetSymbol(b"DAI".to_vec()),
                name: AssetName(b"DAI stablecoin".to_vec()),
                precision: DEFAULT_BALANCE_PRECISION,
                is_mintable: true,
                description: None,
                content_source: None
            }
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use frame_support::assert_ok;

    #[test]
    #[ignore]
    fn test_benchmarks() {
        ExtBuilder::default().build().execute_with(|| {
            assert_ok!(Pallet::<Runtime>::test_benchmark_register());
            assert_ok!(Pallet::<Runtime>::test_benchmark_transfer());
            assert_ok!(Pallet::<Runtime>::test_benchmark_mint());
            assert_ok!(Pallet::<Runtime>::test_benchmark_force_mint());
            assert_ok!(Pallet::<Runtime>::test_benchmark_burn());
            assert_ok!(Pallet::<Runtime>::test_benchmark_update_balance());
            assert_ok!(Pallet::<Runtime>::test_benchmark_set_non_mintable());
        });
    }
}
