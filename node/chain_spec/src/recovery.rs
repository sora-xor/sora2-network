//! Public, non-secret admission data for the checkpoint recovery network.

use codec::DecodeWithMemLimit;
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
/// Client set immediately before the retained checkpoint forced transition.
pub const PREVIOUS_GRANDPA_SET_ID: u64 = 7956;
/// The public head admitted by this runtime's permanent equivocation backstop.
pub const LAST_ABANDONED_SLOT: u64 =
    framenode_runtime::incident_recovery_babe_reports::LAST_ABANDONED_BABE_SLOT;

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
    /// Full bounded policy allows admission to bind its signature domain to this network.
    pub policy_scale: sp_core::Bytes,
    pub bootstrap_code_hash: H256,
    pub final_code_hash: H256,
    pub hold_code_hash: H256,
    /// Commitment to the independently checked checkpoint, mapping and settlement evidence.
    pub evidence_commitment: H256,
    /// Client authority-set ID immediately before importing the checkpoint.
    pub previous_grandpa_set_id: u64,
    /// Ordered authorities immediately before checkpoint import, including weights.
    pub previous_grandpa_authorities: Vec<(AuthorityId, u64)>,
    /// Identical retained GRANDPA keys, order and weights at the forced boundary.
    pub grandpa_authorities: Vec<(AuthorityId, u64)>,
    /// Sorted checkpoint key union, including active, queued and NextKeys keys.
    pub approved_grandpa_keys: Vec<AuthorityId>,
    /// Maximum abandoned BABE slot established by the retained public chain evidence.
    pub last_observed_abandoned_slot: u64,
    pub first_recovery_slot: u64,
}

impl RecoveryConfig {
    /// Validate invariants that do not require access to the checkpoint database.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 3 {
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
        if self.policy_scale.0.len() > 2 * 1024 * 1024 {
            return Err("recovery policy exceeds admission bound".into());
        }
        let mut input = self.policy_scale.0.as_slice();
        let policy =
            common::recovery::RecoveryPolicy::decode_with_mem_limit(&mut input, 16 * 1024 * 1024)
                .map_err(|_| "invalid recovery policy encoding")?;
        policy.validate().map_err(|_| "invalid recovery policy")?;
        if !input.is_empty()
            || policy.commitment() != self.policy_commitment.0
            || policy.evidence_commitment != self.evidence_commitment.0
            || policy.authorization_domain.chain_spec_id != CHAIN_ID.as_bytes()
        {
            return Err("recovery policy commitment or authorization network differs".into());
        }
        if self.bootstrap_code_hash == self.final_code_hash
            || self.bootstrap_code_hash == self.hold_code_hash
            || self.final_code_hash == self.hold_code_hash
        {
            return Err("recovery bootstrap, hold and final runtimes must be distinct".into());
        }
        if self.previous_grandpa_set_id != PREVIOUS_GRANDPA_SET_ID {
            return Err("recovery must force the checkpoint GRANDPA set 7956 to 7957".into());
        }
        if self.last_observed_abandoned_slot != LAST_ABANDONED_SLOT
            || self.first_recovery_slot <= self.last_observed_abandoned_slot
        {
            return Err(
                "abandoned slot must match the runtime backstop and first recovery slot must exceed it".into(),
            );
        }
        if self.grandpa_authorities.is_empty()
            || self.grandpa_authorities != self.previous_grandpa_authorities
        {
            return Err(
                "recovery must retain the exact checkpoint GRANDPA authorities and weights".into(),
            );
        }
        let mut retained = BTreeSet::new();
        for (key, weight) in &self.grandpa_authorities {
            if *weight == 0 || !retained.insert(key.clone()) {
                return Err(
                    "checkpoint GRANDPA authorities require unique keys and nonzero weights".into(),
                );
            }
        }
        if self.approved_grandpa_keys.is_empty()
            || self.approved_grandpa_keys.len() > 3000
            || self
                .approved_grandpa_keys
                .windows(2)
                .any(|keys| keys[0] >= keys[1])
        {
            return Err("checkpoint GRANDPA inventory must be bounded, sorted and unique".into());
        }
        let approved: BTreeSet<_> = self.approved_grandpa_keys.iter().cloned().collect();
        if !retained.is_subset(&approved) {
            return Err("checkpoint GRANDPA inventory omits an active retained key".into());
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
        use codec::Encode;
        let policy = common::recovery::RecoveryPolicy {
            format_version: 2,
            checkpoint_hash: common::recovery::CHECKPOINT_HASH,
            checkpoint_state_root: common::recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![sp_runtime::AccountId32::new([7; 32])],
            chunk_hashes: vec![[8; 32]],
            audit_chunk_hashes: vec![[9; 32]],
            evidence_commitment: [5; 32],
            terminal_audit_commitment: [10; 32],
            authorization_domain: common::recovery::RecoveryDomain {
                chain_spec_id: CHAIN_ID.as_bytes().to_vec(),
                release_nonce: [11; 32],
            },
        };
        RecoveryConfig {
            schema_version: 3,
            checkpoint_hash: common::recovery::CHECKPOINT_HASH.into(),
            policy_commitment: policy.commitment().into(),
            policy_scale: sp_core::Bytes(policy.encode()),
            bootstrap_code_hash: [2; 32].into(),
            final_code_hash: [3; 32].into(),
            hold_code_hash: [4; 32].into(),
            evidence_commitment: [5; 32].into(),
            previous_grandpa_set_id: PREVIOUS_GRANDPA_SET_ID,
            previous_grandpa_authorities: vec![(authority(1), 1), (authority(2), 2)],
            grandpa_authorities: vec![(authority(1), 1), (authority(2), 2)],
            approved_grandpa_keys: vec![authority(1), authority(2)],
            last_observed_abandoned_slot: LAST_ABANDONED_SLOT,
            first_recovery_slot: LAST_ABANDONED_SLOT + 1,
        }
    }

    #[test]
    fn requires_identical_retained_authorities_and_weights() {
        let valid = fixture();
        assert_eq!(valid.validate(), Ok(()));
        let mut reused = valid.clone();
        reused.grandpa_authorities[1].0 = reused.previous_grandpa_authorities[0].0.clone();
        assert!(reused.validate().unwrap_err().contains("retain"));
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
        config.first_recovery_slot = config.last_observed_abandoned_slot;
        assert!(config.validate().is_err());
        config = fixture();
        config.evidence_commitment = H256::zero();
        assert!(config.validate().is_err());
    }

    #[test]
    fn retained_inventory_covers_active_and_pending_keys() {
        let mut config = fixture();
        config.approved_grandpa_keys.push(authority(6));
        assert!(config.validate().is_ok());
        config.approved_grandpa_keys.remove(0);
        assert!(config.validate().is_err());
        config = fixture();
        config.previous_grandpa_set_id += 1;
        assert!(config.validate().is_err());
        config = fixture();
        config.last_observed_abandoned_slot = LAST_ABANDONED_SLOT - 1;
        assert!(config.validate().is_err());
        config.last_observed_abandoned_slot = LAST_ABANDONED_SLOT + 1;
        config.first_recovery_slot = LAST_ABANDONED_SLOT + 2;
        assert!(config.validate().is_err());
        let mut legacy = serde_json::to_value(fixture()).unwrap();
        legacy["retiredGrandpaKeys"] = serde_json::json!([]);
        assert!(serde_json::from_value::<RecoveryConfig>(legacy).is_err());
    }

    #[test]
    fn policy_bytes_bind_the_authorization_network_and_reject_legacy_domains() {
        use codec::{DecodeAll, Encode};
        let valid = fixture();
        let mut config = valid.clone();
        config.schema_version = 1;
        assert!(config.validate().is_err());
        let mut policy =
            common::recovery::RecoveryPolicy::decode_all(&mut valid.policy_scale.0.as_slice())
                .unwrap();
        policy.authorization_domain.chain_spec_id = b"sora-nexus-PUBLIC-FIXTURE-ONLY".to_vec();
        config = valid.clone();
        config.policy_scale = sp_core::Bytes(policy.encode());
        config.policy_commitment = policy.commitment().into();
        assert!(config.validate().unwrap_err().contains("network"));
        config = valid.clone();
        config.policy_scale.0.push(0);
        assert!(config.validate().is_err());
        config = valid.clone();
        config.policy_commitment = H256::zero();
        assert!(config.validate().is_err());
        policy.authorization_domain.chain_spec_id = CHAIN_ID.as_bytes().to_vec();
        policy.authorization_domain.release_nonce = [0; 32];
        config = valid;
        config.policy_scale = sp_core::Bytes(policy.encode());
        config.policy_commitment = policy.commitment().into();
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
