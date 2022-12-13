ethers::contract::abigen!(
    InboundChannel,
    "src/bytes/InboundChannel.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    OutboundChannel,
    "src/bytes/OutboundChannel.abi.json",
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
    IERC20Metadata,
    "src/bytes/IERC20Metadata.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    TestToken,
    "src/bytes/TestToken.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    Bridge,
    "src/bytes/Bridge.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    Master,
    "src/bytes/Master.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
    MigrationApp,
    "src/bytes/MigrationApp.abi.json",
    event_derives (serde::Deserialize, serde::Serialize);
);

// Re-export modules, because it's private

pub mod outbound_channel {
    pub use crate::outboundchannel_mod::*;
}

pub mod inbound_channel {
    pub use crate::inboundchannel_mod::*;
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

pub mod ierc20 {
    pub use crate::ierc20metadata_mod::*;
}

pub mod master {
    pub use crate::master_mod::*;
}

pub mod test_token {
    pub use crate::testtoken_mod::*;
}

pub mod migration_app {
    pub use crate::migrationapp_mod::*;
}
