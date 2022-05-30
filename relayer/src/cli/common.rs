use http::Uri;

use crate::cli::prelude::*;

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
    substrate_url: Uri,
}

impl SubstrateUrl {
    pub fn get(&self) -> Uri {
        self.substrate_url.clone()
    }
}

#[derive(Args, Debug, Clone)]
pub struct EthereumUrl {
    #[clap(long)]
    ethereum_url: Url,
}

impl EthereumUrl {
    pub fn get(&self) -> Url {
        self.ethereum_url.clone()
    }
}
