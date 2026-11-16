//! Recovery consistency and local operator acknowledgement are separate from
//! the guardian emergency signal. The legacy bridge-halt marker is deliberately
//! not an input to consensus admission: a completed recovery needs no unhalt call.

use codec::{Decode, DecodeAll, DecodeWithMemTracking, Encode};
use sp_std::vec::Vec;

const POLICY: &[u8] = b":sora-nexus:recovery:policy:v1";
const STATUS: &[u8] = b":sora-nexus:recovery:status:v1";
const ACTIVE_POLICY: &[u8] = b":sora-nexus:recovery:active-policy:v1";
const ANCHOR: &[u8] = b":sora-nexus:recovery:branch-anchor:v1";
const TERMINAL: &[u8] = b":sora-nexus:recovery:terminal-audit:v1";
const PENDING: &[u8] = b":sora-nexus:recovery:incoming-audit-pending:v1";
const INBOUND: &[u8] = b":sora-nexus:recovery:bridge-inbound-context:v1";
const INCOMING_ACTIVE: &[u8] = b":sora-nexus:recovery:incoming-active:v1";
const TRANSIENT: &[&[u8]] = &[
    INBOUND,
    INCOMING_ACTIVE,
    b":sora-nexus:recovery:incoming-failed:v1",
    b":sora-nexus:recovery:hashi-resolution:v1",
];

/// Maximum storage reads in a complete consistency check. Callers must include
/// these reads in their ordinary-operation weight allowance.
pub const COMPLETION_READS: u64 = 10;
pub const COMPLETION_PROOF_SIZE: u64 = 32_768;

/// Conservative state-independent allowance until dedicated benchmarks replace
/// it. This is safe to query outside runtime externalities for call metadata.
pub fn completion_weight<T: frame_system::Config>(checks: u64) -> frame_support::weights::Weight {
    use frame_support::traits::Get;
    frame_support::weights::Weight::from_parts(5_000_000, COMPLETION_PROOF_SIZE)
        .saturating_mul(checks)
        .saturating_add(T::DbWeight::get().reads(COMPLETION_READS.saturating_mul(checks)))
}

/// Public, fixed-size evidence of completed incident recovery. It confers no
/// transaction authority and does not replace permanent quarantine checks.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct RecoveryCompletion {
    pub policy: [u8; 32],
    pub active_policy: [u8; 32],
    pub recovery_anchor: [u8; 32],
    pub terminal_audit_hash: [u8; 32],
    pub operation_counter: u64,
    pub completed_chunks: u32,
}

impl RecoveryCompletion {
    pub fn context_hash(&self, genesis: [u8; 32]) -> [u8; 32] {
        sp_io::hashing::blake2_256(
            &(b"sora-nexus/ocw-recovery-ack/v1".as_slice(), genesis, self).encode(),
        )
    }
}

/// The exact format-one RecoveryStatus representation when failure is None.
/// Keeping the wire projection here avoids a dependency cycle with common.
/// common's tests compare it against the authoritative RecoveryStatus codec.
#[derive(Decode)]
struct StatusHeader {
    version: u32,
    policy: [u8; 32],
    phase: u8,
    chunks: u32,
    operation: u64,
    failure_tag: u8,
}

fn status(bytes: Option<Vec<u8>>) -> Result<StatusHeader, ()> {
    let bytes = bytes.ok_or(())?;
    if bytes.len() != 50 {
        return Err(());
    }
    let status = StatusHeader::decode_all(&mut &bytes[..]).map_err(|_| ())?;
    if status.version != 1 || status.policy == [0; 32] || status.failure_tag != 0 {
        return Err(());
    }
    Ok(status)
}

fn hash(bytes: Option<Vec<u8>>) -> Result<[u8; 32], ()> {
    let value: [u8; 32] = bytes.ok_or(())?.as_slice().try_into().map_err(|_| ())?;
    if value == [0; 32] {
        Err(())
    } else {
        Ok(value)
    }
}

/// No recovery is Ok(None), a consistent completed recovery is Ok(Some), and
/// unfinished, inconsistent or unknown-format recovery is Err. A caller whose
/// reader can fail must separately reject read errors rather than map them to
/// absent storage. No legacy halt marker or guardian signal is consulted here.
pub fn completion_with(
    mut read: impl FnMut(&[u8]) -> Option<Vec<u8>>,
) -> Result<Option<RecoveryCompletion>, ()> {
    let policy_bytes = read(POLICY);
    let status_bytes = read(STATUS);
    if policy_bytes.is_none() {
        if status_bytes.is_some()
            || [ACTIVE_POLICY, ANCHOR, TERMINAL, PENDING]
                .iter()
                .any(|key| read(key).is_some())
            || TRANSIENT.iter().any(|key| read(key).is_some())
        {
            return Err(());
        }
        return Ok(None);
    }
    let policy = hash(policy_bytes)?;
    let status = status(status_bytes)?;
    if status.phase != 4 || status.policy != policy {
        return Err(());
    }
    let active_policy = hash(read(ACTIVE_POLICY))?;
    let recovery_anchor = hash(read(ANCHOR))?;
    if read(PENDING).as_deref() != Some(&[0, 0, 0, 0])
        || TRANSIENT.iter().any(|key| read(key).is_some())
    {
        return Err(());
    }
    let terminal = read(TERMINAL).ok_or(())?;
    if terminal.len() != 68 {
        return Err(());
    }
    let (active, audit, chunks) =
        <([u8; 32], [u8; 32], u32)>::decode_all(&mut &terminal[..]).map_err(|_| ())?;
    if active != active_policy || audit == [0; 32] || chunks != status.chunks {
        return Err(());
    }
    Ok(Some(RecoveryCompletion {
        policy,
        active_policy,
        recovery_anchor,
        terminal_audit_hash: sp_io::hashing::blake2_256(&terminal),
        operation_counter: status.operation,
        completed_chunks: status.chunks,
    }))
}

pub fn completion() -> Result<Option<RecoveryCompletion>, ()> {
    completion_with(|key| {
        if TRANSIENT.contains(&key) {
            sp_io::storage::exists(key).then(Vec::new)
        } else {
            sp_io::storage::get(key).map(|bytes| bytes.to_vec())
        }
    })
}

/// Consistency guard during actual recovery work, not an emergency-halt latch.
pub fn recovery_in_progress() -> bool {
    completion().is_err()
}

/// Narrow context admission for an authenticated recovery inbound operation.
/// The executive and application still validate the complete typed operation,
/// payload, origins and one-use resolution. This predicate alone grants none of
/// those authorities, and the legacy halt marker cannot establish this context.
pub fn maintenance_inbound_active(block: u32) -> bool {
    let Ok(status) = status(sp_io::storage::get(STATUS).map(|x| x.to_vec())) else {
        return false;
    };
    status.phase == 0
        && sp_io::storage::get(POLICY).as_deref() == Some(status.policy.as_slice())
        && hash(sp_io::storage::get(ACTIVE_POLICY).map(|x| x.to_vec())).is_ok()
        && sp_io::storage::get(INBOUND).as_deref() == Some(block.to_le_bytes().as_slice())
        && sp_io::storage::read(INCOMING_ACTIVE, &mut [], 0)
            .is_some_and(|length| length > 0 && length <= 512 * 1024)
}

pub const RECOVERY_ACK_STORAGE_KEY: &[u8] = b"eth-bridge-ocw::recovery-ack:v1";

/// Local offchain storage only. Consensus never reads this acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct RecoveryAcknowledgement {
    pub version: u8,
    pub genesis: [u8; 32],
    pub context_hash: [u8; 32],
}

impl RecoveryAcknowledgement {
    pub fn new(genesis: [u8; 32], completion: &RecoveryCompletion) -> Self {
        Self {
            version: 1,
            genesis,
            context_hash: completion.context_hash(genesis),
        }
    }
}

pub fn acknowledgement_matches(
    genesis: [u8; 32],
    completion: &RecoveryCompletion,
    bytes: Option<&[u8]>,
) -> bool {
    let Some(bytes) = bytes.filter(|bytes| bytes.len() == 65) else {
        return false;
    };
    RecoveryAcknowledgement::decode_all(&mut &bytes[..])
        .is_ok_and(|ack| ack == RecoveryAcknowledgement::new(genesis, completion))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_std::collections::btree_map::BTreeMap;

    fn complete() -> BTreeMap<Vec<u8>, Vec<u8>> {
        [
            (POLICY, [11; 32].to_vec()),
            (STATUS, (1u32, [11u8; 32], 4u8, 2u32, 3u64, 0u8).encode()),
            (ACTIVE_POLICY, [12; 32].to_vec()),
            (ANCHOR, [13; 32].to_vec()),
            (TERMINAL, ([12u8; 32], [14u8; 32], 2u32).encode()),
            (PENDING, 0u32.encode()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_vec(), value))
        .collect()
    }

    #[test]
    fn completed_recovery_is_independent_of_halt_markers() {
        let mut values = complete();
        let expected = completion_with(|key| values.get(key).cloned())
            .unwrap()
            .unwrap();
        for bytes in [vec![0], vec![1], vec![99], vec![]] {
            values.insert(b":sora-nexus:recovery:bridge-halt:v1".to_vec(), bytes);
            values.insert(b":sora-nexus:bridge-halt:latest:v1".to_vec(), vec![1]);
            assert_eq!(
                completion_with(|key| values.get(key).cloned()),
                Ok(Some(expected.clone()))
            );
        }
        assert_eq!(completion_with(|_| None), Ok(None));
        let only_marker =
            BTreeMap::from([(b":sora-nexus:recovery:bridge-halt:v1".to_vec(), vec![1])]);
        assert_eq!(
            completion_with(|key| only_marker.get(key).cloned()),
            Ok(None)
        );
    }

    #[test]
    fn incomplete_or_inconsistent_recovery_cannot_admit_ordinary_operations() {
        let original = complete();
        for key in [POLICY, STATUS, ACTIVE_POLICY, ANCHOR, TERMINAL, PENDING] {
            let mut values = original.clone();
            values.remove(key);
            assert!(completion_with(|key| values.get(key).cloned()).is_err());
        }
        for phase in [0u8, 1, 2, 3, 5, 255] {
            let mut values = original.clone();
            values.insert(
                STATUS.to_vec(),
                (1u32, [11u8; 32], phase, 2u32, 3u64, 0u8).encode(),
            );
            assert!(completion_with(|key| values.get(key).cloned()).is_err());
        }
        for (key, bytes) in [
            (STATUS, (2u32, [11u8; 32], 4u8, 2u32, 3u64, 0u8).encode()),
            (STATUS, (1u32, [11u8; 32], 4u8, 2u32, 3u64, 1u8).encode()),
            (POLICY, [90; 32].to_vec()),
            (ACTIVE_POLICY, [90; 32].to_vec()),
            (ANCHOR, vec![0; 32]),
            (TERMINAL, ([12u8; 32], [0u8; 32], 2u32).encode()),
            (TERMINAL, ([12u8; 32], [14u8; 32], 99u32).encode()),
            (PENDING, 1u32.encode()),
        ] {
            let mut values = original.clone();
            values.insert(key.to_vec(), bytes);
            assert!(completion_with(|key| values.get(key).cloned()).is_err());
        }
        for key in TRANSIENT {
            let mut values = original.clone();
            values.insert(key.to_vec(), vec![]);
            assert!(completion_with(|key| values.get(key).cloned()).is_err());
        }
        let mut values = original.clone();
        values.get_mut(STATUS).unwrap().push(0);
        assert!(completion_with(|key| values.get(key).cloned()).is_err());
    }

    #[test]
    fn local_acknowledgement_is_exact_and_bound_to_chain_and_context() {
        let values = complete();
        let completion = completion_with(|key| values.get(key).cloned())
            .unwrap()
            .unwrap();
        let genesis = [17; 32];
        let bytes = RecoveryAcknowledgement::new(genesis, &completion).encode();
        assert!(acknowledgement_matches(genesis, &completion, Some(&bytes)));
        assert!(!acknowledgement_matches(genesis, &completion, None));
        assert!(!acknowledgement_matches(
            [18; 32],
            &completion,
            Some(&bytes)
        ));
        let mut changed = completion.clone();
        changed.operation_counter += 1;
        assert!(!acknowledgement_matches(genesis, &changed, Some(&bytes)));
        let mut invalid = bytes.clone();
        invalid.push(0);
        assert!(!acknowledgement_matches(
            genesis,
            &completion,
            Some(&invalid)
        ));
        invalid = bytes.clone();
        invalid[0] = 2;
        assert!(!acknowledgement_matches(
            genesis,
            &completion,
            Some(&invalid)
        ));
    }

    #[test]
    fn maintenance_requires_an_actual_current_applying_context() {
        sp_io::TestExternalities::default().execute_with(|| {
            for (key, value) in complete() {
                sp_io::storage::set(&key, &value);
            }
            sp_io::storage::set(INBOUND, &7u32.encode());
            sp_io::storage::set(INCOMING_ACTIVE, &[1]);
            assert!(!maintenance_inbound_active(7));
            sp_io::storage::set(STATUS, &(1u32, [11u8; 32], 0u8, 2u32, 3u64, 0u8).encode());
            assert!(maintenance_inbound_active(7));
            assert!(!maintenance_inbound_active(8));
            sp_io::storage::clear(INCOMING_ACTIVE);
            assert!(!maintenance_inbound_active(7));
            sp_io::storage::set(b":sora-nexus:recovery:bridge-halt:v1", &[1]);
            assert!(!maintenance_inbound_active(7));
        });
    }
}
