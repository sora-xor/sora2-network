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

pub mod ethashproof;
pub mod proof_loader;
pub mod provider;
pub mod receipt;

use crate::ethereum::provider::UniversalClient;
use crate::prelude::*;
use bridge_types::Header;
pub use ethers::core::k256::ecdsa::SigningKey;
use ethers::prelude::builders::ContractCall;
pub use ethers::prelude::*;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

pub type EthWallet = Wallet<SigningKey>;

pub type SignedClientInner = SignerMiddleware<UnsignedClientInner, EthWallet>;

pub type UnsignedClientInner = Provider<UniversalClient>;

#[derive(Clone, Debug)]
pub struct UnsignedClient(Arc<UnsignedClientInner>);

impl Deref for UnsignedClient {
    type Target = UnsignedClientInner;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UnsignedClient {
    pub async fn new(url: Url) -> AnyResult<Self> {
        debug!("Connect to {}", url);
        let provider = Provider::new(UniversalClient::new(url).await?);
        Ok(Self(Arc::new(provider)))
    }

    pub async fn signed(
        &self,
        key: SigningKey,
        gas_metrics: Option<PathBuf>,
    ) -> AnyResult<SignedClient> {
        let wallet = Wallet::from(key);
        let chain_id = self.get_chainid().await?;
        let wallet = wallet.with_chain_id(chain_id.as_u64());
        let client = SignerMiddleware::new(self.0.deref().clone(), wallet);
        Ok(SignedClient {
            inner: Arc::new(client),
            gas_metrics,
        })
    }

    pub async fn sign_with_string(
        &self,
        key: &str,
        gas_metrics: Option<PathBuf>,
    ) -> AnyResult<SignedClient> {
        let key =
            SigningKey::from_bytes(hex::decode(key.trim()).context("hex decode")?.as_slice())?;
        Ok(self.signed(key, gas_metrics).await?)
    }

    pub fn inner(&self) -> Arc<UnsignedClientInner> {
        self.0.clone()
    }
}

#[derive(Clone, Debug)]
pub struct SignedClient {
    inner: Arc<SignedClientInner>,
    gas_metrics: Option<PathBuf>,
}

impl Deref for SignedClient {
    type Target = SignedClientInner;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl SignedClient {
    pub async fn new(url: Url, key: SigningKey, gas_metrics: Option<PathBuf>) -> AnyResult<Self> {
        debug!("Connect to {}", url);
        let provider =
            Provider::new(UniversalClient::new(url).await?).interval(Duration::from_millis(100));
        let wallet = Wallet::from(key);
        let chain_id = provider.get_chainid().await?;
        let wallet = wallet.with_chain_id(chain_id.as_u64());
        let client = SignerMiddleware::new(provider, wallet);
        Ok(Self {
            inner: Arc::new(client),
            gas_metrics,
        })
    }

    pub fn unsigned(&self) -> UnsignedClient {
        UnsignedClient(Arc::new(self.inner.inner().clone()))
    }

    pub fn inner(&self) -> Arc<SignedClientInner> {
        self.inner.clone()
    }

    pub async fn save_gas_price<D, M>(
        &self,
        call: &ContractCall<M, D>,
        additional: &str,
    ) -> AnyResult<()>
    where
        D: abi::Detokenize + core::fmt::Debug,
        M: Middleware + 'static,
    {
        use std::io::Write;
        let gas = call.estimate_gas().await?.as_u128();
        let metric = format!(
            "{:?} {} '{}' {}\n",
            call.tx.to(),
            call.function.name,
            additional,
            gas
        );
        debug!("Gas metric: {}", metric);
        if let Some(path) = &self.gas_metrics {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)?;
            file.write_all(metric.as_bytes())?;
        }
        Ok(())
    }
}

pub fn make_header(block: Block<H256>) -> Header {
    let mix_hash_rlp = rlp::encode(&block.mix_hash.unwrap_or_default());
    let nonce_rlp = rlp::encode(&block.nonce.unwrap_or_default());
    Header {
        parent_hash: block.parent_hash,
        timestamp: block.timestamp.as_u64(),
        number: block.number.unwrap_or(U64::zero()).as_u64(),
        author: block.author.unwrap_or_default(),
        transactions_root: block.transactions_root,
        ommers_hash: block.uncles_hash,
        extra_data: block.extra_data.to_vec(),
        state_root: block.state_root,
        receipts_root: block.receipts_root,
        logs_bloom: block.logs_bloom.unwrap_or_default(),
        gas_used: block.gas_used,
        gas_limit: block.gas_limit,
        difficulty: block.difficulty,
        // seal: block.seal_fields.into_iter().map(|x| x.to_vec()).collect(),
        seal: vec![mix_hash_rlp, nonce_rlp]
            .into_iter()
            .map(|x| x.as_ref().to_vec())
            .collect(),
        base_fee: block.base_fee_per_gas,
    }
}
