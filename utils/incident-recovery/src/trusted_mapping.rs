//! Trusted recovery CSV admission. Authorization comes from the release's CSV,
//! not from a signature by a compromised identity or a replacement wallet.
//! State authentication is supplied by the caller's verified checkpoint.

use crate::{
    inventory::{self, InventoryOptions, MetadataSchema},
    substitution_semantics::{self, DerivedAccount, DerivedKind},
    Result,
};
use codec::Encode;
use common::recovery::{self, AccountReplacement};
use serde::Serialize;
use sp_core::{crypto::AccountId32, storage::Storage};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const AUTHORIZATION_POLICY: &str = "trusted-csv-v1";
const MAX_DERIVATIVE_DEPTH: usize = 32;

fn hx(value: &[u8]) -> String {
    format!("0x{}", hex::encode(value))
}
fn commitment(map: &[AccountReplacement]) -> String {
    hx(&sp_io::hashing::blake2_256(&map.encode()))
}
fn bytes(account: &AccountId32) -> [u8; 32] {
    *AsRef::<[u8; 32]>::as_ref(account)
}
fn canonical(map: &BTreeMap<[u8; 32], [u8; 32]>) -> Vec<AccountReplacement> {
    map.iter()
        .map(|(old, new)| AccountReplacement {
            old: (*old).into(),
            new: (*new).into(),
        })
        .collect()
}

/// Pallet/technical identities have no independently owned destination key.
/// Registered bridge and other occupied identities are excluded separately by
/// the complete typed checkpoint destination inventory.
fn validate_destination(new: &[u8; 32]) -> Result<()> {
    if new == &[0; 32]
        || new == &recovery::ATTACKER_ACCOUNT
        || new.starts_with(b"modl")
        || new.starts_with(&common::TECH_ACCOUNT_MAGIC_PREFIX)
    {
        return Err(
            "replacement destination is a zero, attacker, pallet or technical identity".into(),
        );
    }
    Ok(())
}

pub struct TrustedMapping {
    replacements: Vec<AccountReplacement>,
    derived: Vec<DerivedAccount>,
    council: Vec<AccountId32>,
    pub report: TrustedMappingReport,
}
impl TrustedMapping {
    pub fn replacements(&self) -> &[AccountReplacement] {
        &self.replacements
    }
    pub fn derived(&self) -> &[DerivedAccount] {
        &self.derived
    }
    pub fn council(&self) -> &[AccountId32] {
        &self.council
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedMappingReport {
    pub schema: &'static str,
    pub authorization_policy: &'static str,
    pub ownership_signatures_required: bool,
    pub checkpoint_hash: String,
    pub metadata_hash: String,
    pub original_mappings: usize,
    pub expanded_mappings: usize,
    pub trusted_csv_commitment: String,
    pub expanded_mapping_commitment: String,
    pub replacements_scale: String,
    pub derived_accounts: Vec<DerivedAccount>,
    pub automatic_derivative_count: usize,
    pub explicit_derivative_count: usize,
    pub direct_rows_without_discoverable_account_state: Vec<String>,
    pub unused_destinations_verified: bool,
    pub all_entries_classified: bool,
    pub checkpoint_council: Vec<String>,
    pub recovered_council: Vec<String>,
    pub excluded_attacker_council_members: usize,
    pub production_ready: bool,
    pub scope: &'static str,
}

/// Expand every discoverable utility descendant. Supplied multisig rows are
/// ordinary trusted CSV pairs: this does not invent a multisig's signatories.
/// A contradictory explicit utility pair is rejected, never silently replaced.
fn expand(
    replacements: &[AccountReplacement],
    holders: &std::collections::HashSet<[u8; 32]>,
) -> Result<(Vec<AccountReplacement>, Vec<DerivedAccount>, usize)> {
    recovery::validate_replacements(replacements).map_err(|e| format!("invalid mapping: {e:?}"))?;
    if replacements.is_empty() || replacements.len() > recovery::MAX_REPLACEMENTS {
        return Err("trusted CSV must contain 1..=4096 mappings".into());
    }
    let mut mappings: BTreeMap<_, _> = replacements
        .iter()
        .map(|r| (bytes(&r.old), bytes(&r.new)))
        .collect();
    let explicit: BTreeSet<_> = mappings.keys().copied().collect();
    let mut destinations: BTreeSet<_> = mappings.values().copied().collect();
    for new in &destinations {
        validate_destination(new)?;
    }
    let mut queue: VecDeque<_> = mappings
        .iter()
        .map(|(old, new)| (*old, *new, 0usize))
        .collect();
    let mut scanned = BTreeSet::new();
    let mut discovered = BTreeMap::new();
    let mut explicit_derivatives = 0;
    while let Some((old, new, depth)) = queue.pop_front() {
        if !scanned.insert(old) {
            continue;
        }
        if holders.is_empty() {
            break;
        }
        // Encode the native derivation tuple once per parent. The u16 suffix
        // is fixed-width SCALE; avoid millions of temporary heap allocations.
        let mut preimage = (b"modlpy/utilisuba", old, 0u16).encode();
        let suffix = preimage.len() - 2;
        for index in 0..=u16::MAX {
            preimage[suffix..].copy_from_slice(&index.to_le_bytes());
            let from = sp_io::hashing::blake2_256(&preimage);
            if !holders.contains(&from) {
                continue;
            }
            if depth >= MAX_DERIVATIVE_DEPTH {
                return Err("utility derivative expansion exceeds depth bound".into());
            }
            let to = substitution_semantics::derivative_account(new, index);
            validate_destination(&to)?;
            if from == recovery::ATTACKER_ACCOUNT || from == to || destinations.contains(&from) {
                return Err(
                    "derived mapping overlaps an attacker, destination or unchanged identity"
                        .into(),
                );
            }
            if let Some(previous) = discovered.get(&from) {
                let previous: &DerivedAccount = previous;
                if previous.new != hx(&to) {
                    return Err("ambiguous utility derivative destination".into());
                }
                // Two distinct derivations cannot be silently collapsed even
                // if an unlikely address/hash collision keeps the destination.
                return Err("utility derivative has multiple controlling derivations".into());
            }
            match mappings.get(&from) {
                Some(mapped) if *mapped != to => {
                    return Err(
                        "CSV utility derivative conflicts with same-index parent remapping".into(),
                    )
                }
                Some(_) => {
                    explicit_derivatives += 1;
                }
                None => {
                    if mappings.len() >= recovery::MAX_REPLACEMENTS
                        || mappings.contains_key(&to)
                        || !destinations.insert(to)
                    {
                        return Err("utility expansion exceeds bounds or collides with the replacement mapping".into());
                    }
                    mappings.insert(from, to);
                }
            }
            discovered.insert(
                from,
                DerivedAccount {
                    old: hx(&from),
                    new: hx(&to),
                    derivation: DerivedKind::Utility {
                        controller: hx(&old),
                        index,
                    },
                },
            );
            // Explicit descendants were queued already; scanning each owner
            // once makes the result independent of CSV input ordering.
            if !explicit.contains(&from) {
                queue.push_back((from, to, depth + 1));
            }
        }
    }
    // CSV-listed descendants are scanned as roots for completeness, but that
    // must not let an explicit row reset the final graph's depth bound.
    for start in discovered.keys() {
        let mut current = *start;
        let mut visited = BTreeSet::new();
        while let Some(row) = discovered.get(&current) {
            if !visited.insert(current) || visited.len() > MAX_DERIVATIVE_DEPTH {
                return Err("utility derivative graph cycles or exceeds depth bound".into());
            }
            let DerivedKind::Utility { controller, .. } = &row.derivation else {
                return Err("automatic derivative has a non-utility derivation".into());
            };
            current = bytes(&crate::account(controller)?);
        }
    }
    let expanded = canonical(&mappings);
    recovery::validate_replacements(&expanded)
        .map_err(|e| format!("invalid expanded mapping: {e:?}"))?;
    Ok((
        expanded,
        discovered.into_values().collect(),
        explicit_derivatives,
    ))
}

/// Requires checkpoint-authenticated schema/state. The complete inventory may
/// retain explicitly reviewed opaque history, but unknown storage cannot stand
/// in for destination non-use. Unresolved transform semantics remain blockers
/// for composition, not a reason to demand replacement-key signatures.
pub fn verify(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    options: &InventoryOptions,
) -> Result<TrustedMapping> {
    if !options.additional_storage.is_empty() {
        return Err(
            "trusted mapping cannot certify destinations with caller-provided diagnostic codecs"
                .into(),
        );
    }
    let holders = substitution_semantics::discoverable_utility_state_owners(schema, storage)?;
    let (expanded, derived, explicit_derivatives) = expand(replacements, &holders)?;
    let mappings = expanded
        .iter()
        .map(|r| (bytes(&r.old), bytes(&r.new)))
        .collect();
    let inventory = inventory::inventory(schema, storage, &mappings, options)?;
    if !inventory.all_entries_classified
        || !inventory.unused_destinations_verified
        || !inventory.used_destinations.is_empty()
    {
        return Err(
            "trusted mapping requires complete typed inventory and unused replacement destinations"
                .into(),
        );
    }
    let checkpoint_council = crate::release_checks::checkpoint_council(schema, storage)?;
    let council = recovered_council(&checkpoint_council, &mappings)?;
    let report = TrustedMappingReport {
        schema: "sora-nexus-trusted-csv-mapping-v1",
        authorization_policy: AUTHORIZATION_POLICY,
        ownership_signatures_required: false,
        checkpoint_hash: hx(&recovery::CHECKPOINT_HASH), metadata_hash: schema.metadata_hash.clone(),
        original_mappings: replacements.len(), expanded_mappings: expanded.len(),
        trusted_csv_commitment: commitment(replacements), expanded_mapping_commitment: commitment(&expanded),
        replacements_scale: hx(&expanded.encode()), derived_accounts: derived.clone(),
        automatic_derivative_count: expanded.len() - replacements.len(), explicit_derivative_count: explicit_derivatives,
        direct_rows_without_discoverable_account_state: replacements.iter().filter(|r| !holders.contains(&bytes(&r.old))).map(|r| hx(r.old.as_ref())).collect(),
        unused_destinations_verified: true, all_entries_classified: true,
        checkpoint_council: checkpoint_council.iter().map(|a| hx(a.as_ref())).collect(),
        recovered_council: council.iter().map(|a| hx(a.as_ref())).collect(),
        excluded_attacker_council_members: checkpoint_council.iter().filter(|a| bytes(a) == recovery::ATTACKER_ACCOUNT).count(),
        production_ready: false,
        scope: "Trusted release CSV authorization; no wallet ownership signature is required or claimed. Utility descendants with System.Account, Tokens.Accounts or crowdloan state are deterministically expanded. Multisigs require explicit CSV rows. No-state warnings mean only absence from those discovery prefixes. Complete semantic, economic, consensus and exact-artifact execution gates remain separate.",
    };
    Ok(TrustedMapping {
        replacements: expanded,
        derived,
        council,
        report,
    })
}

fn recovered_council(
    checkpoint: &[AccountId32],
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
) -> Result<Vec<AccountId32>> {
    if checkpoint.is_empty()
        || checkpoint.len() > recovery::MAX_COUNCIL_MEMBERS
        || checkpoint.windows(2).any(|p| p[0] >= p[1])
    {
        return Err("checkpoint council is empty, oversized or noncanonical".into());
    }
    let mut recovered: Vec<_> = checkpoint
        .iter()
        .filter(|a| bytes(a) != recovery::ATTACKER_ACCOUNT)
        .map(|a| AccountId32::from(mappings.get(&bytes(a)).copied().unwrap_or_else(|| bytes(a))))
        .collect();
    recovered.sort();
    if recovered.is_empty() || recovered.windows(2).any(|p| p[0] >= p[1]) {
        return Err("recovered council is empty or collides".into());
    }
    Ok(recovered)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn map(old: [u8; 32], new: [u8; 32]) -> Vec<AccountReplacement> {
        vec![AccountReplacement {
            old: old.into(),
            new: new.into(),
        }]
    }
    #[test]
    fn unsigned_csv_expands_funded_and_nested_utility_same_indices() {
        let old = [1; 32];
        let new = [2; 32];
        let a = substitution_semantics::derivative_account(old, 65535);
        let b = substitution_semantics::derivative_account(a, 7);
        let aa = substitution_semantics::derivative_account(new, 65535);
        let bb = substitution_semantics::derivative_account(aa, 7);
        let (out, generated, explicit) =
            expand(&map(old, new), &[a, b].into_iter().collect()).unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(generated.len(), 2);
        assert_eq!(explicit, 0);
        assert!(out
            .iter()
            .any(|r| bytes(&r.old) == b && bytes(&r.new) == bb));
        assert!(!out
            .iter()
            .any(|r| bytes(&r.old) == substitution_semantics::derivative_account(old, 6)));
    }
    #[test]
    fn explicit_utility_conflict_and_destination_collisions_fail() {
        let a = substitution_semantics::derivative_account([1; 32], 3);
        let aa = substitution_semantics::derivative_account([2; 32], 3);
        for (old, new) in [(a, [8; 32]), ([9; 32], aa), (aa, [9; 32])] {
            let mut rows = map([1; 32], [2; 32]);
            rows.extend(map(old, new));
            rows.sort_by(|a, b| a.old.cmp(&b.old));
            assert!(expand(&rows, &[a].into_iter().collect()).is_err());
        }
        let mut rows = map([1; 32], [2; 32]);
        rows.extend(map(a, aa));
        rows.sort_by(|a, b| a.old.cmp(&b.old));
        let (out, generated, count) = expand(&rows, &[a].into_iter().collect()).unwrap();
        assert_eq!(out, rows);
        assert_eq!(generated.len(), 1);
        assert_eq!(count, 1);
    }
    #[test]
    fn excluded_destination_and_attacker_pairs_fail() {
        let mut pallet = [7; 32];
        pallet[..4].copy_from_slice(b"modl");
        let mut technical = [7; 32];
        technical[..16].copy_from_slice(&common::TECH_ACCOUNT_MAGIC_PREFIX);
        for dest in [
            [0; 32],
            recovery::ATTACKER_ACCOUNT,
            pallet,
            technical,
            [1; 32],
        ] {
            assert!(expand(&map([1; 32], dest), &Default::default()).is_err());
        }
        assert!(expand(
            &map(recovery::ATTACKER_ACCOUNT, [2; 32]),
            &Default::default()
        )
        .is_err());
    }
    #[test]
    fn council_is_checkpoint_remapped_with_attacker_excluded() {
        let mut council = vec![
            [1; 32].into(),
            [3; 32].into(),
            recovery::ATTACKER_ACCOUNT.into(),
        ];
        council.sort();
        let got = recovered_council(&council, &BTreeMap::from([([1; 32], [2; 32])])).unwrap();
        assert_eq!(
            got,
            vec![AccountId32::from([2; 32]), AccountId32::from([3; 32])]
        );
        assert!(recovered_council(&[recovery::ATTACKER_ACCOUNT.into()], &BTreeMap::new()).is_err());
        assert!(recovered_council(&council, &BTreeMap::from([([1; 32], [3; 32])])).is_err());
    }
    #[test]
    fn manual_multisig_rows_and_no_state_rows_need_no_signatures() {
        let rows = map([5; 32], [6; 32]);
        let (out, derived, _) = expand(&rows, &Default::default()).unwrap();
        assert_eq!(out, rows);
        assert!(derived.is_empty());
        assert_ne!(commitment(&out), commitment(&map([5; 32], [7; 32])));
    }
    fn checkpoint_fixture() -> (MetadataSchema, Storage, InventoryOptions) {
        use crate::inventory::{Descriptor, Hasher};
        use scale_info::{meta_type, Registry};
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let balance = registry.register_type(&meta_type::<u128>()).id;
        let members = registry.register_type(&meta_type::<Vec<AccountId32>>()).id;
        let prefix = |pallet: &[u8], item: &[u8]| {
            [
                sp_io::hashing::twox_128(pallet),
                sp_io::hashing::twox_128(item),
            ]
            .concat()
        };
        let system = Descriptor {
            trie: "top".into(),
            name: "System.Account".into(),
            prefix: prefix(b"System", b"Account"),
            key_types: vec![account],
            hashers: vec![Hasher::Blake2_128Concat],
            value_type: balance,
        };
        let council = Descriptor {
            trie: "top".into(),
            name: "Council.Members".into(),
            prefix: prefix(b"Council", b"Members"),
            key_types: vec![],
            hashers: vec![],
            value_type: members,
        };
        let mut storage = Storage::default();
        storage.top.insert(
            council.prefix.clone(),
            vec![AccountId32::from([3; 32])].encode(),
        );
        let schema = MetadataSchema {
            registry: registry.into(),
            descriptors: vec![system, council],
            metadata_hash: hx(&[4; 32]),
        };
        (schema, storage, InventoryOptions::default())
    }
    #[test]
    fn trusted_witness_binds_unsigned_expansion_and_unused_destinations() {
        use crate::inventory::Hasher;
        let (schema, mut storage, options) = checkpoint_fixture();
        let derived = substitution_semantics::derivative_account([1; 32], 3);
        let new_derived = substitution_semantics::derivative_account([2; 32], 3);
        let key = |owner: [u8; 32]| {
            [
                schema.descriptors[0].prefix.clone(),
                Hasher::Blake2_128Concat.apply(&owner),
            ]
            .concat()
        };
        storage.top.insert(key(derived), 10u128.encode());
        let admitted = verify(&schema, &storage, &map([1; 32], [2; 32]), &options).unwrap();
        assert_eq!(admitted.report.authorization_policy, AUTHORIZATION_POLICY);
        assert!(!admitted.report.ownership_signatures_required);
        assert_eq!(admitted.report.automatic_derivative_count, 1);
        assert_eq!(
            admitted
                .report
                .direct_rows_without_discoverable_account_state,
            vec![hx(&[1; 32])]
        );
        assert_eq!(
            admitted.report.expanded_mapping_commitment,
            commitment(admitted.replacements())
        );
        assert!(admitted.report.unused_destinations_verified);
        let compiled = substitution_semantics::compile_trusted(
            &schema,
            &storage,
            &admitted,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            compiled.report.verified_derived_accounts,
            vec![hx(&derived)]
        );
        storage.top.insert(key(new_derived), 1u128.encode());
        assert!(verify(&schema, &storage, &map([1; 32], [2; 32]), &options).is_err());
    }
    #[test]
    fn unclassified_storage_cannot_certify_destination_non_use() {
        let (schema, mut storage, options) = checkpoint_fixture();
        storage.top.insert(vec![0xff; 40], [2; 32].to_vec());
        assert!(verify(&schema, &storage, &map([1; 32], [2; 32]), &options).is_err());
        let (schema, storage, mut options) = checkpoint_fixture();
        options
            .additional_storage
            .push(crate::inventory::AdditionalStorage {
                trie: "top".into(),
                name: "Caller.Invented".into(),
                prefix: hx(&[9; 32]),
                key_types: vec![],
                hashers: vec![],
                value_type: schema.descriptors[0].value_type,
            });
        assert!(verify(&schema, &storage, &map([1; 32], [2; 32]), &options).is_err());
    }
}
