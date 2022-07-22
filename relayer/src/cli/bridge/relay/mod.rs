mod ethereum_relay;
mod substrate_relay;

use crate::{cli::BaseArgs, prelude::*};
use clap::*;

#[derive(Debug, Subcommand)]
pub(crate) enum Commands {
    Ethereum(ethereum_relay::Command),
    Substrate(substrate_relay::Command),
}

impl Commands {
    pub async fn run(&self, args: &BaseArgs) -> AnyResult<()> {
        match self {
            Commands::Ethereum(cmd) => cmd.run(args).await,
            Commands::Substrate(cmd) => cmd.run(args).await,
        }
    }
}
