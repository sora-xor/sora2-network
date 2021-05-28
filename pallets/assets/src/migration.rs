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

use crate::{Config, Pallet, Weight};
use common::{balance, PSWAP};
use frame_support::traits::{Get, GetPalletVersion, PalletVersion};
use traits::MultiCurrency;

pub fn migrate<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    match Pallet::<T>::storage_version() {
        // Initial version is 0.1.0
        // Version 0.2.0 mints 3 billion PSWAP reserved for team
        Some(version) if version == PalletVersion::new(0, 1, 0) => {
            let migrated_weight = mint_team_rewards::<T>().unwrap_or(100_000);
            weight = weight.saturating_add(migrated_weight)
        }
        _ => (),
    }

    weight
}

pub fn mint_team_rewards<T: Config>() -> Option<Weight> {
    let mut weight: Weight = 0;

    let total_reserved = balance!(3000000000);
    let account_id: T::AccountId = T::GetTeamReservesAccountId::get();
    T::Currency::deposit(PSWAP.into(), &account_id, total_reserved).ok()?;
    weight = weight.saturating_add(T::DbWeight::get().writes(1));

    Some(weight)
}
