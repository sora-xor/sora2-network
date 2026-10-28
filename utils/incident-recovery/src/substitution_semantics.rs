//! Reviewed control-state adapters. These supplement typed ownership mapping;
//! they never reinterpret historical calls or release reserved funds implicitly.

use crate::{
    account, hex_bytes,
    inventory::{AdapterId, AdapterResolution, Descriptor, MetadataSchema, ResolutionAction},
    typed, OwnershipProof, Result,
};
use codec::{Decode, Encode};
use common::recovery::{AccountReplacement, GuardedStorageEdit, StorageScope};
use scale_info::TypeDef;
use scale_value::{Composite, Value, ValueDef};
use serde::{Deserialize, Serialize};
use sp_core::storage::Storage;
use std::collections::{BTreeMap, BTreeSet};

#[path = "bridge_membership.rs"]
mod bridge_membership;
pub use bridge_membership::Review as BridgeMembershipReview;

fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn account_bytes(text: &str) -> Result<[u8; 32]> {
    Ok(*AsRef::<[u8; 32]>::as_ref(&account(text)?))
}
fn commitment(text: &str) -> Result<[u8; 32]> {
    let hash: [u8; 32] = hex_bytes(text)?
        .try_into()
        .map_err(|_| "expected 32-byte commitment")?;
    if hash == [0; 32] {
        return Err("missing evidence commitment".into());
    }
    Ok(hash)
}
fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing metadata descriptor {name}"))
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    DefaultStakingPayee,
    SuspendReferral,
    SuspendProxy,
    CancelProxyAnnouncements,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ControlRow {
    pub storage: String,
    pub key: String,
    pub expected_value_hash: String,
    pub action: ControlAction,
    pub review_evidence_hash: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PreservationWitness {
    pub key: String,
    pub value_hash: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExactCancellationRow {
    pub storage: String,
    pub key: String,
    pub value_hash: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BridgeCancellation {
    pub call_hash: String,
    pub recovered_call_preimage: String,
    /// None explicitly requires the hash-only operation to have no Calls row.
    pub calls_value_hash: Option<String>,
    pub operations: Vec<ExactCancellationRow>,
    /// Underlying request/status/replay records which must remain unchanged.
    pub preserve: Vec<PreservationWitness>,
    pub review_evidence_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DerivedKind {
    Utility {
        controller: String,
        index: u16,
    },
    Multisig {
        signatories: Vec<String>,
        threshold: u16,
    },
    BridgeMultisig {
        creator: String,
        block: u32,
        nonce: u32,
        signatories: Vec<String>,
        accounts_value_hash: String,
    },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DerivedAccount {
    pub old: String,
    pub new: String,
    pub derivation: DerivedKind,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SemanticPlan {
    #[serde(default)]
    pub bridge_memberships: Vec<BridgeMembershipReview>,
    #[serde(default)]
    pub controls: Vec<ControlRow>,
    #[serde(default)]
    pub bridge_cancellations: Vec<BridgeCancellation>,
    #[serde(default)]
    pub derived_accounts: Vec<DerivedAccount>,
    #[serde(default)]
    pub governance_preservation: Option<crate::release_checks::GovernancePreservation>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticReport {
    pub schema: &'static str,
    pub metadata_hash: String,
    pub mutation_count: usize,
    pub verified_derived_accounts: Vec<String>,
    pub proof_only_derived_accounts: Vec<String>,
    pub preserved_records: Vec<String>,
    pub suspended_evidence: Vec<serde_json::Value>,
    pub blockers: Vec<String>,
    pub edits_scale: String,
    pub production_ready: bool,
    pub resolved_records: Vec<AdapterResolution>,
    pub atomic_group_edit_counts: Vec<usize>,
    pub preservation_evidence: Vec<serde_json::Value>,
    pub remaining: &'static str,
}
pub struct SemanticCompilation {
    pub edits: Vec<GuardedStorageEdit>,
    pub report: SemanticReport,
    pub resolutions: Vec<AdapterResolution>,
    pub groups: Vec<Vec<GuardedStorageEdit>>,
    pub preservation_witnesses: Vec<common::recovery::StorageWitness>,
    pub prefix_witnesses: Vec<common::recovery::StoragePrefixWitness>,
    membership_guard: Option<bridge_membership::Guard>,
}
impl SemanticCompilation {
    pub fn verify_membership_groups(&self, groups: &[Vec<GuardedStorageEdit>]) -> Result<()> {
        if let Some(guard) = &self.membership_guard {
            guard.verify(groups)?;
        }
        Ok(())
    }
}

fn call_record_owner(
    value: Option<&Vec<u8>>,
    expected: Option<&str>,
    call: &[u8],
) -> Result<Option<[u8; 32]>> {
    let Some(value) = value else {
        return if expected.is_none() {
            Ok(None)
        } else {
            Err("expected stored call is absent".into())
        };
    };
    let expected =
        expected.ok_or("hash-only Calls absence witness conflicts with stored payload")?;
    if digest(value) != commitment(expected)? || value.len() > typed::MAX_TYPED_BYTES {
        return Err("exact call record mismatch/oversized".into());
    }
    let mut input = value.as_slice();
    let length = codec::Compact::<u32>::decode(&mut input).map_err(|e| e.to_string())?;
    if value.len() - input.len() != length.encode().len()
        || !value.starts_with(&length.encode())
        || length.0 as usize != call.len()
        || input.len() != call.len() + 48
        || !input.starts_with(call)
    {
        return Err("recovered preimage does not match opaque historical Calls layout".into());
    }
    Ok(Some(
        input[call.len()..call.len() + 32]
            .try_into()
            .map_err(|_| "opaque call depositor width")?,
    ))
}

fn dependency_groups(
    edits: &BTreeMap<Vec<u8>, GuardedStorageEdit>,
    dependencies: &[Vec<Vec<u8>>],
) -> Result<Vec<Vec<GuardedStorageEdit>>> {
    let mut adjacency: BTreeMap<Vec<u8>, BTreeSet<Vec<u8>>> = edits
        .keys()
        .cloned()
        .map(|k| (k, BTreeSet::new()))
        .collect();
    for keys in dependencies {
        let first = keys.first().ok_or("empty semantic dependency group")?;
        for key in keys {
            if !edits.contains_key(key) || !edits.contains_key(first) {
                return Err("semantic dependency references a missing edit".into());
            }
            adjacency
                .get_mut(first)
                .ok_or("dependency root absent")?
                .insert(key.clone());
            adjacency
                .get_mut(key)
                .ok_or("dependency key absent")?
                .insert(first.clone());
        }
    }
    let mut visited = BTreeSet::new();
    let mut groups = Vec::new();
    for first in edits.keys() {
        if visited.contains(first) {
            continue;
        }
        let mut stack = vec![first.clone()];
        let mut group = BTreeMap::new();
        while let Some(key) = stack.pop() {
            if !visited.insert(key.clone()) {
                continue;
            }
            group.insert(
                key.clone(),
                edits
                    .get(&key)
                    .ok_or("dependency edit disappeared")?
                    .clone(),
            );
            stack.extend(adjacency[&key].iter().cloned());
        }
        groups.push(group.into_values().collect());
    }
    Ok(groups)
}

pub fn derivative_account(controller: [u8; 32], index: u16) -> [u8; 32] {
    digest(&(b"modlpy/utilisuba", controller, index).encode())
}
pub fn multisig_account(signatories: &[[u8; 32]], threshold: u16) -> Result<[u8; 32]> {
    if !(2..=100).contains(&signatories.len())
        || threshold == 0
        || usize::from(threshold) > signatories.len()
        || signatories.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err("multisig controllers must be sorted/unique with valid threshold".into());
    }
    Ok(digest(
        &(b"modlpy/utilisuba", signatories, threshold).encode(),
    ))
}
pub fn bridge_multisig_account(creator: [u8; 32], block: u32, nonce: u32) -> [u8; 32] {
    digest(&(b"modlpy/utilisuba", creator, block, nonce).encode())
}

type PendingCallIndex = BTreeMap<[u8; 32], BTreeSet<Vec<u8>>>;

/// Index only the exact storage prefix once. Scanning the complete checkpoint
/// per cancellation is both unnecessary and prohibitively expensive.
fn pending_call_index(
    schema: &MetadataSchema,
    storage: &Storage,
    descriptor: &Descriptor,
) -> Result<PendingCallIndex> {
    if descriptor.hashers
        != [
            crate::inventory::Hasher::Twox64Concat,
            crate::inventory::Hasher::Blake2_128Concat,
        ]
        || descriptor.key_types.len() != 2
        || !typed::account_type(&schema.registry, descriptor.key_types[0])
    {
        return Err("pending call index has an unreviewed key codec".into());
    }
    let mut index = PendingCallIndex::new();
    for (key, _) in storage
        .top
        .range(descriptor.prefix.clone()..)
        .take_while(|(key, _)| key.starts_with(&descriptor.prefix))
    {
        // Validate every retained key, including owners omitted by a candidate.
        crate::inventory::transform_key(key, descriptor, &schema.registry, &BTreeMap::new(), None)?;
        if key.len() != descriptor.prefix.len() + 8 + 32 + 16 + 32 {
            return Err("pending call index key width differs".into());
        }
        let hash = key[key.len() - 32..]
            .try_into()
            .map_err(|_| "pending call hash width")?;
        index.entry(hash).or_default().insert(key.clone());
    }
    Ok(index)
}

fn prove_controller(
    old: [u8; 32],
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
    proofs: &[OwnershipProof],
) -> Result<[u8; 32]> {
    let new = mappings.get(&old).copied().unwrap_or(old);
    if old == common::recovery::ATTACKER_ACCOUNT || new == common::recovery::ATTACKER_ACCOUNT {
        return Err("fixed attacker cannot remain a recovery controller; shared ownership requires separate review".into());
    }
    let matching = proofs
        .iter()
        .filter(|p| account_bytes(&p.old).ok() == Some(old))
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err("controlling key requires exactly one checkpoint-bound ownership proof".into());
    }
    crate::verify_ownership_proofs(
        &[AccountReplacement {
            old: old.into(),
            new: new.into(),
        }],
        std::slice::from_ref(matching[0]),
    )?;
    Ok(new)
}

fn owned_key(descriptor: &Descriptor, owner: [u8; 32], schema: &MetadataSchema) -> Result<Vec<u8>> {
    if descriptor.key_types.len() != 1
        || descriptor.hashers.len() != 1
        || !typed::account_type(&schema.registry, descriptor.key_types[0])
    {
        return Err("expected a single AccountId storage key".into());
    }
    let mut key = descriptor.prefix.clone();
    key.extend(descriptor.hashers[0].apply(&owner));
    Ok(key)
}

/// Bound the proof graph independently of dispatch nesting. A derived account
/// can control another derived account without having a private signing key.
const MAX_DERIVED_CONTROL_DEPTH: usize = 32;

struct VerifiedDerivedControls {
    controls: BTreeMap<[u8; 32], [u8; 32]>,
    reported: Vec<String>,
    proof_only: Vec<String>,
}
impl VerifiedDerivedControls {
    fn controller(
        &self,
        old: [u8; 32],
        mappings: &BTreeMap<[u8; 32], [u8; 32]>,
        proofs: &[OwnershipProof],
    ) -> Result<[u8; 32]> {
        if let Some(new) = self.controls.get(&old) {
            if mappings.get(&old).copied().unwrap_or(old) != *new {
                return Err("derived control certificate mapping differs".into());
            }
            Ok(*new)
        } else {
            prove_controller(old, mappings, proofs)
        }
    }
}

struct DerivedControlVerifier<'a> {
    schema: &'a MetadataSchema,
    storage: &'a Storage,
    mappings: &'a BTreeMap<[u8; 32], [u8; 32]>,
    proofs: &'a [OwnershipProof],
    entries: BTreeMap<[u8; 32], &'a DerivedAccount>,
    visiting: BTreeSet<[u8; 32]>,
    // Include the longest proof-path length in memoized nodes. Reusing a node
    // through a deeper parent cannot bypass the depth bound.
    verified: BTreeMap<[u8; 32], ([u8; 32], usize)>,
}
impl DerivedControlVerifier<'_> {
    fn controller(&mut self, old: [u8; 32], depth: usize) -> Result<([u8; 32], usize)> {
        if self.entries.contains_key(&old) {
            self.verify(old, depth)
        } else {
            if depth > MAX_DERIVED_CONTROL_DEPTH {
                return Err("derived control proof exceeds depth bound".into());
            }
            Ok((prove_controller(old, self.mappings, self.proofs)?, 0))
        }
    }

    fn verify(&mut self, old: [u8; 32], depth: usize) -> Result<([u8; 32], usize)> {
        if depth > MAX_DERIVED_CONTROL_DEPTH {
            return Err("derived control proof exceeds depth bound".into());
        }
        if let Some((new, height)) = self.verified.get(&old) {
            if depth + height > MAX_DERIVED_CONTROL_DEPTH {
                return Err("derived control proof exceeds depth bound".into());
            }
            return Ok((*new, *height));
        }
        if !self.visiting.insert(old) {
            return Err("derived control inventory contains a cycle".into());
        }
        let item = self
            .entries
            .get(&old)
            .copied()
            .ok_or("derived control entry missing")?;
        let new = account_bytes(&item.new)?;
        let mut height = 1;
        let (expected_old, expected_new) = match &item.derivation {
            DerivedKind::Utility { controller, index } => {
                let old_controller = account_bytes(controller)?;
                let (new_controller, parent_height) = self.controller(old_controller, depth + 1)?;
                height += parent_height;
                (
                    derivative_account(old_controller, *index),
                    derivative_account(new_controller, *index),
                )
            }
            DerivedKind::Multisig {
                signatories,
                threshold,
            } => {
                let old_signers = signatories
                    .iter()
                    .map(|s| account_bytes(s))
                    .collect::<Result<Vec<_>>>()?;
                let expected_old = multisig_account(&old_signers, *threshold)?;
                let mut new_signers = Vec::with_capacity(old_signers.len());
                for signer in &old_signers {
                    let (new_signer, parent_height) = self.controller(*signer, depth + 1)?;
                    height = height.max(1 + parent_height);
                    new_signers.push(new_signer);
                }
                new_signers.sort();
                (expected_old, multisig_account(&new_signers, *threshold)?)
            }
            DerivedKind::BridgeMultisig {
                creator,
                block,
                nonce,
                signatories,
                accounts_value_hash,
            } => {
                let creator = account_bytes(creator)?;
                let (new_creator, creator_height) = self.controller(creator, depth + 1)?;
                height = height.max(1 + creator_height);
                let signers = signatories
                    .iter()
                    .map(|s| account_bytes(s))
                    .collect::<Result<BTreeSet<_>>>()?;
                if signers.len() != signatories.len() || signers.is_empty() || signers.len() > 100 {
                    return Err("invalid bridge multisig controller inventory".into());
                }
                for signer in &signers {
                    let (_, signer_height) = self.controller(*signer, depth + 1)?;
                    height = height.max(1 + signer_height);
                }
                let descriptor = descriptor(self.schema, "BridgeMultisig.Accounts")?;
                let key = owned_key(descriptor, old, self.schema)?;
                let value = self
                    .storage
                    .top
                    .get(&key)
                    .ok_or("bridge multisig membership record missing")?;
                if digest(value) != commitment(accounts_value_hash)? {
                    return Err("bridge multisig membership preimage mismatch".into());
                }
                let (_, stored_signers) = typed::transform_value(
                    value,
                    descriptor.value_type,
                    &self.schema.registry,
                    &BTreeMap::new(),
                )?;
                if stored_signers != signers {
                    return Err("declared controllers do not match checkpoint membership".into());
                }
                (
                    bridge_multisig_account(creator, *block, *nonce),
                    bridge_multisig_account(new_creator, *block, *nonce),
                )
            }
        };
        if old != expected_old || new != expected_new {
            return Err("derived address does not match controlling account derivation".into());
        }
        self.visiting.remove(&old);
        self.verified.insert(old, (new, height));
        Ok((new, height))
    }
}

fn verify_derived(
    schema: &MetadataSchema,
    storage: &Storage,
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
    proofs: &[OwnershipProof],
    derived: &[DerivedAccount],
) -> Result<VerifiedDerivedControls> {
    if derived.len() > common::recovery::MAX_REPLACEMENTS {
        return Err("derived control inventory exceeds node bound".into());
    }
    let mut entries = BTreeMap::new();
    let mut reported = Vec::new();
    let mut proof_only = Vec::new();
    for item in derived {
        let old = account_bytes(&item.old)?;
        let new = account_bytes(&item.new)?;
        if old == common::recovery::ATTACKER_ACCOUNT || new == common::recovery::ATTACKER_ACCOUNT {
            return Err("fixed attacker cannot be a derived recovery controller or proof-only node; shared ownership requires separate review".into());
        }
        if entries.insert(old, item).is_some() {
            return Err(
                "derived inventory is duplicate or differs from substitution mapping".into(),
            );
        }
        match mappings.get(&old) {
            Some(mapped) if *mapped == new => reported.push(hx(&old)),
            None if old == new => proof_only.push(hx(&old)),
            _ => return Err("derived inventory differs from substitution mapping".into()),
        }
    }
    let mut verifier = DerivedControlVerifier {
        schema,
        storage,
        mappings,
        proofs,
        entries,
        visiting: BTreeSet::new(),
        verified: BTreeMap::new(),
    };
    // Iterate the submitted order, but resolve dependencies recursively. Neither
    // ordering nor an unverified inventory entry can stand in for a proof.
    for item in derived {
        verifier.verify(account_bytes(&item.old)?, 0)?;
    }
    Ok(VerifiedDerivedControls {
        controls: verifier
            .verified
            .into_iter()
            .map(|(old, (new, _))| (old, new))
            .collect(),
        reported,
        proof_only,
    })
}

struct VerifiedOwnership {
    mappings: BTreeMap<[u8; 32], [u8; 32]>,
    derived: VerifiedDerivedControls,
    direct_accounts: Vec<String>,
}

fn verify_account_ownership(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    proofs: &[OwnershipProof],
    plan: &SemanticPlan,
) -> Result<VerifiedOwnership> {
    common::recovery::validate_replacements(replacements)
        .map_err(|e| format!("invalid mapping: {e:?}"))?;
    let mappings: BTreeMap<[u8; 32], [u8; 32]> = replacements
        .iter()
        .map(|r| {
            (
                *AsRef::<[u8; 32]>::as_ref(&r.old),
                *AsRef::<[u8; 32]>::as_ref(&r.new),
            )
        })
        .collect();
    let derived = verify_derived(schema, storage, &mappings, proofs, &plan.derived_accounts)?;
    let mut direct_accounts = Vec::new();
    for old in mappings
        .keys()
        .filter(|old| !derived.controls.contains_key(*old))
    {
        prove_controller(*old, &mappings, proofs)?;
        direct_accounts.push(hx(old));
    }
    Ok(VerifiedOwnership {
        mappings,
        derived,
        direct_accounts,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticOwnershipVerification {
    pub schema: &'static str,
    pub checkpoint_hash: String,
    pub metadata_hash: String,
    pub mapped_accounts: usize,
    pub direct_accounts: Vec<String>,
    pub derived_accounts: Vec<String>,
    pub proof_only_derived_accounts: Vec<String>,
    pub replacement_ownership_verified: bool,
    pub derived_control_graph_verified: bool,
    pub production_ready: bool,
    pub scope: &'static str,
}

/// Verify the same direct/derived control graph used by storage composition.
/// Callers admitting a release must first authenticate `schema` and `storage`
/// with `verify_checkpoint`; arbitrary metadata is not checkpoint evidence.
/// This proves neither complete account-state coverage nor historical grant
/// safety, bridge accounting, operator approval, or readiness to activate.
pub fn verify_semantic_ownership(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    proofs: &[OwnershipProof],
    plan: &SemanticPlan,
) -> Result<SemanticOwnershipVerification> {
    let verified = verify_account_ownership(schema, storage, replacements, proofs, plan)?;
    Ok(SemanticOwnershipVerification {
        schema: "sora-nexus-semantic-ownership-verification-v1",
        checkpoint_hash: hx(&common::recovery::CHECKPOINT_HASH),
        metadata_hash: schema.metadata_hash.clone(),
        mapped_accounts: replacements.len(),
        direct_accounts: verified.direct_accounts,
        derived_accounts: verified.derived.reported,
        proof_only_derived_accounts: verified.derived.proof_only,
        replacement_ownership_verified: true,
        derived_control_graph_verified: true,
        production_ready: false,
        scope: "Every CSV pair is verified by a replacement-key signature or exact bounded derived-control proof graph. Only used controlling-key proofs are asserted verified; unrelated extra proof records are not authorizations. Complete storage/control-history/accounting and exact-artifact release gates remain separate.",
    })
}

fn default_payee(
    before: &[u8],
    descriptor: &Descriptor,
    schema: &MetadataSchema,
    new: [u8; 32],
) -> Result<Vec<u8>> {
    typed::decode_exact(before, descriptor.value_type, &schema.registry)?;
    let Some(TypeDef::Variant(variants)) = schema
        .registry
        .resolve(descriptor.value_type)
        .map(|t| &t.type_def)
    else {
        return Err("staking payee is not the reviewed enum".into());
    };
    let account = variants
        .variants
        .iter()
        .find(|v| v.name == "Account")
        .ok_or("staking destination Account variant missing")?;
    if account.fields.len() != 1 || !typed::account_type(&schema.registry, account.fields[0].ty.id)
    {
        return Err("staking payout account has unexpected encoding".into());
    }
    let mut after = vec![account.index];
    after.extend_from_slice(&new);
    typed::decode_exact(&after, descriptor.value_type, &schema.registry)?;
    Ok(after)
}

fn sequence_mut<'a>(
    value: &'a mut Value<u32>,
    schema: &MetadataSchema,
    depth: usize,
) -> Result<&'a mut Vec<Value<u32>>> {
    if depth > 16 {
        return Err("control sequence nesting exceeds reviewed shape".into());
    }
    match &schema
        .registry
        .resolve(value.context)
        .ok_or("control type missing")?
        .type_def
    {
        TypeDef::Sequence(_) => match &mut value.value {
            ValueDef::Composite(Composite::Unnamed(items)) => Ok(items),
            _ => Err("sequence value shape differs".into()),
        },
        TypeDef::Composite(c) if c.fields.len() == 1 => match &mut value.value {
            ValueDef::Composite(Composite::Unnamed(fields)) if fields.len() == 1 => {
                sequence_mut(&mut fields[0], schema, depth + 1)
            }
            _ => Err("bounded sequence wrapper shape differs".into()),
        },
        _ => Err("control field is not the reviewed sequence".into()),
    }
}

fn suspend_proxy(
    before: &[u8],
    descriptor: &Descriptor,
    schema: &MetadataSchema,
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
    owner_replaced: bool,
) -> Result<Vec<u8>> {
    let mut value = typed::decode_exact(before, descriptor.value_type, &schema.registry)?;
    let ValueDef::Composite(Composite::Unnamed(fields)) = &mut value.value else {
        return Err("proxy row must be the reviewed (grants, deposit) tuple".into());
    };
    if fields.len() != 2 {
        return Err("proxy row has unexpected fields".into());
    }
    let deposit = typed::encode(&fields[1], &schema.registry)?;
    let sequence = sequence_mut(&mut fields[0], schema, 0)?;
    let mut retained = Vec::new();
    for item in sequence.drain(..) {
        let encoded = typed::encode(&item, &schema.registry)?;
        let (_, accounts) =
            typed::transform_value(&encoded, item.context, &schema.registry, &BTreeMap::new())?;
        if !owner_replaced && !accounts.iter().any(|a| mappings.contains_key(a)) {
            retained.push(item);
        }
    }
    *sequence = retained;
    if typed::encode(&fields[1], &schema.registry)? != deposit {
        return Err("proxy suspension changed its reserved deposit".into());
    }
    typed::encode(&value, &schema.registry)
}

/// Mapping proofs are mandatory; derived entries use controlling-key proofs.
/// All listed rows bind exact old values and explicit review evidence.
pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    replacements: &[AccountReplacement],
    proofs: &[OwnershipProof],
    plan: &SemanticPlan,
) -> Result<SemanticCompilation> {
    let ownership = verify_account_ownership(schema, storage, replacements, proofs, plan)?;
    let mappings = ownership.mappings;
    let derived = ownership.derived;
    let mut edits: BTreeMap<Vec<u8>, GuardedStorageEdit> = BTreeMap::new();
    let mut suspended = Vec::new();
    let mut blockers = Vec::new();
    let mut preserved = BTreeMap::new();
    let mut preserved_absent = BTreeSet::new();
    let mut resolved_drafts = Vec::new();
    let mut dependencies = Vec::new();
    let mut pending_indexes = BTreeMap::new();
    if !plan.bridge_cancellations.is_empty() {
        for name in [
            "BridgeMultisig.Multisigs",
            "BridgeMultisig.CancellationApprovals",
            "BridgeMultisig.CountedNewOperations",
            "BridgeMultisig.ProposerCountedOperations",
        ] {
            if let Ok(d) = descriptor(schema, name) {
                pending_indexes.insert(name, pending_call_index(schema, storage, d)?);
            }
        }
    }
    let mut put = |key: Vec<u8>, before: Option<&[u8]>, after: Option<Vec<u8>>| -> Result<()> {
        if edits
            .insert(
                key.clone(),
                GuardedStorageEdit {
                    scope: StorageScope::Top,
                    key,
                    before: before.map(digest),
                    after,
                },
            )
            .is_some()
        {
            return Err("semantic operations collide on a storage key".into());
        }
        Ok(())
    };
    for row in &plan.controls {
        commitment(&row.review_evidence_hash)?;
        let descriptor = descriptor(schema, &row.storage)?;
        let key = hex_bytes(&row.key)?;
        let before = storage
            .top
            .get(&key)
            .ok_or("reviewed control row missing")?;
        if digest(before) != commitment(&row.expected_value_hash)? {
            return Err("reviewed control value changed".into());
        }
        let (new_key, key_accounts) =
            crate::inventory::transform_key(&key, descriptor, &schema.registry, &mappings, None)?;
        let mapped_owners: Vec<_> = key_accounts
            .iter()
            .filter(|a| mappings.contains_key(*a))
            .collect();
        let after = match row.action {
            ControlAction::DefaultStakingPayee => {
                if row.storage != "Staking.Payee"
                    || mapped_owners.len() != 1
                    || key_accounts.len() != 1
                {
                    return Err("staking payout reset requires the mapped stash's Payee row".into());
                }
                Some(default_payee(
                    before,
                    descriptor,
                    schema,
                    mappings[mapped_owners[0]],
                )?)
            }
            ControlAction::SuspendReferral => {
                if row.storage != "Referrals.Referrers"
                    || mapped_owners.len() != 1
                    || key_accounts.len() != 1
                    || !typed::account_type(&schema.registry, descriptor.value_type)
                {
                    return Err(
                        "referral suspension requires the mapped account's typed referral pointer"
                            .into(),
                    );
                }
                typed::decode_exact(before, descriptor.value_type, &schema.registry)?;
                blockers.push(format!("{} {}: referral suspension requires the paired Referrals reverse index and both accounts' provider/consumer reference accounting",row.storage,row.key));
                None
            }
            ControlAction::SuspendProxy | ControlAction::CancelProxyAnnouncements => {
                let required = if matches!(row.action, ControlAction::SuspendProxy) {
                    "Proxy.Proxies"
                } else {
                    "Proxy.Announcements"
                };
                if row.storage != required {
                    return Err("proxy control action targets the wrong pallet row".into());
                }
                let after = suspend_proxy(
                    before,
                    descriptor,
                    schema,
                    &mappings,
                    !mapped_owners.is_empty(),
                )?;
                blockers.push(format!(
                    "{} {}: retained reserved deposit requires reviewed custody/reclaim accounting",
                    row.storage, row.key
                ));
                Some(after)
            }
        };
        if new_key != key && storage.top.contains_key(&new_key) {
            return Err("replacement control key already occupied".into());
        }
        let mut group_keys = vec![key.clone()];
        if new_key != key && after.is_some() {
            group_keys.push(new_key.clone());
        }
        dependencies.push(group_keys);
        if matches!(row.action, ControlAction::DefaultStakingPayee) {
            let (_, mut observed) = typed::transform_value(
                before,
                descriptor.value_type,
                &schema.registry,
                &BTreeMap::new(),
            )?;
            observed.extend(key_accounts);
            let mut required = vec![key.clone()];
            if new_key != key && after.is_some() {
                required.push(new_key.clone());
            }
            resolved_drafts.push((
                row.storage.clone(),
                key.clone(),
                AdapterId::StakingPayeeV1,
                ResolutionAction::ReplaceControl,
                observed,
                required,
            ));
        }
        if new_key != key {
            put(key.clone(), Some(before), None)?;
            if after.is_some() {
                put(new_key, None, after)?;
            }
        } else {
            put(key.clone(), Some(before), after)?;
        }
        suspended.push(serde_json::json!({"storage":row.storage,"oldKey":row.key,"oldValueHash":row.expected_value_hash,"evidenceHash":row.review_evidence_hash,"originalValue":hx(before)}));
    }
    for cancellation in &plan.bridge_cancellations {
        let mut cancellation_dependencies = Vec::new();
        commitment(&cancellation.review_evidence_hash)?;
        let call_hash = commitment(&cancellation.call_hash)?;
        let call = hex_bytes(&cancellation.recovered_call_preimage)?;
        if call.len() > typed::MAX_TYPED_BYTES || digest(&call) != call_hash {
            return Err("recovered opaque call preimage hash/size mismatch".into());
        }
        let calls = descriptor(schema, "BridgeMultisig.Calls")?;
        if calls.hashers.len() != 1 || calls.hashers[0] != crate::inventory::Hasher::Identity {
            return Err("BridgeMultisig call index layout changed".into());
        }
        let mut calls_key = calls.prefix.clone();
        calls_key.extend_from_slice(&call_hash);
        let calls_value = storage.top.get(&calls_key);
        let call_owner =
            call_record_owner(calls_value, cancellation.calls_value_hash.as_deref(), &call)?;
        if calls_value.is_none() {
            preserved_absent.insert(calls_key.clone());
        }
        let multisigs = descriptor(schema, "BridgeMultisig.Multisigs")?;
        let matching = pending_indexes
            .get("BridgeMultisig.Multisigs")
            .and_then(|index| index.get(&call_hash))
            .cloned()
            .unwrap_or_default();
        let declared: BTreeSet<_> = cancellation
            .operations
            .iter()
            .filter(|row| row.storage == "BridgeMultisig.Multisigs")
            .map(|row| hex_bytes(&row.key))
            .collect::<Result<_>>()?;
        if matching.is_empty() || matching != declared {
            return Err(
                "cancellation must enumerate every pending owner of the opaque call".into(),
            );
        }
        if let Some(index) = pending_indexes.get("BridgeMultisig.CancellationApprovals") {
            let existing = index.get(&call_hash).cloned().unwrap_or_default();
            let declared: BTreeSet<_> = cancellation
                .operations
                .iter()
                .filter(|row| row.storage == "BridgeMultisig.CancellationApprovals")
                .map(|row| hex_bytes(&row.key))
                .collect::<Result<_>>()?;
            if existing != declared {
                return Err("cancellation approvals must be completely enumerated".into());
            }
        }
        if cancellation.preserve.is_empty() {
            return Err(
                "bridge cancellation requires underlying request/replay preservation evidence"
                    .into(),
            );
        }
        for witness in &cancellation.preserve {
            let key = hex_bytes(&witness.key)?;
            let expected = witness.value_hash.as_deref().map(commitment).transpose()?;
            if storage.top.get(&key).map(|value| digest(value)) != expected {
                return Err(
                    "underlying preservation witness missing, conflicting or duplicated".into(),
                );
            }
            if let Some(expected) = expected {
                if preserved_absent.contains(&key)
                    || preserved
                        .insert(key, expected)
                        .is_some_and(|previous| previous != expected)
                {
                    return Err("conflicting underlying preservation witnesses".into());
                }
            } else {
                if preserved.contains_key(&key) {
                    return Err("conflicting underlying preservation witnesses".into());
                }
                preserved_absent.insert(key);
            }
        }
        for operation in &cancellation.operations {
            if ![
                "BridgeMultisig.Multisigs",
                "BridgeMultisig.CancellationApprovals",
            ]
            .contains(&operation.storage.as_str())
            {
                return Err(
                    "unsupported pending bookkeeping needs an exact-version cancellation adapter"
                        .into(),
                );
            }
            let descriptor = descriptor(schema, &operation.storage)?;
            let key = hex_bytes(&operation.key)?;
            let (_, mut observed) = crate::inventory::transform_key(
                &key,
                descriptor,
                &schema.registry,
                &BTreeMap::new(),
                None,
            )?;
            if !key.ends_with(&call_hash) {
                return Err("bookkeeping key refers to another call".into());
            }
            let before = storage
                .top
                .get(&key)
                .ok_or("cancelled bookkeeping row missing")?;
            if digest(before) != commitment(&operation.value_hash)? {
                return Err("cancelled bookkeeping preimage mismatch".into());
            }
            let (_, value_accounts) = typed::transform_value(
                before,
                descriptor.value_type,
                &schema.registry,
                &BTreeMap::new(),
            )?;
            observed.extend(value_accounts);
            resolved_drafts.push((
                operation.storage.clone(),
                key.clone(),
                AdapterId::BridgeMultisigCancellationV1,
                ResolutionAction::CancelPending,
                observed,
                vec![key.clone()],
            ));
            cancellation_dependencies.push(key.clone());
            put(key, Some(before), None)?;
        }
        // Newer admission counters need decrements by the exact pallet adapter;
        // never leave them silently inconsistent if such markers already exist.
        for name in [
            "BridgeMultisig.CountedNewOperations",
            "BridgeMultisig.ProposerCountedOperations",
        ] {
            if let Ok(marker) = descriptor(schema, name) {
                if marker.hashers != multisigs.hashers || marker.key_types != multisigs.key_types {
                    return Err("pending admission marker key codec differs".into());
                }
                if pending_indexes
                    .get(name)
                    .is_some_and(|index| index.contains_key(&call_hash))
                {
                    return Err(
                        "counted pending operation requires the admission-counter adapter".into(),
                    );
                }
                for key in &matching {
                    let mut marker_key = marker.prefix.clone();
                    marker_key.extend_from_slice(&key[multisigs.prefix.len()..]);
                    preserved_absent.insert(marker_key);
                }
            }
        }
        if let (Some(calls_value), Some(depositor)) = (calls_value, call_owner) {
            resolved_drafts.push((
                "BridgeMultisig.Calls".into(),
                calls_key.clone(),
                AdapterId::BridgeMultisigCancellationV1,
                ResolutionAction::CancelPending,
                BTreeSet::from([depositor]),
                vec![calls_key.clone()],
            ));
            cancellation_dependencies.push(calls_key.clone());
            put(calls_key, Some(calls_value), None)?;
        }
        dependencies.push(cancellation_dependencies);
        suspended.push(serde_json::json!({"opaqueCallHash":cancellation.call_hash,"callPreimage":cancellation.recovered_call_preimage,"callsValueHash":cancellation.calls_value_hash,"refundGenerated":false}));
    }
    drop(put);
    let membership = bridge_membership::compile(schema, storage, &mappings, proofs, plan, &edits)?;
    let mut prefix_witnesses = Vec::new();
    let mut membership_resolutions = Vec::new();
    let mut membership_guard = None;
    if let Some(compiled) = membership {
        for group in compiled.groups {
            let keys = group.iter().map(|e| e.key.clone()).collect();
            for edit in group {
                if edits.insert(edit.key.clone(), edit).is_some() {
                    return Err("bridge membership collides with another semantic edit".into());
                }
            }
            dependencies.push(keys);
        }
        prefix_witnesses = compiled.prefixes;
        membership_resolutions = compiled.resolutions;
        membership_guard = Some(compiled.guard);
    }
    if preserved
        .keys()
        .chain(preserved_absent.iter())
        .any(|key| edits.contains_key(key))
    {
        return Err("cancellation would mutate an underlying request/replay record".into());
    }
    let mut resolutions = resolved_drafts
        .into_iter()
        .map(
            |(storage_name, key, adapter, action, observed_accounts, required)| {
                let validated_edits = required
                    .iter()
                    .map(|key| {
                        edits
                            .get(key)
                            .cloned()
                            .ok_or("resolved source/destination edit is missing".to_string())
                    })
                    .collect::<Result<Vec<_>>>()?;
                let source = edits.get(&key).ok_or("resolved source edit is missing")?;
                Ok(AdapterResolution {
                    storage: storage_name,
                    key: hx(&key),
                    expected_before_hash: hx(&source.before.ok_or("resolved source must exist")?),
                    expected_after_hash: source.after.as_deref().map(|v| hx(&digest(v))),
                    adapter,
                    action,
                    validated_edits,
                    observed_accounts,
                })
            },
        )
        .collect::<Result<Vec<_>>>()?;
    resolutions.extend(membership_resolutions);
    let groups = dependency_groups(&edits, &dependencies)?;
    if let Some(guard) = &membership_guard {
        guard.verify(&groups)?;
    }
    let preservation_witnesses: Vec<_> = preserved
        .iter()
        .map(|(key, hash)| common::recovery::StorageWitness {
            scope: StorageScope::Top,
            key: key.clone(),
            value_hash: Some(*hash),
        })
        .chain(
            preserved_absent
                .iter()
                .map(|key| common::recovery::StorageWitness {
                    scope: StorageScope::Top,
                    key: key.clone(),
                    value_hash: None,
                }),
        )
        .collect();
    let edits: Vec<_> = edits.into_values().collect();
    let report = SemanticReport {
        schema: "sora-nexus-control-candidates-v1",
        metadata_hash: schema.metadata_hash.clone(),
        mutation_count: edits.len(),
        verified_derived_accounts: derived.reported,
        proof_only_derived_accounts: derived.proof_only,
        preserved_records: preserved.keys().map(|key| hx(key)).collect(),
        suspended_evidence: suspended,
        blockers,
        edits_scale: hx(&edits.encode()),
        production_ready: false,
        resolved_records:resolutions.clone(),
        atomic_group_edit_counts:groups.iter().map(Vec::len).collect(),
        preservation_evidence:preservation_witnesses.iter().map(|w|serde_json::json!({"key":hx(&w.key),"valueHash":w.value_hash.map(|h|hx(&h))})).collect(),
        remaining:"Complete prefix coverage, retired-key admission, all derived accounts and exact-artifact rehearsals remain separate release gates",
    };
    Ok(SemanticCompilation {
        edits,
        report,
        resolutions,
        groups,
        preservation_witnesses,
        prefix_witnesses,
        membership_guard,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};
    use sp_core::crypto::AccountId32;
    fn control_proof(old: [u8; 32], key: &sp_core::ed25519::Pair) -> OwnershipProof {
        use sp_core::Pair;
        let pair = AccountReplacement {
            old: old.into(),
            new: key.public().0.into(),
        };
        OwnershipProof {
            old: hx(&old),
            new: hx(&key.public().0),
            signature: hx(&sp_runtime::MultiSignature::from(
                key.sign(&common::recovery::replacement_ownership_message(&pair)),
            )
            .encode()),
        }
    }

    fn empty_schema() -> MetadataSchema {
        MetadataSchema {
            registry: Registry::new().into(),
            descriptors: vec![],
            metadata_hash: hx(&[1; 32]),
        }
    }

    fn utility_chain(
        length: usize,
        old: [u8; 32],
        new: [u8; 32],
    ) -> (BTreeMap<[u8; 32], [u8; 32]>, Vec<DerivedAccount>) {
        let mut mapping = BTreeMap::from([(old, new)]);
        let mut nodes = Vec::new();
        let (mut old, mut new) = (old, new);
        for _ in 0..length {
            let next_old = derivative_account(old, 7);
            let next_new = derivative_account(new, 7);
            nodes.push(DerivedAccount {
                old: hx(&next_old),
                new: hx(&next_new),
                derivation: DerivedKind::Utility {
                    controller: hx(&old),
                    index: 7,
                },
            });
            mapping.insert(next_old, next_new);
            (old, new) = (next_old, next_new);
        }
        (mapping, nodes)
    }

    #[test]
    fn nested_derived_controls_accept_multisig_then_utility_without_fictitious_signatures() {
        use sp_core::Pair;
        let changed = sp_core::ed25519::Pair::from_seed(&[91; 32]);
        let unchanged = sp_core::ed25519::Pair::from_seed(&[92; 32]);
        let old_leaf = [1; 32];
        let mut old_signers = vec![old_leaf, unchanged.public().0];
        let mut new_signers = vec![changed.public().0, unchanged.public().0];
        old_signers.sort();
        new_signers.sort();
        let old_multisig = multisig_account(&old_signers, 2).unwrap();
        let new_multisig = multisig_account(&new_signers, 2).unwrap();
        let old_utility = derivative_account(old_multisig, 7);
        let new_utility = derivative_account(new_multisig, 7);
        let plan = SemanticPlan {
            derived_accounts: vec![
                DerivedAccount {
                    old: hx(&old_utility),
                    new: hx(&new_utility),
                    derivation: DerivedKind::Utility {
                        controller: hx(&old_multisig),
                        index: 7,
                    },
                },
                DerivedAccount {
                    old: hx(&old_multisig),
                    new: hx(&new_multisig),
                    derivation: DerivedKind::Multisig {
                        signatories: old_signers.iter().map(|s| hx(s)).collect(),
                        threshold: 2,
                    },
                },
            ],
            ..Default::default()
        };
        let mapping = BTreeMap::from([
            (old_leaf, changed.public().0),
            (old_multisig, new_multisig),
            (old_utility, new_utility),
        ]);
        let replacements: Vec<_> = mapping
            .iter()
            .map(|(old, new)| AccountReplacement {
                old: (*old).into(),
                new: (*new).into(),
            })
            .collect();
        let proofs = vec![
            control_proof(old_leaf, &changed),
            control_proof(unchanged.public().0, &unchanged),
        ];
        let compiled = compile(
            &empty_schema(),
            &Storage::default(),
            &replacements,
            &proofs,
            &plan,
        )
        .unwrap();
        assert_eq!(
            compiled.report.verified_derived_accounts,
            vec![hx(&old_utility), hx(&old_multisig)]
        );
        assert!(compiled.edits.is_empty());
        let proof_report = verify_semantic_ownership(
            &empty_schema(),
            &Storage::default(),
            &replacements,
            &proofs,
            &plan,
        )
        .unwrap();
        assert_eq!(proof_report.mapped_accounts, 3);
        assert_eq!(proof_report.direct_accounts, vec![hx(&old_leaf)]);
        assert_eq!(proof_report.derived_accounts.len(), 2);
        assert!(
            proof_report.replacement_ownership_verified
                && proof_report.derived_control_graph_verified
        );
        assert!(!proof_report.production_ready);
        assert!(compile(
            &empty_schema(),
            &Storage::default(),
            &replacements,
            &proofs[..1],
            &plan
        )
        .is_err());
        let certificate = verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &plan.derived_accounts,
        )
        .unwrap();
        let mut changed_pair = mapping;
        changed_pair.insert(old_utility, [98; 32]);
        assert!(certificate
            .controller(old_utility, &changed_pair, &proofs)
            .is_err());
    }

    #[test]
    fn unchanged_derived_controller_is_proof_context_and_never_a_retired_replacement() {
        use sp_core::Pair;
        let changed = sp_core::ed25519::Pair::from_seed(&[91; 32]);
        let unchanged = sp_core::ed25519::Pair::from_seed(&[92; 32]);
        let stable = derivative_account(unchanged.public().0, 7);
        let mut old_signers = vec![[1; 32], stable];
        old_signers.sort();
        let mut new_signers = vec![changed.public().0, stable];
        new_signers.sort();
        let old = multisig_account(&old_signers, 2).unwrap();
        let new = multisig_account(&new_signers, 2).unwrap();
        let mut plan = SemanticPlan {
            derived_accounts: vec![
                DerivedAccount {
                    old: hx(&old),
                    new: hx(&new),
                    derivation: DerivedKind::Multisig {
                        signatories: old_signers.iter().map(|s| hx(s)).collect(),
                        threshold: 2,
                    },
                },
                DerivedAccount {
                    old: hx(&stable),
                    new: hx(&stable),
                    derivation: DerivedKind::Utility {
                        controller: hx(&unchanged.public().0),
                        index: 7,
                    },
                },
            ],
            ..Default::default()
        };
        let mut replacements = vec![
            AccountReplacement {
                old: [1; 32].into(),
                new: changed.public().0.into(),
            },
            AccountReplacement {
                old: old.into(),
                new: new.into(),
            },
        ];
        replacements.sort_by(|a, b| a.old.cmp(&b.old));
        let proofs = vec![
            control_proof([1; 32], &changed),
            control_proof(unchanged.public().0, &unchanged),
        ];
        let verified = verify_semantic_ownership(
            &empty_schema(),
            &Storage::default(),
            &replacements,
            &proofs,
            &plan,
        )
        .unwrap();
        assert_eq!(verified.mapped_accounts, 2);
        assert_eq!(verified.direct_accounts, vec![hx(&[1; 32])]);
        assert_eq!(verified.derived_accounts, vec![hx(&old)]);
        assert_eq!(verified.proof_only_derived_accounts, vec![hx(&stable)]);
        let compiled = compile(
            &empty_schema(),
            &Storage::default(),
            &replacements,
            &proofs,
            &plan,
        )
        .unwrap();
        assert_eq!(compiled.report.verified_derived_accounts, vec![hx(&old)]);
        assert_eq!(
            compiled.report.proof_only_derived_accounts,
            vec![hx(&stable)]
        );
        assert!(compiled.edits.is_empty());
        plan.derived_accounts[1].new = hx(&[99; 32]);
        assert!(verify_semantic_ownership(
            &empty_schema(),
            &Storage::default(),
            &replacements,
            &proofs,
            &plan
        )
        .is_err());
    }

    #[test]
    fn fixed_attacker_controller_and_proof_only_node_never_authorize_shared_recovery() {
        let attacker = common::recovery::ATTACKER_ACCOUNT;
        // Policy rejection precedes signature processing: even a correctly
        // signed unchanged-key proof must not retain the known attacker.
        assert!(prove_controller(attacker, &BTreeMap::new(), &[])
            .err()
            .unwrap()
            .contains("fixed attacker cannot remain"));
        let context = vec![DerivedAccount {
            old: hx(&attacker),
            new: hx(&attacker),
            derivation: DerivedKind::Utility {
                controller: hx(&[1; 32]),
                index: 7,
            },
        }];
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &BTreeMap::new(),
            &[],
            &context
        )
        .err()
        .unwrap()
        .contains("proof-only node"));

        use sp_core::Pair;
        let changed = sp_core::ed25519::Pair::from_seed(&[91; 32]);
        let old_leaf = [1; 32];
        let mut old_signers = vec![old_leaf, attacker];
        old_signers.sort();
        let mut new_signers = vec![changed.public().0, attacker];
        new_signers.sort();
        let old = multisig_account(&old_signers, 2).unwrap();
        let new = multisig_account(&new_signers, 2).unwrap();
        let mapping = BTreeMap::from([(old_leaf, changed.public().0), (old, new)]);
        let nodes = vec![DerivedAccount {
            old: hx(&old),
            new: hx(&new),
            derivation: DerivedKind::Multisig {
                signatories: old_signers.iter().map(|s| hx(s)).collect(),
                threshold: 2,
            },
        }];
        let proofs = vec![control_proof(old_leaf, &changed)];
        let before = Storage::default();
        assert!(
            verify_derived(&empty_schema(), &before, &mapping, &proofs, &nodes)
                .err()
                .unwrap()
                .contains("shared ownership requires separate review")
        );
        assert!(before.top.is_empty());
    }

    #[test]
    fn proof_only_derived_context_has_an_independent_total_node_bound() {
        let nodes = (0..=common::recovery::MAX_REPLACEMENTS)
            .map(|i| {
                let old = digest(&i.to_le_bytes());
                DerivedAccount {
                    old: hx(&old),
                    new: hx(&old),
                    derivation: DerivedKind::Utility {
                        controller: hx(&[1; 32]),
                        index: 7,
                    },
                }
            })
            .collect::<Vec<_>>();
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &BTreeMap::new(),
            &[],
            &nodes
        )
        .err()
        .unwrap()
        .contains("node bound"));
    }

    #[test]
    fn threshold_one_derived_multisig_keeps_native_derivation_and_all_controller_proofs() {
        use sp_core::Pair;
        let changed = sp_core::ed25519::Pair::from_seed(&[91; 32]);
        let unchanged = sp_core::ed25519::Pair::from_seed(&[92; 32]);
        let mut signers = vec![[1; 32], unchanged.public().0];
        signers.sort();
        let mut new_signers = vec![changed.public().0, unchanged.public().0];
        new_signers.sort();
        let old = multisig_account(&signers, 1).unwrap();
        let new = multisig_account(&new_signers, 1).unwrap();
        let nodes = vec![DerivedAccount {
            old: hx(&old),
            new: hx(&new),
            derivation: DerivedKind::Multisig {
                signatories: signers.iter().map(|s| hx(s)).collect(),
                threshold: 1,
            },
        }];
        let mapping = BTreeMap::from([([1; 32], changed.public().0), (old, new)]);
        let proofs = vec![
            control_proof([1; 32], &changed),
            control_proof(unchanged.public().0, &unchanged),
        ];
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &nodes
        )
        .is_ok());
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs[..1],
            &nodes
        )
        .is_err());
        assert!(multisig_account(&[[1; 32]], 1).is_err());
        assert!(multisig_account(&signers, 0).is_err());
        #[cfg(feature = "runtime-work-types")]
        {
            let native_signers: Vec<AccountId32> =
                signers.iter().copied().map(Into::into).collect();
            assert_eq!(
                AccountId32::from(old),
                framenode_runtime::Multisig::multi_account_id(&native_signers, 1)
            );
        }
    }

    #[test]
    fn nested_derived_controls_reject_wrong_leaf_proofs_destinations_and_unlisted_nodes() {
        use sp_core::Pair;
        let changed = sp_core::ed25519::Pair::from_seed(&[91; 32]);
        let (mapping, mut nodes) = utility_chain(2, [1; 32], changed.public().0);
        let mut proofs = vec![control_proof([1; 32], &changed)];
        nodes.reverse();
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &nodes
        )
        .is_ok());
        proofs[0].signature = hx(&sp_runtime::MultiSignature::from(
            changed.sign(b"different checkpoint or pair"),
        )
        .encode());
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &nodes
        )
        .is_err());
        proofs[0] = control_proof([1; 32], &changed);
        let removed = nodes.pop().unwrap();
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &nodes
        )
        .is_err());
        nodes.push(removed);
        nodes[0].new = hx(&[98; 32]);
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &nodes
        )
        .is_err());
    }

    #[test]
    fn nested_derived_controls_reject_cycles_duplicates_and_memoized_depth_bypass() {
        use sp_core::Pair;
        let changed = sp_core::ed25519::Pair::from_seed(&[91; 32]);
        let proofs = vec![control_proof([1; 32], &changed)];
        let (mapping, nodes) =
            utility_chain(MAX_DERIVED_CONTROL_DEPTH, [1; 32], changed.public().0);
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &proofs,
            &nodes
        )
        .is_ok());
        let (mapping, mut nodes) =
            utility_chain(MAX_DERIVED_CONTROL_DEPTH + 1, [1; 32], changed.public().0);
        let check = |nodes: &[DerivedAccount]| {
            verify_derived(
                &empty_schema(),
                &Storage::default(),
                &mapping,
                &proofs,
                nodes,
            )
            .err()
            .unwrap()
        };
        assert!(check(&nodes).contains("depth bound"));
        nodes.reverse();
        assert!(check(&nodes).contains("depth bound"));
        let cycle = vec![
            DerivedAccount {
                old: hx(&[11; 32]),
                new: hx(&[21; 32]),
                derivation: DerivedKind::Utility {
                    controller: hx(&[12; 32]),
                    index: 0,
                },
            },
            DerivedAccount {
                old: hx(&[12; 32]),
                new: hx(&[22; 32]),
                derivation: DerivedKind::Utility {
                    controller: hx(&[11; 32]),
                    index: 0,
                },
            },
        ];
        let mapping = BTreeMap::from([([11; 32], [21; 32]), ([12; 32], [22; 32])]);
        assert!(
            verify_derived(&empty_schema(), &Storage::default(), &mapping, &[], &cycle)
                .err()
                .unwrap()
                .contains("cycle")
        );
        let duplicate = vec![
            DerivedAccount {
                old: hx(&[11; 32]),
                new: hx(&[21; 32]),
                derivation: DerivedKind::Utility {
                    controller: hx(&[12; 32]),
                    index: 0,
                },
            },
            DerivedAccount {
                old: hx(&[11; 32]),
                new: hx(&[21; 32]),
                derivation: DerivedKind::Utility {
                    controller: hx(&[12; 32]),
                    index: 0,
                },
            },
        ];
        assert!(verify_derived(
            &empty_schema(),
            &Storage::default(),
            &mapping,
            &[],
            &duplicate
        )
        .err()
        .unwrap()
        .contains("duplicate"));
    }

    #[allow(dead_code)]
    #[derive(Encode, TypeInfo)]
    enum RewardDestination {
        Staked,
        Stash,
        Controller,
        Account(AccountId32),
        None,
    }
    fn shared_cancellation_fixture() -> (MetadataSchema, Storage, SemanticPlan) {
        use crate::inventory::Hasher;
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let hash = registry.register_type(&meta_type::<[u8; 32]>()).id;
        let value = registry
            .register_type(&meta_type::<(u64, u128, AccountId32)>())
            .id;
        let calls_value = registry
            .register_type(&meta_type::<(Vec<u8>, AccountId32, u128)>())
            .id;
        let descriptors = [
            "BridgeMultisig.Multisigs",
            "BridgeMultisig.CancellationApprovals",
            "BridgeMultisig.Calls",
        ]
        .iter()
        .enumerate()
        .map(|(i, name)| Descriptor {
            trie: "top".into(),
            name: (*name).into(),
            prefix: vec![i as u8 + 1; 32],
            key_types: if i == 2 {
                vec![hash]
            } else {
                vec![account, hash]
            },
            hashers: if i == 2 {
                vec![Hasher::Identity]
            } else {
                vec![Hasher::Twox64Concat, Hasher::Blake2_128Concat]
            },
            value_type: if i == 2 { calls_value } else { value },
        })
        .collect::<Vec<_>>();
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors,
            metadata_hash: hx(&[1; 32]),
        };
        let call = vec![30u8, 11, 0, 7];
        let call_hash = digest(&call);
        let mut storage = Storage::default();
        let mut operations = Vec::new();
        for (which, owner) in [(0, 1u8), (0, 2), (1, 1)] {
            let d = &schema.descriptors[which];
            let mut key = d.prefix.clone();
            key.extend(d.hashers[0].apply(&[owner; 32]));
            key.extend(d.hashers[1].apply(&call_hash));
            let bytes = (0u64, 999u128, AccountId32::from([owner; 32])).encode();
            operations.push(ExactCancellationRow {
                storage: d.name.clone(),
                key: hx(&key),
                value_hash: hx(&digest(&bytes)),
            });
            storage.top.insert(key, bytes);
        }
        storage.top.insert(vec![9; 32], vec![5]);
        // Unrelated prefixes must not enter the cancellation index.
        storage.top.insert(vec![0; 32], vec![7]);
        storage.top.insert(vec![255; 32], vec![8]);
        let plan = SemanticPlan {
            bridge_memberships: Vec::new(),
            controls: vec![],
            derived_accounts: vec![],
            bridge_cancellations: vec![BridgeCancellation {
                call_hash: hx(&call_hash),
                recovered_call_preimage: hx(&call),
                calls_value_hash: None,
                operations,
                preserve: vec![PreservationWitness {
                    key: hx(&[9; 32]),
                    value_hash: Some(hx(&digest(&[5]))),
                }],
                review_evidence_hash: hx(&[7; 32]),
            }],
            governance_preservation: None,
        };
        (schema, storage, plan)
    }
    #[test]
    fn indexed_cancellation_remains_exhaustive_for_shared_owners_and_approvals() {
        let (schema, storage, mut plan) = shared_cancellation_fixture();
        let complete = compile(&schema, &storage, &[], &[], &plan).unwrap();
        assert_eq!(complete.edits.len(), 3);
        assert!(complete.edits.iter().all(|e| e.after.is_none()));
        assert!(complete.report.blockers.is_empty());
        assert_eq!(complete.groups.len(), 1);
        assert_eq!(complete.preservation_witnesses.len(), 2);
        plan.bridge_cancellations[0].operations.remove(1);
        assert!(compile(&schema, &storage, &[], &[], &plan)
            .err()
            .unwrap()
            .contains("every pending owner"));
        let (schema, storage, mut plan) = shared_cancellation_fixture();
        plan.bridge_cancellations[0]
            .operations
            .retain(|op| op.storage != "BridgeMultisig.CancellationApprovals");
        assert!(compile(&schema, &storage, &[], &[], &plan)
            .err()
            .unwrap()
            .contains("completely enumerated"));
    }
    #[test]
    fn hash_only_cancellation_requires_actual_absence_and_never_invents_a_deposit() {
        let call = vec![30u8, 11, 0, 7];
        assert!(call_record_owner(None, None, &call).unwrap().is_none());
        assert!(call_record_owner(None, Some(&hx(&[1; 32])), &call).is_err());
        let mut stored = call.encode();
        stored.extend([3u8; 32]);
        stored.extend(999u128.encode());
        assert_eq!(
            call_record_owner(Some(&stored), Some(&hx(&digest(&stored))), &call).unwrap(),
            Some([3; 32])
        );
        assert!(call_record_owner(Some(&stored), None, &call).is_err());
        assert!(
            call_record_owner(Some(&stored), Some(&hx(&digest(&stored))), &[30, 11, 0, 8]).is_err()
        );
    }
    #[test]
    fn dependency_components_preserve_moves_and_shared_payloads_without_global_grouping() {
        let edits = (1u8..=5)
            .map(|n| {
                (
                    vec![n],
                    GuardedStorageEdit {
                        scope: StorageScope::Top,
                        key: vec![n],
                        before: Some([n; 32]),
                        after: None,
                    },
                )
            })
            .collect();
        let groups = dependency_groups(
            &edits,
            &[
                vec![vec![1], vec![2]],
                vec![vec![2], vec![3]],
                vec![vec![4], vec![5]],
            ],
        )
        .unwrap();
        assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), vec![3, 2]);
        assert!(dependency_groups(&edits, &[vec![vec![1], vec![6]]]).is_err());
    }
    #[test]
    fn payout_resets_to_owner_without_preserving_an_unconfirmed_external_recipient() {
        let mut registry = Registry::new();
        let id = registry.register_type(&meta_type::<RewardDestination>()).id;
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![],
            metadata_hash: hx(&[1; 32]),
        };
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Staking.Payee".into(),
            prefix: vec![1; 32],
            key_types: vec![],
            hashers: vec![],
            value_type: id,
        };
        let old = RewardDestination::Account([9; 32].into()).encode();
        assert_eq!(
            default_payee(&old, &descriptor, &schema, [2; 32]).unwrap(),
            RewardDestination::Account([2; 32].into()).encode()
        );
    }
    #[test]
    fn derivations_require_sorted_unique_members_and_same_algorithm_parameters() {
        assert_ne!(
            derivative_account([1; 32], 0),
            derivative_account([1; 32], 1)
        );
        assert_ne!(
            derivative_account([1; 32], 1),
            derivative_account([2; 32], 1)
        );
        assert!(multisig_account(&[[2; 32], [1; 32]], 2).is_err());
        assert!(multisig_account(&[[1; 32], [1; 32]], 2).is_err());
        assert!(multisig_account(&[[1; 32], [2; 32]], 3).is_err());
        assert_ne!(
            bridge_multisig_account([1; 32], 100, 0),
            bridge_multisig_account([1; 32], 100, 1)
        );
    }
    #[test]
    fn proxy_suspension_keeps_unrelated_delegates_and_entire_reserved_deposit() {
        #[derive(Encode, TypeInfo)]
        struct Proxy {
            delegate: AccountId32,
            delay: u32,
        }
        let mut registry = Registry::new();
        let id = registry
            .register_type(&meta_type::<(Vec<Proxy>, u128)>())
            .id;
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![],
            metadata_hash: hx(&[1; 32]),
        };
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Proxy.Proxies".into(),
            prefix: vec![1; 32],
            key_types: vec![],
            hashers: vec![],
            value_type: id,
        };
        let before = (
            vec![
                Proxy {
                    delegate: [1; 32].into(),
                    delay: 0,
                },
                Proxy {
                    delegate: [3; 32].into(),
                    delay: 4,
                },
            ],
            99u128,
        )
            .encode();
        let after = suspend_proxy(
            &before,
            &descriptor,
            &schema,
            &BTreeMap::from([([1; 32], [2; 32])]),
            false,
        )
        .unwrap();
        assert_eq!(
            after,
            (
                vec![Proxy {
                    delegate: [3; 32].into(),
                    delay: 4
                }],
                99u128
            )
                .encode()
        );
        let cleared = suspend_proxy(&before, &descriptor, &schema, &BTreeMap::new(), true).unwrap();
        assert_eq!(cleared, (Vec::<Proxy>::new(), 99u128).encode());
    }
}
