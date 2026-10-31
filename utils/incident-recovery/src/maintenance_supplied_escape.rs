//! Supplied, artifact-bound council authorizations for offline recovery rehearsals.
//! No signing key is read or generated. Cryptographic quorum is distinct from
//! council inventory approval, source provenance, consensus and release readiness.

use super::*;
use common::recovery::{RecoveryAction, RepairAuthorization};

#[path = "maintenance_public_repair.rs"]
pub(super) mod public_repair;

pub const PUBLIC_DYNAMIC_SCHEMA: &str = "sora-nexus-public-fixture-dynamic-repair-maintenance-v1";
const REPAIR_AUTH: &[u8] = b":sora-nexus:recovery:repair-authorization:v1";

pub const SCHEMA: &str = "sora-nexus-supplied-council-maintenance-replay-v1";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scenario {
    pub escapes: Vec<ExpectedEscape>,
    pub expected_terminal: Terminal,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExpectedEscape {
    /// Zero-based position in the exact manifest step sequence.
    pub step: usize,
    pub envelope_sha256: String,
    pub destination_spec: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Terminal {
    pub spec_version: u32,
    pub phase: String,
    pub cursor: u32,
    pub audit_cursor: u32,
    pub operation_counter: u64,
    pub active_policy_commitment: String,
    pub state_root: String,
    pub terminal_audit_blake2: Option<String>,
    pub ordinary_blocks: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Witness {
    pub step: usize,
    pub block_number: u32,
    pub parent_hash: String,
    pub recovery_anchor: String,
    pub envelope_sha256: String,
    pub authorization_sha256: String,
    pub signing_message_sha256: String,
    pub phase: String,
    pub cursor: u32,
    pub operation_counter: u64,
    pub stored_code_blake2: String,
    pub executing_code_blake2: String,
    pub destination_code_blake2: String,
    pub destination_spec: u32,
    pub revised_policy_commitment: String,
    pub signers: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema: &'static str,
    pub authorizations: Vec<Witness>,
    pub terminal: Terminal,
    pub signatures_verified_against_pinned_policy: bool,
    pub automatic_activation_observed: bool,
    pub public_test_council_generated: bool,
    pub council_inventory_approval_verified: bool,
    pub source_provenance_verified: bool,
    pub production_ready: bool,
    pub public_dynamic_fixture: bool,
}

fn hash32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 66
        || !value.starts_with("0x")
        || !value[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("expected canonical lowercase 0x-prefixed 32-byte hash".into());
    }
    hex_bytes(value)?
        .try_into()
        .map_err(|_| "invalid hash length".into())
}

fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

#[derive(Clone)]
pub struct Session {
    scenario: Scenario,
    original: RecoveryPolicy,
    active: RecoveryPolicy,
    witnesses: Vec<Witness>,
    pending_handover: Option<Vec<u8>>,
    public_dynamic: bool,
    public_scope: Option<public_repair::Scope>,
}

#[derive(Clone)]
pub struct Prepared {
    envelope: Vec<u8>,
    authorization: RepairAuthorization,
    work_policy: Option<RecoveryPolicy>,
    witness: Witness,
}

/// Constructed only from this session's accepted, independently imported quorum
/// transitions. It is not a deserializable caller assertion.
#[derive(Clone)]
pub struct WorkContext {
    original: RecoveryPolicy,
    active: RecoveryPolicy,
    spec: u32,
    engine_hash: [u8; 32],
    counter: u64,
    parent_authorization: Option<Vec<u8>>,
    accepted_escapes_sha256: String,
    parent_status: RecoveryStatus,
}
impl WorkContext {
    pub fn original(&self) -> &RecoveryPolicy {
        &self.original
    }
    pub fn active(&self) -> &RecoveryPolicy {
        &self.active
    }
    pub fn counter(&self) -> u64 {
        self.counter
    }
    pub fn spec(&self) -> u32 {
        self.spec
    }
    pub fn engine_hash(&self) -> [u8; 32] {
        self.engine_hash
    }
    pub fn manifest(&self, parent: &Header, snapshot: &str, transcript: &str) -> serde_json::Value {
        serde_json::json!({"schema":"sora-nexus-native-verified-work-context-v2",
            "parentHash":format!("{:#x}",parent.hash()),"parentStateRoot":format!("{:#x}",parent.state_root()),
            "parentSnapshotManifestSha256":snapshot,"priorTranscriptSha256":transcript,
            "acceptedEscapesSha256":self.accepted_escapes_sha256,"operationCounter":self.counter,
            "parentRepairAuthorization":self.parent_authorization.as_ref().map(|bytes|hx(bytes))})
    }
    pub fn parent_authorization_sha256(&self) -> Option<String> {
        self.parent_authorization.as_ref().map(|b| sha(b))
    }
}
impl Session {
    pub fn new(scenario: &Scenario, policy: &RecoveryPolicy, steps: &[Step]) -> Result<Self> {
        Self::with_mode(scenario, policy, steps, false)
    }
    pub fn public_dynamic(
        scenario: &Scenario,
        policy: &RecoveryPolicy,
        steps: &[Step],
    ) -> Result<Self> {
        let expected: Vec<_> = fixture::pairs().into_iter().map(|(a, _)| a).collect();
        let known_mapping = vec![recovery::AccountReplacement {
            old: sp_runtime::AccountId32::from(hash32(
                "0xde1563365ffd03e7958e9c69ac71fc3265db8214a962706ded8812f9a7265363",
            )?),
            new: sp_runtime::AccountId32::from(hash32(
                "0xed51c2f39620ba2de7cf1321f02d5c9a0706943aa7a796c3f8aeb94e89cba45e",
            )?),
        }];
        if policy.council != expected
            || policy.replacements != known_mapping
            || scenario.expected_terminal.spec_version != 138
            || scenario.escapes.len() < 2
        {
            return Err("public dynamic fixture requires the exact published council, public mapping, forward repair and final activation".into());
        }
        Self::with_mode(scenario, policy, steps, true)
    }
    fn with_mode(
        scenario: &Scenario,
        policy: &RecoveryPolicy,
        steps: &[Step],
        public_dynamic: bool,
    ) -> Result<Self> {
        policy
            .validate()
            .map_err(|e| format!("invalid supplied-council policy: {e:?}"))?;
        // Reject the published escape fixture council without constructing keys.
        for public in [
            "0x3d0bfb1e8c2639da9165a9b841eff0322a91e4726f5b3cfbbab33ae237248cb6",
            "0xa2384b2633793ee7124bd13b0dfaab6b5c820085401e80fed28b415381f99b7c",
            "0xcc75822e1a79475ca9b33bb1e9a6d331032e0b89733524d5881d4096c48649e3",
        ] {
            let bytes = hex_bytes(public)?;
            if !public_dynamic
                && policy
                    .council
                    .iter()
                    .any(|account| AsRef::<[u8]>::as_ref(account) == bytes)
            {
                return Err(
                    "published public-fixture council cannot enter supplied-council mode".into(),
                );
            }
        }
        let destinations: Vec<_> = scenario
            .escapes
            .iter()
            .map(|e| e.destination_spec)
            .collect();
        if !matches!(
            destinations.as_slice(),
            [135] | [135, 136] | [135, 137] | [135, 136, 137]
        ) {
            return Err(
                "supplied scenario requires one hold then strictly forward reserved repairs".into(),
            );
        }
        let mut previous = None;
        for escape in &scenario.escapes {
            if escape.step == 0
                || escape.step >= steps.len()
                || previous.is_some_and(|p| p >= escape.step)
                || steps[escape.step].recovery.as_ref().map(|a| &a.sha256)
                    != Some(&escape.envelope_sha256)
                || escape.envelope_sha256.len() != 64
                || !escape
                    .envelope_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("supplied escape must bind a unique ordered raw-envelope step".into());
            }
            previous = Some(escape.step);
        }
        let terminal = &scenario.expected_terminal;
        hash32(&terminal.active_policy_commitment)?;
        hash32(&terminal.state_root)?;
        if let Some(hash) = &terminal.terminal_audit_blake2 {
            hash32(hash)?;
        }
        let last = *destinations.last().ok_or("missing supplied escape")?;
        let valid_terminal = match terminal.spec_version {
            135 => last == 135 && terminal.phase == "Hold" && terminal.ordinary_blocks == 0,
            136 | 137 => {
                last == terminal.spec_version
                    && matches!(terminal.phase.as_str(), "Applying" | "Auditing" | "Ready")
                    && terminal.ordinary_blocks == 0
            }
            138 => last > 135 && terminal.phase == "Complete" && terminal.ordinary_blocks > 0,
            _ => false,
        };
        if !valid_terminal
            || terminal.operation_counter != scenario.escapes.len() as u64
            || steps.last().is_none_or(|s| {
                s.expected_spec != terminal.spec_version || s.expected_phase != terminal.phase
            })
            || terminal.ordinary_blocks > steps.len()
        {
            return Err("supplied scenario has an invalid explicit terminal".into());
        }
        Ok(Self {
            scenario: scenario.clone(),
            original: policy.clone(),
            active: policy.clone(),
            witnesses: Vec::new(),
            pending_handover: None,
            public_dynamic,
            public_scope: None,
        })
    }

    /// Check the raw parent before exact-Wasm initialization. The first repair
    /// parent must carry the exact authorization accepted on the previous block.
    pub fn before_initialize(
        &self,
        ext: &mut sp_io::TestExternalities,
        spec: u32,
        engine: &[u8],
    ) -> Result<Option<WorkContext>> {
        if value(ext, REPAIR_AUTH) != self.pending_handover {
            return Err("repair parent handover differs from accepted escape prefix".into());
        }
        if !matches!(spec, 136 | 137) {
            return Ok(None);
        }
        let last = self
            .witnesses
            .last()
            .ok_or("repaired engine has no accepted escape prefix")?;
        let current = status(ext)?;
        let counter = last
            .operation_counter
            .checked_add(1)
            .ok_or("counter overflow")?;
        if !matches!((spec, counter), (136, 2) | (137, 2..=3))
            || self.witnesses.len() as u64 != counter
            || last.destination_spec != spec
            || hash32(&last.destination_code_blake2)? != sp_io::hashing::blake2_256(engine)
            || replay::code(ext)? != engine
            || current.format_version != 1
            || current.operation_counter != counter
            || current.policy != self.original.commitment()
            || value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                != Some(self.original.commitment().as_slice())
            || value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                != Some(self.active.commitment().as_slice())
            || value(ext, recovery::RECOVERY_ANCHOR_STORAGE_KEY).as_deref()
                != Some(hash32(&last.recovery_anchor)?.as_slice())
            || hash32(&last.revised_policy_commitment)? != self.active.commitment()
            || value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1][..])
        {
            return Err("repaired work differs from accepted code/policy/counter lineage".into());
        }
        Ok(Some(WorkContext {
            original: self.original.clone(),
            active: self.active.clone(),
            spec,
            engine_hash: sp_io::hashing::blake2_256(engine),
            counter,
            parent_authorization: self.pending_handover.clone(),
            accepted_escapes_sha256: sha(
                &serde_json::to_vec(&self.witnesses).map_err(|e| e.to_string())?
            ),
            parent_status: current,
        }))
    }
    pub fn after_initialize(
        &self,
        ext: &mut sp_io::TestExternalities,
        context: &WorkContext,
    ) -> Result<()> {
        let current = status(ext)?;
        let anchor = hash32(
            &self
                .witnesses
                .last()
                .ok_or("missing accepted escape anchor")?
                .recovery_anchor,
        )?;
        if value(ext, recovery::RECOVERY_ANCHOR_STORAGE_KEY).as_deref() != Some(anchor.as_slice())
            || value(ext, REPAIR_AUTH).is_some()
            || current.format_version != 1
            || current.operation_counter != context.counter
            || current.policy != context.original.commitment()
            || value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                != Some(context.active.commitment().as_slice())
            || value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() != Some(&[1][..])
        {
            return Err(
                "repair initialize did not consume the accepted handover or preserve lineage"
                    .into(),
            );
        }
        Ok(())
    }
    /// Called only after this child was independently imported and checked.
    pub fn imported_initialized_block(&mut self, context: &WorkContext) -> Result<()> {
        if self.pending_handover != context.parent_authorization
            || self.active != context.active
            || self.witnesses.len() as u64 != context.counter
        {
            return Err("repaired initialized block is detached from accepted prefix".into());
        }
        self.pending_handover = None;
        Ok(())
    }

    /// Exposes the active work policy for a separately authenticated native delta
    /// recorder. The original policy remains the immutable lineage and council.
    pub fn work_policy(&self) -> &RecoveryPolicy {
        &self.active
    }

    #[allow(clippy::too_many_arguments)]
    pub fn inspect(
        &self,
        step: usize,
        parent: H256,
        payload: &[u8],
        action: &RecoveryAction,
        observed: &RecoveryStatus,
        expected_anchor: Option<[u8; 32]>,
        stored: &[u8],
        executing: &[u8],
        executing_spec: u32,
        destinations: &BTreeMap<u32, Vec<u8>>,
    ) -> Result<Option<Prepared>> {
        if &RecoveryInherent::decode_remark(payload)
            .map_err(|e| format!("invalid supplied envelope: {e:?}"))?
            .action
            != action
        {
            return Err("supplied decoded action differs from its envelope".into());
        }
        let expected = self.scenario.escapes.iter().find(|e| e.step == step);
        let RecoveryAction::Escape {
            authorization,
            signatures,
            work_policy,
        } = action
        else {
            if expected.is_some() {
                return Err("expected supplied escape step contains another action".into());
            }
            return Ok(None);
        };
        let expected = expected.ok_or("unlisted supplied escape")?;
        if self
            .scenario
            .escapes
            .get(self.witnesses.len())
            .map(|e| e.step)
            != Some(step)
            || sha(payload) != expected.envelope_sha256
            || expected.destination_spec != authorization.destination_spec
        {
            return Err("supplied escape ordering or envelope binding differs".into());
        }
        authorization
            .verify_quorum(
                &self.original,
                observed,
                expected_anchor.ok_or("escape requires an established first recovery child")?,
                signatures,
            )
            .map_err(|e| format!("supplied council quorum failed: {e:?}"))?;
        let destination = destinations
            .get(&authorization.destination_spec)
            .ok_or("unlisted supplied escape runtime")?;
        if observed.phase == RecoveryPhase::Complete
            || observed.operation_counter != self.witnesses.len() as u64
            || authorization.current_code_hash != sp_io::hashing::blake2_256(stored)
            || authorization.current_executor_hash != sp_io::hashing::blake2_256(executing)
            || authorization.destination_code_hash != sp_io::hashing::blake2_256(destination)
        {
            return Err(
                "supplied escape does not bind the observed code and replay context".into(),
            );
        }
        if executing_spec == 134 {
            if authorization.destination_spec != 135
                || work_policy.is_some()
                || authorization.revised_policy != [0; 32]
            {
                return Err(
                    "bootstrap escape must target the pinned hold without revised work".into(),
                );
            }
        } else {
            let revised = work_policy
                .as_ref()
                .ok_or("forward repair requires a signed revised work policy")?;
            revised
                .validate()
                .map_err(|e| format!("invalid supplied work policy: {e:?}"))?;
            if !matches!(executing_spec, 135 | 136)
                || !matches!(authorization.destination_spec, 136 | 137)
                || authorization.destination_spec <= executing_spec
                || authorization.current_executor_hash != authorization.current_code_hash
                || authorization.revised_policy != revised.commitment()
                || revised.replacements != self.original.replacements
                || revised.authorization_domain != self.original.authorization_domain
                || revised.council != self.original.council
            {
                return Err("supplied repair changes identity/ownership or is not a forward code transition".into());
            }
        }
        let witness = Witness {
            step,
            block_number: recovery::CHECKPOINT_NUMBER
                .checked_add(step as u32 + 1)
                .ok_or("block overflow")?,
            parent_hash: format!("{parent:#x}"),
            recovery_anchor: hx(&authorization.recovery_anchor),
            envelope_sha256: sha(payload),
            authorization_sha256: sha(&authorization.encode()),
            signing_message_sha256: sha(&authorization.signing_message()),
            phase: format!("{:?}", observed.phase),
            cursor: observed.next_chunk,
            operation_counter: observed.operation_counter,
            stored_code_blake2: hx(&authorization.current_code_hash),
            executing_code_blake2: hx(&authorization.current_executor_hash),
            destination_code_blake2: hx(&authorization.destination_code_hash),
            destination_spec: authorization.destination_spec,
            revised_policy_commitment: hx(&authorization.revised_policy),
            signers: signatures
                .iter()
                .map(|(account, _)| hx(AsRef::<[u8]>::as_ref(account)))
                .collect(),
        };
        Ok(Some(Prepared {
            envelope: payload.to_vec(),
            authorization: authorization.as_ref().clone(),
            work_policy: work_policy.clone(),
            witness,
        }))
    }

    pub fn accept(
        &mut self,
        prepared: Prepared,
        ext: &mut sp_io::TestExternalities,
        spec: u32,
        output: &Path,
    ) -> Result<Vec<u8>> {
        let current = status(ext)?;
        let active = prepared.work_policy.as_ref().unwrap_or(&self.active);
        let expected_phase = if spec == 135 {
            RecoveryPhase::Hold
        } else {
            RecoveryPhase::Applying
        };
        let expected_cursor = if spec == 135 {
            prepared.authorization.cursor
        } else {
            0
        };
        if current.phase != expected_phase
            || current.next_chunk != expected_cursor
            || current.format_version != 1
            || current.policy != self.original.commitment()
            || value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                != Some(self.original.commitment().as_slice())
            || current.operation_counter
                != prepared
                    .authorization
                    .operation_counter
                    .checked_add(1)
                    .ok_or("counter overflow")?
            || current.failure.is_some()
            || spec != prepared.authorization.destination_spec
            || value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                != Some(active.commitment().as_slice())
            || sp_io::hashing::blake2_256(&replay::code(ext)?)
                != prepared.authorization.destination_code_hash
        {
            return Err(
                "supplied escape did not atomically install its expected code and state".into(),
            );
        }
        if matches!(spec, 136 | 137)
            && (value(ext, recovery::AUDIT_CURSOR_STORAGE_KEY) != Some(0u32.encode())
                || value(ext, recovery::TERMINAL_AUDIT_STORAGE_KEY).is_some())
        {
            return Err("supplied repair did not reset its audit cursor and terminal audit".into());
        }
        let expected_handover = matches!(spec, 136 | 137).then(|| prepared.authorization.encode());
        if value(ext, REPAIR_AUTH) != expected_handover {
            return Err("accepted repair did not install its exact handover authorization".into());
        }
        self.pending_handover = expected_handover;
        let prefix = output.join(format!("{}.supplied", prepared.witness.block_number));
        for (suffix, bytes) in [
            ("escape.scale", prepared.envelope.clone()),
            ("authorization.scale", prepared.authorization.encode()),
            (
                "signing-message.scale",
                prepared.authorization.signing_message(),
            ),
            (
                "authorization.json",
                serde_json::to_vec_pretty(&prepared.witness).map_err(|e| e.to_string())?,
            ),
        ] {
            fs::write(format!("{}.{suffix}", prefix.display()), bytes)
                .map_err(|e| e.to_string())?;
        }
        self.active = active.clone();
        self.witnesses.push(prepared.witness);
        Ok(prepared.envelope)
    }

    pub fn finish(
        self,
        ext: &mut sp_io::TestExternalities,
        spec: u32,
        ordinary: usize,
        completed: bool,
    ) -> Result<Report> {
        if self
            .public_scope
            .as_ref()
            .is_some_and(|scope| scope.is_prefix())
        {
            return Err(
                "public signing prefix cannot produce an activation terminal report".into(),
            );
        }
        let terminal = &self.scenario.expected_terminal;
        let current = status(ext)?;
        let audit_bytes =
            value(ext, recovery::AUDIT_CURSOR_STORAGE_KEY).ok_or("missing audit cursor")?;
        let audit_cursor =
            u32::decode_all(&mut audit_bytes.as_slice()).map_err(|e| e.to_string())?;
        let audit = value(ext, recovery::TERMINAL_AUDIT_STORAGE_KEY);
        let audit_hash = audit
            .as_ref()
            .map(|bytes| sp_io::hashing::blake2_256(bytes));
        if self.witnesses.len() != self.scenario.escapes.len()
            || current.format_version != 1
            || current.policy != self.original.commitment()
            || value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                != Some(self.original.commitment().as_slice())
            || spec != terminal.spec_version
            || format!("{:?}", current.phase) != terminal.phase
            || current.next_chunk != terminal.cursor
            || audit_cursor != terminal.audit_cursor
            || current.operation_counter != terminal.operation_counter
            || current.failure.is_some()
            || hash32(&terminal.active_policy_commitment)? != self.work_policy().commitment()
            || value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                != Some(self.active.commitment().as_slice())
            || hash32(&terminal.state_root)? != replay::root(ext).0
            || terminal
                .terminal_audit_blake2
                .as_ref()
                .map(|s| hash32(s))
                .transpose()?
                != audit_hash
            || ordinary != terminal.ordinary_blocks
            || completed != (terminal.spec_version == 138)
        {
            return Err(
                "supplied-council rehearsal differs from its explicit expected terminal".into(),
            );
        }
        if matches!(
            current.phase,
            RecoveryPhase::Ready | RecoveryPhase::Complete
        ) && (audit
            != Some(
                (
                    self.active.commitment(),
                    self.active.terminal_audit_commitment,
                    current.next_chunk,
                )
                    .encode(),
            )
            || current.next_chunk as usize != self.active.chunk_hashes.len()
            || audit_cursor as usize != self.active.audit_chunk_hashes.len())
        {
            return Err(
                "supplied-council terminal audit does not bind the active complete work policy"
                    .into(),
            );
        }
        Ok(Report {
            schema: if self.public_dynamic {
                "sora-nexus-public-dynamic-repair-council-witness-v1"
            } else {
                "sora-nexus-supplied-council-replay-witness-v1"
            },
            authorizations: self.witnesses,
            terminal: self.scenario.expected_terminal,
            signatures_verified_against_pinned_policy: true,
            automatic_activation_observed: completed,
            public_test_council_generated: false,
            council_inventory_approval_verified: false,
            source_provenance_verified: false,
            production_ready: false,
            public_dynamic_fixture: self.public_dynamic,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::{ed25519, Pair};
    use sp_runtime::{AccountId32, MultiSignature};

    fn pairs() -> Vec<(AccountId32, ed25519::Pair)> {
        let mut pairs: Vec<_> = (80..83)
            .map(|i| {
                let pair = ed25519::Pair::from_seed(&[i; 32]);
                (AccountId32::from(pair.public().0), pair)
            })
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        pairs
    }

    fn policy() -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: pairs().into_iter().map(|(a, _)| a).collect(),
            chunk_hashes: vec![[1; 32]],
            audit_chunk_hashes: vec![[2; 32]],
            evidence_commitment: [3; 32],
            terminal_audit_commitment: [4; 32],
        }
    }

    fn observed(policy: &RecoveryPolicy) -> RecoveryStatus {
        RecoveryStatus {
            format_version: 1,
            policy: policy.commitment(),
            phase: RecoveryPhase::Applying,
            next_chunk: 0,
            operation_counter: 0,
            failure: None,
        }
    }

    fn sign(auth: RepairAuthorization, work: Option<RecoveryPolicy>) -> RecoveryInherent {
        let signatures = pairs()
            .into_iter()
            .take(2)
            .map(|(a, p)| (a, MultiSignature::Ed25519(p.sign(&auth.signing_message()))))
            .collect();
        RecoveryInherent {
            format_version: 1,
            action: RecoveryAction::Escape {
                authorization: Box::new(auth),
                signatures,
                work_policy: work,
            },
        }
    }

    fn hold(policy: &RecoveryPolicy) -> RecoveryInherent {
        sign(
            RepairAuthorization {
                recovery_anchor: [0x42; 32],
                authorization_domain: policy.authorization_domain.clone(),
                policy: policy.commitment(),
                phase: RecoveryPhase::Applying,
                cursor: 0,
                operation_counter: 0,
                current_code_hash: sp_io::hashing::blake2_256(b"deployed"),
                current_executor_hash: sp_io::hashing::blake2_256(b"bootstrap"),
                destination_code_hash: sp_io::hashing::blake2_256(b"hold"),
                destination_spec: 135,
                revised_policy: [0; 32],
            },
            None,
        )
    }

    fn step(envelope: &RecoveryInherent, spec: u32, phase: &str) -> Step {
        Step {
            slot: 1,
            timestamp: 1,
            authority_index: 0,
            recovery: Some(replay::Artifact {
                file: "public-envelope.scale".into(),
                sha256: sha(&envelope.encode_remark()),
            }),
            fixture_escape: None,
            fixture_hashi: false,
            reject_previous_escapes: false,
            rejected_recovery: vec![],
            expected_spec: spec,
            expected_phase: phase.into(),
        }
    }

    fn tick() -> Step {
        step(
            &RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::Tick,
            },
            134,
            "Applying",
        )
    }

    fn scenario(policy: &RecoveryPolicy, envelope: &RecoveryInherent) -> Scenario {
        Scenario {
            escapes: vec![ExpectedEscape {
                step: 1,
                envelope_sha256: sha(&envelope.encode_remark()),
                destination_spec: 135,
            }],
            expected_terminal: Terminal {
                spec_version: 135,
                phase: "Hold".into(),
                cursor: 0,
                audit_cursor: 0,
                operation_counter: 1,
                active_policy_commitment: hx(&policy.commitment()),
                state_root: hx(&[0; 32]),
                terminal_audit_blake2: None,
                ordinary_blocks: 0,
            },
        }
    }

    fn inspect(
        session: &Session,
        envelope: &RecoveryInherent,
        observed: &RecoveryStatus,
    ) -> Result<Option<Prepared>> {
        session.inspect(
            1,
            H256::from(recovery::CHECKPOINT_HASH),
            &envelope.encode_remark(),
            &envelope.action,
            observed,
            Some([0x42; 32]),
            b"deployed",
            b"bootstrap",
            134,
            &BTreeMap::from([(135, b"hold".to_vec())]),
        )
    }

    #[test]
    fn supplied_escape_uses_external_quorum_and_both_observed_code_identities() {
        let p = policy();
        let envelope = hold(&p);
        let sc = scenario(&p, &envelope);
        let session = Session::new(&sc, &p, &[tick(), step(&envelope, 135, "Hold")]).unwrap();
        let prepared = inspect(&session, &envelope, &observed(&p))
            .unwrap()
            .unwrap();
        assert_eq!(prepared.witness.signers.len(), 2);
        assert_ne!(
            prepared.witness.stored_code_blake2,
            prepared.witness.executing_code_blake2
        );
        let mut replayed = observed(&p);
        replayed.operation_counter = 1;
        assert_eq!(
            inspect(&session, &envelope, &replayed).err().as_deref(),
            Some("supplied council quorum failed: Replay")
        );
        assert!(inspect(&session, &envelope, &observed(&p))
            .unwrap()
            .is_some());
        assert_eq!(
            session
                .inspect(
                    1,
                    H256::zero(),
                    &envelope.encode_remark(),
                    &envelope.action,
                    &observed(&p),
                    Some([0x42; 32]),
                    b"deployed",
                    b"different substitute",
                    134,
                    &BTreeMap::from([(135, b"hold".to_vec())])
                )
                .err()
                .as_deref(),
            Some("supplied escape does not bind the observed code and replay context")
        );
    }

    #[test]
    fn supplied_escape_rejects_insufficient_duplicate_and_tampered_signatures() {
        let p = policy();
        for kind in 0..3 {
            let mut envelope = hold(&p);
            let control = Session::new(
                &scenario(&p, &envelope),
                &p,
                &[tick(), step(&envelope, 135, "Hold")],
            )
            .unwrap();
            assert!(inspect(&control, &envelope, &observed(&p))
                .unwrap()
                .is_some());
            let RecoveryAction::Escape {
                authorization,
                signatures,
                ..
            } = &mut envelope.action
            else {
                unreachable!()
            };
            match kind {
                0 => signatures.truncate(1),
                1 => signatures[1] = signatures[0].clone(),
                _ => authorization.current_executor_hash = [7; 32],
            }
            let session = Session::new(
                &scenario(&p, &envelope),
                &p,
                &[tick(), step(&envelope, 135, "Hold")],
            )
            .unwrap();
            assert_eq!(
                inspect(&session, &envelope, &observed(&p)).err().as_deref(),
                Some("supplied council quorum failed: BadQuorum")
            );
        }
    }

    #[test]
    fn supplied_scenario_rejects_public_fixture_keys_and_missing_forward_repair() {
        let mut p = policy();
        let envelope = hold(&p);
        let original = scenario(&p, &envelope);
        assert!(Session::new(&original, &p, &[tick(), step(&envelope, 135, "Hold")]).is_ok());
        let mut sc = original.clone();
        sc.expected_terminal.spec_version = 138;
        sc.expected_terminal.phase = "Complete".into();
        sc.expected_terminal.ordinary_blocks = 1;
        assert_eq!(
            Session::new(&sc, &p, &[tick(), step(&envelope, 138, "Complete")])
                .err()
                .as_deref(),
            Some("supplied scenario has an invalid explicit terminal")
        );
        assert!(Session::new(&original, &p, &[tick(), step(&envelope, 135, "Hold")]).is_ok());
        p.council = fixture::pairs().into_iter().map(|(a, _)| a).collect();
        let envelope = hold(&p);
        assert_eq!(
            Session::new(
                &scenario(&p, &envelope),
                &p,
                &[tick(), step(&envelope, 135, "Hold")]
            )
            .err()
            .as_deref(),
            Some("published public-fixture council cannot enter supplied-council mode")
        );
    }

    #[test]
    fn supplied_scenario_requires_bound_unique_steps_and_canonical_terminal_hashes() {
        let p = policy();
        let envelope = hold(&p);
        let base = scenario(&p, &envelope);
        assert!(Session::new(&base, &p, &[tick(), step(&envelope, 135, "Hold")]).is_ok());
        let mut sc = base.clone();
        sc.escapes[0].envelope_sha256 = "0".repeat(64);
        assert_eq!(
            Session::new(&sc, &p, &[tick(), step(&envelope, 135, "Hold")])
                .err()
                .as_deref(),
            Some("supplied escape must bind a unique ordered raw-envelope step")
        );
        assert!(Session::new(&base, &p, &[tick(), step(&envelope, 135, "Hold")]).is_ok());
        let mut sc = base.clone();
        sc.expected_terminal.state_root = "0xAA".into();
        assert_eq!(
            Session::new(&sc, &p, &[tick(), step(&envelope, 135, "Hold")])
                .err()
                .as_deref(),
            Some("expected canonical lowercase 0x-prefixed 32-byte hash")
        );
        assert!(Session::new(&base, &p, &[tick(), step(&envelope, 135, "Hold")]).is_ok());
        let mut sc = base;
        sc.escapes.push(ExpectedEscape {
            step: 1,
            envelope_sha256: sha(&envelope.encode_remark()),
            destination_spec: 136,
        });
        assert_eq!(
            Session::new(&sc, &p, &[tick(), step(&envelope, 135, "Hold")])
                .err()
                .as_deref(),
            Some("supplied escape must bind a unique ordered raw-envelope step")
        );
    }

    #[test]
    fn supplied_forward_repair_keeps_original_quorum_and_rejects_ownership_changes() {
        let original = policy();
        let hold = hold(&original);
        let mut revised = original.clone();
        revised.chunk_hashes = vec![[6; 32]];
        for change_ownership in [false, true] {
            let mut work = revised.clone();
            if change_ownership {
                work.replacements.push(recovery::AccountReplacement {
                    old: AccountId32::from([9; 32]),
                    new: AccountId32::from([10; 32]),
                });
            }
            let repair = sign(
                RepairAuthorization {
                    recovery_anchor: [0x42; 32],
                    authorization_domain: original.authorization_domain.clone(),
                    policy: original.commitment(),
                    phase: RecoveryPhase::Hold,
                    cursor: 0,
                    operation_counter: 1,
                    current_code_hash: sp_io::hashing::blake2_256(b"hold"),
                    current_executor_hash: sp_io::hashing::blake2_256(b"hold"),
                    destination_code_hash: sp_io::hashing::blake2_256(b"repair"),
                    destination_spec: 136,
                    revised_policy: work.commitment(),
                },
                Some(work.clone()),
            );
            let mut sc = scenario(&original, &hold);
            sc.escapes.push(ExpectedEscape {
                step: 2,
                envelope_sha256: sha(&repair.encode_remark()),
                destination_spec: 136,
            });
            sc.expected_terminal.spec_version = 136;
            sc.expected_terminal.phase = "Applying".into();
            sc.expected_terminal.operation_counter = 2;
            sc.expected_terminal.active_policy_commitment = hx(&work.commitment());
            let mut session = Session::new(
                &sc,
                &original,
                &[
                    tick(),
                    step(&hold, 135, "Hold"),
                    step(&repair, 136, "Applying"),
                ],
            )
            .unwrap();
            session.witnesses.push(
                inspect(&session, &hold, &observed(&original))
                    .unwrap()
                    .unwrap()
                    .witness,
            );
            let state = RecoveryStatus {
                phase: RecoveryPhase::Hold,
                operation_counter: 1,
                ..observed(&original)
            };
            let result = session.inspect(
                2,
                H256::zero(),
                &repair.encode_remark(),
                &repair.action,
                &state,
                Some([0x42; 32]),
                b"hold",
                b"hold",
                135,
                &BTreeMap::from([(136, b"repair".to_vec())]),
            );
            if change_ownership {
                assert_eq!(result.err().as_deref(), Some("supplied repair changes identity/ownership or is not a forward code transition"));
            } else {
                assert!(result.unwrap().is_some());
            }
            assert_eq!(session.work_policy().commitment(), original.commitment());
        }
    }

    struct TestDirectory(std::path::PathBuf);
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn test_directory(name: &str) -> TestDirectory {
        let path = std::env::temp_dir().join(format!(
            "sora-nexus-supplied-council-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        TestDirectory(path)
    }

    #[test]
    fn supplied_accept_requires_actual_atomic_transition_before_recording() {
        let p = policy();
        let envelope = hold(&p);
        let sc = scenario(&p, &envelope);
        let mut session = Session::new(&sc, &p, &[tick(), step(&envelope, 135, "Hold")]).unwrap();
        let prepared = inspect(&session, &envelope, &observed(&p))
            .unwrap()
            .unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            let state = RecoveryStatus {
                phase: RecoveryPhase::Hold,
                operation_counter: 1,
                ..observed(&p)
            };
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &state.encode());
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &p.commitment());
            sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &p.commitment());
            sp_io::storage::set(b":code", b"hold");
        });
        let output = test_directory("hold");
        let mut control = session.clone();
        assert_eq!(
            control
                .accept(prepared.clone(), &mut ext, 135, &output.0)
                .unwrap(),
            envelope.encode_remark()
        );
        assert_eq!(control.witnesses.len(), 1);
        // Existing writable output rules out an unrelated missing-path rejection.
        ext.execute_with(|| {
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &observed(&p).encode())
        });
        assert_eq!(
            session
                .accept(prepared, &mut ext, 135, &output.0)
                .err()
                .as_deref(),
            Some("supplied escape did not atomically install its expected code and state")
        );
        assert!(session.witnesses.is_empty());
        assert_eq!(
            fs::read(output.0.join("28191819.supplied.escape.scale")).unwrap(),
            envelope.encode_remark()
        );
    }

    #[test]
    fn supplied_repair_accept_requires_reset_audit_state() {
        let original = policy();
        let hold = hold(&original);
        let mut revised = original.clone();
        revised.chunk_hashes = vec![[6; 32]];
        let repair = sign(
            RepairAuthorization {
                recovery_anchor: [0x42; 32],
                authorization_domain: original.authorization_domain.clone(),
                policy: original.commitment(),
                phase: RecoveryPhase::Hold,
                cursor: 0,
                operation_counter: 1,
                current_code_hash: sp_io::hashing::blake2_256(b"hold"),
                current_executor_hash: sp_io::hashing::blake2_256(b"hold"),
                destination_code_hash: sp_io::hashing::blake2_256(b"repair"),
                destination_spec: 136,
                revised_policy: revised.commitment(),
            },
            Some(revised.clone()),
        );
        let mut sc = scenario(&original, &hold);
        sc.escapes.push(ExpectedEscape {
            step: 2,
            envelope_sha256: sha(&repair.encode_remark()),
            destination_spec: 136,
        });
        sc.expected_terminal.spec_version = 136;
        sc.expected_terminal.phase = "Applying".into();
        sc.expected_terminal.operation_counter = 2;
        sc.expected_terminal.active_policy_commitment = hx(&revised.commitment());
        let mut session = Session::new(
            &sc,
            &original,
            &[
                tick(),
                step(&hold, 135, "Hold"),
                step(&repair, 136, "Applying"),
            ],
        )
        .unwrap();
        let first = inspect(&session, &hold, &observed(&original))
            .unwrap()
            .unwrap();
        session.witnesses.push(first.witness);
        let state = RecoveryStatus {
            phase: RecoveryPhase::Hold,
            operation_counter: 1,
            ..observed(&original)
        };
        let prepared = session
            .inspect(
                2,
                H256::zero(),
                &repair.encode_remark(),
                &repair.action,
                &state,
                Some([0x42; 32]),
                b"hold",
                b"hold",
                135,
                &BTreeMap::from([(136, b"repair".to_vec())]),
            )
            .unwrap()
            .unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            let current = RecoveryStatus {
                operation_counter: 2,
                ..observed(&original)
            };
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &current.encode());
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &original.commitment());
            sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &revised.commitment());
            sp_io::storage::set(b":code", b"repair");
            sp_io::storage::set(REPAIR_AUTH, &prepared.authorization.encode());
            sp_io::storage::set(recovery::RECOVERY_ANCHOR_STORAGE_KEY, &[0x42; 32]);
        });
        for kind in 0..3 {
            ext.execute_with(|| {
                sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &0u32.encode());
                sp_io::storage::clear(recovery::TERMINAL_AUDIT_STORAGE_KEY);
            });
            let output = test_directory(&format!("repair-{kind}"));
            let mut control = session.clone();
            assert_eq!(
                control
                    .accept(prepared.clone(), &mut ext, 136, &output.0)
                    .unwrap(),
                repair.encode_remark()
            );
            assert_eq!(control.witnesses.len(), 2);
            assert_eq!(control.work_policy().commitment(), revised.commitment());
            ext.execute_with(|| match kind {
                0 => sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &1u32.encode()),
                1 => sp_io::storage::set(
                    recovery::TERMINAL_AUDIT_STORAGE_KEY,
                    &(
                        original.commitment(),
                        original.terminal_audit_commitment,
                        1u32,
                    )
                        .encode(),
                ),
                _ => sp_io::storage::clear(recovery::AUDIT_CURSOR_STORAGE_KEY),
            });
            assert_eq!(
                session
                    .accept(prepared.clone(), &mut ext, 136, &output.0)
                    .err()
                    .as_deref(),
                Some("supplied repair did not reset its audit cursor and terminal audit")
            );
            assert_eq!(session.witnesses.len(), 1);
            assert_eq!(session.work_policy().commitment(), original.commitment());
        }
    }

    fn completed_session() -> (Session, sp_io::TestExternalities) {
        // Pure storage/unit fixture: no Wasm execution or real council approval.
        let p = policy();
        let envelope = hold(&p);
        let mut sc = scenario(&p, &envelope);
        sc.escapes.push(ExpectedEscape {
            step: 2,
            envelope_sha256: "0".repeat(64),
            destination_spec: 136,
        });
        sc.expected_terminal = Terminal {
            spec_version: 138,
            phase: "Complete".into(),
            cursor: 1,
            audit_cursor: 1,
            operation_counter: 2,
            active_policy_commitment: hx(&p.commitment()),
            state_root: hx(&[0; 32]),
            terminal_audit_blake2: None,
            ordinary_blocks: 1,
        };
        let mut s = Session {
            scenario: sc,
            original: p.clone(),
            active: p.clone(),
            witnesses: vec![],
            pending_handover: None,
            public_dynamic: false,
            public_scope: None,
        };
        let prepared = inspect(&s, &envelope, &observed(&p)).unwrap().unwrap();
        s.witnesses = vec![prepared.witness.clone(), prepared.witness];
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            let mut current = observed(&p);
            current.phase = RecoveryPhase::Complete;
            current.next_chunk = 1;
            current.operation_counter = 2;
            sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &current.encode());
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &p.commitment());
            sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &p.commitment());
            sp_io::storage::set(recovery::AUDIT_CURSOR_STORAGE_KEY, &1u32.encode());
            sp_io::storage::set(
                recovery::TERMINAL_AUDIT_STORAGE_KEY,
                &(p.commitment(), p.terminal_audit_commitment, 1u32).encode(),
            );
        });
        s.scenario.expected_terminal.state_root = format!("{:#x}", replay::root(&mut ext));
        s.scenario.expected_terminal.terminal_audit_blake2 = Some(hx(&sp_io::hashing::blake2_256(
            &value(&mut ext, recovery::TERMINAL_AUDIT_STORAGE_KEY).unwrap(),
        )));
        (s, ext)
    }

    #[test]
    fn supplied_explicit_terminal_checks_audit_counter_root_and_activation() {
        let (s, mut ext) = completed_session();
        s.clone().finish(&mut ext, 138, 1, true).unwrap();
        assert_eq!(
            s.clone().finish(&mut ext, 138, 0, false).err().as_deref(),
            Some("supplied-council rehearsal differs from its explicit expected terminal")
        );
        s.clone().finish(&mut ext, 138, 1, true).unwrap();
        let mut wrong = s.clone();
        wrong.scenario.expected_terminal.state_root = hx(&[9; 32]);
        assert_eq!(
            wrong.finish(&mut ext, 138, 1, true).err().as_deref(),
            Some("supplied-council rehearsal differs from its explicit expected terminal")
        );
        s.clone().finish(&mut ext, 138, 1, true).unwrap();
        let mut wrong = s.clone();
        wrong.scenario.expected_terminal.operation_counter += 1;
        assert_eq!(
            wrong.finish(&mut ext, 138, 1, true).err().as_deref(),
            Some("supplied-council rehearsal differs from its explicit expected terminal")
        );
        let report = s.finish(&mut ext, 138, 1, true).unwrap();
        assert!(
            !report.production_ready
                && !report.council_inventory_approval_verified
                && !report.source_provenance_verified
        );
        assert!(report.automatic_activation_observed);
    }

    #[test]
    fn supplied_terminal_rejects_audit_for_another_active_policy_even_with_expected_root() {
        let (mut s, mut ext) = completed_session();
        s.clone().finish(&mut ext, 138, 1, true).unwrap();
        let bad = ([8u8; 32], s.active.terminal_audit_commitment, 1u32).encode();
        ext.execute_with(|| sp_io::storage::set(recovery::TERMINAL_AUDIT_STORAGE_KEY, &bad));
        s.scenario.expected_terminal.state_root = format!("{:#x}", replay::root(&mut ext));
        s.scenario.expected_terminal.terminal_audit_blake2 =
            Some(hx(&sp_io::hashing::blake2_256(&bad)));
        assert!(s
            .finish(&mut ext, 138, 1, true)
            .unwrap_err()
            .contains("terminal audit"));
    }
    fn repaired_session(spec: u32, counter: u64) -> (Session, sp_io::TestExternalities) {
        let original = policy();
        let mut active = original.clone();
        active.chunk_hashes = vec![[6; 32]];
        let hold = hold(&original);
        let prepared = inspect(
            &Session::new(
                &scenario(&original, &hold),
                &original,
                &[tick(), step(&hold, 135, "Hold")],
            )
            .unwrap(),
            &hold,
            &observed(&original),
        )
        .unwrap()
        .unwrap();
        let auth = RepairAuthorization {
            recovery_anchor: [0x42; 32],
            authorization_domain: original.authorization_domain.clone(),
            policy: original.commitment(),
            phase: RecoveryPhase::Hold,
            cursor: 0,
            operation_counter: counter - 1,
            current_code_hash: sp_io::hashing::blake2_256(b"hold"),
            current_executor_hash: sp_io::hashing::blake2_256(b"hold"),
            destination_code_hash: sp_io::hashing::blake2_256(b"repair"),
            destination_spec: spec,
            revised_policy: active.commitment(),
        };
        let mut witnesses = vec![prepared.witness.clone(); counter as usize];
        let last = witnesses.last_mut().unwrap();
        last.destination_spec = spec;
        last.operation_counter = counter - 1;
        last.destination_code_blake2 = hx(&auth.destination_code_hash);
        last.revised_policy_commitment = hx(&active.commitment());
        let session = Session {
            scenario: scenario(&original, &hold),
            original: original.clone(),
            active: active.clone(),
            witnesses,
            pending_handover: Some(auth.encode()),
            public_dynamic: false,
            public_scope: None,
        };
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(b":code", b"repair");
            sp_io::storage::set(REPAIR_AUTH, &auth.encode());
            sp_io::storage::set(recovery::RECOVERY_ANCHOR_STORAGE_KEY, &[0x42; 32]);
            sp_io::storage::set(
                recovery::STATUS_STORAGE_KEY,
                &RecoveryStatus {
                    operation_counter: counter,
                    ..observed(&original)
                }
                .encode(),
            );
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &original.commitment());
            sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &active.commitment());
            sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        });
        (session, ext)
    }
    #[test]
    fn verified_work_context_requires_exact_pending_marker_then_consumption() {
        for (spec, counter) in [(136, 2), (137, 2), (137, 3)] {
            let (mut session, mut ext) = repaired_session(spec, counter);
            let context = session
                .before_initialize(&mut ext, spec, b"repair")
                .unwrap()
                .unwrap();
            assert_eq!(context.counter(), counter);
            assert!(session
                .after_initialize(&mut ext, &context)
                .unwrap_err()
                .contains("consume"));
            ext.execute_with(|| sp_io::storage::clear(REPAIR_AUTH));
            assert!(session
                .before_initialize(&mut ext, spec, b"repair")
                .err()
                .unwrap()
                .contains("handover differs"));
            session.after_initialize(&mut ext, &context).unwrap();
            session.imported_initialized_block(&context).unwrap();
            assert!(session
                .before_initialize(&mut ext, spec, b"repair")
                .unwrap()
                .unwrap()
                .parent_authorization
                .is_none());
            ext.execute_with(|| sp_io::storage::set(REPAIR_AUTH, b"stale"));
            assert!(session
                .before_initialize(&mut ext, spec, b"repair")
                .err()
                .unwrap()
                .contains("handover differs"));
        }
    }
    #[test]
    fn verified_work_context_rejects_counter_engine_active_policy_and_false_halt() {
        for mutation in 0..5 {
            let (mut session, mut ext) = repaired_session(136, 2);
            assert!(session
                .before_initialize(&mut ext, 136, b"repair")
                .unwrap()
                .is_some());
            ext.execute_with(|| match mutation {
                0 => {
                    let mut state = recovery::storage::status().unwrap().unwrap();
                    state.operation_counter = 3;
                    sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &state.encode());
                }
                1 => sp_io::storage::set(
                    recovery::ACTIVE_POLICY_STORAGE_KEY,
                    &session.original.commitment(),
                ),
                2 => sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[0]),
                3 => sp_io::storage::set(b":code", b"wrong"),
                _ => {}
            });
            if mutation == 4 {
                session.witnesses.clear();
            }
            assert!(session.before_initialize(&mut ext, 136, b"repair").is_err());
        }
    }
    #[test]
    fn public_dynamic_mode_cannot_reclassify_supplied_or_different_mapping() {
        let mut p = policy();
        p.council = fixture::pairs().into_iter().map(|(a, _)| a).collect();
        p.replacements = vec![recovery::AccountReplacement {
            old: AccountId32::from(
                hash32("0xde1563365ffd03e7958e9c69ac71fc3265db8214a962706ded8812f9a7265363")
                    .unwrap(),
            ),
            new: AccountId32::from(
                hash32("0xed51c2f39620ba2de7cf1321f02d5c9a0706943aa7a796c3f8aeb94e89cba45e")
                    .unwrap(),
            ),
        }];
        let envelope = hold(&p);
        let mut sc = scenario(&p, &envelope);
        sc.escapes.push(ExpectedEscape {
            step: 2,
            envelope_sha256: sha(b"repair-placeholder"),
            destination_spec: 136,
        });
        sc.expected_terminal.spec_version = 138;
        sc.expected_terminal.phase = "Complete".into();
        sc.expected_terminal.operation_counter = 2;
        sc.expected_terminal.ordinary_blocks = 1;
        let mut repair_step = step(&envelope, 138, "Complete");
        repair_step.recovery.as_mut().unwrap().sha256 = sha(b"repair-placeholder");
        let steps = vec![tick(), step(&envelope, 135, "Hold"), repair_step];
        Session::public_dynamic(&sc, &p, &steps).unwrap();
        assert!(Session::new(&sc, &p, &steps)
            .err()
            .unwrap()
            .contains("public-fixture council"));
        p.replacements[0].new = AccountId32::from([123; 32]);
        assert!(Session::public_dynamic(&sc, &p, &steps)
            .err()
            .unwrap()
            .contains("public mapping"));
    }
}
