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

use crate::{Config, PermissionedTechAccount, Weight};
use common::{
    AssetName, AssetSymbol, Balance, FromGenericPair, LiquiditySourceType,
    DEFAULT_BALANCE_PRECISION, XSTUSD,
};
use frame_support::log::{error, info};
use frame_support::traits::Get;
use permissions::{Scope, BURN, MINT};
use sp_runtime::runtime_logger::RuntimeLogger;
use sp_runtime::traits::Zero;

pub fn migrate<T: Config>() -> Weight {
    let weight: Weight = 0;

    // match Pallet::<T>::storage_version() {
    //     // Register token when pallet is first created, i.e. None version
    //     None => {
    //         let migrated_weight = register_new_token::<T>().unwrap_or(100_000);
    //         weight = weight.saturating_add(migrated_weight);
    //         let migrated_weight = register_xst_tech_account::<T>().unwrap_or(100_000);
    //         weight = weight.saturating_add(migrated_weight);
    //         weight = weight.saturating_add(register_in_dex_api::<T>());
    //     }
    //     _ => (),
    // }

    weight
}

#[allow(dead_code)]
pub fn get_assets_owner_account<T: Config>() -> T::AccountId {
    let assets_and_permissions_tech_account_id = T::TechAccountId::from_generic_pair(
        b"SYSTEM_ACCOUNT".to_vec(),
        b"ASSETS_PERMISSIONS".to_vec(),
    );
    technical::Pallet::<T>::tech_account_id_to_account_id(&assets_and_permissions_tech_account_id)
        .unwrap()
}

#[allow(dead_code)]
pub fn register_new_token<T: Config>() -> Option<Weight> {
    RuntimeLogger::init();

    let result = assets::Pallet::<T>::register_asset_id(
        get_assets_owner_account::<T>(),
        XSTUSD.into(),
        AssetSymbol(b"XSTUSD".to_vec()),
        AssetName(b"SORA Synthetic USD".to_vec()),
        DEFAULT_BALANCE_PRECISION,
        Balance::zero(),
        true,
        None,
        None,
    );

    if result.is_err() {
        error!(
            target: "runtime",
            "failed to register SORA Synthetic USD asset"
        );
    } else {
        info!(
            target: "runtime",
            "registered SORA Synthetic USD asset successfully"
        );
    }

    Some(T::DbWeight::get().writes(1))
}

#[allow(dead_code)]
pub fn get_permissioned_tech_account_id<T: Config>() -> (T::TechAccountId, T::AccountId) {
    let tech_account_id = T::TechAccountId::from_generic_pair(
        crate::TECH_ACCOUNT_PREFIX.to_vec(),
        crate::TECH_ACCOUNT_PERMISSIONED.to_vec(),
    );
    // 1 read, unwrap is guaranteed to work
    let account_id = technical::Pallet::<T>::tech_account_id_to_account_id(&tech_account_id)
        .expect("Couldn't generate tech account for XST pallet during migration.");
    (tech_account_id, account_id)
}

#[allow(dead_code)]
pub fn register_xst_tech_account<T: Config>() -> Option<Weight> {
    RuntimeLogger::init();
    let (xst_permissioned_tech_account_id, xst_permissioned_account_id) =
        get_permissioned_tech_account_id::<T>();

    // 1 read, 2 writes
    let register_result =
        technical::Pallet::<T>::register_tech_account_id(xst_permissioned_tech_account_id.clone());

    PermissionedTechAccount::<T>::set(xst_permissioned_tech_account_id);

    if register_result.is_ok() {
        let permissions = [BURN, MINT];
        info!(
            target: "runtime",
            "registered XST pallet tech account successfully"
        );
        for permission in &permissions {
            // 2 times: 1 read, 3 writes
            let assign_permission_result = permissions::Pallet::<T>::assign_permission(
                xst_permissioned_account_id.clone(),
                &xst_permissioned_account_id,
                *permission,
                Scope::Unlimited,
            );
            if assign_permission_result.is_err() {
                error!(
                    target: "runtime",
                    "failed to assign permissions for XST pallet tech account"
                );
            }
        }
    } else {
        error!(
            target: "runtime",
            "failed to register XST pallet tech account"
        );
    }
    Some(T::DbWeight::get().reads_writes(4, 8))
}

#[allow(dead_code)]
pub fn register_in_dex_api<T: Config>() -> Weight {
    dex_api::EnabledSourceTypes::<T>::mutate(|types| types.push(LiquiditySourceType::XSTPool));
    T::DbWeight::get().writes(1)
}
