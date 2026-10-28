//! Public, non-secret admission data for the checkpoint recovery network.

use sc_chain_spec::ChainSpecExtension;
use sc_finality_grandpa::AuthorityId;
use serde::{Deserialize, Serialize};
use sp_core::H256;
use std::collections::BTreeSet;

/// A distinct ID also selects a distinct default database directory.
pub const CHAIN_ID: &str = "sora-nexus-recovery-28191817";
/// Never share discovery or gossip with the abandoned branch.
pub const PROTOCOL_ID: &str = "sora-nexus-recovery-28191817-v1";
/// Network fork identity; the original genesis remains unchanged.
pub const FORK_ID: &str = "recovery-28191817-v1";

/// Optional extensions preserve compatibility with existing chain specifications.
#[derive(Clone, Debug, Default, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    /// Present only on the explicitly prepared recovery chain specification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryConfig>,
}

/// Reviewed release inputs. No default keys, code hashes or evidence are supplied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryConfig {
    pub schema_version: u32,
    pub checkpoint_hash: H256,
    pub policy_commitment: H256,
    pub bootstrap_code_hash: H256,
    pub final_code_hash: H256,
    pub hold_code_hash: H256,
    /// Commitment to the independently checked checkpoint, mapping and settlement evidence.
    pub evidence_commitment: H256,
    /// Client authority-set ID immediately before importing the checkpoint.
    pub previous_grandpa_set_id: u64,
    /// Ordered authorities immediately before checkpoint import, including weights.
    pub previous_grandpa_authorities: Vec<(AuthorityId, u64)>,
    /// Fresh GRANDPA keys in the same operator order and with the same weights.
    pub grandpa_authorities: Vec<(AuthorityId, u64)>,
    /// Sorted complete old-key inventory, including announced and NextKeys keys.
    pub retired_grandpa_keys: Vec<AuthorityId>,
    /// Sorted fresh-key inventory for active and queued approved operators.
    pub approved_grandpa_keys: Vec<AuthorityId>,
    /// Maximum previously signed BABE slot reported by all restarting operators.
    pub last_abandoned_signed_slot: u64,
    pub first_recovery_slot: u64,
}

impl RecoveryConfig {
    /// Validate invariants that do not require access to the checkpoint database.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("unsupported recovery configuration schema".into());
        }
        if self.checkpoint_hash != H256::from(common::recovery::CHECKPOINT_HASH) {
            return Err("recovery checkpoint hash differs from the approved checkpoint".into());
        }
        if [
            self.policy_commitment,
            self.bootstrap_code_hash,
            self.final_code_hash,
            self.hold_code_hash,
            self.evidence_commitment,
        ]
        .iter()
        .any(H256::is_zero)
        {
            return Err("recovery requires nonzero policy, code and evidence commitments".into());
        }
        if self.bootstrap_code_hash == self.final_code_hash
            || self.bootstrap_code_hash == self.hold_code_hash
            || self.final_code_hash == self.hold_code_hash
        {
            return Err("recovery bootstrap, hold and final runtimes must be distinct".into());
        }
        self.previous_grandpa_set_id
            .checked_add(1)
            .ok_or("GRANDPA recovery set ID overflows")?;
        if self.last_abandoned_signed_slot == 0
            || self.first_recovery_slot <= self.last_abandoned_signed_slot
        {
            return Err(
                "first recovery slot must exceed the reported abandoned signing maximum".into(),
            );
        }
        if self.grandpa_authorities.is_empty()
            || self.grandpa_authorities.len() != self.previous_grandpa_authorities.len()
        {
            return Err(
                "recovery requires complete old and fresh GRANDPA authority inventories".into(),
            );
        }
        let mut old = BTreeSet::new();
        let mut fresh = BTreeSet::new();
        for ((old_key, old_weight), (new_key, new_weight)) in self
            .previous_grandpa_authorities
            .iter()
            .zip(&self.grandpa_authorities)
        {
            if *old_weight == 0 || old_weight != new_weight {
                return Err(
                    "recovery must preserve nonzero GRANDPA weights and operator ordering".into(),
                );
            }
            if !old.insert(old_key.clone()) || !fresh.insert(new_key.clone()) {
                return Err("duplicate GRANDPA authority in recovery inventory".into());
            }
        }
        if !old.is_disjoint(&fresh) {
            return Err("recovery GRANDPA keys must all be fresh".into());
        }
        if self.retired_grandpa_keys.is_empty()
            || self.retired_grandpa_keys.len() > 3000
            || self.approved_grandpa_keys.is_empty()
            || self.approved_grandpa_keys.len() > 1000
            || self
                .retired_grandpa_keys
                .windows(2)
                .any(|keys| keys[0] >= keys[1])
            || self
                .approved_grandpa_keys
                .windows(2)
                .any(|keys| keys[0] >= keys[1])
        {
            return Err("complete GRANDPA inventories must be bounded, sorted and unique".into());
        }
        let retired: BTreeSet<_> = self.retired_grandpa_keys.iter().cloned().collect();
        let approved: BTreeSet<_> = self.approved_grandpa_keys.iter().cloned().collect();
        if !old.is_subset(&retired)
            || !fresh.is_subset(&approved)
            || !retired.is_disjoint(&approved)
        {
            return Err(
                "complete GRANDPA inventories omit active keys or reuse retired keys".into(),
            );
        }
        Ok(())
    }
}

/// Read the typed extension without parsing or trusting arbitrary properties.
pub fn config(spec: &dyn sc_service::ChainSpec) -> Option<&RecoveryConfig> {
    sc_chain_spec::get_extension::<Extensions>(spec.extensions())
        .and_then(|extensions| extensions.recovery.as_ref())
}

/// Build a separate recovery specification while retaining the original genesis.
/// Bootstrap bytes are supplied only after they have been built and rehearsed.
pub fn prepare(
    original: &dyn sc_service::ChainSpec,
    recovery: RecoveryConfig,
    bootstrap: &[u8],
) -> Result<super::ChainSpec, String> {
    recovery.validate()?;
    if H256::from(sp_io::hashing::blake2_256(bootstrap)) != recovery.bootstrap_code_hash {
        return Err("recovery bootstrap code does not match its commitment".into());
    }
    let mut raw: serde_json::Value =
        serde_json::from_str(&original.as_json(true)?).map_err(|error| error.to_string())?;
    raw["name"] = "SORA Nexus recovery at 28,191,817".into();
    raw["id"] = CHAIN_ID.into();
    raw["protocolId"] = PROTOCOL_ID.into();
    raw["forkId"] = FORK_ID.into();
    // A new fork must not inherit production discovery destinations.
    raw["bootNodes"] = serde_json::json!([]);
    raw["telemetryEndpoints"] = serde_json::json!([]);
    raw["recovery"] = serde_json::to_value(recovery).map_err(|error| error.to_string())?;
    let substitutes = raw
        .as_object_mut()
        .ok_or("chain specification is not an object")?
        .entry("codeSubstitutes")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or("codeSubstitutes is not an object")?;
    let height = common::recovery::CHECKPOINT_NUMBER.to_string();
    if substitutes.contains_key(&height) {
        return Err(
            "a checkpoint substitute already exists; immutable bootstrap replacement refused"
                .into(),
        );
    }
    substitutes.insert(
        height,
        serde_json::to_value(sp_core::Bytes(bootstrap.to_vec()))
            .map_err(|error| error.to_string())?,
    );
    super::ChainSpec::from_json_bytes(serde_json::to_vec(&raw).map_err(|error| error.to_string())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authority(byte: u8) -> AuthorityId {
        sp_core::ed25519::Public::from_raw([byte; 32]).into()
    }

    fn fixture() -> RecoveryConfig {
        RecoveryConfig {
            schema_version: 1,
            checkpoint_hash: common::recovery::CHECKPOINT_HASH.into(),
            policy_commitment: [1; 32].into(),
            bootstrap_code_hash: [2; 32].into(),
            final_code_hash: [3; 32].into(),
            hold_code_hash: [4; 32].into(),
            evidence_commitment: [5; 32].into(),
            previous_grandpa_set_id: 42,
            previous_grandpa_authorities: vec![(authority(1), 1), (authority(2), 2)],
            grandpa_authorities: vec![(authority(3), 1), (authority(4), 2)],
            retired_grandpa_keys: vec![authority(1), authority(2)],
            approved_grandpa_keys: vec![authority(3), authority(4)],
            last_abandoned_signed_slot: 100,
            first_recovery_slot: 101,
        }
    }

    #[test]
    fn requires_fresh_complete_weight_preserving_authorities() {
        let valid = fixture();
        assert_eq!(valid.validate(), Ok(()));
        let mut reused = valid.clone();
        reused.grandpa_authorities[1].0 = reused.previous_grandpa_authorities[0].0.clone();
        assert!(reused.validate().unwrap_err().contains("fresh"));
        let mut reordered_weights = valid.clone();
        reordered_weights.grandpa_authorities.swap(0, 1);
        assert!(reordered_weights.validate().is_err());
        let mut duplicate = valid;
        duplicate.grandpa_authorities[1].0 = duplicate.grandpa_authorities[0].0.clone();
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn rejects_unbound_checkpoint_and_replayed_slot() {
        let mut config = fixture();
        config.checkpoint_hash = H256::zero();
        assert!(config.validate().is_err());
        config = fixture();
        config.first_recovery_slot = config.last_abandoned_signed_slot;
        assert!(config.validate().is_err());
        config = fixture();
        config.evidence_commitment = H256::zero();
        assert!(config.validate().is_err());
    }

    #[test]
    fn inventories_cover_pending_keys_without_reusing_any_retired_key() {
        let mut config = fixture();
        config.retired_grandpa_keys.push(authority(5));
        config.approved_grandpa_keys.push(authority(6));
        assert!(config.validate().is_ok());
        config.approved_grandpa_keys.insert(2, authority(5));
        assert!(config.validate().is_err());
        config = fixture();
        config.retired_grandpa_keys.remove(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn legacy_extensions_are_empty_and_unknown_recovery_fields_fail() {
        let legacy: Extensions = serde_json::from_str("{}").unwrap();
        assert!(legacy.recovery.is_none());
        let mut value = serde_json::to_value(fixture()).unwrap();
        value["uncheckedBypass"] = true.into();
        assert!(serde_json::from_value::<RecoveryConfig>(value).is_err());
    }
}
