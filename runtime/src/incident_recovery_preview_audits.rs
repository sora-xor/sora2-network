//! cfg(test)-only ordered audits for diagnostic native public fixture exporters.
use codec::Encode;
use common::recovery::{self, RecoveryAction, RecoveryInherent, RecoveryPolicy};

pub fn run(policy: &RecoveryPolicy, envelopes: &[Vec<u8>]) -> Result<(), String> {
    if envelopes.is_empty() {
        // Discovery has no complete policy audit stream yet; it is not an audit pass.
        return Ok(());
    }
    if envelopes.len() != policy.audit_chunk_hashes.len()
        || envelopes.len() > recovery::MAX_WORK_CHUNKS
    {
        return Err("preview ordered audit count differs".into());
    }
    let mut total = 0usize;
    let mut actions = Vec::with_capacity(envelopes.len());
    for bytes in envelopes {
        total = total
            .checked_add(bytes.len())
            .ok_or("preview audit bytes overflow")?;
        // A public fixture exporter must not retain an unbounded decoded stream.
        if total > 32 * 1024 * 1024 {
            return Err("preview audit byte bound".into());
        }
        let envelope = RecoveryInherent::decode_remark(bytes)
            .map_err(|e| format!("preview audit decode {e:?}"))?;
        if !matches!(
            &envelope.action,
            RecoveryAction::Audit(_)
                | RecoveryAction::PrefixAudit(_)
                | RecoveryAction::IncomingAudit(_)
        ) {
            return Err("preview accepts only ordered audit actions".into());
        }
        actions.push(envelope.action);
    }
    for action in actions {
        let next = crate::System::block_number()
            .checked_add(1)
            .ok_or("preview block overflow")?;
        crate::System::set_block_number(next);
        match action {
            RecoveryAction::Audit(audit) => {
                recovery::storage::audit_chunk(policy, &audit).map(|_| ())
            }
            RecoveryAction::PrefixAudit(audit) => {
                recovery::storage::audit_prefix_chunk(policy, &audit).map(|_| ())
            }
            RecoveryAction::IncomingAudit(payload) => {
                crate::incident_recovery_incoming::audit_encoded_with_policy(policy, &payload)
                    .map(|_| ())
            }
            _ => unreachable!("prechecked audit action"),
        }
        .map_err(|e| format!("preview ordered audit {e:?}"))?;
    }
    if sp_io::storage::get(recovery::AUDIT_CURSOR_STORAGE_KEY).as_deref()
        != Some((envelopes.len() as u32).encode().as_slice())
        || sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY).as_deref()
            != Some(0u32.encode().as_slice())
    {
        return Err("preview audit completion/pending count differs".into());
    }
    Ok(())
}

#[test]
fn preview_audit_codec_rejects_forbidden_trailing_and_mismatched_streams() {
    use common::recovery::AccountReplacement;
    let policy = RecoveryPolicy {
        format_version: 1,
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![AccountReplacement {
            old: sp_core::crypto::AccountId32::new([1; 32]),
            new: sp_core::crypto::AccountId32::new([2; 32]),
        }],
        council: vec![sp_core::crypto::AccountId32::new([3; 32])],
        chunk_hashes: vec![[4; 32]],
        audit_chunk_hashes: vec![[5; 32]],
        evidence_commitment: [6; 32],
        terminal_audit_commitment: [7; 32],
    };
    assert!(run(&policy, &[]).is_ok());
    let forbidden = RecoveryInherent {
        format_version: 1,
        action: RecoveryAction::Tick,
    }
    .encode_remark();
    assert_eq!(
        run(&policy, std::slice::from_ref(&forbidden)).unwrap_err(),
        "preview accepts only ordered audit actions"
    );
    let mut trailing = forbidden;
    trailing.push(0);
    assert!(run(&policy, &[trailing])
        .unwrap_err()
        .starts_with("preview audit decode"));
    assert_eq!(
        run(&policy, &[vec![], vec![]]).unwrap_err(),
        "preview ordered audit count differs"
    );
}
