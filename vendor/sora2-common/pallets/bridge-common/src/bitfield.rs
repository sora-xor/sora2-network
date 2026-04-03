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

use core::{
    ops::{Deref, DerefMut},
    usize,
};

use bitvec::{prelude::*, ptr::BitSpanError};
use codec::{Decode, Encode};
use scale_info::prelude::vec::Vec;
use sp_runtime::RuntimeDebug;

pub const SIZE: u128 = core::mem::size_of::<u128>() as u128;

#[derive(
    Encode,
    Decode,
    Clone,
    RuntimeDebug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    scale_info::TypeInfo,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct BitField(pub BitVec<u8, Msb0>);

impl BitField {
    // Constuctors:

    #[inline]
    pub fn with_capacity(len: usize) -> Self {
        Self(BitVec::with_capacity(len))
    }

    #[inline]
    pub fn with_zeroes(len: usize) -> Self {
        Self(BitVec::repeat(false, len))
    }

    #[inline]
    pub fn try_from_slice(slice: &[u8]) -> Result<Self, BitSpanError<u8>> {
        Ok(Self(BitVec::try_from_slice(slice)?))
    }

    pub fn create_bitfield(bits_to_set: &[u32], length: usize) -> Self {
        let mut bitfield = Self::with_zeroes(length);
        for i in bits_to_set {
            bitfield.set(*i as usize);
        }
        bitfield
    }

    pub fn create_random_bitfield(prior: &BitField, n: u32, length: u32, seed: u128) -> Self {
        let mut bitfield = BitField::with_zeroes(prior.len());
        let mut found = 0;
        let mut i = 0;
        while found < n {
            let randomness = sp_io::hashing::blake2_128(&(seed + i).to_be_bytes());

            // length is u32, so mod is u32
            let index = (u128::from_be_bytes(randomness) % length as u128) as u32;

            if !prior.is_set(index as usize) {
                i += 1;
                continue;
            }

            if bitfield.is_set(index as usize) {
                i += 1;
                continue;
            }

            bitfield.set(index as usize);
            found += 1;
            i += 1;
        }
        bitfield
    }

    // Util:
    #[inline]
    pub fn count_set_bits(&self) -> usize {
        self.0.count_ones()
    }

    #[inline]
    pub fn to_bits(self) -> Vec<u8> {
        self.0.into_vec()
    }

    #[inline]
    pub fn set(&mut self, index: usize) {
        self.0.set(index, true)
    }

    #[inline]
    pub fn clear(&mut self, index: usize) {
        self.0.set(index, false)
    }

    #[inline]
    pub fn is_set(&self, index: usize) -> bool {
        self.0[index]
    }
}

impl Deref for BitField {
    type Target = BitVec<u8, Msb0>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BitField {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(test)]
mod test {
    #[test]
    pub fn create_bitfield_success() {
        let bits_to_set = vec![0, 1, 2];
        let len = 3;
        let bf = super::BitField::create_bitfield(&bits_to_set, len);
        assert!(bf[0]);
        assert!(bf[1]);
        assert!(bf[2]);
    }
}
