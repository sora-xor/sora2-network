//! Identity-only transformation of current staking economic history.
//! Historical block headers and Session proof roots are never changed. Historical
//! exposures, slash maxima and claim markers in current state must follow the
//! replacement ledger, since the native reward/slash paths look them up by stash.
use crate::{
    hex_bytes,
    inventory::{
        self, AdapterId, AdapterResolution, Descriptor, Hasher, MetadataSchema, ResolutionAction,
    },
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, StoragePrefixWitness, StorageScope,
    StorageWitness,
};
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};
const METADATA: &str = "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8";
const MARKER: &[u8] = b"runtime:migrations:staking_reward_points_stash_remapped";
const MAX_ROWS: usize = 100_000;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_VALUE: usize = 1024 * 1024;
pub const ITEMS: &[&str] = &[
    "ErasStakers",
    "ErasStakersClipped",
    "ErasStakersOverview",
    "ErasStakersPaged",
    "ErasValidatorPrefs",
    "ErasRewardPoints",
    "ClaimedRewards",
    "ValidatorSlashInEra",
    "NominatorSlashInEra",
    "SlashingSpans",
    "SpanSlash",
    "UnappliedSlashes",
    "Invulnerables",
    "ErasTotalStake",
    "ErasValidatorReward",
    "ErasStartSessionIndex",
    "BondedEras",
];
// These are not present in deployed v16. Their absence is part of admission, not
// an invitation to interpret an async queue using a legacy record codec.
const ASYNC_ABSENT: &[&str] = &[
    "AsyncUnappliedSlashes",
    "OffenceQueue",
    "OffenceQueueEras",
    "ProcessingOffence",
    "HistoricalSlashes",
    "HistoricalSlashHead",
    "HistoricalSlashTail",
    "LegacySlashCursor",
    "CutoffEra",
    "FirstExecutionEra",
    "ErasValidatorIncentiveWeight",
    "ErasSumValidatorIncentiveWeight",
    "ErasValidatorIncentiveBudget",
    "ErasSumWeightedPoints",
    "ErasNominatorsSlashable",
    "LastValidatorEra",
    "WeightedPointsFormulaStartEra",
    "NextDeferredSlashEra",
    "CancelledSlashes",
    "VoterSnapshotStatus",
    "NextElectionPage",
    "ElectableStashes",
    "EraPruningState",
];
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub evidence_hash: String,
    pub records: Vec<Record>,
    pub absent_prefixes: Vec<String>,
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
pub struct Report {
    pub evidence_hash: String,
    pub records: usize,
    pub changed_records: usize,
    pub changed_keys: usize,
    pub overview_records: usize,
    pub exposure_pages: usize,
    pub claim_records: usize,
    pub span_records: usize,
    pub preserved_empty_prefixes: Vec<String>,
    pub historical_proof_roots_changed: bool,
    pub production_ready: bool,
}
pub struct Compilation {
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub edits: Vec<GuardedStorageEdit>,
    pub resolutions: Vec<AdapterResolution>,
    pub witnesses: Vec<StorageWitness>,
    pub prefix_witnesses: Vec<StoragePrefixWitness>,
    pub report: Report,
    protected_prefixes: Vec<Vec<u8>>,
    expected: BTreeMap<Vec<u8>, Vec<u8>>,
    before: BTreeMap<Vec<u8>, Vec<u8>>,
    mapping_hash: [u8; 32],
}
impl Compilation {
    /// Must run after all generic/adapter groups are assembled. Partial migrations
    /// and unrelated alterations of retained liabilities cannot pass this gate.
    pub fn verify_groups(
        &self,
        groups: &[Vec<GuardedStorageEdit>],
        replacements: &[AccountReplacement],
    ) -> Result<()> {
        if hash(&replacements.encode()) != self.mapping_hash {
            return Err("staking history replacement certificate mismatch".into());
        }
        let mut projected = self.before.clone();
        let mut seen = BTreeSet::new();
        for e in groups.iter().flatten().filter(|e| {
            e.scope == StorageScope::Top
                && (e.key == MARKER || self.protected_prefixes.iter().any(|p| e.key.starts_with(p)))
        }) {
            if !seen.insert(e.key.clone()) || self.before.get(&e.key).map(|v| hash(v)) != e.before {
                return Err("staking history overlapping or stale edit".into());
            }
            if let Some(v) = &e.after {
                projected.insert(e.key.clone(), v.clone());
            } else {
                projected.remove(&e.key);
            }
        }
        if projected != self.expected {
            return Err(
                "staking economic history is missing its exact coupled transformation".into(),
            );
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Individual {
    who: AccountId32,
    #[codec(compact)]
    value: u128,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Exposure {
    #[codec(compact)]
    total: u128,
    #[codec(compact)]
    own: u128,
    others: Vec<Individual>,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Overview {
    #[codec(compact)]
    total: u128,
    #[codec(compact)]
    own: u128,
    nominator_count: u32,
    page_count: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Page {
    #[codec(compact)]
    page_total: u128,
    others: Vec<Individual>,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Points {
    total: u32,
    individual: BTreeMap<AccountId32, u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Spans {
    span_index: u32,
    last_start: u32,
    last_nonzero_slash: u32,
    prior: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Span {
    slashed: u128,
    paid_out: u128,
}
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(test, derive(scale_info::TypeInfo))]
struct Slash {
    validator: AccountId32,
    own: u128,
    others: Vec<(AccountId32, u128)>,
    reporters: Vec<AccountId32>,
    payout: u128,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn prefix(item: &str) -> Vec<u8> {
    let mut p = sp_io::hashing::twox_128(b"Staking").to_vec();
    p.extend(sp_io::hashing::twox_128(item.as_bytes()));
    p
}
fn bytes32(s: &str) -> Result<[u8; 32]> {
    hex_bytes(s)?
        .try_into()
        .map_err(|_| "staking history hash width".into())
}
fn decode<T: Decode + Encode>(bytes: &[u8]) -> Result<T> {
    let mut input = bytes;
    let v = T::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty() || v.encode() != bytes {
        return Err("noncanonical staking history native codec".into());
    }
    Ok(v)
}
fn component<T: Decode + Encode>(input: &mut &[u8]) -> Result<T> {
    if input.len() < 8 {
        return Err("truncated staking history map key".into());
    }
    let expected = &input[..8];
    let bytes = &input[8..];
    let mut rest = bytes;
    let value = T::decode(&mut rest).map_err(|e| e.to_string())?;
    let consumed = bytes.len() - rest.len();
    if value.encode() != bytes[..consumed]
        || sp_io::hashing::twox_64(&bytes[..consumed]) != expected
    {
        return Err("staking history map key did not roundtrip".into());
    }
    *input = rest;
    Ok(value)
}
fn era_owner(d: &Descriptor, key: &[u8]) -> Result<(u32, AccountId32)> {
    let mut input = key
        .strip_prefix(d.prefix.as_slice())
        .ok_or("staking history prefix")?;
    let era = component(&mut input)?;
    let owner = component(&mut input)?;
    if !input.is_empty() {
        return Err("staking history key suffix".into());
    }
    Ok((era, owner))
}
fn owner(d: &Descriptor, key: &[u8]) -> Result<AccountId32> {
    let mut input = key
        .strip_prefix(d.prefix.as_slice())
        .ok_or("staking history prefix")?;
    let owner = component(&mut input)?;
    if !input.is_empty() {
        return Err("staking history key suffix".into());
    }
    Ok(owner)
}
fn checked_sum(mut values: impl Iterator<Item = u128>) -> Result<u128> {
    values.try_fold(0u128, |a, b| {
        a.checked_add(b)
            .ok_or_else(|| "staking history financial sum overflow".into())
    })
}
fn individuals(rows: &[Individual]) -> Result<u128> {
    if rows.len() > 256 || rows.iter().map(|i| &i.who).collect::<BTreeSet<_>>().len() != rows.len()
    {
        return Err("staking history exposure exceeds bound or repeats a nominator".into());
    }
    checked_sum(rows.iter().map(|i| i.value))
}
type Records = BTreeMap<Vec<u8>, (String, Vec<u8>)>;
fn validate_financial(
    schema: &MetadataSchema,
    records: &Records,
) -> Result<(usize, usize, usize, usize)> {
    let mut overviews = BTreeMap::new();
    let mut pages: BTreeMap<(u32, AccountId32), BTreeMap<u32, Page>> = BTreeMap::new();
    let mut full = BTreeMap::new();
    let mut clipped = BTreeMap::new();
    let mut claims = Vec::new();
    let mut spans = BTreeMap::new();
    let mut span_values = Vec::new();
    for (key, (name, value)) in records {
        let d = schema
            .descriptors
            .iter()
            .find(|d| &d.name == name && d.trie == "top")
            .ok_or("staking history descriptor missing")?;
        typed::decode_exact(value, d.value_type, &schema.registry)?;
        inventory::transform_key(key, d, &schema.registry, &BTreeMap::new(), None)?;
        match name
            .strip_prefix("Staking.")
            .ok_or("staking history name")?
        {
            "ErasStakersOverview" => {
                let v: Overview = decode(value)?;
                if v.own > v.total || v.nominator_count > 256 || v.page_count > 1 {
                    return Err(
                        "staking historical overview exceeds admitted financial/page bounds".into(),
                    );
                }
                overviews.insert(era_owner(d, key)?, v);
            }
            "ErasStakersPaged" => {
                let mut input = &key[d.prefix.len()..];
                let era = component(&mut input)?;
                let account = component(&mut input)?;
                let page = component::<u32>(&mut input)?;
                if !input.is_empty() {
                    return Err("exposure page key suffix".into());
                }
                let v: Page = decode(value)?;
                if individuals(&v.others)? != v.page_total {
                    return Err("staking exposure page total mismatch".into());
                }
                pages.entry((era, account)).or_default().insert(page, v);
            }
            "ErasStakers" | "ErasStakersClipped" => {
                let v: Exposure = decode(value)?;
                let sum = individuals(&v.others)?
                    .checked_add(v.own)
                    .ok_or("exposure sum overflow")?;
                if sum > v.total || (name.ends_with("ErasStakers") && sum != v.total) {
                    return Err("staking full/clipped exposure total mismatch".into());
                }
                if name.ends_with("ErasStakers") {
                    full.insert(era_owner(d, key)?, v);
                } else {
                    clipped.insert(era_owner(d, key)?, v);
                }
            }
            "ClaimedRewards" => {
                let v: Vec<u32> = decode(value)?;
                if v.len() > 256 || v.iter().collect::<BTreeSet<_>>().len() != v.len() {
                    return Err("duplicate/oversized claimed reward pages".into());
                }
                claims.push((era_owner(d, key)?, v));
            }
            "ErasRewardPoints" => {
                let v: Points = decode(value)?;
                if v.individual.len() > 1200
                    || v.individual
                        .values()
                        .try_fold(0u32, |a, b| a.checked_add(*b))
                        != Some(v.total)
                {
                    return Err("staking reward points total/map bound mismatch".into());
                }
            }
            "SlashingSpans" => {
                let v: Spans = decode(value)?;
                if v.prior.len() > 10000
                    || v.prior.len() > v.span_index as usize
                    || v.prior.contains(&0)
                    || v.prior
                        .iter()
                        .try_fold(v.last_start, |a, b| a.checked_sub(*b))
                        .is_none()
                {
                    return Err("staking slashing span arithmetic mismatch".into());
                }
                spans.insert(owner(d, key)?, v);
            }
            "SpanSlash" => {
                let mut input = &key[d.prefix.len()..];
                let k = component::<(AccountId32, u32)>(&mut input)?;
                if !input.is_empty() {
                    return Err("span slash key suffix".into());
                }
                decode::<Span>(value)?;
                span_values.push(k);
            }
            "UnappliedSlashes" => {
                let v: Vec<Slash> = decode(value)?;
                if value.len() > 512 * 1024
                    || v.iter()
                        .any(|s| s.others.len() > 256 || s.reporters.len() > 16384)
                {
                    return Err("legacy slash queue exceeds execution bounds".into());
                }
                for s in v {
                    if s.others
                        .iter()
                        .map(|(a, _)| a)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != s.others.len()
                    {
                        return Err("duplicate legacy slash nominator".into());
                    }
                    checked_sum(
                        s.others
                            .iter()
                            .map(|(_, b)| *b)
                            .chain(std::iter::once(s.own)),
                    )?;
                }
            }
            "Invulnerables" => {
                let v: Vec<AccountId32> = decode(value)?;
                if v.len() > 1200 || v.iter().collect::<BTreeSet<_>>().len() != v.len() {
                    return Err("invalid invulnerable roster".into());
                }
            }
            "ValidatorSlashInEra" => {
                let v: (u32, u128) = decode(value)?;
                if v.0 > 1_000_000_000 {
                    return Err("slash fraction exceeds Perbill".into());
                }
            }
            "NominatorSlashInEra" | "ErasTotalStake" | "ErasValidatorReward" => {
                decode::<u128>(value)?;
            }
            "ErasStartSessionIndex" => {
                decode::<u32>(value)?;
            }
            "BondedEras" => {
                let v: Vec<(u32, u32)> = decode(value)?;
                if v.len() > 1000 || v.windows(2).any(|p| p[0].0 >= p[1].0 || p[0].1 >= p[1].1) {
                    return Err("invalid bonded-era ordering".into());
                }
            }
            "ErasValidatorPrefs" => {}
            _ => return Err("unreviewed staking economic storage".into()),
        }
    }
    for (k, v) in &overviews {
        let p = pages.get(k);
        let count = p.map_or(0, BTreeMap::len);
        let nominators = p.map_or(0, |p| p.values().map(|p| p.others.len()).sum());
        let total = checked_sum(p.into_iter().flat_map(|p| p.values()).map(|p| p.page_total))?;
        if count != v.page_count as usize
            || nominators != v.nominator_count as usize
            || v.own.checked_add(total) != Some(v.total)
            || p.is_some_and(|p| p.keys().copied().ne(0..v.page_count))
        {
            return Err("staking overview/page relationships mismatch".into());
        }
    }
    if pages.keys().any(|k| !overviews.contains_key(k)) {
        return Err("orphan staking exposure page".into());
    }
    for (k, v) in &clipped {
        if let Some(f) = full.get(k) {
            if f.own != v.own
                || f.total != v.total
                || v.others.iter().any(|i| !f.others.contains(i))
            {
                return Err("clipped exposure differs from full financial ownership".into());
            }
        }
    }
    for (k, c) in &claims {
        let max = overviews
            .get(k)
            .map(|v| v.page_count.max(u32::from(v.own > 0)))
            .or_else(|| (full.contains_key(k) || clipped.contains_key(k)).then_some(1))
            .ok_or("reward claim without retained exposure")?;
        if c.iter().any(|p| *p >= max) {
            return Err("claimed reward page outside retained exposure".into());
        }
    }
    for (account, index) in &span_values {
        let s = spans
            .get(account)
            .ok_or("span liability without slashing spans")?;
        if *index > s.span_index || *index < s.span_index - s.prior.len() as u32 {
            return Err("span liability index outside retained spans".into());
        }
    }
    Ok((
        overviews.len(),
        pages.values().map(BTreeMap::len).sum(),
        claims.len(),
        span_values.len(),
    ))
}

pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    review: &Review,
) -> Result<Compilation> {
    if schema.metadata_hash != METADATA {
        return Err("staking history requires exact checkpoint metadata".into());
    }
    recovery::validate_replacements(replacements)
        .map_err(|e| format!("staking history replacements: {e:?}"))?;
    if bytes32(&review.evidence_hash)? == [0; 32]
        || review.records.len() > MAX_ROWS
        || review.absent_prefixes.len() > ITEMS.len() + ASYNC_ABSENT.len()
    {
        return Err("staking history evidence/bounds invalid".into());
    }
    if storage.top.get(MARKER).map(Vec::as_slice) != Some(&[1]) {
        return Err(
            "staking reward controller-to-stash normalization must already be complete".into(),
        );
    }
    let mappings: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| {
            (
                *AsRef::<[u8; 32]>::as_ref(&r.old),
                *AsRef::<[u8; 32]>::as_ref(&r.new),
            )
        })
        .collect();
    let mut records = Records::new();
    let mut absent = BTreeSet::new();
    let mut protected_prefixes = Vec::new();
    let mut byte_count = 0usize;
    for item in ITEMS {
        let name = format!("Staking.{item}");
        let d = schema
            .descriptors
            .iter()
            .find(|d| d.trie == "top" && d.name == name)
            .ok_or_else(|| format!("missing staking history descriptor {name}"))?;
        if d.prefix != prefix(item) || d.hashers.iter().any(|h| *h != Hasher::Twox64Concat) {
            return Err("staking history native prefix/hasher mismatch".into());
        }
        protected_prefixes.push(d.prefix.clone());
        let mut count = 0;
        for (k, v) in storage
            .top
            .range(d.prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&d.prefix))
        {
            byte_count = byte_count
                .checked_add(k.len() + v.len())
                .ok_or("staking history byte overflow")?;
            if records.len() >= MAX_ROWS || v.len() > MAX_VALUE || byte_count > MAX_BYTES {
                return Err("staking history capture exceeds bounds".into());
            }
            records.insert(k.clone(), (name.clone(), v.clone()));
            count += 1;
        }
        if count == 0 {
            absent.insert(name);
        }
    }
    for item in ASYNC_ABSENT {
        let p = prefix(item);
        if storage
            .top
            .range(p.clone()..)
            .next()
            .is_some_and(|(k, _)| k.starts_with(&p))
        {
            return Err(format!(
                "unexpected pre-activation async staking state {item}"
            ));
        }
        protected_prefixes.push(p);
        absent.insert(format!("Staking.{item}"));
    }
    let supplied_absent: BTreeSet<_> = review.absent_prefixes.iter().cloned().collect();
    if supplied_absent.len() != review.absent_prefixes.len() || supplied_absent != absent {
        return Err("staking history absent-prefix review is incomplete".into());
    }
    let mut supplied = BTreeMap::new();
    for row in &review.records {
        let key = hex_bytes(&row.key)?;
        let digest = bytes32(&row.value_hash)?;
        if supplied
            .insert(key, (row.storage.clone(), digest))
            .is_some()
        {
            return Err("duplicate staking history review record".into());
        }
    }
    let mut observed: BTreeMap<_, _> = records
        .iter()
        .map(|(k, (name, v))| (k.clone(), (name.clone(), hash(v))))
        .collect();
    observed.insert(
        MARKER.to_vec(),
        ("StakingRewardPointsStashRemapped".into(), hash(&[1])),
    );
    if supplied != observed {
        return Err("staking history review is not the complete exact checkpoint inventory".into());
    }
    let counts = validate_financial(schema, &records)?;
    let mut expected_records = Records::new();
    let mut groups = Vec::new();
    let mut resolutions = Vec::new();
    let mut changed_records = 0;
    for (key, (name, value)) in &records {
        let d = schema
            .descriptors
            .iter()
            .find(|d| &d.name == name && d.trie == "top")
            .ok_or("descriptor vanished")?;
        let (new_key, mut accounts) =
            inventory::transform_key(key, d, &schema.registry, &mappings, None)?;
        let (mut new_value, found) =
            typed::transform_value(value, d.value_type, &schema.registry, &mappings)?;
        accounts.extend(found);
        if name == "Staking.ErasRewardPoints" {
            let old: Points = decode(value)?;
            let mut points = BTreeMap::new();
            for (account, points_value) in old.individual {
                let bytes = *AsRef::<[u8; 32]>::as_ref(&account);
                let effective = mappings
                    .get(&bytes)
                    .copied()
                    .map(AccountId32::from)
                    .unwrap_or(account);
                if points.insert(effective, points_value).is_some() {
                    return Err("staking reward points identity collision".into());
                }
            }
            new_value = Points {
                total: old.total,
                individual: points,
            }
            .encode();
        }
        if new_key != *key && storage.top.contains_key(&new_key) {
            return Err("staking history replacement destination already occupied".into());
        }
        if expected_records
            .insert(new_key.clone(), (name.clone(), new_value.clone()))
            .is_some()
        {
            return Err("staking history transformed key collision".into());
        }
        if new_key == *key && new_value == *value {
            continue;
        }
        changed_records += 1;
        let group = if new_key == *key {
            vec![GuardedStorageEdit {
                scope: StorageScope::Top,
                key: key.clone(),
                before: Some(hash(value)),
                after: Some(new_value.clone()),
            }]
        } else {
            vec![
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: key.clone(),
                    before: Some(hash(value)),
                    after: None,
                },
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key: new_key,
                    before: None,
                    after: Some(new_value),
                },
            ]
        };
        resolutions.push(AdapterResolution {
            storage: name.clone(),
            key: hx(key),
            expected_before_hash: hx(&hash(value)),
            expected_after_hash: group[0].after.as_ref().map(|v| hx(&hash(v))),
            adapter: AdapterId::StakingHistoryV1,
            action: ResolutionAction::ReplaceControl,
            validated_edits: group.clone(),
            observed_accounts: accounts,
        });
        groups.push(group);
    }
    if validate_financial(schema, &expected_records)? != counts {
        return Err("staking history relationship inventory changed".into());
    }
    let mut before: BTreeMap<_, _> = records.into_iter().map(|(k, (_, v))| (k, v)).collect();
    before.insert(MARKER.to_vec(), vec![1]);
    let mut expected: BTreeMap<_, _> = expected_records
        .into_iter()
        .map(|(k, (_, v))| (k, v))
        .collect();
    expected.insert(MARKER.to_vec(), vec![1]);
    let keys: BTreeSet<_> = before.keys().chain(expected.keys()).cloned().collect();
    let witnesses = keys
        .into_iter()
        .map(|key| StorageWitness {
            scope: StorageScope::Top,
            value_hash: expected.get(&key).map(|v| hash(v)),
            key,
        })
        .collect();
    let edits: Vec<_> = groups.iter().flatten().cloned().collect();
    let mut prefix_witnesses: Vec<_> = absent
        .iter()
        .map(|name| StoragePrefixWitness {
            scope: StorageScope::Top,
            prefix: prefix(
                name.strip_prefix("Staking.")
                    .expect("constructed staking prefix name"),
            ),
        })
        .collect();
    prefix_witnesses.sort_by(|a, b| a.prefix.cmp(&b.prefix));
    Ok(Compilation {
        report: Report {
            evidence_hash: review.evidence_hash.clone(),
            records: before.len(),
            changed_records,
            changed_keys: edits.len(),
            overview_records: counts.0,
            exposure_pages: counts.1,
            claim_records: counts.2,
            span_records: counts.3,
            preserved_empty_prefixes: absent.into_iter().collect(),
            historical_proof_roots_changed: false,
            production_ready: false,
        },
        groups,
        edits,
        resolutions,
        witnesses,
        prefix_witnesses,
        protected_prefixes,
        expected,
        before,
        mapping_hash: hash(&replacements.encode()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry};
    fn id(n: u8) -> AccountId32 {
        AccountId32::from([n; 32])
    }
    fn desc<'a>(s: &'a MetadataSchema, item: &str) -> &'a Descriptor {
        s.descriptors
            .iter()
            .find(|d| d.name == format!("Staking.{item}"))
            .unwrap()
    }
    fn key(d: &Descriptor, components: &[Vec<u8>]) -> Vec<u8> {
        let mut k = d.prefix.clone();
        for c in components {
            k.extend(sp_io::hashing::twox_64(c));
            k.extend(c);
        }
        k
    }
    fn insert<T: Encode>(
        s: &MetadataSchema,
        st: &mut Storage,
        item: &str,
        components: &[Vec<u8>],
        v: T,
    ) {
        st.top.insert(key(desc(s, item), components), v.encode());
    }
    fn fixture() -> (MetadataSchema, Storage) {
        let mut r = Registry::new();
        let account = r.register_type(&meta_type::<AccountId32>()).id;
        let era = r.register_type(&meta_type::<u32>()).id;
        let balance = r.register_type(&meta_type::<u128>()).id;
        let exposure = r.register_type(&meta_type::<Exposure>()).id;
        let overview = r.register_type(&meta_type::<Overview>()).id;
        let page = r.register_type(&meta_type::<Page>()).id;
        let points = r.register_type(&meta_type::<Points>()).id;
        let spans = r.register_type(&meta_type::<Spans>()).id;
        let span = r.register_type(&meta_type::<Span>()).id;
        let span_key = r.register_type(&meta_type::<(AccountId32, u32)>()).id;
        let queue = r.register_type(&meta_type::<Vec<Slash>>()).id;
        let prefs = r
            .register_type(&meta_type::<(codec::Compact<u32>, bool)>())
            .id;
        let claims = r.register_type(&meta_type::<Vec<u32>>()).id;
        let accounts = r.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let val_max = r.register_type(&meta_type::<(u32, u128)>()).id;
        let eras = r.register_type(&meta_type::<Vec<(u32, u32)>>()).id;
        let descriptors = ITEMS
            .iter()
            .map(|item| {
                let (keys, value_type) = match *item {
                    "ErasStakers" | "ErasStakersClipped" => (vec![era, account], exposure),
                    "ErasStakersOverview" => (vec![era, account], overview),
                    "ErasStakersPaged" => (vec![era, account, era], page),
                    "ErasValidatorPrefs" => (vec![era, account], prefs),
                    "ErasRewardPoints" => (vec![era], points),
                    "ClaimedRewards" => (vec![era, account], claims),
                    "ValidatorSlashInEra" => (vec![era, account], val_max),
                    "NominatorSlashInEra" => (vec![era, account], balance),
                    "SlashingSpans" => (vec![account], spans),
                    "SpanSlash" => (vec![span_key], span),
                    "UnappliedSlashes" => (vec![era], queue),
                    "Invulnerables" => (vec![], accounts),
                    "ErasTotalStake" | "ErasValidatorReward" => (vec![era], balance),
                    "ErasStartSessionIndex" => (vec![era], era),
                    "BondedEras" => (vec![], eras),
                    _ => unreachable!(),
                };
                Descriptor {
                    trie: "top".into(),
                    name: format!("Staking.{item}"),
                    prefix: prefix(item),
                    hashers: vec![Hasher::Twox64Concat; keys.len()],
                    key_types: keys,
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
        st.top.insert(MARKER.to_vec(), vec![1]);
        insert(
            &s,
            &mut st,
            "ErasStakersOverview",
            &[1u32.encode(), id(1).encode()],
            Overview {
                total: 30,
                own: 10,
                nominator_count: 1,
                page_count: 1,
            },
        );
        insert(
            &s,
            &mut st,
            "ErasStakersPaged",
            &[1u32.encode(), id(1).encode(), 0u32.encode()],
            Page {
                page_total: 20,
                others: vec![Individual {
                    who: id(3),
                    value: 20,
                }],
            },
        );
        insert(
            &s,
            &mut st,
            "ErasValidatorPrefs",
            &[1u32.encode(), id(1).encode()],
            (codec::Compact(100u32), false),
        );
        insert(
            &s,
            &mut st,
            "ClaimedRewards",
            &[1u32.encode(), id(1).encode()],
            vec![0u32],
        );
        insert(
            &s,
            &mut st,
            "ErasRewardPoints",
            &[1u32.encode()],
            Points {
                total: 10,
                individual: BTreeMap::from([(id(1), 4), (id(3), 6)]),
            },
        );
        for owner in [id(1), id(3)] {
            insert(
                &s,
                &mut st,
                "SlashingSpans",
                &[owner.encode()],
                Spans {
                    span_index: 2,
                    last_start: 10,
                    last_nonzero_slash: 9,
                    prior: vec![3, 2],
                },
            );
            insert(
                &s,
                &mut st,
                "SpanSlash",
                &[(owner, 1u32).encode()],
                Span {
                    slashed: 7,
                    paid_out: 1,
                },
            );
        }
        insert(
            &s,
            &mut st,
            "UnappliedSlashes",
            &[2u32.encode()],
            vec![Slash {
                validator: id(1),
                own: 5,
                others: vec![(id(3), 10)],
                reporters: vec![id(4)],
                payout: 2,
            }],
        );
        insert(
            &s,
            &mut st,
            "ValidatorSlashInEra",
            &[1u32.encode(), id(1).encode()],
            (10u32, 5u128),
        );
        insert(
            &s,
            &mut st,
            "NominatorSlashInEra",
            &[1u32.encode(), id(3).encode()],
            7u128,
        );
        insert(&s, &mut st, "Invulnerables", &[], Vec::<AccountId32>::new());
        insert(&s, &mut st, "ErasTotalStake", &[1u32.encode()], 30u128);
        insert(
            &s,
            &mut st,
            "ErasValidatorReward",
            &[1u32.encode()],
            1000u128,
        );
        insert(&s, &mut st, "ErasStartSessionIndex", &[1u32.encode()], 9u32);
        insert(&s, &mut st, "BondedEras", &[], vec![(1u32, 9u32)]);
        (s, st)
    }
    fn review(s: &MetadataSchema, st: &Storage) -> Review {
        let mut absent_prefixes: Vec<_> = ITEMS
            .iter()
            .filter(|i| !st.top.keys().any(|k| k.starts_with(&prefix(i))))
            .chain(ASYNC_ABSENT.iter())
            .map(|i| format!("Staking.{i}"))
            .collect();
        absent_prefixes.sort();
        let records = st
            .top
            .iter()
            .filter_map(|(k, v)| {
                let storage = if k == MARKER {
                    Some("StakingRewardPointsStashRemapped".into())
                } else {
                    s.descriptors
                        .iter()
                        .find(|d| k.starts_with(&d.prefix))
                        .map(|d| d.name.clone())
                }?;
                Some(Record {
                    storage,
                    key: hx(k),
                    value_hash: hx(&hash(v)),
                })
            })
            .collect();
        Review {
            evidence_hash: hx(&[7; 32]),
            records,
            absent_prefixes,
        }
    }
    fn replacements() -> Vec<AccountReplacement> {
        vec![
            AccountReplacement {
                old: id(1),
                new: id(9),
            },
            AccountReplacement {
                old: id(3),
                new: id(8),
            },
        ]
    }
    #[test]
    fn historical_exposures_claims_and_slash_liabilities_follow_same_effective_owners() {
        let (s, st) = fixture();
        let c = compile(&s, &st, &replacements(), &review(&s, &st)).unwrap();
        c.verify_groups(&c.groups, &replacements()).unwrap();
        let page: Page = decode(
            &c.expected[&key(
                desc(&s, "ErasStakersPaged"),
                &[1u32.encode(), id(9).encode(), 0u32.encode()],
            )],
        )
        .unwrap();
        assert_eq!(
            page,
            Page {
                page_total: 20,
                others: vec![Individual {
                    who: id(8),
                    value: 20
                }]
            }
        );
        let queue: Vec<Slash> =
            decode(&c.expected[&key(desc(&s, "UnappliedSlashes"), &[2u32.encode()])]).unwrap();
        assert_eq!(
            queue[0],
            Slash {
                validator: id(9),
                own: 5,
                others: vec![(id(8), 10)],
                reporters: vec![id(4)],
                payout: 2
            }
        );
        assert_eq!(c.report.claim_records, 1);
        assert_eq!(c.report.span_records, 2);
        assert!(!c.report.historical_proof_roots_changed);
    }
    #[test]
    fn reward_map_is_reordered_canonically_without_combining_points() {
        let (s, st) = fixture();
        let c = compile(&s, &st, &replacements(), &review(&s, &st)).unwrap();
        let p: Points =
            decode(&c.expected[&key(desc(&s, "ErasRewardPoints"), &[1u32.encode()])]).unwrap();
        assert_eq!(
            p.individual.into_iter().collect::<Vec<_>>(),
            vec![(id(8), 6), (id(9), 4)]
        );
        assert_eq!(p.total, 10);
    }
    #[test]
    fn missing_edit_claim_reset_and_added_async_queue_fail_private_certificate() {
        let (s, st) = fixture();
        let c = compile(&s, &st, &replacements(), &review(&s, &st)).unwrap();
        let mut missing = c.groups.clone();
        missing.pop();
        assert!(c.verify_groups(&missing, &replacements()).is_err());
        let mut reset = c.groups.clone();
        let claim = key(desc(&s, "ClaimedRewards"), &[1u32.encode(), id(9).encode()]);
        reset
            .iter_mut()
            .flatten()
            .find(|e| e.key == claim)
            .unwrap()
            .after = Some(Vec::<u32>::new().encode());
        assert!(c.verify_groups(&reset, &replacements()).is_err());
        let mut extra = c.groups.clone();
        extra.push(vec![GuardedStorageEdit {
            scope: StorageScope::Top,
            key: prefix("AsyncUnappliedSlashes"),
            before: None,
            after: Some(vec![0]),
        }]);
        assert!(c.verify_groups(&extra, &replacements()).is_err());
        assert!(c.verify_groups(&c.groups, &[]).is_err());
    }
    #[test]
    fn absent_inventory_marker_and_occupied_destination_fail() {
        let (s, mut st) = fixture();
        let mut r = review(&s, &st);
        r.absent_prefixes.pop();
        assert!(compile(&s, &st, &replacements(), &r).is_err());
        st.top.insert(MARKER.to_vec(), vec![0]);
        assert!(compile(&s, &st, &replacements(), &review(&s, &st)).is_err());
        st.top.insert(MARKER.to_vec(), vec![1]);
        let d = desc(&s, "SlashingSpans");
        st.top.insert(
            key(d, &[id(9).encode()]),
            st.top[&key(d, &[id(1).encode()])].clone(),
        );
        assert!(compile(&s, &st, &replacements(), &review(&s, &st)).is_err());
    }
    #[test]
    fn orphan_pages_bad_totals_and_span_relationships_fail() {
        let (s, st) = fixture();
        for mode in 0..3 {
            let mut bad = st.clone();
            if mode == 0 {
                bad.top.remove(&key(
                    desc(&s, "ErasStakersOverview"),
                    &[1u32.encode(), id(1).encode()],
                ));
            } else if mode == 1 {
                insert(
                    &s,
                    &mut bad,
                    "ErasStakersPaged",
                    &[1u32.encode(), id(1).encode(), 0u32.encode()],
                    Page {
                        page_total: 21,
                        others: vec![Individual {
                            who: id(3),
                            value: 20,
                        }],
                    },
                );
            } else {
                bad.top
                    .remove(&key(desc(&s, "SlashingSpans"), &[id(1).encode()]));
            }
            assert!(compile(&s, &bad, &replacements(), &review(&s, &bad)).is_err());
        }
    }
    #[test]
    fn third_party_economic_bytes_are_unchanged_and_unknown_metadata_fails() {
        let (s, st) = fixture();
        let c = compile(&s, &st, &[], &review(&s, &st)).unwrap();
        assert!(c.groups.is_empty());
        assert_eq!(c.before, c.expected);
        let mut wrong = s;
        wrong.metadata_hash = hx(&[8; 32]);
        assert!(compile(&wrong, &st, &[], &review(&wrong, &st)).is_err());
    }
}
