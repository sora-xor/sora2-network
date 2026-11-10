//! Permanent SORA Nexus outgoing quarantine identifiers.
//!
//! These markers survive removal of the independent global bridge halt. They are
//! written only by manifest-authorized recovery work. Native records and public
//! signatures remain evidence; existence of a marker never authorizes a refund.

use crate::{GenericNetworkId, H256};
use codec::{Decode, DecodeWithMemTracking, Encode};
use sp_std::vec::Vec;

fn key(prefix: &[u8], identity: impl Encode) -> Vec<u8> {
    let mut key = prefix.to_vec();
    key.extend(identity.encode());
    key
}

pub fn hashi_key(network: impl Encode, request: H256) -> Vec<u8> {
    key(
        b":sora-nexus:recovery:outgoing-hashi:v1:",
        (network, request),
    )
}

pub fn message_key(network: GenericNetworkId, message: H256) -> Vec<u8> {
    key(
        b":sora-nexus:recovery:outgoing-message:v1:",
        (network, message),
    )
}

/// Guarded historical ownership work uses a data namespace separate from the
/// executive's protected control namespace. This marker conveys no financial
/// claim and cannot establish the authority of an external batch or signature.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct HistoricalMessageTombstone {
    pub tag: [u8; 8],
    pub source_record_hash: [u8; 32],
    pub original_owner: [u8; 32],
    pub effective_owner: [u8; 32],
    /// Raw review/data commitment; never the policy that commits this value.
    pub evidence: [u8; 32],
    pub refund_authorized: bool,
}

impl HistoricalMessageTombstone {
    pub const TAG: [u8; 8] = *b"snxhist1";
    pub fn is_valid(&self) -> bool {
        self.tag == Self::TAG
            && !self.refund_authorized
            && self.source_record_hash != [0; 32]
            && self.evidence != [0; 32]
    }
}

pub fn historical_message_key(network: GenericNetworkId, message: H256) -> Vec<u8> {
    key(
        b":sora-nexus:historical-outgoing-message:v1:",
        (network, message),
    )
}

/// Channel 0 is the generic channel; channel 1 is the substrate channel. Keeping
/// them separate prevents a nonce on one implementation from blocking the other.
pub fn batch_key(channel: u8, network: GenericNetworkId, nonce: u64) -> Vec<u8> {
    key(
        b":sora-nexus:recovery:outgoing-batch:v1:",
        (channel, network, nonce),
    )
}

pub const HISTORICAL_COMMITMENT_PREFIX: &[u8] = b":sora-nexus:historical-signing-commitment:v1:";

/// Exact checkpoint-known signing hash, independent of message-ID hash domains.
/// No retained signature or external redemption authorization is invalidated.
pub fn historical_commitment_key(network: GenericNetworkId, commitment: H256) -> Vec<u8> {
    key(HISTORICAL_COMMITMENT_PREFIX, (network, commitment))
}

pub fn commitment_key(network: GenericNetworkId, commitment: H256) -> Vec<u8> {
    key(
        b":sora-nexus:recovery:outgoing-commitment:v1:",
        (network, commitment),
    )
}

pub const HISTORICAL_HASHI_PREFIX: &[u8] = b":sora-nexus:historical-hashi-request:v1:";

/// Native HASHI lookup identity, including old requests without BridgeProxy rows.
/// Stored request hashes remain authoritative even when historical migrations
/// changed a payload amount. A marker never authorizes an external refund.
pub fn historical_hashi_key(network: impl Encode, request: H256) -> Vec<u8> {
    key(HISTORICAL_HASHI_PREFIX, (network, request))
}

pub fn hashi_quarantined(network: impl Encode, request: H256) -> bool {
    sp_io::storage::exists(&hashi_key(&network, request))
        || sp_io::storage::exists(&historical_hashi_key(network, request))
}

pub fn message_quarantined(network: GenericNetworkId, message: H256) -> bool {
    sp_io::storage::exists(&message_key(network, message))
        // Malformed or unknown marker bytes also stop execution. Only typed,
        // reviewed work may create the canonical historical marker; decoding
        // failure must never revive an old refund or outgoing authorization.
        || sp_io::storage::exists(&historical_message_key(network, message))
}

pub fn batch_quarantined(channel: u8, network: GenericNetworkId, nonce: u64) -> bool {
    sp_io::storage::exists(&batch_key(channel, network, nonce))
}

pub fn commitment_quarantined(network: GenericNetworkId, commitment: H256) -> bool {
    sp_io::storage::exists(&commitment_key(network, commitment))
        || sp_io::storage::exists(&historical_commitment_key(network, commitment))
}
