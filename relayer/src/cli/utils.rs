use super::error::*;
use crate::prelude::*;
use clap::*;

#[derive(Args, Debug, Clone)]
pub struct SubstrateClient {
    #[clap(long, from_global)]
    substrate_key: Option<String>,
    #[clap(long, from_global)]
    substrate_key_file: Option<String>,
    #[clap(long, from_global)]
    substrate_url: Option<String>,
}

impl SubstrateClient {
    pub fn get_key_string(&self) -> AnyResult<String> {
        match (&self.substrate_key, &self.substrate_key_file) {
            (Some(_), Some(_)) => Err(CliError::BothKeyTypesProvided.into()),
            (None, None) => Err(CliError::SubstrateKey.into()),
            (Some(key), _) => Ok(key.clone()),
            (_, Some(key_file)) => Ok(std::fs::read_to_string(key_file)?),
        }
    }

    pub fn get_url(&self) -> AnyResult<String> {
        Ok(self
            .substrate_url
            .clone()
            .ok_or(CliError::SubstrateEndpoint)?)
    }

    pub async fn get_unsigned_substrate(&self) -> AnyResult<SubUnsignedClient> {
        let sub = SubUnsignedClient::new(self.get_url()?).await?;
        Ok(sub)
    }

    pub async fn get_signed_substrate(&self) -> AnyResult<SubSignedClient> {
        let sub = self
            .get_unsigned_substrate()
            .await?
            .try_sign_with(self.get_key_string()?.as_str())
            .await?;
        Ok(sub)
    }
}

#[derive(Args, Debug, Clone)]
pub struct EthereumClient {
    #[clap(long, global = true, from_global)]
    ethereum_key: Option<String>,
    #[clap(long, global = true, from_global)]
    ethereum_key_file: Option<String>,
    #[clap(long, global = true, from_global)]
    ethereum_url: Option<Url>,
}

impl EthereumClient {
    pub fn get_key_string(&self) -> AnyResult<String> {
        match (&self.ethereum_key, &self.ethereum_key_file) {
            (Some(_), Some(_)) => Err(CliError::BothKeyTypesProvided.into()),
            (None, None) => Err(CliError::EthereumKey.into()),
            (Some(key), _) => Ok(key.clone()),
            (_, Some(key_file)) => Ok(std::fs::read_to_string(key_file)?),
        }
    }

    pub fn get_url(&self) -> AnyResult<Url> {
        Ok(self
            .ethereum_url
            .clone()
            .ok_or(CliError::EthereumEndpoint)?)
    }

    pub async fn get_unsigned_ethereum(&self) -> AnyResult<EthUnsignedClient> {
        let eth = EthUnsignedClient::new(self.get_url()?).await?;
        Ok(eth)
    }

    pub async fn get_signed_ethereum(&self) -> AnyResult<EthSignedClient> {
        let eth = self
            .get_unsigned_ethereum()
            .await?
            .sign_with_string(self.get_key_string()?.as_str())
            .await?;
        Ok(eth)
    }
}
