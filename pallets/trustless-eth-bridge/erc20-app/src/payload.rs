use codec::Encode;
use sp_core::RuntimeDebug;
use sp_std::prelude::*;

use bridge_types::{H160, H256, U256};
use ethabi::{self, Token};

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RegisterNativeAssetPayload {
    pub asset_id: H256,
    pub decimals: u8,
    pub name: Vec<u8>,
    pub symbol: Vec<u8>,
}

impl RegisterNativeAssetPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Vec<u8> {
        ethabi::encode_function(
            "registerAsset(string,string,uint256,bytes32)",
            &[
                Token::String(self.name.clone()),
                Token::String(self.symbol.clone()),
                Token::Uint(U256::from(self.decimals)),
                Token::FixedBytes(self.asset_id.encode()),
            ],
        )
    }
}

// Message to Ethereum (ABI-encoded)
#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
pub struct RegisterErc20AssetPayload {
    pub address: H160,
}

impl RegisterErc20AssetPayload {
    /// ABI-encode this payload
    pub fn encode(&self) -> Vec<u8> {
        ethabi::encode_function(
            "registerAsset(address)",
            &[Token::Address(self.address.clone())],
        )
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
    pub fn encode(&self) -> Vec<u8> {
        let tokens = vec![
            Token::Address(self.token),
            Token::FixedBytes(self.sender.encode()),
            Token::Address(self.recipient),
            Token::Uint(self.amount),
        ];
        ethabi::encode_function("unlock(address,bytes32,address,uint256)", tokens.as_ref())
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
        println!("  {:?}", payload.encode().to_hex::<String>());
    }
}
