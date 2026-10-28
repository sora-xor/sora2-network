//! Internal dispatch for exact executing-runtime native comparison.
//! A manifest selects admitted binaries, never which binary handles a block.
use crate::Result;
use common::recovery::{RecoveryPhase, RecoveryStatus};

pub const PUBLIC_PRIOR_AUDIT_SCHEMA: &str =
    "sora-nexus-public-fixture-prior-policy-audit-maintenance-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    Original,
    Repaired,
}

pub fn select(allow_dual_lanes: bool, original: bool, repaired: bool, spec: u32) -> Result<Lane> {
    if allow_dual_lanes {
        if !original || !repaired {
            return Err(
                "prior-policy fixture requires both separately bound native engines".into(),
            );
        }
        return match spec {
            134 => Ok(Lane::Original),
            136 | 137 => Ok(Lane::Repaired),
            _ => Err("dynamic native operation executed outside original or repair runtime".into()),
        };
    }
    match (original, repaired) {
        (true, false) => Ok(Lane::Original),
        (false, true) => Ok(Lane::Repaired),
        _ => Err("ambiguous or absent native recorder lane".into()),
    }
}

pub fn check_original(
    spec: u32,
    transaction: u32,
    exact_bootstrap: bool,
    status: &RecoveryStatus,
    active_policy: Option<&[u8]>,
    original_policy: [u8; 32],
) -> Result<()> {
    if spec != 134
        || transaction != 131
        || !exact_bootstrap
        || status.format_version != 1
        || status.policy != original_policy
        || status.operation_counter != 0
        || status.phase != RecoveryPhase::Applying
        || status.failure.is_some()
        || active_policy != Some(original_policy.as_slice())
    {
        return Err(
            "native original operation lacks exact bootstrap code, policy and pre-escape lineage"
                .into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lane_follows_observed_spec_without_fallback() {
        assert_eq!(select(true, true, true, 134).unwrap(), Lane::Original);
        for spec in [136, 137] {
            assert_eq!(select(true, true, true, spec).unwrap(), Lane::Repaired);
        }
        for spec in [0, 133, 135, 138] {
            assert!(select(true, true, true, spec).is_err());
        }
        assert!(select(true, false, true, 134).is_err());
        assert!(select(true, true, false, 136).is_err());
        assert!(select(false, true, true, 134).is_err());
        assert!(select(false, false, false, 134).is_err());
    }

    #[test]
    fn original_engine_cannot_cross_policy_counter_or_code_boundary() {
        let policy = [7; 32];
        let valid = RecoveryStatus {
            format_version: 1,
            policy,
            phase: RecoveryPhase::Applying,
            next_chunk: 1,
            operation_counter: 0,
            failure: None,
        };
        assert!(check_original(134, 131, true, &valid, Some(&policy), policy).is_ok());
        assert!(check_original(136, 132, true, &valid, Some(&policy), policy).is_err());
        assert!(check_original(134, 132, true, &valid, Some(&policy), policy).is_err());
        assert!(check_original(134, 131, false, &valid, Some(&policy), policy).is_err());
        assert!(check_original(134, 131, true, &valid, None, policy).is_err());
        assert!(check_original(134, 131, true, &valid, Some(&[8; 32]), policy).is_err());
        let mut changed = valid.clone();
        changed.operation_counter = 2;
        assert!(check_original(134, 131, true, &changed, Some(&policy), policy).is_err());
        changed = valid.clone();
        changed.policy = [8; 32];
        assert!(check_original(134, 131, true, &changed, Some(&policy), policy).is_err());
        changed = valid;
        changed.phase = RecoveryPhase::Auditing;
        assert!(check_original(134, 131, true, &changed, Some(&policy), policy).is_err());
    }
}
