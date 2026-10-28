//! Native chill or explicitly reconfirmed staking roles at the recovery boundary.
//! Funds, ledgers, payout configuration and historical liabilities are untouched.
use crate::{
    bags_list, hex_bytes,
    inventory::{
        self, AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StorageScope, StorageWitness,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use sp_runtime::{traits::Verify, MultiSignature};
use std::collections::{BTreeMap, BTreeSet};
const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const ITEMS: &[&str] = &[
    "Staking.Nominators",
    "Staking.Validators",
    "Staking.CounterForNominators",
    "Staking.CounterForValidators",
    "BagsList.ListNodes",
    "BagsList.ListBags",
    "BagsList.CounterForListNodes",
    "BagsList.__STORAGE_VERSION__",
];
const MAX_ROWS: usize = 100_000;
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub operators_hash: String,
    pub records: Vec<Record>,
    #[serde(default)]
    pub retained_nominations: Vec<Retention>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Record {
    pub storage: String,
    pub key: String,
    pub value_hash: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retention {
    pub old: String,
    pub signature: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub evidence_hash: String,
    pub operator_inventory_hash: String,
    pub operator_approval_verified: bool,
    pub original_nominators: usize,
    pub original_validators: usize,
    pub chilled_nominators: Vec<String>,
    pub chilled_validators: Vec<String>,
    pub retained_mapped_nominators: usize,
    pub retained_mapped_validators: usize,
    pub changed_keys: usize,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub report: Report,
    certificate: bags_list::RemovalCertificate,
    mapping_hash: [u8; 32],
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    prefixes: Vec<Vec<u8>>,
}
impl Compilation {
    pub fn graph_certificate(&self) -> &bags_list::RemovalCertificate {
        &self.certificate
    }
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        replacements: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&replacements.encode()) != self.mapping_hash {
            return Err("staking role replacement certificate mismatch".into());
        }
        let mut expected = self.before.clone();
        let mut seen = BTreeSet::new();
        for e in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top && self.prefixes.iter().any(|p| e.key.starts_with(p))
        }) {
            if !seen.insert(e.key.clone()) || self.before.get(&e.key).map(|v| hash(v)) != e.before {
                return Err("overlapping/stale staking role work".into());
            }
            if let Some(v) = &e.after {
                expected.insert(e.key.clone(), v.clone());
            } else {
                expected.remove(&e.key);
            }
        }
        if expected != self.expected {
            return Err("incomplete or altered native staking chill projection".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Nominations {
    targets: Vec<AccountId32>,
    submitted_in: u32,
    suppressed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Prefs {
    #[codec(compact)]
    commission: u32,
    blocked: bool,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn bytes32(text: &str) -> Result<[u8; 32]> {
    hex_bytes(text)?
        .try_into()
        .map_err(|_| "staking role account/hash width".into())
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing staking role descriptor {name}"))
}
fn decode<T: Decode + Encode>(schema: &MetadataSchema, d: &Descriptor, value: &[u8]) -> Result<T> {
    typed::decode_exact(value, d.value_type, &schema.registry)?;
    let mut input = value;
    let decoded = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || decoded.encode() != value {
        return Err("noncanonical staking role value".into());
    }
    Ok(decoded)
}
fn account_key(d: &Descriptor, owner: &AccountId32) -> Vec<u8> {
    let mut key = d.prefix.clone();
    key.extend(sp_io::hashing::twox_64(owner.as_ref()));
    key.extend(owner.encode());
    key
}
fn map_owner(d: &Descriptor, key: &[u8]) -> Result<AccountId32> {
    if d.hashers != [Hasher::Twox64Concat] || key.len() != d.prefix.len() + 40 {
        return Err("staking role key codec mismatch".into());
    }
    let owner = AccountId32::decode(&mut &key[key.len() - 32..]).map_err(|e| e.to_string())?;
    if account_key(d, &owner) != key {
        return Err("staking role key hash mismatch".into());
    }
    Ok(owner)
}
/// Reconfirmation signs the existing exact grant after identity substitution.
/// It cannot replace the target list, alter its order, or approve operator keys.
pub fn nomination_confirmation_message(
    old: &AccountId32,
    new: &AccountId32,
    before_hash: [u8; 32],
    targets: &[AccountId32],
    operators_hash: [u8; 32],
) -> Vec<u8> {
    (
        b"sora-nexus/recovery-retain-nominations/v1".as_slice(),
        recovery::GENESIS_HASH,
        recovery::CHECKPOINT_NUMBER,
        recovery::CHECKPOINT_HASH,
        old,
        new,
        before_hash,
        targets,
        operators_hash,
    )
        .encode()
}
#[cfg(feature = "runtime-work-types")]
fn operators(bytes: &[u8]) -> Result<BTreeSet<AccountId32>> {
    use codec::DecodeWithMemLimit;
    use framenode_runtime::incident_recovery_consensus::OperatorReplacement;
    if bytes.len() > 1024 * 1024 {
        return Err("operator inventory exceeds bound".into());
    }
    let mut input = bytes;
    let records = Vec::<OperatorReplacement>::decode_with_mem_limit(&mut input, 4 * 1024 * 1024)
        .map_err(|e| e.to_string())?;
    if !input.is_empty() || records.encode() != bytes || records.len() > 1200 {
        return Err("operator inventory is not canonical/bounded".into());
    }
    let mut owners = BTreeSet::new();
    let mut fresh = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut babe = BTreeSet::new();
    let mut im_online = BTreeSet::new();
    for record in records {
        if !owners.insert(record.owner)
            || !fresh.insert(record.fresh_grandpa)
            || record.retired_grandpa.is_empty()
            || record.approved_babe.is_empty()
            || record.approved_im_online.is_empty()
            || record.retired_grandpa.windows(2).any(|p| p[0] >= p[1])
            || record.approved_babe.windows(2).any(|p| p[0] >= p[1])
            || record.approved_im_online.windows(2).any(|p| p[0] >= p[1])
            || record
                .retired_grandpa
                .iter()
                .any(|key| !retired.insert(key.clone()))
            || record
                .approved_babe
                .iter()
                .any(|key| !babe.insert(key.clone()))
            || record
                .approved_im_online
                .iter()
                .any(|key| !im_online.insert(key.clone()))
        {
            return Err("operator inventory has duplicate/unreviewed records".into());
        }
    }
    if !fresh.is_disjoint(&retired) {
        return Err("fresh GRANDPA key overlaps retired operator inventory".into());
    }
    Ok(owners)
}
#[cfg(not(feature = "runtime-work-types"))]
fn operators(_: &[u8]) -> Result<BTreeSet<AccountId32>> {
    Err("staking roles require runtime-work-types for exact operator SCALE decoding".into())
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    operators_scale: &[u8],
    review: &Review,
) -> Result<Compilation> {
    if schema.metadata_hash != METADATA {
        return Err("staking roles require pinned checkpoint metadata".into());
    }
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("staking role replacements: {e:?}"))?;
    if bytes32(&review.evidence_hash)? == [0; 32]
        || bytes32(&review.operators_hash)? != hash(operators_scale)
        || review.records.len() > MAX_ROWS
        || review.retained_nominations.len() > replacements.len()
    {
        return Err("staking role evidence/hash/bounds mismatch".into());
    }
    let approved = operators(operators_scale)?;
    let mapping: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| (r.old.clone(), r.new.clone()))
        .collect();
    let mappings_bytes: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| {
            (
                *AsRef::<[u8; 32]>::as_ref(&r.old),
                *AsRef::<[u8; 32]>::as_ref(&r.new),
            )
        })
        .collect();
    let mut before = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut prefixes = Vec::new();
    let mut bytes = 0usize;
    for name in ITEMS {
        let d = descriptor(schema, name)?;
        let (pallet, item) = name.split_once('.').ok_or("staking role storage name")?;
        let mut expected_prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
        expected_prefix.extend(sp_io::hashing::twox_128(if item == "__STORAGE_VERSION__" {
            b":__STORAGE_VERSION__:"
        } else {
            item.as_bytes()
        }));
        if expected_prefix != d.prefix {
            return Err("staking role prefix mismatch".into());
        }
        prefixes.push(d.prefix.clone());
        for (k, v) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            bytes = bytes
                .checked_add(k.len() + v.len())
                .ok_or("staking role byte overflow")?;
            if before.len() >= MAX_ROWS || v.len() > 4096 || bytes > 64 * 1024 * 1024 {
                return Err("staking role inventory exceeds bound".into());
            }
            typed::decode_exact(v, d.value_type, &schema.registry)?;
            inventory::transform_key(k, d, &schema.registry, &BTreeMap::new(), None)?;
            before.insert(k.clone(), v.clone());
            names.insert(k.clone(), name.to_string());
        }
    }
    let mut supplied = BTreeMap::new();
    for row in &review.records {
        if supplied
            .insert(
                hex_bytes(&row.key)?,
                (row.storage.clone(), bytes32(&row.value_hash)?),
            )
            .is_some()
        {
            return Err("duplicate staking role review record".into());
        }
    }
    let observed: BTreeMap<_, _> = before
        .iter()
        .map(|(k, v)| (k.clone(), (names[k].clone(), hash(v))))
        .collect();
    if supplied != observed {
        return Err("staking role review is not the complete checkpoint inventory".into());
    }
    let nom_d = descriptor(schema, "Staking.Nominators")?;
    let val_d = descriptor(schema, "Staking.Validators")?;
    if nom_d.key_types.len() != 1
        || val_d.key_types.len() != 1
        || !typed::account_type(&schema.registry, nom_d.key_types[0])
        || !typed::account_type(&schema.registry, val_d.key_types[0])
    {
        return Err("staking role account codec changed".into());
    }
    let mut nominators = BTreeMap::new();
    let mut validators = BTreeMap::new();
    for (k, v) in &before {
        if names[k] == "Staking.Nominators" {
            let n: Nominations = decode(schema, nom_d, v)?;
            // Deployed legacy nominate preserved order and duplicate targets.
            // Native kick can leave an empty role; snapshot generation skips it.
            // Recovery must preserve these existing grants, not reinterpret them
            // using the newer async dispatch-time sort/dedup requirements.
            if n.targets.len() > 24 {
                return Err("native nomination target bound exceeded".into());
            }
            nominators.insert(map_owner(nom_d, k)?, n);
        } else if names[k] == "Staking.Validators" {
            let p: Prefs = decode(schema, val_d, v)?;
            if p.commission > 1_000_000_000 {
                return Err("validator commission exceeds Perbill".into());
            }
            validators.insert(map_owner(val_d, k)?, p);
        }
    }
    if nominators.keys().any(|k| validators.contains_key(k)) {
        return Err("account has overlapping validator/nominator roles".into());
    }
    let original_nominators = nominators.len();
    let original_validators = validators.len();
    for (name, count) in [
        ("Staking.CounterForNominators", original_nominators),
        ("Staking.CounterForValidators", original_validators),
    ] {
        let d = descriptor(schema, name)?;
        if !d.hashers.is_empty()
            || before
                .keys()
                .any(|k| k.starts_with(&d.prefix) && k != &d.prefix)
            || decode::<u32>(
                schema,
                d,
                before.get(&d.prefix).ok_or("staking role counter absent")?,
            )? as usize
                != count
        {
            return Err("staking role counter mismatch".into());
        }
    }
    let voters = bags_list::voter_owners(schema, storage)?;
    let role_owners: BTreeSet<_> = nominators
        .keys()
        .chain(validators.keys())
        .cloned()
        .collect();
    if voters != role_owners {
        return Err("staking roles differ from complete BagsList voter membership".into());
    }
    let mut retention = BTreeMap::new();
    for row in &review.retained_nominations {
        let old = AccountId32::from(bytes32(&row.old)?);
        if !mapping.contains_key(&old)
            || !nominators.contains_key(&old)
            || retention.insert(old, hex_bytes(&row.signature)?).is_some()
        {
            return Err("nomination confirmation does not identify one mapped nominator".into());
        }
    }
    let mut expected = before.clone();
    let mut removals = BTreeSet::new();
    let mut chilled_nominators = Vec::new();
    let mut chilled_validators = Vec::new();
    let mut retained_mapped_nominators = 0;
    let mut retained_mapped_validators = 0;
    for (old, nominations) in &nominators {
        let original_key = account_key(nom_d, old);
        let Some(new) = mapping.get(old) else {
            continue;
        };
        if !retention.contains_key(old) {
            expected.remove(&original_key);
            removals.insert(old.clone());
            chilled_nominators.push(hx(old.as_ref()));
        } else {
            retained_mapped_nominators += 1;
            let targets: Vec<_> = nominations
                .targets
                .iter()
                .map(|a| mapping.get(a).unwrap_or(a).clone())
                .collect();
            let bytes = &retention[old];
            if bytes.len() > 128 {
                return Err("nomination confirmation signature exceeds bound".into());
            }
            let mut input = bytes.as_slice();
            let signature = MultiSignature::decode(&mut input).map_err(|e| e.to_string())?;
            let message = nomination_confirmation_message(
                old,
                new,
                hash(&before[&original_key]),
                &targets,
                hash(operators_scale),
            );
            if !input.is_empty()
                || signature.encode() != *bytes
                || !signature.verify(message.as_slice(), new)
            {
                return Err("replacement key did not reconfirm exact nomination grant".into());
            }
        }
    }
    for old in validators.keys() {
        if mapping.contains_key(old) {
            if approved.contains(old) {
                retained_mapped_validators += 1;
            } else {
                expected.remove(&account_key(val_d, old));
                removals.insert(old.clone());
                chilled_validators.push(hx(old.as_ref()));
            }
        }
    }
    for (old, nominations) in &nominators {
        if removals.contains(old) {
            continue;
        }
        let old_key = account_key(nom_d, old);
        let effective = mapping.get(old).unwrap_or(old);
        let new_key = account_key(nom_d, effective);
        if new_key != old_key && storage.top.contains_key(&new_key) {
            return Err("replacement nomination role destination occupied".into());
        }
        let mut mapped = nominations.clone();
        for target in &mut mapped.targets {
            if let Some(new) = mapping.get(target) {
                if !approved.contains(target) {
                    return Err("retained nomination target needs manifest-bound operator inventory expansion".into());
                }
                *target = new.clone();
            }
        }
        expected.remove(&old_key);
        expected.insert(new_key, mapped.encode());
    }
    for (old, prefs) in &validators {
        if removals.contains(old) {
            continue;
        }
        let old_key = account_key(val_d, old);
        let new_key = account_key(val_d, mapping.get(old).unwrap_or(old));
        if new_key != old_key && storage.top.contains_key(&new_key) {
            return Err("replacement validator role destination occupied".into());
        }
        expected.remove(&old_key);
        expected.insert(new_key, prefs.encode());
    }
    for (name, count) in [
        (
            "Staking.CounterForNominators",
            original_nominators - chilled_nominators.len(),
        ),
        (
            "Staking.CounterForValidators",
            original_validators - chilled_validators.len(),
        ),
    ] {
        expected.insert(
            descriptor(schema, name)?.prefix.clone(),
            u32::try_from(count)
                .map_err(|_| "role counter overflow")?
                .encode(),
        );
    }
    let graph = bags_list::compile_role_graph(schema, storage, replacements, removals)?;
    for e in graph.edits {
        if let Some(v) = e.after {
            expected.insert(e.key, v);
        } else {
            expected.remove(&e.key);
        }
    }
    let keys: BTreeSet<_> = before.keys().chain(expected.keys()).cloned().collect();
    let edits: Vec<_> = keys
        .iter()
        .filter(|k| before.get(*k) != expected.get(*k))
        .map(|key| GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: before.get(key).map(|v| hash(v)),
            after: expected.get(key).cloned(),
        })
        .collect();
    let groups: Vec<_> = edits
        .chunks(recovery::MAX_EDITS_PER_CHUNK)
        .map(|c| c.to_vec())
        .collect();
    for group in &groups {
        recovery::RecoveryChunk {
            index: 0,
            edits: group.clone(),
        }
        .validate()
        .map_err(|e| format!("staking role work chunk: {e:?}"))?;
    }
    let edit_lookup: BTreeMap<_, _> = edits.iter().map(|edit| (edit.key.clone(), edit)).collect();
    let mut resolutions = Vec::new();
    for (k, v) in &before {
        if expected.get(k) == Some(v) {
            continue;
        }
        let name = &names[k];
        let d = descriptor(schema, name)?;
        let (new_key, mut accounts) =
            inventory::transform_key(k, d, &schema.registry, &mappings_bytes, None)?;
        let mut validated_edits =
            vec![(**edit_lookup.get(k).ok_or("missing native role edit")?).clone()];
        if new_key != *k {
            if let Some(edit) = edit_lookup.get(&new_key) {
                validated_edits.push((**edit).clone());
            }
        }
        accounts
            .extend(typed::transform_value(v, d.value_type, &schema.registry, &BTreeMap::new())?.1);
        resolutions.push(AdapterResolution {
            storage: name.clone(),
            key: hx(k),
            expected_before_hash: hx(&hash(v)),
            expected_after_hash: expected.get(k).map(|v| hx(&hash(v))),
            adapter: AdapterId::StakingRolesV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits,
            observed_accounts: accounts,
        });
    }
    let witnesses = keys
        .into_iter()
        .map(|key| StorageWitness {
            scope: StorageScope::Top,
            value_hash: expected.get(&key).map(|v| hash(v)),
            key,
        })
        .collect();
    let compilation = Compilation {
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            operator_inventory_hash: hx(&hash(operators_scale)),
            operator_approval_verified: false,
            original_nominators,
            original_validators,
            chilled_nominators,
            chilled_validators,
            retained_mapped_nominators,
            retained_mapped_validators,
            changed_keys: edits.len(),
            production_ready: false,
        },
        groups,
        edits,
        resolutions,
        witnesses,
        certificate: graph.certificate,
        mapping_hash: hash(&replacements.encode()),
        before,
        expected,
        prefixes,
    };
    compilation.verify_groups(&compilation.groups, replacements)?;
    bags_list::verify_groups_with_certificate(
        schema,
        storage,
        &compilation.groups,
        replacements,
        Some(compilation.graph_certificate()),
    )?;
    Ok(compilation)
}

#[cfg(all(test, feature = "runtime-work-types"))]
mod tests {
    use super::*;
    use framenode_runtime::incident_recovery_consensus::OperatorReplacement;
    use scale_info::{meta_type, PortableRegistry, Registry};
    use sp_core::Pair;
    #[derive(Encode, Decode, scale_info::TypeInfo)]
    struct Node {
        id: AccountId32,
        prev: Option<AccountId32>,
        next: Option<AccountId32>,
        bag_upper: u64,
        score: u64,
    }
    #[derive(Encode, Decode, scale_info::TypeInfo)]
    struct Bag {
        head: Option<AccountId32>,
        tail: Option<AccountId32>,
    }
    fn id(n: u8) -> AccountId32 {
        AccountId32::from([n; 32])
    }
    fn fixture() -> (MetadataSchema, Storage) {
        let mut r = Registry::new();
        let account = r.register_type(&meta_type::<AccountId32>()).id;
        let score = r.register_type(&meta_type::<u64>()).id;
        let count = r.register_type(&meta_type::<u32>()).id;
        let version = r.register_type(&meta_type::<u16>()).id;
        let node = r.register_type(&meta_type::<Node>()).id;
        let bag = r.register_type(&meta_type::<Bag>()).id;
        let noms = r.register_type(&meta_type::<Nominations>()).id;
        let prefs = r.register_type(&meta_type::<Prefs>()).id;
        let descriptors = ITEMS
            .iter()
            .map(|name| {
                let (pallet, item) = name.split_once('.').unwrap();
                let mut prefix = sp_io::hashing::twox_128(pallet.as_bytes()).to_vec();
                prefix.extend(sp_io::hashing::twox_128(if item == "__STORAGE_VERSION__" {
                    b":__STORAGE_VERSION__:"
                } else {
                    item.as_bytes()
                }));
                let (key, value_type) = match *name {
                    "Staking.Nominators" => (Some(account), noms),
                    "Staking.Validators" => (Some(account), prefs),
                    "BagsList.ListNodes" => (Some(account), node),
                    "BagsList.ListBags" => (Some(score), bag),
                    "BagsList.__STORAGE_VERSION__" => (None, version),
                    _ => (None, count),
                };
                Descriptor {
                    trie: "top".into(),
                    name: name.to_string(),
                    prefix,
                    key_types: key.into_iter().collect(),
                    hashers: if key.is_some() {
                        vec![Hasher::Twox64Concat]
                    } else {
                        vec![]
                    },
                    value_type,
                }
            })
            .collect();
        let s = MetadataSchema {
            registry: PortableRegistry::from(r),
            descriptors,
            metadata_hash: METADATA.into(),
        };
        let mut st = Storage::default();
        for who in [id(1), id(2)] {
            st.top.insert(
                account_key(descriptor(&s, "Staking.Nominators").unwrap(), &who),
                Nominations {
                    targets: vec![id(3)],
                    submitted_in: 8,
                    suppressed: false,
                }
                .encode(),
            );
        }
        st.top.insert(
            account_key(descriptor(&s, "Staking.Validators").unwrap(), &id(3)),
            Prefs {
                commission: 10,
                blocked: false,
            }
            .encode(),
        );
        for (name, value) in [
            ("Staking.CounterForNominators", 2u32),
            ("Staking.CounterForValidators", 1),
            ("BagsList.CounterForListNodes", 3),
        ] {
            st.top
                .insert(descriptor(&s, name).unwrap().prefix.clone(), value.encode());
        }
        st.top.insert(
            descriptor(&s, "BagsList.__STORAGE_VERSION__")
                .unwrap()
                .prefix
                .clone(),
            1u16.encode(),
        );
        for n in 1..=3 {
            let v = Node {
                id: id(n),
                prev: (n > 1).then(|| id(n - 1)),
                next: (n < 3).then(|| id(n + 1)),
                bag_upper: 100,
                score: 90,
            };
            st.top.insert(
                account_key(descriptor(&s, "BagsList.ListNodes").unwrap(), &id(n)),
                v.encode(),
            );
        }
        let mut k = descriptor(&s, "BagsList.ListBags").unwrap().prefix.clone();
        k.extend(sp_io::hashing::twox_64(&100u64.encode()));
        k.extend(100u64.encode());
        st.top.insert(
            k,
            Bag {
                head: Some(id(1)),
                tail: Some(id(3)),
            }
            .encode(),
        );
        (s, st)
    }
    fn operator(owner: AccountId32) -> OperatorReplacement {
        OperatorReplacement {
            owner,
            retired_grandpa: vec![sp_core::ed25519::Public::from_raw([4; 32]).into()]
                .try_into()
                .unwrap(),
            fresh_grandpa: sp_core::ed25519::Public::from_raw([5; 32]).into(),
            approved_babe: vec![sp_core::sr25519::Public::from_raw([6; 32]).into()]
                .try_into()
                .unwrap(),
            approved_im_online: vec![sp_core::sr25519::Public::from_raw([7; 32]).into()]
                .try_into()
                .unwrap(),
        }
    }
    fn review(s: &MetadataSchema, st: &Storage, operators: &[u8]) -> Review {
        Review {
            evidence_hash: hx(&[8; 32]),
            operators_hash: hx(&hash(operators)),
            records: st
                .top
                .iter()
                .map(|(k, v)| Record {
                    storage: s
                        .descriptors
                        .iter()
                        .find(|d| k.starts_with(&d.prefix))
                        .unwrap()
                        .name
                        .clone(),
                    key: hx(k),
                    value_hash: hx(&hash(v)),
                })
                .collect(),
            retained_nominations: vec![],
        }
    }
    fn nom_map() -> (Vec<AccountReplacement>, sp_core::ed25519::Pair) {
        let pair = sp_core::ed25519::Pair::from_seed(&[9; 32]);
        (
            vec![AccountReplacement {
                old: id(1),
                new: AccountId32::from(pair.public().0),
            }],
            pair,
        )
    }
    #[test]
    fn unconfirmed_nominator_chills_with_exact_links_counters_and_private_certificate() {
        let (s, st) = fixture();
        let (map, _) = nom_map();
        let ops = Vec::<OperatorReplacement>::new().encode();
        let c = compile(&s, &st, &map, &ops, &review(&s, &st, &ops)).unwrap();
        assert_eq!(c.report.chilled_nominators, vec![hx(id(1).as_ref())]);
        assert_eq!(
            c.expected[&descriptor(&s, "Staking.CounterForNominators")
                .unwrap()
                .prefix],
            1u32.encode()
        );
        assert_eq!(
            c.expected[&descriptor(&s, "BagsList.CounterForListNodes")
                .unwrap()
                .prefix],
            2u32.encode()
        );
        assert!(!c.expected.contains_key(&account_key(
            descriptor(&s, "Staking.Nominators").unwrap(),
            &map[0].new
        )));
        c.verify_groups(&c.groups, &map).unwrap();
        assert!(bags_list::verify_groups(&s, &st, &c.groups, &map).is_err());
        let checked = bags_list::verify_groups_with_certificate(
            &s,
            &st,
            &c.groups,
            &map,
            Some(c.graph_certificate()),
        )
        .unwrap();
        assert_eq!(checked.report.removed_nodes, 1);
        assert!(!c.report.operator_approval_verified);
    }
    #[test]
    fn legacy_duplicate_and_kicked_empty_targets_are_preserved_exactly() {
        let (s, mut st) = fixture();
        let nom = descriptor(&s, "Staking.Nominators").unwrap();
        st.top.insert(
            account_key(nom, &id(1)),
            Nominations {
                targets: vec![id(3), id(3)],
                submitted_in: 8,
                suppressed: false,
            }
            .encode(),
        );
        st.top.insert(
            account_key(nom, &id(2)),
            Nominations {
                targets: vec![],
                submitted_in: 1,
                suppressed: false,
            }
            .encode(),
        );
        let ops = vec![operator(id(3))].encode();
        let c = compile(&s, &st, &[], &ops, &review(&s, &st, &ops)).unwrap();
        assert!(c.edits.is_empty());
        c.verify_groups(&[], &[]).unwrap();
        let map = vec![AccountReplacement {
            old: id(3),
            new: id(9),
        }];
        let c = compile(&s, &st, &map, &ops, &review(&s, &st, &ops)).unwrap();
        let n: Nominations = decode(&s, nom, &c.expected[&account_key(nom, &id(1))]).unwrap();
        assert_eq!(n.targets, vec![id(9), id(9)]);
        assert_eq!(
            c.expected[&account_key(nom, &id(2))],
            st.top[&account_key(nom, &id(2))]
        );
        c.verify_groups(&c.groups, &map).unwrap();
    }
    #[test]
    fn replacement_signature_reconfirms_only_exact_original_targets() {
        let (s, st) = fixture();
        let (map, pair) = nom_map();
        let ops = vec![operator(id(3))].encode();
        let mut r = review(&s, &st, &ops);
        let old_key = account_key(descriptor(&s, "Staking.Nominators").unwrap(), &id(1));
        let message = nomination_confirmation_message(
            &id(1),
            &map[0].new,
            hash(&st.top[&old_key]),
            &[id(3)],
            hash(&ops),
        );
        r.retained_nominations.push(Retention {
            old: hx(id(1).as_ref()),
            signature: hx(&MultiSignature::from(pair.sign(&message)).encode()),
        });
        let c = compile(&s, &st, &map, &ops, &r).unwrap();
        assert!(c.report.chilled_nominators.is_empty());
        assert_eq!(c.report.retained_mapped_nominators, 1);
        assert_eq!(
            c.expected[&account_key(descriptor(&s, "Staking.Nominators").unwrap(), &map[0].new)],
            st.top[&old_key]
        );
        r.retained_nominations[0].signature =
            hx(&MultiSignature::from(pair.sign(b"different grant")).encode());
        assert!(compile(&s, &st, &map, &ops, &r).is_err());
    }
    #[test]
    fn mapped_target_requires_operator_record_and_keeps_third_party_order() {
        let (s, st) = fixture();
        let map = vec![AccountReplacement {
            old: id(3),
            new: id(9),
        }];
        let absent = Vec::<OperatorReplacement>::new().encode();
        assert!(compile(&s, &st, &map, &absent, &review(&s, &st, &absent)).is_err());
        let ops = vec![operator(id(3))].encode();
        let c = compile(&s, &st, &map, &ops, &review(&s, &st, &ops)).unwrap();
        assert_eq!(c.report.retained_mapped_validators, 1);
        let d = descriptor(&s, "Staking.Nominators").unwrap();
        let n: Nominations = decode(&s, d, &c.expected[&account_key(d, &id(2))]).unwrap();
        assert_eq!(n.targets, vec![id(9)]);
        assert_eq!(n.submitted_in, 8);
    }
    #[test]
    fn uncovered_inactive_validator_chills_without_touching_unrelated_roles() {
        let (s, mut st) = fixture();
        let d = descriptor(&s, "Staking.Nominators").unwrap();
        for who in [id(1), id(2)] {
            st.top.insert(
                account_key(d, &who),
                Nominations {
                    targets: vec![id(4)],
                    submitted_in: 8,
                    suppressed: false,
                }
                .encode(),
            );
        }
        let map = vec![AccountReplacement {
            old: id(3),
            new: id(9),
        }];
        let ops = Vec::<OperatorReplacement>::new().encode();
        let c = compile(&s, &st, &map, &ops, &review(&s, &st, &ops)).unwrap();
        assert_eq!(c.report.chilled_validators, vec![hx(id(3).as_ref())]);
        assert_eq!(
            c.expected[&descriptor(&s, "Staking.CounterForValidators")
                .unwrap()
                .prefix],
            0u32.encode()
        );
        assert_eq!(
            c.expected[&account_key(d, &id(1))],
            st.top[&account_key(d, &id(1))]
        );
    }
    #[test]
    fn unreviewed_removal_missing_counter_and_changed_mapping_fail() {
        let (s, st) = fixture();
        let (map, _) = nom_map();
        let ops = Vec::<OperatorReplacement>::new().encode();
        let c = compile(&s, &st, &map, &ops, &review(&s, &st, &ops)).unwrap();
        let mut missing = c.groups.clone();
        missing[0].pop();
        assert!(c.verify_groups(&missing, &map).is_err());
        assert!(bags_list::verify_groups_with_certificate(
            &s,
            &st,
            &c.groups,
            &[],
            Some(c.graph_certificate())
        )
        .is_err());
        let mut bad = c.groups.clone();
        let key = account_key(descriptor(&s, "Staking.Nominators").unwrap(), &id(2));
        bad[0].push(GuardedStorageEdit {
            scope: StorageScope::Top,
            key: key.clone(),
            before: Some(hash(&st.top[&key])),
            after: None,
        });
        assert!(c.verify_groups(&bad, &map).is_err());
    }
    #[test]
    fn malformed_operator_inventory_and_fresh_retired_reuse_fail() {
        let mut row = operator(id(3));
        row.fresh_grandpa = row.retired_grandpa[0].clone();
        assert!(operators(&vec![row].encode()).is_err());
        assert!(operators(&vec![operator(id(3)), operator(id(4))].encode()).is_err());
        assert!(operators(&[255; 16]).is_err());
        let (s, st) = fixture();
        let ops = vec![operator(id(3))].encode();
        let mut r = review(&s, &st, &ops);
        r.operators_hash = hx(&[0; 32]);
        assert!(compile(&s, &st, &[], &ops, &r).is_err());
    }
}
