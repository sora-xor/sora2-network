use std::path::PathBuf;

use super::error::*;
use crate::prelude::*;
use bridge_types::network_config::NetworkConfig;
use clap::*;

#[derive(Clone, Debug)]
pub enum Network {
    Mainnet,
    Ropsten,
    Sepolia,
    Rinkeby,
    Goerli,
    Classic,
    Mordor,
    Custom { path: PathBuf },
    None,
}

const NETWORKS: [&str; 8] = [
    "mainnet", "ropsten", "sepolia", "rinkeby", "goerli", "classic", "mordor", "custom",
];

impl Args for Network {
    fn augment_args(app: App<'_>) -> App<'_> {
        let mut app = app;
        for network in NETWORKS.iter() {
            let mut arg = Arg::new(*network).long(network).required(false);
            if *network == "custom" {
                arg = arg.value_name("PATH").takes_value(true);
            }
            app = app.arg(arg);
        }
        app
    }

    fn augment_args_for_update(app: App<'_>) -> App<'_> {
        Self::augment_args(app)
    }
}

impl FromArgMatches for Network {
    fn from_arg_matches(matches: &ArgMatches) -> Result<Self> {
        let mut network = None;
        let mut occurrences = 0;
        for network_name in NETWORKS.iter() {
            if matches.is_present(network_name) {
                occurrences += 1;
                if occurrences > 1 {
                    return Err(Error::raw(
                        ErrorKind::ArgumentConflict,
                        "Only one network can be specified at a time",
                    ));
                }
                network = Some(match *network_name {
                    "mainnet" => Network::Mainnet,
                    "ropsten" => Network::Ropsten,
                    "sepolia" => Network::Sepolia,
                    "rinkeby" => Network::Rinkeby,
                    "goerli" => Network::Goerli,
                    "classic" => Network::Classic,
                    "mordor" => Network::Mordor,
                    "custom" => {
                        let path = matches.value_of(network_name).expect("required value");
                        Network::Custom {
                            path: PathBuf::from(path),
                        }
                    }
                    _ => unreachable!(),
                });
            }
        }
        Ok(network.unwrap_or(Network::None))
    }

    fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<()> {
        *self = Self::from_arg_matches(matches)?;
        Ok(())
    }
}

impl Network {
    pub fn config(&self) -> AnyResult<NetworkConfig> {
        let res = match self {
            Network::Mainnet => NetworkConfig::Mainnet,
            Network::Ropsten => NetworkConfig::Ropsten,
            Network::Sepolia => NetworkConfig::Sepolia,
            Network::Rinkeby => NetworkConfig::Rinkeby,
            Network::Goerli => NetworkConfig::Goerli,
            Network::Classic => NetworkConfig::Classic,
            Network::Mordor => NetworkConfig::Mordor,
            Network::Custom { path } => {
                let bytes = std::fs::read(path)?;
                serde_json::de::from_slice(&bytes)?
            }
            Network::None => {
                return Err(
                    Error::raw(ErrorKind::MissingRequiredArgument, "No network specified").into(),
                )
            }
        };
        Ok(res)
    }
}

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
    #[clap(long, global = true, from_global)]
    gas_metrics_path: Option<PathBuf>,
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
            .sign_with_string(
                self.get_key_string()?.as_str(),
                self.gas_metrics_path.clone(),
            )
            .await?;
        Ok(eth)
    }
}
