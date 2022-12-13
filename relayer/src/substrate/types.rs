use crate::prelude::*;
use bridge_types::types::LeafExtraData;
use bridge_types::H256;
use codec::IoReader;
use common::{AssetId32, PredefinedAssetId};
pub use parachain_gen::{
    parachain_runtime, DefaultConfig as ParachainConfig,
    SoraExtrinsicParams as ParachainExtrinsicParams,
};
use sp_core::{sr25519, Bytes};
use sp_mmr_primitives::Proof;
pub use substrate_gen::{
    runtime as mainnet_runtime, DefaultConfig as MainnetConfig,
    SoraExtrinsicParams as MainnetExtrinsicParams,
};
pub use subxt::rpc::Subscription;
use subxt::OnlineClient;

pub type ApiInner<T> = OnlineClient<T>;
pub type KeyPair = sr25519::Pair;
pub type PairSigner<T> = subxt::tx::PairSigner<T, KeyPair>;
pub type AccountId<T> = <T as subxt::Config>::AccountId;
pub type Address<T> = <T as subxt::Config>::Address;
pub type Index<T> = <T as subxt::Config>::Index;
pub type BlockNumber<T> = <T as subxt::Config>::BlockNumber;
pub type BlockHash<T> = <T as subxt::Config>::Hash;
pub type Signature<T> = <T as subxt::Config>::Signature;
pub type MmrHash = H256;
pub type LeafExtra = LeafExtraData<H256, H256>;
pub type BeefySignedCommitment<T> =
    beefy_primitives::VersionedFinalityProof<BlockNumber<T>, beefy_primitives::crypto::Signature>;
pub type BeefyCommitment<T> = beefy_primitives::Commitment<BlockNumber<T>>;
pub type MmrLeaf<T> =
    beefy_primitives::mmr::MmrLeaf<BlockNumber<T>, BlockHash<T>, MmrHash, LeafExtra>;
pub type AssetId = AssetId32<PredefinedAssetId>;

pub enum StorageKind {
    Persistent,
    Local,
}

impl StorageKind {
    pub fn as_string(&self) -> &'static str {
        match self {
            StorageKind::Persistent => "PERSISTENT",
            StorageKind::Local => "LOCAL",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeafProof<T: subxt::Config> {
    pub block_hash: BlockHash<T>,
    pub leaf: MmrLeaf<T>,
    pub proof: Proof<MmrHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedBeefyCommitment(pub Bytes);

impl EncodedBeefyCommitment {
    pub fn decode<T: subxt::Config>(&self) -> AnyResult<BeefySignedCommitment<T>> {
        let mut reader = IoReader(&self.0[..]);
        Ok(Decode::decode(&mut reader)?)
    }
}
