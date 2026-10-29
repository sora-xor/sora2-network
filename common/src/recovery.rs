//! Shared, versioned commitments for the SORA Nexus incident recovery.
//!
//! This module contains no production account substitutions or authority keys.
//! A release must compile a validated policy from the independently reviewed
//! checkpoint evidence. Empty, ambiguous or inconsistent policies fail closed.

use alloc::{boxed::Box, collections::BTreeSet, vec::Vec};
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use scale_info::TypeInfo;
use sp_core::crypto::AccountId32;
use sp_runtime::{traits::Verify, MultiSignature};

pub mod invariants;
pub mod issuance;
pub mod storage;

pub const CHECKPOINT_NUMBER: u32 = 28_191_817;
pub const CHECKPOINT_HASH: [u8; 32] =
    hex_literal::hex!("7f7c253fa72f32e7b03f32bdc9ae456fd7174fb131b689be6d876006124129f0");
pub const CHECKPOINT_STATE_ROOT: [u8; 32] =
    hex_literal::hex!("79bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b140452");
pub const GENESIS_HASH: [u8; 32] =
    hex_literal::hex!("7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5");
pub const ATTACKER_ACCOUNT: [u8; 32] =
    hex_literal::hex!("64650caa38ca952fc214cbb38cb625d779d1387b52910e664012cbd7d7593a7d");
pub const ATTACKER_EVM: [u8; 20] = hex_literal::hex!("af6fd7a11087bfbaddc5c4e1ab14ab0c592af63f");
pub const POLICY_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:policy:v1";
pub const STATUS_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:status:v1";
/// First recovery child hash, recorded from its child after that header exists.
pub const RECOVERY_ANCHOR_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:branch-anchor:v1";
pub const BRIDGE_HALT_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:bridge-halt:v1";
pub const BRIDGE_INBOUND_CONTEXT_STORAGE_KEY: &[u8] =
    b":sora-nexus:recovery:bridge-inbound-context:v1";
pub const STORAGE_PREFIX: &[u8] = b":sora-nexus:recovery:";
pub const LINEAGE_MAGIC: [u8; 16] = *b"soranx-recov-v1\0";
pub const INHERENT_IDENTIFIER: [u8; 8] = *b"snxrec01";
pub const FINAL_SPEC_VERSION: u32 = 138;
pub const FINAL_TRANSACTION_VERSION: u32 = 134;
pub const HOLD_SPEC_VERSION: u32 = 135;
pub const MAX_CHUNK_BYTES: usize = 512 * 1024;
pub const MAX_EDITS_PER_CHUNK: usize = 128;
/// Read-only exact-key audits have a separate cardinality budget. Their encoded
/// envelope and aggregate value reads remain bounded by `MAX_CHUNK_BYTES`.
pub const MAX_AUDIT_WITNESSES: usize = 512;
pub const MAX_STORAGE_KEY_BYTES: usize = 512;
pub const MAX_COUNCIL_MEMBERS: usize = 100;
pub const MAX_REPLACEMENTS: usize = 4096;
pub const MAX_WORK_CHUNKS: usize = 16_384;
pub const MAX_CODE_CHUNK_BYTES: usize = 64 * 1024;
pub const MAX_INHERENT_BYTES: usize = MAX_CHUNK_BYTES + MAX_CODE_CHUNK_BYTES;
pub const AUDIT_CURSOR_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:audit-cursor:v1";
/// Permanent count of authenticated Incoming operations still awaiting typed audit.
pub const INCOMING_AUDIT_PENDING_KEY: &[u8] = b":sora-nexus:recovery:incoming-audit-pending:v1";
pub const ACTIVE_POLICY_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:active-policy:v1";
pub const TERMINAL_AUDIT_STORAGE_KEY: &[u8] = b":sora-nexus:recovery:terminal-audit:v1";
pub const TRANSIENT_CONTEXT_KEYS: &[&[u8]] = &[
    BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
    b":sora-nexus:recovery:incoming-active:v1",
    b":sora-nexus:recovery:incoming-failed:v1",
    b":sora-nexus:recovery:hashi-resolution:v1",
];

/// The header commitment is emitted on every recovery descendant, including
/// ordinary blocks after final activation. It is not a substitute for finality.
pub fn lineage_marker(policy: [u8; 32]) -> Vec<u8> {
    (LINEAGE_MAGIC, CHECKPOINT_HASH, policy).encode()
}

pub fn parse_lineage_marker(bytes: &[u8]) -> Option<[u8; 32]> {
    let mut input = bytes;
    let (magic, checkpoint, policy) = <([u8; 16], [u8; 32], [u8; 32])>::decode(&mut input).ok()?;
    (input.is_empty() && magic == LINEAGE_MAGIC && checkpoint == CHECKPOINT_HASH).then_some(policy)
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct AccountReplacement {
    pub old: AccountId32,
    pub new: AccountId32,
}

/// Domain-separated proof for an independently controlled replacement key.
/// Derived accounts require their controlling-key/derivation evidence instead.
pub fn replacement_ownership_message(replacement: &AccountReplacement) -> Vec<u8> {
    (
        b"sora-nexus/replacement-ownership/v1".as_slice(),
        GENESIS_HASH,
        CHECKPOINT_HASH,
        replacement,
    )
        .encode()
}

pub fn verify_replacement_ownership(
    replacement: &AccountReplacement,
    signature: &MultiSignature,
) -> bool {
    signature.verify(
        replacement_ownership_message(replacement).as_slice(),
        &replacement.new,
    )
}

/// Validates the canonical map, not destination non-use or beneficial ownership.
/// Those require the complete typed checkpoint inventory and signed evidence.
pub fn validate_replacements(map: &[AccountReplacement]) -> Result<(), RecoveryError> {
    let mut old = BTreeSet::new();
    let mut new = BTreeSet::new();
    let mut previous = None;
    for entry in map {
        if entry.old == entry.new
            || entry.old == AccountId32::from(ATTACKER_ACCOUNT)
            || entry.new == AccountId32::from(ATTACKER_ACCOUNT)
            || previous.as_ref().is_some_and(|prior| prior >= &entry.old)
            || !old.insert(entry.old.clone())
            || !new.insert(entry.new.clone())
        {
            return Err(RecoveryError::InvalidMapping);
        }
        previous = Some(entry.old.clone());
    }
    if !old.is_disjoint(&new) {
        return Err(RecoveryError::InvalidMapping);
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum StorageScope {
    Top,
    /// Default child-trie identifier, not the parent trie root value.
    Child(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct GuardedStorageEdit {
    pub scope: StorageScope,
    pub key: Vec<u8>,
    /// None means absent. Some(hash(empty)) is intentionally different.
    pub before: Option<[u8; 32]>,
    pub after: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryChunk {
    pub index: u32,
    pub edits: Vec<GuardedStorageEdit>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct StorageWitness {
    pub scope: StorageScope,
    pub key: Vec<u8>,
    pub value_hash: Option<[u8; 32]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryAuditChunk {
    pub index: u32,
    pub witnesses: Vec<StorageWitness>,
}

/// A complete empty namespace, rather than the absence of one guessed map key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct StoragePrefixWitness {
    pub scope: StorageScope,
    pub prefix: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryPrefixAuditChunk {
    pub index: u32,
    pub prefixes: Vec<StoragePrefixWitness>,
}

impl RecoveryPrefixAuditChunk {
    pub fn commitment(&self) -> [u8; 32] {
        // Separate the semantic domain from exact-key audit encodings.
        sp_io::hashing::blake2_256(&(b"sora-nexus/empty-prefix-audit/v1".as_slice(), self).encode())
    }

    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.prefixes.is_empty()
            || self.prefixes.len() > MAX_EDITS_PER_CHUNK
            || self.encoded_size() > MAX_CHUNK_BYTES
        {
            return Err(RecoveryError::ChunkBounds);
        }
        let mut previous: Option<(Option<Vec<u8>>, Vec<u8>)> = None;
        for witness in &self.prefixes {
            let scope = match &witness.scope {
                StorageScope::Top => None,
                StorageScope::Child(id) if !id.is_empty() && id.len() <= MAX_STORAGE_KEY_BYTES => {
                    Some(id.clone())
                }
                _ => return Err(RecoveryError::ChunkBounds),
            };
            if witness.prefix.is_empty() || witness.prefix.len() > MAX_STORAGE_KEY_BYTES {
                return Err(RecoveryError::ChunkBounds);
            }
            let current = (scope, witness.prefix.clone());
            if previous.as_ref().is_some_and(|prior| {
                prior >= &current || (prior.0 == current.0 && current.1.starts_with(&prior.1))
            }) {
                return Err(RecoveryError::DuplicateEdit);
            }
            previous = Some(current);
        }
        Ok(())
    }
}

impl RecoveryAuditChunk {
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(&self.encode())
    }

    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.witnesses.is_empty()
            || self.witnesses.len() > MAX_AUDIT_WITNESSES
            || self.encoded_size() > MAX_CHUNK_BYTES
        {
            return Err(RecoveryError::ChunkBounds);
        }
        let mut seen = BTreeSet::new();
        for witness in &self.witnesses {
            let child = match &witness.scope {
                StorageScope::Top => None,
                StorageScope::Child(id) if !id.is_empty() && id.len() <= MAX_STORAGE_KEY_BYTES => {
                    Some(id)
                }
                _ => return Err(RecoveryError::ChunkBounds),
            };
            if witness.key.is_empty() || witness.key.len() > MAX_STORAGE_KEY_BYTES {
                return Err(RecoveryError::ChunkBounds);
            }
            if !seen.insert((child, &witness.key)) {
                return Err(RecoveryError::DuplicateEdit);
            }
        }
        Ok(())
    }
}

impl RecoveryChunk {
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(&self.encode())
    }

    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.edits.is_empty()
            || self.edits.len() > MAX_EDITS_PER_CHUNK
            || self.encoded_size() > MAX_CHUNK_BYTES
        {
            return Err(RecoveryError::ChunkBounds);
        }
        let mut seen = BTreeSet::new();
        for edit in &self.edits {
            let child = match &edit.scope {
                StorageScope::Top => None,
                StorageScope::Child(id) => {
                    if id.is_empty() || id.len() > MAX_STORAGE_KEY_BYTES {
                        return Err(RecoveryError::ChunkBounds);
                    }
                    Some(id.clone())
                }
            };
            if edit.key.is_empty() || edit.key.len() > MAX_STORAGE_KEY_BYTES {
                return Err(RecoveryError::ChunkBounds);
            }
            // Runtime installation and recovery control never use generic edits.
            if matches!(edit.scope, StorageScope::Top)
                && (edit.key.starts_with(STORAGE_PREFIX)
                    || edit.key == b":code"
                    || edit.key == b":heappages"
                    || edit.key == b":extrinsic_index"
                    || edit.key == b":intrablock_entropy"
                    || edit.key.starts_with(b":transaction_level:")
                    || edit.key.starts_with(b":child_storage:")
                    || structural_storage(&edit.key))
            {
                return Err(RecoveryError::ProtectedStorage);
            }
            if !seen.insert((child, edit.key.clone())) {
                return Err(RecoveryError::DuplicateEdit);
            }
        }
        Ok(())
    }
}

/// Generic ownership edits cannot rewrite header history, executive bookkeeping
/// or consensus progression. Dedicated maintenance routines own those changes.
fn structural_storage(key: &[u8]) -> bool {
    if key.len() < 32 {
        return false;
    }
    for pallet in [b"Babe".as_slice(), b"Grandpa"] {
        if key.starts_with(&sp_io::hashing::twox_128(pallet)) {
            return true;
        }
    }
    let fields: &[(&[u8], &[&[u8]])] = &[
        (
            b"System",
            &[
                b"Number",
                b"ParentHash",
                b"BlockHash",
                b"Digest",
                b"ExecutionPhase",
                b"ExtrinsicIndex",
                b"ExtrinsicData",
                b"ExtrinsicCount",
                b"Events",
                b"EventCount",
                b"EventTopics",
                b"BlockWeight",
                b"AllExtrinsicsLen",
                b"LastRuntimeUpgrade",
            ],
        ),
        (b"Session", &[b"CurrentIndex", b"QueuedChanged"]),
        (
            b"Staking",
            &[b"CurrentEra", b"ActiveEra", b"ErasStartSessionIndex"],
        ),
    ];
    fields.iter().any(|(pallet, entries)| {
        key[..16] == sp_io::hashing::twox_128(pallet)
            && entries
                .iter()
                .any(|entry| key[16..32] == sp_io::hashing::twox_128(entry))
    })
}

/// Independent, acyclic identity for one recovery network/release. Synthetic
/// releases use distinct domains; exact-artifact rehearsals additionally bind
/// their unsealed first-child hash, distinct from a sealed production branch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryDomain {
    pub chain_spec_id: Vec<u8>,
    pub release_nonce: [u8; 32],
}

impl RecoveryDomain {
    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.chain_spec_id.is_empty()
            || self.chain_spec_id.len() > 128
            || !self
                .chain_spec_id
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b'_')
            || self.release_nonce == [0; 32]
        {
            return Err(RecoveryError::InvalidPolicy);
        }
        Ok(())
    }

    /// Public synthetic identity. Never an implicit production compiler default.
    pub fn public_fixture() -> Self {
        Self {
            chain_spec_id: b"sora-nexus-PUBLIC-FIXTURE-ONLY".to_vec(),
            release_nonce: [0x46; 32],
        }
    }
}

/// Data/policy commitment only. Artifact hashes are recorded in an outer signed
/// release manifest; the bootstrap separately pins final/hold Wasm hashes.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryPolicy {
    pub format_version: u32,
    pub checkpoint_hash: [u8; 32],
    pub checkpoint_state_root: [u8; 32],
    pub replacements: Vec<AccountReplacement>,
    pub council: Vec<AccountId32>,
    pub chunk_hashes: Vec<[u8; 32]>,
    pub audit_chunk_hashes: Vec<[u8; 32]>,
    pub authorization_domain: RecoveryDomain,
    pub evidence_commitment: [u8; 32],
    pub terminal_audit_commitment: [u8; 32],
}

impl RecoveryPolicy {
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(&self.encode())
    }

    pub fn validate(&self) -> Result<(), RecoveryError> {
        if self.format_version != 2
            || self.checkpoint_hash != CHECKPOINT_HASH
            || self.checkpoint_state_root != CHECKPOINT_STATE_ROOT
            || self.chunk_hashes.is_empty()
            || self.chunk_hashes.len() > MAX_WORK_CHUNKS
            || self.audit_chunk_hashes.is_empty()
            || self.audit_chunk_hashes.len() > MAX_WORK_CHUNKS
            || self.replacements.len() > MAX_REPLACEMENTS
            || self.evidence_commitment == [0; 32]
            || self.terminal_audit_commitment == [0; 32]
            || self.council.is_empty()
            || self.council.len() > MAX_COUNCIL_MEMBERS
        {
            return Err(RecoveryError::InvalidPolicy);
        }
        self.authorization_domain.validate()?;
        validate_replacements(&self.replacements)?;
        let retired: BTreeSet<_> = self.replacements.iter().map(|e| &e.old).collect();
        let council: BTreeSet<_> = self.council.iter().collect();
        if council.len() != self.council.len()
            || self.council.windows(2).any(|pair| pair[0] >= pair[1])
            || council.iter().any(|member| retired.contains(*member))
            || council.contains(&AccountId32::from(ATTACKER_ACCOUNT))
        {
            return Err(RecoveryError::InvalidCouncil);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum RecoveryPhase {
    Applying,
    Auditing,
    Ready,
    Hold,
    Complete,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryStatus {
    pub format_version: u32,
    pub policy: [u8; 32],
    pub phase: RecoveryPhase,
    pub next_chunk: u32,
    pub operation_counter: u64,
    pub failure: Option<RecoveryError>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum RecoveryError {
    InvalidPolicy,
    InvalidMapping,
    InvalidCouncil,
    ChunkBounds,
    ProtectedStorage,
    DuplicateEdit,
    WrongChunk,
    WrongPreimage,
    WrongPhase,
    WrongPolicy,
    BadQuorum,
    BadCode,
    Replay,
    IncompleteAudit,
    MissingCompiledPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum RecoveryCodeKind {
    Final,
    Hold,
    Repair,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum RecoveryAction {
    Tick,
    Apply(RecoveryChunk),
    Audit(RecoveryAuditChunk),
    StageCode {
        kind: RecoveryCodeKind,
        index: u32,
        bytes: Vec<u8>,
    },
    Escape {
        authorization: Box<RepairAuthorization>,
        signatures: Vec<(AccountId32, MultiSignature)>,
        work_policy: Option<RecoveryPolicy>,
    },
    TerminalAudit(invariants::TerminalAudit),
    Incoming(Vec<u8>),
    /// Permanent, evidence-bound outgoing cancellation without an implicit refund.
    /// Appended to preserve all previously defined recovery action discriminants.
    Outgoing(Vec<u8>),
    /// Independently committed namespace absence. Appended for codec stability.
    PrefixAudit(RecoveryPrefixAuditChunk),
    /// Typed authenticated incoming post-state audit. Appended at index9.
    IncomingAudit(Vec<u8>),
    /// Explicit native-derived account balance audit; appended at index 10.
    NativeBalanceAudit(Vec<u8>),
    /// Bounded terminal-audit page; appended at index 11 for codec stability.
    TerminalAuditPage(invariants::TerminalAuditPage),
}

/// Transported as a bare System::remark only by the recovery executive, above
/// the checkpoint. Ordinary System dispatch must never execute this envelope.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RecoveryInherent {
    pub format_version: u8,
    pub action: RecoveryAction,
}

impl RecoveryInherent {
    pub fn encode_remark(&self) -> Vec<u8> {
        let mut bytes = INHERENT_IDENTIFIER.to_vec();
        bytes.extend(self.encode());
        bytes
    }

    pub fn decode_remark(bytes: &[u8]) -> Result<Self, RecoveryError> {
        if bytes.len() > MAX_INHERENT_BYTES || !bytes.starts_with(&INHERENT_IDENTIFIER) {
            return Err(RecoveryError::ChunkBounds);
        }
        let mut input = &bytes[INHERENT_IDENTIFIER.len()..];
        let envelope = Self::decode_with_mem_limit(&mut input, MAX_INHERENT_BYTES * 3)
            .map_err(|_| RecoveryError::WrongPreimage)?;
        if !input.is_empty() || envelope.format_version != 1 {
            return Err(RecoveryError::WrongPreimage);
        }
        match &envelope.action {
            RecoveryAction::PrefixAudit(chunk) => chunk.validate()?,
            RecoveryAction::Incoming(bytes)
            | RecoveryAction::Outgoing(bytes)
            | RecoveryAction::IncomingAudit(bytes)
            | RecoveryAction::NativeBalanceAudit(bytes)
                if bytes.is_empty() || bytes.len() > MAX_CHUNK_BYTES =>
            {
                return Err(RecoveryError::ChunkBounds);
            }
            RecoveryAction::Incoming(_)
            | RecoveryAction::Outgoing(_)
            | RecoveryAction::IncomingAudit(_)
            | RecoveryAction::NativeBalanceAudit(_) => {}
            RecoveryAction::TerminalAudit(audit) => audit.validate_bounds()?,
            RecoveryAction::TerminalAuditPage(page) => page.validate()?,
            RecoveryAction::Apply(chunk) => chunk.validate()?,
            RecoveryAction::StageCode { bytes, .. } => {
                if bytes.is_empty() || bytes.len() > MAX_CODE_CHUNK_BYTES {
                    return Err(RecoveryError::ChunkBounds);
                }
            }
            RecoveryAction::Escape {
                signatures,
                work_policy,
                ..
            } => {
                if signatures.is_empty() || signatures.len() > MAX_COUNCIL_MEMBERS {
                    return Err(RecoveryError::BadQuorum);
                }
                if let Some(policy) = work_policy {
                    policy.validate()?;
                }
            }
            RecoveryAction::Audit(audit) => audit.validate()?,
            RecoveryAction::Tick => {}
        }
        Ok(envelope)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct RepairAuthorization {
    pub policy: [u8; 32],
    pub phase: RecoveryPhase,
    pub cursor: u32,
    pub operation_counter: u64,
    pub current_code_hash: [u8; 32],
    /// Actual executor identity. For spec 134 the node checks the immutable
    /// substitute; for installed recovery runtimes this must equal :code.
    pub current_executor_hash: [u8; 32],
    pub destination_code_hash: [u8; 32],
    pub destination_spec: u32,
    /// Zero for the pinned hold runtime; a repair binds reviewed new data here.
    pub revised_policy: [u8; 32],
    pub authorization_domain: RecoveryDomain,
    /// Canonical first recovery child (including its production seal, if present).
    pub recovery_anchor: [u8; 32],
}

impl RepairAuthorization {
    pub fn signing_message(&self) -> Vec<u8> {
        (
            b"sora-nexus/recovery-forward-upgrade/v2".as_slice(),
            GENESIS_HASH,
            CHECKPOINT_HASH,
            self,
        )
            .encode()
    }

    pub fn verify_quorum(
        &self,
        policy: &RecoveryPolicy,
        status: &RecoveryStatus,
        expected_anchor: [u8; 32],
        signatures: &[(AccountId32, MultiSignature)],
    ) -> Result<(), RecoveryError> {
        policy.validate()?;
        if self.authorization_domain != policy.authorization_domain
            || self.policy != policy.commitment()
            || self.policy != status.policy
        {
            return Err(RecoveryError::WrongPolicy);
        }
        if expected_anchor == [0; 32]
            || self.recovery_anchor != expected_anchor
            || self.phase != status.phase
            || self.cursor != status.next_chunk
            || self.operation_counter != status.operation_counter
        {
            return Err(RecoveryError::Replay);
        }
        if signatures.len() <= policy.council.len() / 2 || signatures.len() > policy.council.len() {
            return Err(RecoveryError::BadQuorum);
        }
        let message = self.signing_message();
        let mut signers = BTreeSet::new();
        for (account, signature) in signatures {
            if policy.council.binary_search(account).is_err()
                || !signers.insert(account)
                || !signature.verify(message.as_slice(), account)
            {
                return Err(RecoveryError::BadQuorum);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::{sr25519, Pair};

    #[test]
    fn lineage_rejects_foreign_checkpoint_and_trailing_bytes() {
        let marker = lineage_marker([9; 32]);
        assert_eq!(marker.len(), 80);
        assert_eq!(parse_lineage_marker(&marker), Some([9; 32]));
        let mut foreign = marker.clone();
        foreign[16] ^= 1;
        assert_eq!(parse_lineage_marker(&foreign), None);
        let mut trailing = marker;
        trailing.push(0);
        assert_eq!(parse_lineage_marker(&trailing), None);
    }

    #[test]
    fn outgoing_transport_is_appended_and_bounded() {
        assert_eq!(RecoveryAction::Incoming(vec![1]).encode()[0], 6);
        assert_eq!(RecoveryAction::Outgoing(vec![1]).encode()[0], 7);
        for payload in [vec![], vec![1; MAX_CHUNK_BYTES + 1]] {
            let envelope = RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::Outgoing(payload),
            };
            assert_eq!(
                RecoveryInherent::decode_remark(&envelope.encode_remark()),
                Err(RecoveryError::ChunkBounds)
            );
        }
        let envelope = RecoveryInherent {
            format_version: 1,
            action: RecoveryAction::Outgoing(vec![1; MAX_CHUNK_BYTES]),
        };
        assert_eq!(
            RecoveryInherent::decode_remark(&envelope.encode_remark()),
            Ok(envelope)
        );
    }

    #[test]
    fn mappings_reject_collisions_chains_and_attacker_overlap() {
        let entry = |old, new| AccountReplacement {
            old: AccountId32::from([old; 32]),
            new: AccountId32::from([new; 32]),
        };
        assert_eq!(validate_replacements(&[entry(1, 3), entry(2, 4)]), Ok(()));
        for map in [
            vec![entry(1, 1)],
            vec![entry(1, 3), entry(2, 3)],
            vec![entry(1, 2), entry(2, 3)],
            vec![entry(2, 4), entry(1, 3)],
            vec![AccountReplacement {
                old: AccountId32::from(ATTACKER_ACCOUNT),
                new: AccountId32::from([1; 32]),
            }],
        ] {
            assert_eq!(
                validate_replacements(&map),
                Err(RecoveryError::InvalidMapping)
            );
        }
    }

    #[test]
    fn replacement_proof_binds_both_accounts() {
        let new_key = sr25519::Pair::from_seed(&[7; 32]);
        let mut replacement = AccountReplacement {
            old: AccountId32::from([1; 32]),
            new: AccountId32::from(new_key.public().0),
        };
        let signature =
            MultiSignature::from(new_key.sign(&replacement_ownership_message(&replacement)));
        assert!(verify_replacement_ownership(&replacement, &signature));
        replacement.old = AccountId32::from([2; 32]);
        assert!(!verify_replacement_ownership(&replacement, &signature));
    }

    #[test]
    fn generic_chunks_cannot_install_code_or_rewrite_control_state() {
        for key in [b":code".to_vec(), POLICY_STORAGE_KEY.to_vec()] {
            let chunk = RecoveryChunk {
                index: 0,
                edits: vec![GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key,
                    before: None,
                    after: Some(vec![1]),
                }],
            };
            assert_eq!(chunk.validate(), Err(RecoveryError::ProtectedStorage));
        }
    }

    #[test]
    fn quorum_cannot_be_reused_at_another_cursor_or_count_duplicates() {
        let keys: Vec<_> = (1..=3)
            .map(|i| sr25519::Pair::from_seed(&[i; 32]))
            .collect();
        let mut council: Vec<_> = keys
            .iter()
            .map(|k| AccountId32::from(k.public().0))
            .collect();
        council.sort();
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: crate::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: CHECKPOINT_STATE_ROOT,
            replacements: Vec::new(),
            council,
            chunk_hashes: vec![[1; 32]],
            audit_chunk_hashes: vec![[6; 32]],
            evidence_commitment: [2; 32],
            terminal_audit_commitment: [3; 32],
        };
        let mut status = RecoveryStatus {
            format_version: 1,
            policy: policy.commitment(),
            phase: RecoveryPhase::Failed,
            next_chunk: 3,
            operation_counter: 0,
            failure: Some(RecoveryError::WrongPreimage),
        };
        let auth = RepairAuthorization {
            recovery_anchor: [0x42; 32],
            authorization_domain: policy.authorization_domain.clone(),
            policy: status.policy,
            phase: status.phase,
            cursor: status.next_chunk,
            operation_counter: 0,
            current_code_hash: [4; 32],
            current_executor_hash: [5; 32],
            destination_code_hash: [5; 32],
            destination_spec: HOLD_SPEC_VERSION,
            revised_policy: [0; 32],
        };
        let signatures: Vec<_> = keys
            .iter()
            .take(2)
            .map(|key| {
                (
                    AccountId32::from(key.public().0),
                    MultiSignature::from(key.sign(&auth.signing_message())),
                )
            })
            .collect();
        assert_eq!(
            auth.verify_quorum(&policy, &status, [0x42; 32], &signatures),
            Ok(())
        );
        assert_eq!(
            auth.verify_quorum(
                &policy,
                &status,
                [0x42; 32],
                &[signatures[0].clone(), signatures[0].clone()]
            ),
            Err(RecoveryError::BadQuorum)
        );
        // Even at the identical phase/cursor/counter with the same council,
        // code hashes and economic work, another network or release is distinct.
        for domain in [
            RecoveryDomain {
                chain_spec_id: b"another-network".to_vec(),
                ..policy.authorization_domain.clone()
            },
            RecoveryDomain {
                release_nonce: [0x47; 32],
                ..policy.authorization_domain.clone()
            },
        ] {
            let mut other_policy = policy.clone();
            other_policy.authorization_domain = domain.clone();
            let mut other_status = status.clone();
            other_status.policy = other_policy.commitment();
            assert_eq!(
                auth.verify_quorum(&other_policy, &other_status, [0x42; 32], &signatures),
                Err(RecoveryError::WrongPolicy)
            );
            let mut other_auth = auth.clone();
            other_auth.policy = other_status.policy;
            other_auth.authorization_domain = domain;
            assert_ne!(auth.signing_message(), other_auth.signing_message());
            assert_eq!(
                other_auth.verify_quorum(&other_policy, &other_status, [0x42; 32], &signatures),
                Err(RecoveryError::BadQuorum)
            );
            let fresh: Vec<_> = keys
                .iter()
                .take(2)
                .map(|key| {
                    (
                        AccountId32::from(key.public().0),
                        MultiSignature::from(key.sign(&other_auth.signing_message())),
                    )
                })
                .collect();
            assert_eq!(
                other_auth.verify_quorum(&other_policy, &other_status, [0x42; 32], &fresh),
                Ok(())
            );
        }
        assert_eq!(
            auth.verify_quorum(&policy, &status, [0; 32], &signatures),
            Err(RecoveryError::Replay)
        );
        assert_eq!(
            auth.verify_quorum(&policy, &status, [0x43; 32], &signatures),
            Err(RecoveryError::Replay)
        );
        let mut different_branch = auth.clone();
        different_branch.recovery_anchor = [0x43; 32];
        assert_eq!(
            different_branch.verify_quorum(&policy, &status, [0x43; 32], &signatures),
            Err(RecoveryError::BadQuorum)
        );
        let mut legacy = policy.clone();
        legacy.format_version = 1;
        assert_eq!(legacy.validate(), Err(RecoveryError::InvalidPolicy));
        for domain in [
            RecoveryDomain {
                chain_spec_id: vec![],
                ..policy.authorization_domain.clone()
            },
            RecoveryDomain {
                chain_spec_id: vec![b'x'; 129],
                ..policy.authorization_domain.clone()
            },
            RecoveryDomain {
                chain_spec_id: b"ambiguous chain".to_vec(),
                ..policy.authorization_domain.clone()
            },
            RecoveryDomain {
                release_nonce: [0; 32],
                ..policy.authorization_domain.clone()
            },
        ] {
            assert_eq!(domain.validate(), Err(RecoveryError::InvalidPolicy));
        }
        status.next_chunk += 1;
        assert_eq!(
            auth.verify_quorum(&policy, &status, [0x42; 32], &signatures),
            Err(RecoveryError::Replay)
        );
    }
    #[test]
    fn native_balance_audit_is_appended_without_changing_older_audit_transport() {
        assert_eq!(RecoveryAction::IncomingAudit(vec![1]).encode()[0], 9);
        assert_eq!(RecoveryAction::NativeBalanceAudit(vec![1]).encode()[0], 10);
        for payload in [vec![], vec![1; MAX_CHUNK_BYTES + 1]] {
            let envelope = RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::NativeBalanceAudit(payload),
            };
            assert!(RecoveryInherent::decode_remark(&envelope.encode_remark()).is_err());
        }
        let envelope = RecoveryInherent {
            format_version: 1,
            action: RecoveryAction::NativeBalanceAudit(vec![1]),
        };
        assert_eq!(
            RecoveryInherent::decode_remark(&envelope.encode_remark()).unwrap(),
            envelope
        );
    }
}
