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

use core::convert::TryInto;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::ensure;
use frame_support::traits::Get;

use common::prelude::{Balance, SwapAmount};
use common::{ToFeeAccount, ToTechUnitFromDEXAndTradingPair};

use crate::aliases::{AssetIdOf, DEXManager, ExtraAccountIdOf, TechAccountIdOf, TechAssetIdOf};
use crate::bounds::*;
use crate::{Config, Error, Module};

impl<T: Config> Module<T> {
    pub fn get_marking_asset_repr(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<AssetIdOf<T>, DispatchError> {
        use assets::AssetRecord::*;
        use assets::AssetRecordArg::*;
        use common::AssetIdExtraAssetRecordArg::*;
        let repr_extra: ExtraAccountIdOf<T> =
            technical::Module::<T>::tech_account_id_to_account_id(&tech_acc)?.into();
        let tag = GenericU128(common::hash_to_u128_pair(b"Marking asset").0);
        let lst_extra = Extra(LstId(common::LiquiditySourceType::XYKPool.into()).into());
        let acc_extra = Extra(AccountId(repr_extra).into());
        let asset_id =
            assets::Module::<T>::register_asset_id_from_tuple(&Arity3(tag, lst_extra, acc_extra));
        Ok(asset_id)
    }

    pub fn get_marking_asset(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<TechAssetIdOf<T>, DispatchError> {
        let asset_id = Module::<T>::get_marking_asset_repr(tech_acc)?;
        asset_id
            .try_into()
            .map_err(|_| Error::<T>::UnableToConvertAssetToTechAssetId.into())
    }

    /// Using try into to get Result with some error, after this convert Result into Option,
    /// after this AssetDecodingError is used if None.
    pub fn try_decode_asset(asset: AssetIdOf<T>) -> Result<TechAssetIdOf<T>, DispatchError> {
        TryInto::<TechAssetIdOf<T>>::try_into(asset)
            .map_err(|_| Error::<T>::AssetDecodingError.into())
    }

    pub fn decide_is_fee_from_destination(
        asset_a: &AssetIdOf<T>,
        asset_b: &AssetIdOf<T>,
    ) -> Result<bool, DispatchError> {
        let base_asset_id: T::AssetId = T::GetBaseAssetId::get();
        if &base_asset_id == asset_a {
            Ok(false)
        } else if &base_asset_id == asset_b {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn guard_fee_from_destination(
        _asset_a: &AssetIdOf<T>,
        _asset_b: &AssetIdOf<T>,
    ) -> DispatchResult {
        Ok(())
    }

    pub fn guard_fee_from_source(
        _asset_a: &AssetIdOf<T>,
        _asset_b: &AssetIdOf<T>,
    ) -> DispatchResult {
        Ok(())
    }

    pub fn get_min_liquidity_for(
        _asset_id: AssetIdOf<T>,
        _tech_acc: &TechAccountIdOf<T>,
    ) -> Balance {
        //TODO: get this value from DEXInfo.
        1000
    }

    pub fn get_fee_account(
        tech_acc: &TechAccountIdOf<T>,
    ) -> Result<TechAccountIdOf<T>, DispatchError> {
        let fee_acc = tech_acc
            .to_fee_account()
            .ok_or(Error::<T>::UnableToDeriveFeeAccount)?;
        Ok(fee_acc)
    }

    pub fn is_fee_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
        fee_acc: &TechAccountIdOf<T>,
    ) -> DispatchResult {
        let recommended = Self::get_fee_account(tech_acc)?;
        if fee_acc != &recommended {
            Err(Error::<T>::FeeAccountIsInvalid)?;
        }
        Ok(())
    }

    pub fn is_pool_account_valid_for(
        _asset_id: AssetIdOf<T>,
        tech_acc: &TechAccountIdOf<T>,
    ) -> DispatchResult {
        technical::Module::<T>::ensure_tech_account_registered(tech_acc)?;
        //TODO: Maybe checking that asset and dex is exist, it is not really needed if
        //registration of technical account is a garanty that pair and dex exist.
        Ok(())
    }

    pub fn tech_account_from_dex_and_asset_pair(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
    ) -> Result<(common::TradingPair<TechAssetIdOf<T>>, TechAccountIdOf<T>), DispatchError> {
        let dexinfo = DEXManager::<T>::get_dex_info(&dex_id)?;
        let base_asset_id = dexinfo.base_asset_id;
        ensure!(asset_a != asset_b, Error::<T>::AssetsMustNotBeSame);
        let ba = Module::<T>::try_decode_asset(base_asset_id)?;
        let ta = if base_asset_id == asset_a {
            Module::<T>::try_decode_asset(asset_b)?
        } else if base_asset_id == asset_b {
            Module::<T>::try_decode_asset(asset_a)?
        } else {
            Err(Error::<T>::BaseAssetIsNotMatchedWithAnyAssetArguments)?
        };
        let tpair = common::TradingPair::<TechAssetIdOf<T>> {
            base_asset_id: ba,
            target_asset_id: ta,
        };
        Ok((
            tpair,
            TechAccountIdOf::<T>::to_tech_unit_from_dex_and_trading_pair(dex_id, tpair),
        ))
    }

    pub fn get_bounds_from_swap_amount(
        swap_amount: SwapAmount<Balance>,
    ) -> Result<(Bounds<Balance>, Bounds<Balance>), DispatchError> {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in,
                min_amount_out,
            } => Ok((
                Bounds::Desired(desired_amount_in),
                Bounds::Min(min_amount_out),
            )),
            SwapAmount::WithDesiredOutput {
                desired_amount_out,
                max_amount_in,
            } => Ok((
                Bounds::Max(max_amount_in),
                Bounds::Desired(desired_amount_out),
            )),
        }
    }

    #[allow(dead_code)]
    fn get_bounded_asset_pair_for_liquidity(
        dex_id: T::DEXId,
        asset_a: T::AssetId,
        asset_b: T::AssetId,
        swap_amount_a: SwapAmount<Balance>,
        swap_amount_b: SwapAmount<Balance>,
    ) -> Result<
        (
            Bounds<Balance>,
            Bounds<Balance>,
            Bounds<Balance>,
            Bounds<Balance>,
            TechAccountIdOf<T>,
        ),
        DispatchError,
    > {
        let (_, tech_acc_id) =
            Module::<T>::tech_account_from_dex_and_asset_pair(dex_id, asset_a, asset_b)?;
        let (source_amount_a, destination_amount_a) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount_a)?;
        let (source_amount_b, destination_amount_b) =
            Module::<T>::get_bounds_from_swap_amount(swap_amount_b)?;
        Ok((
            source_amount_a,
            destination_amount_a,
            source_amount_b,
            destination_amount_b,
            tech_acc_id,
        ))
    }
}
