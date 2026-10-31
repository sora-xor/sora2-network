//! Exact graph-preserving identity replacement for the checkpoint bags list.
//! No score refresh, rebagging, sorting, insertion or removal is authorized here.
use crate::{
    inventory::{Descriptor, Hasher, MetadataSchema},
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness,
};
use scale_info::{TypeDef, TypeDefPrimitive};
use serde::Serialize;
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MAX_ROWS: usize = 100_000;
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Node {
    id: AccountId32,
    prev: Option<AccountId32>,
    next: Option<AccountId32>,
    bag_upper: u64,
    score: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Bag {
    head: Option<AccountId32>,
    tail: Option<AccountId32>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub nodes: usize,
    pub bags: usize,
    pub edges: usize,
    pub remapped_nodes: usize,
    pub removed_nodes: usize,
    pub graph_isomorphic: bool,
    pub production_ready: bool,
}
pub struct Verification {
    pub report: Report,
    pub witnesses: Vec<StorageWitness>,
}
struct Descriptors<'a> {
    nodes: &'a Descriptor,
    bags: &'a Descriptor,
    counter: &'a Descriptor,
    version: &'a Descriptor,
    pallet: Vec<u8>,
}
struct Graph {
    nodes: BTreeMap<AccountId32, Node>,
    bags: BTreeMap<u64, Bag>,
    edges: usize,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing bags-list descriptor {name}"))
}
fn primitive(schema: &MetadataSchema, id: u32, expected: TypeDefPrimitive) -> bool {
    matches!(schema.registry.resolve(id).map(|t| &t.type_def), Some(TypeDef::Primitive(actual)) if *actual == expected)
}
fn optional_account(schema: &MetadataSchema, id: u32) -> bool {
    let Some(TypeDef::Variant(v)) = schema.registry.resolve(id).map(|t| &t.type_def) else {
        return false;
    };
    v.variants.len() == 2
        && v.variants[0].name == "None"
        && v.variants[0].index == 0
        && v.variants[0].fields.is_empty()
        && v.variants[1].name == "Some"
        && v.variants[1].index == 1
        && v.variants[1].fields.len() == 1
        && typed::account_type(&schema.registry, v.variants[1].fields[0].ty.id)
}
fn descriptors(schema: &MetadataSchema) -> Result<Descriptors<'_>> {
    if schema.metadata_hash != METADATA {
        return Err("bags-list metadata is not the pinned checkpoint metadata".into());
    }
    let d = Descriptors {
        nodes: descriptor(schema, "BagsList.ListNodes")?,
        bags: descriptor(schema, "BagsList.ListBags")?,
        counter: descriptor(schema, "BagsList.CounterForListNodes")?,
        version: descriptor(schema, "BagsList.__STORAGE_VERSION__")?,
        pallet: sp_io::hashing::twox_128(b"BagsList").to_vec(),
    };
    for (entry, suffix) in [
        (d.nodes, "ListNodes"),
        (d.bags, "ListBags"),
        (d.counter, "CounterForListNodes"),
        (d.version, ":__STORAGE_VERSION__:"),
    ] {
        let mut expected = d.pallet.clone();
        expected.extend(sp_io::hashing::twox_128(suffix.as_bytes()));
        if entry.prefix != expected {
            return Err("bags-list native prefix mismatch".into());
        }
    }
    if d.nodes.hashers != [Hasher::Twox64Concat]
        || d.nodes.key_types.len() != 1
        || !typed::account_type(&schema.registry, d.nodes.key_types[0])
        || d.bags.hashers != [Hasher::Twox64Concat]
        || d.bags.key_types.len() != 1
        || !primitive(schema, d.bags.key_types[0], TypeDefPrimitive::U64)
        || !d.counter.hashers.is_empty()
        || !d.counter.key_types.is_empty()
        || !primitive(schema, d.counter.value_type, TypeDefPrimitive::U32)
        || !d.version.hashers.is_empty()
        || !d.version.key_types.is_empty()
        || !primitive(schema, d.version.value_type, TypeDefPrimitive::U16)
    {
        return Err("bags-list native key/count codec mismatch".into());
    }
    for (entry, names) in [
        (d.nodes, vec!["id", "prev", "next", "bag_upper", "score"]),
        (d.bags, vec!["head", "tail"]),
    ] {
        let Some(TypeDef::Composite(c)) = schema
            .registry
            .resolve(entry.value_type)
            .map(|t| &t.type_def)
        else {
            return Err("bags-list value is not composite".into());
        };
        if c.fields.len() != names.len()
            || c.fields
                .iter()
                .zip(names)
                .any(|(f, n)| f.name.as_deref() != Some(n))
        {
            return Err("bags-list value shape changed".into());
        }
        if entry.name.ends_with("ListNodes") {
            if !typed::account_type(&schema.registry, c.fields[0].ty.id)
                || !optional_account(schema, c.fields[1].ty.id)
                || !optional_account(schema, c.fields[2].ty.id)
                || !primitive(schema, c.fields[3].ty.id, TypeDefPrimitive::U64)
                || !primitive(schema, c.fields[4].ty.id, TypeDefPrimitive::U64)
            {
                return Err("bags-list node field types changed".into());
            }
        } else if c.fields.iter().any(|f| !optional_account(schema, f.ty.id)) {
            return Err("bags-list bag field types changed".into());
        }
    }
    Ok(d)
}
fn key(d: &Descriptor, bytes: &[u8]) -> Vec<u8> {
    let mut key = d.prefix.clone();
    key.extend(sp_io::hashing::twox_64(bytes));
    key.extend(bytes);
    key
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, d: &Descriptor, bytes: &[u8]) -> Result<T> {
    typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let mut input = bytes;
    let value = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || value.encode() != bytes {
        return Err("bags-list value did not roundtrip".into());
    }
    Ok(value)
}
fn graph(
    schema: &MetadataSchema,
    d: &Descriptors<'_>,
    raw: &BTreeMap<Vec<u8>, Vec<u8>>,
) -> Result<Graph> {
    if raw.len() > MAX_ROWS {
        return Err("bags-list inventory exceeds bound".into());
    }
    let mut nodes = BTreeMap::new();
    let mut bags = BTreeMap::new();
    for (k, v) in raw {
        if v.len() > 256 {
            return Err("bags-list record exceeds bounded native encoding".into());
        }
        if k.starts_with(&d.nodes.prefix) {
            let node: Node = decode(schema, d.nodes, v)?;
            if key(d.nodes, node.id.as_ref()) != *k || nodes.insert(node.id.clone(), node).is_some()
            {
                return Err("bags-list node/key mismatch".into());
            }
        } else if k.starts_with(&d.bags.prefix) {
            if k.len() != d.bags.prefix.len() + 16 {
                return Err("bags-list bag key width".into());
            }
            let upper =
                u64::from_le_bytes(k[k.len() - 8..].try_into().map_err(|_| "bag key width")?);
            if key(d.bags, &upper.encode()) != *k
                || bags
                    .insert(upper, decode::<Bag>(schema, d.bags, v)?)
                    .is_some()
            {
                return Err("bags-list bag key mismatch".into());
            }
        } else if k != &d.counter.prefix && k != &d.version.prefix {
            return Err("unreviewed bags-list state or maintenance cursor".into());
        }
    }
    let counter = raw
        .get(&d.counter.prefix)
        .ok_or("bags-list counter absent")?;
    if decode::<u32>(schema, d.counter, counter)? as usize != nodes.len() {
        return Err("bags-list node counter mismatch".into());
    }
    if let Some(v) = raw.get(&d.version.prefix) {
        decode::<u16>(schema, d.version, v)?;
    }
    let mut seen = BTreeSet::new();
    let mut edges = 0;
    for (upper, bag) in &bags {
        if bag.head.is_none() || bag.tail.is_none() {
            return Err("stored bags-list bag is empty or one-sided".into());
        }
        let mut previous = None;
        let mut next = bag.head.clone();
        while let Some(id) = next {
            if !seen.insert(id.clone()) {
                return Err("bags-list cycle or shared node".into());
            }
            let node = nodes.get(&id).ok_or("bags-list dangling link")?;
            if node.prev != previous || node.bag_upper != *upper {
                return Err("bags-list reciprocal/bag link mismatch".into());
            }
            previous = Some(id);
            next = node.next.clone();
            if next.is_some() {
                edges += 1;
            }
        }
        if previous != bag.tail {
            return Err("bags-list tail mismatch".into());
        }
    }
    if seen.len() != nodes.len() {
        return Err("bags-list orphan nodes".into());
    }
    Ok(Graph { nodes, bags, edges })
}
/// A compiler-created authority to remove only reviewed mapped staking roles.
/// Fields cannot be supplied by JSON or constructed by the CLI.
pub struct RemovalCertificate {
    removals: BTreeSet<AccountId32>,
    mapping_hash: [u8; 32],
    before_hash: [u8; 32],
}
pub(crate) struct RoleGraph {
    pub edits: Vec<GuardedStorageEdit>,
    pub certificate: RemovalCertificate,
}
fn capture(storage: &Storage, d: &Descriptors<'_>) -> BTreeMap<Vec<u8>, Vec<u8>> {
    storage
        .top
        .range(d.pallet.clone()..)
        .take_while(|(k, _)| k.starts_with(&d.pallet))
        .take(MAX_ROWS + 1)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}
fn expected_graph(
    d: &Descriptors<'_>,
    old: &Graph,
    before: &BTreeMap<Vec<u8>, Vec<u8>>,
    replacements: &[AccountReplacement],
    removals: &BTreeSet<AccountId32>,
) -> Result<BTreeMap<Vec<u8>, Vec<u8>>> {
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| (r.old.clone(), r.new.clone()))
        .collect();
    if removals
        .iter()
        .any(|id| !mapping.contains_key(id) || !old.nodes.contains_key(id))
    {
        return Err("bags-list removal lacks a mapped existing voter".into());
    }
    let effective = |id: &AccountId32| mapping.get(id).unwrap_or(id).clone();
    let mut expected = before.clone();
    for node in old.nodes.values() {
        expected.remove(&key(d.nodes, node.id.as_ref()));
    }
    for (upper, bag) in &old.bags {
        let mut retained = Vec::new();
        let mut next = bag.head.clone();
        while let Some(id) = next {
            let node = old.nodes.get(&id).ok_or("bags-list traversal changed")?;
            next = node.next.clone();
            if !removals.contains(&id) {
                retained.push(node);
            }
        }
        if retained.is_empty() {
            expected.remove(&key(d.bags, &upper.encode()));
            continue;
        }
        for (index, node) in retained.iter().enumerate() {
            let mapped = Node {
                id: effective(&node.id),
                prev: index.checked_sub(1).map(|i| effective(&retained[i].id)),
                next: retained.get(index + 1).map(|n| effective(&n.id)),
                bag_upper: node.bag_upper,
                score: node.score,
            };
            if expected
                .insert(key(d.nodes, mapped.id.as_ref()), mapped.encode())
                .is_some()
            {
                return Err("bags-list replacement node collision".into());
            }
        }
        expected.insert(
            key(d.bags, &upper.encode()),
            Bag {
                head: retained.first().map(|n| effective(&n.id)),
                tail: retained.last().map(|n| effective(&n.id)),
            }
            .encode(),
        );
    }
    let count =
        u32::try_from(old.nodes.len() - removals.len()).map_err(|_| "bags-list count overflow")?;
    expected.insert(d.counter.prefix.clone(), count.encode());
    Ok(expected)
}
pub(crate) fn voter_owners(
    schema: &MetadataSchema,
    storage: &Storage,
) -> Result<BTreeSet<AccountId32>> {
    let d = descriptors(schema)?;
    Ok(graph(schema, &d, &capture(storage, &d))?
        .nodes
        .into_keys()
        .collect())
}
/// Called only by the typed staking-role compiler after checking role ownership.
pub(crate) fn compile_role_graph(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    removals: BTreeSet<AccountId32>,
) -> Result<RoleGraph> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("bags-list replacements: {e:?}"))?;
    let d = descriptors(schema)?;
    let before = capture(storage, &d);
    let old = graph(schema, &d, &before)?;
    let expected = expected_graph(&d, &old, &before, replacements, &removals)?;
    graph(schema, &d, &expected)?;
    let keys: BTreeSet<_> = before.keys().chain(expected.keys()).cloned().collect();
    let edits = keys
        .into_iter()
        .filter(|k| before.get(k) != expected.get(k))
        .map(|key| GuardedStorageEdit {
            scope: StorageScope::Top,
            before: before.get(&key).map(|v| hash(v)),
            after: expected.get(&key).cloned(),
            key,
        })
        .collect();
    Ok(RoleGraph {
        edits,
        certificate: RemovalCertificate {
            removals,
            mapping_hash: hash(&replacements.encode()),
            before_hash: hash(&before.encode()),
        },
    })
}
/// Validate the exact before/after graph around the caller's guarded generic moves.
pub fn verify_groups(
    schema: &MetadataSchema,
    storage: &Storage,
    groups: &[Vec<GuardedStorageEdit>],
    replacements: &[AccountReplacement],
) -> Result<Verification> {
    verify_groups_with_certificate(schema, storage, groups, replacements, None)
}
/// A removal certificate comes only from a completed typed staking-role compiler.
pub fn verify_groups_with_certificate(
    schema: &MetadataSchema,
    storage: &Storage,
    groups: &[Vec<GuardedStorageEdit>],
    replacements: &[AccountReplacement],
    certificate: Option<&RemovalCertificate>,
) -> Result<Verification> {
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("bags-list replacements: {e:?}"))?;
    let d = descriptors(schema)?;
    let before = capture(storage, &d);
    let old = graph(schema, &d, &before)?;
    let empty = BTreeSet::new();
    let removals = if let Some(c) = certificate {
        if c.mapping_hash != hash(&replacements.encode()) || c.before_hash != hash(&before.encode())
        {
            return Err("bags-list removal certificate does not match checkpoint/mapping".into());
        }
        &c.removals
    } else {
        &empty
    };
    let expected = expected_graph(&d, &old, &before, replacements, removals)?;
    let mut projected = before.clone();
    let mut edited = BTreeSet::new();
    for e in groups
        .iter()
        .flatten()
        .filter(|e| e.scope == StorageScope::Top && e.key.starts_with(&d.pallet))
    {
        if !edited.insert(e.key.clone()) || before.get(&e.key).map(|v| hash(v)) != e.before {
            return Err("bags-list overlapping edit or before-value mismatch".into());
        }
        if let Some(value) = &e.after {
            projected.insert(e.key.clone(), value.clone());
        } else {
            projected.remove(&e.key);
        }
    }
    let new = graph(schema, &d, &projected)?;
    if expected != projected || old.nodes.len() - removals.len() != new.nodes.len() {
        return Err(
            "bags-list projected graph differs from exact identity mapping and reviewed chills"
                .into(),
        );
    }
    let keys: BTreeSet<_> = before.keys().chain(projected.keys()).cloned().collect();
    let witnesses = keys
        .into_iter()
        .map(|key| StorageWitness {
            scope: StorageScope::Top,
            value_hash: projected.get(&key).map(|v| hash(v)),
            key,
        })
        .collect();
    let mapped: BTreeSet<_> = replacements.iter().map(|r| &r.old).collect();
    Ok(Verification {
        report: Report {
            nodes: old.nodes.len(),
            bags: old.bags.len(),
            edges: old.edges,
            remapped_nodes: old
                .nodes
                .keys()
                .filter(|id| mapped.contains(id) && !removals.contains(*id))
                .count(),
            removed_nodes: removals.len(),
            graph_isomorphic: true,
            production_ready: false,
        },
        witnesses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry};
    fn id(n: u8) -> AccountId32 {
        AccountId32::from([n; 32])
    }
    fn fixture() -> (MetadataSchema, Storage) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let score = registry.register_type(&meta_type::<u64>()).id;
        let node = registry.register_type(&meta_type::<Node>()).id;
        let bag = registry.register_type(&meta_type::<Bag>()).id;
        let count = registry.register_type(&meta_type::<u32>()).id;
        let version = registry.register_type(&meta_type::<u16>()).id;
        let entries = vec![
            ("ListNodes", Some(account), node),
            ("ListBags", Some(score), bag),
            ("CounterForListNodes", None, count),
            (":__STORAGE_VERSION__:", None, version),
        ]
        .into_iter()
        .map(|(name, k, value_type)| {
            let mut prefix = sp_io::hashing::twox_128(b"BagsList").to_vec();
            prefix.extend(sp_io::hashing::twox_128(name.as_bytes()));
            Descriptor {
                trie: "top".into(),
                name: format!(
                    "BagsList.{}",
                    if name == ":__STORAGE_VERSION__:" {
                        "__STORAGE_VERSION__"
                    } else {
                        name
                    }
                ),
                prefix,
                key_types: k.into_iter().collect(),
                hashers: if k.is_some() {
                    vec![Hasher::Twox64Concat]
                } else {
                    vec![]
                },
                value_type,
            }
        })
        .collect();
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: entries,
            metadata_hash: METADATA.into(),
        };
        let d = descriptors(&schema).unwrap();
        let mut storage = Storage::default();
        for node in [
            Node {
                id: id(1),
                prev: None,
                next: Some(id(2)),
                bag_upper: 100,
                score: 90,
            },
            Node {
                id: id(2),
                prev: Some(id(1)),
                next: Some(id(3)),
                bag_upper: 100,
                score: 85,
            },
            Node {
                id: id(3),
                prev: Some(id(2)),
                next: None,
                bag_upper: 100,
                score: 95,
            },
        ] {
            storage
                .top
                .insert(key(d.nodes, node.id.as_ref()), node.encode());
        }
        storage.top.insert(
            key(d.bags, &100u64.encode()),
            Bag {
                head: Some(id(1)),
                tail: Some(id(3)),
            }
            .encode(),
        );
        storage.top.insert(d.counter.prefix.clone(), 3u32.encode());
        storage.top.insert(d.version.prefix.clone(), 1u16.encode());
        (schema, storage)
    }
    fn moves(schema: &MetadataSchema, storage: &Storage) -> Vec<Vec<GuardedStorageEdit>> {
        let d = descriptors(schema).unwrap();
        let mut after = storage.top.clone();
        let old = key(d.nodes, id(2).as_ref());
        after.remove(&old);
        for n in 1..=3 {
            let k = key(d.nodes, id(n).as_ref());
            let mut node: Node = decode(schema, d.nodes, &storage.top[&k]).unwrap();
            if n == 2 {
                node.id = id(9);
            }
            if node.prev == Some(id(2)) {
                node.prev = Some(id(9));
            }
            if node.next == Some(id(2)) {
                node.next = Some(id(9));
            }
            after.insert(key(d.nodes, node.id.as_ref()), node.encode());
        }
        let keys: BTreeSet<_> = storage.top.keys().chain(after.keys()).cloned().collect();
        vec![keys
            .into_iter()
            .filter(|k| storage.top.get(k) != after.get(k))
            .map(|key| GuardedStorageEdit {
                scope: StorageScope::Top,
                before: storage.top.get(&key).map(|v| hash(v)),
                after: after.get(&key).cloned(),
                key,
            })
            .collect()]
    }
    fn map() -> Vec<AccountReplacement> {
        vec![AccountReplacement {
            old: id(2),
            new: id(9),
        }]
    }
    #[test]
    fn exact_neighbour_renaming_preserves_order_and_scores() {
        let (s, st) = fixture();
        let g = moves(&s, &st);
        let v = verify_groups(&s, &st, &g, &map()).unwrap();
        assert_eq!(v.report.nodes, 3);
        assert_eq!(v.report.edges, 2);
        assert_eq!(v.report.remapped_nodes, 1);
        assert!(!v.report.production_ready);
    }
    #[test]
    fn missing_neighbour_score_change_and_counter_change_fail() {
        let (s, st) = fixture();
        let g = moves(&s, &st);
        let mut missing = g.clone();
        missing[0].pop();
        assert!(verify_groups(&s, &st, &missing, &map()).is_err());
        let d = descriptors(&s).unwrap();
        let mut changed = g.clone();
        let e = changed[0]
            .iter_mut()
            .find(|e| e.key == key(d.nodes, id(9).as_ref()))
            .unwrap();
        let mut n: Node = decode(&s, d.nodes, e.after.as_ref().unwrap()).unwrap();
        n.score += 1;
        e.after = Some(n.encode());
        assert!(verify_groups(&s, &st, &changed, &map()).is_err());
        let mut count = g.clone();
        count[0].push(GuardedStorageEdit {
            scope: StorageScope::Top,
            key: d.counter.prefix.clone(),
            before: Some(hash(&3u32.encode())),
            after: Some(4u32.encode()),
        });
        assert!(verify_groups(&s, &st, &count, &map()).is_err());
    }
    #[test]
    fn corrupt_prestate_cycles_orphans_and_native_keys_fail() {
        let (s, st) = fixture();
        let d = descriptors(&s).unwrap();
        for mode in 0..3 {
            let mut bad = st.clone();
            let k = key(d.nodes, id(3).as_ref());
            let mut n: Node = decode(&s, d.nodes, &bad.top[&k]).unwrap();
            if mode == 0 {
                n.next = Some(id(1));
                bad.top.insert(k, n.encode());
            } else if mode == 1 {
                bad.top.remove(&key(d.bags, &100u64.encode()));
            } else {
                let v = bad.top.remove(&k).unwrap();
                let mut malformed = k;
                malformed[32] ^= 1;
                bad.top.insert(malformed, v);
            }
            assert!(verify_groups(&s, &bad, &[], &[]).is_err());
        }
    }
    #[test]
    fn new_cursor_unknown_metadata_and_duplicate_edit_fail() {
        let (s, st) = fixture();
        let g = moves(&s, &st);
        let mut duplicate = g.clone();
        duplicate[0].push(g[0][0].clone());
        assert!(verify_groups(&s, &st, &duplicate, &map()).is_err());
        let mut unknown = st.clone();
        let mut key = sp_io::hashing::twox_128(b"BagsList").to_vec();
        key.extend(sp_io::hashing::twox_128(b"NextNodeAutoRebagged"));
        unknown.top.insert(key, id(1).encode());
        assert!(verify_groups(&s, &unknown, &[], &[]).is_err());
        let mut wrong = s;
        wrong.metadata_hash = "wrong".into();
        assert!(verify_groups(&wrong, &st, &[], &[]).is_err());
    }
}
