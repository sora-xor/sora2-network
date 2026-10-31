//! Outer-bound raw-state checks for public exact-Wasm fixtures. Receipts contain
//! the policy hash, so their expected hashes cannot be part of that same policy.
use super::*;
const SCHEMA: &str = "sora-nexus-public-fixture-storage-expectations-v1";
const MAX_ROWS: usize = 4096;
const MAX_VALUE: usize = 32 * 1024 * 1024;
const MAX_TOTAL: usize = 64 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Document {
    schema: String,
    fixture_only: bool,
    policy_commitment: String,
    records: Vec<Record>,
    #[serde(default)]
    operations: Vec<Operation>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    key: String,
    value_hash: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Operation {
    index: u32,
    records: Vec<Record>,
}
pub(super) struct Expectations {
    records: Vec<(Vec<u8>, Option<[u8; 32]>)>,
    operations: BTreeMap<u32, BTreeMap<Vec<u8>, Option<[u8; 32]>>>,
}

pub(super) fn engine_owned_key(key: &[u8]) -> bool {
    key == b":extrinsic_index"
        || key == b":sora-nexus:recovery:babe-intervals:v1"
        || (!key.starts_with(&storage_key(b"System", b"Account"))
            && [
                b"System".as_slice(),
                b"Babe",
                b"Grandpa",
                b"Session",
                b"Timestamp",
            ]
            .iter()
            .any(|p| key.starts_with(&sp_io::hashing::twox_128(p))))
}
impl Expectations {
    pub fn decode(bytes: &[u8], policy: &RecoveryPolicy) -> Result<Self> {
        let d: Document = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if d.schema != SCHEMA
            || !d.fixture_only
            || hex_bytes(&d.policy_commitment)?.as_slice() != policy.commitment()
            || d.records.is_empty()
            || d.records.len() > MAX_ROWS
        {
            return Err("invalid public fixture state expectations".into());
        }
        let mut records = Vec::new();
        let mut previous: Option<Vec<u8>> = None;
        for r in d.records {
            let key = hex_bytes(&r.key)?;
            if key.is_empty()
                || key.len() > recovery::MAX_STORAGE_KEY_BYTES
                || previous.as_ref().is_some_and(|p| p >= &key)
            {
                return Err("fixture expectations require canonical unique bounded keys".into());
            }
            previous = Some(key.clone());
            let expected = r
                .value_hash
                .map(|h| {
                    hex_bytes(&h)?
                        .try_into()
                        .map_err(|_| "invalid fixture value hash".to_string())
                })
                .transpose()?;
            records.push((key, expected));
        }
        if d.operations.len() > 4096 {
            return Err("fixture operation expectation bound".into());
        }
        let mut operations = BTreeMap::new();
        for operation in d.operations {
            if operation.index as usize >= policy.chunk_hashes.len()
                || operation.records.is_empty()
                || operation.records.len() > MAX_ROWS
            {
                return Err("invalid fixture operation expectation".into());
            }
            let mut rows = BTreeMap::new();
            let mut prior: Option<Vec<u8>> = None;
            for r in operation.records {
                let key = hex_bytes(&r.key)?;
                if key.is_empty()
                    || key.len() > recovery::MAX_STORAGE_KEY_BYTES
                    || engine_owned_key(&key)
                    || prior.as_ref().is_some_and(|p| p >= &key)
                {
                    return Err(
                        "fixture operation keys must be canonical, unique and non-consensus".into(),
                    );
                }
                prior = Some(key.clone());
                let hash = r
                    .value_hash
                    .map(|h| {
                        hex_bytes(&h)?
                            .try_into()
                            .map_err(|_| "invalid fixture operation hash".to_string())
                    })
                    .transpose()?;
                rows.insert(key, hash);
            }
            if operations.insert(operation.index, rows).is_some() {
                return Err("duplicate fixture operation index".into());
            }
        }
        Ok(Self {
            records,
            operations,
        })
    }
    pub fn check_operation(
        &self,
        index: u32,
        ext: &mut sp_io::TestExternalities,
        parent: &mut sp_io::TestExternalities,
    ) -> Result<()> {
        let expected = self
            .operations
            .get(&index)
            .ok_or("dynamic recovery action lacks exact non-consensus write expectations")?;
        let keys: Vec<_> = ext
            .overlayed_changes()
            .changes()
            .map(|(key, _)| key.clone())
            .collect();
        let mut actual = BTreeMap::new();
        let mut budget = MAX_TOTAL;
        let mut hash_at =
            |state: &mut sp_io::TestExternalities, key: &[u8]| -> Result<Option<[u8; 32]>> {
                state.execute_with(|| {
                    let Some(len) = sp_io::storage::read(key, &mut [], 0) else {
                        return Ok(None);
                    };
                    if len as usize > MAX_VALUE {
                        return Err("dynamic fixture value exceeds 32 MiB".into());
                    }
                    budget = budget
                        .checked_sub(len as usize)
                        .ok_or("dynamic fixture aggregate read bound")?;
                    Ok(Some(sp_io::hashing::blake2_256(
                        &sp_io::storage::get(key).ok_or("dynamic fixture value disappeared")?,
                    )))
                })
            };
        for key in keys {
            if engine_owned_key(&key) {
                continue;
            }
            let before = hash_at(parent, &key)?;
            let after = hash_at(ext, &key)?;
            if before != after {
                actual.insert(key, after);
            }
        }
        if &actual != expected {
            return Err(format!(
                "dynamic recovery action {index} differs from its complete non-consensus write set"
            ));
        }
        let children: Vec<_> = ext
            .overlayed_changes()
            .children()
            .flat_map(|(changes, info)| {
                changes
                    .map(|(key, _)| (info.storage_key().to_vec(), key.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (child, key) in children {
            let mut child_hash =
                |state: &mut sp_io::TestExternalities| -> Result<Option<[u8; 32]>> {
                    state.execute_with(|| {
                        let Some(len) =
                            sp_io::default_child_storage::read(&child, &key, &mut [], 0)
                        else {
                            return Ok(None);
                        };
                        if len as usize > MAX_VALUE {
                            return Err("dynamic fixture child value exceeds bound".into());
                        }
                        budget = budget
                            .checked_sub(len as usize)
                            .ok_or("dynamic fixture aggregate child read bound")?;
                        Ok(Some(sp_io::hashing::blake2_256(
                            &sp_io::default_child_storage::get(&child, &key)
                                .ok_or("dynamic fixture child disappeared")?,
                        )))
                    })
                };
            let before = child_hash(parent)?;
            let after = child_hash(ext)?;
            if before != after {
                return Err("dynamic fixture action changed unapproved child storage".into());
            }
        }
        Ok(())
    }
    pub fn check(&self, ext: &mut sp_io::TestExternalities) -> Result<usize> {
        ext.execute_with(|| {
            let mut total = 0usize;
            for (key, expected) in &self.records {
                let len = sp_io::storage::read(key, &mut [], 0);
                let actual = if let Some(len) = len {
                    let len = len as usize;
                    total = total
                        .checked_add(len)
                        .filter(|n| *n <= MAX_TOTAL)
                        .ok_or("fixture expectations exceed aggregate read bound")?;
                    if len > MAX_VALUE {
                        return Err("fixture expected value exceeds 32 MiB".into());
                    }
                    let value = sp_io::storage::get(key).ok_or("fixture value disappeared")?;
                    if value.len() != len {
                        return Err("fixture value length changed".into());
                    }
                    Some(sp_io::hashing::blake2_256(&value))
                } else {
                    None
                };
                if &actual != expected {
                    return Err(format!(
                        "fixture raw state expectation failed at 0x{}",
                        hex::encode(key)
                    ));
                }
            }
            Ok(self.records.len())
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> RecoveryPolicy {
        RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![[1; 32].into()],
            chunk_hashes: vec![[2; 32]],
            audit_chunk_hashes: vec![[3; 32]],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
        }
    }
    fn document(records: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"schema":SCHEMA,"fixtureOnly":true,"policyCommitment":format!("0x{}",hex::encode(policy().commitment())),"records":records})).unwrap()
    }
    #[test]
    fn hashes_large_opaque_values_and_distinguishes_absent_from_empty() {
        let large = vec![9; recovery::MAX_CHUNK_BYTES + 1];
        let d = document(
            serde_json::json!([{"key":"0x61","valueHash":format!("0x{}",hex::encode(sp_io::hashing::blake2_256(&large)))},{"key":"0x62","valueHash":null}]),
        );
        let checks = Expectations::decode(&d, &policy()).unwrap();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| sp_io::storage::set(b"a", &large));
        assert_eq!(checks.check(&mut ext).unwrap(), 2);
        ext.execute_with(|| sp_io::storage::set(b"b", &[]));
        assert!(checks.check(&mut ext).is_err());
        ext.execute_with(|| {
            sp_io::storage::clear(b"b");
            sp_io::storage::set(b"a", &[8]);
        });
        assert!(checks.check(&mut ext).is_err());
    }
    #[test]
    fn dynamic_operation_requires_complete_exact_non_consensus_diff() {
        let mut document: serde_json::Value = serde_json::from_slice(&document(
            serde_json::json!([{"key":"0x61","valueHash":null}]),
        ))
        .unwrap();
        let account = storage_key(b"System", b"Account");
        document["operations"] = serde_json::json!([{"index":0,"records":[{"key":format!("0x{}",hex::encode(&account)),"valueHash":format!("0x{}",hex::encode(sp_io::hashing::blake2_256(&[1])))}]}]);
        let expected =
            Expectations::decode(&serde_json::to_vec(&document).unwrap(), &policy()).unwrap();
        let mut parent = sp_io::TestExternalities::default();
        let mut after = sp_io::TestExternalities::default();
        after.execute_with(|| {
            sp_io::storage::set(&account, &[1]);
            sp_io::storage::set(&storage_key(b"Babe", b"CurrentSlot"), &99u64.encode());
        });
        expected
            .check_operation(0, &mut after, &mut parent)
            .unwrap();
        after
            .execute_with(|| sp_io::storage::set(b":sora-nexus:recovery:unexpected-receipt", &[7]));
        assert!(expected
            .check_operation(0, &mut after, &mut parent)
            .is_err());
        after.execute_with(|| {
            sp_io::storage::clear(b":sora-nexus:recovery:unexpected-receipt");
            sp_io::default_child_storage::set(b"child", b"key", &[1]);
        });
        assert!(expected
            .check_operation(0, &mut after, &mut parent)
            .is_err());
        assert!(expected
            .check_operation(1, &mut after, &mut parent)
            .is_err());
    }
    #[test]
    fn rejects_duplicates_reordering_and_wrong_policy() {
        let rows =
            serde_json::json!([{"key":"0x62","valueHash":null},{"key":"0x61","valueHash":null}]);
        assert!(Expectations::decode(&document(rows), &policy()).is_err());
        let rows =
            serde_json::json!([{"key":"0x61","valueHash":null},{"key":"0x61","valueHash":null}]);
        assert!(Expectations::decode(&document(rows), &policy()).is_err());
        let mut p = policy();
        p.evidence_commitment = [6; 32];
        assert!(Expectations::decode(
            &document(serde_json::json!([{"key":"0x61","valueHash":null}])),
            &p
        )
        .is_err());
    }
}
