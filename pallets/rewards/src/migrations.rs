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

pub mod v1_2 {
    use crate::{Config, EthAddress, PswapFarmOwners, ReservesAcc, RewardInfo, Weight};
    use common::{balance, Balance, PSWAP};
    use frame_support::debug;
    use frame_support::dispatch::DispatchError;
    use frame_support::traits::Get;
    use hex_literal::hex;
    use orml_traits::MultiCurrency;
    use sp_std::collections::btree_map::BTreeMap;
    use sp_std::vec::Vec;

    // Migrate to version 1.2.0
    pub fn migrate<T: Config>() -> Weight {
        debug::RuntimeLogger::init();

        (0 as Weight)
            .saturating_add(add_lost_pswap::<T>())
            .saturating_add(migrate_val_owners::<T>())
    }

    // Version 1.2.0 adds lost tokens compensation for user according to
    // https://etherscan.io/tx/0x5605564eadc8b912de930fb9e3405b0aa1010cf3decc0eace176b6cf5aeee166
    pub fn add_lost_pswap<T: Config>() -> Weight {
        let user_account =
            EthAddress::from_slice(&hex!("e687c6c6b28745864871566134b5589aa05b953d"));
        let compensation_amount = balance!(74339.224845900297630556);
        PswapFarmOwners::<T>::insert(user_account, compensation_amount);
        let reserves_tech_acc = ReservesAcc::<T>::get();
        let reserves_acc =
            technical::Module::<T>::tech_account_id_to_account_id(&reserves_tech_acc);
        let mut error: Option<(&'static str, DispatchError)> = Default::default();
        if reserves_acc.is_err() {
            let header_message: &'static str =
                "failed to convert from tech account during migration";
            error = Some((header_message, reserves_acc.unwrap_err()));
        } else {
            let res =
                T::Currency::deposit(PSWAP.into(), &reserves_acc.unwrap(), compensation_amount);
            if res.is_err() {
                let header_message: &'static str =
                    "failed to mint compensation pswap during migration";
                error = Some((header_message, res.unwrap_err()));
            } else {
                debug::info!(
                    target: "runtime",
                    "successfully minted compensation pswap during migration"
                );
            }
        }

        if let Some((header, err)) = error {
            match err {
                DispatchError::Module {
                    index,
                    error,
                    message,
                } => debug::error!(
                    target: "runtime",
                    "{}: Module{{index: {}, error: {}, message: {}}}", header, index, error, message.unwrap_or("")
                ),
                _ => debug::error!(
                    target: "runtime",
                    "{}: Non-Module Error", header,
                ),
            }
        }

        // Approximate weight
        T::DbWeight::get().reads_writes(2, 2)
    }

    // Version 1.2.0 changes the type of the `ValOwners` struct so that the latter now
    // contains a `(claimable, total)` rewards pair for each ERC20 XOR holder address.
    // Additional storage variables are introduced to be used in strategic VAL vesting.
    pub fn migrate_val_owners<T: Config>() -> Weight {
        let mut weight: Weight = 0;

        // Change value type in ValOwners map from Balance -> RewardInfo
        let mut total = balance!(0);
        crate::ValOwners::<T>::translate_values::<Balance, _>(|v| {
            total += v;
            Some(RewardInfo::new(v, v))
        });

        let val_owners = crate::ValOwners::<T>::iter().collect::<Vec<_>>();

        // Split the addresses in groups to avoid processing all rewards within a single block
        let mut iter = val_owners.chunks(T::MAX_CHUNK_SIZE);
        let mut batch_index: u32 = 0;
        while let Some(chunk) = iter.next() {
            crate::EthAddresses::<T>::insert(
                batch_index,
                chunk
                    .iter()
                    .cloned()
                    .map(|(addr, _)| addr)
                    .collect::<Vec<_>>(),
            );
            batch_index += 1;
        }

        crate::TotalValRewards::<T>::put(total);
        crate::TotalClaimableVal::<T>::put(total);
        crate::CurrentClaimableVal::<T>::put(balance!(0));
        crate::ValBurnedSinceLastVesting::<T>::put(balance!(0));
        crate::MigrationPending::<T>::put(true);

        debug::info!(
            target: "runtime",
            "Storage for VAL rewards data successflly migrated"
        );

        // The exact weight of the StorageMap::translate_values() is unknown
        // Since runtime upgrade is executed regardless the weight we can use approximate value
        weight = weight.saturating_add(T::DbWeight::get().writes(1000));

        weight
    }

    // This function is called inside the `finalize_storage_migration` extrinsic
    // to complete storage migration from the pallet version 1.1.0 to version 1.2.0.
    // Since ver. 1.1.0 all the former ERC20 XOR holders' addresses are in the `ValOwners`.
    // Therefore only existing entires of `ValOwners` storage map are allowed to be updated.
    // No new entries should be created in the `ValOwners` map should there be
    // a pair in the `amounts` whose key is not already in the storage map.
    pub fn update_val_owners<T: Config>(amounts: Vec<(EthAddress, Balance)>) {
        let amounts = amounts
            .iter()
            .cloned()
            .collect::<BTreeMap<EthAddress, Balance>>();

        let mut total = balance!(0);
        crate::ValOwners::<T>::translate::<RewardInfo, _>(|addr, info| {
            let t = match amounts.get(&addr) {
                Some(r) => info.total.saturating_add(*r),
                None => info.total,
            };

            total += t;
            Some(RewardInfo::new(info.claimable, t))
        });

        crate::TotalValRewards::<T>::put(total);
        crate::MigrationPending::<T>::put(false);
    }
}

pub mod v1_3 {
    use crate::{Config, EthereumAddress, Weight};
    use frame_support::debug;
    use frame_support::traits::Get;
    use serde::Deserialize;
    use serde_json;
    use sp_std::vec::Vec;

    // we don't use include! with rust code, because it takes ages to compile. It's a lot faster
    // with on-runtime serde.
    const NFTS_RECEIVERS: &'static str = include_str!("bytes/ETH-ERC20-Ukraine.json");

    // Migrate to version 1.3.0
    pub fn migrate<T: Config>() -> Weight {
        debug::RuntimeLogger::init();
        prepare_umi_nft_rewards::<T>()
    }

    fn prepare_umi_nft_rewards<T: Config>() -> Weight {
        let receivers = serde_json::from_str::<Vec<NftReceiver>>(NFTS_RECEIVERS)
            .expect("Can't deserialize UMI NFT receivers.");

        let writes_num = receivers.len() as u64;
        receivers.into_iter().for_each(|receiver| {
            crate::UmiNftOwners::<T>::insert(
                EthereumAddress::from_slice(
                    &hex::decode(&receiver.address[2..])
                        .expect("Can't decode EthereumAddress from String"),
                ),
                vec![1, 1, 1],
            )
        });

        T::DbWeight::get().writes(writes_num)
    }

    #[derive(Deserialize, Debug)]
    struct NftReceiver {
        #[serde(rename = "Address")]
        address: String,
    }
}
