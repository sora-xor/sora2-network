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

//! XST pool module benchmarking.

#![cfg(feature = "runtime-benchmarks")]
#![cfg_attr(not(feature = "std"), no_std)]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

use assets::Event as AssetsEvent;
use band::Pallet as Band;
use codec::{Decode as _, Encode as _};
use common::prelude::{QuoteAmount, SwapAmount};
use common::{
    balance, fixed, AssetName, AssetSymbol, DEXId, LiquiditySource, Oracle, PriceToolsProvider,
    PriceVariant, DAI, XST, XSTUSD,
};
use frame_benchmarking::benchmarks;
use frame_support::pallet_prelude::DispatchResultWithPostInfo;
use frame_support::traits::Get;
use frame_system::{EventRecord, RawOrigin};
use hex_literal::hex;
use oracle_proxy::Pallet as OracleProxy;
use price_tools::Pallet as PriceTools;
use sp_std::prelude::*;
use technical::Pallet as Technical;
use xst::{Call, Event, Pallet as XSTPool};

#[cfg(test)]
mod mock;

mod utils {
    use common::AssetId32;
    use frame_support::{dispatch::DispatchErrorWithPostInfo, Parameter};

    use super::*;

    pub const REFERENCE_SYMBOL: &str = "EURO";

    pub fn symbol<Symbol: Parameter>() -> Symbol {
        let bytes = REFERENCE_SYMBOL.encode();
        Symbol::decode(&mut &bytes[..]).expect("Failed to decode symbol")
    }

    pub fn symbol_asset_id<T: Config>() -> T::AssetId {
        AssetId32::<common::PredefinedAssetId>::from_synthetic_reference_symbol(&symbol::<
            <T as xst::Config>::Symbol,
        >())
        .into()
    }

    pub fn permissioned_account_id<T: Config>() -> T::AccountId {
        let permissioned_tech_account_id = T::GetXSTPoolPermissionedTechAccountId::get();
        Technical::<T>::tech_account_id_to_account_id(&permissioned_tech_account_id)
            .expect("Expected to generate account id from technical")
    }

    pub fn set_asset_mock_price<T: Config>(asset_id: &T::AssetId)
    where
        T: price_tools::Config,
    {
        let _ = PriceTools::<T>::register_asset(asset_id);

        for _ in 0..31 {
            PriceTools::<T>::incoming_spot_price(asset_id, balance!(1), PriceVariant::Buy)
                .expect("Failed to relay spot price");
            PriceTools::<T>::incoming_spot_price(asset_id, balance!(1), PriceVariant::Sell)
                .expect("Failed to relay spot price");
        }
    }

    pub fn setup_exchange_benchmark<T: Config>() {
        set_asset_mock_price::<T>(&DAI.into());
        set_asset_mock_price::<T>(&XST.into());

        let amount: i128 = balance!(1).try_into().unwrap();
        assets::Pallet::<T>::update_balance(
            RawOrigin::Root.into(),
            alice::<T>().into(),
            XST.into(),
            amount,
        )
        .unwrap();
    }

    pub fn alice<T: Config>() -> T::AccountId {
        let bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
        T::AccountId::decode(&mut &bytes[..]).unwrap()
    }

    pub fn assert_last_event<T: Config>(generic_event: <T as xst::Config>::RuntimeEvent) {
        let events = frame_system::Pallet::<T>::events();
        let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
        // compare to the last event record
        let EventRecord { event, .. } = &events[events.len() - 1];
        assert_eq!(event, &system_event);
    }

    pub fn assert_last_assets_event<T: Config>(generic_event: <T as assets::Config>::RuntimeEvent) {
        let events = frame_system::Pallet::<T>::events();
        let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
        // compare to the event record precending to trading pair and xst event records
        let EventRecord { event, .. } = &events[events.len() - 3];
        assert_eq!(event, &system_event);
    }

    pub fn relay_symbol<T: Config>() -> DispatchResultWithPostInfo {
        OracleProxy::<T>::enable_oracle(RawOrigin::Root.into(), Oracle::BandChainFeed)?;
        Band::<T>::add_relayers(RawOrigin::Root.into(), vec![alice::<T>()])?;
        Band::<T>::relay(
            RawOrigin::Signed(alice::<T>()).into(),
            vec![(symbol::<<T as band::Config>::Symbol>(), 1000000000)]
                .try_into()
                .unwrap(),
            0,
            0,
        )
    }

    pub fn enable_synthetic_asset<T: Config>() -> Result<T::AssetId, DispatchErrorWithPostInfo> {
        relay_symbol::<T>()?;
        XSTPool::<T>::register_synthetic_asset(
            RawOrigin::Root.into(),
            AssetSymbol(b"XSTEURO".to_vec()),
            AssetName(b"Sora Synthetic EURO".to_vec()),
            symbol::<<T as xst::Config>::Symbol>(),
            fixed!(0),
        )?;

        Ok(
            XSTPool::<T>::enabled_symbols(symbol::<<T as xst::Config>::Symbol>())
                .expect("Expected enabled synthetic"),
        )
    }
}
pub struct Pallet<T: Config>(xst::Pallet<T>);
pub trait Config: xst::Config + band::Config + oracle_proxy::Config + price_tools::Config {}

benchmarks! {
    set_reference_asset {
    }: _(
        RawOrigin::Root,
        DAI.into()
    )
    verify {
        utils::assert_last_event::<T>(Event::ReferenceAssetChanged(DAI.into()).into())
    }

    enable_synthetic_asset {
        utils::relay_symbol::<T>()?;
    }: _(
        RawOrigin::Root,
        XSTUSD.into(),
        utils::symbol(),
        fixed!(0)
    )
    verify {
        assert!(
            XSTPool::<T>::enabled_symbols(
                utils::symbol::<<T as xst::Config>::Symbol>()
            )
            .is_some()
        );
    }

    disable_synthetic_asset {
        let asset_id = utils::enable_synthetic_asset::<T>()?;
    }: _(
        RawOrigin::Root,
        asset_id
    )
    verify {
        utils::assert_last_event::<T>(Event::SyntheticAssetDisabled(asset_id).into())
    }

    remove_synthetic_asset {
        let asset_id = utils::enable_synthetic_asset::<T>()?;
    }: _(
        RawOrigin::Root,
        asset_id
    )
    verify {
        utils::assert_last_event::<T>(Event::SyntheticAssetRemoved(asset_id, utils::symbol::<<T as xst::Config>::Symbol>()).into())
    }

    register_synthetic_asset {
        let permissioned_account_id = utils::permissioned_account_id::<T>();
        let reference_symbol = utils::symbol::<<T as xst::Config>::Symbol>();
        utils::relay_symbol::<T>()?;
    }: _(
        RawOrigin::Root,
        AssetSymbol(b"XSTEURO".to_vec()),
        AssetName(b"Sora Synthetic EURO".to_vec()),
        reference_symbol,
        fixed!(0)
    )
    verify {
        utils::assert_last_assets_event::<T>(
            AssetsEvent::AssetRegistered(
                utils::symbol_asset_id::<T>(),
                permissioned_account_id
            ).into()
        );
        assert!(
            XSTPool::<T>::enabled_symbols(
                utils::symbol::<<T as xst::Config>::Symbol>()
            )
            .is_some()
        );
    }

    set_synthetic_asset_fee {
        let asset_id = utils::enable_synthetic_asset::<T>()?;
        let fee_ratio = fixed!(0.06);
    }: _(
        RawOrigin::Root,
        asset_id.clone(),
        fee_ratio
    )
    verify {
        utils::assert_last_event::<T>(Event::SyntheticAssetFeeChanged(asset_id, fee_ratio).into())
    }

    set_synthetic_base_asset_floor_price {
    }: _(RawOrigin::Root, balance!(200))
    verify {
        utils::assert_last_event::<T>(Event::SyntheticBaseAssetFloorPriceChanged(balance!(200)).into())
    }

    quote {
        utils::setup_exchange_benchmark::<T>();
        let asset_id = utils::enable_synthetic_asset::<T>()?;
    }: {
        let _ = XSTPool::<T>::quote(
            &DEXId::Polkaswap.into(),
            &XST.into(),
            &asset_id,
            QuoteAmount::with_desired_input(balance!(1)),
            true,
        ).unwrap();
    }

    step_quote {
        utils::setup_exchange_benchmark::<T>();
        let asset_id = utils::enable_synthetic_asset::<T>()?;
    }: {
        let _ = XSTPool::<T>::step_quote(
            &DEXId::Polkaswap.into(),
            &XST.into(),
            &asset_id,
            QuoteAmount::with_desired_input(balance!(1000)),
            1000,
            true,
        ).unwrap();
    }

    exchange {
        utils::setup_exchange_benchmark::<T>();
        let asset_id = utils::enable_synthetic_asset::<T>()?;
    }: {
        let _ = XSTPool::<T>::exchange(
            &utils::alice::<T>(),
            &utils::alice::<T>(),
            &DEXId::Polkaswap.into(),
            &XST.into(),
            &asset_id,
            SwapAmount::with_desired_input(balance!(1), 1),
        ).unwrap();
    }

    impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
