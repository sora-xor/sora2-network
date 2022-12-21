#![allow(deprecated)]

use codec::Encode;
use sp_core::RuntimeDebug;
use sp_std::prelude::*;
use sp_std::vec;

use crate::Config;
use bridge_types::{H160, U256};
use ethabi::{self, Function, Param, ParamType, StateMutability, Token};

fn unlock_function() -> Function {
    Function {
        name: "unlock".into(),
        state_mutability: StateMutability::NonPayable,
        constant: None,
        outputs: vec![],
        inputs: vec![
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

// Message to Ethereum (ABI-encoded)
#[derive(Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub struct OutboundPayload<T: Config> {
    pub sender: T::AccountId,
    pub recipient: H160,
    pub amount: U256,
}

impl<T: Config> OutboundPayload<T> {
    /// ABI-encode this payload
    #[allow(deprecated)] // Avoid error on constant
    pub fn encode(&self) -> Result<Vec<u8>, ethabi::Error> {
        let tokens = vec![
            Token::FixedBytes(self.sender.encode()),
            Token::Address(self.recipient),
            Token::Uint(self.amount),
        ];
        unlock_function().encode_input(tokens.as_ref())
    }
}
