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
