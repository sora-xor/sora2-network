//! Observe the real first final-runtime initialization on the completed maintenance state.
//! Only this invocation's externalities are admitted; JSON cannot construct a witness.
use super::{replay, storage_key, value, BlockWitness, Result};
use codec::{Compact, DecodeAll};
use serde_json::{json, Value};
use sp_core::H256;

fn required<T: DecodeAll>(
    ext: &mut sp_io::TestExternalities,
    pallet: &[u8],
    item: &[u8],
) -> Result<T> {
    let raw = value(ext, &storage_key(pallet, item)).ok_or("activation state value missing")?;
    if raw.len() > 4096 {
        return Err("activation state value exceeds bound".into());
    }
    T::decode_all(&mut raw.as_slice()).map_err(|e| e.to_string())
}

pub(super) struct Track {
    number: u32,
    parent: H256,
    parent_root: H256,
    snapshot: String,
    policy: [u8; 32],
    code_hash: [u8; 32],
    spec_name: Vec<u8>,
    planned: u32,
    active: u32,
    initialized_root: Option<H256>,
}

impl Track {
    pub(super) fn before(
        ext: &mut sp_io::TestExternalities,
        number: u32,
        parent: H256,
        snapshot: &str,
        policy: [u8; 32],
        code: &[u8],
        version: &sp_version::RuntimeVersion,
    ) -> Result<Self> {
        let root = replay::root(ext);
        let status = super::status(ext)?;
        let previous: (Compact<u32>, Vec<u8>) = required(ext, b"System", b"LastRuntimeUpgrade")?;
        let staking: u16 = required(ext, b"Staking", b":__STORAGE_VERSION__:")?;
        let planned: u32 = required(ext, b"Staking", b"CurrentEra")?;
        let (active, _start): (u32, Option<u64>) = required(ext, b"Staking", b"ActiveEra")?;
        if status.phase != common::recovery::RecoveryPhase::Complete
            || status.policy != policy
            || version.spec_version != 138
            || version.transaction_version != 134
            || previous.0 .0 >= 138
            || previous.1.as_slice() != version.spec_name.as_bytes()
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
            code_hash: sp_io::hashing::blake2_256(code),
            spec_name: version.spec_name.as_bytes().to_vec(),
            planned,
            active,
            initialized_root: None,
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

    pub(super) fn finish(self, blocks: &[BlockWitness], ordinary: usize) -> Result<Value> {
        let first = blocks
            .iter()
            .position(|b| b.number == self.number)
            .ok_or("activation block witness missing")?;
        let rows = &blocks[first..];
        if rows.len() != ordinary
            || rows.is_empty()
            || rows.iter().any(|row| {
                row.executing_code_blake2 != hex::encode(self.code_hash)
                    || row.resulting_spec != 138
                    || row.phase != "Complete"
                    || !row.independent_import_root_matches
                    || !row.disk_restart_root_matches
            })
        {
            return Err(
                "activation continuation differs from actual final-runtime execution".into(),
            );
        }
        Ok(json!({
            "schema":"sora-nexus-exact-post-maintenance-activation-v1",
            "policyCommitment":hex::encode(self.policy),
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

    #[test]
    fn missing_migration_or_wrong_era_markers_reject_activation() {
        let mut ext = sp_io::TestExternalities::default();
        let mut track = Track {
            number: 1,
            parent: H256::zero(),
            parent_root: H256::zero(),
            snapshot: "00".repeat(32),
            policy: [1; 32],
            code_hash: [2; 32],
            spec_name: b"test".to_vec(),
            planned: 7,
            active: 6,
            initialized_root: None,
        };
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
}
