pub use ethers::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use url::Url;

#[derive(Clone, Debug)]
pub enum UniversalClient {
    Ws(Ws),
    Http(Http),
}

#[derive(Debug, thiserror::Error)]
pub enum UniversalClientError {
    #[error(transparent)]
    Ws(#[from] WsClientError),
    #[error(transparent)]
    Http(#[from] HttpClientError),
    #[error("Invalid scheme")]
    InvalidScheme,
}

impl From<UniversalClientError> for ProviderError {
    fn from(err: UniversalClientError) -> Self {
        ProviderError::JsonRpcClientError(Box::new(err))
    }
}

#[async_trait::async_trait]
impl JsonRpcClient for UniversalClient {
    type Error = UniversalClientError;

    /// Sends a request with the provided JSON-RPC and parameters serialized as JSON
    async fn request<T, R>(&self, method: &str, params: T) -> Result<R, Self::Error>
    where
        T: Debug + Serialize + Send + Sync,
        R: DeserializeOwned,
    {
        match self {
            Self::Ws(client) => client.request(method, params).await.map_err(From::from),
            Self::Http(client) => client.request(method, params).await.map_err(From::from),
        }
    }
}

impl UniversalClient {
    pub async fn new(url: Url) -> Result<Self, UniversalClientError> {
        match url.scheme() {
            "ws" | "wss" => Ok(UniversalClient::Ws(Ws::connect(url).await?)),
            "http" | "https" => Ok(UniversalClient::Http(Http::new(url))),
            _ => Err(UniversalClientError::InvalidScheme),
        }
    }
}
