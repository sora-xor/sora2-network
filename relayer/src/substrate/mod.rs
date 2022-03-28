use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::prelude::*;
use bridge_types::H256;
use codec::IoReader;
use common::{AssetId32, Balance, PredefinedAssetId};
use pallet_mmr_primitives::{EncodableOpaqueLeaf, Proof};
use pallet_mmr_rpc::{LeafIndex, LeafProof as RawLeafProof};
use std::sync::RwLock;
use substrate_gen::runtime::DefaultAccountData;
pub use substrate_gen::{runtime, DefaultConfig};
pub use subxt::rpc::Subscription;
use subxt::rpc::{rpc_params, ClientT, SubscriptionClientT};
use subxt::sp_core::{Bytes, Pair};
pub use subxt::*;
use tokio::time::Instant;

pub type DefaultExtra = subxt::DefaultExtraWithTxPayment<
    DefaultConfig,
    subxt::extrinsic::ChargeTransactionPayment<DefaultConfig>,
>;
pub type ApiInner = runtime::RuntimeApi<DefaultConfig, DefaultExtra>;
pub type KeyPair = subxt::sp_core::sr25519::Pair;
pub type PairSigner = subxt::PairSigner<DefaultConfig, DefaultExtra, KeyPair>;
pub type AccountId = <DefaultConfig as subxt::Config>::AccountId;
pub type Index = <DefaultConfig as subxt::Config>::Index;
pub type BlockNumber = <DefaultConfig as subxt::Config>::BlockNumber;
pub type BlockHash = <DefaultConfig as subxt::Config>::Hash;
pub type SignedPayload = subxt::extrinsic::SignedPayload<DefaultConfig, DefaultExtra>;
pub type UncheckedExtrinsic = subxt::extrinsic::UncheckedExtrinsic<DefaultConfig, DefaultExtra>;
pub type MmrHash = H256;
pub type DigestHash = beefy_merkle_tree::Hash;
pub type BeefySignedCommitment =
    beefy_primitives::SignedCommitment<BlockNumber, beefy_primitives::crypto::Signature>;
pub type BeefyCommitment = beefy_primitives::Commitment<BlockNumber>;
pub type MmrLeaf = bridge_types::types::MmrLeaf<BlockNumber, BlockHash, MmrHash, DigestHash>;
pub type AssetId = AssetId32<PredefinedAssetId>;

pub enum StorageKind {
    Persistent,
    Local,
}

impl StorageKind {
    fn as_string(&self) -> &'static str {
        match self {
            StorageKind::Persistent => "PERSISTENT",
            StorageKind::Local => "LOCAL",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeafProof {
    pub block_hash: BlockHash,
    pub leaf: MmrLeaf,
    pub proof: Proof<MmrHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedBeefyCommitment(pub sp_core::Bytes);

impl EncodedBeefyCommitment {
    pub fn decode(&self) -> AnyResult<BeefySignedCommitment> {
        let mut reader = IoReader(&self.0[..]);
        Ok(Decode::decode(&mut reader)?)
    }
}

pub struct UnsignedClient(ApiInner);

impl Clone for UnsignedClient {
    fn clone(&self) -> Self {
        Self(self.0.client.clone().into())
    }
}

impl UnsignedClient {
    pub async fn new(url: impl Into<Url>) -> AnyResult<Self> {
        let api = ClientBuilder::new()
            .set_url(url.into())
            .build()
            .await
            .context("Substrate client api build")?
            .to_runtime_api::<ApiInner>();
        Ok(Self(api))
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
            .number_of_leaves(Some(latest_finalized_hash))
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

    pub async fn subscribe_beefy(&self) -> AnyResult<Subscription<EncodedBeefyCommitment>> {
        let sub = self
            .api()
            .client
            .rpc()
            .client
            .subscribe(
                "beefy_subscribeJustifications",
                None,
                "beefy_unsubscribeJustifications",
            )
            .await?;
        Ok(sub)
    }

    pub async fn mmr_generate_proof(
        &self,
        leaf_index: LeafIndex,
        at: Option<BlockHash>,
    ) -> AnyResult<LeafProof> {
        let res = self
            .api()
            .client
            .rpc()
            .client
            .request::<RawLeafProof<BlockHash>>("mmr_generateProof", rpc_params![leaf_index, at])
            .await?;
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

    pub async fn get_total_balance(
        &self,
        asset_id: AssetId,
        account: AccountId,
    ) -> AnyResult<Option<Balance>> {
        let res = self
            .api()
            .client
            .rpc()
            .client
            .request::<Option<assets_runtime_api::BalanceInfo<Balance>>>(
                "assets_totalBalance",
                rpc_params![asset_id, account],
            )
            .await?;
        Ok(res.map(|x| x.balance))
    }

    pub fn api(&self) -> &ApiInner {
        &self.0
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
        debug!("Set nonce to {}", index);
        let mut nonce = self.nonce.write().expect("poisoned");
        *nonce = Some(index);
    }

    pub async fn load_nonce(&self) -> AnyResult<()> {
        let account_storage_entry =
            DefaultAccountData::storage_entry(self.account_id().clone().into());
        let account_data = self
            .api()
            .client
            .storage()
            .fetch_or_default(&account_storage_entry, None)
            .await?;
        let nonce = DefaultAccountData::nonce(&account_data);
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

#[async_trait::async_trait]
impl Signer<DefaultConfig, DefaultExtra> for SignedClient {
    fn account_id(&self) -> &AccountId {
        self.key.account_id()
    }

    fn nonce(&self) -> Option<Index> {
        let start = Instant::now();
        let res = *self.nonce.read().expect("poisoned");
        self.nonce
            .write()
            .expect("poisoned")
            .as_mut()
            .map(|nonce| *nonce += 1);
        debug!("Get nonce in {}s: {:?}", start.elapsed().as_secs_f64(), res);
        res
    }

    async fn sign(&self, extrinsic: SignedPayload) -> Result<UncheckedExtrinsic, String> {
        self.key.sign(extrinsic).await
    }
}
