mod error;
mod estimate_gas;
mod ethereum_relay;
mod fetch_ethereum_header;
mod mint_test_token;
mod register_bridge;
mod register_erc20_app;
mod register_erc20_asset;
mod subscribe_beefy;
mod substrate_relay;
mod transfer_to_ethereum;
mod transfer_to_sora;

use error::*;

use crate::prelude::*;
use clap::*;

/// App struct
#[derive(Parser, Debug)]
#[clap(version, author)]
pub struct Cli {
    #[clap(subcommand)]
    commands: Commands,
}

impl Cli {
    pub async fn run(&self) -> AnyResult<()> {
        self.commands.run().await
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    SubscribeBeefy(subscribe_beefy::Command),
    FetchEthereumHeader(fetch_ethereum_header::Command),
    EthereumRelay(ethereum_relay::Command),
    SubstrateRelay(substrate_relay::Command),
    TransferToSora(transfer_to_sora::Command),
    TransferToEthereum(transfer_to_ethereum::Command),
    RegisterBridge(register_bridge::Command),
    RegisterErc20App(register_erc20_app::Command),
    RegisterErc20Asset(register_erc20_asset::Command),
    EstimateGas(estimate_gas::Command),
    MintTestToken(mint_test_token::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::SubscribeBeefy(cmd) => cmd.run().await,
            Self::SubstrateRelay(cmd) => cmd.run().await,
            Self::FetchEthereumHeader(cmd) => cmd.run().await,
            Self::EthereumRelay(cmd) => cmd.run().await,
            Self::TransferToSora(cmd) => cmd.run().await,
            Self::TransferToEthereum(cmd) => cmd.run().await,
            Self::RegisterBridge(cmd) => cmd.run().await,
            Self::RegisterErc20App(cmd) => cmd.run().await,
            Self::RegisterErc20Asset(cmd) => cmd.run().await,
            Self::EstimateGas(cmd) => cmd.run().await,
            Self::MintTestToken(cmd) => cmd.run().await,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct SubstrateKey {
    #[clap(long)]
    substrate_key: Option<String>,
    #[clap(long)]
    substrate_key_file: Option<String>,
}

impl SubstrateKey {
    pub fn get_key_string(&self) -> AnyResult<String> {
        match (&self.substrate_key, &self.substrate_key_file) {
            (Some(_), Some(_)) => Err(CliError::BothKeyTypesProvided.into()),
            (None, None) => Err(CliError::KeyNotProvided.into()),
            (Some(key), _) => Ok(key.clone()),
            (_, Some(key_file)) => Ok(std::fs::read_to_string(key_file)?),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct EthereumKey {
    #[clap(long)]
    ethereum_key: Option<String>,
    #[clap(long)]
    ethereum_key_file: Option<String>,
}

impl EthereumKey {
    pub fn get_key_string(&self) -> AnyResult<String> {
        match (&self.ethereum_key, &self.ethereum_key_file) {
            (Some(_), Some(_)) => Err(CliError::BothKeyTypesProvided.into()),
            (None, None) => Err(CliError::KeyNotProvided.into()),
            (Some(key), _) => Ok(key.clone()),
            (_, Some(key_file)) => Ok(std::fs::read_to_string(key_file)?),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct SubstrateUrl {
    #[clap(long)]
    substrate_url: Url,
}

#[derive(Args, Debug, Clone)]
pub struct EthereumUrl {
    #[clap(long)]
    ethereum_url: Url,
}
