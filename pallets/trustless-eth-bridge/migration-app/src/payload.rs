use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use bridge_types::H160;
use ethabi::{self, Token};

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct MigrateErc20Payload {
    pub contract_address: H160,
    pub erc20_tokens: Vec<H160>,
}

impl MigrateErc20Payload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Vec<u8> {
        let tokens = vec![
            Token::Address(self.contract_address),
            Token::Array(
                self.erc20_tokens
                    .iter()
                    .map(|token| Token::Address(*token))
                    .collect(),
            ),
        ];
        ethabi::encode_function("migrateNativeErc20(address,address[])", tokens.as_ref())
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
    pub fn encode(&self) -> Vec<u8> {
        let tokens = vec![
            Token::Address(self.contract_address),
            Token::Array(
                self.sidechain_tokens
                    .iter()
                    .map(|token| Token::Address(*token))
                    .collect(),
            ),
        ];
        ethabi::encode_function("migrateSidechain(address,address[])", tokens.as_ref())
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct MigrateEthPayload {
    pub contract_address: H160,
}

impl MigrateEthPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Vec<u8> {
        let tokens = vec![Token::Address(self.contract_address)];
        ethabi::encode_function("migrateEth(address)", tokens.as_ref())
    }
}
