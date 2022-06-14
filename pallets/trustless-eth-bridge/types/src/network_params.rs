use crate::{difficulty::DifficultyConfig, EthNetworkId};
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Encode, Decode, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Protocol {
    Ethash,
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
        difficulty_config: DifficultyConfig,
        protocol: Protocol,
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

    pub const fn difficulty_config(&self) -> DifficultyConfig {
        match self {
            NetworkConfig::Mainnet => DifficultyConfig {
                byzantium_fork_block: 4_370_000,
                constantinople_fork_block: 7_280_000,
                muir_glacier_fork_block: 9_200_000,
                london_fork_block: 12_965_000,
                arrow_glacier_fork_block: 13_773_000,
            },
            NetworkConfig::Ropsten => DifficultyConfig {
                byzantium_fork_block: 1_700_000,
                constantinople_fork_block: 4_230_000,
                muir_glacier_fork_block: 7_117_117,
                london_fork_block: 10_499_401,
                arrow_glacier_fork_block: u64::max_value(),
            },
            NetworkConfig::Sepolia => DifficultyConfig {
                byzantium_fork_block: 0,
                constantinople_fork_block: 0,
                muir_glacier_fork_block: 0,
                london_fork_block: 0,
                arrow_glacier_fork_block: u64::max_value(),
            },
            NetworkConfig::Rinkeby => DifficultyConfig {
                byzantium_fork_block: 1_035_301,
                constantinople_fork_block: 3_660_663,
                muir_glacier_fork_block: 8_290_928,
                london_fork_block: 8_897_988,
                arrow_glacier_fork_block: u64::max_value(),
            },
            NetworkConfig::Goerli => DifficultyConfig {
                byzantium_fork_block: 0,
                constantinople_fork_block: 0,
                muir_glacier_fork_block: 4_460_644,
                london_fork_block: 5_062_605,
                arrow_glacier_fork_block: u64::max_value(),
            },
            NetworkConfig::Custom {
                difficulty_config, ..
            } => *difficulty_config,
        }
    }

    pub fn consensus(&self) -> Protocol {
        match self {
            NetworkConfig::Mainnet => Protocol::Ethash,
            NetworkConfig::Ropsten => Protocol::Ethash,
            NetworkConfig::Sepolia => Protocol::Ethash,
            NetworkConfig::Rinkeby => Protocol::Clique {
                period: 15,
                epoch: 30000,
            },
            NetworkConfig::Goerli => Protocol::Clique {
                period: 15,
                epoch: 30000,
            },
            NetworkConfig::Custom { protocol, .. } => *protocol,
        }
    }
}
