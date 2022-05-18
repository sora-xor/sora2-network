mod cli;
mod ethereum;
mod relay;
mod substrate;
use clap::StructOpt;
use prelude::*;

#[macro_use]
extern crate log;

#[macro_use]
extern crate anyhow;

#[tokio::main]
async fn main() -> AnyResult<()> {
    init_log();
    let cli = cli::Cli::parse();
    debug!("Cli: {:?}", cli);
    cli.run().await?;
    Ok(())
}

fn init_log() {
    if std::env::var_os("RUST_LOG").is_none() {
        env_logger::builder().parse_filters("info").init();
    } else {
        env_logger::init();
    }
}

pub mod prelude {
    pub use crate::ethereum::{
        SignedClient as EthSignedClient, UnsignedClient as EthUnsignedClient,
    };
    pub use crate::substrate::runtime::runtime_types as sub_types;
    pub use crate::substrate::{
        runtime as sub_runtime, SignedClient as SubSignedClient,
        UnsignedClient as SubUnsignedClient,
    };
    pub use anyhow::{Context, Result as AnyResult};
    pub use codec::{Decode, Encode};
    pub use hex_literal::hex;
    pub use http::Uri;
    pub use serde::{Deserialize, Serialize};
    pub use url::Url;
}
