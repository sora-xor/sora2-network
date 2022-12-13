use crate::difficulty::{ClassicForkConfig, ForkConfig};
use crate::EthNetworkId;
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Encode, Decode, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Consensus {
    Ethash { fork_config: ForkConfig },
    Etchash { fork_config: ClassicForkConfig },
    Clique { period: u64, epoch: u64 },
}

impl Consensus {
    pub fn calc_epoch_length(&self, block_number: u64) -> u64 {
        match self {
            Consensus::Clique { epoch, .. } => *epoch,
            Consensus::Ethash { fork_config } => fork_config.epoch_length(),
            Consensus::Etchash { fork_config } => fork_config.calc_epoch_length(block_number),
        }
    }
}

#[derive(Copy, Clone, Encode, Decode, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum NetworkConfig {
    Mainnet,
    Ropsten,
    Sepolia,
    Rinkeby,
    Goerli,
    Classic,
    Mordor,
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
            NetworkConfig::Classic => 61u32.into(),
            NetworkConfig::Mordor => 63u32.into(),
            NetworkConfig::Custom { chain_id, .. } => *chain_id,
        }
    }

    pub fn consensus(&self) -> Consensus {
        match self {
            NetworkConfig::Mainnet => Consensus::Ethash {
                fork_config: ForkConfig::mainnet(),
            },
            NetworkConfig::Ropsten => Consensus::Ethash {
                fork_config: ForkConfig::ropsten(),
            },
            NetworkConfig::Sepolia => Consensus::Ethash {
                fork_config: ForkConfig::sepolia(),
            },
            NetworkConfig::Classic => Consensus::Etchash {
                fork_config: ClassicForkConfig::classic(),
            },
            NetworkConfig::Mordor => Consensus::Etchash {
                fork_config: ClassicForkConfig::mordor(),
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
