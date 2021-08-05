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

use crate::{ClaimableShares, Config, Pallet, ShareholderAccounts, SubscribedAccounts, Weight};
use common::fixnum::ops::{CheckedAdd, Zero};
use common::prelude::{Fixed, FixedWrapper};
use common::{balance, PoolXykPallet};
use frame_support::debug;
use frame_support::dispatch::DispatchError;
use frame_support::traits::{Get, GetPalletVersion, PalletVersion};
use sp_std::convert::TryInto;

pub fn migrate<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    match Pallet::<T>::storage_version() {
        // Initial version is 0.1.0 which uses shares from total amount to determine owned pswap by users
        // Version 0.2.0 performs share calculated on distribution, so only absolute pswap amounts are stored
        // Version 1.1.1 fixes subscribed accounts table, which wasn't migrated from pool tokens to new flow with pool accounts
        Some(version) if version == PalletVersion::new(0, 1, 0) => {
            let migrated_weight =
                migrate_from_shares_to_absolute_rewards::<T>().unwrap_or(100_000_000);
            weight = weight.saturating_add(migrated_weight)
        }
        Some(version) if version == PalletVersion::new(0, 2, 0) => {
            let migrated_weight = migrate_subscribed_accounts::<T>().unwrap_or(100_000);
            weight = weight.saturating_add(migrated_weight)
        }
        _ => (),
    }

    weight
}

pub fn migrate_from_shares_to_absolute_rewards<T: Config>() -> Result<Weight, DispatchError> {
    common::with_transaction(|| {
        let mut weight: Weight = 0;

        let incentives_asset_id = T::GetIncentiveAssetId::get();
        let tech_account_id = T::GetTechnicalAccountId::get();
        let total_claimable =
            assets::Module::<T>::free_balance(&incentives_asset_id, &tech_account_id)?;
        let shares_total = FixedWrapper::from(ClaimableShares::<T>::get());

        ShareholderAccounts::<T>::translate(|_key: T::AccountId, current_position: Fixed| {
            let claimable_incentives = FixedWrapper::from(current_position)
                * total_claimable.clone()
                / shares_total.clone();
            let claimable_incentives: Fixed =
                claimable_incentives.get().unwrap_or(current_position);
            Some(claimable_incentives)
        });

        let mut calculated_total_shares = Fixed::ZERO;
        for (_acc, val) in ShareholderAccounts::<T>::iter() {
            calculated_total_shares = calculated_total_shares.saturating_add(val);
            weight = weight.saturating_add(T::DbWeight::get().reads_writes(2, 2));
        }
        ClaimableShares::<T>::put(calculated_total_shares);
        weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));

        let distribution_remainder = total_claimable.saturating_sub(
            calculated_total_shares
                .into_bits()
                .try_into()
                .unwrap_or(balance!(0)),
        );
        if distribution_remainder > 0 {
            assets::Module::<T>::transfer_from(
                &incentives_asset_id,
                &tech_account_id,
                &T::GetParliamentAccountId::get(),
                distribution_remainder,
            )?;
        }

        Ok(weight)
    })
}

pub fn migrate_subscribed_accounts<T: Config>() -> Result<Weight, DispatchError> {
    common::with_transaction(|| {
        let mut weight: Weight = 0;

        for (_base_asset, _target_asset, (pool_account, fees_account)) in
            T::PoolXykPallet::all_properties()
        {
            SubscribedAccounts::<T>::mutate(&fees_account, |opt_value| {
                if let Some((_, ref mut old_pool_account, _, _)) = opt_value {
                    *old_pool_account = pool_account;
                } else {
                    debug::error!("Unable to find fees account: {:?}", fees_account);
                }
            });
            weight = weight.saturating_add(T::DbWeight::get().writes(1));
        }

        Ok(weight)
    })
}
