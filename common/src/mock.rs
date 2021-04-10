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

use crate::{AssetId, AssetId32, Balance, TechAssetId};
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchError;
use orml_traits::parameter_type_with_key;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::convert::TryFrom;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum ComicAssetId {
    GoldenTicket,
    AppleTree,
    Apple,
    Teapot,
    Flower,
    RedPepper,
    BlackPepper,
    AcmeSpyKit,
    BatteryForMusicPlayer,
    MusicPlayer,
    Headphones,
    GreenPromise,
    BluePromise,
}

impl crate::traits::IsRepresentation for ComicAssetId {
    fn is_representation(&self) -> bool {
        false
    }
}

impl From<AssetId> for AssetId32<ComicAssetId> {
    fn from(asset: AssetId) -> Self {
        let comic = ComicAssetId::from(asset);
        AssetId32::<ComicAssetId>::from(comic)
    }
}

impl From<AssetId> for ComicAssetId {
    fn from(asset_id: AssetId) -> Self {
        use ComicAssetId::*;
        match asset_id {
            AssetId::XOR => GoldenTicket,
            AssetId::DOT => AppleTree,
            AssetId::KSM => Apple,
            AssetId::USDT => Teapot,
            AssetId::VAL => Flower,
            AssetId::PSWAP => RedPepper,
            AssetId::DAI => BlackPepper,
        }
    }
}

impl Default for ComicAssetId {
    fn default() -> Self {
        Self::GoldenTicket
    }
}

// This is never used, and just makes some tests compatible.
impl From<AssetId32<AssetId>> for AssetId32<ComicAssetId> {
    fn from(_asset: AssetId32<AssetId>) -> Self {
        unreachable!()
    }
}

// This is never used, and just makes some tests compatible.
impl From<TechAssetId<AssetId>> for AssetId {
    fn from(_tech: TechAssetId<AssetId>) -> Self {
        unimplemented!()
    }
}

// This is never used, and just makes some tests compatible.
impl TryFrom<AssetId> for TechAssetId<TechAssetId<AssetId>>
where
    TechAssetId<AssetId>: Decode,
{
    type Error = DispatchError;
    fn try_from(_asset: AssetId) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl From<AssetId> for TechAssetId<ComicAssetId> {
    fn from(asset_id: AssetId) -> Self {
        TechAssetId::Wrapped(ComicAssetId::from(asset_id))
    }
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId32<AssetId>| -> Balance {
        0
    };
}
