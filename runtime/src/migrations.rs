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

use common::Balance;
use frame_support::{dispatch::DispatchResult, traits::OnRuntimeUpgrade};
use sp_runtime::BoundedVec;

pub type Migrations = (DenominateVXor,);

const DENOM_COEFF: Balance = 100_000_000_000_000;

pub struct DenominateVXor;

impl OnRuntimeUpgrade for DenominateVXor {
    fn on_runtime_upgrade() -> frame_election_provider_support::Weight {
        let result = common::with_transaction(|| {
            let mut new_issuance = 0;
            tokens::Accounts::<crate::Runtime>::translate::<tokens::AccountData<Balance>, _>(
                |account, asset_id, mut data| {
                    if asset_id == common::VXOR {
                        let before = data.free;
                        data.free /= DENOM_COEFF;
                        data.reserved /= DENOM_COEFF;
                        data.frozen /= DENOM_COEFF;
                        new_issuance += data.free;
                        log::debug!(
                            "Denominated balance of {:?}, balance:  {} => {}",
                            account,
                            before,
                            data.free
                        );
                    }
                    Some(data)
                },
            );
            tokens::Locks::<crate::Runtime>::translate::<
                BoundedVec<tokens::BalanceLock<Balance>, crate::MaxLocksTokens>,
                _,
            >(|account, asset_id, mut locks| {
                if asset_id == common::VXOR {
                    for lock in locks.iter_mut() {
                        lock.amount /= DENOM_COEFF;
                    }
                    log::debug!("Denominated locks of {:?}", account);
                }
                Some(locks)
            });
            tokens::TotalIssuance::<crate::Runtime>::mutate(common::VXOR, |issuance| {
                *issuance = new_issuance;
            });

            for (dex_id, dex_info) in dex_manager::DEXInfos::<crate::Runtime>::iter() {
                if dex_info.base_asset_id == common::VXOR {
                    for (target_asset_id, (pool_account, _fee_account)) in
                        pool_xyk::Properties::<crate::Runtime>::iter_prefix(dex_info.base_asset_id)
                    {
                        pool_xyk::Pallet::<crate::Runtime>::fix_pool_parameters(
                            dex_id,
                            &pool_account,
                            &dex_info.base_asset_id,
                            &target_asset_id,
                        )?;
                    }
                } else if let Some((pool_account, _fee_account)) =
                    pool_xyk::Properties::<crate::Runtime>::get(
                        &dex_info.base_asset_id,
                        &common::VXOR,
                    )
                {
                    pool_xyk::Pallet::<crate::Runtime>::fix_pool_parameters(
                        dex_id,
                        &pool_account,
                        &dex_info.base_asset_id,
                        &common::VXOR,
                    )?;
                }
            }

            DispatchResult::Ok(())
        });
        if let Err(err) = result {
            log::info!("Failed to denominate VXOR, reverting...: {:?}", err);
        }
        crate::BlockWeights::get().max_block
    }
}
