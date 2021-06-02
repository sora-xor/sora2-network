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

use crate::{Config, EthereumAddress, Pallet, Weight};
use common::prelude::Balance;
use common::{balance, vec_push, VAL};
use frame_support::debug;
use frame_support::traits::{Get, GetPalletVersion, PalletVersion};
use hex_literal::hex;
use orml_traits::currency::MultiCurrency;
use sp_std::vec::Vec;

pub fn migrate<T: Config>() -> Weight {
    let mut weight: Weight = 0;

    match Pallet::<T>::storage_version() {
        // Initial version is 0.1.0 with the storage initialized in genesis block
        // Version 1.1.0 updates claimable VAL data structure with the corrected data
        Some(version) if version == PalletVersion::new(0, 1, 0) => {
            // First, necessary amount of VAL must be minted and deposited to the rewards tech account
            // Amount of VAL rewards minted in genesis block
            let minted_rewards = balance!(3725659.407846136445216435);
            weight = weight.saturating_add(mint_remaining_val::<T>(minted_rewards));

            // Adjustment that needs to be applied to the airdrop data
            // In order to avoid stack overflow during WASM build while having to handle
            // large volumes of data, we split the adjustment data into two chunks
            let data_0 = include!("bytes/val_rewards_airdrop_adjustment.0.in");
            weight = weight.saturating_add(update_val_airdrop_data::<T>(data_0));
            let data_1 = include!("bytes/val_rewards_airdrop_adjustment.1.in");
            weight = weight.saturating_add(update_val_airdrop_data::<T>(data_1));
        }
        _ => (),
    }

    weight
}

pub fn mint_remaining_val<T: Config>(already_minted: Balance) -> Weight {
    let rewards_tech_acc = crate::ReservesAcc::<T>::get();
    let rewards_account_id =
        technical::Module::<T>::tech_account_id_to_account_id(&rewards_tech_acc).unwrap();

    // Total claimable amount of VAL to be distributed among ERC20 XOR holders is 33,100,000
    // (https://medium.com/sora-xor/sora-v2-implementation-1febd3260b87)
    let total_claimable = balance!(33100000.0);

    let offset: Balance = total_claimable.saturating_sub(already_minted);
    T::Currency::deposit(VAL.into(), &rewards_account_id, offset.clone()).unwrap();
    debug::RuntimeLogger::init();
    debug::info!(
        "Minted remaining {} VAL for ERC20 XOR holders rewards vesting",
        offset
    );
    T::DbWeight::get().reads_writes(2, 1)
}

pub fn update_val_airdrop_data<T: Config>(
    adjustment_data: Vec<(EthereumAddress, Balance)>,
) -> Weight {
    let mut weight: Weight = 0;

    for (addr, diff) in adjustment_data {
        if diff == balance!(0) {
            continue;
        }
        crate::ValOwners::<T>::mutate(addr, |v| *v = v.saturating_add(diff));
        weight = weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
    }

    // Remove entries for known liquidity pools addresses (Uniswap, Mooniswap)
    crate::ValOwners::<T>::mutate_exists(
        EthereumAddress::from(hex!("01962144d41415cca072900fe87bbe2992a99f10")),
        |v| *v = None,
    );
    crate::ValOwners::<T>::mutate_exists(
        EthereumAddress::from(hex!("b90d8c0c2ace705fad8ad7e447dcf3e858c20448")),
        |v| *v = None,
    );
    crate::ValOwners::<T>::mutate_exists(
        EthereumAddress::from(hex!("4fd3f9811224bf5a87bbaf002a345560c2d98d76")),
        |v| *v = None,
    );
    crate::ValOwners::<T>::mutate_exists(
        EthereumAddress::from(hex!("215470102a05b02a3a2898f317b5382f380afc0e")),
        |v| *v = None,
    );
    weight = weight.saturating_add(T::DbWeight::get().reads(4));

    weight
}
