ethers::contract::abigen!(
    BasicInboundChannel,
    "src/bytes/BasicInboundChannel.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    BasicOutboundChannel,
    "src/bytes/BasicOutboundChannel.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    IncentivizedInboundChannel,
    "src/bytes/IncentivizedInboundChannel.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    IncentivizedOutboundChannel,
    "src/bytes/IncentivizedOutboundChannel.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    BeefyLightClient,
    "src/bytes/BeefyLightClient.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    ETHApp,
    "src/bytes/ETHApp.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    ERC20App,
    "src/bytes/ERC20App.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    SidechainApp,
    "src/bytes/SidechainApp.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    ValidatorRegistry,
    "src/bytes/ValidatorRegistry.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    IERC20Metadata,
    "src/bytes/IERC20Metadata.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    TestToken,
    "src/bytes/TestToken.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    Bridge,
    "src/bytes/Bridge.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    WCFG,
    "src/bytes/WCFG.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    Master,
    "src/bytes/Master.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
);

// Re-export modules, because it's private

pub mod basic_inbound_channel {
    pub use crate::basicinboundchannel_mod::*;
}

pub mod basic_outbound_channel {
    pub use crate::basicoutboundchannel_mod::*;
}

pub mod incentivized_outbound_channel {
    pub use crate::incentivizedoutboundchannel_mod::*;
}

pub mod incentivized_inbound_channel {
    pub use crate::incentivizedinboundchannel_mod::*;
}

pub mod beefy_light_client {
    pub use crate::beefylightclient_mod::*;
}

pub mod erc20_app {
    pub use crate::erc20app_mod::*;
}

pub mod eth_app {
    pub use crate::ethapp_mod::*;
}

pub mod sidechain_app {
    pub use crate::sidechainapp_mod::*;
}

pub mod validator_registry {
    pub use crate::validatorregistry_mod::*;
}

pub mod eth_bridge {
    pub use crate::bridge_mod::*;
}

pub mod wcfg {
    pub use crate::wcfg_mod::*;
}

pub mod ierc20 {
    pub use crate::ierc20metadata_mod::*;
}

pub mod master {
    pub use crate::master_mod::*;
}

pub mod test_token {
    pub use crate::testtoken_mod::*;
}
