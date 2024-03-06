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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::type_complexity)]

#[macro_use]
extern crate alloc;

pub use {fixnum, paste};

use fixnum::typenum::{Unsigned, U18};
use fixnum::FixedPoint;

#[cfg(any(feature = "test", test))]
pub mod mock;
#[cfg(any(feature = "test", test))]
pub mod test_utils;

mod balance_unit;
pub mod cache_storage;
pub mod eth;
mod fixed_wrapper;
pub mod macros;
pub mod migrations;
mod outcome_fee;
mod primitives;
pub mod serialization;
pub mod storage;
mod swap_amount;
mod traits;
pub mod utils;
pub mod weights;

use codec::Encode;
use sp_core::hash::H512;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::TransactionOutcome;

pub use traits::Config;
pub mod prelude {
    pub use super::balance_unit::*;
    pub use super::fixed_wrapper::*;
    pub use super::outcome_fee::*;
    pub use super::primitives::*;
    pub use super::serialization::*;
    pub use super::swap_amount::*;
    pub use super::traits::*;
    pub use super::weights::*;
    pub use super::{Fixed, FixedInner};
    pub use fixnum;
}
use sp_core::crypto::AccountId32;

pub use macros::*;
pub use primitives::*;
pub use traits::*;
pub use utils::*;

/// Basic type representing asset.
pub type Asset<T, GetAssetId> = currencies::Currency<T, GetAssetId>;

/// Basic type representing assets quantity.
///
/// MAX = (2 ** (BITS_COUNT - 1) - 1) / 10 ** PRECISION =
///     = (2 ** (128 - 1) - 1) / 1e18 =
///     = 170_141_183_460_469_231_731.687_303_715_884_105_727 ~
///     ~ 1.7e20
/// ERROR_MAX = 0.5 / (10 ** PRECISION) =
///           = 0.5 / 1e18 =
///           = 5e-19
pub type Fixed = FixedPoint<FixedInner, FixedPrecision>;
pub type FixedInner = i128;
type FixedPrecision = U18;

pub type Price = Fixed;

pub type Amount = i128;
/// Type definition representing financial basis points (1bp is 0.01%)
pub type BasisPoints = u16;

pub const FIXED_PRECISION: u32 = FixedPrecision::U32;

/// Similar to #\[transactional]
pub fn with_transaction<T, E>(f: impl FnOnce() -> Result<T, E>) -> Result<T, E>
where
    E: From<sp_runtime::DispatchError>,
{
    frame_support::storage::with_transaction(|| {
        let result = f();
        if result.is_ok() {
            TransactionOutcome::Commit(result)
        } else {
            TransactionOutcome::Rollback(result)
        }
    })
}

pub fn hash<T: Encode>(val: &T) -> H512 {
    H512::from_slice(blake2_rfc::blake2b::blake2b(64, &[], &val.encode()).as_bytes())
}

pub fn hash_to_u128_pair<T: Encode>(val: &T) -> (u128, u128) {
    let data = blake2_rfc::blake2b::blake2b(32, &[], &val.encode());
    let bytes = data.as_bytes();
    let mut result: (u128, u128) = (0, 0);
    for i in 0..16 {
        result.0 += (bytes[i] as u128) << (8 * i);
        result.1 += (bytes[i + 16] as u128) << (8 * i);
    }
    result
}

/// Commutative merkle operation, is crypto safe, defined as hash(a,b) `xor` hash(b,a).
pub fn comm_merkle_op<T: Encode>(val_a: &T, val_b: &T) -> H512 {
    use sp_std::ops::BitXor;
    let hash_u = H512::from_slice(
        blake2_rfc::blake2b::blake2b(64, &[], &(val_a, val_b).encode()).as_bytes(),
    );
    let hash_v = H512::from_slice(
        blake2_rfc::blake2b::blake2b(64, &[], &(val_b, val_a).encode()).as_bytes(),
    );
    hash_u.bitxor(hash_v)
}

/// Sorting of keys and values by key with hash_key, useful for crypto sorting with commutative
/// merkle operator.
pub fn sort_with_hash_key<'a, T: Encode, V>(
    hash_key: H512,
    pair_a: (&'a T, &'a V),
    pair_b: (&'a T, &'a V),
) -> ((&'a T, &'a V), (&'a T, &'a V)) {
    use sp_std::ops::BitXor;
    let hash_a = hash(pair_a.0);
    let hash_b = hash(pair_b.0);
    if hash_key.bitxor(hash_a) < hash_key.bitxor(hash_b) {
        (pair_a, pair_b)
    } else {
        (pair_b, pair_a)
    }
}

/// This data is used as prefix in AccountId32, if it is representative for TechAccId encode twox
/// hash (128 + 128 = 256 bit of AccountId32 for example).
pub const TECH_ACCOUNT_MAGIC_PREFIX: [u8; 16] = [
    84, 115, 79, 144, 249, 113, 160, 44, 96, 155, 45, 104, 78, 97, 181, 87,
];

impl IsRepresentation for AccountId32 {
    fn is_representation(&self) -> bool {
        let b: [u8; 32] = self.clone().into();
        b[0..16] == TECH_ACCOUNT_MAGIC_PREFIX
    }
}

type BlockNumberOf<T> = <T as frame_system::Config>::BlockNumber;
type MomentOf<T> = <T as pallet_timestamp::Config>::Moment;
/// Converts block_number to timestamp
pub fn convert_block_number_to_timestamp<T: Config + pallet_timestamp::Config>(
    unlocking_block: BlockNumberOf<T>,
    current_block: BlockNumberOf<T>,
    current_timestamp: MomentOf<T>,
) -> MomentOf<T> {
    if unlocking_block > current_block {
        let num_of_seconds: u32 =
            ((unlocking_block - current_block) * 6u32.into()).unique_saturated_into();
        let mut timestamp: T::Moment = num_of_seconds.into();
        timestamp *= 1000u32.into();
        current_timestamp + timestamp
    } else {
        let num_of_seconds: u32 =
            ((current_block - unlocking_block) * 6u32.into()).unique_saturated_into();
        let mut timestamp: T::Moment = num_of_seconds.into();
        timestamp *= 1000u32.into();
        current_timestamp - timestamp
    }
}
