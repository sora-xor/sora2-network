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

pub mod beefy_subscription;
pub mod traits;
pub mod types;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::prelude::*;
use bridge_types::types::AuxiliaryDigest;
use common::{AssetName, AssetSymbol, Balance, ContentSource, Description};
use mmr_rpc::MmrApiClient;
use sp_core::Bytes;
use sp_mmr_primitives::{EncodableOpaqueLeaf, Proof};
use sp_runtime::traits::AtLeast32BitUnsigned;
use std::sync::RwLock;
pub use substrate_gen::{runtime, DefaultConfig};
use subxt::blocks::ExtrinsicEvents;
use subxt::constants::ConstantAddress;
use subxt::events::EventDetails;
use subxt::metadata::DecodeWithMetadata;
pub use subxt::rpc::Subscription;
use subxt::rpc::{rpc_params, ChainBlockResponse, RpcClientT};
use subxt::storage::address::Yes;
use subxt::storage::StorageAddress;
use subxt::tx::Signer;
pub use types::*;

/// Finds the first occurrence of an element 'e' so that 'f(e)' is greater or equal 'value' in
/// storage with ascending values. Returns the index of 'e'.
pub async fn binary_search_first_occurrence<N: AtLeast32BitUnsigned, T: PartialOrd, F, Fut>(
    low: N,
    high: N,
    value: T,
    f: F,
) -> AnyResult<Option<N>>
where
    F: Fn(N) -> Fut,
    Fut: futures::Future<Output = AnyResult<Option<T>>>,
{
    let mut low = low;
    let mut high = high;
    while low < high {
        let mid = (high.clone() + low.clone()) / 2u32.into();
        let found_value = f(mid.clone()).await?;
        match found_value {
            None => low = mid + 1u32.into(),
            Some(found_value) if found_value < value => low = mid + 1u32.into(),
            _ => high = mid,
        }
    }
    // If value between blocks can increase more than by 1
    if f(low.clone()).await? >= Some(value) {
        Ok(Some(low))
    } else {
        Ok(None)
    }
}

pub fn event_to_string<T: ConfigExt>(ev: EventDetails) -> String {
    let input = &mut ev.bytes();
    let phase = subxt::events::Phase::decode(input);
    let event = T::Event::decode(input);
    format!("(Phase: {:?}, Event: {:?})", phase, event)
}

pub fn log_extrinsic_events<T: ConfigExt>(events: ExtrinsicEvents<T::Config>) {
    for ev in events.iter() {
        match ev {
            Ok(ev) => {
                debug!("{}", event_to_string::<T>(ev));
            }
            Err(err) => {
                warn!("Failed to decode event: {:?}", err);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClonableClient(Arc<jsonrpsee::async_client::Client>);

impl RpcClientT for ClonableClient {
    fn request_raw<'a>(
        &'a self,
        method: &'a str,
        params: Option<Box<jsonrpsee::core::JsonRawValue>>,
    ) -> subxt::rpc::RpcFuture<'a, Box<jsonrpsee::core::JsonRawValue>> {
        self.0.request_raw(method, params)
    }

    fn subscribe_raw<'a>(
        &'a self,
        sub: &'a str,
        params: Option<Box<jsonrpsee::core::JsonRawValue>>,
        unsub: &'a str,
    ) -> subxt::rpc::RpcFuture<'a, subxt::rpc::RpcSubscription> {
        self.0.subscribe_raw(sub, params, unsub)
    }
}

#[derive(Debug, Clone)]
pub struct UnsignedClient<T: ConfigExt> {
    api: ApiInner<T>,
    client: ClonableClient,
}

impl<T: ConfigExt> UnsignedClient<T> {
    pub async fn new(url: impl Into<String>) -> AnyResult<Self> {
        let url: Uri = url.into().parse()?;
        let (sender, receiver) =
            jsonrpsee::client_transport::ws::WsTransportClientBuilder::default()
                .build(url)
                .await?;
        let client = jsonrpsee::async_client::ClientBuilder::default()
            .max_notifs_per_subscription(4096)
            .build_with_tokio(sender, receiver);
        let client = ClonableClient(Arc::new(client));
        let api = ApiInner::<T>::from_rpc_client(client.clone().0).await?;
        Ok(Self { api, client })
    }

    pub fn rpc(&self) -> &jsonrpsee::async_client::Client {
        &self.client.0
    }

    pub fn mmr(&self) -> &impl mmr_rpc::MmrApiClient<BlockHash<T>, BlockNumber<T>, MmrHash> {
        self.rpc()
    }

    pub fn beefy(
        &self,
    ) -> &impl beefy_gadget_rpc::BeefyApiClient<types::EncodedBeefyCommitment, BlockHash<T>> {
        self.rpc()
    }

    pub fn assets(
        &self,
    ) -> &impl assets_rpc::AssetsAPIClient<
        BlockHash<T>,
        AccountId<T>,
        AssetId,
        Balance,
        Option<assets_runtime_api::BalanceInfo<Balance>>,
        Option<
            assets_runtime_api::AssetInfo<
                AssetId,
                AssetSymbol,
                AssetName,
                u8,
                ContentSource,
                Description,
            >,
        >,
        Vec<
            assets_runtime_api::AssetInfo<
                AssetId,
                AssetSymbol,
                AssetName,
                u8,
                ContentSource,
                Description,
            >,
        >,
        Vec<AssetId>,
    > {
        self.rpc()
    }

    pub async fn auxiliary_digest(&self, at: Option<BlockHash<T>>) -> AnyResult<AuxiliaryDigest> {
        let res = leaf_provider_rpc::LeafProviderAPIClient::latest_digest(self.rpc(), at).await?;
        Ok(res.unwrap_or_default())
    }

    pub async fn bridge_commitment(
        &self,
        network_id: bridge_types::GenericNetworkId,
        batch_nonce: u64,
    ) -> AnyResult<OffchainDataOf<T>> {
        Ok(
            bridge_channel_rpc::BridgeChannelAPIClient::<OffchainDataOf<T>>::commitment(
                self.rpc(),
                network_id,
                batch_nonce,
            )
            .await?
            .ok_or(anyhow!(
                "Connect to substrate server with enabled offchain indexing"
            ))?,
        )
    }

    pub async fn beefy_start_block(&self) -> AnyResult<u64> {
        let latest_finalized_hash = self.api().rpc().finalized_head().await?;
        let latest_finalized_number = self
            .api()
            .rpc()
            .block(Some(latest_finalized_hash))
            .await?
            .expect("should exist")
            .block
            .header
            .number()
            .clone();
        let mmr_leaves = self
            .storage_fetch_or_default(&runtime::storage().mmr().number_of_leaves(), ())
            .await?;
        let beefy_start_block = latest_finalized_number.into().saturating_sub(mmr_leaves);
        debug!("Beefy started at: {}", beefy_start_block);
        Ok(beefy_start_block)
    }

    pub async fn offchain_local_get(
        &self,
        storage: StorageKind,
        key: Vec<u8>,
    ) -> AnyResult<Option<Vec<u8>>> {
        let res = self
            .api()
            .rpc()
            .request::<Option<Bytes>>(
                "offchain_localStorageGet",
                rpc_params![storage.as_string(), Bytes(key)],
            )
            .await?;
        Ok(res.map(|x| x.0))
    }

    pub async fn mmr_generate_proof(
        &self,
        block_number: BlockNumber<T>,
        at: BlockNumber<T>,
    ) -> AnyResult<LeafProof<T>>
    where
        BlockNumber<T>: Serialize,
    {
        let res = self
            .mmr()
            .generate_proof(vec![block_number], Some(at), None)
            .await?;

        let enc_opaque_leaf = match Vec::<EncodableOpaqueLeaf>::decode(&mut res.leaves.as_ref()) {
            Ok(mut v) => {
                if v.len() == 0 {
                    error!("Opaque leaves count is zero");
                    Err(anyhow::anyhow!("Opaque leaves count error"))?;
                }
                v.remove(0)
            }
            Err(e) => {
                error!("Error decoding opaque mmr leaves");
                Err(e)?
            }
        };

        let leaf = MmrLeaf::<T>::decode(&mut &*enc_opaque_leaf.into_opaque_leaf().0)?;

        let proof = Proof::<MmrHash>::decode(&mut res.proof.as_ref())?;
        Ok(LeafProof {
            leaf,
            proof,
            block_hash: res.block_hash,
        })
    }

    pub fn api(&self) -> &ApiInner<T> {
        &self.api
    }

    pub async fn header<N: Into<BlockNumberOrHash>>(&self, at: N) -> AnyResult<Header<T>> {
        let hash = self.block_hash(at).await?;
        let header = self
            .api()
            .rpc()
            .header(Some(hash.into()))
            .await?
            .ok_or(anyhow::anyhow!("Header not found"))?;
        Ok(header)
    }

    pub async fn block_number<N: Into<BlockNumberOrHash>>(
        &self,
        at: N,
    ) -> AnyResult<BlockNumber<T>> {
        let header = self.header(at).await?;
        Ok(BlockNumber::<T>::from(header.number().clone()))
    }

    pub async fn finalized_head(&self) -> AnyResult<BlockHash<T>> {
        let hash = self.api().rpc().finalized_head().await?;
        Ok(hash.into())
    }

    pub async fn block_hash<N: Into<BlockNumberOrHash>>(&self, at: N) -> AnyResult<BlockHash<T>> {
        let block_number = match at.into() {
            BlockNumberOrHash::Number(n) => Some(n),
            BlockNumberOrHash::Hash(h) => return Ok(h.into()),
            BlockNumberOrHash::Best => None,
        };
        let res = self
            .api()
            .rpc()
            .block_hash(block_number.map(Into::into))
            .await
            .context("Get block hash")?
            .ok_or(anyhow::anyhow!("Block not found"))?;
        Ok(res.into())
    }

    pub async fn block<N: Into<BlockNumberOrHash>>(
        &self,
        at: N,
    ) -> AnyResult<ChainBlockResponse<T::Config>> {
        let hash = self.block_hash(at).await?;
        let block = self
            .api()
            .rpc()
            .block(Some(hash.into()))
            .await?
            .ok_or(anyhow::anyhow!("Block not found"))?;
        Ok(block)
    }

    pub async fn storage_fetch<N, Address>(
        &self,
        address: &Address,
        hash: N,
    ) -> AnyResult<Option<<Address::Target as DecodeWithMetadata>::Target>>
    where
        Address: StorageAddress<IsFetchable = Yes>,
        N: Into<BlockNumberOrHash>,
    {
        let hash = self.block_hash(hash).await?;
        let res = self
            .api()
            .storage()
            .fetch(address, Some(hash.into()))
            .await
            .context(format!(
                "Fetch storage {}::{} at hash {:?}",
                address.pallet_name(),
                address.entry_name(),
                hash
            ))?;
        Ok(res)
    }

    pub async fn storage_fetch_or_default<N, Address>(
        &self,
        address: &Address,
        hash: N,
    ) -> AnyResult<<Address::Target as DecodeWithMetadata>::Target>
    where
        Address: StorageAddress<IsFetchable = Yes, IsDefaultable = Yes>,
        N: Into<BlockNumberOrHash>,
    {
        let hash = self.block_hash(hash).await?;
        let res = self
            .api()
            .storage()
            .fetch_or_default(address, Some(hash.into()))
            .await?;
        Ok(res)
    }

    pub fn constant_fetch_or_default<Address>(
        &self,
        address: &Address,
    ) -> AnyResult<<Address::Target as DecodeWithMetadata>::Target>
    where
        Address: ConstantAddress,
    {
        let res = self.api().constants().at(address)?;
        Ok(res)
    }

    pub async fn signed(self, signer: PairSigner<T>) -> AnyResult<SignedClient<T>> {
        SignedClient::<T>::new(self, signer).await
    }

    pub async fn submit_unsigned_extrinsic<P: subxt::tx::TxPayload>(
        &self,
        xt: &P,
    ) -> AnyResult<()> {
        if let Some(validation) = xt.validation_details() {
            debug!(
                "Submitting extrinsic: {}::{}",
                validation.pallet_name, validation.call_name
            );
        } else {
            debug!("Submitting extrinsic without validation data");
        }
        let res = self
            .api()
            .tx()
            .create_unsigned(xt)?
            .submit_and_watch()
            .await
            .map_err(|e| {
                error!("submit then watch error: {:?}", e);
                e
            })
            .context("sign and submit then watch")?
            .wait_for_in_block()
            .await
            .map_err(|e| {
                error!("wait for in block error: {:?}", e);
                e
            })
            .context("wait for in block")?
            .wait_for_success()
            .await
            .map_err(|e| {
                error!("wait for success error: {:?}", e);
                e
            })
            .context("wait for success")?;
        log_extrinsic_events::<T>(res);
        Ok(())
    }
}

#[derive(Clone)]
pub struct SignedClient<T: ConfigExt> {
    inner: UnsignedClient<T>,
    key: PairSigner<T>,
    nonce: Arc<RwLock<Option<Index<T>>>>,
}

impl<T: ConfigExt> SignedClient<T> {
    pub async fn new(client: UnsignedClient<T>, key: PairSigner<T>) -> AnyResult<Self> {
        let res = Self {
            inner: client,
            key,
            nonce: Arc::new(RwLock::new(None)),
        };
        res.load_nonce().await?;
        Ok(res)
    }

    pub fn account_id(&self) -> AccountId<T> {
        self.key.account_id().clone()
    }

    pub async fn submit_extrinsic<P: subxt::tx::TxPayload>(&self, xt: &P) -> AnyResult<()>
    where
        <<<T as ConfigExt>::Config as subxt::Config>::ExtrinsicParams as subxt::tx::ExtrinsicParams<
            <<T as ConfigExt>::Config as subxt::Config>::Index,
            <<T as ConfigExt>::Config as subxt::Config>::Hash,
        >>::OtherParams: Default,
    {
        if let Some(validation) = xt.validation_details() {
            debug!(
                "Submitting extrinsic: {}::{}",
                validation.pallet_name, validation.call_name
            );
        } else {
            debug!("Submitting extrinsic without validation data");
        }
        // Metadata validation often works incorrectly, so we turn it off for now
        let xt = UnvalidatedTxPayload(xt);
        let res = self
            .api()
            .tx()
            .sign_and_submit_then_watch_default(&xt, self)
            .await
            .map_err(|e| {
                error!("sign and submit then watch error: {:?}", e);
                e
            })
            .context("sign and submit then watch")?
            .wait_for_in_block()
            .await
            .context("wait for in block")?
            .wait_for_success()
            .await
            .context("wait for success")?;
        log_extrinsic_events::<T>(res);
        Ok(())
    }

    pub async fn load_nonce(&self) -> AnyResult<()> {
        let nonce = self
            .inner
            .api()
            .rpc()
            .system_account_next_index(&self.key.account_id())
            .await?;
        self.set_nonce(nonce);
        Ok(())
    }

    pub fn unsigned(self) -> UnsignedClient<T> {
        self.inner
    }

    pub fn api(&self) -> &ApiInner<T> {
        &self.inner.api()
    }

    pub fn set_nonce(&self, index: Index<T>) {
        let mut nonce = self.nonce.write().expect("poisoned");
        *nonce = Some(index);
    }
}

impl<T: ConfigExt> Deref for SignedClient<T> {
    type Target = UnsignedClient<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: ConfigExt> DerefMut for SignedClient<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: ConfigExt> Signer<T::Config> for SignedClient<T> {
    fn account_id(&self) -> &AccountId<T> {
        self.key.account_id()
    }

    fn sign(&self, extrinsic: &[u8]) -> Signature<T> {
        self.key.sign(extrinsic)
    }

    fn address(&self) -> Address<T> {
        self.key.address()
    }
}
