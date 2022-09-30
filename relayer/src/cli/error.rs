use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Both file and arg key provided")]
    BothKeyTypesProvided,
    #[error("Provide ethereum endpoint via --ethereum-url")]
    EthereumEndpoint,
    #[error("Provide ethereum key via --ethereum-key or --ethereum-key-file")]
    EthereumKey,
    #[error("Provide substrate endpoint via --substrate-url")]
    SubstrateEndpoint,
    #[error("Provide substrate key via --substrate-key or --substrate-key-file")]
    SubstrateKey,
}
