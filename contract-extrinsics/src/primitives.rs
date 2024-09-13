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
// pub const ASSET_ID_PREFIX_PREDEFINED: u8 = 2;
//

// TODO: For build use in the way, but it is duplicate, so need try use pallets::order_book::src::types (maybe make `types` - package?)
#[derive(Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug, Hash)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub struct OrderBookId<AssetId, DEXId> {
    /// DEX id
    pub dex_id: DEXId,
    /// Base asset.
    pub base: AssetId,
    /// Quote asset. It should be a base asset of DEX.
    pub quote: AssetId,
}

pub type OrderId = u128;

// pub type DEXId = u32;
// pub type Balance = u128;
//
// /// This code is H256 like.
// pub type AssetId32Code = [u8; 32];
//
// /// This is wrapped structure, this is like H256 or Р512, extra
// /// PhantomData is added for typing reasons.
// #[derive(Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
// #[ink::scale_derive(Encode, Decode, TypeInfo)]
// #[cfg_attr(feature = "std", derive(Hash))]
// pub struct AssetId32 {
//     /// Internal data representing given AssetId.
//     pub code: AssetId32Code,
// }
//
// impl AssetId32 {
//     pub const fn new(code: AssetId32Code) -> Self {
//         Self { code }
//     }
//
//     pub const fn from_bytes(bytes: [u8; 32]) -> Self {
//         Self { code: bytes }
//     }
// }
//
// #[derive(PartialEq, Eq, Copy, Clone)]
// #[ink::scale_derive(Encode, Decode, TypeInfo)]
// pub enum PriceVariant {
//     Buy,
//     Sell,
// }
//
// impl PriceVariant {
//     pub fn switched(&self) -> Self {
//         match self {
//             PriceVariant::Buy => PriceVariant::Sell,
//             PriceVariant::Sell => PriceVariant::Buy,
//         }
//     }
// }
//
