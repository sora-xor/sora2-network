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

use crate::*;
use common::{KEN, KUSD};
use core::marker::PhantomData;
use frame_support::{log::error, traits::OnRuntimeUpgrade};
use sp_runtime::traits::Zero;

pub struct RegisterKensetsuTokens<T>(PhantomData<T>);

/// Initializes Kensetsu predefined assets:
/// - KEN
/// - KUSD
impl<T: assets::Config + technical::Config> OnRuntimeUpgrade for RegisterKensetsuTokens<T> {
    fn on_runtime_upgrade() -> Weight {
        let assets_permissions_tech_account_id = T::TechAccountId::from_generic_pair(
            b"SYSTEM_ACCOUNT".to_vec(),
            b"ASSETS_PERMISSIONS".to_vec(),
        );
        let assets_permissions_account_id =
            match technical::Pallet::<T>::tech_account_id_to_account_id(
                &assets_permissions_tech_account_id,
            ) {
                Ok(account) => account,
                Err(err) => {
                    error!(
                            "Failed to get account id for assets permissions technical account id: {:?}, error: {:?}",
                            assets_permissions_tech_account_id, err
                        );
                    return <T as frame_system::Config>::DbWeight::get().reads(1);
                }
            };
        if let Err(err) = assets::Pallet::<T>::register_asset_id(
            assets_permissions_account_id.clone(),
            KEN.into(),
            AssetSymbol(b"KEN".to_vec()),
            AssetName(b"Kensetsu token".to_vec()),
            common::DEFAULT_BALANCE_PRECISION,
            common::Balance::zero(),
            true,
            None,
            None,
        ) {
            error!("Failed to register KEN asset, error: {:?}", err);
            return <T as frame_system::Config>::DbWeight::get().reads(1);
        }
        if let Err(err) = assets::Pallet::<T>::register_asset_id(
            assets_permissions_account_id.clone(),
            KUSD.into(),
            AssetSymbol(b"KUSD".to_vec()),
            AssetName(b"Kensetsu Stable Dollar".to_vec()),
            common::DEFAULT_BALANCE_PRECISION,
            common::Balance::zero(),
            true,
            None,
            None,
        ) {
            error!("Failed to register KUSD asset, error: {:?}", err);
            return <T as frame_system::Config>::DbWeight::get().reads(1);
        }
        <T as frame_system::Config>::BlockWeights::get().max_block
    }
}

pub type Migrations = (
    RegisterKensetsuTokens<Runtime>,
    bridge_proxy::migrations::generic_account_v2::LiberlandGenericAccount<Runtime>,
);
