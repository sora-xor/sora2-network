use crate::prelude::*;
use bridge_types::types::LeafExtraData;
use bridge_types::H256;
use codec::IoReader;
use common::{AssetId32, PredefinedAssetId};
use sp_mmr_primitives::Proof;
pub use substrate_gen::{runtime, DefaultConfig};
pub use subxt::rpc::Subscription;
use subxt::sp_core::Bytes;
use subxt::PolkadotExtrinsicParams;

pub type SoraExtrinsicParams = PolkadotExtrinsicParams<DefaultConfig>;
pub type ApiInner = runtime::RuntimeApi<DefaultConfig, SoraExtrinsicParams>;
pub type KeyPair = subxt::sp_core::sr25519::Pair;
pub type PairSigner = subxt::PairSigner<DefaultConfig, KeyPair>;
pub type AccountId = <DefaultConfig as subxt::Config>::AccountId;
pub type Index = <DefaultConfig as subxt::Config>::Index;
pub type BlockNumber = <DefaultConfig as subxt::Config>::BlockNumber;
pub type BlockHash = <DefaultConfig as subxt::Config>::Hash;
pub type Header = <DefaultConfig as subxt::Config>::Header;
pub type Extrinsic = <DefaultConfig as subxt::Config>::Extrinsic;
pub type SignedBlock = subxt::sp_runtime::generic::SignedBlock<Block>;
pub type Block = subxt::sp_runtime::generic::Block<Header, Extrinsic>;
pub type MmrHash = H256;
pub type LeafExtra = LeafExtraData<H256, H256>;
pub type BeefySignedCommitment =
    beefy_primitives::SignedCommitment<BlockNumber, beefy_primitives::crypto::Signature>;
pub type BeefyCommitment = beefy_primitives::Commitment<BlockNumber>;
pub type MmrLeaf = beefy_primitives::mmr::MmrLeaf<BlockNumber, BlockHash, MmrHash, LeafExtra>;
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
pub struct LeafProof {
    pub block_hash: BlockHash,
    pub leaf: MmrLeaf,
    pub proof: Proof<MmrHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedBeefyCommitment(pub Bytes);

impl EncodedBeefyCommitment {
    pub fn decode(&self) -> AnyResult<BeefySignedCommitment> {
        let mut reader = IoReader(&self.0[..]);
        Ok(Decode::decode(&mut reader)?)
    }
}

pub enum NumberOrHash {
    Number(BlockNumber),
    Hash(BlockHash),
}

impl From<u32> for NumberOrHash {
    fn from(number: u32) -> Self {
        Self::Number(BlockNumber::try_from(number).unwrap())
    }
}

impl From<u64> for NumberOrHash {
    fn from(number: u64) -> Self {
        Self::Number(BlockNumber::try_from(number).unwrap())
    }
}

impl From<H256> for NumberOrHash {
    fn from(hash: H256) -> Self {
        Self::Hash(hash)
    }
}
