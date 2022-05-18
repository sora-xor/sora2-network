#![allow(deprecated)]
use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use bridge_types::H160;
use ethabi::{self, Function, Param, ParamType, StateMutability, Token};

fn migrate_erc20_function() -> Function {
    Function {
        name: "migrateNativeErc20".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![
            Param {
                name: "contractAddress".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "erc20nativeTokens".into(),
                kind: ParamType::Array(Box::new(ParamType::Address)),
                internal_type: None,
            },
        ],
    }
}

fn migrate_eth_function() -> Function {
    Function {
        name: "migrateEth".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![Param {
            name: "contractAddress".into(),
            kind: ParamType::Address,
            internal_type: None,
        }],
    }
}

fn migrate_sidechain_function() -> Function {
    Function {
        name: "migrateSidechain".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![
            Param {
                name: "contractAddress".into(),
                kind: ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "sidechainTokens".into(),
                kind: ParamType::Array(Box::new(ParamType::Address)),
                internal_type: None,
            },
        ],
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct MigrateErc20Payload {
    pub contract_address: H160,
    pub erc20_tokens: Vec<H160>,
}

impl MigrateErc20Payload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = vec![
            Token::Address(self.contract_address),
            Token::Array(
                self.erc20_tokens
                    .iter()
                    .map(|token| Token::Address(*token))
                    .collect(),
            ),
        ];
        migrate_erc20_function().encode_input(tokens.as_ref())
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct MigrateSidechainPayload {
    pub contract_address: H160,
    pub sidechain_tokens: Vec<H160>,
}

impl MigrateSidechainPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = vec![
            Token::Address(self.contract_address),
            Token::Array(
                self.sidechain_tokens
                    .iter()
                    .map(|token| Token::Address(*token))
                    .collect(),
            ),
        ];
        migrate_sidechain_function().encode_input(tokens.as_ref())
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct MigrateEthPayload {
    pub contract_address: H160,
}

impl MigrateEthPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = vec![Token::Address(self.contract_address)];
        migrate_eth_function().encode_input(tokens.as_ref())
    }
}
