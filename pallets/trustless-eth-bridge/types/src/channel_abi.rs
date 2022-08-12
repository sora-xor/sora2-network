#![allow(deprecated)]

use ethabi::{Function, Param, ParamType, StateMutability, Token};
use ethereum_types::H160;
use frame_support::RuntimeDebug;
use sp_std::prelude::*;

fn authorize_operator_function() -> Function {
    Function {
        name: "authorizeDefaultOperator".into(),
        constant: None,
        state_mutability: StateMutability::NonPayable,
        outputs: vec![],
        inputs: vec![Param {
            name: "operator".into(),
            kind: ParamType::Address,
            internal_type: None,
        }],
    }
}

fn revoke_operator_function() -> Function {
    Function {
        name: "revokeDefaultOperator".into(),
        constant: None,
        state_mutability: StateMutability::NonPayable,
        outputs: vec![],
        inputs: vec![Param {
            name: "operator".into(),
            kind: ParamType::Address,
            internal_type: None,
        }],
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct DeregisterOperatorPayload {
    pub operator: H160,
}

impl DeregisterOperatorPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = &[Token::Address(self.operator)];
        revoke_operator_function().encode_input(tokens.as_ref())
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RegisterOperatorPayload {
    pub operator: H160,
}

impl RegisterOperatorPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = &[Token::Address(self.operator)];
        authorize_operator_function().encode_input(tokens.as_ref())
    }
}
