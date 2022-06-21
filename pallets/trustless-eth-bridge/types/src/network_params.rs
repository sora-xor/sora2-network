use crate::{difficulty::DifficultyConfig, EthNetworkId};
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Encode, Decode, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Consensus {
    Ethash { difficulty_config: DifficultyConfig },
    Clique { period: u64, epoch: u64 },
}

#[derive(Copy, Clone, Encode, Decode, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum NetworkConfig {
    Mainnet,
    Ropsten,
    Sepolia,
    Rinkeby,
    Goerli,
    Custom {
        chain_id: EthNetworkId,
        consensus: Consensus,
    },
}

impl NetworkConfig {
    pub fn chain_id(&self) -> EthNetworkId {
        match self {
            NetworkConfig::Mainnet => 1u32.into(),
            NetworkConfig::Ropsten => 3u32.into(),
            NetworkConfig::Sepolia => 11155111u32.into(),
            NetworkConfig::Rinkeby => 4u32.into(),
            NetworkConfig::Goerli => 5u32.into(),
            NetworkConfig::Custom { chain_id, .. } => *chain_id,
        }
    }

    pub fn consensus(&self) -> Consensus {
        match self {
            NetworkConfig::Mainnet => Consensus::Ethash {
                difficulty_config: DifficultyConfig::mainnet(),
            },
            NetworkConfig::Ropsten => Consensus::Ethash {
                difficulty_config: DifficultyConfig::ropsten(),
            },
            NetworkConfig::Sepolia => Consensus::Ethash {
                difficulty_config: DifficultyConfig::sepolia(),
            },
            NetworkConfig::Rinkeby => Consensus::Clique {
                period: 15,
                epoch: 30000,
            },
            NetworkConfig::Goerli => Consensus::Clique {
                period: 15,
                epoch: 30000,
            },
            NetworkConfig::Custom {
                consensus: protocol,
                ..
            } => *protocol,
        }
    }
}
