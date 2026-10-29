//! Observe the real first final-runtime initialization on the completed maintenance state.
//! Only this invocation's externalities are admitted; JSON cannot construct a witness.
use super::{replay, storage_key, value, BlockWitness, Result};
use codec::{Compact, Decode, DecodeLimit, Encode};
use serde_json::{json, Value};
use sp_core::H256;

#[path = "maintenance_activation_state.rs"]
mod state;
pub(super) use state::Maintenance;

fn required<T: Decode + Encode>(
    ext: &mut sp_io::TestExternalities,
    pallet: &[u8],
    item: &[u8],
) -> Result<T> {
    let raw = value(ext, &storage_key(pallet, item)).ok_or("activation state value missing")?;
    if raw.len() > 256 * 1024 {
        return Err("activation state value exceeds bound".into());
    }
    let mut input = raw.as_slice();
    let decoded = T::decode_with_depth_limit(
        32,
        &mut codec::MemTrackingInput::new(&mut input, 1024 * 1024),
    )
    .map_err(|e| e.to_string())?;
    if !input.is_empty() || decoded.encode() != raw {
        return Err("activation noncanonical state encoding".into());
    }
    Ok(decoded)
}

fn active_policy(ext: &mut sp_io::TestExternalities, lineage: [u8; 32]) -> Result<[u8; 32]> {
    let status = super::status(ext)?;
    if status.format_version != 1
        || status.phase != common::recovery::RecoveryPhase::Complete
        || status.policy != lineage
        || status.failure.is_some()
        || value(ext, common::recovery::POLICY_STORAGE_KEY).as_deref() != Some(lineage.as_slice())
    {
        return Err("activation completed status/immutable policy lineage differs".into());
    }
    value(ext, common::recovery::ACTIVE_POLICY_STORAGE_KEY)
        .ok_or("activation active policy missing")?
        .try_into()
        .map_err(|_| "activation active policy width differs".into())
}

pub(super) struct Track {
    number: u32,
    parent: H256,
    parent_root: H256,
    snapshot: String,
    policy: [u8; 32],
    lineage: [u8; 32],
    code_hash: [u8; 32],
    spec_name: Vec<u8>,
    planned: u32,
    active: u32,
    initialized_root: Option<H256>,
    maintenance: Value,
    previous: state::Core,
    rows: Vec<Value>,
    report_bytes: usize,
    first_session: Option<(u32, u32, u32)>,
    first_era: Option<(u32, u32, u32)>,
}

impl Track {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn before(
        ext: &mut sp_io::TestExternalities,
        number: u32,
        parent: H256,
        snapshot: &str,
        lineage: [u8; 32],
        code: &[u8],
        version: &sp_version::RuntimeVersion,
        maintenance: &Maintenance,
    ) -> Result<Self> {
        let root = replay::root(ext);
        let policy = active_policy(ext, lineage)?;
        let previous_upgrade: (Compact<u32>, Vec<u8>) =
            required(ext, b"System", b"LastRuntimeUpgrade")?;
        let staking: u16 = required(ext, b"Staking", b":__STORAGE_VERSION__:")?;
        let planned: u32 = required(ext, b"Staking", b"CurrentEra")?;
        let (active, _start): (u32, Option<u64>) = required(ext, b"Staking", b"ActiveEra")?;
        let previous = state::Core::read(ext)?;
        maintenance.parent(parent, &previous)?;
        if version.spec_version != 138
            || version.transaction_version != 134
            || previous_upgrade.0 .0 >= 138
            || previous_upgrade.1.as_slice() != version.spec_name.as_bytes()
            || staking != 16
            || replay::root(ext) != root
        {
            return Err(
                "first final-runtime block lacks exact completed legacy activation state".into(),
            );
        }
        Ok(Self {
            number,
            parent,
            parent_root: root,
            snapshot: snapshot.into(),
            policy,
            lineage,
            code_hash: sp_io::hashing::blake2_256(code),
            spec_name: version.spec_name.as_bytes().to_vec(),
            planned,
            active,
            initialized_root: None,
            maintenance: maintenance.report()?,
            previous,
            rows: Vec::new(),
            report_bytes: 0,
            first_session: None,
            first_era: None,
        })
    }

    pub(super) fn initialized(&mut self, ext: &mut sp_io::TestExternalities) -> Result<()> {
        if self.initialized_root.is_some() {
            return Err("activation initialization already observed".into());
        }
        let root = replay::root(ext);
        let upgraded: (Compact<u32>, Vec<u8>) = required(ext, b"System", b"LastRuntimeUpgrade")?;
        let staking: u16 = required(ext, b"Staking", b":__STORAGE_VERSION__:")?;
        let cutoff: u32 = required(ext, b"Staking", b"CutoffEra")?;
        let first: u32 = required(ext, b"Staking", b"FirstExecutionEra")?;
        let weighted: u32 = required(ext, b"Staking", b"WeightedPointsFormulaStartEra")?;
        // on_initialize can legitimately leave timestamp_era pending until
        // finalization; all other queue/recency relationships already apply.
        state::State::read(ext, false)?;
        if upgraded.0 .0 != 138
            || upgraded.1 != self.spec_name
            || staking != 18
            || cutoff != self.planned
            || first != self.active
            || weighted != self.active.saturating_add(1)
            || replay::root(ext) != root
        {
            return Err(
                "exact final-runtime initialization failed async migration postconditions".into(),
            );
        }
        self.initialized_root = Some(root);
        Ok(())
    }

    pub(super) fn accepted(
        &mut self,
        ext: &mut sp_io::TestExternalities,
        witness: &BlockWitness,
        slot: u64,
    ) -> Result<()> {
        let root = replay::root(ext);
        let number = self
            .number
            .checked_add(self.rows.len() as u32)
            .ok_or("activation block overflow")?;
        if self.initialized_root.is_none()
            || self.rows.len() >= state::MAX_ROWS
            || witness.number != number
            || witness.state_root != format!("{root:#x}")
            || witness.executing_code_blake2 != hex::encode(self.code_hash)
            || witness.resulting_spec != 138
            || witness.phase != "Complete"
            || witness.maintenance_session_frozen
            || !witness.independent_import_root_matches
            || !witness.disk_restart_root_matches
        {
            return Err(
                "activation post-block witness is missing, reordered or from different code/state"
                    .into(),
            );
        }
        let after = state::State::read(ext, true)?;
        if after.core.slot != slot {
            return Err("activation post-block slot differs from executed input".into());
        }
        let (session_changed, era_changed) = state::transition(&self.previous, &after.core)?;
        if era_changed && after.core.start != Some(after.timestamp) {
            return Err("activation era did not start at this block's actual timestamp".into());
        }
        if session_changed && self.first_session.is_none() {
            self.first_session = Some((number, self.previous.session, after.core.session));
        }
        if era_changed && self.first_era.is_none() {
            self.first_era = Some((number, self.previous.active, after.core.active));
        }
        let row = json!({"block":number,"blockHash":witness.hash,"stateRoot":witness.state_root,
            "snapshotManifestSha256":witness.disk_snapshot_manifest_sha256,
            "executingCodeBlake2":witness.executing_code_blake2,
            "slot":slot,"timestamp":after.timestamp,"epoch":after.core.epoch,"session":after.core.session,
            "currentEra":after.core.current,"activeEra":after.core.active,"activeEraStart":after.core.start,
            "asyncState":after.async_report,"lastValidatorEra":after.recency_report,
            "readOnlyStateRootMatches":true});
        self.report_bytes = self
            .report_bytes
            .checked_add(serde_json::to_vec(&row).map_err(|e| e.to_string())?.len())
            .ok_or("activation report byte overflow")?;
        if self.report_bytes > state::MAX_REPORT_BYTES || replay::root(ext) != root {
            return Err("activation observation bound or read-only root differs".into());
        }
        self.previous = after.core;
        self.rows.push(row);
        Ok(())
    }

    pub(super) fn finish(self, blocks: &[BlockWitness], ordinary: usize) -> Result<Value> {
        let first = blocks
            .iter()
            .position(|b| b.number == self.number)
            .ok_or("activation block witness missing")?;
        let rows = &blocks[first..];
        if rows.len() != ordinary
            || self.rows.len() != ordinary
            || rows.is_empty()
            || rows.iter().zip(&self.rows).any(|(row, observation)| {
                row.executing_code_blake2 != hex::encode(self.code_hash)
                    || row.resulting_spec != 138
                    || row.phase != "Complete"
                    || !row.independent_import_root_matches
                    || !row.disk_restart_root_matches
                    || observation["block"] != row.number
                    || observation["blockHash"] != row.hash
                    || observation["stateRoot"] != row.state_root
                    || observation["snapshotManifestSha256"] != row.disk_snapshot_manifest_sha256
            })
        {
            return Err(
                "activation continuation differs from actual final-runtime execution".into(),
            );
        }
        let boundary = |value: Option<(u32, u32, u32)>| {
            value.map(|(block, before, after)| json!({"block":block,"before":before,"after":after}))
        };
        let followups = |value: Option<(u32, u32, u32)>| {
            value.map_or(0, |(block, _, _)| {
                rows.iter().filter(|row| row.number > block).count()
            })
        };
        Ok(json!({
            "schema":"sora-nexus-exact-post-maintenance-activation-v2",
            "policyCommitment":hex::encode(self.policy),
            "lineagePolicyCommitment":hex::encode(self.lineage),
            "executingCodeBlake2":hex::encode(self.code_hash),
            "firstBlock":self.number,"maintenanceParentHash":format!("{:#x}",self.parent),
            "maintenanceStateRoot":format!("{:#x}",self.parent_root),
            "maintenanceSnapshotSha256":self.snapshot,
            "initializedStateRoot":format!("{:#x}",self.initialized_root.ok_or("initialization not observed")?),
            "previousStakingVersion":16,"activatedStakingVersion":18,
            "cutoffEra":self.planned,"firstExecutionEra":self.active,
            "weightedPointsStartEra":self.active.saturating_add(1),
            "ordinaryBlocks":ordinary,"lastBlockHash":rows.last().unwrap().hash,
            "lastStateRoot":rows.last().unwrap().state_root,
            "maintenance":self.maintenance,
            "transitions":{"firstSession":boundary(self.first_session),"firstActiveEra":boundary(self.first_era),
                "postFirstSessionBlocks":followups(self.first_session),"postFirstActiveEraBlocks":followups(self.first_era)},
            "rows":self.rows,
            "runtimeUpgradeExecutedAsWasm":true,"independentImportAndRestartVerified":true,
            "tryRuntimeFeatureChecksExecuted":false,"fullCustomStakingVerified":false,
            "productionReady":false
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::Encode;

    fn core() -> state::Core {
        state::Core {
            slot: 6001,
            epoch: 10,
            genesis: 1,
            session: 10,
            current: 7,
            active: 6,
            start: Some(100),
        }
    }
    fn track() -> Track {
        Track {
            number: 1,
            parent: H256::zero(),
            parent_root: H256::zero(),
            snapshot: "00".repeat(32),
            policy: [1; 32],
            lineage: [1; 32],
            code_hash: [2; 32],
            spec_name: b"test".to_vec(),
            planned: 7,
            active: 6,
            initialized_root: None,
            maintenance: json!({"checkpointSession":10,"checkpointActiveEra":6}),
            previous: core(),
            rows: Vec::new(),
            report_bytes: 0,
            first_session: None,
            first_era: None,
        }
    }
    fn witness(ext: &mut sp_io::TestExternalities, number: u32) -> BlockWitness {
        BlockWitness {
            number,
            hash: format!("{:#x}", H256::repeat_byte(number as u8)),
            state_root: format!("{:#x}", replay::root(ext)),
            executing_code_blake2: hex::encode([2; 32]),
            resulting_spec: 138,
            phase: "Complete".into(),
            cursor: 0,
            operation_counter: 0,
            block_sha256: "03".repeat(32),
            disk_snapshot_manifest_sha256: "04".repeat(32),
            forbidden_extrinsics_rejected: 0,
            maintenance_session_frozen: false,
            independent_import_root_matches: true,
            disk_restart_root_matches: true,
            generated_authorization_sha256: None,
        }
    }

    #[test]
    fn missing_migration_or_wrong_era_markers_reject_activation() {
        let mut ext = sp_io::TestExternalities::default();
        state::fixture(&mut ext, &core());
        let mut track = track();
        let set =
            |ext: &mut sp_io::TestExternalities, pallet: &[u8], item: &[u8], bytes: Vec<u8>| {
                ext.execute_with(|| sp_io::storage::set(&storage_key(pallet, item), &bytes));
            };
        set(
            &mut ext,
            b"System",
            b"LastRuntimeUpgrade",
            (Compact(138u32), b"test".to_vec()).encode(),
        );
        set(
            &mut ext,
            b"Staking",
            b":__STORAGE_VERSION__:",
            16u16.encode(),
        );
        set(&mut ext, b"Staking", b"CutoffEra", 7u32.encode());
        set(&mut ext, b"Staking", b"FirstExecutionEra", 6u32.encode());
        set(
            &mut ext,
            b"Staking",
            b"WeightedPointsFormulaStartEra",
            7u32.encode(),
        );
        assert!(track.initialized(&mut ext).is_err());
        set(
            &mut ext,
            b"Staking",
            b":__STORAGE_VERSION__:",
            18u16.encode(),
        );
        set(&mut ext, b"Staking", b"CutoffEra", 8u32.encode());
        assert!(track.initialized(&mut ext).is_err());
        set(&mut ext, b"Staking", b"CutoffEra", 7u32.encode());
        let before = replay::root(&mut ext);
        track.initialized(&mut ext).unwrap();
        assert_eq!(replay::root(&mut ext), before);
        assert!(track.initialized(&mut ext).is_err());
    }

    #[test]
    fn actual_post_block_boundaries_and_followups_are_bound_to_exact_witnesses() {
        let mut ext = sp_io::TestExternalities::default();
        let mut track = track();
        track.initialized_root = Some(H256::repeat_byte(9));
        let mut after = core();
        after.slot += 600;
        after.epoch += 1;
        after.session += 1;
        after.active += 1;
        after.start = Some(200);
        state::fixture(&mut ext, &after);
        let first = witness(&mut ext, 1);
        track.accepted(&mut ext, &first, after.slot).unwrap();
        after.slot += 1;
        state::fixture(&mut ext, &after);
        let second = witness(&mut ext, 2);
        track.accepted(&mut ext, &second, after.slot).unwrap();
        let report = track.finish(&[first, second], 2).unwrap();
        assert_eq!(
            report["schema"],
            "sora-nexus-exact-post-maintenance-activation-v2"
        );
        assert_eq!(
            report["transitions"]["firstSession"],
            json!({"block":1,"before":10,"after":11})
        );
        assert_eq!(
            report["transitions"]["firstActiveEra"],
            json!({"block":1,"before":6,"after":7})
        );
        assert_eq!(report["transitions"]["postFirstSessionBlocks"], 1);
        assert_eq!(report["transitions"]["postFirstActiveEraBlocks"], 1);
        assert_eq!(report["rows"][0]["lastValidatorEra"]["active"][0]["era"], 7);
        assert_eq!(report["tryRuntimeFeatureChecksExecuted"], false);
        assert_eq!(report["fullCustomStakingVerified"], false);
    }

    #[test]
    fn ordinary_count_alone_does_not_claim_session_or_era_coverage() {
        let mut ext = sp_io::TestExternalities::default();
        let mut track = track();
        track.initialized_root = Some(H256::repeat_byte(9));
        let mut after = core();
        after.slot += 1;
        state::fixture(&mut ext, &after);
        let first = witness(&mut ext, 1);
        track.accepted(&mut ext, &first, after.slot).unwrap();
        let report = track.finish(&[first], 1).unwrap();
        assert!(report["transitions"]["firstSession"].is_null());
        assert!(report["transitions"]["firstActiveEra"].is_null());
        assert_eq!(report["transitions"]["postFirstActiveEraBlocks"], 0);
    }

    #[test]
    fn misordered_wrong_code_or_corrupt_continuation_witnesses_reject() {
        let mut ext = sp_io::TestExternalities::default();
        let mut track = track();
        track.initialized_root = Some(H256::repeat_byte(9));
        let mut after = core();
        after.slot += 1;
        state::fixture(&mut ext, &after);
        let mut row = witness(&mut ext, 2);
        assert!(track.accepted(&mut ext, &row, after.slot).is_err());
        row.number = 1;
        row.executing_code_blake2 = "05".repeat(32);
        assert!(track.accepted(&mut ext, &row, after.slot).is_err());
        row.executing_code_blake2 = hex::encode([2; 32]);
        assert!(track.accepted(&mut ext, &row, after.slot + 1).is_err());
        track.accepted(&mut ext, &row, after.slot).unwrap();
        row.disk_snapshot_manifest_sha256 = "06".repeat(32);
        assert!(track.finish(&[row], 1).is_err());
    }

    #[test]
    fn repaired_activation_binds_active_policy_without_replacing_original_lineage() {
        use common::recovery::{
            RecoveryPhase, RecoveryStatus, ACTIVE_POLICY_STORAGE_KEY, POLICY_STORAGE_KEY,
            STATUS_STORAGE_KEY,
        };
        let mut ext = sp_io::TestExternalities::default();
        let mut status = RecoveryStatus {
            format_version: 1,
            policy: [1; 32],
            phase: RecoveryPhase::Complete,
            next_chunk: 3,
            operation_counter: 2,
            failure: None,
        };
        ext.execute_with(|| {
            sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
            sp_io::storage::set(POLICY_STORAGE_KEY, &[1; 32]);
            sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &[2; 32]);
        });
        let root = replay::root(&mut ext);
        assert_eq!(active_policy(&mut ext, [1; 32]).unwrap(), [2; 32]);
        assert_eq!(replay::root(&mut ext), root);
        assert!(active_policy(&mut ext, [2; 32]).is_err());
        status.policy = [2; 32];
        ext.execute_with(|| sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode()));
        assert!(active_policy(&mut ext, [1; 32]).is_err());
        status.policy = [1; 32];
        ext.execute_with(|| {
            sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
            sp_io::storage::set(POLICY_STORAGE_KEY, &[2; 32]);
        });
        assert!(active_policy(&mut ext, [1; 32]).is_err());
    }
}
