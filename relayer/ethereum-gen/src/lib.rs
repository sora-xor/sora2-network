// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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

pub mod eth_bridge {
    pub use crate::bridge_mod::*;
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
