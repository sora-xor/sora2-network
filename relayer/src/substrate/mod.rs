pub mod types;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::prelude::*;
use bridge_types::types::AuxiliaryDigest;
use bridge_types::H256;
use common::{AssetName, AssetSymbol, Balance, ContentSource, Description};
use pallet_mmr_rpc::MmrApiClient;
use sp_core::{Bytes, Pair};
use sp_mmr_primitives::{EncodableOpaqueLeaf, Proof};
use sp_runtime::MultiSigner;
use std::sync::RwLock;
pub use substrate_gen::{runtime, DefaultConfig};
use subxt::events::EventDetails;
pub use subxt::rpc::Subscription;
use subxt::rpc::{rpc_params, RpcClientT};
use subxt::tx::{Signer, TxEvents};
pub use types::*;

pub fn event_to_string(ev: EventDetails) -> String {
    let input = &mut ev.bytes();
    let phase = subxt::events::Phase::decode(input);
    let event = mainnet_runtime::Event::decode(input);
    format!("(Phase: {:?}, Event: {:?})", phase, event)
}

pub fn log_tx_events<T: subxt::Config>(events: TxEvents<T>) {
    for ev in events.iter() {
        match ev {
            Ok(ev) => {
                debug!("{}", event_to_string(ev));
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
pub struct UnsignedClient<T: subxt::Config> {
    api: ApiInner<T>,
    client: ClonableClient,
}

impl<T: subxt::Config> UnsignedClient<T> {
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
        let api = subxt::OnlineClient::<T>::from_rpc_client(client.clone()).await?;
        Ok(Self { api, client })
    }

    pub fn rpc(&self) -> &jsonrpsee::async_client::Client {
        &self.client.0
    }

    pub fn mmr(&self) -> &impl pallet_mmr_rpc::MmrApiClient<BlockHash<T>, BlockNumber<T>>
    where
        <T as subxt::Config>::BlockNumber: Serialize,
    {
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

    pub async fn bridge_commitments(
        &self,
        hash: H256,
    ) -> AnyResult<bridge_channel_rpc::Commitment> {
        Ok(
            bridge_channel_rpc::BridgeChannelAPIClient::commitment(self.rpc(), hash)
                .await?
                .ok_or(anyhow!(
                    "Connect to substrate server with enabled offhcain indexing"
                ))?,
        )
    }

    pub async fn auxiliary_digest(&self, at: Option<T::Hash>) -> AnyResult<AuxiliaryDigest> {
        let res = leaf_provider_rpc::LeafProviderAPIClient::latest_digest(self.rpc(), at).await?;
        Ok(res.unwrap_or_default())
    }

    pub async fn substrate_bridge_commitments(
        &self,
        hash: H256,
    ) -> AnyResult<substrate_bridge_channel_rpc::Commitment<Balance>> {
        Ok(
            substrate_bridge_channel_rpc::BridgeChannelAPIClient::commitment(self.rpc(), hash)
                .await?
                .ok_or(anyhow!(
                    "Connect to substrate server with enabled offhcain indexing"
                ))?,
        )
    }

    pub async fn sign_with_keypair(self, key: impl Into<KeyPair>) -> AnyResult<SignedClient<T>>
    where
        T::Signature: From<<KeyPair as Pair>::Signature>,
        <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
            + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
        T::AccountId: Into<T::Address>,
    {
        SignedClient::<T>::new(self, PairSigner::<T>::new(key.into())).await
    }

    pub async fn try_sign_with(self, key: &str) -> AnyResult<SignedClient<T>>
    where
        T::Signature: From<<KeyPair as Pair>::Signature>,
        <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
            + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
        T::AccountId: Into<T::Address>,
    {
        SignedClient::<T>::new(
            self,
            PairSigner::<T>::new(
                KeyPair::from_string(key, None).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))?,
            ),
        )
        .await
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
            .api()
            .storage()
            .fetch_or_default(&runtime::storage().mmr().number_of_leaves(), None)
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

    // pub async fn subscribe_beefy(&self) -> AnyResult<Subscription<EncodedBeefyCommitment>> {
    //     let sub = self
    //         .api()
    //         .client
    //         .rpc()
    //         .client
    //         .subscribe(
    //             "beefy_subscribeJustifications",
    //             None,
    //             "beefy_unsubscribeJustifications",
    //         )
    //         .await?;
    //     Ok(sub)
    // }

    pub async fn mmr_generate_proof(
        &self,
        block_number: BlockNumber<T>,
        at: Option<BlockHash<T>>,
    ) -> AnyResult<LeafProof<T>>
    where
        BlockNumber<T>: Serialize,
    {
        let res = self.mmr().generate_proof(block_number, at).await?;
        let leaf = MmrLeaf::<T>::decode(
            &mut &*EncodableOpaqueLeaf::decode(&mut res.leaf.as_ref())?
                .into_opaque_leaf()
                .0,
        )?;
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

    pub async fn block_number(&self, at: Option<T::Hash>) -> AnyResult<BlockNumber<T>> {
        let header = self
            .api()
            .rpc()
            .header(at)
            .await?
            .ok_or(anyhow::anyhow!("Header not found"))?;
        Ok(*header.number())
    }
}

#[derive(Clone)]
pub struct SignedClient<T: subxt::Config> {
    inner: UnsignedClient<T>,
    key: PairSigner<T>,
    nonce: Arc<RwLock<Option<Index<T>>>>,
}

impl<T: subxt::Config> SignedClient<T> {
    pub async fn new(client: UnsignedClient<T>, key: impl Into<PairSigner<T>>) -> AnyResult<Self>
    where
        T::Signature: From<<KeyPair as Pair>::Signature>,
        <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
            + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
        T::AccountId: Into<T::Address>,
    {
        let res = Self {
            inner: client,
            key: key.into(),
            nonce: Arc::new(RwLock::new(None)),
        };
        res.load_nonce().await?;
        Ok(res)
    }

    pub fn account_id(&self) -> AccountId<T>
    where
        T::Signature: From<<KeyPair as Pair>::Signature>,
        <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
            + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
    {
        self.key.account_id().clone()
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

    pub async fn load_nonce(&self) -> AnyResult<()>
    where
        T::Signature: From<<KeyPair as Pair>::Signature>,
        <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
            + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
        T::AccountId: Into<T::Address>,
    {
        let nonce = self
            .inner
            .api()
            .rpc()
            .system_account_next_index(&self.account_id())
            .await?;
        self.set_nonce(nonce);
        Ok(())
    }

    pub fn public_key(&self) -> MultiSigner
    where
        T::Signature: From<<KeyPair as Pair>::Signature>,
        <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
            + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
        T::AccountId: Into<T::Address>,
    {
        MultiSigner::Sr25519(self.key.signer().public())
    }
}

impl<T: subxt::Config> Deref for SignedClient<T> {
    type Target = UnsignedClient<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: subxt::Config> DerefMut for SignedClient<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: subxt::Config> Signer<T> for SignedClient<T>
where
    T::Signature: From<<KeyPair as Pair>::Signature>,
    <T::Signature as sp_runtime::traits::Verify>::Signer: From<<KeyPair as Pair>::Public>
        + sp_runtime::traits::IdentifyAccount<AccountId = T::AccountId>,
    T::AccountId: Into<T::Address>,
{
    fn account_id(&self) -> &AccountId<T> {
        self.key.account_id()
    }

    fn nonce(&self) -> Option<Index<T>> {
        let res = *self.nonce.read().expect("poisoned");
        self.nonce
            .write()
            .expect("poisoned")
            .as_mut()
            .map(|nonce| *nonce += 1u32.into());
        res
    }

    fn sign(&self, extrinsic: &[u8]) -> Signature<T> {
        self.key.sign(extrinsic)
    }

    fn address(&self) -> Address<T> {
        self.account_id().into()
    }
}
