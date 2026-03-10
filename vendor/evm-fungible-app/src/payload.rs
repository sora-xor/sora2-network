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

#![allow(deprecated)]

#[cfg(not(feature = "std"))]
use alloc::string::{String, ToString};

use codec::Encode;
use sp_core::RuntimeDebug;
use sp_std::prelude::*;
use sp_std::vec;

use bridge_types::{H160, H256, U256};
use ethabi::{self, Function, Param, ParamType, StateMutability, Token};

fn unlock_function() -> Function {
    Function {
        name: "unlock".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![
            Param {
                name: "token".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "sender".into(),
                kind: ParamType::FixedBytes(32),
                internal_type: None,
            },
            Param {
                name: "recipient".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "amount".into(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
    }
}

fn register_native_asset_function() -> Function {
    Function {
        name: "createNewToken".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![
            Param {
                name: "name".into(),
                kind: ParamType::String,
                internal_type: None,
            },
            Param {
                name: "symbol".into(),
                kind: ParamType::String,
                internal_type: None,
            },
            Param {
                name: "sidechainAssetId".into(),
                kind: ParamType::FixedBytes(32),
                internal_type: None,
            },
        ],
    }
}

fn add_token_to_whitelist_function() -> Function {
    Function {
        name: "addTokenToWhitelist".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![
            Param {
                name: "token".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "assetType".into(),
                kind: ParamType::Uint(8),
                internal_type: None,
            },
        ],
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RegisterNativeAssetPayload {
    pub asset_id: H256,
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
}

impl RegisterNativeAssetPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = &[
            Token::String(String::from_utf8_lossy(&self.name).to_string()),
            Token::String(String::from_utf8_lossy(&self.symbol).to_string()),
            Token::FixedBytes(self.asset_id.encode()),
        ];
        register_native_asset_function().encode_input(tokens.as_ref())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub enum EthAbiAssetKind {
    _Unregistered = 0,
    Evm = 1,
    _Sora = 2,
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct AddTokenToWhitelistPayload {
    pub address: H160,
    pub asset_kind: EthAbiAssetKind,
}

impl AddTokenToWhitelistPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = &[
            Token::Address(self.address),
            Token::Uint((self.asset_kind as u8).into()),
        ];
        add_token_to_whitelist_function().encode_input(tokens.as_ref())
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct MintPayload<AccountId: Encode> {
    pub token: H160,
    pub sender: AccountId,
    pub recipient: H160,
    pub amount: U256,
}

impl<AccountId: Encode> MintPayload<AccountId> {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = vec![
            Token::Address(self.token),
            Token::FixedBytes(self.sender.encode()),
            Token::Address(self.recipient),
            Token::Uint(self.amount),
        ];
        unlock_function().encode_input(tokens.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex::ToHex;
    use hex_literal::hex;

    #[test]
    fn test_outbound_payload_encode() {
        let payload: MintPayload<[u8; 32]> = MintPayload {
            token: hex!["e1638d0a9f5349bb7d3d748b514b8553dfddb46c"].into(),
            sender: hex!["1aabf8593d9d109b6288149afa35690314f0b798289f8c5c466838dd218a4d50"],
            recipient: hex!["ccb3c82493ac988cebe552779e7195a3a9dc651f"].into(),
            amount: U256::from_str_radix("100", 10).unwrap(), // 1 ETH
        };

        println!("Payload:");
        println!("  {:?}", payload);
        println!("Payload (ABI-encoded):");
        println!("  {:?}", payload.encode().unwrap().to_hex::<String>());
    }
}
