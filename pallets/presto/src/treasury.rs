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

use crate::Config;
use common::{AccountIdOf, AssetManager, Balance};
use core::marker::PhantomData;
use sp_runtime::traits::Get;
use sp_runtime::DispatchResult;

pub struct Treasury<T: Config>(PhantomData<T>);

impl<T: Config> Treasury<T> {
    pub fn mint_presto_usd(amount: Balance) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        T::AssetManager::mint_to(
            &T::PrestoUsdAssetId::get(),
            &presto_tech_account_id,
            &presto_tech_account_id,
            amount,
        )?;

        Ok(())
    }

    pub fn burn_presto_usd(amount: Balance) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        T::AssetManager::burn_from(
            &T::PrestoUsdAssetId::get(),
            &presto_tech_account_id,
            &presto_tech_account_id,
            amount,
        )?;

        Ok(())
    }

    pub fn send_presto_usd(amount: Balance, to: &AccountIdOf<T>) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            &presto_tech_account_id,
            to,
            amount,
        )?;

        Ok(())
    }

    pub fn transfer_from_buffer_to_main(amount: Balance) -> DispatchResult {
        let presto_tech_account_id =
            technical::Pallet::<T>::tech_account_id_to_account_id(&T::PrestoTechAccount::get())?;

        let presto_buffer_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::PrestoBufferTechAccount::get(),
        )?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            &presto_buffer_tech_account_id,
            &presto_tech_account_id,
            amount,
        )?;

        Ok(())
    }

    pub fn return_from_buffer(amount: Balance, to: &AccountIdOf<T>) -> DispatchResult {
        let presto_buffer_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::PrestoBufferTechAccount::get(),
        )?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            &presto_buffer_tech_account_id,
            to,
            amount,
        )?;

        Ok(())
    }

    pub fn collect_to_buffer(amount: Balance, from: &AccountIdOf<T>) -> DispatchResult {
        let presto_buffer_tech_account_id = technical::Pallet::<T>::tech_account_id_to_account_id(
            &T::PrestoBufferTechAccount::get(),
        )?;

        T::AssetManager::transfer_from(
            &T::PrestoUsdAssetId::get(),
            from,
            &presto_buffer_tech_account_id,
            amount,
        )?;

        Ok(())
    }
}
