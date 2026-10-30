//! Bounded read-only HASHI recovery responses. These contain no signing key.
//!
//! A matching state-root/header pair does not authenticate consensus finality.
//! The host must call the exact release-bound executor at its independently
//! verified recovery head. External source admission remains the existing
//! HASHI peers' responsibility; these responses never certify beacon finality.

use codec::{Decode, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

/// One native signed payload, derived from an immutable approval template.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiPayload {
    pub signer: [u8; 32],
    pub nonce: u32,
    pub birth_number: u64,
    pub birth_hash: [u8; 32],
    pub death_number: u64,
    pub call_hash: [u8; 32],
    pub extra_hash: [u8; 32],
    /// `SignedPayload::using_encoded`, including its native long-payload hash.
    pub payload: Vec<u8>,
}

/// Native template and state admission, without signature or source finality claims.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiPreparation {
    pub format_version: u32,
    pub parent_hash: [u8; 32],
    pub parent_state_root: [u8; 32],
    pub execution_number: u32,
    pub policy_commitment: [u8; 32],
    pub recovery_anchor: [u8; 32],
    pub operation_index: u32,
    pub operation_commitment: [u8; 32],
    pub network: u32,
    pub spec_version: u32,
    pub transaction_version: u32,
    /// Stored `:code`, which differs from the executing spec-134 substitute.
    /// The host must separately pin/verify the actual executing Wasm.
    pub stored_code_hash: [u8; 32],
    pub source_evidence_commitment: [u8; 32],
    pub approvals: Vec<RecoveryHashiPayload>,
    pub external_finality_verified: bool,
}

/// Successful native incoming execution inside a discarded storage overlay.
/// This is neither a whole-block simulation nor proof of external finality.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiAdmission {
    pub preparation: RecoveryHashiPreparation,
    pub operation_bytes_hash: [u8; 32],
    pub resulting_cursor: u32,
    pub resulting_operation_counter: u64,
    pub resulting_status_scale: Vec<u8>,
    pub weight_ref_time: u64,
    pub weight_proof_size: u64,
    pub native_signatures_checked: bool,
    pub native_incoming_executed: bool,
    pub persistent_state_changed: bool,
    pub external_finality_verified: bool,
}

/// Ordered receipt log projection, supplied by an independently observing peer.
/// This codec does not authenticate receipt inclusion or completeness.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiSourceLog {
    pub address: [u8; 20],
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
    pub log_index: u32,
}

/// Exact source coordinates and native load bytes. The signed raw transaction
/// is taken from the manifest operation, never from a second caller argument.
/// `status` is the receipt field, not a caller-provided verification assertion.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiSource {
    pub format_version: u32,
    pub block_hash: [u8; 32],
    pub block_number: u64,
    pub transaction_hash: [u8; 32],
    pub transaction_index: u32,
    pub status: u8,
    pub logs: Vec<RecoveryHashiSourceLog>,
    pub load_scale: Vec<u8>,
}

/// Derived from native checkpoint records; callers cannot select provenance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum RecoveryHashiLoadProvenance {
    CheckpointPending { submission_height: u32 },
    AutomaticEventImport,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiSourceAssociation {
    pub source_scale_hash: [u8; 32],
    pub original_request_hash: [u8; 32],
    pub native_incoming_hash: [u8; 32],
    pub load_scale_hash: [u8; 32],
    pub load_provenance: RecoveryHashiLoadProvenance,
    pub source_contract: [u8; 20],
    pub source_block_hash: [u8; 32],
    pub source_block_number: u64,
    pub source_transaction_hash: [u8; 32],
    pub source_transaction_index: u32,
    pub source_log_index: u32,
    pub receipt_log_index: u32,
    pub authenticated_outer_actor: [u8; 20],
    pub authenticated_outer_destination: Option<[u8; 20]>,
    pub complete_original_request_matches: bool,
    pub source_inclusion_verified: bool,
    pub external_finality_verified: bool,
}

/// Source association precedes preparation and replacement-owner resolution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryHashiSourcePreparation {
    pub preparation: RecoveryHashiPreparation,
    pub association: RecoveryHashiSourceAssociation,
}
