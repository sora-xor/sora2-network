//! Typed post-state gates before binding recovery work. This module never
//! changes membership, sorts stored account vectors, or invents issuance.
use crate::{
    economics, hex_bytes,
    inventory::{Descriptor, GuardedCandidate, Hasher, MetadataSchema},
    typed, Result,
};
use codec::{Decode, Encode};
use common::recovery::{
    invariants::TerminalAudit, GuardedStorageEdit, StorageScope, StorageWitness,
    MAX_COUNCIL_MEMBERS,
};
use scale_info::TypeDef;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet};

type Overlay<'a> = BTreeMap<(Option<Vec<u8>>, Vec<u8>), Option<&'a [u8]>>;
fn hx(b: &[u8]) -> String {
    format!("0x{}", hex::encode(b))
}
fn hash(b: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(b)
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing release-gate metadata {name}"))
}
fn read<'a>(storage: &'a Storage, overlay: &Overlay<'a>, key: &[u8]) -> Option<&'a [u8]> {
    overlay
        .get(&(None, key.to_vec()))
        .copied()
        .unwrap_or_else(|| storage.top.get(key).map(Vec::as_slice))
}
fn accounts_shape(schema: &MetadataSchema, mut id: u32) -> bool {
    for _ in 0..16 {
        match schema.registry.resolve(id).map(|t| &t.type_def) {
            Some(TypeDef::Composite(c)) if c.fields.len() == 1 => id = c.fields[0].ty.id,
            Some(TypeDef::Sequence(s)) => {
                return typed::account_type(&schema.registry, s.type_param.id)
            }
            _ => return false,
        }
    }
    false
}
fn members(
    schema: &MetadataSchema,
    storage: &Storage,
    overlay: &Overlay<'_>,
    name: &str,
) -> Result<(Vec<AccountId32>, StorageWitness, usize)> {
    let d = descriptor(schema, name)?;
    if !d.hashers.is_empty() || !accounts_shape(schema, d.value_type) {
        return Err(format!(
            "{name} does not have the reviewed AccountId sequence codec"
        ));
    }
    // These exact FRAME member stores use ValueQuery with an empty Vec default.
    // Preserve the raw absence in the witness rather than manufacturing a row.
    let record = read(storage, overlay, &d.prefix);
    let encoded_empty = Vec::<AccountId32>::new().encode();
    let bytes = record.unwrap_or(&encoded_empty);
    typed::decode_exact(bytes, d.value_type, &schema.registry)?;
    let mut input = bytes;
    let values = Vec::<AccountId32>::decode(&mut input).map_err(|e| e.to_string())?;
    if !input.is_empty()
        || values.encode() != bytes
        || values.len() > MAX_COUNCIL_MEMBERS
        || values.windows(2).any(|p| p[0] >= p[1])
    {
        return Err(format!(
            "{name} must be exactly encoded, unique and sorted within the 100-member bound"
        ));
    }
    Ok((
        values,
        StorageWitness {
            scope: StorageScope::Top,
            key: d.prefix.clone(),
            value_hash: record.map(hash),
        },
        record.map_or(0, <[u8]>::len),
    ))
}
/// Observe typed checkpoint membership for an isolated diagnostic rehearsal.
/// This is not the approved recovered-council list and cannot authorize release.
pub fn checkpoint_council(schema: &MetadataSchema, storage: &Storage) -> Result<Vec<AccountId32>> {
    members(schema, storage, &BTreeMap::new(), "Council.Members").map(|(members, _, _)| members)
}
fn prime(
    schema: &MetadataSchema,
    storage: &Storage,
    overlay: &Overlay<'_>,
    name: &str,
    members: &[AccountId32],
) -> Result<(Option<AccountId32>, StorageWitness, usize)> {
    let d = descriptor(schema, name)?;
    if !d.hashers.is_empty() || !typed::account_type(&schema.registry, d.value_type) {
        return Err(format!("{name} is not a typed optional-storage AccountId"));
    }
    let bytes = read(storage, overlay, &d.prefix);
    let value = if let Some(bytes) = bytes {
        typed::decode_exact(bytes, d.value_type, &schema.registry)?;
        let account = AccountId32::decode(&mut &bytes[..]).map_err(|e| e.to_string())?;
        if bytes.len() != 32 || !members.contains(&account) {
            return Err(format!("{name} is outside projected membership"));
        }
        Some(account)
    } else {
        None
    };
    Ok((
        value,
        StorageWitness {
            scope: StorageScope::Top,
            key: d.prefix.clone(),
            value_hash: bytes.map(hash),
        },
        bytes.map_or(0, <[u8]>::len),
    ))
}
fn components(schema: &MetadataSchema, d: &Descriptor, key: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut input = key
        .strip_prefix(d.prefix.as_slice())
        .ok_or("mapping key prefix mismatch")?;
    let mut out = Vec::new();
    for (hasher, id) in d.hashers.iter().zip(&d.key_types) {
        let width = match hasher {
            Hasher::Identity => 0,
            Hasher::Twox64Concat => 8,
            Hasher::Blake2_128Concat => 16,
            _ => return Err("mapping supply gate requires exact reversible asset key".into()),
        };
        if input.len() < width {
            return Err("truncated mapping key hash".into());
        }
        let (encoded, _, consumed) =
            typed::transform_prefix(&input[width..], *id, &schema.registry, &BTreeMap::new())?;
        if hasher.apply(&encoded) != input[..width + consumed] {
            return Err("mapping asset key hash differs".into());
        }
        input = &input[width + consumed..];
        out.push(encoded);
    }
    if !input.is_empty() {
        return Err("mapping key trailing bytes".into());
    }
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionReport {
    pub council: Vec<String>,
    pub technical_committee: Vec<String>,
    pub added_unchanged_issuance_assets: Vec<String>,
    pub governance_witnesses: Vec<StorageWitnessSummary>,
    pub preserved_technical_discrepancy: Option<String>,
    pub production_ready: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageWitnessSummary {
    pub key: String,
    pub value_hash: Option<String>,
    pub bytes: usize,
}
pub struct CheckedProjection {
    pub terminal: TerminalAudit,
    /// Exact records that must also be included in committed on-chain audits.
    pub governance_witnesses: Vec<(StorageWitness, usize)>,
    pub report: ProjectionReport,
}
/// Explicit retention of a pre-existing technical membership/collective
/// discrepancy. Hashes bind raw storage, including absence; this authorizes no
/// governance mutation and never waives mapping/control reconciliation.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GovernancePreservation {
    pub membership: Option<String>,
    pub committee: Option<String>,
    pub membership_prime: Option<String>,
    pub committee_prime: Option<String>,
    pub review_evidence_hash: String,
}
#[derive(Default)]
pub struct GovernanceReview<'a> {
    pub replacements: &'a [common::recovery::AccountReplacement],
    pub preservation: Option<&'a GovernancePreservation>,
    pub transformation: Option<&'a crate::governance::Compilation>,
}
pub fn check(
    schema: &MetadataSchema,
    storage: &Storage,
    groups: &[Vec<GuardedStorageEdit>],
    mapping_candidates: &[GuardedCandidate],
    council: &[AccountId32],
    terminal: TerminalAudit,
) -> Result<CheckedProjection> {
    check_with_governance_preservation(
        schema,
        storage,
        groups,
        mapping_candidates,
        council,
        terminal,
        GovernanceReview::default(),
    )
}
pub fn check_with_governance_preservation(
    schema: &MetadataSchema,
    storage: &Storage,
    groups: &[Vec<GuardedStorageEdit>],
    mapping_candidates: &[GuardedCandidate],
    council: &[AccountId32],
    mut terminal: TerminalAudit,
    review: GovernanceReview<'_>,
) -> Result<CheckedProjection> {
    if council.is_empty()
        || council.len() > MAX_COUNCIL_MEMBERS
        || council.windows(2).any(|p| p[0] >= p[1])
    {
        return Err("recovered council must be nonempty, unique and sorted".into());
    }
    if review
        .transformation
        .is_some_and(|compilation| !compilation.certifies_work(review.replacements, groups))
    {
        return Err("governance transformation requires its exact replacement map and complete pending-operation cancellations".into());
    }
    let mut overlay = Overlay::new();
    for edit in groups.iter().flatten() {
        let (trie, old) = match &edit.scope {
            StorageScope::Top => (None, storage.top.get(&edit.key)),
            StorageScope::Child(id) => (
                Some(id.clone()),
                storage
                    .children_default
                    .get(id)
                    .and_then(|t| t.data.get(&edit.key)),
            ),
        };
        if old.map(|v| hash(v)) != edit.before {
            return Err("projected edit does not match checkpoint".into());
        }
        if overlay
            .insert((trie, edit.key.clone()), edit.after.as_deref())
            .is_some()
        {
            return Err("overlapping projected edits cannot be ordered away".into());
        }
    }
    let (stored_council, council_witness, council_bytes) =
        members(schema, storage, &overlay, "Council.Members")?;
    if stored_council != council {
        return Err("projected Council.Members differs from manifest recovered council".into());
    }
    let (membership, membership_witness, membership_bytes) =
        members(schema, storage, &overlay, "TechnicalMembership.Members")?;
    let (committee, committee_witness, committee_bytes) =
        members(schema, storage, &overlay, "TechnicalCommittee.Members")?;
    let (_, council_prime, council_prime_bytes) =
        prime(schema, storage, &overlay, "Council.Prime", &stored_council)?;
    let (membership_prime, membership_prime_witness, membership_prime_bytes) = prime(
        schema,
        storage,
        &overlay,
        "TechnicalMembership.Prime",
        &membership,
    )?;
    let (committee_prime, committee_prime_witness, committee_prime_bytes) = prime(
        schema,
        storage,
        &overlay,
        "TechnicalCommittee.Prime",
        &committee,
    )?;
    let discrepancy = membership != committee || membership_prime != committee_prime;
    let transformed_discrepancy = if discrepancy {
        review.transformation.filter(|compilation| {
            compilation.certifies_work(review.replacements, groups)
                && [
                    &membership_witness,
                    &committee_witness,
                    &membership_prime_witness,
                    &committee_prime_witness,
                ]
                .iter()
                .all(|witness| {
                    compilation.certifies_technical(
                        &witness.key,
                        storage.top.get(&witness.key).map(Vec::as_slice),
                        read(storage, &overlay, &witness.key),
                    )
                })
        })
    } else {
        None
    };
    let preserved_technical_discrepancy = if let Some(compilation) = transformed_discrepancy {
        if review.preservation.is_some() {
            return Err(
                "governance transformation cannot also use an unchanged preservation waiver".into(),
            );
        }
        Some(compilation.evidence_hash().to_owned())
    } else if discrepancy {
        let evidence = review.preservation.ok_or("projected TechnicalMembership/TechnicalCommittee differ; explicit unchanged-state preservation or paired repair required")?;
        let hash32 = |value: &str| -> Result<[u8; 32]> {
            let bytes = hex_bytes(value)?;
            let hash: [u8; 32] = bytes
                .try_into()
                .map_err(|_| "governance evidence must be 32 bytes")?;
            if hash == [0; 32] {
                return Err("zero governance review commitment".into());
            }
            Ok(hash)
        };
        hash32(&evidence.review_evidence_hash)?;
        for (witness, expected) in [
            (&membership_witness, &evidence.membership),
            (&committee_witness, &evidence.committee),
            (&membership_prime_witness, &evidence.membership_prime),
            (&committee_prime_witness, &evidence.committee_prime),
        ] {
            if overlay.contains_key(&(None, witness.key.clone()))
                || expected.as_deref().map(hash32).transpose()? != witness.value_hash
                || storage.top.get(&witness.key).map(|bytes| hash(bytes)) != witness.value_hash
            {
                return Err(
                    "governance preservation must bind exact unchanged checkpoint records".into(),
                );
            }
        }
        let identities: BTreeSet<_> = membership
            .iter()
            .chain(committee.iter())
            .chain(membership_prime.iter())
            .chain(committee_prime.iter())
            .collect();
        if review.replacements.iter().any(|replacement| {
            identities.contains(&replacement.old) || identities.contains(&replacement.new)
        }) {
            return Err("mapped technical member/prime requires paired semantic repair; preservation is not allowed".into());
        }
        Some(evidence.review_evidence_hash.clone())
    } else {
        if review.preservation.is_some() {
            return Err("unused governance discrepancy preservation evidence".into());
        }
        None
    };
    let mut assets = BTreeSet::from([common::XOR
        .encode()
        .try_into()
        .map_err(|_| "native asset codec")?]);
    for candidate in mapping_candidates {
        if candidate.storage != "Tokens.Accounts" {
            continue;
        }
        if candidate.trie != "top" {
            return Err("token mapping has unexpected child-trie scope".into());
        }
        let d = descriptor(schema, "Tokens.Accounts")?;
        if d.key_types.len() != 2 || !typed::account_type(&schema.registry, d.key_types[0]) {
            return Err("Tokens.Accounts owner/asset key codec differs".into());
        }
        let before = components(schema, d, &hex_bytes(&candidate.source_key)?)?;
        let after = components(schema, d, &hex_bytes(&candidate.destination_key)?)?;
        if before.len() != 2 || after.len() != 2 || before[1] != after[1] {
            return Err("account replacement would alter the asset identity".into());
        }
        assets.insert(
            before[1]
                .clone()
                .try_into()
                .map_err(|_| "mapped asset must encode exactly 32 bytes")?,
        );
    }
    // Catch-up can mint an asset not owned by a mapped account at checkpoint.
    // Cover all registered checkpoint assets plus every retained issuance row.
    // Terminal paging keeps this complete inventory bounded per recovery block.
    for name in ["Tokens.TotalIssuance", "Assets.AssetInfosV2"] {
        if let Some(d) = schema
            .descriptors
            .iter()
            .find(|d| d.trie == "top" && d.name == name)
        {
            if d.key_types.len() != 1 {
                return Err("asset inventory key codec differs".into());
            }
            for key in storage.top.keys().filter(|key| key.starts_with(&d.prefix)) {
                let parts = components(schema, d, key)?;
                assets.insert(
                    parts
                        .first()
                        .ok_or("asset inventory key absent")?
                        .as_slice()
                        .try_into()
                        .map_err(|_| "registered asset must encode exactly 32 bytes")?,
                );
            }
        }
    }
    let mut added = Vec::new();
    for asset in assets {
        if !terminal.issuance.iter().any(|i| i.asset == asset) {
            terminal
                .issuance
                .push(economics::issuance_baseline(schema, storage, asset)?);
            added.push(hx(&asset));
        }
    }
    // Check every supply projection against post-state, preserving existing
    // authenticated mint/burn arithmetic rather than replacing it with zero.
    for issuance in &terminal.issuance {
        if issuance.authenticated_mints != 0 {
            return Err(
                "incoming issuance must be derived by native execution, not supplied amounts"
                    .into(),
            );
        }
        if issuance.current.scope != StorageScope::Top {
            return Err("supply projection is not top-level storage".into());
        }
        let zero = 0u128.encode();
        let bytes = read(storage, &overlay, &issuance.current.key).unwrap_or(&zero);
        let offset = issuance.current.offset as usize;
        if hash(bytes) != issuance.current.record_hash
            || offset.checked_add(16).is_none_or(|end| end > bytes.len())
        {
            return Err("projected supply hash/offset mismatch".into());
        }
        let actual = u128::from_le_bytes(
            bytes[offset..offset + 16]
                .try_into()
                .map_err(|_| "supply width")?,
        );
        let expected = issuance
            .checkpoint_issuance
            .checked_add(issuance.authenticated_mints)
            .and_then(|n| n.checked_sub(issuance.burned))
            .ok_or("supply equation overflow/underflow")?;
        if actual != expected {
            return Err("projected issuance violates conservation".into());
        }
    }
    common::recovery::invariants::terminal_pages(&terminal)
        .map_err(|e| format!("projected audit bounds: {e:?}"))?;
    let governance_witnesses = vec![
        (council_witness, council_bytes),
        (membership_witness, membership_bytes),
        (committee_witness, committee_bytes),
        (council_prime, council_prime_bytes),
        (membership_prime_witness, membership_prime_bytes),
        (committee_prime_witness, committee_prime_bytes),
    ];
    let report = ProjectionReport {
        council: stored_council.iter().map(|a| hx(a.as_ref())).collect(),
        technical_committee: committee.iter().map(|a| hx(a.as_ref())).collect(),
        added_unchanged_issuance_assets: added,
        governance_witnesses: governance_witnesses
            .iter()
            .map(|(w, bytes)| StorageWitnessSummary {
                key: hx(&w.key),
                value_hash: w.value_hash.map(|h| hx(&h)),
                bytes: *bytes,
            })
            .collect(),
        preserved_technical_discrepancy,
        production_ready: false,
    };
    Ok(CheckedProjection {
        terminal,
        governance_witnesses,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry};
    fn fixture() -> (MetadataSchema, Storage, TerminalAudit) {
        let mut r = Registry::new();
        let account = r.register_type(&meta_type::<AccountId32>()).id;
        let accounts = r.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let balance = r.register_type(&meta_type::<u128>()).id;
        let asset = r.register_type(&meta_type::<[u8; 32]>()).id;
        let names = [
            "Council.Members",
            "TechnicalMembership.Members",
            "TechnicalCommittee.Members",
            "Council.Prime",
            "TechnicalMembership.Prime",
            "TechnicalCommittee.Prime",
            "Balances.TotalIssuance",
        ];
        let mut descriptors = names
            .iter()
            .enumerate()
            .map(|(i, n)| Descriptor {
                trie: "top".into(),
                name: (*n).into(),
                prefix: vec![i as u8 + 1; 32],
                key_types: vec![],
                hashers: vec![],
                value_type: if i < 3 {
                    accounts
                } else if i < 6 {
                    account
                } else {
                    balance
                },
            })
            .collect::<Vec<_>>();
        descriptors.push(Descriptor {
            trie: "top".into(),
            name: "Tokens.Accounts".into(),
            prefix: vec![8; 32],
            key_types: vec![account, asset],
            hashers: vec![Hasher::Blake2_128Concat, Hasher::Twox64Concat],
            value_type: balance,
        });
        descriptors.push(Descriptor {
            trie: "top".into(),
            name: "Tokens.TotalIssuance".into(),
            prefix: vec![9; 32],
            key_types: vec![asset],
            hashers: vec![Hasher::Twox64Concat],
            value_type: balance,
        });
        let mut storage = Storage::default();
        storage
            .top
            .insert(vec![1; 32], vec![AccountId32::from([1; 32])].encode());
        storage
            .top
            .insert(vec![2; 32], vec![AccountId32::from([2; 32])].encode());
        storage
            .top
            .insert(vec![3; 32], vec![AccountId32::from([2; 32])].encode());
        storage.top.insert(vec![7; 32], 99u128.encode());
        let mut issuance = vec![9; 32];
        issuance.extend(Hasher::Twox64Concat.apply(&[7; 32]));
        storage.top.insert(issuance, 1000u128.encode());
        (
            MetadataSchema {
                registry: PortableRegistry::from(r),
                descriptors,
                metadata_hash: hx(&[1; 32]),
            },
            storage,
            TerminalAudit {
                issuance: vec![],
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            },
        )
    }
    #[test]
    fn absent_membership_is_preserved_only_with_exact_unaffected_checkpoint_evidence() {
        let (schema, mut storage, terminal) = fixture();
        storage.top.remove(&vec![2; 32]);
        let council = vec![AccountId32::from([1; 32])];
        assert!(check(&schema, &storage, &[], &[], &council, terminal.clone()).is_err());
        let mut preservation = GovernancePreservation {
            membership: None,
            committee: Some(hx(&hash(&storage.top[&vec![3; 32]]))),
            membership_prime: None,
            committee_prime: None,
            review_evidence_hash: hx(&[9; 32]),
        };
        let check_preserved = |preservation: &GovernancePreservation,
                               replacements: &[common::recovery::AccountReplacement],
                               groups: &[Vec<GuardedStorageEdit>]| {
            check_with_governance_preservation(
                &schema,
                &storage,
                groups,
                &[],
                &council,
                terminal.clone(),
                GovernanceReview {
                    replacements,
                    preservation: Some(preservation),
                    transformation: None,
                },
            )
        };
        let checked = check_preserved(&preservation, &[], &[]).unwrap();
        assert!(checked.report.preserved_technical_discrepancy.is_some());
        assert_eq!(checked.report.technical_committee, vec![hx(&[2; 32])]);
        assert_eq!(checked.governance_witnesses[1].0.value_hash, None);
        assert!(check_preserved(
            &preservation,
            &[common::recovery::AccountReplacement {
                old: [2; 32].into(),
                new: [3; 32].into()
            }],
            &[]
        )
        .is_err());
        let edited = GuardedStorageEdit {
            scope: StorageScope::Top,
            key: vec![3; 32],
            before: Some(hash(&storage.top[&vec![3; 32]])),
            after: Some(vec![AccountId32::from([4; 32])].encode()),
        };
        assert!(check_preserved(&preservation, &[], &[vec![edited]]).is_err());
        preservation.committee = Some(hx(&[8; 32]));
        assert!(check_preserved(&preservation, &[], &[]).is_err());
    }
    #[test]
    fn council_matches_manifest_while_technical_members_remain_distinct_and_audited() {
        let (schema, storage, terminal) = fixture();
        let checked = check(
            &schema,
            &storage,
            &[],
            &[],
            &[AccountId32::from([1; 32])],
            terminal.clone(),
        )
        .unwrap();
        assert_eq!(checked.governance_witnesses.len(), 6);
        assert_ne!(checked.report.council, checked.report.technical_committee);
        assert_eq!(checked.terminal.issuance[0].checkpoint_issuance, 99);
        assert!(check(
            &schema,
            &storage,
            &[],
            &[],
            &[AccountId32::from([2; 32])],
            terminal
        )
        .is_err());
    }
    #[test]
    fn projected_membership_order_prime_and_issuance_cannot_be_forged() {
        let (schema, storage, terminal) = fixture();
        let edit = |key: Vec<u8>, after: Vec<u8>| GuardedStorageEdit {
            scope: StorageScope::Top,
            before: storage.top.get(&key).map(|v| hash(v)),
            key,
            after: Some(after),
        };
        for edits in [
            vec![edit(vec![3; 32], vec![AccountId32::from([3; 32])].encode())],
            vec![edit(
                vec![1; 32],
                vec![AccountId32::from([2; 32]), AccountId32::from([1; 32])].encode(),
            )],
            vec![edit(vec![5; 32], AccountId32::from([2; 32]).encode())],
            vec![edit(vec![7; 32], 100u128.encode())],
        ] {
            assert!(check(
                &schema,
                &storage,
                &[edits],
                &[],
                &[AccountId32::from([1; 32])],
                terminal.clone()
            )
            .is_err());
        }
    }
    #[test]
    fn token_owner_mapping_adds_unchanged_asset_supply_guard() {
        let (schema, mut storage, terminal) = fixture();
        let token_key = |owner: [u8; 32]| {
            let mut k = vec![8; 32];
            k.extend(Hasher::Blake2_128Concat.apply(&owner));
            k.extend(Hasher::Twox64Concat.apply(&[7; 32]));
            k
        };
        let old = token_key([3; 32]);
        let new = token_key([4; 32]);
        let value = 55u128.encode();
        storage.top.insert(old.clone(), value.clone());
        let candidate = GuardedCandidate {
            trie: "top".into(),
            storage: "Tokens.Accounts".into(),
            source_key: hx(&old),
            destination_key: hx(&new),
            expected_source_value_hash: hx(&hash(&value)),
            require_destination_absent: true,
            replacement_value: hx(&value),
            disposition_evidence_hash: hx(&[9; 32]),
        };
        let groups = vec![vec![
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: old,
                before: Some(hash(&value)),
                after: None,
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: new,
                before: None,
                after: Some(value),
            },
        ]];
        let checked = check(
            &schema,
            &storage,
            &groups,
            &[candidate],
            &[AccountId32::from([1; 32])],
            terminal,
        )
        .unwrap();
        assert!(checked
            .terminal
            .issuance
            .iter()
            .any(|i| i.asset == [7; 32] && i.checkpoint_issuance == 1000 && i.burned == 0));
    }
}
