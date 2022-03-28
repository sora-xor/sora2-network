mod ethereum;
mod substrate;

use crate::cli::prelude::*;

#[derive(Subcommand, Debug)]
pub enum Commands {
    Ethereum(ethereum::Command),
    Substrate(substrate::Command),
}

impl Commands {
    pub async fn run(&self) -> AnyResult<()> {
        match self {
            Self::Substrate(cmd) => cmd.run().await,
            Self::Ethereum(cmd) => cmd.run().await,
        }
    }
}
