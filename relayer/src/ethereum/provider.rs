// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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

impl ethers::providers::RpcError for UniversalClientError {
    fn as_error_response(&self) -> Option<&JsonRpcError> {
        match self {
            Self::Ws(err) => err.as_error_response(),
            Self::Http(err) => err.as_error_response(),
            Self::InvalidScheme => None,
        }
    }

    fn as_serde_error(&self) -> Option<&serde_json::Error> {
        match self {
            Self::Ws(err) => err.as_serde_error(),
            Self::Http(err) => err.as_serde_error(),
            Self::InvalidScheme => None,
        }
    }
}

#[async_trait::async_trait]
impl JsonRpcClient for UniversalClient {
    type Error = UniversalClientError;

    /// Sends a request with the provided JSON-RPC and parameters serialized as JSON
    async fn request<T, R>(&self, method: &str, params: T) -> Result<R, Self::Error>
    where
        T: Debug + Serialize + Send + Sync,
        R: DeserializeOwned + Send,
    {
        metrics::increment_counter!(crate::metrics::ETH_TOTAL_RPC_REQUESTS);
        match self {
            Self::Ws(client) => JsonRpcClient::request(client, method, params)
                .await
                .map_err(From::from),
            Self::Http(client) => JsonRpcClient::request(client, method, params)
                .await
                .map_err(From::from),
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
