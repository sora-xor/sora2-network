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

mod mcbc;
mod order_book;
mod permissions;
mod pool_xyk;
mod price_tools;
mod xst;

use assets::AssetIdOf;
use common::{AssetName, AssetSymbol, Balance};
use frame_support::assert_ok;
use frame_support::dispatch::RawOrigin;
use framenode_runtime::qa_tools;
use framenode_runtime::{Runtime, RuntimeEvent};

pub(crate) type FrameSystem = framenode_runtime::frame_system::Pallet<Runtime>;
pub(crate) type QaToolsPallet = qa_tools::Pallet<Runtime>;

pub(crate) fn alice() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([1u8; 32])
}

pub(crate) fn bob() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([2u8; 32])
}

pub(crate) fn charlie() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([3u8; 32])
}

pub(crate) fn dave() -> <Runtime as frame_system::Config>::AccountId {
    <Runtime as frame_system::Config>::AccountId::new([4u8; 32])
}

pub(crate) fn register_custom_asset() -> AssetIdOf<Runtime> {
    assert!(
        frame_system::Pallet::<Runtime>::block_number() >= 1,
        "events are not dispatched at block 0"
    );
    frame_system::Pallet::<Runtime>::inc_providers(&alice());
    assert_ok!(assets::Pallet::<Runtime>::register(
        RawOrigin::Signed(alice()).into(),
        AssetSymbol(b"BP".to_vec()),
        AssetName(b"Black Pepper".to_vec()),
        Balance::from(0u32),
        true,
        false,
        None,
        None
    ));
    let register_event = frame_system::Pallet::<Runtime>::events()
        .last()
        .expect("must've produced an event")
        .event
        .clone();
    let RuntimeEvent::Assets(assets::Event::AssetRegistered(asset_id, _account)) = register_event
    else {
        panic!("Expected asset register event")
    };
    asset_id
}
