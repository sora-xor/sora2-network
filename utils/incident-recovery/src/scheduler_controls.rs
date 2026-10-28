//! Cancel the six retained, stale Root scheduler authorizations without calling
//! Scheduler::cancel/Preimage::drop. Those native paths may migrate deposits,
//! unreserve funds, decrement another request or delete a shared preimage.
use crate::{
    hex_bytes,
    inventory::{
        AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::Encode;
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StoragePrefixWitness, StorageScope,
    StorageWitness,
};
use serde::{Deserialize, Serialize};
use sp_core::storage::Storage;
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const BLOCKS: [u32; 6] = [
    12_646_428, 15_823_193, 17_832_069, 25_769_290, 25_811_959, 25_852_498,
];
const MAX_RECORDS: usize = 128;
const MAX_PRESERVED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub records: Vec<Record>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    pub value_hash: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskReport {
    pub when: u32,
    pub name: String,
    pub call_hash: String,
    pub declared_call_bytes: u32,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub cancelled: Vec<TaskReport>,
    pub preserved_records: usize,
    pub preserved_preimage_providers: Vec<String>,
    pub empty_prefixes: Vec<String>,
    pub native_multisig_pending: usize,
    pub historical_calls_executed: bool,
    pub refunds_created: u32,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub empty_prefixes: Vec<StoragePrefixWitness>,
    pub report: Report,
    protected: BTreeMap<Vec<u8>, Option<Vec<u8>>>,
    protected_prefixes: Vec<Vec<u8>>,
}
impl Compilation {
    /// The complete static write set must preserve retained preimage namespaces
    /// and deposit backing. Dynamic financial work still needs exact execution.
    pub fn verify_preservation(&self, groups: &[Vec<GuardedStorageEdit>]) -> Result<()> {
        for edit in groups
            .iter()
            .flatten()
            .filter(|edit| edit.scope == StorageScope::Top)
        {
            if self
                .protected_prefixes
                .iter()
                .any(|prefix| edit.key.starts_with(prefix))
                || self
                    .protected
                    .get(&edit.key)
                    .is_some_and(|before| &edit.after != before)
            {
                return Err("scheduler cancellation overlaps protected preimage/deposit custody; separate reviewed provider adapter required".into());
            }
        }
        Ok(())
    }
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn desc<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    let d = schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing scheduler custody descriptor {name}"))?;
    let (pallet, item) = name.split_once('.').ok_or("invalid descriptor name")?;
    if d.prefix
        != [
            sp_io::hashing::twox_128(pallet.as_bytes()),
            sp_io::hashing::twox_128(item.as_bytes()),
        ]
        .concat()
    {
        return Err("scheduler descriptor prefix changed".into());
    }
    Ok(d)
}
fn rows<'a>(
    storage: &'a Storage,
    prefix: &'a [u8],
) -> impl Iterator<Item = (&'a Vec<u8>, &'a Vec<u8>)> {
    storage
        .top
        .range(prefix.to_vec()..)
        .take_while(move |(key, _)| key.starts_with(prefix))
}
fn agenda_key(d: &Descriptor, when: u32) -> Vec<u8> {
    [
        d.prefix.clone(),
        sp_io::hashing::twox_64(&when.encode()).to_vec(),
        when.encode(),
    ]
    .concat()
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    recovery::validate_replacements(replacements)
        .map_err(|error| format!("invalid scheduler mapping: {error:?}"))?;
    let evidence = hex_bytes(&review.evidence_hash)?;
    if schema.metadata_hash != METADATA
        || evidence.len() != 32
        || evidence.iter().all(|byte| *byte == 0)
        || review.records.is_empty()
        || review.records.len() > MAX_RECORDS
    {
        return Err(
            "scheduler requires pinned checkpoint metadata and bounded nonempty evidence".into(),
        );
    }
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|pair| {
            (
                *AsRef::<[u8; 32]>::as_ref(&pair.old),
                *AsRef::<[u8; 32]>::as_ref(&pair.new),
            )
        })
        .collect();
    let mut reviewed = BTreeMap::new();
    for record in &review.records {
        let key = hex_bytes(&record.key)?;
        let expected = hex_bytes(&record.value_hash)?;
        if expected.len() != 32
            || reviewed
                .insert(key, (record.storage.as_str(), expected))
                .is_some()
        {
            return Err("duplicate/invalid scheduler evidence row".into());
        }
    }
    let mut used = BTreeSet::new();
    let mut bind = |d: &Descriptor, key: &[u8], value: &[u8]| -> Result<()> {
        if reviewed
            .get(key)
            .is_none_or(|(name, expected)| *name != d.name || expected.as_slice() != hash(value))
        {
            return Err(format!(
                "missing exact scheduler custody evidence for {}",
                d.name
            ));
        }
        used.insert(key.to_vec());
        Ok(())
    };
    let agenda = desc(schema, "Scheduler.Agenda")?;
    let incomplete = desc(schema, "Scheduler.IncompleteSince")?;
    if agenda.hashers != [Hasher::Twox64Concat]
        || agenda.key_types.len() != 1
        || !incomplete.hashers.is_empty()
    {
        return Err("unreviewed scheduler storage layout".into());
    }
    let expected_keys: BTreeSet<_> = BLOCKS
        .iter()
        .map(|when| agenda_key(agenda, *when))
        .collect();
    let actual_keys: BTreeSet<_> = rows(storage, &agenda.prefix)
        .map(|(key, _)| key.clone())
        .collect();
    if actual_keys != expected_keys {
        return Err("scheduler agenda inventory is not the complete six checkpoint rows".into());
    }
    let mut empty_prefixes = Vec::new();
    for name in [
        "Scheduler.Agenda",
        "Scheduler.Lookup",
        "Scheduler.Retries",
        "Multisig.Multisigs",
    ] {
        let d = desc(schema, name)?;
        if name != "Scheduler.Agenda" && rows(storage, &d.prefix).next().is_some() {
            return Err(format!("unexpected pending authorization in {name}"));
        }
        empty_prefixes.push(StoragePrefixWitness {
            scope: StorageScope::Top,
            prefix: d.prefix.clone(),
        });
    }
    // A new native multisig map cannot be silently omitted by an empty-prefix claim.
    if schema.descriptors.iter().any(|d| {
        d.name.starts_with("Multisig.")
            && !["Multisig.Multisigs", "Multisig.__STORAGE_VERSION__"].contains(&d.name.as_str())
    }) {
        return Err("unreviewed native multisig storage".into());
    }
    empty_prefixes.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    let mut tasks = Vec::new();
    let mut hashes = BTreeSet::new();
    let mut changes = Vec::new();
    for when in BLOCKS {
        let key = agenda_key(agenda, when);
        let value = storage.top.get(&key).ok_or("missing scheduler agenda")?;
        typed::decode_exact(value, agenda.value_type, &schema.registry)?;
        // Pinned SCALE layout: exactly one Some named, nonperiodic Root task;
        // Lookup carries only a hash/length. Never decode its historical Call.
        if value.len() != 76
            || value[..3] != [4, 1, 1]
            || value[35..37] != [63, 2]
            || value[73..] != [0, 0, 0]
            || value[3..11] != *b"democrac"
        {
            return Err("unreviewed scheduler task/origin/periodic/call layout".into());
        }
        bind(agenda, &key, value)?;
        let call_hash: [u8; 32] = value[37..69]
            .try_into()
            .map_err(|_| "scheduler call hash width")?;
        hashes.insert(call_hash);
        tasks.push(TaskReport {
            when,
            name: hx(&value[3..35]),
            call_hash: hx(&call_hash),
            declared_call_bytes: u32::from_le_bytes(
                value[69..73]
                    .try_into()
                    .map_err(|_| "scheduler call length")?,
            ),
        });
        changes.push((agenda, key, value.clone()));
    }
    let since = storage
        .top
        .get(&incomplete.prefix)
        .ok_or("missing scheduler scan cursor")?;
    typed::decode_exact(since, incomplete.value_type, &schema.registry)?;
    if since != &(recovery::CHECKPOINT_NUMBER + 1).encode() {
        return Err("scheduler cursor differs from checkpoint+1".into());
    }
    bind(incomplete, &incomplete.prefix, since)?;
    changes.push((incomplete, incomplete.prefix.clone(), since.clone()));
    let mut protected = BTreeMap::new();
    let mut protected_prefixes = Vec::new();
    let mut providers = BTreeSet::new();
    let mut preserved_bytes = 0usize;
    for name in [
        "Preimage.StatusFor",
        "Preimage.RequestStatusFor",
        "Preimage.PreimageFor",
    ] {
        let d = desc(schema, name)?;
        if d.hashers != [Hasher::Identity] || d.key_types.len() != 1 {
            return Err("unreviewed preimage custody key codec".into());
        }
        for call_hash in &hashes {
            let prefix = [d.prefix.clone(), call_hash.to_vec()].concat();
            protected_prefixes.push(prefix.clone());
            let matching: Vec<_> = rows(storage, &prefix).collect();
            if matching.is_empty() && name != "Preimage.PreimageFor" {
                protected.insert(prefix.clone(), None);
            }
            for (key, value) in matching {
                if value.len() > 32 * 1024 * 1024
                    || key.len()
                        != prefix.len() + if name == "Preimage.PreimageFor" { 4 } else { 0 }
                {
                    return Err("unexpected preimage custody width".into());
                }
                preserved_bytes = preserved_bytes
                    .checked_add(value.len())
                    .filter(|bytes| *bytes <= MAX_PRESERVED_BYTES)
                    .ok_or("scheduler preimage custody exceeds aggregate bound")?;
                bind(d, key, value)?;
                if name != "Preimage.PreimageFor" {
                    let (transformed, accounts) =
                        typed::transform_value(value, d.value_type, &schema.registry, &mapping)?;
                    if transformed != *value
                        || accounts.iter().any(|account| {
                            mapping.contains_key(account) || *account == recovery::ATTACKER_ACCOUNT
                        })
                    {
                        return Err("preimage provider/control needs a separate remap or attacker-claim adapter; cancellation cannot silently preserve it".into());
                    }
                    providers.extend(accounts);
                }
                protected.insert(key.clone(), Some(value.clone()));
            }
        }
    }
    let accounts = desc(schema, "System.Account")?;
    if accounts.hashers != [Hasher::Blake2_128Concat]
        || accounts.key_types.len() != 1
        || !typed::account_type(&schema.registry, accounts.key_types[0])
    {
        return Err("unreviewed preimage backing account codec".into());
    }
    for provider in &providers {
        let key = [
            accounts.prefix.clone(),
            sp_io::hashing::blake2_128(provider).to_vec(),
            provider.to_vec(),
        ]
        .concat();
        let value = storage.top.get(&key).cloned();
        if let Some(value) = &value {
            typed::decode_exact(value, accounts.value_type, &schema.registry)?;
            bind(accounts, &key, value)?;
        }
        protected.insert(key, value);
    }
    drop(bind);
    if used.len() != reviewed.len() {
        return Err("extraneous scheduler cancellation evidence".into());
    }
    let edits: Vec<_> = changes
        .iter()
        .map(|(_, key, value)| GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: Some(hash(value)),
            after: None,
        })
        .collect();
    let resolutions = changes
        .iter()
        .zip(&edits)
        .map(|((d, key, value), edit)| AdapterResolution {
            storage: d.name.clone(),
            key: hx(key),
            expected_before_hash: hx(&hash(value)),
            expected_after_hash: None,
            adapter: AdapterId::SchedulerCancellationV1,
            action: ResolutionAction::CancelPending,
            validated_edits: vec![edit.clone()],
            observed_accounts: BTreeSet::new(),
        })
        .collect();
    let witnesses = protected
        .iter()
        .filter(|(_, value)| {
            value
                .as_ref()
                .is_none_or(|bytes| bytes.len() <= recovery::MAX_CHUNK_BYTES)
        })
        .map(|(key, value)| StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: value.as_ref().map(|bytes| hash(bytes)),
        })
        .collect();
    let report = Report {
        evidence_hash: review.evidence_hash.clone(),
        cancelled: tasks,
        preserved_records: protected.len(),
        preserved_preimage_providers: providers.iter().map(|provider| hx(provider)).collect(),
        empty_prefixes: empty_prefixes
            .iter()
            .map(|witness| hx(&witness.prefix))
            .collect(),
        native_multisig_pending: 0,
        historical_calls_executed: false,
        refunds_created: 0,
        production_ready: false,
    };
    Ok(Compilation {
        groups: vec![edits.clone()],
        edits,
        resolutions,
        witnesses,
        empty_prefixes,
        report,
        protected,
        protected_prefixes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry};
    use sp_core::crypto::AccountId32;

    fn fixture(provider: [u8; 32]) -> (MetadataSchema, Storage) {
        let mut r = Registry::new();
        let account = r.register_type(&meta_type::<AccountId32>()).id;
        let block = r.register_type(&meta_type::<u32>()).id;
        let hash_id = r.register_type(&meta_type::<[u8; 32]>()).id;
        let agenda = r.register_type(&meta_type::<[u8; 76]>()).id;
        let status = r
            .register_type(&meta_type::<Option<(AccountId32, u128)>>())
            .id;
        let bytes = r.register_type(&meta_type::<Vec<u8>>()).id;
        let balance = r.register_type(&meta_type::<u128>()).id;
        let mut descriptors = Vec::new();
        for (name, key_types, hashers, value_type) in [
            (
                "Scheduler.Agenda",
                vec![block],
                vec![Hasher::Twox64Concat],
                agenda,
            ),
            ("Scheduler.IncompleteSince", vec![], vec![], block),
            (
                "Scheduler.Lookup",
                vec![hash_id],
                vec![Hasher::Identity],
                block,
            ),
            (
                "Scheduler.Retries",
                vec![block],
                vec![Hasher::Identity],
                block,
            ),
            (
                "Multisig.Multisigs",
                vec![hash_id],
                vec![Hasher::Identity],
                block,
            ),
            (
                "Preimage.StatusFor",
                vec![hash_id],
                vec![Hasher::Identity],
                status,
            ),
            (
                "Preimage.RequestStatusFor",
                vec![hash_id],
                vec![Hasher::Identity],
                status,
            ),
            (
                "Preimage.PreimageFor",
                vec![hash_id],
                vec![Hasher::Identity],
                bytes,
            ),
            (
                "System.Account",
                vec![account],
                vec![Hasher::Blake2_128Concat],
                balance,
            ),
        ] {
            let (p, item) = name.split_once('.').unwrap();
            descriptors.push(Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix: [
                    sp_io::hashing::twox_128(p.as_bytes()),
                    sp_io::hashing::twox_128(item.as_bytes()),
                ]
                .concat(),
                key_types,
                hashers,
                value_type,
            });
        }
        let schema = MetadataSchema {
            registry: r.into(),
            descriptors,
            metadata_hash: METADATA.into(),
        };
        let mut storage = Storage::default();
        for (index, when) in BLOCKS.into_iter().enumerate() {
            let mut task = vec![0; 76];
            task[..3].copy_from_slice(&[4, 1, 1]);
            task[3..11].copy_from_slice(b"democrac");
            task[11..15].copy_from_slice(&(index as u32).encode());
            task[35..37].copy_from_slice(&[63, 2]);
            // Last two tasks share a preimage: cancelling both cannot release it twice.
            let call_hash = [index.min(4) as u8 + 1; 32];
            task[37..69].copy_from_slice(&call_hash);
            task[69..73].copy_from_slice(&3u32.encode());
            storage.top.insert(
                agenda_key(desc(&schema, "Scheduler.Agenda").unwrap(), when),
                task,
            );
            storage.top.insert(
                [
                    desc(&schema, "Preimage.StatusFor").unwrap().prefix.clone(),
                    call_hash.to_vec(),
                ]
                .concat(),
                Some((AccountId32::from(provider), 123u128)).encode(),
            );
            storage.top.insert(
                [
                    desc(&schema, "Preimage.PreimageFor")
                        .unwrap()
                        .prefix
                        .clone(),
                    call_hash.to_vec(),
                    3u32.encode(),
                ]
                .concat(),
                vec![1u8, 2, 3].encode(),
            );
        }
        storage.top.insert(
            desc(&schema, "Scheduler.IncompleteSince")
                .unwrap()
                .prefix
                .clone(),
            (recovery::CHECKPOINT_NUMBER + 1).encode(),
        );
        storage.top.insert(
            [
                desc(&schema, "System.Account").unwrap().prefix.clone(),
                sp_io::hashing::blake2_128(&provider).to_vec(),
                provider.to_vec(),
            ]
            .concat(),
            999u128.encode(),
        );
        (schema, storage)
    }
    fn review(s: &MetadataSchema, storage: &Storage) -> Review {
        Review {
            evidence_hash: hx(&[7; 32]),
            records: storage
                .top
                .iter()
                .map(|(key, value)| Record {
                    storage: s
                        .descriptors
                        .iter()
                        .find(|d| key.starts_with(&d.prefix))
                        .unwrap()
                        .name
                        .clone(),
                    key: hx(key),
                    value_hash: hx(&hash(value)),
                })
                .collect(),
        }
    }
    #[test]
    fn cancels_all_seven_records_without_touching_shared_preimage_or_deposit() {
        let (s, storage) = fixture([1; 32]);
        let c = compile(&s, &storage, &[], &review(&s, &storage)).unwrap();
        assert_eq!(c.edits.len(), 7);
        assert_eq!(c.resolutions.len(), 7);
        assert_eq!(c.empty_prefixes.len(), 4);
        assert_eq!(c.report.cancelled.len(), 6);
        assert_eq!(
            c.report.cancelled[4].call_hash,
            c.report.cancelled[5].call_hash
        );
        assert_eq!(c.report.preserved_records, 16); // five status/content/absence + one backing account
        assert_eq!(c.report.refunds_created, 0);
        assert!(!c.report.historical_calls_executed);
        c.verify_preservation(&c.groups).unwrap();
        let mut projected = storage.clone();
        for edit in &c.edits {
            assert!(edit.after.is_none());
            projected.top.remove(&edit.key);
        }
        assert!(c
            .empty_prefixes
            .iter()
            .all(|p| !projected.top.keys().any(|key| key.starts_with(&p.prefix))));
        assert!(c
            .protected
            .iter()
            .all(|(key, value)| projected.top.get(key) == value.as_ref()));
    }
    #[test]
    fn rejects_any_unreviewed_pending_authorization_and_extra_or_missing_evidence() {
        for item in [
            "Scheduler.Agenda",
            "Scheduler.Lookup",
            "Scheduler.Retries",
            "Multisig.Multisigs",
        ] {
            let (s, mut storage) = fixture([1; 32]);
            storage.top.insert(
                [desc(&s, item).unwrap().prefix.clone(), vec![0; 4]].concat(),
                vec![0],
            );
            assert!(
                compile(&s, &storage, &[], &review(&s, &storage)).is_err(),
                "{item}"
            );
        }
        let (s, storage) = fixture([1; 32]);
        let mut r = review(&s, &storage);
        r.records.pop();
        assert!(compile(&s, &storage, &[], &r).is_err());
        let mut r = review(&s, &storage);
        r.records.push(Record {
            storage: "Preimage.StatusFor".into(),
            key: hx(&[4; 32]),
            value_hash: hx(&[1; 32]),
        });
        assert!(compile(&s, &storage, &[], &r).is_err());
    }
    #[test]
    fn compromised_or_attacker_provider_requires_separate_custody_adapter() {
        let (s, storage) = fixture([1; 32]);
        let replacements = [AccountReplacement {
            old: AccountId32::from([1; 32]),
            new: AccountId32::from([2; 32]),
        }];
        assert!(compile(&s, &storage, &replacements, &review(&s, &storage))
            .err()
            .unwrap()
            .contains("separate remap"));
        let (s, storage) = fixture(recovery::ATTACKER_ACCOUNT);
        assert!(compile(&s, &storage, &[], &review(&s, &storage)).is_err());
    }
    #[test]
    fn forbids_custody_edits_including_creation_under_absent_preimage_namespace() {
        let (s, storage) = fixture([1; 32]);
        let c = compile(&s, &storage, &[], &review(&s, &storage)).unwrap();
        for key in c.protected.keys().cloned().chain(std::iter::once(
            [c.protected_prefixes[0].clone(), vec![9]].concat(),
        )) {
            let e = GuardedStorageEdit {
                scope: StorageScope::Top,
                key: key.clone(),
                before: storage.top.get(&key).map(|v| hash(v)),
                after: Some(vec![9]),
            };
            assert!(c.verify_preservation(&[vec![e]]).is_err());
        }
    }
    #[test]
    fn rejects_unreviewed_call_origin_periodic_task_and_scan_cursor() {
        for offset in [0, 1, 2, 3, 35, 36, 73, 74, 75] {
            let (s, mut storage) = fixture([1; 32]);
            let key = agenda_key(desc(&s, "Scheduler.Agenda").unwrap(), BLOCKS[0]);
            storage.top.get_mut(&key).unwrap()[offset] ^= 1;
            assert!(
                compile(&s, &storage, &[], &review(&s, &storage)).is_err(),
                "offset {offset}"
            );
        }
        let (s, mut storage) = fixture([1; 32]);
        storage.top.insert(
            desc(&s, "Scheduler.IncompleteSince")
                .unwrap()
                .prefix
                .clone(),
            recovery::CHECKPOINT_NUMBER.encode(),
        );
        assert!(compile(&s, &storage, &[], &review(&s, &storage)).is_err());
    }
}
