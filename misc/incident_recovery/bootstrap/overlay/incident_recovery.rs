//! Source-preserving spec-134 maintenance executive. Ordinary execution is
//! delegated unchanged through the checkpoint; no pallet list is re-ordered.

use crate::InherentDataExt;
use crate::{
    Block, Executive, Header, Runtime, RuntimeCall, RuntimeOrigin, System, Timestamp,
    UncheckedExtrinsic,
};
use alloc::{vec, vec::Vec};
use codec::{DecodeAll, Encode};
use common::recovery::{
    self, RecoveryAction, RecoveryCodeKind, RecoveryError, RecoveryInherent, RecoveryPhase,
    RecoveryPolicy, CHECKPOINT_HASH, CHECKPOINT_NUMBER, CHECKPOINT_STATE_ROOT,
};
use frame_support::{
    dispatch::{DispatchClass, DispatchInfo, DispatchResultWithPostInfo, Pays},
    inherent::ProvideInherent,
    traits::OnFinalize,
    weights::Weight,
};
use sp_runtime::{
    generic::Preamble,
    traits::{Block as BlockT, Header as HeaderT, LazyBlock as _},
    transaction_validity::{InvalidTransaction, TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, Digest, DigestItem, ExtrinsicInclusionMode,
};

mod generated;
const APPLIED: &[u8] = b":sora-nexus:recovery:inherent-applied:v1";
const STAGE_PREFIX: &[u8] = b":sora-nexus:recovery:code-stage:v1:";
const REPAIR_AUTH: &[u8] = b":sora-nexus:recovery:repair-authorization:v1";
pub const CHECKPOINT_TIME: &[u8] = b":sora-nexus:recovery:checkpoint-time:v1";
pub const ACTIVATION_TIME: &[u8] = b":sora-nexus:recovery:activation-time:v1";
const MAX_CODE_BYTES: usize = 32 * 1024 * 1024;

#[cfg(test)]
std::thread_local! {
    static TEST_POLICY: std::cell::RefCell<Option<RecoveryPolicy>> = const { std::cell::RefCell::new(None) };
}

/// Bind native fixture execution without making a production binding optional.
/// No test-policy entry point is compiled into a release Wasm or node.
#[cfg(test)]
pub(crate) fn with_test_policy<R>(policy: &RecoveryPolicy, run: impl FnOnce() -> R) -> R {
    policy.validate().expect("invalid test policy");
    struct Restore(Option<RecoveryPolicy>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_POLICY.with(|cell| *cell.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(TEST_POLICY.with(|cell| cell.replace(Some(policy.clone()))));
    run()
}

pub fn compiled_policy() -> Result<RecoveryPolicy, RecoveryError> {
    #[cfg(test)]
    if let Some(policy) = TEST_POLICY.with(|cell| cell.borrow().clone()) {
        return Ok(policy);
    }
    if generated::POLICY_SCALE.is_empty() || generated::POLICY_COMMITMENT == [0; 32] {
        return Err(RecoveryError::MissingCompiledPolicy);
    }
    let mut encoded = generated::POLICY_SCALE;
    let policy =
        RecoveryPolicy::decode_all(&mut encoded).map_err(|_| RecoveryError::InvalidPolicy)?;
    policy.validate()?;
    if policy.commitment() != generated::POLICY_COMMITMENT {
        return Err(RecoveryError::WrongPolicy);
    }
    Ok(policy)
}

pub fn original_policy() -> Result<RecoveryPolicy, RecoveryError> {
    #[cfg(test)]
    if let Some(policy) = TEST_POLICY.with(|cell| cell.borrow().clone()) {
        return Ok(policy);
    }
    if generated::ORIGINAL_POLICY_SCALE.is_empty() {
        return Err(RecoveryError::MissingCompiledPolicy);
    }
    let mut encoded = generated::ORIGINAL_POLICY_SCALE;
    let policy =
        RecoveryPolicy::decode_all(&mut encoded).map_err(|_| RecoveryError::InvalidPolicy)?;
    if policy.commitment() != generated::ORIGINAL_POLICY_COMMITMENT {
        return Err(RecoveryError::WrongPolicy);
    }
    policy.validate()?;
    Ok(policy)
}

pub fn initialize_block(header: &Header) -> ExtrinsicInclusionMode {
    if *header.number() <= CHECKPOINT_NUMBER {
        return Executive::initialize_block(header);
    }
    for key in recovery::TRANSIENT_CONTEXT_KEYS {
        assert!(
            !sp_io::storage::exists(key),
            "stale recovery dispatch context at block start"
        );
    }
    let policy = compiled_policy().expect("unbound recovery runtime");
    let original = original_policy().expect("unbound original recovery policy");
    if *header.number() == CHECKPOINT_NUMBER + 1 {
        assert_eq!(
            header.parent_hash().as_bytes(),
            &CHECKPOINT_HASH,
            "wrong checkpoint parent"
        );
        assert_eq!(
            sp_io::storage::root(sp_runtime::StateVersion::V0),
            CHECKPOINT_STATE_ROOT.to_vec(),
            "wrong checkpoint post-state"
        );
    }
    let digest = Digest {
        logs: header
            .digest()
            .logs()
            .iter()
            .filter(|item| item.as_pre_runtime().is_some())
            .cloned()
            .collect(),
    };
    System::reset_events();
    System::initialize(header.number(), header.parent_hash(), &digest);
    System::register_extra_weight_unchecked(
        <Runtime as frame_system::Config>::BlockWeights::get().base_block,
        DispatchClass::Mandatory,
    );
    sp_io::storage::clear(APPLIED);
    if *header.number() == CHECKPOINT_NUMBER + 1 {
        recovery::storage::initialize(&policy).expect("invalid recovery policy");
        let mut encoded_operators = generated::OPERATORS_SCALE;
        let operators = Vec::<crate::incident_recovery_consensus::OperatorReplacement>::decode_all(
            &mut encoded_operators,
        )
        .expect("invalid operator inventory");
        crate::incident_recovery_consensus::install(
            policy.commitment(),
            generated::PREVIOUS_GRANDPA_SET,
            &operators,
        )
        .expect("consensus checkpoint mismatch");
        sp_io::storage::set(CHECKPOINT_TIME, &Timestamp::get().encode());
    } else {
        accept_forward_repair(&policy).expect("invalid forward repair handover");
        assert_eq!(
            sp_io::storage::get(recovery::POLICY_STORAGE_KEY).as_deref(),
            Some(original.commitment().as_slice()),
            "wrong policy lineage"
        );
    }
    crate::incident_recovery_consensus::initialize_maintenance_epoch()
        .expect("invalid maintenance epoch");
    if *header.number() == CHECKPOINT_NUMBER + 1 {
        assert!(
            *pallet_babe::CurrentSlot::<Runtime>::get() >= generated::FIRST_RECOVERY_SLOT
                && generated::FIRST_RECOVERY_SLOT != 0,
            "unsafe first recovery slot"
        );
    }
    System::deposit_log(DigestItem::Other(recovery::lineage_marker(
        original.commitment(),
    )));
    System::note_finished_initialize();
    ExtrinsicInclusionMode::OnlyInherents
}

pub fn execute_block(block: <Block as BlockT>::LazyBlock) {
    if *block.header().number() <= CHECKPOINT_NUMBER {
        return Executive::execute_block(block);
    }
    initialize_block(block.header());
    for extrinsic in block.extrinsics() {
        apply_extrinsic(extrinsic.expect("invalid recovery extrinsic encoding"))
            .expect("invalid recovery extrinsic")
            .expect("failed recovery inherent");
    }
    assert_eq!(
        finalize_block(),
        *block.header(),
        "recovery block execution differs from header"
    );
}

pub fn apply_extrinsic(extrinsic: UncheckedExtrinsic) -> ApplyExtrinsicResult {
    if System::block_number() <= CHECKPOINT_NUMBER {
        return Executive::apply_extrinsic(extrinsic);
    }
    if !matches!(extrinsic.preamble, Preamble::Bare(_)) {
        return Err(InvalidTransaction::Call.into());
    }
    let encoded = extrinsic.encode();
    let mut call_weight = Weight::from_parts(500_000_000, recovery::MAX_INHERENT_BYTES as u64);
    let result: DispatchResultWithPostInfo = match extrinsic.function {
        RuntimeCall::Timestamp(pallet_timestamp::Call::set { now })
            if System::extrinsic_index() == Some(0) =>
        {
            ensure_remaining_weight(call_weight)?;
            Timestamp::set(RuntimeOrigin::none(), now)
                .map(|_| Default::default())
                .map_err(Into::into)
        }
        RuntimeCall::System(frame_system::Call::remark { remark })
            if System::extrinsic_index() == Some(1) && !sp_io::storage::exists(APPLIED) =>
        {
            let envelope =
                RecoveryInherent::decode_remark(&remark).map_err(|_| InvalidTransaction::Call)?;
            let policy = compiled_policy().map_err(|_| InvalidTransaction::Call)?;
            call_weight = call_weight.saturating_add(
                checked_storage_weight(&envelope.action).map_err(|_| InvalidTransaction::Call)?,
            );
            ensure_remaining_weight(call_weight)?;
            apply_action(&policy, envelope.action).map_err(|_| InvalidTransaction::Call)?;
            sp_io::storage::set(APPLIED, &[1]);
            Ok(Default::default())
        }
        _ => return Err(InvalidTransaction::Call.into()),
    };
    if result.is_err() {
        return Err(InvalidTransaction::BadMandatory.into());
    }
    System::note_extrinsic(encoded);
    let info = DispatchInfo {
        call_weight,
        extension_weight: Weight::zero(),
        class: DispatchClass::Mandatory,
        pays_fee: Pays::No,
    };
    System::register_extra_weight_unchecked(info.call_weight, info.class);
    System::note_applied_extrinsic(&result, info);
    Ok(Ok(()))
}

/// Include already-accounted base/timestamp work and this entire envelope before
/// any execution. Checked addition also rejects saturated/overflowing totals.
fn ensure_remaining_weight(weight: Weight) -> Result<(), InvalidTransaction> {
    let used = frame_system::BlockWeight::<Runtime>::get().total();
    if used.checked_add(&weight).is_some_and(|total| {
        total.all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block)
    }) {
        Ok(())
    } else {
        Err(InvalidTransaction::ExhaustsResources)
    }
}

/// Decoding and each native/bookkeeping limit are checked before action execution
/// or unchecked Mandatory registration, even in a failed phase.
fn checked_storage_weight(action: &RecoveryAction) -> Result<Weight, RecoveryError> {
    let weight = match action {
        RecoveryAction::Incoming(payload) => {
            crate::incident_recovery_incoming::incoming_encoded_weight(payload)?
        }
        RecoveryAction::Outgoing(payload) => {
            crate::incident_recovery_outgoing::outgoing_encoded_weight(payload)?
        }
        _ => return Ok(committed_storage_weight(action)),
    };
    if !weight.all_lte(<Runtime as frame_system::Config>::BlockWeights::get().max_block) {
        return Err(RecoveryError::ChunkBounds);
    }
    Ok(weight)
}

/// Charge bounded storage work in addition to envelope/consensus overhead.
/// Exact reads probe length before loading bytes. Empty-prefix checks probe the
/// exact key and successor; writes include cursor/status and failure handling.
/// This applies only after the checkpoint, never to preserved execution. Native
/// incoming/outgoing operations are charged even when committed work freezes
/// recovery. No native action registers a duplicate success-only charge.
fn committed_storage_weight(action: &RecoveryAction) -> Weight {
    let fixed_reads = 8u64.saturating_add(if matches!(action, RecoveryAction::TerminalAudit(_)) {
        recovery::TRANSIENT_CONTEXT_KEYS.len() as u64
    } else {
        0
    });
    let (items, reads, writes) = match action {
        RecoveryAction::Incoming(payload) => {
            // The executable path uses checked_storage_weight to reject malformed
            // envelopes; this estimator remains conservative on decode errors.
            return crate::incident_recovery_incoming::incoming_encoded_weight(payload)
                .unwrap_or(Weight::MAX);
        }
        RecoveryAction::Outgoing(payload) => {
            return crate::incident_recovery_outgoing::outgoing_encoded_weight(payload)
                .unwrap_or(Weight::MAX);
        }
        RecoveryAction::IncomingAudit(_) => {
            return crate::incident_recovery_incoming::incoming_audit_weight()
        }
        RecoveryAction::Audit(chunk) => {
            let n = chunk.witnesses.len() as u64;
            (n, n.saturating_mul(2), 1)
        }
        RecoveryAction::Apply(chunk) => {
            let n = chunk.edits.len() as u64;
            (n, n.saturating_mul(2), n.saturating_add(1))
        }
        RecoveryAction::PrefixAudit(chunk) => {
            let n = chunk.prefixes.len() as u64;
            (n, n.saturating_mul(2), 1)
        }
        RecoveryAction::TerminalAudit(audit) => {
            let n = audit
                .issuance
                .len()
                .saturating_add(audit.zero_claims.len())
                .saturating_add(audit.absent.len())
                .saturating_add(
                    audit
                        .collateral
                        .iter()
                        .map(|item| item.retained.len())
                        .sum::<usize>(),
                ) as u64;
            (n, n.saturating_mul(2), 2)
        }
        _ => return Weight::zero(),
    };
    // Include fixed policy/status/cursor admission, a full bounded-value hashing
    // allowance, and conservative trie-path/key proof overhead per witness.
    <Runtime as frame_system::Config>::DbWeight::get()
        .reads_writes(reads.saturating_add(fixed_reads), writes.saturating_add(2))
        .saturating_add(Weight::from_parts(
            5_000_000_000u64.saturating_add(items.saturating_mul(100_000_000)),
            (recovery::MAX_CHUNK_BYTES as u64).saturating_add(items.saturating_mul(8_192)),
        ))
}

pub fn finalize_block() -> Header {
    if System::block_number() <= CHECKPOINT_NUMBER {
        return Executive::finalize_block();
    }
    assert!(sp_io::storage::exists(APPLIED), "recovery inherent missing");
    sp_io::storage::clear(APPLIED);
    System::note_inherents_applied();
    System::note_finished_extrinsics();
    Timestamp::on_finalize(System::block_number());
    crate::incident_recovery_consensus::finalize_maintenance_epoch();
    automatic_activation().expect("recovery activation invariant failed");
    for key in recovery::TRANSIENT_CONTEXT_KEYS {
        assert!(
            !sp_io::storage::exists(key),
            "recovery dispatch context escaped its transaction"
        );
    }
    System::finalize()
}

pub fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<UncheckedExtrinsic> {
    if System::block_number() < CHECKPOINT_NUMBER
        || (System::block_number() == CHECKPOINT_NUMBER
            && !sp_io::storage::exists(recovery::POLICY_STORAGE_KEY))
    {
        // Called after initialize_block during ordinary authoring; checkpoint
        // replay remains entirely the original executive/inherent implementation.
        return data.create_extrinsics();
    }
    let time = Timestamp::create_inherent(&data).expect("timestamp inherent missing");
    let remark = data
        .get_data::<Vec<u8>>(&recovery::INHERENT_IDENTIFIER)
        .expect("malformed recovery data")
        .expect("recovery data missing");
    RecoveryInherent::decode_remark(&remark).expect("malformed recovery envelope");
    vec![
        UncheckedExtrinsic::new_bare_legacy(RuntimeCall::Timestamp(time)),
        UncheckedExtrinsic::new_bare_legacy(RuntimeCall::System(frame_system::Call::remark {
            remark,
        })),
    ]
}

pub fn check_inherents(
    block: <Block as BlockT>::LazyBlock,
    data: sp_inherents::InherentData,
) -> sp_inherents::CheckInherentsResult {
    if *block.header().number() <= CHECKPOINT_NUMBER {
        return data.check_extrinsics(&block);
    }
    let mut result = sp_inherents::CheckInherentsResult::new();
    let fail = |result: &mut sp_inherents::CheckInherentsResult| {
        let _ = result.put_error(
            recovery::INHERENT_IDENTIFIER,
            &sp_inherents::MakeFatalError::from(1u8),
        );
    };
    let mut count = 0;
    for (index, item) in block.extrinsics().enumerate() {
        count += 1;
        let Ok(item) = item else {
            fail(&mut result);
            break;
        };
        if !matches!(item.preamble, Preamble::Bare(_)) {
            fail(&mut result);
            break;
        }
        match (index, item.function) {
            (0, RuntimeCall::Timestamp(call @ pallet_timestamp::Call::set { .. })) => {
                if let Err(error) = Timestamp::check_inherent(&call, &data) {
                    let _ = result
                        .put_error(<Timestamp as ProvideInherent>::INHERENT_IDENTIFIER, &error);
                }
            }
            (1, RuntimeCall::System(frame_system::Call::remark { remark }))
                if RecoveryInherent::decode_remark(&remark).is_ok() => {}
            _ => {
                fail(&mut result);
                break;
            }
        }
    }
    if count != 2 {
        fail(&mut result);
    }
    result
}

pub fn validate_transaction(
    source: TransactionSource,
    tx: UncheckedExtrinsic,
    block_hash: crate::Hash,
) -> TransactionValidity {
    if System::block_number() < CHECKPOINT_NUMBER {
        Executive::validate_transaction(source, tx, block_hash)
    } else {
        InvalidTransaction::Call.into()
    }
}

pub fn offchain_worker(header: &Header) {
    if *header.number() < CHECKPOINT_NUMBER {
        Executive::offchain_worker(header);
    }
}

fn apply_action(policy: &RecoveryPolicy, action: RecoveryAction) -> Result<(), RecoveryError> {
    apply_action_with_artifacts(
        policy,
        action,
        crate::VERSION.spec_version,
        generated::HOLD_WASM,
        generated::HOLD_CODE_HASH,
        generated::HOLD_CODE_LEN,
    )
}

// Inputs are always compiled constants in production. Keeping this part separate
// lets native tests exercise the exact same escape state machine for every
// reserved maintenance version, with an explicitly mocked runtime-version host.
fn apply_action_with_artifacts(
    policy: &RecoveryPolicy,
    action: RecoveryAction,
    executing_spec: u32,
    hold_wasm: &[u8],
    hold_code_hash: [u8; 32],
    hold_code_len: u32,
) -> Result<(), RecoveryError> {
    // Escape is handled first; a failed ordinary chunk cannot lock it out.
    if let RecoveryAction::Escape {
        authorization,
        signatures,
        work_policy,
    } = action
    {
        let original = original_policy()?;
        let code = if executing_spec == 134 {
            if work_policy.is_some()
                || hold_wasm.is_empty()
                || hold_wasm.len() != hold_code_len as usize
            {
                return Err(RecoveryError::BadCode);
            }
            hold_wasm.to_vec()
        } else {
            assembled(
                RecoveryCodeKind::Repair,
                Some(authorization.destination_code_hash),
            )?
        };
        let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
        if executing_spec == 134 {
            recovery::storage::authorize_escape(
                &original,
                &authorization,
                &signatures,
                hold_code_hash,
                &code,
            )?;
        } else {
            if !matches!(authorization.destination_spec, 136 | 137)
                || authorization.destination_spec <= executing_spec
                || authorization.current_code_hash
                    != sp_io::hashing::blake2_256(
                        &sp_io::storage::get(b":code").ok_or(RecoveryError::BadCode)?,
                    )
                || authorization.current_executor_hash != authorization.current_code_hash
                || work_policy.is_none()
            {
                return Err(RecoveryError::BadCode);
            }
        }
        validate_code(&code, authorization.destination_spec)?;
        sp_io::storage::start_transaction();
        let transition = if let Some(revised) = work_policy {
            recovery::storage::install_work_policy(&original, &revised, &authorization, &signatures)
        } else {
            recovery::storage::record_hold(status)
        };
        if let Err(error) = transition {
            sp_io::storage::rollback_transaction();
            return Err(error);
        }
        if executing_spec != 134 {
            sp_io::storage::set(REPAIR_AUTH, &authorization.encode());
        }
        install_code(&code);
        sp_io::storage::commit_transaction();
        return Ok(());
    }
    let before = recovery::storage::status()?;
    let allow_new_failure = |error| allow_frozen_failure(before.as_ref(), error);
    match action {
        RecoveryAction::Tick => Ok(()),
        RecoveryAction::StageCode { kind, index, bytes } => stage(kind, index, &bytes),
        RecoveryAction::Apply(chunk) => recovery::storage::apply_chunk(policy, &chunk)
            .map(|_| ())
            .or_else(allow_new_failure),
        RecoveryAction::Audit(chunk) => recovery::storage::audit_chunk(policy, &chunk)
            .map(|_| ())
            .or_else(allow_new_failure),
        RecoveryAction::PrefixAudit(chunk) => recovery::storage::audit_prefix_chunk(policy, &chunk)
            .map(|_| ())
            .or_else(allow_new_failure),
        RecoveryAction::TerminalAudit(audit) => {
            recovery::invariants::verify_and_record(policy, &audit)
                .map(|_| ())
                .or_else(allow_new_failure)
        }
        RecoveryAction::Incoming(payload) => {
            // committed_storage_weight includes native/bookkeeping work for
            // both success and accepted frozen failure, without double charging.
            crate::incident_recovery_incoming::apply_encoded_with_policy(policy, &payload)
                .map(|_| ())
                .or_else(allow_new_failure)
        }
        RecoveryAction::IncomingAudit(payload) => {
            // committed_storage_weight charges before execution, including failure.
            crate::incident_recovery_incoming::audit_encoded_with_policy(policy, &payload)
                .map(|_| ())
                .or_else(allow_new_failure)
        }
        RecoveryAction::Outgoing(payload) => {
            crate::incident_recovery_outgoing::apply_encoded_with_policy(policy, &payload)
                .map(|_| ())
                .or_else(allow_new_failure)
        }
        RecoveryAction::Escape { .. } => unreachable!(),
    }
}

fn allow_frozen_failure(
    before: Option<&recovery::RecoveryStatus>,
    error: RecoveryError,
) -> Result<(), RecoveryError> {
    let Some(before) = before.filter(|status| {
        matches!(
            status.phase,
            RecoveryPhase::Applying | RecoveryPhase::Auditing
        )
    }) else {
        return Err(error);
    };
    let mut expected = before.clone();
    expected.phase = RecoveryPhase::Failed;
    expected.failure = Some(error);
    if recovery::storage::status()?.as_ref() == Some(&expected) {
        Ok(())
    } else {
        Err(error)
    }
}

fn stage_key(kind: RecoveryCodeKind, index: Option<u32>) -> Vec<u8> {
    let mut key = STAGE_PREFIX.to_vec();
    key.extend((kind, index).encode());
    key
}
fn stage(kind: RecoveryCodeKind, index: u32, bytes: &[u8]) -> Result<(), RecoveryError> {
    let committed = match kind {
        RecoveryCodeKind::Final => Some(generated::FINAL_CHUNK_HASHES),
        RecoveryCodeKind::Hold => Some(generated::HOLD_CHUNK_HASHES),
        RecoveryCodeKind::Repair => None,
    };
    stage_with_commitments(kind, index, bytes, committed)
}

fn stage_with_commitments(
    kind: RecoveryCodeKind,
    index: u32,
    bytes: &[u8],
    committed: Option<&[[u8; 32]]>,
) -> Result<(), RecoveryError> {
    if bytes.is_empty()
        || bytes.len() > recovery::MAX_CODE_CHUNK_BYTES
        || index as usize >= MAX_CODE_BYTES / recovery::MAX_CODE_CHUNK_BYTES
    {
        return Err(RecoveryError::ChunkBounds);
    }
    let cursor = stage_count(kind)?.unwrap_or(0);
    if let Some(hashes) = committed {
        if hashes.get(index as usize) != Some(&sp_io::hashing::blake2_256(bytes)) || index != cursor
        {
            return Err(RecoveryError::WrongChunk);
        }
    } else if index > cursor {
        return Err(RecoveryError::WrongChunk);
    }
    if index > 0
        && sp_io::storage::read(&stage_key(kind, Some(index - 1)), &mut [], 0)
            != Some(recovery::MAX_CODE_CHUNK_BYTES as u32)
    {
        return Err(RecoveryError::ChunkBounds);
    }
    sp_io::storage::set(&stage_key(kind, Some(index)), bytes);
    let next = if kind == RecoveryCodeKind::Repair && index == 0 {
        1
    } else {
        core::cmp::max(cursor, index + 1)
    };
    sp_io::storage::set(&stage_key(kind, None), &next.encode());
    Ok(())
}
fn stage_count(kind: RecoveryCodeKind) -> Result<Option<u32>, RecoveryError> {
    let key = stage_key(kind, None);
    let Some(len) = sp_io::storage::read(&key, &mut [], 0) else {
        return Ok(None);
    };
    if len != 4 {
        return Err(RecoveryError::ChunkBounds);
    }
    let bytes = sp_io::storage::get(&key).ok_or(RecoveryError::BadCode)?;
    let count = u32::decode_all(&mut bytes.as_ref()).map_err(|_| RecoveryError::BadCode)?;
    if count as usize > MAX_CODE_BYTES / recovery::MAX_CODE_CHUNK_BYTES {
        return Err(RecoveryError::ChunkBounds);
    }
    Ok(Some(count))
}
fn assembled(
    kind: RecoveryCodeKind,
    authorized: Option<[u8; 32]>,
) -> Result<Vec<u8>, RecoveryError> {
    let (hash, len) = match kind {
        RecoveryCodeKind::Final => (generated::FINAL_CODE_HASH, Some(generated::FINAL_CODE_LEN)),
        RecoveryCodeKind::Hold => (generated::HOLD_CODE_HASH, Some(generated::HOLD_CODE_LEN)),
        RecoveryCodeKind::Repair => (authorized.ok_or(RecoveryError::BadCode)?, None),
    };
    assembled_with_commitment(kind, hash, len)
}

fn assembled_with_commitment(
    kind: RecoveryCodeKind,
    hash: [u8; 32],
    len: Option<u32>,
) -> Result<Vec<u8>, RecoveryError> {
    let count = stage_count(kind)?.ok_or(RecoveryError::BadCode)?;
    if count == 0 {
        return Err(RecoveryError::BadCode);
    }
    // Inspect every stored length before allocating a guest buffer. This also
    // rejects corrupted staging data after a crash or an authorized repair.
    let mut total = 0usize;
    for index in 0..count {
        let len = sp_io::storage::read(&stage_key(kind, Some(index)), &mut [], 0)
            .ok_or(RecoveryError::BadCode)? as usize;
        if len == 0
            || len > recovery::MAX_CODE_CHUNK_BYTES
            || (index + 1 < count && len != recovery::MAX_CODE_CHUNK_BYTES)
        {
            return Err(RecoveryError::ChunkBounds);
        }
        total = total.checked_add(len).ok_or(RecoveryError::ChunkBounds)?;
        if total > MAX_CODE_BYTES {
            return Err(RecoveryError::ChunkBounds);
        }
    }
    let mut code = Vec::with_capacity(total);
    for index in 0..count {
        code.extend_from_slice(
            &sp_io::storage::get(&stage_key(kind, Some(index))).ok_or(RecoveryError::BadCode)?,
        );
    }
    if hash == [0; 32]
        || sp_io::hashing::blake2_256(&code) != hash
        || len.is_some_and(|len| code.len() != len as usize)
    {
        return Err(RecoveryError::BadCode);
    }
    Ok(code)
}
fn validate_code(code: &[u8], spec: u32) -> Result<(), RecoveryError> {
    let bytes = sp_io::misc::runtime_version(code).ok_or(RecoveryError::BadCode)?;
    let version = sp_version::RuntimeVersion::decode_all(&mut bytes.as_slice())
        .map_err(|_| RecoveryError::BadCode)?;
    validate_runtime_version(&version, spec)
}
fn validate_runtime_version(
    version: &sp_version::RuntimeVersion,
    spec: u32,
) -> Result<(), RecoveryError> {
    let expected_transaction = match spec {
        135..=137 => 132,
        138 => 134,
        _ => return Err(RecoveryError::BadCode),
    };
    if version.spec_name != "sora-substrate"
        || version.spec_version != spec
        || version.transaction_version != expected_transaction
    {
        return Err(RecoveryError::BadCode);
    }
    Ok(())
}
fn install_code(code: &[u8]) {
    sp_io::storage::set(b":code", code);
    System::deposit_log(DigestItem::RuntimeEnvironmentUpdated);
    System::deposit_event(frame_system::Event::<Runtime>::CodeUpdated);
}
fn automatic_activation() -> Result<(), RecoveryError> {
    automatic_activation_with_artifacts(
        generated::FINAL_CODE_HASH,
        generated::FINAL_CODE_LEN,
        generated::OPERATORS_SCALE,
    )
}

fn automatic_activation_with_artifacts(
    final_hash: [u8; 32],
    final_len: u32,
    encoded_operators: &[u8],
) -> Result<(), RecoveryError> {
    let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if status.phase != RecoveryPhase::Ready {
        return Ok(());
    }
    let code = match assembled_with_commitment(RecoveryCodeKind::Final, final_hash, Some(final_len))
    {
        Ok(code) => code,
        Err(RecoveryError::BadCode) => return Ok(()),
        Err(error) => return Err(error),
    };
    let policy = compiled_policy()?;
    if policy.terminal_audit_commitment == [0; 32] {
        return Err(RecoveryError::IncompleteAudit);
    }
    validate_code(&code, 138)?;
    validate_fee_clocks()?;
    let operators = Vec::<crate::incident_recovery_consensus::OperatorReplacement>::decode_all(
        &mut &encoded_operators[..],
    )
    .map_err(|_| RecoveryError::WrongPreimage)?;
    crate::incident_recovery_consensus::validate_activation(&operators)
        .map_err(|_| RecoveryError::IncompleteAudit)?;
    complete_and_install(&policy, &code)
}

fn complete_and_install(policy: &RecoveryPolicy, code: &[u8]) -> Result<(), RecoveryError> {
    sp_io::storage::start_transaction();
    if let Err(error) = recovery::storage::mark_complete(policy) {
        sp_io::storage::rollback_transaction();
        return Err(error);
    }
    sp_io::storage::set(ACTIVATION_TIME, &Timestamp::get().encode());
    // The final runtime's first pre-Executive admission clears liveness once,
    // shifts downtime fee baselines and starts its reviewed migration path.
    install_code(code);
    sp_io::storage::commit_transaction();
    Ok(())
}
fn accept_forward_repair(policy: &RecoveryPolicy) -> Result<(), RecoveryError> {
    accept_forward_repair_at(policy, crate::VERSION.spec_version)
}

fn accept_forward_repair_at(
    policy: &RecoveryPolicy,
    executing_spec: u32,
) -> Result<(), RecoveryError> {
    let Some(bytes) = sp_io::storage::get(REPAIR_AUTH) else {
        return Ok(());
    };
    let authorization = recovery::RepairAuthorization::decode_all(&mut bytes.as_ref())
        .map_err(|_| RecoveryError::BadCode)?;
    if authorization.destination_spec != executing_spec
        || authorization.revised_policy != policy.commitment()
        || authorization.destination_code_hash
            != sp_io::hashing::blake2_256(
                &sp_io::storage::get(b":code").ok_or(RecoveryError::BadCode)?,
            )
    {
        return Err(RecoveryError::BadCode);
    }
    let status = recovery::storage::status()?.ok_or(RecoveryError::WrongPolicy)?;
    if status.phase != RecoveryPhase::Applying
        || status.next_chunk != 0
        || Some(status.operation_counter) != authorization.operation_counter.checked_add(1)
        || status.policy != original_policy()?.commitment()
        || sp_io::storage::get(recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref()
            != Some(policy.commitment().as_slice())
    {
        return Err(RecoveryError::Replay);
    }
    sp_io::storage::clear(REPAIR_AUTH);
    Ok(())
}

fn validate_fee_clocks() -> Result<(), RecoveryError> {
    let checkpoint = sp_io::storage::get(CHECKPOINT_TIME).ok_or(RecoveryError::WrongPreimage)?;
    let checkpoint =
        u64::decode_all(&mut checkpoint.as_ref()).map_err(|_| RecoveryError::WrongPreimage)? / 1000;
    let activation = Timestamp::get() / 1000;
    let downtime = activation
        .checked_sub(checkpoint)
        .ok_or(RecoveryError::WrongPreimage)?;
    let prefix = frame_support::storage::storage_prefix(b"Kensetsu", b"CollateralInfos");
    let mut last = prefix.to_vec();
    let mut raw_count = 0;
    while let Some(key) = sp_io::storage::next_key(&last) {
        if !key.starts_with(&prefix) {
            break;
        }
        raw_count += 1;
        if raw_count > 4096 {
            return Err(RecoveryError::ChunkBounds);
        }
        last = key;
    }
    let mut decoded_count = 0;
    for (_, info) in kensetsu::CollateralInfos::<Runtime>::iter().take(4097) {
        decoded_count += 1;
        if info.last_fee_update_time > checkpoint
            || info.last_fee_update_time.checked_add(downtime).is_none()
        {
            return Err(RecoveryError::WrongPreimage);
        }
    }
    if decoded_count != raw_count {
        return Err(RecoveryError::WrongPreimage);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_audit_storage_charge_scales_with_the_full_bounded_read_set() {
        let action = |count| {
            RecoveryAction::Audit(recovery::RecoveryAuditChunk {
                index: 0,
                witnesses: (0..count)
                    .map(|index| recovery::StorageWitness {
                        scope: recovery::StorageScope::Top,
                        key: (index as u32).encode(),
                        value_hash: None,
                    })
                    .collect(),
            })
        };
        let small = committed_storage_weight(&action(1));
        let maximum = committed_storage_weight(&action(recovery::MAX_AUDIT_WITNESSES));
        let database = <Runtime as frame_system::Config>::DbWeight::get();
        assert!(maximum
            .all_gte(database.reads_writes((recovery::MAX_AUDIT_WITNESSES as u64) * 2 + 8, 3)));
        assert!(maximum.ref_time() > small.ref_time());
        assert!(maximum.proof_size() > small.proof_size());
        assert_eq!(
            maximum.proof_size(),
            recovery::MAX_CHUNK_BYTES as u64 + recovery::MAX_AUDIT_WITNESSES as u64 * 8_192
        );
        let full_block_allowance = maximum
            .saturating_add(
                Weight::from_parts(500_000_000, recovery::MAX_INHERENT_BYTES as u64)
                    .saturating_mul(2),
            )
            .saturating_add(<Runtime as frame_system::Config>::BlockWeights::get().base_block);
        assert!(
            full_block_allowance.ref_time()
                < <Runtime as frame_system::Config>::BlockWeights::get()
                    .max_block
                    .ref_time()
        );
        let (_, _, _, terminal) = fixture();
        let expected_reads = 2 + 8 + recovery::TRANSIENT_CONTEXT_KEYS.len() as u64;
        assert!(
            committed_storage_weight(&RecoveryAction::TerminalAudit(terminal))
                .all_gte(database.reads_writes(expected_reads, 4))
        );
    }

    fn fixture() -> (
        RecoveryPolicy,
        recovery::RecoveryChunk,
        recovery::RecoveryAuditChunk,
        recovery::invariants::TerminalAudit,
    ) {
        use recovery::invariants::{IssuanceInvariant, StoredAmount, TerminalAudit};
        use recovery::{GuardedStorageEdit, StorageScope, StorageWitness};
        let balance = 10u128.encode();
        let hash = sp_io::hashing::blake2_256(&balance);
        let chunk = recovery::RecoveryChunk {
            index: 0,
            edits: vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key: b"fixture-issuance".to_vec(),
                before: None,
                after: Some(balance),
            }],
        };
        let audit = recovery::RecoveryAuditChunk {
            index: 0,
            witnesses: vec![StorageWitness {
                scope: StorageScope::Top,
                key: b"fixture-issuance".to_vec(),
                value_hash: Some(hash),
            }],
        };
        let terminal = TerminalAudit {
            issuance: vec![IssuanceInvariant {
                asset: [1; 32],
                current: StoredAmount {
                    scope: StorageScope::Top,
                    key: b"fixture-issuance".to_vec(),
                    record_hash: hash,
                    offset: 0,
                },
                checkpoint_issuance: 12,
                authenticated_mints: 0,
                burned: 2,
            }],
            collateral: vec![],
            zero_claims: vec![],
            absent: vec![],
        };
        let policy = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![sp_runtime::AccountId32::new([1; 32])],
            chunk_hashes: vec![chunk.commitment()],
            audit_chunk_hashes: vec![audit.commitment()],
            evidence_commitment: [2; 32],
            terminal_audit_commitment: terminal.commitment(),
        };
        (policy, chunk, audit, terminal)
    }

    fn maintenance_header(number: u32, parent: sp_core::H256, slot: u64) -> Header {
        let digest = sp_consensus_babe::digests::PreDigest::SecondaryPlain(
            sp_consensus_babe::digests::SecondaryPlainPreDigest {
                authority_index: 0,
                slot: slot.into(),
            },
        );
        Header::new(
            number,
            Default::default(),
            Default::default(),
            parent,
            Digest {
                logs: vec![DigestItem::PreRuntime(
                    sp_consensus_babe::BABE_ENGINE_ID,
                    digest.encode(),
                )],
            },
        )
    }

    fn maintenance_fixture(policy: &RecoveryPolicy) -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::new_with_state_version(
            Default::default(),
            sp_runtime::StateVersion::V0,
        );
        ext.execute_with(|| {
            recovery::storage::initialize(policy).unwrap();
            System::set_block_number(CHECKPOINT_NUMBER + 1);
            pallet_session::CurrentIndex::<Runtime>::put(7);
            pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
            pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
            pallet_babe::EpochIndex::<Runtime>::put(0);
            let authorities =
                frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::try_from(vec![(
                    sp_consensus_babe::AuthorityId::from(sp_core::sr25519::Public::from_raw(
                        [31; 32],
                    )),
                    1,
                )])
                .unwrap();
            pallet_babe::Authorities::<Runtime>::put(authorities.clone());
            pallet_babe::NextAuthorities::<Runtime>::put(authorities);
            pallet_babe::EpochConfig::<Runtime>::put(sp_consensus_babe::BabeEpochConfiguration {
                c: (1, 4),
                allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
            });
            // Economic state deliberately uses sentinel values. The maintenance
            // executive must never call the session/staking/authorship hooks.
            pallet_staking::CurrentEra::<Runtime>::put(42);
            sp_io::storage::set(b":code", b"fixture-deployed-code");
        });
        ext
    }

    fn maintenance_extrinsics(action: RecoveryAction, slot: u64) -> Vec<UncheckedExtrinsic> {
        vec![
            UncheckedExtrinsic::new_bare_legacy(RuntimeCall::Timestamp(
                pallet_timestamp::Call::set {
                    now: slot * crate::Babe::slot_duration(),
                },
            )),
            UncheckedExtrinsic::new_bare_legacy(RuntimeCall::System(frame_system::Call::remark {
                remark: RecoveryInherent {
                    format_version: 1,
                    action,
                }
                .encode_remark(),
            })),
        ]
    }

    fn restored(mut ext: sp_io::TestExternalities) -> sp_io::TestExternalities {
        ext.commit_all().unwrap();
        let (raw, root) = ext.into_raw_snapshot();
        sp_io::TestExternalities::from_raw_snapshot(raw, root, sp_runtime::StateVersion::V0)
    }

    #[test]
    fn maintenance_blocks_replay_identically_and_resume_committed_work_after_restart() {
        let (policy, chunk, audit, terminal) = fixture();
        let mut author = maintenance_fixture(&policy);
        let mut importer = maintenance_fixture(&policy);
        let mut parent = sp_core::H256::repeat_byte(5);
        let epoch_duration = <Runtime as pallet_babe::Config>::EpochDuration::get();
        for (offset, action) in [
            RecoveryAction::Apply(chunk),
            RecoveryAction::Audit(audit),
            RecoveryAction::TerminalAudit(terminal),
        ]
        .into_iter()
        .enumerate()
        {
            // Cross an epoch during the frozen economic session.
            let slot = epoch_duration + 2 + offset as u64;
            let header = maintenance_header(CHECKPOINT_NUMBER + 2 + offset as u32, parent, slot);
            let extrinsics = maintenance_extrinsics(action, slot);
            let completed = author.execute_with(|| {
                with_test_policy(&policy, || {
                    assert_eq!(
                        initialize_block(&header),
                        ExtrinsicInclusionMode::OnlyInherents
                    );
                    for extrinsic in extrinsics.clone() {
                        apply_extrinsic(extrinsic).unwrap().unwrap();
                    }
                    let completed = finalize_block();
                    assert_eq!(crate::Session::current_index(), 7);
                    assert_eq!(pallet_staking::CurrentEra::<Runtime>::get(), Some(42));
                    assert_eq!(pallet_babe::EpochIndex::<Runtime>::get(), 1);
                    let bytes = sp_io::storage::get(pallet_babe::recovery::STORAGE_KEY).unwrap();
                    let epochs =
                        pallet_babe::recovery::RecoveryEpochs::decode_all(&mut bytes.as_ref())
                            .unwrap();
                    assert_eq!(epochs.session_for_epoch(1), Some(7));
                    assert!(recovery::TRANSIENT_CONTEXT_KEYS
                        .iter()
                        .all(|key| !sp_io::storage::exists(key)));
                    completed
                })
            });
            importer.execute_with(|| {
                with_test_policy(&policy, || {
                    execute_block(sp_runtime::generic::LazyBlock::new(
                        completed.clone(),
                        extrinsics,
                    ));
                    assert_eq!(
                        sp_io::storage::root(sp_runtime::StateVersion::V0),
                        completed.state_root().as_bytes()
                    );
                })
            });
            parent = completed.hash();
            // This rebuilds externalities from the persisted trie, dropping all
            // process-local state and open overlays between each recovery block.
            author = restored(author);
            importer = restored(importer);
        }
        author.execute_with(|| {
            let status = recovery::storage::status().unwrap().unwrap();
            assert_eq!(status.phase, RecoveryPhase::Ready);
            assert_eq!(status.next_chunk, 1);
            assert_eq!(
                sp_io::storage::get(b"fixture-issuance").unwrap().as_ref(),
                10u128.encode()
            );
            assert!(sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            assert!(!sp_io::storage::exists(ACTIVATION_TIME));
        });
    }

    #[test]
    fn committed_failure_finalizes_without_mutation_and_keeps_maintenance_live() {
        let (mut policy, mut chunk, _, _) = fixture();
        chunk.edits[0].before = Some([9; 32]);
        policy.chunk_hashes[0] = chunk.commitment();
        let mut ext = maintenance_fixture(&policy);
        ext.execute_with(|| {
            with_test_policy(&policy, || {
                let mut parent = sp_core::H256::repeat_byte(5);
                for (offset, action) in [RecoveryAction::Apply(chunk), RecoveryAction::Tick]
                    .into_iter()
                    .enumerate()
                {
                    let slot = 10 + offset as u64;
                    initialize_block(&maintenance_header(
                        CHECKPOINT_NUMBER + 2 + offset as u32,
                        parent,
                        slot,
                    ));
                    for extrinsic in maintenance_extrinsics(action, slot) {
                        apply_extrinsic(extrinsic).unwrap().unwrap();
                    }
                    parent = finalize_block().hash();
                    let status = recovery::storage::status().unwrap().unwrap();
                    assert_eq!(status.phase, RecoveryPhase::Failed);
                    assert_eq!(status.next_chunk, 0);
                    assert_eq!(status.failure, Some(RecoveryError::WrongPreimage));
                    assert!(!sp_io::storage::exists(b"fixture-issuance"));
                }
            })
        });
    }

    fn incoming_descriptor_failure_operation(
    ) -> crate::incident_recovery_incoming::IncomingOperation {
        use crate::incident_recovery_incoming::{
            IncomingChannel, IncomingOperation, NetworkAttackerRegistry,
        };
        use bridge_types::{
            substrate::{BridgeCall, BridgeMessage, SubstrateAppCall},
            GenericAccount, GenericAssetId, GenericBalance, GenericNetworkId, GenericTimepoint,
            SubNetworkId,
        };
        let payload = BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
            sender: GenericAccount::Liberland(sp_runtime::AccountId32::new([6; 32])),
            recipient: GenericAccount::Sora(sp_runtime::AccountId32::new([7; 32])),
            asset_id: GenericAssetId::Sora(common::XOR.into()),
            amount: GenericBalance::Substrate(1),
        })
        .encode();
        IncomingOperation {
            index: 0,
            channel: IncomingChannel::Substrate {
                network: SubNetworkId::Liberland,
                commitment: bridge_types::GenericCommitment::Sub(
                    bridge_types::substrate::Commitment {
                        nonce: 1,
                        messages: vec![BridgeMessage {
                            payload: payload.try_into().unwrap(),
                            timepoint: GenericTimepoint::Sora(CHECKPOINT_NUMBER),
                        }]
                        .try_into()
                        .unwrap(),
                    },
                ),
                proof: crate::MultiProof::Multisig(multisig_verifier::Proof {
                    digest: bridge_types::types::AuxiliaryDigest { logs: vec![] },
                    proof: vec![],
                }),
            },
            finalized_source_evidence: [3; 32],
            registries: vec![
                NetworkAttackerRegistry {
                    network: GenericNetworkId::Sub(SubNetworkId::Liberland),
                    accounts: vec![],
                    no_known_attackers: true,
                    attestation: [1; 32],
                },
                NetworkAttackerRegistry {
                    network: crate::ThisNetworkId::get(),
                    accounts: vec![GenericAccount::Sora(sp_runtime::AccountId32::new(
                        recovery::ATTACKER_ACCOUNT,
                    ))],
                    no_known_attackers: false,
                    attestation: [2; 32],
                },
            ],
        }
    }

    #[test]
    fn committed_failed_incoming_preparation_is_charged_and_keeps_maintenance_live() {
        let operation = incoming_descriptor_failure_operation();
        let (mut policy, _, _, _) = fixture();
        policy.chunk_hashes = vec![operation.commitment()];
        let mut ext = maintenance_fixture(&policy);
        ext.execute_with(|| {
            with_test_policy(&policy, || {
                let weight = operation.required_weight();
                assert_eq!(
                    committed_storage_weight(&RecoveryAction::Incoming(operation.encode())),
                    weight
                );
                assert!(
                    crate::incident_recovery_incoming::audit_targets(&operation, &policy).is_ok()
                );
                assert_eq!(
                    crate::incident_recovery_incoming::expected_requests(&operation, &policy),
                    Err(RecoveryError::WrongPreimage)
                );
                let mut parent = sp_core::H256::repeat_byte(5);
                for (offset, action) in [
                    RecoveryAction::Incoming(operation.encode()),
                    RecoveryAction::Tick,
                ]
                .into_iter()
                .enumerate()
                {
                    let slot = 10 + offset as u64;
                    initialize_block(&maintenance_header(
                        CHECKPOINT_NUMBER + 2 + offset as u32,
                        parent,
                        slot,
                    ));
                    for extrinsic in maintenance_extrinsics(action, slot) {
                        apply_extrinsic(extrinsic).unwrap().unwrap();
                    }
                    if offset == 0 {
                        assert!(frame_system::BlockWeight::<Runtime>::get()
                            .get(DispatchClass::Mandatory)
                            .all_gte(weight));
                    }
                    let status = recovery::storage::status().unwrap().unwrap();
                    assert_eq!(status.phase, RecoveryPhase::Failed);
                    assert_eq!(status.next_chunk, 0);
                    assert_eq!(status.failure, Some(RecoveryError::WrongPreimage));
                    assert_eq!(
                        substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(
                            bridge_types::SubNetworkId::Liberland
                        ),
                        0
                    );
                    parent = finalize_block().hash();
                }
            });
        });
    }

    #[test]
    fn already_failed_work_rejection_does_not_leak_storage_or_weight() {
        let operation = incoming_descriptor_failure_operation();
        let (mut policy, chunk, audit, terminal) = fixture();
        policy.chunk_hashes = vec![operation.commitment()];
        let malformed = RecoveryAction::Incoming(vec![1]);
        // Legacy dispatch weights may read runtime storage while computing DB weight.
        let mut weight_ext = maintenance_fixture(&policy);
        weight_ext.execute_with(|| {
            assert_eq!(committed_storage_weight(&malformed), Weight::MAX);
            assert!(checked_storage_weight(&malformed).is_err());
            assert_eq!(
                checked_storage_weight(&RecoveryAction::Incoming(operation.encode())).unwrap(),
                operation.required_weight()
            );
        });
        let actions = vec![
            malformed,
            RecoveryAction::Incoming(vec![1; recovery::MAX_CHUNK_BYTES + 1]),
            RecoveryAction::Incoming(operation.encode()),
            RecoveryAction::IncomingAudit(vec![1]),
            RecoveryAction::Outgoing(vec![1]),
            RecoveryAction::Apply(chunk),
            RecoveryAction::Audit(audit),
            RecoveryAction::PrefixAudit(recovery::RecoveryPrefixAuditChunk {
                index: 0,
                prefixes: vec![recovery::StoragePrefixWitness {
                    scope: recovery::StorageScope::Top,
                    prefix: b"fixture-prefix".to_vec(),
                }],
            }),
            RecoveryAction::TerminalAudit(terminal),
        ];
        for action in actions {
            let mut ext = maintenance_fixture(&policy);
            ext.execute_with(|| {
                with_test_policy(&policy, || {
                    let mut status = recovery::storage::status().unwrap().unwrap();
                    status.phase = RecoveryPhase::Failed;
                    status.failure = Some(RecoveryError::WrongPreimage);
                    sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
                    initialize_block(&maintenance_header(
                        CHECKPOINT_NUMBER + 2,
                        sp_core::H256::repeat_byte(5),
                        10,
                    ));
                    let mut extrinsics = maintenance_extrinsics(action, 10).into_iter();
                    apply_extrinsic(extrinsics.next().unwrap())
                        .unwrap()
                        .unwrap();
                    let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
                    let weight = frame_system::BlockWeight::<Runtime>::get();
                    assert!(apply_extrinsic(extrinsics.next().unwrap()).is_err());
                    assert_eq!(frame_system::BlockWeight::<Runtime>::get(), weight);
                    assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
                    // A rejected work envelope does not consume the one-work slot.
                    let tick = maintenance_extrinsics(RecoveryAction::Tick, 10)
                        .pop()
                        .unwrap();
                    apply_extrinsic(tick).unwrap().unwrap();
                    assert_eq!(recovery::storage::status().unwrap().unwrap(), status);
                    finalize_block();
                })
            });
        }
    }

    #[test]
    fn incoming_mandatory_weight_rejects_decoded_unbounded_native_call() {
        use crate::incident_recovery_incoming::{HashiPeerApproval, IncomingChannel};
        let mut operation = incoming_descriptor_failure_operation();
        operation.channel = IncomingChannel::Hashi {
            network: 0,
            request: vec![1],
            approvals: vec![HashiPeerApproval {
                signer: sp_runtime::AccountId32::new([7; 32]),
                call: vec![1],
                extra: vec![],
                signature: sp_core::sr25519::Signature::from_raw([0; 64]).into(),
            }],
        };
        let action = RecoveryAction::Incoming(operation.encode());
        assert_eq!(
            crate::incident_recovery_incoming::incoming_encoded_weight(&operation.encode()),
            Ok(Weight::MAX)
        );
        assert_eq!(
            checked_storage_weight(&action),
            Err(RecoveryError::ChunkBounds)
        );
    }

    #[test]
    fn committed_failed_outgoing_is_charged_without_quarantine_writes() {
        use crate::incident_recovery_outgoing::{OutgoingOperation, OutgoingTarget};
        let operation = OutgoingOperation {
            index: 0,
            target: OutgoingTarget::Hashi {
                network: 0,
                request: sp_core::H256::repeat_byte(71),
                request_hash: [72; 32],
                status_hash: [73; 32],
                author: sp_runtime::AccountId32::new([7; 32]),
                transfer: None,
            },
            reconciliation_evidence: [74; 32],
        };
        let (mut policy, _, _, _) = fixture();
        policy.chunk_hashes = vec![operation.commitment()];
        let mut ext = maintenance_fixture(&policy);
        ext.execute_with(|| {
            with_test_policy(&policy, || {
                initialize_block(&maintenance_header(
                    CHECKPOINT_NUMBER + 2,
                    sp_core::H256::repeat_byte(5),
                    10,
                ));
                for extrinsic in
                    maintenance_extrinsics(RecoveryAction::Outgoing(operation.encode()), 10)
                {
                    apply_extrinsic(extrinsic).unwrap().unwrap();
                }
                assert!(frame_system::BlockWeight::<Runtime>::get()
                    .get(DispatchClass::Mandatory)
                    .all_gte(operation.required_weight()));
                let status = recovery::storage::status().unwrap().unwrap();
                assert_eq!(status.phase, RecoveryPhase::Failed);
                assert_eq!(status.next_chunk, 0);
                assert_eq!(status.failure, Some(RecoveryError::WrongPreimage));
                assert!(eth_bridge::Requests::<Runtime>::iter().next().is_none());
                assert!(!sp_io::storage::exists(&bridge_types::recovery::hashi_key(
                    0,
                    sp_core::H256::repeat_byte(71)
                )));
                finalize_block();
            })
        });
    }

    #[test]
    fn recovery_envelope_weight_includes_already_recorded_work_and_both_dimensions() {
        for excess in [
            Weight::zero(),
            Weight::from_parts(1, 0),
            Weight::from_parts(0, 1),
        ] {
            let operation = incoming_descriptor_failure_operation();
            let (mut policy, _, _, _) = fixture();
            policy.chunk_hashes = vec![operation.commitment()];
            let mut ext = maintenance_fixture(&policy);
            ext.execute_with(|| {
                with_test_policy(&policy, || {
                    initialize_block(&maintenance_header(
                        CHECKPOINT_NUMBER + 2,
                        sp_core::H256::repeat_byte(5),
                        10,
                    ));
                    let mut extrinsics =
                        maintenance_extrinsics(RecoveryAction::Incoming(operation.encode()), 10)
                            .into_iter();
                    apply_extrinsic(extrinsics.next().unwrap())
                        .unwrap()
                        .unwrap();
                    let full = Weight::from_parts(500_000_000, recovery::MAX_INHERENT_BYTES as u64)
                        .saturating_add(operation.required_weight());
                    let max = <Runtime as frame_system::Config>::BlockWeights::get().max_block;
                    let used = max.checked_sub(&full).unwrap().saturating_add(excess);
                    frame_system::BlockWeight::<Runtime>::put(
                        frame_support::dispatch::PerDispatchClass::new(|class| {
                            if class == DispatchClass::Mandatory {
                                used
                            } else {
                                Weight::zero()
                            }
                        }),
                    );
                    let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
                    let recorded = frame_system::BlockWeight::<Runtime>::get();
                    let result = apply_extrinsic(extrinsics.next().unwrap());
                    if excess == Weight::zero() {
                        result.unwrap().unwrap();
                        assert_eq!(frame_system::BlockWeight::<Runtime>::get().total(), max);
                        assert_eq!(
                            recovery::storage::status().unwrap().unwrap().phase,
                            RecoveryPhase::Failed
                        );
                    } else {
                        assert_eq!(result, Err(InvalidTransaction::ExhaustsResources.into()));
                        assert_eq!(frame_system::BlockWeight::<Runtime>::get(), recorded);
                        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
                        assert_eq!(
                            recovery::storage::status().unwrap().unwrap().phase,
                            RecoveryPhase::Applying
                        );
                    }
                })
            });
        }
    }

    #[test]
    fn committed_failed_incoming_audit_is_charged_and_keeps_escape_available() {
        let (mut policy, chunk, _, _) = fixture();
        let audit = crate::incident_recovery_incoming::IncomingAudit {
            index: 0,
            operation_index: chunk.index,
            operation_commitment: chunk.commitment(),
            requests: vec![None],
            prior_policy: None,
        };
        policy.audit_chunk_hashes = vec![audit.commitment()];
        let mut ext = maintenance_fixture(&policy);
        ext.execute_with(|| {
            with_test_policy(&policy, || {
                let mut parent = sp_core::H256::repeat_byte(5);
                for (offset, action) in [
                    RecoveryAction::Apply(chunk),
                    RecoveryAction::IncomingAudit(audit.encode()),
                ]
                .into_iter()
                .enumerate()
                {
                    let slot = 10 + offset as u64;
                    initialize_block(&maintenance_header(
                        CHECKPOINT_NUMBER + 2 + offset as u32,
                        parent,
                        slot,
                    ));
                    for extrinsic in maintenance_extrinsics(action, slot) {
                        apply_extrinsic(extrinsic).unwrap().unwrap();
                    }
                    if offset == 1 {
                        let recorded = frame_system::BlockWeight::<Runtime>::get();
                        assert!(recorded
                            .get(DispatchClass::Mandatory)
                            .all_gte(crate::incident_recovery_incoming::incoming_audit_weight()));
                        let state = recovery::storage::status().unwrap().unwrap();
                        assert_eq!(state.phase, RecoveryPhase::Failed);
                        assert_eq!(state.failure, Some(RecoveryError::IncompleteAudit));
                    }
                    parent = finalize_block().hash();
                }
            })
        });
    }

    #[test]
    fn staging_rejects_corrupted_lengths_and_nonfinal_short_chunks_before_assembly() {
        sp_io::TestExternalities::default().execute_with(|| {
            let cursor = stage_key(RecoveryCodeKind::Repair, None);
            let zero = stage_key(RecoveryCodeKind::Repair, Some(0));
            sp_io::storage::set(&cursor, &[0; 5]);
            assert_eq!(
                stage(RecoveryCodeKind::Repair, 0, b"x"),
                Err(RecoveryError::ChunkBounds)
            );
            assert_eq!(
                assembled(RecoveryCodeKind::Repair, Some([1; 32])),
                Err(RecoveryError::ChunkBounds)
            );
            sp_io::storage::set(&cursor, &513u32.encode());
            assert_eq!(
                assembled(RecoveryCodeKind::Repair, Some([1; 32])),
                Err(RecoveryError::ChunkBounds)
            );
            sp_io::storage::set(&cursor, &1u32.encode());
            sp_io::storage::set(&zero, &vec![0; recovery::MAX_CODE_CHUNK_BYTES + 1]);
            assert_eq!(
                assembled(RecoveryCodeKind::Repair, Some([1; 32])),
                Err(RecoveryError::ChunkBounds)
            );
            sp_io::storage::set(&zero, b"short");
            sp_io::storage::set(&cursor, &2u32.encode());
            sp_io::storage::set(&stage_key(RecoveryCodeKind::Repair, Some(1)), b"tail");
            assert_eq!(
                assembled(RecoveryCodeKind::Repair, Some([1; 32])),
                Err(RecoveryError::ChunkBounds)
            );
            sp_io::storage::clear(&cursor);
            let first = vec![1; recovery::MAX_CODE_CHUNK_BYTES];
            stage(RecoveryCodeKind::Repair, 0, &first).unwrap();
            stage(RecoveryCodeKind::Repair, 1, b"tail").unwrap();
            let mut expected = first;
            expected.extend_from_slice(b"tail");
            assert_eq!(
                assembled(
                    RecoveryCodeKind::Repair,
                    Some(sp_io::hashing::blake2_256(&expected))
                )
                .unwrap(),
                expected
            );
        });
    }

    #[test]
    fn every_installable_runtime_requires_its_reserved_spec_and_transaction_pair() {
        let mut version = crate::VERSION;
        for (spec, transaction) in [(135, 132), (136, 132), (137, 132), (138, 134)] {
            version.spec_version = spec;
            version.transaction_version = transaction;
            assert_eq!(validate_runtime_version(&version, spec), Ok(()));
            version.transaction_version += 1;
            assert_eq!(
                validate_runtime_version(&version, spec),
                Err(RecoveryError::BadCode)
            );
        }
        version.spec_version = 134;
        version.transaction_version = 131;
        assert_eq!(
            validate_runtime_version(&version, 134),
            Err(RecoveryError::BadCode)
        );
    }

    /// Native state-machine test only: the host recognizes SCALE version
    /// fixtures. This is deliberately not Wasm execution or artifact evidence.
    struct FixtureVersions;
    impl sp_core::traits::ReadRuntimeVersion for FixtureVersions {
        fn read_runtime_version(
            &self,
            code: &[u8],
            _ext: &mut dyn sp_externalities::Externalities,
        ) -> Result<Vec<u8>, String> {
            sp_version::RuntimeVersion::decode_all(&mut &code[..])
                .map(|version| version.encode())
                .map_err(|_| "not a native version fixture".into())
        }
    }

    fn version_fixture(spec: u32, transaction: u32) -> Vec<u8> {
        let mut version = crate::VERSION;
        version.spec_version = spec;
        version.transaction_version = transaction;
        version.encode()
    }

    #[test]
    fn failed_chunk_can_escape_then_repair_forward_without_reusing_quorum_authorization() {
        use sp_core::Pair;
        let pairs = [21u8, 22, 23].map(|seed| sp_core::ed25519::Pair::from_seed(&[seed; 32]));
        let (mut original, mut broken, audit, terminal) = fixture();
        original.council = pairs
            .iter()
            .map(|pair| sp_runtime::AccountId32::new(pair.public().0))
            .collect();
        original.council.sort();
        broken.edits[0].before = Some([9; 32]);
        original.chunk_hashes[0] = broken.commitment();
        let hold = version_fixture(135, 132);
        let hold_hash = sp_io::hashing::blake2_256(&hold);
        let repair = version_fixture(136, 132);
        let mut ext = maintenance_fixture(&original);
        ext.register_extension(sp_core::traits::ReadRuntimeVersionExt::new(FixtureVersions));
        ext.execute_with(|| {
            with_test_policy(&original, || {
                apply_action(&original, RecoveryAction::Apply(broken.clone())).unwrap();
                let state = recovery::storage::status().unwrap().unwrap();
                assert_eq!(state.phase, RecoveryPhase::Failed);
                // A poisoned or absent staging area cannot prevent the embedded hold escape.
                sp_io::storage::set(&stage_key(RecoveryCodeKind::Repair, None), &[0; 5]);
                let authorization = recovery::RepairAuthorization {
                    policy: original.commitment(),
                    phase: state.phase,
                    cursor: state.next_chunk,
                    operation_counter: state.operation_counter,
                    current_code_hash: sp_io::hashing::blake2_256(b"fixture-deployed-code"),
                    current_executor_hash: [77; 32],
                    destination_code_hash: hold_hash,
                    destination_spec: 135,
                    revised_policy: [0; 32],
                };
                let sign = |authorization: &recovery::RepairAuthorization| {
                    pairs
                        .iter()
                        .take(2)
                        .map(|pair| {
                            (
                                sp_runtime::AccountId32::new(pair.public().0),
                                sp_runtime::MultiSignature::Ed25519(
                                    pair.sign(&authorization.signing_message()),
                                ),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let signatures = sign(&authorization);
                let action = |signatures| RecoveryAction::Escape {
                    authorization: alloc::boxed::Box::new(authorization.clone()),
                    signatures,
                    work_policy: None,
                };
                let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert_eq!(
                    apply_action_with_artifacts(
                        &original,
                        action(signatures[..1].to_vec()),
                        134,
                        &hold,
                        hold_hash,
                        hold.len() as u32
                    ),
                    Err(RecoveryError::BadQuorum)
                );
                assert_eq!(
                    apply_action_with_artifacts(
                        &original,
                        action(vec![signatures[0].clone(); 2]),
                        134,
                        &hold,
                        hold_hash,
                        hold.len() as u32
                    ),
                    Err(RecoveryError::BadQuorum)
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
                apply_action_with_artifacts(
                    &original,
                    action(signatures.clone()),
                    134,
                    &hold,
                    hold_hash,
                    hold.len() as u32,
                )
                .unwrap();
                let state = recovery::storage::status().unwrap().unwrap();
                assert_eq!(
                    (state.phase, state.next_chunk, state.operation_counter),
                    (RecoveryPhase::Hold, 0, 1)
                );
                assert_eq!(sp_io::storage::get(b":code").unwrap().as_ref(), hold);
                let held_root = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert!(apply_action_with_artifacts(
                    &original,
                    action(signatures),
                    134,
                    &hold,
                    hold_hash,
                    hold.len() as u32
                )
                .is_err());
                assert_eq!(
                    sp_io::storage::root(sp_runtime::StateVersion::V0),
                    held_root
                );

                let mut revised = original.clone();
                broken.edits[0].before = None;
                revised.chunk_hashes[0] = broken.commitment();
                revised.evidence_commitment = [42; 32];
                let forward = recovery::RepairAuthorization {
                    policy: original.commitment(),
                    phase: state.phase,
                    cursor: state.next_chunk,
                    operation_counter: state.operation_counter,
                    current_code_hash: hold_hash,
                    current_executor_hash: hold_hash,
                    destination_code_hash: sp_io::hashing::blake2_256(&repair),
                    destination_spec: 136,
                    revised_policy: revised.commitment(),
                };
                sp_io::storage::clear(&stage_key(RecoveryCodeKind::Repair, None));
                stage(RecoveryCodeKind::Repair, 0, &repair).unwrap();
                let repair_action = RecoveryAction::Escape {
                    authorization: alloc::boxed::Box::new(forward.clone()),
                    signatures: sign(&forward),
                    work_policy: Some(revised.clone()),
                };
                apply_action_with_artifacts(&original, repair_action.clone(), 135, &[], [0; 32], 0)
                    .unwrap();
                let state = recovery::storage::status().unwrap().unwrap();
                assert_eq!(
                    (state.phase, state.next_chunk, state.operation_counter),
                    (RecoveryPhase::Applying, 0, 2)
                );
                assert_eq!(sp_io::storage::get(b":code").unwrap().as_ref(), repair);
                assert_eq!(
                    sp_io::storage::get(recovery::POLICY_STORAGE_KEY)
                        .unwrap()
                        .as_ref(),
                    original.commitment()
                );
                assert_eq!(
                    sp_io::storage::get(recovery::ACTIVE_POLICY_STORAGE_KEY)
                        .unwrap()
                        .as_ref(),
                    revised.commitment()
                );
                assert!(sp_io::storage::exists(REPAIR_AUTH));
                assert!(accept_forward_repair_at(&revised, 137).is_err());
                assert!(sp_io::storage::exists(REPAIR_AUTH));
                accept_forward_repair_at(&revised, 136).unwrap();
                assert!(!sp_io::storage::exists(REPAIR_AUTH));
                let repaired_root = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert!(apply_action_with_artifacts(
                    &original,
                    repair_action,
                    136,
                    &[],
                    [0; 32],
                    0
                )
                .is_err());
                assert_eq!(
                    sp_io::storage::root(sp_runtime::StateVersion::V0),
                    repaired_root
                );
                apply_action(&revised, RecoveryAction::Apply(broken)).unwrap();
                apply_action(&revised, RecoveryAction::Audit(audit)).unwrap();
                apply_action(&revised, RecoveryAction::TerminalAudit(terminal)).unwrap();
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Ready
                );
                assert!(sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            })
        });
    }

    #[test]
    fn automatic_activation_requires_all_audits_code_consensus_and_fee_clock_checks() {
        let (policy, chunk, audit, terminal) = fixture();
        let final_code = version_fixture(138, 134);
        let final_hash = sp_io::hashing::blake2_256(&final_code);
        let mut ext = maintenance_fixture(&policy);
        ext.register_extension(sp_core::traits::ReadRuntimeVersionExt::new(FixtureVersions));
        ext.execute_with(|| {
            with_test_policy(&policy, || {
                let operator =
                    crate::incident_recovery_consensus::tests::pending_key_fixture(&policy);
                crate::incident_recovery_consensus::install(
                    policy.commitment(),
                    7,
                    core::slice::from_ref(&operator),
                )
                .unwrap();
                crate::incident_recovery_consensus::initialize_maintenance_epoch().unwrap();
                let operators = vec![operator].encode();
                pallet_timestamp::Now::<Runtime>::put(400_000u64);
                sp_io::storage::set(CHECKPOINT_TIME, &300_000u64.encode());
                let activate = || {
                    automatic_activation_with_artifacts(
                        final_hash,
                        final_code.len() as u32,
                        &operators,
                    )
                };
                activate().unwrap();
                assert!(!sp_io::storage::exists(ACTIVATION_TIME));
                apply_action(&policy, RecoveryAction::Apply(chunk)).unwrap();
                apply_action(&policy, RecoveryAction::Audit(audit)).unwrap();
                activate().unwrap();
                assert!(!sp_io::storage::exists(ACTIVATION_TIME));
                apply_action(&policy, RecoveryAction::TerminalAudit(terminal)).unwrap();
                activate().unwrap();
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Ready
                );
                assert!(!sp_io::storage::exists(ACTIVATION_TIME));
                let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert_eq!(
                    stage_with_commitments(
                        RecoveryCodeKind::Final,
                        0,
                        b"wrong",
                        Some(&[final_hash])
                    ),
                    Err(RecoveryError::WrongChunk)
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
                stage_with_commitments(
                    RecoveryCodeKind::Final,
                    0,
                    &final_code,
                    Some(&[final_hash]),
                )
                .unwrap();

                sp_io::storage::set(CHECKPOINT_TIME, &500_000u64.encode());
                let invalid_clock = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert_eq!(activate(), Err(RecoveryError::WrongPreimage));
                assert_eq!(
                    sp_io::storage::root(sp_runtime::StateVersion::V0),
                    invalid_clock
                );
                sp_io::storage::set(CHECKPOINT_TIME, &300_000u64.encode());
                pallet_grandpa::CurrentSetId::<Runtime>::put(99);
                let invalid_consensus = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert_eq!(activate(), Err(RecoveryError::IncompleteAudit));
                assert_eq!(
                    sp_io::storage::root(sp_runtime::StateVersion::V0),
                    invalid_consensus
                );
                pallet_grandpa::CurrentSetId::<Runtime>::put(8);
                activate().unwrap();
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Complete
                );
                assert_eq!(sp_io::storage::get(b":code").unwrap().as_ref(), final_code);
                assert_eq!(
                    sp_io::storage::get(ACTIVATION_TIME).unwrap().as_ref(),
                    400_000u64.encode()
                );
                assert!(sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
                let completed = sp_io::storage::root(sp_runtime::StateVersion::V0);
                activate().unwrap();
                assert_eq!(
                    sp_io::storage::root(sp_runtime::StateVersion::V0),
                    completed
                );
            })
        });
    }

    #[test]
    fn final_handoff_records_complete_and_code_atomically() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, chunk, audit, terminal) = fixture();
            recovery::storage::initialize(&policy).unwrap();
            let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
            assert_eq!(
                complete_and_install(&policy, b"fixture-code"),
                Err(RecoveryError::IncompleteAudit)
            );
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
            recovery::storage::apply_chunk(&policy, &chunk).unwrap();
            recovery::storage::audit_chunk(&policy, &audit).unwrap();
            recovery::invariants::verify_and_record(&policy, &terminal).unwrap();
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Ready
            );
            complete_and_install(&policy, b"fixture-code").unwrap();
            assert_eq!(
                recovery::storage::status().unwrap().unwrap().phase,
                RecoveryPhase::Complete
            );
            assert_eq!(
                sp_io::storage::get(b":code").unwrap().as_ref(),
                b"fixture-code"
            );
            assert!(sp_io::storage::exists(ACTIVATION_TIME));
            assert_eq!(
                System::digest().logs().last(),
                Some(&DigestItem::RuntimeEnvironmentUpdated)
            );
        });
    }

    #[test]
    fn maintenance_rejects_ordinary_calls_and_wrong_inherent_order_without_writes() {
        sp_io::TestExternalities::default().execute_with(|| {
            System::set_block_number(CHECKPOINT_NUMBER + 2);
            let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
            let ordinary = UncheckedExtrinsic::new_bare_legacy(RuntimeCall::System(
                frame_system::Call::remark {
                    remark: b"ordinary".to_vec(),
                },
            ));
            assert!(apply_extrinsic(ordinary).is_err());
            let tick = RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::Tick,
            }
            .encode_remark();
            let out_of_order = UncheckedExtrinsic::new_bare_legacy(RuntimeCall::System(
                frame_system::Call::remark { remark: tick },
            ));
            assert!(apply_extrinsic(out_of_order).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
        });
    }

    #[test]
    fn authoring_and_import_require_exactly_timestamp_then_recovery() {
        sp_io::TestExternalities::default().execute_with(|| {
            let header = Header::new(
                CHECKPOINT_NUMBER + 1,
                Default::default(),
                Default::default(),
                CHECKPOINT_HASH.into(),
                Digest::default(),
            );
            let timestamp = UncheckedExtrinsic::new_bare_legacy(RuntimeCall::Timestamp(
                pallet_timestamp::Call::set { now: 1_000_000 },
            ));
            let envelope = RecoveryInherent {
                format_version: 1,
                action: RecoveryAction::Tick,
            }
            .encode_remark();
            let work = UncheckedExtrinsic::new_bare_legacy(RuntimeCall::System(
                frame_system::Call::remark { remark: envelope },
            ));
            let mut data = sp_inherents::InherentData::new();
            data.put_data(
                <Timestamp as ProvideInherent>::INHERENT_IDENTIFIER,
                &1_000_000u64,
            )
            .unwrap();
            let valid = sp_runtime::generic::LazyBlock::new(
                header.clone(),
                vec![timestamp.clone(), work.clone()],
            );
            assert!(check_inherents(valid, data.clone()).ok());
            let extra = sp_runtime::generic::LazyBlock::new(
                header.clone(),
                vec![timestamp.clone(), work.clone(), work.clone()],
            );
            assert!(check_inherents(extra, data.clone()).fatal_error());
            let reversed = sp_runtime::generic::LazyBlock::new(header, vec![work, timestamp]);
            assert!(check_inherents(reversed, data).fatal_error());
        });
    }

    #[test]
    fn unbound_bootstrap_cannot_start_recovery() {
        assert_eq!(compiled_policy(), Err(RecoveryError::MissingCompiledPolicy));
    }
    #[test]
    fn uncommitted_final_and_hold_code_cannot_poison_escape_staging() {
        sp_io::TestExternalities::default().execute_with(|| {
            assert!(stage(RecoveryCodeKind::Final, 0, b"unapproved").is_err());
            assert!(stage(RecoveryCodeKind::Hold, 0, b"unapproved").is_err());
            assert!(!sp_io::storage::exists(&stage_key(
                RecoveryCodeKind::Hold,
                None
            )));
        });
    }
    #[test]
    fn repair_staging_is_bounded_and_can_replace_a_bad_untrusted_chunk() {
        sp_io::TestExternalities::default().execute_with(|| {
            assert!(stage(RecoveryCodeKind::Repair, 1, b"gap").is_err());
            stage(RecoveryCodeKind::Repair, 0, b"wrong").unwrap();
            stage(RecoveryCodeKind::Repair, 0, b"correct").unwrap();
            assert_eq!(
                sp_io::storage::get(&stage_key(RecoveryCodeKind::Repair, Some(0)))
                    .unwrap()
                    .as_ref(),
                b"correct"
            );
            assert!(assembled(RecoveryCodeKind::Repair, Some([0; 32])).is_err());
            assert!(stage(
                RecoveryCodeKind::Repair,
                0,
                &vec![0; recovery::MAX_CODE_CHUNK_BYTES + 1]
            )
            .is_err());
        });
    }
}

/// Maintenance runtimes have no ordinary delivery phase after the checkpoint.
/// Preserve historical no-policy execution, and fail closed if ordinary ingress
/// is accidentally attempted under a recovery policy. Native shared tests use
/// only their existing explicit public fixture policy; no release override exists.
pub fn replacement(account: &crate::AccountId) -> Result<Option<crate::AccountId>, RecoveryError> {
    if !sp_io::storage::exists(recovery::POLICY_STORAGE_KEY) {
        return Ok(None);
    }
    #[cfg(test)]
    if let Some(policy) = TEST_POLICY.with(|cell| cell.borrow().clone()) {
        if sp_io::storage::get(recovery::POLICY_STORAGE_KEY).as_deref()
            != Some(policy.commitment().as_slice())
        {
            return Err(RecoveryError::WrongPolicy);
        }
        return Ok(policy
            .replacements
            .iter()
            .find(|entry| &entry.old == account)
            .map(|entry| entry.new.clone()));
    }
    let _ = account;
    Err(RecoveryError::WrongPhase)
}

#[cfg(test)]
mod ordinary_incoming_admission_tests {
    use super::*;

    #[test]
    fn production_legacy_mapping_path_preserves_history_and_rejects_recovery_delivery() {
        sp_io::TestExternalities::default().execute_with(|| {
            let account = crate::AccountId::new([7; 32]);
            assert_eq!(replacement(&account), Ok(None));
            sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &[1; 32]);
            // No public test policy is installed: this exercises the same
            // fail-closed branch present in the production legacy runtime.
            assert_eq!(replacement(&account), Err(RecoveryError::WrongPhase));
        });
    }
}
