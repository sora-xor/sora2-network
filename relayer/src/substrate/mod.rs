pub mod types;

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::prelude::*;
use bridge_types::H256;
use common::{AssetName, AssetSymbol, Balance, ContentSource, Description};
use pallet_mmr_rpc::MmrApiClient;
use sp_mmr_primitives::{EncodableOpaqueLeaf, LeafIndex, Proof};
use std::sync::RwLock;
pub use substrate_gen::{runtime, DefaultConfig};
use subxt::extrinsic::Signer;
pub use subxt::rpc::Subscription;
use subxt::rpc::{rpc_params, ClientT};
use subxt::sp_core::{Bytes, Pair};
use subxt::{ClientBuilder, Config, RpcClient};
pub use types::*;

pub struct UnsignedClient(ApiInner);

impl Clone for UnsignedClient {
    fn clone(&self) -> Self {
        Self(self.0.client.clone().into())
    }
}

impl UnsignedClient {
    pub async fn new(url: impl Into<String>) -> AnyResult<Self> {
        let api = ClientBuilder::new()
            .set_url(url)
            .build()
            .await
            .context("Substrate client api build")?
            .to_runtime_api::<ApiInner>();
        Ok(Self(api))
    }

    pub fn rpc(&self) -> &RpcClient {
        &self.api().client.rpc().client
    }

    pub fn mmr(&self) -> &impl pallet_mmr_rpc::MmrApiClient<BlockHash> {
        self.rpc()
    }

    pub fn beefy(
        &self,
    ) -> &impl beefy_gadget_rpc::BeefyApiClient<types::EncodedBeefyCommitment, BlockHash> {
        self.rpc()
    }

    pub fn assets(
        &self,
    ) -> &impl assets_rpc::AssetsAPIClient<
        BlockHash,
        AccountId,
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

    pub async fn basic_commitments(
        &self,
        hash: H256,
    ) -> AnyResult<Vec<basic_channel_rpc::Message>> {
        Ok(
            basic_channel_rpc::BasicChannelAPIClient::commitment(self.rpc(), hash)
                .await?
                .ok_or(anyhow!(
                    "Connect to substrate server with enabled offhcain indexing"
                ))?,
        )
    }

    pub async fn incentivized_commitments(
        &self,
        hash: H256,
    ) -> AnyResult<Vec<incentivized_channel_rpc::Message>> {
        Ok(
            incentivized_channel_rpc::IncentivizedChannelAPIClient::commitment(self.rpc(), hash)
                .await?
                .ok_or(anyhow!(
                    "Connect to substrate server with enabled offhcain indexing"
                ))?,
        )
    }

    pub async fn sign_with_keypair(self, key: impl Into<KeyPair>) -> AnyResult<SignedClient> {
        SignedClient::new(self, PairSigner::new(key.into())).await
    }

    pub async fn try_sign_with(self, key: &str) -> AnyResult<SignedClient> {
        SignedClient::new(
            self,
            PairSigner::new(
                KeyPair::from_string(key, None).map_err(|e| anyhow::anyhow!(format!("{:?}", e)))?,
            ),
        )
        .await
    }

    pub async fn beefy_start_block(&self) -> AnyResult<u64> {
        let latest_finalized_hash = self.api().client.rpc().finalized_head().await?;
        let latest_finalized_number = self
            .api()
            .client
            .rpc()
            .block(Some(latest_finalized_hash))
            .await?
            .expect("should exist")
            .block
            .header
            .number;
        let mmr_leaves = self
            .api()
            .storage()
            .mmr()
            .number_of_leaves(false, Some(latest_finalized_hash))
            .await?;
        let beefy_start_block = latest_finalized_number as u64 - mmr_leaves;
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
            .client
            .rpc()
            .client
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
        leaf_index: LeafIndex,
        at: Option<BlockHash>,
    ) -> AnyResult<LeafProof> {
        let res = self.mmr().generate_proof(leaf_index, at).await?;
        let leaf = MmrLeaf::decode(
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

    pub fn api(&self) -> &ApiInner {
        &self.0
    }

    pub async fn block<T: Into<NumberOrHash>>(&self, block: Option<T>) -> AnyResult<SignedBlock> {
        let hash = self.block_hash(block).await?;
        let block = self
            .api()
            .client
            .rpc()
            .block(Some(hash))
            .await?
            .ok_or(anyhow::anyhow!("Block not found"))?;
        Ok(block)
    }

    pub async fn block_hash<T: Into<NumberOrHash>>(
        &self,
        block: Option<T>,
    ) -> AnyResult<BlockHash> {
        let number = match block.map(|x| x.into()) {
            Some(NumberOrHash::Hash(hash)) => return Ok(hash),
            Some(NumberOrHash::Number(number)) => Some(number),
            None => None,
        };
        let hash = self
            .api()
            .client
            .rpc()
            .block_hash(number.map(|x| x.into()))
            .await?
            .ok_or(anyhow::anyhow!("Block not found"))?;
        Ok(hash)
    }

    pub async fn header<T: Into<NumberOrHash>>(&self, block: Option<T>) -> AnyResult<Header> {
        let hash = self.block_hash(block).await?;
        let header = self
            .api()
            .client
            .rpc()
            .header(Some(hash))
            .await?
            .ok_or(anyhow::anyhow!("Header not found"))?;
        Ok(header)
    }

    pub async fn block_number<T: Into<NumberOrHash>>(
        &self,
        block: Option<T>,
    ) -> AnyResult<BlockNumber> {
        let header = self.header(block).await?;
        Ok(header.number)
    }
}

#[derive(Clone)]
pub struct SignedClient {
    inner: UnsignedClient,
    key: PairSigner,
    nonce: Arc<RwLock<Option<Index>>>,
}

impl SignedClient {
    pub async fn new(client: UnsignedClient, key: impl Into<PairSigner>) -> AnyResult<Self> {
        let res = Self {
            inner: client,
            key: key.into(),
            nonce: Arc::new(RwLock::new(None)),
        };
        res.load_nonce().await?;
        Ok(res)
    }

    pub fn account_id(&self) -> AccountId {
        self.key.account_id().clone()
    }

    pub fn unsigned(self) -> UnsignedClient {
        self.inner
    }

    pub fn api(&self) -> &ApiInner {
        &self.inner.0
    }

    pub fn set_nonce(&self, index: Index) {
        let mut nonce = self.nonce.write().expect("poisoned");
        *nonce = Some(index);
    }

    pub async fn load_nonce(&self) -> AnyResult<()> {
        let nonce = self
            .inner
            .api()
            .client
            .rpc()
            .system_account_next_index(&self.account_id())
            .await?;
        self.set_nonce(nonce);
        Ok(())
    }
}

impl Deref for SignedClient {
    type Target = UnsignedClient;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for SignedClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Signer<DefaultConfig> for SignedClient {
    fn account_id(&self) -> &AccountId {
        self.key.account_id()
    }

    fn nonce(&self) -> Option<Index> {
        let res = *self.nonce.read().expect("poisoned");
        self.nonce
            .write()
            .expect("poisoned")
            .as_mut()
            .map(|nonce| *nonce += 1);
        res
    }

    fn sign(&self, extrinsic: &[u8]) -> <DefaultConfig as Config>::Signature {
        self.key.sign(extrinsic)
    }

    fn address(&self) -> <DefaultConfig as subxt::Config>::Address {
        self.account_id()
    }
}
