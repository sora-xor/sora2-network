//! Bounded public-only signing prefixes and deliberately rejected repair handovers.
//! A prefix observes state for a later signature; it never certifies activation.

use super::*;

pub const PREFIX_SCHEMA: &str = "sora-nexus-public-fixture-repair-signing-prefix-v1";
pub const PREFIX_REPORT_SCHEMA: &str = "sora-nexus-public-fixture-repair-signing-prefix-witness-v1";
pub const NEGATIVE_SCHEMA: &str = "sora-nexus-public-fixture-rejected-handover-maintenance-v1";
pub const NEGATIVE_REPORT_SCHEMA: &str =
    "sora-nexus-public-fixture-rejected-handover-maintenance-witness-v1";
const MAX_STEPS: usize = 4096;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Hold135,
    Failed136,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrefixPlan {
    pub target: Target,
    pub escapes: Vec<ExpectedEscape>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RejectedHandover {
    pub spec_version: u32,
    pub executing_code_blake2: String,
}

#[derive(Clone)]
pub(super) struct Scope {
    target: Option<Target>,
    rejected: Option<RejectedHandover>,
}
impl Scope {
    pub(super) fn is_prefix(&self) -> bool {
        self.target.is_some()
    }
}

fn need(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn public_identity(policy: &RecoveryPolicy) -> Result<()> {
    policy
        .validate()
        .map_err(|e| format!("public repair policy: {e:?}"))?;
    let council: Vec<_> = fixture::pairs().into_iter().map(|(a, _)| a).collect();
    let mapping = vec![recovery::AccountReplacement {
        old: sp_core::crypto::AccountId32::from(hash32(
            "0xde1563365ffd03e7958e9c69ac71fc3265db8214a962706ded8812f9a7265363",
        )?),
        new: sp_core::crypto::AccountId32::from(hash32(
            "0xed51c2f39620ba2de7cf1321f02d5c9a0706943aa7a796c3f8aeb94e89cba45e",
        )?),
    }];
    need(
        policy.authorization_domain == recovery::RecoveryDomain::public_fixture()
            && policy.council == council
            && policy.replacements == mapping
            && policy.chunk_hashes.len() == 3
            && policy.audit_chunk_hashes.len() == 2,
        "repair prefix requires the fixed public domain/council/mapping and finite fixture work",
    )
}

fn rejected_identity(rejected: &RejectedHandover) -> Result<()> {
    need(
        rejected.spec_version == 136 && hash32(&rejected.executing_code_blake2)? != [0; 32],
        "only an explicitly pinned rejected136 executor is supported",
    )
}

fn escapes(escapes: &[ExpectedEscape], steps: &[Step], destinations: &[u32]) -> Result<()> {
    need(
        (6..=MAX_STEPS).contains(&steps.len())
            && escapes
                .iter()
                .map(|e| e.destination_spec)
                .collect::<Vec<_>>()
                == destinations,
        "public repair prefix has another escape sequence or step bound",
    )?;
    let mut previous = 0;
    for escape in escapes {
        need(
            escape.step > previous
                && escape.step < steps.len()
                && steps[escape.step].recovery.as_ref().map(|r| &r.sha256)
                    == Some(&escape.envelope_sha256)
                && steps[escape.step].expected_spec == escape.destination_spec
                && steps[escape.step].expected_phase
                    == if escape.destination_spec == 135 {
                        "Hold"
                    } else {
                        "Applying"
                    }
                && escape.envelope_sha256.len() == 64
                && escape
                    .envelope_sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "public repair escape is not an exact ordered raw-envelope step",
        )?;
        previous = escape.step;
    }
    Ok(())
}

/// Check the new modes without changing the existing one-child prefix format.
pub fn validate_manifest(manifest: &Manifest, base: &Path) -> Result<()> {
    let prefix = manifest.schema == PREFIX_SCHEMA;
    let negative = manifest.schema == NEGATIVE_SCHEMA;
    need(
        prefix == manifest.public_repair_prefix.is_some()
            && (prefix || negative || manifest.public_rejected_handover.is_none()),
        "public repair descriptors cannot enter another rehearsal mode",
    )?;
    if !prefix && !negative {
        return Ok(());
    }
    need(
        manifest.steps.len() <= MAX_STEPS
            && manifest.fixture_council.is_none()
            && manifest.supplied_council.is_none()
            && manifest.fixture_hashi_peers.is_none()
            && manifest.fixture_expectations.is_none()
            && manifest.fixture_signed_transactions.is_none(),
        "public repair witness forbids other fixture modes or overrides",
    )?;
    if prefix {
        let plan = manifest
            .public_repair_prefix
            .as_ref()
            .ok_or("prefix plan absent")?;
        need(
            manifest.public_dynamic_repair.is_none()
                && manifest.native_operation_recorders_v2.is_empty()
                && manifest.repairs.len() == usize::from(plan.target == Target::Failed136)
                && manifest.steps.iter().all(|s| {
                    s.recovery.is_some()
                        && s.fixture_escape.is_none()
                        && !s.fixture_hashi
                        && s.expected_spec != 138
                }),
            "signing prefix cannot activate, run repaired native work or ordinary transactions",
        )?;
        let last = manifest.steps.last().ok_or("empty public prefix")?;
        let bytes = replay::artifact(
            base,
            last.recovery.as_ref().ok_or("prefix tick absent")?,
            recovery::MAX_INHERENT_BYTES,
        )?;
        let envelope = RecoveryInherent::decode_remark(&bytes)
            .map_err(|e| format!("prefix terminal: {e:?}"))?;
        need(
            envelope.encode_remark() == bytes && matches!(envelope.action, RecoveryAction::Tick),
            "public signing prefix must end with an executed maintenance Tick",
        )?;
    } else {
        need(
            manifest.public_dynamic_repair.is_some()
                && manifest.repairs.len() == 2
                && manifest.public_rejected_handover.is_some(),
            "negative handover replay requires both reserved repairs and its explicit descriptor",
        )?;
    }
    Ok(())
}

impl Session {
    pub fn public_prefix(
        plan: &PrefixPlan,
        rejected: Option<&RejectedHandover>,
        policy: &RecoveryPolicy,
        steps: &[Step],
    ) -> Result<Self> {
        public_identity(policy)?;
        let (destinations, spec, phase, cursor, counter) = match plan.target {
            Target::Hold135 => (&[135][..], 135, "Hold", 2, 1),
            Target::Failed136 => (&[135, 136][..], 136, "Failed", 0, 2),
        };
        escapes(&plan.escapes, steps, destinations)?;
        need(
            rejected.is_some() == (plan.target == Target::Failed136),
            "prefix rejected executor differs from its target",
        )?;
        if let Some(rejected) = rejected {
            rejected_identity(rejected)?;
        }
        let last = steps.last().ok_or("prefix terminal absent")?;
        need(
            last.expected_spec == spec
                && last.expected_phase == phase
                && plan
                    .escapes
                    .last()
                    .is_some_and(|e| e.step + 1 < steps.len()),
            "prefix must execute the installed terminal runtime after its last escape",
        )?;
        // No state root or nonce is supplied by the caller. This private terminal
        // is replaced by observed state in finish_prefix, never passed to finish.
        Ok(Self {
            scenario: Scenario {
                escapes: plan.escapes.clone(),
                expected_terminal: Terminal {
                    spec_version: spec,
                    phase: phase.into(),
                    cursor,
                    audit_cursor: 0,
                    operation_counter: counter,
                    active_policy_commitment: hx(&[0; 32]),
                    state_root: hx(&[0; 32]),
                    terminal_audit_blake2: None,
                    ordinary_blocks: 0,
                },
            },
            original: policy.clone(),
            active: policy.clone(),
            witnesses: Vec::new(),
            pending_handover: None,
            public_dynamic: true,
            public_scope: Some(Scope {
                target: Some(plan.target),
                rejected: rejected.cloned(),
            }),
        })
    }

    pub fn public_negative(
        scenario: &Scenario,
        rejected: &RejectedHandover,
        policy: &RecoveryPolicy,
        steps: &[Step],
    ) -> Result<Self> {
        public_identity(policy)?;
        rejected_identity(rejected)?;
        escapes(&scenario.escapes, steps, &[135, 136, 137])?;
        let mut session = Self::public_dynamic(scenario, policy, steps)?;
        session.public_scope = Some(Scope {
            target: None,
            rejected: Some(rejected.clone()),
        });
        Ok(session)
    }

    pub fn after_initialize_observed(
        &self,
        ext: &mut sp_io::TestExternalities,
        context: &WorkContext,
    ) -> Result<Option<RejectedInitialization>> {
        let rejected = self.public_scope.as_ref().and_then(|s| s.rejected.as_ref());
        if let Some(rejected) = rejected.filter(|r| r.spec_version == context.spec) {
            return RejectedInitialization::observe(self, rejected, ext, context).map(Some);
        }
        self.after_initialize(ext, context)?;
        Ok(None)
    }

    pub fn imported_observed_block(
        &mut self,
        context: &WorkContext,
        rejected: Option<&RejectedInitialization>,
    ) -> Result<()> {
        if let Some(rejected) = rejected {
            need(
                self.pending_handover == context.parent_authorization
                    && self.pending_handover.as_deref() == Some(rejected.authorization.as_slice())
                    && self.active == context.active
                    && self.witnesses.len() as u64 == context.counter,
                "rejected handover import is detached from its accepted escape",
            )?;
            // Deliberate rejection retains the handover until a new quorum repair.
            return Ok(());
        }
        self.imported_initialized_block(context)
    }

    pub fn finish_prefix(
        self,
        ext: &mut sp_io::TestExternalities,
        spec: u32,
        ordinary: usize,
        completed: bool,
        row: &BlockWitness,
        rejected_count: usize,
    ) -> Result<(Report, serde_json::Value)> {
        let target = self
            .public_scope
            .as_ref()
            .and_then(|s| s.target)
            .ok_or("not a public signing prefix")?;
        let expected = &self.scenario.expected_terminal;
        let current = status(ext)?;
        let root = replay::root(ext);
        let code = replay::code(ext)?;
        let code_hash = sp_io::hashing::blake2_256(&code);
        let nonce = staging_nonce(ext)?;
        let anchor = recovery_anchor(ext)?.ok_or("prefix branch anchor absent")?;
        let last_escape = self
            .witnesses
            .last()
            .ok_or("prefix never accepted an escape")?;
        let expected_failure =
            (target == Target::Failed136).then_some(recovery::RecoveryError::BadCode);
        need(
            !completed
                && ordinary == 0
                && self.witnesses.len() == self.scenario.escapes.len()
                && current.format_version == 1
                && current.policy == self.original.commitment()
                && current.phase
                    == if target == Target::Hold135 {
                        RecoveryPhase::Hold
                    } else {
                        RecoveryPhase::Failed
                    }
                && current.next_chunk == expected.cursor
                && current.operation_counter == expected.operation_counter
                && current.failure == expected_failure
                && spec == expected.spec_version
                && value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                    == Some(self.original.commitment().as_slice())
                && value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                    == Some(self.active.commitment().as_slice())
                && value(ext, recovery::AUDIT_CURSOR_STORAGE_KEY) == Some(0u32.encode())
                && value(ext, recovery::TERMINAL_AUDIT_STORAGE_KEY).is_none()
                && value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() == Some(&[1][..])
                && nonce == u64::from(target == Target::Failed136)
                && anchor == hash32(&last_escape.recovery_anchor)?
                && row.resulting_spec == spec
                && row.executing_code_blake2 == hex::encode(code_hash)
                && row.state_root == format!("{root:#x}")
                && row.phase == expected.phase
                && row.cursor == current.next_chunk
                && row.operation_counter == current.operation_counter
                && row.maintenance_session_frozen
                && row.independent_import_root_matches
                && row.disk_restart_root_matches,
            "public signing prefix does not match actual installed maintenance state",
        )?;
        let pending = value(ext, REPAIR_AUTH);
        need(
            pending == self.pending_handover
                && pending.is_some() == (target == Target::Failed136)
                && (rejected_count > 0) == (target == Target::Failed136),
            "prefix retained handover differs from observed initialization",
        )?;
        if let Some(raw) = &pending {
            let authorization =
                RepairAuthorization::decode_all(&mut raw.as_slice()).map_err(|e| e.to_string())?;
            need(
                sha(raw) == last_escape.authorization_sha256
                    && authorization.destination_spec == spec
                    && authorization.destination_code_hash == code_hash
                    && authorization.revised_policy == self.active.commitment(),
                "prefix pending authorization is not the accepted rejected repair",
            )?;
        }
        let observation = serde_json::json!({
            "schema":"sora-nexus-public-fixture-observed-repair-signing-context-v1",
            "parentBlock":row.number,"parentHash":row.hash,"stateRoot":row.state_root,
            "snapshotManifestSha256":row.disk_snapshot_manifest_sha256,
            "nextBlockNumber":row.number.checked_add(1).ok_or("prefix block overflow")?,
            "executingSpec":spec,"storedCodeBlake2":hx(&code_hash),"executingCodeBlake2":hx(&code_hash),
            "originalPolicyCommitment":hx(&self.original.commitment()),"activePolicyCommitment":hx(&self.active.commitment()),
            "originalPolicyScale":hx(&self.original.encode()),"activePolicyScale":hx(&self.active.encode()),
            "phase":format!("{:?}",current.phase),"cursor":current.next_chunk,"operationCounter":current.operation_counter,
            "failure":current.failure.map(|e|format!("{e:?}")),"stagingNonce":nonce,"recoveryAnchor":hx(&anchor),
            "pendingRepairAuthorizationScale":pending.as_ref().map(|b|hx(b)),
            "pendingRepairAuthorizationSha256":pending.as_ref().map(|b|sha(b)),
            "independentImportAndRestartVerified":true,"bridgeHaltRetained":true,
            "fixtureOnly":true,"productionReady":false,"activationVerified":false,"consensusSealsVerified":false
        });
        need(replay::root(ext) == root, "prefix observer changed state")?;
        Ok((
            Report {
                schema: "sora-nexus-public-fixture-prefix-council-witness-v1",
                authorizations: self.witnesses,
                terminal: Terminal {
                    active_policy_commitment: hx(&self.active.commitment()),
                    state_root: format!("{root:#x}"),
                    ..self.scenario.expected_terminal
                },
                signatures_verified_against_pinned_policy: true,
                automatic_activation_observed: false,
                public_test_council_generated: false,
                council_inventory_approval_verified: false,
                source_provenance_verified: false,
                production_ready: false,
                public_dynamic_fixture: true,
            },
            observation,
        ))
    }
}

fn staging_nonce(ext: &mut sp_io::TestExternalities) -> Result<u64> {
    value(ext, crate::repair_staging::NONCE_KEY)
        .map(|raw| u64::decode_all(&mut raw.as_slice()).map_err(|e| e.to_string()))
        .transpose()
        .map(|v| v.unwrap_or(0))
}

/// Created only after actual Wasm initialization, then joined to the independently
/// imported and disk-restored child. Never deserialized from caller JSON.
pub struct RejectedInitialization {
    authorization: Vec<u8>,
    initialized_root: H256,
    original: [u8; 32],
    active: [u8; 32],
    anchor: [u8; 32],
    executor: [u8; 32],
    nonce: u64,
}
impl RejectedInitialization {
    fn observe(
        session: &Session,
        rejected: &RejectedHandover,
        ext: &mut sp_io::TestExternalities,
        context: &WorkContext,
    ) -> Result<Self> {
        let root = replay::root(ext);
        let current = status(ext)?;
        let mut expected = context.parent_status.clone();
        expected.phase = RecoveryPhase::Failed;
        expected.failure = Some(recovery::RecoveryError::BadCode);
        let raw = context
            .parent_authorization
            .as_ref()
            .ok_or("negative handover has no accepted authorization")?;
        let authorization =
            RepairAuthorization::decode_all(&mut raw.as_slice()).map_err(|e| e.to_string())?;
        let anchor = recovery_anchor(ext)?.ok_or("negative handover anchor absent")?;
        let nonce = staging_nonce(ext)?;
        need(session.public_dynamic && context.spec == 136 && context.counter == 2
            && context.parent_status.next_chunk == 0
            && matches!(context.parent_status.phase, RecoveryPhase::Applying | RecoveryPhase::Failed)
            && (context.parent_status.failure.is_none() || context.parent_status.failure == Some(recovery::RecoveryError::BadCode))
            && current == expected && context.engine_hash == hash32(&rejected.executing_code_blake2)?
            && sp_io::hashing::blake2_256(&replay::code(ext)?) == context.engine_hash
            && value(ext, REPAIR_AUTH).as_ref() == Some(raw)
            && session.pending_handover.as_ref() == Some(raw)
            && authorization.destination_spec == 136 && authorization.operation_counter == 1
            && authorization.destination_code_hash == context.engine_hash
            && authorization.revised_policy == context.active.commitment()
            && authorization.policy == context.original.commitment()
            && authorization.recovery_anchor == anchor && authorization.authorization_domain == recovery::RecoveryDomain::public_fixture()
            && value(ext, recovery::POLICY_STORAGE_KEY).as_deref() == Some(context.original.commitment().as_slice())
            && value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref() == Some(context.active.commitment().as_slice())
            && value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() == Some(&[1][..])
            && value(ext, recovery::AUDIT_CURSOR_STORAGE_KEY) == Some(0u32.encode())
            && value(ext, recovery::TERMINAL_AUDIT_STORAGE_KEY).is_none() && (1..=2).contains(&nonce),
            "explicit negative handover did not freeze exact state while retaining its accepted authorization")?;
        need(
            replay::root(ext) == root,
            "negative handover observer changed state",
        )?;
        Ok(Self {
            authorization: raw.clone(),
            initialized_root: root,
            original: context.original.commitment(),
            active: context.active.commitment(),
            anchor,
            executor: context.engine_hash,
            nonce,
        })
    }

    pub fn accepted(
        self,
        ext: &mut sp_io::TestExternalities,
        row: &BlockWitness,
    ) -> Result<serde_json::Value> {
        let root = replay::root(ext);
        need(
            row.executing_code_blake2 == hex::encode(self.executor)
                && row.state_root == format!("{root:#x}")
                && row.maintenance_session_frozen
                && row.independent_import_root_matches
                && row.disk_restart_root_matches,
            "negative handover observation lacks the exact imported/restored child",
        )?;
        let current = status(ext)?;
        let pending =
            value(ext, REPAIR_AUTH).ok_or("rejected handover disappeared without a repair")?;
        let nonce = staging_nonce(ext)?;
        need(
            current.policy == self.original
                && recovery_anchor(ext)? == Some(self.anchor)
                && value(ext, recovery::POLICY_STORAGE_KEY).as_deref()
                    == Some(self.original.as_slice())
                && value(ext, recovery::BRIDGE_HALT_STORAGE_KEY).as_deref() == Some(&[1][..]),
            "negative handover child changed immutable recovery identity",
        )?;
        let escaped = row.resulting_spec == 137;
        if escaped {
            let authorization = RepairAuthorization::decode_all(&mut pending.as_slice())
                .map_err(|e| e.to_string())?;
            need(
                current.phase == RecoveryPhase::Applying
                    && current.failure.is_none()
                    && current.next_chunk == 0
                    && current.operation_counter == 3
                    && nonce == self.nonce
                    && authorization.destination_spec == 137
                    && authorization.operation_counter == 2
                    && authorization.phase == RecoveryPhase::Failed
                    && authorization.cursor == 0
                    && authorization.current_code_hash == self.executor
                    && authorization.current_executor_hash == self.executor
                    && authorization.destination_code_hash
                        == sp_io::hashing::blake2_256(&replay::code(ext)?)
                    && value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                        == Some(authorization.revised_policy.as_slice()),
                "rejected136 did not exit through the next atomic quorum repair",
            )?;
        } else {
            need(
                row.resulting_spec == 136
                    && current.phase == RecoveryPhase::Failed
                    && current.failure == Some(recovery::RecoveryError::BadCode)
                    && current.next_chunk == 0
                    && current.operation_counter == 2
                    && pending == self.authorization
                    && value(ext, recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
                        == Some(self.active.as_slice())
                    && sp_io::hashing::blake2_256(&replay::code(ext)?) == self.executor
                    && (nonce == self.nonce || self.nonce.checked_add(1) == Some(nonce)),
                "rejected136 work changed frozen state or lost its pending handover",
            )?;
        }
        need(
            row.cursor == current.next_chunk
                && row.operation_counter == current.operation_counter
                && row.phase == format!("{:?}", current.phase)
                && replay::root(ext) == root,
            "negative handover child/status join differs",
        )?;
        Ok(
            serde_json::json!({"schema":"sora-nexus-public-fixture-rejected-handover-observation-v1",
            "block":row.number,"blockHash":row.hash,"stateRoot":row.state_root,"snapshotManifestSha256":row.disk_snapshot_manifest_sha256,
            "executingCodeBlake2":row.executing_code_blake2,"initializedStateRoot":format!("{:#x}",self.initialized_root),
            "originalPolicyCommitment":hx(&self.original),"activePolicyCommitmentBefore":hx(&self.active),"recoveryAnchor":hx(&self.anchor),
            "failure":"BadCode","cursorBeforeWork":0,"operationCounterBeforeWork":2,
            "stagingNonceBeforeWork":self.nonce,"stagingNonceAfterWork":nonce,
            "retainedAuthorizationScale":hx(&self.authorization),"retainedAuthorizationSha256":sha(&self.authorization),
            "forwardEscapeTo137Observed":escaped,"independentImportAndRestartVerified":true,"readOnlyStateRootMatches":true,
            "fixtureOnly":true,"productionReady":false,"consensusSealsVerified":false}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 2,
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![recovery::AccountReplacement {
                old: sp_core::crypto::AccountId32::from(
                    hash32("0xde1563365ffd03e7958e9c69ac71fc3265db8214a962706ded8812f9a7265363")
                        .unwrap(),
                ),
                new: sp_core::crypto::AccountId32::from(
                    hash32("0xed51c2f39620ba2de7cf1321f02d5c9a0706943aa7a796c3f8aeb94e89cba45e")
                        .unwrap(),
                ),
            }],
            council: fixture::pairs().into_iter().map(|(a, _)| a).collect(),
            chunk_hashes: vec![[1; 32], [2; 32], [3; 32]],
            audit_chunk_hashes: vec![[4; 32], [5; 32]],
            evidence_commitment: [6; 32],
            terminal_audit_commitment: [7; 32],
        }
    }
    fn steps() -> Vec<Step> {
        (0..6)
            .map(|i| Step {
                slot: i + 1,
                timestamp: (i + 1) * 6000,
                authority_index: 0,
                recovery: Some(replay::Artifact {
                    file: "unused.scale".into(),
                    sha256: "01".repeat(32),
                }),
                fixture_escape: None,
                fixture_hashi: false,
                reject_previous_escapes: false,
                rejected_recovery: vec![],
                expected_spec: if i == 1 {
                    135
                } else if i >= 3 {
                    136
                } else {
                    134
                },
                expected_phase: if i == 1 {
                    "Hold"
                } else if i >= 4 {
                    "Failed"
                } else {
                    "Applying"
                }
                .into(),
            })
            .collect()
    }
    fn plan() -> PrefixPlan {
        PrefixPlan {
            target: Target::Failed136,
            escapes: vec![
                ExpectedEscape {
                    step: 1,
                    destination_spec: 135,
                    envelope_sha256: "01".repeat(32),
                },
                ExpectedEscape {
                    step: 3,
                    destination_spec: 136,
                    envelope_sha256: "01".repeat(32),
                },
            ],
        }
    }
    fn negative() -> (Session, sp_io::TestExternalities, WorkContext, Vec<u8>) {
        let original = policy();
        let mut active = original.clone();
        active.evidence_commitment = [9; 32];
        let code = b"unit-test-installed136".to_vec();
        let code_hash = sp_io::hashing::blake2_256(&code);
        let descriptor = RejectedHandover {
            spec_version: 136,
            executing_code_blake2: hx(&code_hash),
        };
        let mut session =
            Session::public_prefix(&plan(), Some(&descriptor), &original, &steps()).unwrap();
        session.active = active.clone();
        let auth = RepairAuthorization {
            policy: original.commitment(),
            phase: RecoveryPhase::Hold,
            cursor: 2,
            operation_counter: 1,
            current_code_hash: [11; 32],
            current_executor_hash: [11; 32],
            destination_code_hash: code_hash,
            destination_spec: 136,
            revised_policy: active.commitment(),
            authorization_domain: recovery::RecoveryDomain::public_fixture(),
            recovery_anchor: [42; 32],
        };
        let raw = auth.encode();
        session.pending_handover = Some(raw.clone());
        let witness = Witness {
            step: 3,
            block_number: recovery::CHECKPOINT_NUMBER + 4,
            parent_hash: hx(&[0; 32]),
            recovery_anchor: hx(&[42; 32]),
            envelope_sha256: "01".repeat(32),
            authorization_sha256: sha(&raw),
            signing_message_sha256: sha(&auth.signing_message()),
            phase: "Hold".into(),
            cursor: 2,
            operation_counter: 1,
            stored_code_blake2: hx(&[11; 32]),
            executing_code_blake2: hx(&[11; 32]),
            destination_code_blake2: hx(&code_hash),
            destination_spec: 136,
            revised_policy_commitment: hx(&active.commitment()),
            signers: vec![],
        };
        session.witnesses = vec![witness.clone(), witness];
        let current = RecoveryStatus {
            format_version: 1,
            policy: original.commitment(),
            phase: RecoveryPhase::Applying,
            next_chunk: 0,
            operation_counter: 2,
            failure: None,
        };
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            for (key, raw) in [
                (b":code".as_slice(), code.clone()),
                (REPAIR_AUTH, raw),
                (recovery::STATUS_STORAGE_KEY, current.encode()),
                (recovery::POLICY_STORAGE_KEY, original.commitment().to_vec()),
                (
                    recovery::ACTIVE_POLICY_STORAGE_KEY,
                    active.commitment().to_vec(),
                ),
                (recovery::RECOVERY_ANCHOR_STORAGE_KEY, vec![42; 32]),
                (recovery::BRIDGE_HALT_STORAGE_KEY, vec![1]),
                (recovery::AUDIT_CURSOR_STORAGE_KEY, 0u32.encode()),
                (crate::repair_staging::NONCE_KEY, 1u64.encode()),
            ] {
                sp_io::storage::set(key, &raw);
            }
        });
        let context = session
            .before_initialize(&mut ext, 136, &code)
            .unwrap()
            .unwrap();
        (session, ext, context, code)
    }
    fn fail(ext: &mut sp_io::TestExternalities) {
        let mut current = status(ext).unwrap();
        current.phase = RecoveryPhase::Failed;
        current.failure = Some(recovery::RecoveryError::BadCode);
        ext.execute_with(|| sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &current.encode()));
    }
    fn row(ext: &mut sp_io::TestExternalities, code: &[u8]) -> BlockWitness {
        BlockWitness {
            number: recovery::CHECKPOINT_NUMBER + 6,
            hash: hx(&[10; 32]),
            state_root: format!("{:#x}", replay::root(ext)),
            executing_code_blake2: hex::encode(sp_io::hashing::blake2_256(code)),
            resulting_spec: 136,
            phase: "Failed".into(),
            cursor: 0,
            operation_counter: 2,
            block_sha256: "12".repeat(32),
            disk_snapshot_manifest_sha256: "13".repeat(32),
            forbidden_extrinsics_rejected: 2,
            maintenance_session_frozen: true,
            independent_import_root_matches: true,
            disk_restart_root_matches: true,
            generated_authorization_sha256: None,
        }
    }
    #[test]
    fn fixed_public_identity_and_finite_prefix_targets_cannot_be_rebound() {
        let original = policy();
        public_identity(&original).unwrap();
        let mut changed = original.clone();
        changed.replacements[0].new = sp_core::crypto::AccountId32::new([91; 32]);
        assert!(public_identity(&changed).is_err());
        changed = original;
        changed.council.clear();
        assert!(public_identity(&changed).is_err());
        let mut plan = plan();
        plan.escapes[1].destination_spec = 137;
        assert!(Session::public_prefix(&plan, None, &policy(), &steps()).is_err());
    }
    #[test]
    fn normal_observer_stays_strict_and_negative_mode_requires_actual_bad_code() {
        let (session, mut ext, context, _) = negative();
        assert!(session
            .after_initialize_observed(&mut ext, &context)
            .is_err());
        fail(&mut ext);
        assert!(session.after_initialize(&mut ext, &context).is_err());
        let root = replay::root(&mut ext);
        assert!(session
            .after_initialize_observed(&mut ext, &context)
            .unwrap()
            .is_some());
        assert_eq!(replay::root(&mut ext), root);
        let mut ordinary = session.clone();
        ordinary.public_scope = None;
        assert!(ordinary
            .after_initialize_observed(&mut ext, &context)
            .is_err());
    }
    #[test]
    fn rejected_handover_requires_exact_authorization_code_and_counters() {
        let (session, mut ext, context, _) = negative();
        fail(&mut ext);
        let raw = value(&mut ext, REPAIR_AUTH).unwrap();
        ext.execute_with(|| sp_io::storage::set(REPAIR_AUTH, b"unrelated"));
        assert!(session
            .after_initialize_observed(&mut ext, &context)
            .is_err());
        ext.execute_with(|| sp_io::storage::set(REPAIR_AUTH, &raw));
        let mut wrong = context.clone();
        wrong.engine_hash = [8; 32];
        assert!(session.after_initialize_observed(&mut ext, &wrong).is_err());
        wrong = context;
        wrong.parent_status.next_chunk = 1;
        assert!(session.after_initialize_observed(&mut ext, &wrong).is_err());
    }
    #[test]
    fn negative_observation_requires_imported_restored_child_and_retains_handover() {
        let (mut session, mut ext, context, code) = negative();
        fail(&mut ext);
        let observation = session
            .after_initialize_observed(&mut ext, &context)
            .unwrap()
            .unwrap();
        session
            .imported_observed_block(&context, Some(&observation))
            .unwrap();
        assert!(session.pending_handover.is_some());
        let mut block = row(&mut ext, &code);
        block.disk_restart_root_matches = false;
        assert!(observation.accepted(&mut ext, &block).is_err());
        let observation = session
            .after_initialize_observed(&mut ext, &context)
            .unwrap()
            .unwrap();
        let block = row(&mut ext, &code);
        let result = observation.accepted(&mut ext, &block).unwrap();
        assert_eq!(result["forwardEscapeTo137Observed"], false);
    }
    #[test]
    fn signing_prefix_reads_actual_nonce_policy_and_pending_handover_without_activation() {
        let (session, mut ext, context, code) = negative();
        fail(&mut ext);
        let observation = session
            .after_initialize_observed(&mut ext, &context)
            .unwrap()
            .unwrap();
        let block = row(&mut ext, &code);
        observation.accepted(&mut ext, &block).unwrap();
        let (report, witness) = session
            .clone()
            .finish_prefix(&mut ext, 136, 0, false, &block, 1)
            .unwrap();
        assert!(!report.automatic_activation_observed);
        assert_eq!(witness["stagingNonce"], 1);
        assert_eq!(witness["cursor"], 0);
        assert_eq!(witness["operationCounter"], 2);
        assert_eq!(witness["failure"], "BadCode");
        assert_eq!(witness["activationVerified"], false);
        assert!(session.clone().finish(&mut ext, 136, 0, false).is_err());
        ext.execute_with(|| sp_io::storage::set(crate::repair_staging::NONCE_KEY, &2u64.encode()));
        let block = row(&mut ext, &code);
        assert!(session
            .finish_prefix(&mut ext, 136, 0, false, &block, 1)
            .is_err());
    }
}
