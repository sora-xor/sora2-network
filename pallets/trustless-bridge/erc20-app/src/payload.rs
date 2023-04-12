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
                name: "_token".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "_sender".into(),
                kind: ParamType::FixedBytes(32),
                internal_type: None,
            },
            Param {
                name: "_recipient".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "_amount".into(),
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

fn register_erc20_asset_function() -> Function {
    Function {
        name: "addTokenToWhitelist".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![Param {
            name: "token".into(),
            kind: ParamType::Address,
            internal_type: None,
        }],
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

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RegisterErc20AssetPayload {
    pub address: H160,
}

impl RegisterErc20AssetPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = &[Token::Address(self.address.clone())];
        register_erc20_asset_function().encode_input(tokens.as_ref())
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
