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

use crate::Balance;
use secp256k1::{Message, PublicKey};
use sp_core::{H160, U256};
use sp_io::hashing::keccak_256;
use sp_runtime::traits::CheckedConversion;

pub type EthereumAddress = H160;

pub fn public_key_to_eth_address(pub_key: &PublicKey) -> EthereumAddress {
    let hash = keccak_256(&pub_key.serialize()[1..]);
    EthereumAddress::from_slice(&hash[12..])
}

pub fn prepare_message(msg: &[u8]) -> Message {
    let msg = keccak_256(msg);
    let mut prefix = b"\x19Ethereum Signed Message:\n32".to_vec();
    prefix.extend(&msg);
    let hash = keccak_256(&prefix);
    frame_support::log::error!(
        "Prepare message: {}, {}",
        hex::encode(&msg),
        hex::encode(&hash)
    );
    Message::parse_slice(&hash).expect("hash size == 256 bits; qed")
}

fn granularity(decimals: u32) -> Option<U256> {
    Some(U256::from(u64::checked_pow(10, 18 - decimals)?))
}

pub fn unwrap_balance(value: U256, decimals: u32) -> Option<Balance> {
    let granularity = match granularity(decimals) {
        Some(value) => value,
        None => return None,
    };

    let unwrapped = match value.checked_div(granularity) {
        Some(value) => value,
        None => return None,
    };

    unwrapped.low_u128().checked_into()
}

pub fn wrap_balance(value: Balance, decimals: u32) -> Option<U256> {
    let granularity = match granularity(decimals) {
        Some(value) => value,
        None => return None,
    };

    let value_u256 = match value.checked_into::<u128>() {
        Some(value) => U256::from(value),
        None => return None,
    };

    value_u256.checked_mul(granularity)
}
