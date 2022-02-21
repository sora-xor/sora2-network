use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use bridge_types::H160;
use ethabi::{self, Token};

// Message to Ethereum (ABI-encoded)
#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RegisterOperatorPayload {
    pub operator: H160,
}

impl RegisterOperatorPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Vec<u8> {
        let tokens = vec![Token::Address(self.operator)];
        ethabi::encode_function("authorizeDefaultOperator(address)", tokens.as_ref())
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct DeregisterOperatorPayload {
    pub operator: H160,
}

impl DeregisterOperatorPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Vec<u8> {
        let tokens = vec![Token::Address(self.operator)];
        ethabi::encode_function("revokeDefaultOperator(address)", tokens.as_ref())
    }
}
