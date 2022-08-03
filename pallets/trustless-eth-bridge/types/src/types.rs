//! Types for representing messages

use beefy_primitives::mmr::{BeefyNextAuthoritySet, MmrLeafVersion};
use codec::{Decode, Encode};
use enum_iterator::IntoEnumIterator;
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::H256;
use sp_runtime::{Digest, DigestItem};
use sp_std::vec::Vec;

pub use crate::EthNetworkId;

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum MessageDirection {
    Inbound,
    Outbound,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub struct MessageId {
    channel_id: ChannelId,
    direction: MessageDirection,
    nonce: MessageNonce,
}

impl From<(ChannelId, MessageDirection, MessageNonce)> for MessageId {
    fn from((channel_id, direction, nonce): (ChannelId, MessageDirection, MessageNonce)) -> Self {
        MessageId {
            channel_id,
            direction,
            nonce,
        }
    }
}

impl From<MessageId> for MessageNonce {
    fn from(id: MessageId) -> Self {
        id.nonce
    }
}

impl MessageId {
    pub fn inbound(channel_id: ChannelId, nonce: MessageNonce) -> Self {
        MessageId::from((channel_id, MessageDirection::Inbound, nonce))
    }

    pub fn outbound(channel_id: ChannelId, nonce: MessageNonce) -> Self {
        MessageId::from((channel_id, MessageDirection::Outbound, nonce))
    }
}

pub type MessageNonce = u64;

#[repr(u8)]
#[derive(
    Encode, Decode, Copy, Clone, PartialEq, Eq, IntoEnumIterator, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum ChannelId {
    Basic,
    Incentivized,
}

/// A message relayed from Ethereum.
#[derive(PartialEq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub struct Message {
    /// The raw message data.
    pub data: Vec<u8>,
    /// Input to the message verifier
    pub proof: Proof,
}

/// Verification input for the message verifier.
///
/// This data type allows us to support multiple verification schemes. In the near future,
/// A light-client scheme will be added too.
#[derive(PartialEq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub struct Proof {
    // The block hash of the block in which the receipt was included.
    pub block_hash: H256,
    // The index of the transaction (and receipt) within the block.
    // !!! Untrusted value used just for logging purposes.
    pub tx_index: u32,
    // Proof values
    pub data: Vec<Vec<u8>>,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct AuxiliaryDigest {
    pub logs: Vec<AuxiliaryDigestItem>,
}

impl From<Digest> for AuxiliaryDigest {
    fn from(digest: Digest) -> Self {
        Self {
            logs: digest
                .logs
                .into_iter()
                .filter_map(|log| AuxiliaryDigestItem::try_from(log).ok())
                .collect::<Vec<_>>(),
        }
    }
}

/// Auxiliary [`DigestItem`] to include in header digest.
#[derive(Encode, Decode, Copy, Clone, PartialEq, RuntimeDebug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum AuxiliaryDigestItem {
    /// A batch of messages has been committed.
    Commitment(EthNetworkId, ChannelId, H256),
}

impl Into<DigestItem> for AuxiliaryDigestItem {
    fn into(self) -> DigestItem {
        DigestItem::Other(self.encode())
    }
}

impl TryFrom<DigestItem> for AuxiliaryDigestItem {
    type Error = codec::Error;
    fn try_from(value: DigestItem) -> Result<Self, Self::Error> {
        match value {
            DigestItem::Other(data) => Ok(Decode::decode(&mut &*data)?),
            _ => Err(codec::Error::from("wrong digest item kind")),
        }
    }
}

/// Modified leaf data for SORA
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
pub struct MmrLeaf<BlockNumber, Hash, MerkleRoot, DigestHash> {
    /// Version of the leaf format.
    ///
    /// Can be used to enable future format migrations and compatibility.
    /// See [`MmrLeafVersion`] documentation for details.
    pub version: MmrLeafVersion,
    /// Current block parent number and hash.
    pub parent_number_and_hash: (BlockNumber, Hash),
    /// A merkle root of the next BEEFY authority set.
    pub beefy_next_authority_set: BeefyNextAuthoritySet<MerkleRoot>,
    /// Digest hash of previous block (because digest for current block can be incomplete)
    pub digest_hash: DigestHash,
}

/// A type of asset registered on a bridge.
///
/// - Thischain: a Sora asset.
/// - Sidechain: an Ethereum token.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Encode, Decode, PartialEq, Eq, RuntimeDebug, scale_info::TypeInfo)]
pub enum AssetKind {
    Thischain,
    Sidechain,
}

#[derive(Clone, Copy, RuntimeDebug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
pub enum MessageStatus {
    InQueue,
    Committed,
    Done,
    // TODO: add extrinsic to track status of committed messages
    Failed,
}

#[derive(Clone, Copy, RuntimeDebug, Encode, Decode, PartialEq, Eq, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AppKind {
    EthApp,
    ERC20App,
    SidechainApp,
}

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"trustless-evm-bridge";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";
pub const TECH_ACCOUNT_FEES: &[u8] = b"fees";
pub const TECH_ACCOUNT_TREASURY_PREFIX: &[u8] = b"treasury";
