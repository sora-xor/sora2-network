use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Both file and arg key provided")]
    BothKeyTypesProvided,
    #[error("Key is not provided")]
    KeyNotProvided,
}
