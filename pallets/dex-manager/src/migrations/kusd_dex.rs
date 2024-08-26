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

use crate::pallet::{Config, Pallet};
use crate::DEXInfos;
use common::{DEXId, DEXInfo, DexIdOf, KUSD, XST};
use core::marker::PhantomData;
use frame_support::pallet_prelude::Get;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::{traits::GetStorageVersion as _, weights::Weight};
use log::info;

pub struct AddKusdBasedDex<T>(PhantomData<T>);

impl<T: Config> OnRuntimeUpgrade for AddKusdBasedDex<T> {
    fn on_runtime_upgrade() -> Weight {
        if Pallet::<T>::on_chain_storage_version() != 2 {
            info!("Migration with KUSD based DEX is available only for version 2");
            return Weight::zero();
        }

        let dex_id: DexIdOf<T> = DEXId::PolkaswapKUSD.into();

        let reads = 1;
        let mut writes = 0;
        if !DEXInfos::<T>::contains_key(dex_id) {
            DEXInfos::<T>::insert(
                dex_id,
                DEXInfo {
                    base_asset_id: KUSD.into(),
                    synthetic_base_asset_id: XST.into(),
                    is_public: true,
                },
            );
            writes += 1;
        }

        T::DbWeight::get().reads_writes(reads, writes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{ExtBuilder, Runtime};
    use common::{XOR, XSTUSD};
    use frame_support::pallet_prelude::StorageVersion;

    #[test]
    fn test_kusd_dex() {
        let mut ext = ExtBuilder {
            initial_dex_list: vec![
                (
                    DEXId::Polkaswap,
                    DEXInfo {
                        base_asset_id: XOR,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    DEXId::PolkaswapXSTUSD,
                    DEXInfo {
                        base_asset_id: XSTUSD,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
            ],
            ..Default::default()
        }
        .build();
        ext.execute_with(|| {
            StorageVersion::new(2).put::<Pallet<Runtime>>();

            let mut dex_infos: Vec<_> = DEXInfos::<Runtime>::iter().collect();
            dex_infos.sort_by(|(left_dex_id, _), (right_dex_id, _)| left_dex_id.cmp(right_dex_id));

            assert_eq!(
                dex_infos,
                vec![
                    (
                        DEXId::Polkaswap,
                        DEXInfo {
                            base_asset_id: XOR,
                            synthetic_base_asset_id: XST,
                            is_public: true,
                        },
                    ),
                    (
                        DEXId::PolkaswapXSTUSD,
                        DEXInfo {
                            base_asset_id: XSTUSD,
                            synthetic_base_asset_id: XST,
                            is_public: true,
                        },
                    ),
                ]
            );

            // migration
            AddKusdBasedDex::<Runtime>::on_runtime_upgrade();

            let mut dex_infos: Vec<_> = DEXInfos::<Runtime>::iter().collect();
            dex_infos.sort_by(|(left_dex_id, _), (right_dex_id, _)| left_dex_id.cmp(right_dex_id));
            assert_eq!(
                dex_infos,
                vec![
                    (
                        DEXId::Polkaswap,
                        DEXInfo {
                            base_asset_id: XOR,
                            synthetic_base_asset_id: XST,
                            is_public: true,
                        },
                    ),
                    (
                        DEXId::PolkaswapXSTUSD,
                        DEXInfo {
                            base_asset_id: XSTUSD,
                            synthetic_base_asset_id: XST,
                            is_public: true,
                        },
                    ),
                    (
                        DEXId::PolkaswapKUSD,
                        DEXInfo {
                            base_asset_id: KUSD,
                            synthetic_base_asset_id: XST,
                            is_public: true,
                        },
                    ),
                ]
            );

            // storage version should not change
            assert_eq!(Pallet::<Runtime>::on_chain_storage_version(), 2);
        });
    }
}
