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

use super::alice;
use common::{balance, AccountIdOf};
use frame_support::assert_err;
use frame_support::dispatch::RawOrigin;
use framenode_chain_spec::ext;
use framenode_runtime::qa_tools;
use framenode_runtime::Runtime;
use qa_tools::pallet_tools::price_tools::AssetPrices;
use qa_tools::InputAssetId;
use sp_runtime::DispatchError;

fn check_all_extrinsics_are_denied(origin: RawOrigin<AccountIdOf<Runtime>>) {
    assert_err!(
        qa_tools::Pallet::<Runtime>::order_book_create_and_fill_batch(
            origin.clone().into(),
            alice(),
            alice(),
            vec![],
        ),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::order_book_fill_batch(
            origin.clone().into(),
            alice(),
            alice(),
            vec![],
        ),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::xyk_initialize(origin.clone().into(), alice(), vec![],),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::xst_initialize(origin.clone().into(), None, vec![], alice()),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::mcbc_initialize(origin.clone().into(), None, vec![], None,),
        DispatchError::BadOrigin
    );
    assert_err!(
        qa_tools::Pallet::<Runtime>::price_tools_set_asset_price(
            origin.into(),
            AssetPrices {
                buy: balance!(1),
                sell: balance!(1),
            },
            InputAssetId::McbcReference,
        ),
        DispatchError::BadOrigin
    );
}

#[test]
fn should_deny_non_root_callers() {
    ext().execute_with(|| {
        check_all_extrinsics_are_denied(RawOrigin::Signed(alice()));
        check_all_extrinsics_are_denied(RawOrigin::None);
    })
}
