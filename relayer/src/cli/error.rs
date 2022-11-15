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
    #[error("Provide parachain endpoint via --parachain-url")]
    ParachainEndpoint,
    #[error("Provide parachain key via --parachain-key or --parachain-key-file")]
    ParachainKey,
}
