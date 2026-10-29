//! Public, deterministic test-council signatures for the separate escape fixture.
//! No keystore, production authority or externally supplied secret is accepted.

use crate::{replay, Result};
use codec::DecodeWithMemLimit;
use common::recovery::{
    self, RecoveryAction, RecoveryInherent, RecoveryPolicy, RecoveryStatus, RepairAuthorization,
};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use sp_core::{ed25519, Pair};
use sp_runtime::{AccountId32, MultiSignature};
use std::path::Path;

pub const SCHEMA: &str = "sora-nexus-public-fixture-escape-replay-v1";
pub const SCHEME: &str = "sora-nexus/PUBLIC-FIXTURE-ONLY/council/v1/";
pub const PROBE_KEY: &[u8] = b":sora-nexus:public-fixture:escape-probe:v1";
pub const PROBE_VALUE: &[u8] = b"verified-forward-repair-probe";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Council {
    pub scheme: String,
    pub provenance: replay::Artifact,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Escape {
    pub destination_spec: u32,
    pub work_policy: Option<replay::Artifact>,
}

pub fn pairs() -> Vec<(AccountId32, ed25519::Pair)> {
    let mut pairs: Vec<_> = (0..3)
        .map(|index| {
            let seed: [u8; 32] = Sha256::digest(format!("{SCHEME}{index}").as_bytes()).into();
            let pair = ed25519::Pair::from_seed(&seed);
            (AccountId32::from(pair.public().0), pair)
        })
        .collect();
    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    pairs
}

pub fn validate(base: &Path, council: &Council, policy: &RecoveryPolicy) -> Result<()> {
    if council.scheme != SCHEME || !policy.replacements.is_empty() {
        return Err(
            "escape fixture accepts only its fixed public test council and no victim mapping"
                .into(),
        );
    }
    let expected: Vec<_> = pairs().into_iter().map(|(account, _)| account).collect();
    if policy.council != expected {
        return Err("escape policy council does not match the public fixture keys".into());
    }
    let evidence = replay::artifact(base, &council.provenance, 64 * 1024)?;
    let evidence: serde_json::Value =
        serde_json::from_slice(&evidence).map_err(|error| error.to_string())?;
    let public: Vec<_> = expected
        .iter()
        .map(|account| format!("0x{}", hex::encode(AsRef::<[u8]>::as_ref(account))))
        .collect();
    if evidence["schema"] != "sora-nexus-public-fixture-escape-inputs-v1"
        || evidence["purpose"] != "escape-only"
        || evidence["fixtureOnly"] != true
        || evidence["productionReady"] != false
        || evidence["automaticActivationAllowed"] != false
        || evidence["governanceStateReplaced"] != false
        || evidence["councilScheme"] != SCHEME
        || evidence["publicCouncil"] != serde_json::json!(public)
    {
        return Err("missing conspicuous escape-only fixture provenance".into());
    }
    Ok(())
}

fn signed(
    authorization: RepairAuthorization,
    work_policy: Option<RecoveryPolicy>,
) -> RecoveryInherent {
    let signatures = pairs()
        .into_iter()
        .take(2)
        .map(|(account, pair)| {
            (
                account,
                MultiSignature::Ed25519(pair.sign(&authorization.signing_message())),
            )
        })
        .collect();
    RecoveryInherent {
        format_version: 1,
        action: RecoveryAction::Escape {
            authorization: Box::new(authorization),
            signatures,
            work_policy,
        },
    }
}

pub struct Prepared {
    pub envelope: RecoveryInherent,
    pub negative: Vec<(String, RecoveryInherent)>,
}

#[allow(clippy::too_many_arguments)]
pub fn prepare(
    base: &Path,
    request: &Escape,
    original: &RecoveryPolicy,
    current: &RecoveryStatus,
    expected_anchor: [u8; 32],
    stored_code: &[u8],
    executing_code: &[u8],
    destination_code: &[u8],
) -> Result<Prepared> {
    if expected_anchor == [0; 32] {
        return Err("escape requires an established first recovery child".into());
    }
    let revised = if let Some(reference) = &request.work_policy {
        let bytes = replay::artifact(base, reference, recovery::MAX_INHERENT_BYTES)?;
        let mut input = bytes.as_slice();
        let revised =
            RecoveryPolicy::decode_with_mem_limit(&mut input, recovery::MAX_INHERENT_BYTES * 3)
                .map_err(|error| error.to_string())?;
        revised
            .validate()
            .map_err(|error| format!("invalid fixture repair policy: {error:?}"))?;
        if !input.is_empty()
            || revised.authorization_domain != original.authorization_domain
            || revised.council != original.council
            || revised.replacements != original.replacements
        {
            return Err("fixture forward repair changes the original council or ownership".into());
        }
        Some(revised)
    } else {
        None
    };
    if (request.destination_spec == 135) != revised.is_none()
        || !matches!(request.destination_spec, 135 | 136)
    {
        return Err("escape-only fixture supports the pinned hold then one 136 repair".into());
    }
    let authorization = RepairAuthorization {
        recovery_anchor: expected_anchor,
        authorization_domain: original.authorization_domain.clone(),
        policy: original.commitment(),
        phase: current.phase,
        cursor: current.next_chunk,
        operation_counter: current.operation_counter,
        current_code_hash: sp_io::hashing::blake2_256(stored_code),
        current_executor_hash: sp_io::hashing::blake2_256(executing_code),
        destination_code_hash: sp_io::hashing::blake2_256(destination_code),
        destination_spec: request.destination_spec,
        revised_policy: revised
            .as_ref()
            .map(RecoveryPolicy::commitment)
            .unwrap_or([0; 32]),
    };
    let envelope = signed(authorization.clone(), revised.clone());
    let mut negative = Vec::new();
    for (name, mutate) in [
        (
            "wrong-recovery-anchor",
            (|a: &mut RepairAuthorization| a.recovery_anchor = [0; 32])
                as fn(&mut RepairAuthorization),
        ),
        (
            "wrong-policy",
            (|a: &mut RepairAuthorization| a.policy = [0; 32]) as fn(&mut RepairAuthorization),
        ),
        ("wrong-phase", |a| {
            a.phase = if a.phase == recovery::RecoveryPhase::Hold {
                recovery::RecoveryPhase::Failed
            } else {
                recovery::RecoveryPhase::Hold
            }
        }),
        ("wrong-cursor", |a| a.cursor = a.cursor.saturating_add(1)),
        ("wrong-counter", |a| {
            a.operation_counter = a.operation_counter.saturating_add(1)
        }),
        ("wrong-stored-code", |a| a.current_code_hash = [0; 32]),
        ("wrong-destination-code", |a| {
            a.destination_code_hash = [0; 32]
        }),
        ("wrong-destination-version", |a| {
            a.destination_spec = if a.destination_spec == 135 { 136 } else { 135 }
        }),
        ("wrong-revised-policy", |a| {
            a.revised_policy = if a.revised_policy == [0; 32] {
                [1; 32]
            } else {
                [0; 32]
            }
        }),
    ] {
        let mut changed = authorization.clone();
        mutate(&mut changed);
        negative.push((name.into(), signed(changed, revised.clone())));
    }
    for name in [
        "insufficient-quorum",
        "duplicate-signer",
        "executing-code-signature-tamper",
        "wrong-signing-domain",
    ] {
        let mut changed = envelope.clone();
        let RecoveryAction::Escape {
            authorization,
            signatures,
            ..
        } = &mut changed.action
        else {
            unreachable!()
        };
        match name {
            "insufficient-quorum" => {
                signatures.truncate(1);
            }
            "duplicate-signer" => {
                signatures[1] = signatures[0].clone();
            }
            "executing-code-signature-tamper" => {
                authorization.current_executor_hash = [0; 32];
            }
            _ => {
                let mut message = authorization.signing_message();
                message.extend(b"wrong fork/domain");
                *signatures = pairs()
                    .into_iter()
                    .take(2)
                    .map(|(account, pair)| (account, MultiSignature::Ed25519(pair.sign(&message))))
                    .collect();
            }
        }
        negative.push((name.into(), changed));
    }
    Ok(Prepared { envelope, negative })
}

/// This diagnostic may alter only its public probe key and recovery state. It
/// cannot install final138, alter observed Council.Members or touch finances.
pub fn admitted_action(action: &RecoveryAction) -> bool {
    match action {
        RecoveryAction::Tick | RecoveryAction::Escape { .. } => true,
        RecoveryAction::StageCode {
            kind: recovery::RecoveryCodeKind::Repair,
            ..
        } => true,
        RecoveryAction::Apply(chunk) => {
            chunk.edits.len() == 1
                && chunk.edits.iter().all(|edit| {
                    matches!(edit.scope, recovery::StorageScope::Top)
                        && edit.key == PROBE_KEY
                        && edit.after.as_deref() == Some(PROBE_VALUE)
                })
        }
        RecoveryAction::Audit(chunk) => {
            chunk.witnesses.len() == 1
                && chunk.witnesses.iter().all(|witness| {
                    matches!(witness.scope, recovery::StorageScope::Top)
                        && witness.key == PROBE_KEY
                        && witness.value_hash == Some(sp_io::hashing::blake2_256(PROBE_VALUE))
                })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_keys_match_generator_and_authorizations_use_observed_state() {
        let members: Vec<_> = pairs().into_iter().map(|(account, _)| account).collect();
        let encoded: Vec<_> = members
            .iter()
            .map(|account| hex::encode(AsRef::<[u8]>::as_ref(account)))
            .collect();
        assert_eq!(
            encoded,
            [
                "3d0bfb1e8c2639da9165a9b841eff0322a91e4726f5b3cfbbab33ae237248cb6",
                "a2384b2633793ee7124bd13b0dfaab6b5c820085401e80fed28b415381f99b7c",
                "cc75822e1a79475ca9b33bb1e9a6d331032e0b89733524d5881d4096c48649e3",
            ]
        );
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: members,
            chunk_hashes: vec![[1; 32]],
            audit_chunk_hashes: vec![[2; 32]],
            evidence_commitment: [3; 32],
            terminal_audit_commitment: [4; 32],
        };
        let observed = RecoveryStatus {
            format_version: 1,
            policy: policy.commitment(),
            phase: recovery::RecoveryPhase::Failed,
            next_chunk: 9,
            operation_counter: 17,
            failure: Some(recovery::RecoveryError::WrongPreimage),
        };
        let result = prepare(
            Path::new("."),
            &Escape {
                destination_spec: 135,
                work_policy: None,
            },
            &policy,
            &observed,
            [0x42; 32],
            b"actual stored code",
            b"actual substitute code",
            b"actual hold code",
        )
        .unwrap();
        let RecoveryAction::Escape {
            authorization,
            signatures,
            ..
        } = &result.envelope.action
        else {
            panic!()
        };
        assert_eq!(authorization.phase, observed.phase);
        assert_eq!(authorization.cursor, 9);
        assert_eq!(authorization.operation_counter, 17);
        assert_eq!(
            authorization.current_code_hash,
            sp_io::hashing::blake2_256(b"actual stored code")
        );
        assert_eq!(
            authorization.current_executor_hash,
            sp_io::hashing::blake2_256(b"actual substitute code")
        );
        assert!(authorization
            .verify_quorum(&policy, &observed, [0x42; 32], signatures)
            .is_ok());
        for (name, envelope) in &result.negative {
            let RecoveryAction::Escape {
                authorization,
                signatures,
                ..
            } = &envelope.action
            else {
                panic!()
            };
            if [
                "wrong-recovery-anchor",
                "wrong-policy",
                "wrong-phase",
                "wrong-cursor",
                "wrong-counter",
                "insufficient-quorum",
                "duplicate-signer",
                "executing-code-signature-tamper",
                "wrong-signing-domain",
            ]
            .contains(&name.as_str())
            {
                assert!(
                    authorization
                        .verify_quorum(&policy, &observed, [0x42; 32], signatures)
                        .is_err(),
                    "{name}"
                );
            }
        }
        let changed = RecoveryStatus {
            operation_counter: 18,
            ..observed
        };
        assert!(authorization
            .verify_quorum(&policy, &changed, [0x42; 32], signatures)
            .is_err());
    }

    #[test]
    fn escape_fixture_cannot_stage_final_or_edit_observed_governance() {
        assert!(!admitted_action(&RecoveryAction::StageCode {
            kind: recovery::RecoveryCodeKind::Final,
            index: 0,
            bytes: vec![1]
        }));
        let mut action = RecoveryAction::Apply(recovery::RecoveryChunk {
            index: 0,
            edits: vec![recovery::GuardedStorageEdit {
                scope: recovery::StorageScope::Top,
                key: PROBE_KEY.to_vec(),
                before: None,
                after: Some(PROBE_VALUE.to_vec()),
            }],
        });
        assert!(admitted_action(&action));
        if let RecoveryAction::Apply(chunk) = &mut action {
            chunk.edits[0].key = b"Council.Members".to_vec();
        }
        assert!(!admitted_action(&action));
    }
}
