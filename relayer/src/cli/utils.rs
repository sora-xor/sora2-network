use super::error::*;
use crate::prelude::*;
use clap::*;

#[derive(Args, Debug)]
pub struct BaseArgs {
    #[clap(flatten)]
    pub sub: SubstrateUrl,
    #[clap(flatten)]
    pub subkey: SubstrateKey,
    #[clap(flatten)]
    pub eth: EthereumUrl,
    #[clap(flatten)]
    pub ethkey: EthereumKey,
}

impl BaseArgs {
    pub async fn get_unsigned_substrate(&self) -> AnyResult<SubUnsignedClient> {
        let sub = SubUnsignedClient::new(self.sub.get()?).await?;
        Ok(sub)
    }

    pub async fn get_signed_substrate(&self) -> AnyResult<SubSignedClient> {
        let sub = self
            .get_unsigned_substrate()
            .await?
            .try_sign_with(self.subkey.get_key_string()?.as_str())
            .await?;
        Ok(sub)
    }

    pub async fn get_unsigned_ethereum(&self) -> AnyResult<EthUnsignedClient> {
        let eth = EthUnsignedClient::new(self.eth.get()?).await?;
        Ok(eth)
    }

    pub async fn get_signed_ethereum(&self) -> AnyResult<EthSignedClient> {
        let eth = self
            .get_unsigned_ethereum()
            .await?
            .sign_with_string(self.ethkey.get_key_string()?.as_str())
            .await?;
        Ok(eth)
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
            (None, None) => Err(CliError::SubstrateKey.into()),
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
            (None, None) => Err(CliError::EthereumKey.into()),
            (Some(key), _) => Ok(key.clone()),
            (_, Some(key_file)) => Ok(std::fs::read_to_string(key_file)?),
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct SubstrateUrl {
    #[clap(long)]
    substrate_url: Option<String>,
}

impl SubstrateUrl {
    pub fn get(&self) -> AnyResult<String> {
        Ok(self
            .substrate_url
            .clone()
            .ok_or(CliError::SubstrateEndpoint)?)
    }
}

#[derive(Args, Debug, Clone)]
pub struct EthereumUrl {
    #[clap(long)]
    ethereum_url: Option<Url>,
}

impl EthereumUrl {
    pub fn get(&self) -> AnyResult<Url> {
        Ok(self
            .ethereum_url
            .clone()
            .ok_or(CliError::EthereumEndpoint)?)
    }
}
