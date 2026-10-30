//! Read-only HASHI preparation against an already admitted live recovery client.
//!
//! This module is deliberately not an RPC, signer, worker, or block producer.
//! The constructor must receive the same Configuration used to construct the
//! client: `code_at` includes substitutes but deliberately ignores overrides.
//! The normal recovery startup guard forbids overrides; this constructor repeats
//! that guard. A caller cannot supply an arbitrary method or runtime extension.

use codec::{Compact, Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{self, RecoveryPhase, RecoveryPolicy, RecoveryStatus};
use eth_bridge_runtime_api::recovery::{RecoveryHashiAdmission, RecoveryHashiPreparation};
use framenode_chain_spec::recovery::RecoveryConfig;
use framenode_runtime::{
    incident_recovery_incoming::{HashiPeerApproval, IncomingChannel, IncomingOperation},
    opaque::Block,
};
use sc_client_api::{CallExecutor, ExecutorProvider, StorageProvider};
use serde::Deserialize;
use sp_blockchain::HeaderBackend;
use sp_core::{storage::StorageKey, H256};
use sp_externalities::Extensions;
use sp_runtime::{
    generic::Era,
    traits::{HashingFor, Header as _},
    DispatchError,
};
use sp_state_machine::OverlayedChanges;
use std::{cell::RefCell, collections::BTreeSet, sync::Arc};

const MAX_JSON: usize = 16 * 1024 * 1024;
const MAX_METADATA: usize = 16 * 1024 * 1024;
const MAX_CODE: usize = 64 * 1024 * 1024;
const MAX_RESPONSE: usize = 2 * 1024 * 1024;
const MAX_APPROVALS: usize = 100;

type Header = <Block as sp_runtime::traits::Block>::Header;
type Result<T> = std::result::Result<T, String>;

fn require(condition: bool, message: &str) -> Result<()> {
    condition.then_some(()).ok_or_else(|| message.to_owned())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(value, "{byte:02x}").expect("String writes cannot fail");
    }
    value
}

fn decode<T: DecodeWithMemTracking>(bytes: &[u8], limit: usize) -> Result<T> {
    require(
        !bytes.is_empty() && bytes.len() <= limit,
        "SCALE response/input exceeds bound",
    )?;
    let mut input = bytes;
    let value = T::decode_with_mem_limit(&mut input, limit.saturating_mul(4))
        .map_err(|_| "invalid bounded SCALE response/input")?;
    require(input.is_empty(), "trailing SCALE response/input bytes")?;
    Ok(value)
}

#[derive(Deserialize)]
struct Manifest {
    format: String,
    checkpoint: Checkpoint,
    policy: PolicyIdentity,
    artifacts: Vec<Artifact>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    number: u32,
    hash: H256,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PolicyIdentity {
    commitment: String,
    evidence_commitment: String,
}

#[derive(Deserialize)]
struct Artifact {
    role: String,
    bytes: usize,
    sha256: String,
    blake2_256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionPlan {
    schema: String,
    template_commitment: H256,
    network: u32,
    first_execution_block: u32,
    last_execution_block: u32,
    spec_version: u32,
    transaction_version: u32,
    execution_code_hash: H256,
    metadata_hash: H256,
}

/// Bounded artifact bytes, loaded by the local release coordinator. The expected
/// manifest hash is an independently admitted release pin, NOT a hash supplied
/// alongside an untrusted request. This helper does not replace release gates.
pub(crate) struct ReleaseArtifacts<'a> {
    pub expected_manifest_sha256: [u8; 32],
    pub manifest: &'a [u8],
    pub plan: &'a [u8],
    pub plan_role: &'a str,
    pub bootstrap: &'a [u8],
    pub metadata: &'a [u8],
    pub metadata_role: &'a str,
}

struct BoundRelease {
    manifest_sha256: [u8; 32],
    plan: ExecutionPlan,
    policy: RecoveryPolicy,
}

impl BoundRelease {
    fn new(config: &RecoveryConfig, artifacts: ReleaseArtifacts<'_>) -> Result<Self> {
        config.validate()?;
        require(
            artifacts.expected_manifest_sha256 != [0; 32]
                && artifacts.manifest.len() <= MAX_JSON
                && sp_io::hashing::sha2_256(artifacts.manifest)
                    == artifacts.expected_manifest_sha256,
            "release manifest differs from the independently admitted release pin",
        )?;
        require(
            !artifacts.plan.is_empty()
                && artifacts.plan.len() <= MAX_JSON
                && !artifacts.metadata.is_empty()
                && artifacts.metadata.len() <= MAX_METADATA
                && !artifacts.bootstrap.is_empty()
                && artifacts.bootstrap.len() <= MAX_CODE,
            "release artifact exceeds bound",
        )?;
        let manifest: Manifest =
            serde_json::from_slice(artifacts.manifest).map_err(|_| "invalid release manifest")?;
        require(
            manifest.format == "sora-nexus-recovery-release-v1"
                && manifest.checkpoint.number == recovery::CHECKPOINT_NUMBER
                && manifest.checkpoint.hash == H256::from(recovery::CHECKPOINT_HASH)
                && manifest.policy.commitment == hex(config.policy_commitment.as_bytes())
                && manifest.policy.evidence_commitment
                    == hex(config.evidence_commitment.as_bytes()),
            "only the original immutable recovery release is supported",
        )?;
        let roles = [
            "policyScale",
            "bootstrapWasm",
            artifacts.plan_role,
            artifacts.metadata_role,
        ];
        require(
            roles
                .iter()
                .all(|role| !role.is_empty() && role.len() <= 256)
                && roles.iter().collect::<BTreeSet<_>>().len() == roles.len()
                && manifest.artifacts.len() <= 16_384,
            "release artifact role bounds/identity differ",
        )?;
        let mut seen = BTreeSet::new();
        for row in &manifest.artifacts {
            require(
                !row.role.is_empty() && seen.insert(row.role.as_str()),
                "duplicate release artifact role",
            )?;
        }
        for (role, data) in [
            ("policyScale", config.policy_scale.0.as_slice()),
            ("bootstrapWasm", artifacts.bootstrap),
            (artifacts.plan_role, artifacts.plan),
            (artifacts.metadata_role, artifacts.metadata),
        ] {
            let row = manifest
                .artifacts
                .iter()
                .find(|row| row.role == role)
                .ok_or_else(|| format!("release artifact role is absent: {role}"))?;
            require(
                row.bytes == data.len()
                    && row.sha256 == hex(&sp_io::hashing::sha2_256(data))
                    && row.blake2_256 == hex(&sp_io::hashing::blake2_256(data)),
                "release artifact bytes differ",
            )?;
        }
        let plan: ExecutionPlan = serde_json::from_slice(artifacts.plan)
            .map_err(|_| "invalid immutable execution plan")?;
        require(
            plan.schema == "sora-nexus-hashi-execution-plan-v1"
                && plan.spec_version == 134
                && plan.transaction_version == 131
                && plan.first_execution_block > recovery::CHECKPOINT_NUMBER + 2
                && plan.first_execution_block <= plan.last_execution_block
                && plan.execution_code_hash == config.bootstrap_code_hash
                && plan.execution_code_hash.0 == sp_io::hashing::blake2_256(artifacts.bootstrap)
                && plan.metadata_hash.0 == sp_io::hashing::blake2_256(artifacts.metadata),
            "execution plan code, metadata, version or absolute window differs",
        )?;
        let policy: RecoveryPolicy = decode(&config.policy_scale.0, 2 * 1024 * 1024)?;
        policy.validate().map_err(|_| "invalid recovery policy")?;
        require(
            policy.encode() == config.policy_scale.0,
            "noncanonical policy artifact",
        )?;
        Ok(Self {
            manifest_sha256: artifacts.expected_manifest_sha256,
            plan,
            policy,
        })
    }

    fn operation(&self, bytes: &[u8]) -> Result<IncomingOperation> {
        let operation: IncomingOperation = decode(bytes, recovery::MAX_CHUNK_BYTES)?;
        require(
            operation.encode() == bytes,
            "noncanonical incoming operation",
        )?;
        let (network, approvals) = hashi(&operation)?;
        require(
            network == self.plan.network
                && operation.commitment() == self.plan.template_commitment.0
                && self.policy.chunk_hashes.get(operation.index as usize)
                    == Some(&operation.commitment())
                && operation.finalized_source_evidence != [0; 32]
                && !approvals.is_empty()
                && approvals.len() <= MAX_APPROVALS,
            "incoming operation is not the release-committed HASHI work",
        )?;
        for approval in approvals {
            require(
                !self
                    .policy
                    .replacements
                    .iter()
                    .any(|row| row.old == approval.signer),
                "retired HASHI signer",
            )?;
            approval_window(approval, &self.plan)?;
        }
        Ok(operation)
    }
}

fn hashi(operation: &IncomingOperation) -> Result<(u32, &[HashiPeerApproval])> {
    match &operation.channel {
        IncomingChannel::Hashi {
            network, approvals, ..
        }
        | IncomingChannel::HashiAuthenticatedSource {
            network, approvals, ..
        } => Ok((*network, approvals)),
        _ => Err("only native HASHI operations are supported".into()),
    }
}

/// Parse only the stable wire prefix. Native API admission separately decodes
/// the full native SignedExtra and RuntimeCall; there is no second call decoder.
fn approval_window(approval: &HashiPeerApproval, plan: &ExecutionPlan) -> Result<(u32, u64, u64)> {
    require(approval.extra.len() <= 128, "approval extra exceeds bound")?;
    let mut bytes = approval.extra.as_slice();
    let era = Era::decode(&mut bytes).map_err(|_| "invalid approval era")?;
    let Compact(nonce) =
        Compact::<u32>::decode(&mut bytes).map_err(|_| "invalid approval nonce")?;
    let Compact(tip) = Compact::<u128>::decode(&mut bytes).map_err(|_| "invalid approval tip")?;
    require(
        bytes.is_empty()
            && tip == 0
            && !era.is_immortal()
            && (era, Compact(nonce), Compact(tip)).encode() == approval.extra,
        "noncanonical or unsupported approval extra",
    )?;
    let first = u64::from(plan.first_execution_block);
    let last = u64::from(plan.last_execution_block);
    let birth = era.birth(first);
    let death = era.death(first);
    require(
        birth > u64::from(recovery::CHECKPOINT_NUMBER)
            && birth < first
            && era.birth(last) == birth
            && last < death,
        "approval era does not cover the immutable absolute execution window",
    )?;
    Ok((nonce, birth, death))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Parent {
    header: Header,
    anchor: [u8; 32],
    stored_code_hash: [u8; 32],
    status: RecoveryStatus,
}

/// The only concrete host adapter. It reuses an already admitted full client.
/// Construction creates no keystore, network, pool, telemetry, or offchain task.
pub(crate) struct LiveHashiAdapter {
    client: Arc<crate::service::FullClient>,
    recovery: RecoveryConfig,
    release: BoundRelease,
}

/// Constructed only at the actual service Configuration/client boundary.
struct AdmittedClient {
    client: Arc<crate::service::FullClient>,
    recovery: RecoveryConfig,
}

impl AdmittedClient {
    fn new(
        client: Arc<crate::service::FullClient>,
        config: &sc_service::Configuration,
    ) -> Result<Self> {
        crate::recovery::validate_configuration(config).map_err(|e| e.to_string())?;
        let recovery = framenode_chain_spec::recovery::config(&*config.chain_spec)
            .ok_or("recovery configuration is required")?
            .clone();
        crate::recovery::validate_database(&client, &recovery).map_err(|e| e.to_string())?;
        Ok(Self { client, recovery })
    }
}

/// Native results plus identities actually checked by the host. The node's
/// finalized head is NOT an independently exported GRANDPA verification proof.
pub(crate) struct HostWitness<T> {
    pub release_manifest_sha256: [u8; 32],
    pub executing_code_hash: [u8; 32],
    pub executing_metadata_hash: [u8; 32],
    pub finalized_parent: H256,
    pub native: T,
}

impl LiveHashiAdapter {
    fn new(context: &AdmittedClient, artifacts: ReleaseArtifacts<'_>) -> Result<Self> {
        let release = BoundRelease::new(&context.recovery, artifacts)?;
        Ok(Self {
            client: context.client.clone(),
            recovery: context.recovery.clone(),
            release,
        })
    }

    fn storage(&self, at: H256, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.client
            .storage(at, &StorageKey(key.to_vec()))
            .map(|value| value.map(|value| value.0))
            .map_err(|e| e.to_string())
    }

    fn parent(&self, expected: H256, operation: &IncomingOperation) -> Result<Parent> {
        // Best may be ahead. Only the actual finalized head is a signing parent.
        require(
            self.client.info().finalized_hash == expected,
            "stale/nonfinalized recovery parent",
        )?;
        crate::recovery::validate_database(&self.client, &self.recovery)
            .map_err(|e| e.to_string())?;
        let header = self
            .client
            .header(expected)
            .map_err(|e| e.to_string())?
            .ok_or("finalized parent header is unavailable")?;
        let child = header
            .number()
            .checked_add(1)
            .ok_or("execution number overflow")?;
        require(
            header.hash() == expected
                && *header.number() > recovery::CHECKPOINT_NUMBER + 1
                && child >= self.release.plan.first_execution_block
                && child <= self.release.plan.last_execution_block,
            "finalized parent is outside the immutable execution window",
        )?;
        require(
            sp_io::hashing::blake2_256(&self.client.code_at(expected).map_err(|e| e.to_string())?)
                == self.release.plan.execution_code_hash.0,
            "executing substitute differs from the immutable release",
        )?;
        let commitment = self.release.policy.commitment();
        let markers: Vec<_> = header
            .digest()
            .logs()
            .iter()
            .filter_map(|item| item.as_other().and_then(recovery::parse_lineage_marker))
            .collect();
        require(
            markers == [commitment],
            "finalized recovery lineage differs",
        )?;
        for key in [
            recovery::POLICY_STORAGE_KEY,
            recovery::ACTIVE_POLICY_STORAGE_KEY,
        ] {
            require(
                self.storage(expected, key)?.as_deref() == Some(commitment.as_slice()),
                "original recovery policy is not active",
            )?;
        }
        require(
            self.storage(expected, recovery::BRIDGE_HALT_STORAGE_KEY)?
                .as_deref()
                == Some(&[1]),
            "bridge halt is not active",
        )?;
        for key in recovery::TRANSIENT_CONTEXT_KEYS {
            require(
                self.storage(expected, key)?.is_none(),
                "transient recovery context exists at finalized parent",
            )?;
        }
        let status: RecoveryStatus = decode(
            &self
                .storage(expected, recovery::STATUS_STORAGE_KEY)?
                .ok_or("missing recovery status")?,
            MAX_RESPONSE,
        )?;
        require(
            status.format_version == 1
                && status.policy == commitment
                && status.phase == RecoveryPhase::Applying
                && status.operation_counter == 0
                && status.failure.is_none()
                && status.next_chunk == operation.index,
            "finalized recovery phase/cursor differs",
        )?;
        let anchor: [u8; 32] = decode(
            &self
                .storage(expected, recovery::RECOVERY_ANCHOR_STORAGE_KEY)?
                .ok_or("missing recovery anchor")?,
            32,
        )?;
        require(anchor != [0; 32], "empty recovery anchor")?;
        let first = self
            .client
            .header(anchor.into())
            .map_err(|e| e.to_string())?
            .ok_or("recovery anchor header unavailable")?;
        require(
            first.hash() == H256::from(anchor)
                && *first.number() == recovery::CHECKPOINT_NUMBER + 1
                && first.parent_hash() == &H256::from(recovery::CHECKPOINT_HASH),
            "recovery anchor header differs",
        )?;
        let stored_code = self
            .storage(expected, b":code")?
            .ok_or("missing stored runtime code")?;
        Ok(Parent {
            header,
            anchor,
            stored_code_hash: sp_io::hashing::blake2_256(&stored_code),
            status,
        })
    }

    fn call(&self, parent: H256, method: Method, arguments: &[u8]) -> Result<Vec<u8>> {
        disposable_call(method, |changes, extensions| {
            self.client
                .executor()
                .contextual_call(
                    parent,
                    method.name(),
                    arguments,
                    changes,
                    &None,
                    sp_core::traits::CallContext::Onchain { import: false },
                    extensions,
                )
                .map_err(|e| e.to_string())
        })
    }

    fn metadata(&self, parent: H256) -> Result<()> {
        // OpaqueMetadata SCALE is a Vec<u8>; use actual executing code identity,
        // never a spec-version metadata cache (the substitute retains spec134).
        let encoded = self.call(parent, Method::Metadata, &[])?;
        let metadata: Vec<u8> = decode(&encoded, MAX_METADATA + 8)?;
        require(
            !metadata.is_empty()
                && metadata.len() <= MAX_METADATA
                && sp_io::hashing::blake2_256(&metadata) == self.release.plan.metadata_hash.0,
            "executing metadata differs from the immutable release artifact",
        )
    }

    fn witness<T>(&self, parent: H256, native: T) -> HostWitness<T> {
        HostWitness {
            release_manifest_sha256: self.release.manifest_sha256,
            executing_code_hash: self.release.plan.execution_code_hash.0,
            executing_metadata_hash: self.release.plan.metadata_hash.0,
            finalized_parent: parent,
            native,
        }
    }

    pub(crate) fn prepare(
        &self,
        expected_parent: H256,
        operation: &[u8],
    ) -> Result<HostWitness<RecoveryHashiPreparation>> {
        let typed = self.release.operation(operation)?;
        let parent = self.parent(expected_parent, &typed)?;
        self.metadata(expected_parent)?;
        let encoded = self.call(
            expected_parent,
            Method::Prepare,
            &(parent.header.encode(), operation.to_vec()).encode(),
        )?;
        let result: std::result::Result<RecoveryHashiPreparation, DispatchError> =
            decode(&encoded, MAX_RESPONSE)?;
        let native = result.map_err(|e| format!("native HASHI preparation rejected: {e:?}"))?;
        validate_preparation(&self.release, &parent, &typed, &native)?;
        self.birth_headers(&native)?;
        require(
            self.parent(expected_parent, &typed)? == parent,
            "finalized context changed during preparation",
        )?;
        Ok(self.witness(expected_parent, native))
    }

    pub(crate) fn admit(
        &self,
        expected_parent: H256,
        operation: &[u8],
    ) -> Result<HostWitness<RecoveryHashiAdmission>> {
        let typed = self.release.operation(operation)?;
        let parent = self.parent(expected_parent, &typed)?;
        self.metadata(expected_parent)?;
        let encoded = self.call(
            expected_parent,
            Method::Admit,
            &(parent.header.encode(), operation.to_vec()).encode(),
        )?;
        let result: std::result::Result<RecoveryHashiAdmission, DispatchError> =
            decode(&encoded, MAX_RESPONSE)?;
        let native = result.map_err(|e| format!("native HASHI admission rejected: {e:?}"))?;
        validate_preparation(&self.release, &parent, &typed, &native.preparation)?;
        self.birth_headers(&native.preparation)?;
        let status: RecoveryStatus = decode(&native.resulting_status_scale, MAX_RESPONSE)?;
        let cursor = typed
            .index
            .checked_add(1)
            .ok_or("operation cursor overflow")?;
        let expected_phase = if cursor as usize == self.release.policy.chunk_hashes.len() {
            RecoveryPhase::Auditing
        } else {
            RecoveryPhase::Applying
        };
        require(
            native.operation_bytes_hash == sp_io::hashing::blake2_256(operation)
                && native.resulting_cursor == cursor
                && native.resulting_operation_counter == 0
                && status.format_version == 1
                && status.policy == self.release.policy.commitment()
                && status.phase == expected_phase
                && status.next_chunk == cursor
                && status.operation_counter == 0
                && status.failure.is_none()
                && status.encode() == native.resulting_status_scale
                && native.native_signatures_checked
                && native.native_incoming_executed
                && !native.persistent_state_changed
                && !native.external_finality_verified,
            "native HASHI admission response differs from the committed operation",
        )?;
        require(
            self.parent(expected_parent, &typed)? == parent,
            "finalized context changed during admission",
        )?;
        Ok(self.witness(expected_parent, native))
    }

    fn birth_headers(&self, native: &RecoveryHashiPreparation) -> Result<()> {
        for approval in &native.approvals {
            let header = self
                .client
                .header(approval.birth_hash.into())
                .map_err(|e| e.to_string())?
                .ok_or("native mortal birth header unavailable")?;
            require(
                header.hash() == H256::from(approval.birth_hash)
                    && u64::from(*header.number()) == approval.birth_number,
                "native mortal birth header differs",
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Method {
    Metadata,
    Prepare,
    Admit,
}

impl Method {
    fn name(self) -> &'static str {
        match self {
            Self::Metadata => "Metadata_metadata",
            Self::Prepare => "EthBridgeRuntimeApi_recovery_hashi_prepare",
            Self::Admit => "EthBridgeRuntimeApi_recovery_hashi_admit",
        }
    }
}

fn disposable_call(
    method: Method,
    run: impl FnOnce(
        &RefCell<OverlayedChanges<HashingFor<Block>>>,
        &RefCell<Extensions>,
    ) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let changes = RefCell::new(OverlayedChanges::<HashingFor<Block>>::default());
    // Deliberately bypass the live client's global extension factory. There is
    // no keystore, HTTP, transaction-pool or offchain-database extension.
    let extensions = RefCell::new(Extensions::new());
    let result = run(&changes, &extensions);
    ensure_discarded(&changes.borrow())?;
    let bytes = result?;
    let limit = if matches!(method, Method::Metadata) {
        MAX_METADATA + 8
    } else {
        MAX_RESPONSE
    };
    require(bytes.len() <= limit, "runtime API response exceeds bound")?;
    Ok(bytes)
}

fn ensure_discarded(changes: &OverlayedChanges<HashingFor<Block>>) -> Result<()> {
    require(
        changes.transaction_depth() == 0
            && changes.is_empty()
            && changes.offchain().clone().into_iter().next().is_none(),
        "read-only runtime API left storage/offchain changes in its disposable overlay",
    )
}

fn validate_preparation(
    release: &BoundRelease,
    parent: &Parent,
    operation: &IncomingOperation,
    native: &RecoveryHashiPreparation,
) -> Result<()> {
    let (network, approvals) = hashi(operation)?;
    require(
        native.format_version == 1
            && native.parent_hash == parent.header.hash().0
            && native.parent_state_root == parent.header.state_root().0
            && native.execution_number
                == parent
                    .header
                    .number()
                    .checked_add(1)
                    .ok_or("execution number overflow")?
            && native.policy_commitment == release.policy.commitment()
            && native.recovery_anchor == parent.anchor
            && native.operation_index == operation.index
            && native.operation_commitment == operation.commitment()
            && native.network == network
            && native.spec_version == 134
            && native.transaction_version == 131
            && native.stored_code_hash == parent.stored_code_hash
            && native.source_evidence_commitment == operation.finalized_source_evidence
            && !native.external_finality_verified
            && native.approvals.len() == approvals.len(),
        "native HASHI preparation response differs from the finalized release context",
    )?;
    for (expected, actual) in approvals.iter().zip(&native.approvals) {
        let (nonce, birth, death) = approval_window(expected, &release.plan)?;
        require(
            actual.signer == *AsRef::<[u8; 32]>::as_ref(&expected.signer)
                && actual.nonce == nonce
                && actual.birth_number == birth
                && actual.death_number == death
                && actual.birth_hash != [0; 32]
                && actual.call_hash == sp_io::hashing::blake2_256(&expected.call)
                && actual.extra_hash == sp_io::hashing::blake2_256(&expected.extra)
                && !actual.payload.is_empty()
                && actual.payload.len() <= 256,
            "native signer, payload or absolute mortal window differs",
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "recovery_hashi_tests.rs"]
mod tests;

#[path = "recovery_hashi_local.rs"]
mod local;
pub(crate) use local::LocalRequestPump;
