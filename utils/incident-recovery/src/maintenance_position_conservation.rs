//! Complete-prefix owner conservation, backed only by this invocation's states.
//! Incoming movements are provisional until joined to the opaque accepted native
//! replay/audit certificate. No JSON value or arbitrary Apply edit grants a delta.
use super::{replay, BlockWitness};
use crate::{
    inventory::{Descriptor, Hasher, MetadataSchema},
    typed, Result,
};
use codec::{DecodeAll, DecodeWithMemLimit, Encode};
use common::recovery::{self, RecoveryAction, RecoveryPolicy};
use framenode_runtime::incident_recovery_incoming::IncomingOperation;
use scale_info::{TypeDef, TypeDefPrimitive};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sp_core::H256;
use std::collections::{BTreeMap, BTreeSet};

const MAX_ROWS: usize = 2_000_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;
const MAX_DELTA_ROWS: usize = 65_536;
type Id = [u8; 32];
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode)]
enum Key {
    Native(Id),
    Token(Id, Id),
    Pool(Id, Id),
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Encode)]
struct Amount {
    free: u128,
    reserved: u128,
}
type Positions = BTreeMap<Key, Amount>;
struct Snapshot {
    root: H256,
    metadata: String,
    positions: Positions,
    accounts: BTreeSet<Id>,
    rows: [usize; 3],
    bytes: usize,
}
#[derive(Encode)]
struct Delta {
    key: Key,
    before: Amount,
    after: Amount,
}
#[derive(Encode)]
struct Movement {
    operation_index: u32,
    operation: Id,
    block: u32,
    initialized_root: H256,
    operation_root: H256,
    consumed_root: H256,
    deltas: Vec<Delta>,
}
struct Pending {
    operation: IncomingOperation,
    block: u32,
    schema: MetadataSchema,
    before: Snapshot,
    after: Option<Snapshot>,
}
struct Inspection {
    snapshot: Snapshot,
    block: u32,
    hash: String,
    disk: String,
}
/// Fields remain private. No Deserialize/JSON constructor can fabricate this journal.
pub(super) struct Track {
    policy: Id,
    mapping: BTreeMap<Id, Id>,
    checkpoint: Snapshot,
    pending: Option<Pending>,
    movements: Vec<Movement>,
    delta_rows: usize,
    inspection: Option<Inspection>,
    fixture: bool,
}
fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(why.into())
    }
}
fn account(who: &sp_core::crypto::AccountId32) -> Id {
    *AsRef::<Id>::as_ref(who)
}
fn id(raw: &[u8]) -> Result<Id> {
    raw.try_into()
        .map_err(|_| "position identity width differs".into())
}
fn prefix(name: &str) -> Vec<u8> {
    let (pallet, item) = name.split_once('.').expect("fixed position prefix");
    [
        sp_io::hashing::twox_128(pallet.as_bytes()),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}
fn descriptor<'a>(
    schema: &'a MetadataSchema,
    name: &str,
    hashers: &[Hasher],
) -> Result<&'a Descriptor> {
    let found: Vec<_> = schema
        .descriptors
        .iter()
        .filter(|d| d.name == name)
        .collect();
    need(found.len() == 1, "position descriptor absent or duplicated")?;
    let d = found[0];
    need(
        d.trie == "top"
            && d.prefix == prefix(name)
            && d.hashers == hashers
            && d.key_types.len() == hashers.len(),
        "position descriptor identity/hasher differs",
    )?;
    Ok(d)
}
fn concat(raw: &[u8], hasher: Hasher) -> Result<Id> {
    let width = match hasher {
        Hasher::Identity => 0,
        Hasher::Blake2_128Concat => 16,
        Hasher::Twox64Concat => 8,
        _ => return Err("unsupported position hasher".into()),
    };
    need(raw.len() == width + 32, "position key width differs")?;
    let value = id(&raw[width..])?;
    need(hasher.apply(&value) == raw, "position key hash differs")?;
    Ok(value)
}
fn schema_for(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
) -> Result<MetadataSchema> {
    let root = replay::root(ext);
    let raw = Vec::<u8>::decode_all(
        &mut replay::call(executor, ext, code, "Metadata_metadata", &[])?.as_slice(),
    )
    .map_err(|e| e.to_string())?;
    need(
        replay::root(ext) == root,
        "position metadata query changed state",
    )?;
    MetadataSchema::decode(&raw)
}
fn snapshot(ext: &mut sp_io::TestExternalities, schema: &MetadataSchema) -> Result<Snapshot> {
    let root = replay::root(ext);
    let mut result = Snapshot {
        root,
        metadata: schema.metadata_hash.clone(),
        positions: Positions::new(),
        accounts: BTreeSet::new(),
        rows: [0; 3],
        bytes: 0,
    };
    ext.execute_with(|| -> Result<()> {
        for (kind, name, hashers) in [
            (0, "System.Account", vec![Hasher::Blake2_128Concat]),
            (
                1,
                "Tokens.Accounts",
                vec![Hasher::Blake2_128Concat, Hasher::Twox64Concat],
            ),
            (
                2,
                "PoolXYK.PoolProviders",
                vec![Hasher::Identity, Hasher::Identity],
            ),
        ] {
            let d = descriptor(schema, name, &hashers)?;
            need(
                typed::account_type(&schema.registry, d.key_types[0]),
                "position owner metadata differs",
            )?;
            if kind == 1 {
                need(
                    schema.registry.resolve(d.key_types[1]).is_some_and(|t| {
                        t.path.segments.last().is_some_and(|n| n == "AssetId32")
                            && t.path.segments.iter().any(|n| n == "common")
                    }),
                    "position asset metadata differs",
                )?;
            } else if kind == 2 {
                need(
                    typed::account_type(&schema.registry, d.key_types[1])
                        && matches!(
                            schema.registry.resolve(d.value_type).map(|t| &t.type_def),
                            Some(TypeDef::Primitive(TypeDefPrimitive::U128))
                        ),
                    "pool position metadata differs",
                )?;
            }
            let mut next = if sp_io::storage::exists(&d.prefix) {
                Some(d.prefix.clone())
            } else {
                sp_io::storage::next_key(&d.prefix)
            };
            while let Some(key) = next.filter(|k| k.starts_with(&d.prefix)) {
                let length = sp_io::storage::read(&key, &mut [], 0)
                    .ok_or("position row disappeared")? as usize;
                need(
                    length <= 4096 && key.len() <= 128,
                    "position row byte bound",
                )?;
                result.rows[kind] = result.rows[kind]
                    .checked_add(1)
                    .ok_or("position count overflow")?;
                result.bytes = result
                    .bytes
                    .checked_add(key.len())
                    .and_then(|n| n.checked_add(length))
                    .ok_or("position byte overflow")?;
                need(
                    result.rows.iter().sum::<usize>() <= MAX_ROWS && result.bytes <= MAX_BYTES,
                    "position prefix scan bound",
                )?;
                let raw = sp_io::storage::get(&key)
                    .ok_or("position row disappeared")?
                    .to_vec();
                need(raw.len() == length, "position row changed while reading")?;
                typed::decode_exact(&raw, d.value_type, &schema.registry)?;
                let suffix = &key[d.prefix.len()..];
                let (position, amount) = match kind {
                    0 => {
                        let owner = concat(suffix, Hasher::Blake2_128Concat)?;
                        result.accounts.insert(owner);
                        (
                            Key::Native(owner),
                            Amount {
                                free: typed::u128_field(
                                    &raw,
                                    d.value_type,
                                    &schema.registry,
                                    &["data", "free"],
                                )?
                                .0,
                                reserved: typed::u128_field(
                                    &raw,
                                    d.value_type,
                                    &schema.registry,
                                    &["data", "reserved"],
                                )?
                                .0,
                            },
                        )
                    }
                    1 => {
                        need(suffix.len() == 88, "token position key width")?;
                        let owner = concat(&suffix[..48], Hasher::Blake2_128Concat)?;
                        let asset = concat(&suffix[48..], Hasher::Twox64Concat)?;
                        result.accounts.insert(owner);
                        (
                            Key::Token(owner, asset),
                            Amount {
                                free: typed::u128_field(
                                    &raw,
                                    d.value_type,
                                    &schema.registry,
                                    &["free"],
                                )?
                                .0,
                                reserved: typed::u128_field(
                                    &raw,
                                    d.value_type,
                                    &schema.registry,
                                    &["reserved"],
                                )?
                                .0,
                            },
                        )
                    }
                    _ => {
                        need(
                            suffix.len() == 64 && raw.len() == 16,
                            "pool position key/value width",
                        )?;
                        let pool = id(&suffix[..32])?;
                        let owner = id(&suffix[32..])?;
                        result.accounts.insert(pool);
                        result.accounts.insert(owner);
                        (
                            Key::Pool(pool, owner),
                            Amount {
                                free: u128::from_le_bytes(
                                    raw.as_slice().try_into().map_err(|_| "pool amount width")?,
                                ),
                                reserved: 0,
                            },
                        )
                    }
                };
                // Zero account rows carry lifecycle/ref-count state, not an owned amount.
                // They are fully scanned and still participate in destination non-use.
                if amount != Amount::default() {
                    need(
                        result.positions.insert(position, amount).is_none(),
                        "duplicate financial position",
                    )?;
                }
                next = sp_io::storage::next_key(&key);
                need(
                    next.as_ref().is_none_or(|new| new > &key),
                    "nonmonotonic position scan",
                )?;
            }
        }
        Ok(())
    })?;
    need(replay::root(ext) == root, "position scan changed state")?;
    Ok(result)
}
fn owner(key: &Key) -> Id {
    match key {
        Key::Native(who) | Key::Token(who, _) | Key::Pool(_, who) => *who,
    }
}
fn mapped(key: &Key, mapping: &BTreeMap<Id, Id>) -> Key {
    let map = |who: &Id| mapping.get(who).copied().unwrap_or(*who);
    match key {
        Key::Native(who) => Key::Native(map(who)),
        Key::Token(who, asset) => Key::Token(map(who), *asset),
        Key::Pool(pool, who) => Key::Pool(map(pool), map(who)),
    }
}
fn remap(before: &Snapshot, mapping: &BTreeMap<Id, Id>) -> Result<Positions> {
    need(
        mapping.values().all(|new| !before.accounts.contains(new)),
        "replacement destination has an existing financial row",
    )?;
    let mut result = Positions::new();
    for (key, amount) in &before.positions {
        need(
            owner(key) != recovery::ATTACKER_ACCOUNT,
            "positive attacker checkpoint position requires a separate typed economic disposition",
        )?;
        need(
            result.insert(mapped(key, mapping), *amount).is_none(),
            "mapped financial position collision",
        )?;
    }
    Ok(result)
}
fn delta(before: &Positions, after: &Positions) -> Result<Vec<Delta>> {
    let keys: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    let mut out = Vec::new();
    for key in keys {
        let a = before.get(&key).copied().unwrap_or_default();
        let b = after.get(&key).copied().unwrap_or_default();
        if a != b {
            need(
                owner(&key) != recovery::ATTACKER_ACCOUNT,
                "incoming changed an attacker-owned position",
            )?;
            need(
                !matches!(key, Key::Pool(..)),
                "incoming execution changed LP ownership shares",
            )?;
            need(
                out.len() < MAX_DELTA_ROWS,
                "incoming position delta row bound",
            )?;
            out.push(Delta {
                key,
                before: a,
                after: b,
            });
        }
    }
    Ok(out)
}
fn movement(value: u128, before: u128, after: u128) -> Result<u128> {
    if after >= before {
        value
            .checked_add(after - before)
            .ok_or("owner position credit overflow".into())
    } else {
        value
            .checked_sub(before - after)
            .ok_or("owner position debit exceeds preserved entitlement".into())
    }
}
fn apply(expected: &mut Positions, mapping: &BTreeMap<Id, Id>, changes: &[Delta]) -> Result<()> {
    for change in changes {
        let key = mapped(&change.key, mapping);
        let value = expected.get(&key).copied().unwrap_or_default();
        let updated = Amount {
            free: movement(value.free, change.before.free, change.after.free)?,
            reserved: movement(
                value.reserved,
                change.before.reserved,
                change.after.reserved,
            )?,
        };
        if updated == Amount::default() {
            expected.remove(&key);
        } else {
            expected.insert(key, updated);
        }
    }
    Ok(())
}
fn reconcile(expected: &Positions, actual: &Positions) -> Result<()> {
    need(expected == actual, "per-owner balance/reserve/LP conservation differs; unexplained changes need a typed disposition")
}
fn digest(value: &Positions) -> String {
    hex::encode(Sha256::digest(value.encode()))
}
impl Track {
    pub(super) fn checkpoint(
        ext: &mut sp_io::TestExternalities,
        schema: &MetadataSchema,
        policy: &RecoveryPolicy,
        fixture: bool,
    ) -> Result<Self> {
        policy
            .validate()
            .map_err(|e| format!("position policy: {e:?}"))?;
        let checkpoint = snapshot(ext, schema)?;
        need(
            checkpoint.root == H256::from(recovery::CHECKPOINT_STATE_ROOT),
            "position checkpoint root differs",
        )?;
        let mapping = policy
            .replacements
            .iter()
            .map(|p| (account(&p.old), account(&p.new)))
            .collect();
        remap(&checkpoint, &mapping)?;
        Ok(Self {
            policy: policy.commitment(),
            mapping,
            checkpoint,
            pending: None,
            movements: Vec::new(),
            delta_rows: 0,
            inspection: None,
            fixture,
        })
    }
    pub(super) fn before_work(
        &mut self,
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        action: &RecoveryAction,
        block: u32,
        spec: u32,
    ) -> Result<()> {
        let RecoveryAction::Incoming(raw) = action else {
            return Ok(());
        };
        need(
            spec == 134 && self.pending.is_none() && self.inspection.is_none(),
            "position incoming outside original maintenance phase",
        )?;
        let mut input = raw.as_slice();
        let operation =
            IncomingOperation::decode_with_mem_limit(&mut input, recovery::MAX_CHUNK_BYTES * 4)
                .map_err(|e| e.to_string())?;
        need(
            input.is_empty() && operation.encode().as_slice() == raw.as_slice(),
            "noncanonical position incoming envelope",
        )?;
        let schema = schema_for(executor, ext, code)?;
        let before = snapshot(ext, &schema)?;
        self.pending = Some(Pending {
            operation,
            block,
            schema,
            before,
            after: None,
        });
        Ok(())
    }
    pub(super) fn after_work(&mut self, ext: &mut sp_io::TestExternalities) -> Result<()> {
        if let Some(pending) = &mut self.pending {
            need(
                pending.after.is_none(),
                "duplicate incoming position after-state",
            )?;
            pending.after = Some(snapshot(ext, &pending.schema)?);
        }
        Ok(())
    }
    pub(super) fn accepted(
        &mut self,
        ext: &mut sp_io::TestExternalities,
        witness: &BlockWitness,
    ) -> Result<()> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        let after = pending
            .after
            .ok_or("incoming position after-state absent")?;
        let restored = snapshot(ext, &pending.schema)?;
        need(
            witness.number == pending.block
                && witness.state_root == format!("{:#x}", restored.root)
                && witness.independent_import_root_matches
                && witness.disk_restart_root_matches
                && restored.positions == after.positions,
            "incoming restored positions/block differ from native operation state",
        )?;
        let changes = delta(&pending.before.positions, &after.positions)?;
        self.delta_rows = self
            .delta_rows
            .checked_add(changes.len())
            .ok_or("position delta count overflow")?;
        need(
            self.delta_rows <= MAX_DELTA_ROWS
                && self.movements.len() < recovery::MAX_WORK_CHUNKS
                && self
                    .movements
                    .last()
                    .is_none_or(|p| p.block < witness.number),
            "incoming position journal bound/order",
        )?;
        self.movements.push(Movement {
            operation_index: pending.operation.index,
            operation: pending.operation.commitment(),
            block: witness.number,
            initialized_root: pending.before.root,
            operation_root: after.root,
            consumed_root: restored.root,
            deltas: changes,
        });
        Ok(())
    }
    pub(super) fn inspect(
        &mut self,
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        witness: &BlockWitness,
    ) -> Result<()> {
        need(
            self.pending.is_none()
                && self.inspection.is_none()
                && witness.resulting_spec == 138
                && witness.phase == "Complete"
                && witness.executing_code_blake2 == hex::encode(sp_io::hashing::blake2_256(code)),
            "position inspection requires one executed138 state",
        )?;
        let schema = schema_for(executor, ext, code)?;
        let snapshot = snapshot(ext, &schema)?;
        need(
            witness.state_root == format!("{:#x}", snapshot.root)
                && witness.independent_import_root_matches
                && witness.disk_restart_root_matches,
            "position inspection import/root differs",
        )?;
        self.inspection = Some(Inspection {
            snapshot,
            block: witness.number,
            hash: witness.hash.clone(),
            disk: witness.disk_snapshot_manifest_sha256.clone(),
        });
        Ok(())
    }
    pub(super) fn finish(
        self,
        policy: &RecoveryPolicy,
        terminal_root: H256,
        incoming: &crate::incoming_replay::VerifiedReplay,
    ) -> Result<Value> {
        need(
            policy.commitment() == self.policy && self.pending.is_none(),
            "position final policy or pending operation differs",
        )?;
        let inspected = self
            .inspection
            .ok_or("position executed138 inspection absent")?;
        let mut accepted = BTreeMap::new();
        for event in &self.movements {
            need(
                event.block <= inspected.block
                    && accepted
                        .insert(
                            event.operation_index,
                            (
                                event.operation,
                                event.block,
                                event.initialized_root,
                                event.consumed_root,
                            ),
                        )
                        .is_none(),
                "duplicate or post-inspection position movement",
            )?;
        }
        incoming.verify_position_consumptions(
            policy,
            terminal_root,
            self.fixture,
            inspected.block,
            &accepted,
        )?;
        let mut expected = remap(&self.checkpoint, &self.mapping)?;
        for event in &self.movements {
            apply(&mut expected, &self.mapping, &event.deltas)?;
        }
        reconcile(&expected, &inspected.snapshot.positions)?;
        Ok(
            json!({"schema":"sora-nexus-owner-position-conservation-v1","fixtureOnly":self.fixture,
            "checkpointStateRoot":format!("{:#x}",self.checkpoint.root),"checkpointMetadataHash":self.checkpoint.metadata,
            "inspectionStateRoot":format!("{:#x}",inspected.snapshot.root),"inspectionBlock":inspected.block,
            "inspectionBlockHash":inspected.hash,"inspectionSnapshotManifestSha256":inspected.disk,
            "finalMetadataHash":inspected.snapshot.metadata,"certificateTerminalStateRoot":format!("{terminal_root:#x}"),
            "policyCommitment":hex::encode(self.policy),"replacementCount":self.mapping.len(),
            "checkpointRows":self.checkpoint.rows,"inspectionRows":inspected.snapshot.rows,
            "checkpointBytes":self.checkpoint.bytes,"inspectionBytes":inspected.snapshot.bytes,
            "checkpointPositionSha256":digest(&self.checkpoint.positions),"expectedPositionSha256":digest(&expected),
            "inspectionPositionSha256":digest(&inspected.snapshot.positions),"incomingOperations":self.movements.len(),
            "incomingDeltaRows":self.delta_rows,"incomingJournalSha256":hex::encode(Sha256::digest(self.movements.encode())),
            "allThreePrefixesEnumerated":true,"replacementFinancialDestinationsUnused":true,"freeReservedAndLpOwnershipConserved":true,
            "nativeIncomingMovementsOpaqueVerified":true,"genericStorageEditAllowanceUsed":false,
            "attackerDispositionsVerified":false,"debtAndCollateralConservationVerified":false,
            "derivedOwnershipComplete":false,"nativeIncomingEconomicsIndependentlyVerified":false,
            "accountLifecycleFieldsConserved":false,"completeFinancialCoverageVerified":false,
            "completeStorageSemantics":false,"productionReady":false}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry, TypeInfo};
    #[derive(Encode, TypeInfo)]
    struct Funds {
        free: u128,
        reserved: u128,
    }
    #[derive(Encode, TypeInfo)]
    struct Native {
        data: Funds,
    }
    fn fixture_schema() -> MetadataSchema {
        fixture_schema_account::<Native>()
    }
    fn fixture_schema_account<A: TypeInfo + 'static>() -> MetadataSchema {
        let mut r = Registry::new();
        let who = r
            .register_type(&meta_type::<sp_core::crypto::AccountId32>())
            .id;
        let asset = r
            .register_type(&meta_type::<common::AssetId32<common::PredefinedAssetId>>())
            .id;
        let native = r.register_type(&meta_type::<A>()).id;
        let funds = r.register_type(&meta_type::<Funds>()).id;
        let balance = r.register_type(&meta_type::<u128>()).id;
        MetadataSchema {
            registry: r.into(),
            metadata_hash: format!("0x{}", "33".repeat(32)),
            descriptors: vec![
                Descriptor {
                    trie: "top".into(),
                    name: "System.Account".into(),
                    prefix: prefix("System.Account"),
                    key_types: vec![who],
                    hashers: vec![Hasher::Blake2_128Concat],
                    value_type: native,
                },
                Descriptor {
                    trie: "top".into(),
                    name: "Tokens.Accounts".into(),
                    prefix: prefix("Tokens.Accounts"),
                    key_types: vec![who, asset],
                    hashers: vec![Hasher::Blake2_128Concat, Hasher::Twox64Concat],
                    value_type: funds,
                },
                Descriptor {
                    trie: "top".into(),
                    name: "PoolXYK.PoolProviders".into(),
                    prefix: prefix("PoolXYK.PoolProviders"),
                    key_types: vec![who, who],
                    hashers: vec![Hasher::Identity, Hasher::Identity],
                    value_type: balance,
                },
            ],
        }
    }
    fn sample() -> Snapshot {
        Snapshot {
            root: H256::zero(),
            metadata: String::new(),
            positions: BTreeMap::from([
                (
                    Key::Native([1; 32]),
                    Amount {
                        free: 10,
                        reserved: 5,
                    },
                ),
                (
                    Key::Token([1; 32], [9; 32]),
                    Amount {
                        free: 20,
                        reserved: 3,
                    },
                ),
                (
                    Key::Pool([8; 32], [1; 32]),
                    Amount {
                        free: 100,
                        reserved: 0,
                    },
                ),
                (
                    Key::Native([3; 32]),
                    Amount {
                        free: 50,
                        reserved: 0,
                    },
                ),
            ]),
            accounts: BTreeSet::from([[1; 32], [3; 32], [8; 32]]),
            rows: [2, 1, 1],
            bytes: 200,
        }
    }
    #[test]
    fn full_typed_position_scan_keeps_native_token_lp_and_zero_destination_rows() {
        let schema = fixture_schema();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(
                &[
                    prefix("System.Account"),
                    Hasher::Blake2_128Concat.apply(&[1; 32]),
                ]
                .concat(),
                &Native {
                    data: Funds {
                        free: 10,
                        reserved: 5,
                    },
                }
                .encode(),
            );
            sp_io::storage::set(
                &[
                    prefix("System.Account"),
                    Hasher::Blake2_128Concat.apply(&[2; 32]),
                ]
                .concat(),
                &Native {
                    data: Funds {
                        free: 0,
                        reserved: 0,
                    },
                }
                .encode(),
            );
            sp_io::storage::set(
                &[
                    prefix("Tokens.Accounts"),
                    Hasher::Blake2_128Concat.apply(&[1; 32]),
                    Hasher::Twox64Concat.apply(&[9; 32]),
                ]
                .concat(),
                &Funds {
                    free: 20,
                    reserved: 3,
                }
                .encode(),
            );
            sp_io::storage::set(
                &[prefix("PoolXYK.PoolProviders"), vec![8; 32], vec![1; 32]].concat(),
                &100u128.encode(),
            );
        });
        let before = replay::root(&mut ext);
        let observed = snapshot(&mut ext, &schema).unwrap();
        assert_eq!(replay::root(&mut ext), before);
        assert_eq!(observed.rows, [2, 1, 1]);
        assert_eq!(observed.positions.len(), 3);
        assert_eq!(
            observed.positions[&Key::Token([1; 32], [9; 32])],
            Amount {
                free: 20,
                reserved: 3
            }
        );
        assert!(observed.accounts.contains(&[2; 32]));
        assert!(remap(&observed, &BTreeMap::from([([1; 32], [2; 32])])).is_err());
    }
    #[test]
    fn native_funded_registration_reference_rows_do_not_create_balance_allowance() {
        use framenode_runtime::{Runtime, RuntimeOrigin, System};
        let schema = fixture_schema_account::<framenode_runtime::AccountData>();
        let mut ext = sp_io::TestExternalities::default();
        let old = sp_core::crypto::AccountId32::new([1; 32]);
        ext.execute_with(|| {
            frame_system::Account::<Runtime>::mutate(&old, |account| {
                account.data.free = 99;
                account.data.reserved = 7;
            });
        });
        let before = snapshot(&mut ext, &schema).unwrap();
        let peers: Vec<_> = (11..14)
            .map(|n| sp_core::crypto::AccountId32::new([n; 32]))
            .collect();
        ext.execute_with(|| {
            for peer in &peers {
                System::inc_providers(peer);
            }
            // These are the funded fixture's native account-creation operations.
            // Registration of its existing asset and scoped permission adds no
            // System.Account/Tokens.Accounts/PoolProviders economic writes.
            framenode_runtime::EthBridge::register_bridge(
                RuntimeOrigin::root(),
                sp_core::H160::repeat_byte(7),
                peers,
                eth_bridge::BridgeSignatureVersion::V3,
            )
            .unwrap();
        });
        let after = snapshot(&mut ext, &schema).unwrap();
        assert_eq!(after.rows[0], before.rows[0] + 4);
        assert_ne!(before.root, after.root);
        reconcile(&before.positions, &after.positions).unwrap();
        // Neither a public key nor a native-derived configuration edit grants
        // a financial exception. The completed-state equation rejects it.
        ext.execute_with(|| frame_system::Account::<Runtime>::mutate(&old, |a| a.data.free += 1));
        assert!(reconcile(
            &before.positions,
            &snapshot(&mut ext, &schema).unwrap().positions
        )
        .is_err());
    }
    #[test]
    fn wrong_concat_and_wrong_metadata_hashers_fail_closed() {
        let mut schema = fixture_schema();
        let mut ext = sp_io::TestExternalities::default();
        let mut key = [
            prefix("System.Account"),
            Hasher::Blake2_128Concat.apply(&[1; 32]),
        ]
        .concat();
        key[32] ^= 1;
        ext.execute_with(|| {
            sp_io::storage::set(
                &key,
                &Native {
                    data: Funds {
                        free: 10,
                        reserved: 5,
                    },
                }
                .encode(),
            )
        });
        assert!(snapshot(&mut ext, &schema).is_err());
        schema.descriptors[0].hashers = vec![Hasher::Identity];
        assert!(snapshot(&mut ext, &schema).is_err());
    }
    #[test]
    fn mapping_preserves_each_owner_and_does_not_accept_same_total_theft_or_reserve_release() {
        let mapping = BTreeMap::from([([1; 32], [2; 32])]);
        let expected = remap(&sample(), &mapping).unwrap();
        assert!(expected.contains_key(&Key::Pool([8; 32], [2; 32])));
        assert!(!expected.contains_key(&Key::Native([1; 32])));
        let mut stolen = expected.clone();
        stolen.get_mut(&Key::Native([2; 32])).unwrap().free -= 1;
        stolen.get_mut(&Key::Native([3; 32])).unwrap().free += 1;
        assert!(reconcile(&expected, &stolen).is_err());
        let mut released = expected.clone();
        let funds = released.get_mut(&Key::Native([2; 32])).unwrap();
        funds.free += 1;
        funds.reserved -= 1;
        assert!(reconcile(&expected, &released).is_err());
        let mut removed = expected.clone();
        removed.remove(&Key::Pool([8; 32], [2; 32]));
        assert!(reconcile(&expected, &removed).is_err());
    }
    #[test]
    fn only_exact_observed_incoming_deltas_can_adjust_expected_positions() {
        let mapping = BTreeMap::from([([1; 32], [2; 32])]);
        let mut expected = remap(&sample(), &mapping).unwrap();
        let mut actual = expected.clone();
        actual.get_mut(&Key::Native([2; 32])).unwrap().free += 7;
        let changes = delta(&expected, &actual).unwrap();
        apply(&mut expected, &mapping, &changes).unwrap();
        reconcile(&expected, &actual).unwrap();
        actual.get_mut(&Key::Native([2; 32])).unwrap().free += 1;
        assert!(reconcile(&expected, &actual).is_err());
        assert!(movement(u128::MAX, 0, 1).is_err());
        assert!(movement(0, 1, 0).is_err());
    }
    #[test]
    fn positive_attacker_positions_and_incoming_lp_or_attacker_changes_need_disposition() {
        let mut before = sample();
        before.positions.insert(
            Key::Token(recovery::ATTACKER_ACCOUNT, [9; 32]),
            Amount {
                free: 1,
                reserved: 0,
            },
        );
        assert!(remap(&before, &BTreeMap::new()).is_err());
        let before = sample().positions;
        let mut after = before.clone();
        after.get_mut(&Key::Pool([8; 32], [1; 32])).unwrap().free += 1;
        assert!(delta(&before, &after).is_err());
        let mut after = before.clone();
        after.insert(
            Key::Native(recovery::ATTACKER_ACCOUNT),
            Amount {
                free: 1,
                reserved: 0,
            },
        );
        assert!(delta(&before, &after).is_err());
    }
}
