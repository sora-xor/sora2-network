//! Evidence-bound economic candidates from a verified checkpoint. Free balance
//! burns and unissued entitlement extinction are different operations. Shared,
//! encumbered, LP and debt positions never become unconditional token burns.

use crate::{
    account, hex_bytes,
    inventory::{
        Descriptor, InventoryOptions, InventoryReport, MetadataSchema, NativeWatchSummary,
    },
    typed, Result,
};
use codec::Encode;
use common::recovery::{
    invariants::{CollateralInvariant, IssuanceInvariant, StoredAmount, TerminalAudit},
    GuardedStorageEdit, StorageScope,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sp_core::{storage::Storage, U256};
use std::collections::{BTreeMap, BTreeSet};

fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    sp_io::hashing::blake2_256(bytes)
}
fn id(text: &str) -> Result<[u8; 32]> {
    hex_bytes(text)?
        .try_into()
        .map_err(|_| "expected canonical 32-byte identity".into())
}
fn owner(text: &str) -> Result<[u8; 32]> {
    let parsed = account(text)?;
    Ok(*AsRef::<[u8; 32]>::as_ref(&parsed))
}
fn evidence(text: &str) -> Result<()> {
    if id(text)? == [0; 32] {
        return Err("zero evidence commitment is not an approval".into());
    }
    Ok(())
}
fn amount(text: &str) -> Result<u128> {
    if text.is_empty()
        || text.bytes().any(|b| !b.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err("amount must be a canonical decimal integer".into());
    }
    text.parse().map_err(|_| "amount exceeds u128".into())
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BalanceKind {
    Native,
    Tokens,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DirectBurn {
    pub owner: String,
    pub asset: String,
    pub kind: BalanceKind,
    /// Reviewed title/third-party ownership evidence, not a byte match.
    pub ownership_evidence_hash: String,
    /// Missing evidence keeps the entire balance in quarantine. Nonzero frozen
    /// or reserved fields still block burning, regardless of this assertion.
    pub unencumbered_evidence_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EntitlementExtinction {
    pub owner: String,
    pub storage: String,
    pub key: String,
    pub key_preimages: Option<Vec<String>>,
    pub amount_path: Vec<String>,
    pub ownership_evidence_hash: String,
    /// Must establish an UNISSUED liability. Escrow-backed refunds require a
    /// dedicated backing burn/settlement, and cannot use this operation.
    pub unissued_evidence_hash: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionKind {
    LiquidityPool,
    Farming,
    Debt,
    SharedOwnership,
    OutgoingRefund,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QuarantinedPosition {
    pub position_id: String,
    pub owner: String,
    pub asset: String,
    pub kind: PositionKind,
    pub ownership_evidence_hash: String,
    /// Optional exact collateral amount retained in its original record. This
    /// proves preservation only; custody enforcement/settlement remain gated.
    pub collateral: Option<CollateralReference>,
    /// Identifies canonical backing positions to prevent reusing the same
    /// underlying interest as both LP/farming/debt/refund entitlement.
    pub underlying_position_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CollateralReference {
    pub storage: String,
    pub key: String,
    pub amount_path: Vec<String>,
    pub minimum: String,
    pub settlement_evidence_hash: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct EconomicPlan {
    pub attackers: Vec<String>,
    /// Additional explicitly approved EVM identities, including legacy H160
    /// claim owners. The fixed incident address is always included in coverage.
    /// Addresses on other networks require their own typed registry adapters.
    #[serde(default)]
    pub attacker_evm_addresses: Vec<String>,
    #[serde(default)]
    pub protected_owners: Vec<String>,
    #[serde(default)]
    pub direct_burns: Vec<DirectBurn>,
    #[serde(default)]
    pub extinguish_unissued: Vec<EntitlementExtinction>,
    #[serde(default)]
    pub quarantine: Vec<QuarantinedPosition>,
}

/// This is a coverage statement over exact typed references, not a finding of
/// beneficial ownership, derived accounts, or off-chain/external-chain claims.
/// A positive reference requires a source-specific economic disposition; it
/// cannot be promoted to a burn merely because the bytes identify an attacker.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackerCoverageReport {
    pub schema: &'static str,
    pub metadata_hash: String,
    pub attacker_registry: Vec<String>,
    pub attacker_registry_sha256: String,
    pub native_inventory: NativeWatchSummary,
    pub no_observed_direct_typed_identity_references: bool,
    pub beneficial_or_derived_ownership_established: bool,
    pub external_network_registry_complete: bool,
    pub production_ready: bool,
    pub limitation: &'static str,
}

fn attacker_registry(plan: &EconomicPlan) -> Result<BTreeSet<Vec<u8>>> {
    use common::recovery::{ATTACKER_ACCOUNT, ATTACKER_EVM};
    let mut registry = BTreeSet::new();
    for attacker in &plan.attackers {
        if !registry.insert(owner(attacker)?.to_vec()) {
            return Err("attacker coverage registry contains a duplicate AccountId32".into());
        }
    }
    if !registry.contains(ATTACKER_ACCOUNT.as_slice()) {
        return Err("attacker coverage registry omits the fixed incident AccountId32".into());
    }
    let mut evm = BTreeSet::from([ATTACKER_EVM.to_vec()]);
    for address in &plan.attacker_evm_addresses {
        let bytes = hex_bytes(address)?;
        if bytes.len() != 20 || hx(&bytes) != *address || !evm.insert(bytes) {
            return Err("additional attacker EVM identity is not a unique canonical H160".into());
        }
    }
    registry.extend(evm);
    for protected in &plan.protected_owners {
        if registry.contains(owner(protected)?.as_slice()) {
            return Err("attacker coverage registry overlaps protected ownership".into());
        }
    }
    Ok(registry)
}

/// Must run BEFORE the complete native inventory. It does not replace other
/// watched identities, and does not accept a precomputed JSON scan as evidence.
pub fn require_attacker_watch(plan: &EconomicPlan, options: &mut InventoryOptions) -> Result<()> {
    let mut watched = attacker_registry(plan)?;
    let mut supplied = BTreeSet::new();
    for identity in &options.watched_accounts {
        let bytes = hex_bytes(identity)?;
        if ![20, 32].contains(&bytes.len()) || !supplied.insert(bytes.clone()) {
            return Err("existing watched identity is invalid or duplicated".into());
        }
        watched.insert(bytes);
    }
    options.watched_accounts = watched.iter().map(|bytes| hx(bytes)).collect();
    Ok(())
}

/// Current supported acceptance is strict zero observed direct references.
/// Positive interests remain blocked until their exact native ownership,
/// backing and aggregate accounting adapters can certify every affected row.
pub fn validate_attacker_coverage(
    plan: &EconomicPlan,
    inventory: &InventoryReport,
) -> Result<AttackerCoverageReport> {
    let registry = attacker_registry(plan)?;
    let native_inventory = inventory.require_native_watch(&registry)?;
    let canonical: BTreeSet<_> = registry.iter().map(|identity| hx(identity)).collect();
    for record in &inventory.watched_references {
        if record
            .references
            .iter()
            .any(|reference| canonical.contains(&reference.identity))
        {
            return Err(format!("unresolved attacker typed reference at {}/{}/{}; exact ownership/backing adapter required, never infer ownership from a reference", record.trie, record.storage, record.key));
        }
    }
    // An operation itself asserts an interest, so it must not coexist with an
    // absence certificate even when a malformed plan points to unrelated rows.
    if !plan.direct_burns.is_empty()
        || !plan.extinguish_unissued.is_empty()
        || !plan.quarantine.is_empty()
    {
        return Err("attacker economic operations require positive native ownership coverage; zero-reference certificate cannot authorize them".into());
    }
    let canonical: Vec<_> = canonical.into_iter().collect();
    let registry_hash: [u8; 32] = Sha256::digest(
        (
            b"sora-nexus/attacker-typed-registry/v1".as_slice(),
            &registry,
        )
            .encode(),
    )
    .into();
    Ok(AttackerCoverageReport {
        schema: "sora-nexus-attacker-typed-coverage-v1",
        metadata_hash: inventory.metadata_hash.clone(),
        attacker_registry: canonical,
        attacker_registry_sha256: hx(&registry_hash),
        native_inventory,
        no_observed_direct_typed_identity_references: true,
        beneficial_or_derived_ownership_established: false,
        external_network_registry_complete: false,
        production_ready: false,
        limitation: "Complete source-pinned native storage traversal only. Opaque artifacts are retained under their existing non-ownership classification; cancelled incompatible calls are never executed. This does not establish beneficial/derived ownership, complete external-network attacker registries, or TS/SS and external claim evidence.",
    })
}

impl EconomicPlan {
    /// The production composer supplies its actual validated replacement list.
    /// An omitted caller-provided protectedOwners list cannot permit an overlap.
    pub fn protect_replacements(
        &mut self,
        replacements: &[common::recovery::AccountReplacement],
    ) -> Result<()> {
        common::recovery::validate_replacements(replacements)
            .map_err(|e| format!("invalid protected mapping: {e:?}"))?;
        let mut protected = self
            .protected_owners
            .iter()
            .map(|value| owner(value))
            .collect::<Result<BTreeSet<_>>>()?;
        for replacement in replacements {
            protected.insert(*AsRef::<[u8; 32]>::as_ref(&replacement.old));
            protected.insert(*AsRef::<[u8; 32]>::as_ref(&replacement.new));
        }
        for attacker in &self.attackers {
            if protected.contains(&owner(attacker)?) {
                return Err(
                    "attacker registry overlaps a victim or replacement destination".into(),
                );
            }
        }
        self.protected_owners = protected.iter().map(|account| hx(account)).collect();
        Ok(())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BurnSummary {
    pub owner: String,
    pub asset: String,
    pub amount: String,
    pub balance_key: String,
    pub issuance_key: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtinctionSummary {
    pub owner: String,
    pub storage: String,
    pub key: String,
    pub amount: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineEntry {
    pub position_id: String,
    pub owner: String,
    pub reason: String,
    pub retained_value_hash: Option<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomicReport {
    pub schema: &'static str,
    pub metadata_hash: String,
    pub burns: Vec<BurnSummary>,
    pub extinguished_unissued: Vec<ExtinctionSummary>,
    pub quarantine: Vec<QuarantineEntry>,
    pub blockers: Vec<String>,
    pub production_ready: bool,
    pub edits_scale: String,
    pub terminal_audit_scale: String,
}

pub struct EconomicCompilation {
    pub report: EconomicReport,
    pub edits: Vec<GuardedStorageEdit>,
    pub terminal_audit: TerminalAudit,
}

fn descriptor<'a>(schema: &'a MetadataSchema, name: &str) -> Result<&'a Descriptor> {
    schema
        .descriptors
        .iter()
        .find(|d| d.trie == "top" && d.name == name)
        .ok_or_else(|| format!("missing reviewed checkpoint storage {name}"))
}
fn storage_key(
    schema: &MetadataSchema,
    descriptor: &Descriptor,
    components: &[Vec<u8>],
) -> Result<Vec<u8>> {
    if components.len() != descriptor.hashers.len() {
        return Err("economic key components do not match metadata".into());
    }
    let mut key = descriptor.prefix.clone();
    for ((hasher, type_id), component) in descriptor
        .hashers
        .iter()
        .zip(&descriptor.key_types)
        .zip(components)
    {
        typed::transform_value(component, *type_id, &schema.registry, &BTreeMap::new())?;
        key.extend(hasher.apply(component));
    }
    Ok(key)
}
fn record<'a>(storage: &'a Storage, key: &[u8]) -> Result<&'a [u8]> {
    storage
        .top
        .get(key)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing checkpoint record {}", hx(key)))
}
fn projection(
    schema: &MetadataSchema,
    descriptor: &Descriptor,
    key: &[u8],
    value: &[u8],
    path: &[&str],
) -> Result<(u128, StoredAmount)> {
    let (value_amount, offset) =
        typed::u128_field(value, descriptor.value_type, &schema.registry, path)?;
    Ok((
        value_amount,
        StoredAmount {
            scope: StorageScope::Top,
            key: key.to_vec(),
            record_hash: hash(value),
            offset: offset
                .try_into()
                .map_err(|_| "amount projection too large")?,
        },
    ))
}

/// Baseline supply guard for a mapping-only asset. Account replacement never
/// authorizes issuance changes; callers merge this only when no burn/mint
/// invariant for the same asset already exists.
pub fn issuance_baseline(
    schema: &MetadataSchema,
    storage: &Storage,
    asset: [u8; 32],
) -> Result<IssuanceInvariant> {
    let native: [u8; 32] = common::XOR
        .encode()
        .try_into()
        .map_err(|_| "native asset codec")?;
    let descriptor = descriptor(
        schema,
        if asset == native {
            "Balances.TotalIssuance"
        } else {
            "Tokens.TotalIssuance"
        },
    )?;
    let components = if asset == native {
        vec![]
    } else {
        vec![asset.to_vec()]
    };
    let key = storage_key(schema, descriptor, &components)?;
    let before = record(storage, &key)?;
    let (checkpoint_issuance, current) = projection(schema, descriptor, &key, before, &[])?;
    Ok(IssuanceInvariant {
        asset,
        current,
        checkpoint_issuance,
        authenticated_mints: 0,
        burned: 0,
    })
}
fn checked_attacker(attacker: &[u8; 32], attackers: &BTreeSet<[u8; 32]>) -> Result<()> {
    if !attackers.contains(attacker) {
        return Err("economic burn/extinction owner is not an explicit attacker".into());
    }
    Ok(())
}

/// Proportional entitlement arithmetic for review. No pool mutation is emitted
/// from this calculation: pool fees, minimum liquidity, lazy farming rewards and
/// exact withdrawal semantics require the dedicated checkpoint adapter.
pub fn proportional_lp_entitlement(
    reserves: &[u128],
    shares: u128,
    total_shares: u128,
) -> Result<Vec<u128>> {
    if total_shares == 0 || shares > total_shares {
        return Err("invalid LP share ownership".into());
    }
    reserves
        .iter()
        .map(|reserve| {
            let amount = U256::from(*reserve) * U256::from(shares) / U256::from(total_shares);
            if amount > U256::from(u128::MAX) {
                return Err("LP entitlement overflow".into());
            }
            Ok(amount.low_u128())
        })
        .collect()
}

/// Compile from state already independently verified against the checkpoint.
/// The returned candidates are atomic groups, not permission to publish them.
pub fn compile(
    schema: &MetadataSchema,
    storage: &Storage,
    plan: &EconomicPlan,
) -> Result<EconomicCompilation> {
    let attackers = plan
        .attackers
        .iter()
        .map(|a| owner(a))
        .collect::<Result<BTreeSet<_>>>()?;
    if attackers.len() != plan.attackers.len() || attackers.is_empty() {
        return Err("attacker registry is empty or duplicated".into());
    }
    for protected in &plan.protected_owners {
        if attackers.contains(&owner(protected)?) {
            return Err("attacker overlaps protected third-party ownership".into());
        }
    }
    let mut mutations: BTreeMap<Vec<u8>, (Vec<u8>, Vec<u8>)> = BTreeMap::new();
    let mut claimed_records = BTreeSet::new();
    let mut burned_by_asset: BTreeMap<[u8; 32], (Vec<u8>, u128, u128)> = BTreeMap::new();
    let mut burns = Vec::new();
    let extinguished = Vec::new();
    let mut quarantine = Vec::new();
    let mut blockers = Vec::new();
    let mut terminal = TerminalAudit {
        issuance: vec![],
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![],
    };
    let native_asset: [u8; 32] = common::XOR
        .encode()
        .try_into()
        .map_err(|_| "native asset encoding differs from AccountId32 size")?;
    for burn in &plan.direct_burns {
        let attacker = owner(&burn.owner)?;
        checked_attacker(&attacker, &attackers)?;
        let asset = id(&burn.asset)?;
        evidence(&burn.ownership_evidence_hash)?;
        let (
            account_descriptor,
            issuance_descriptor,
            account_components,
            issuance_components,
            data_path,
        ) = match burn.kind {
            BalanceKind::Native => {
                if asset != native_asset {
                    return Err("native balance burn has the wrong asset identity".into());
                }
                (
                    descriptor(schema, "System.Account")?,
                    descriptor(schema, "Balances.TotalIssuance")?,
                    vec![attacker.to_vec()],
                    vec![],
                    vec!["data"],
                )
            }
            BalanceKind::Tokens => {
                if asset == native_asset {
                    return Err("native asset cannot also be counted as Tokens issuance".into());
                }
                (
                    descriptor(schema, "Tokens.Accounts")?,
                    descriptor(schema, "Tokens.TotalIssuance")?,
                    vec![attacker.to_vec(), asset.to_vec()],
                    vec![asset.to_vec()],
                    vec![],
                )
            }
        };
        if account_descriptor
            .key_types
            .first()
            .is_none_or(|id| !typed::account_type(&schema.registry, *id))
        {
            return Err("balance key does not have the expected typed AccountId owner".into());
        }
        let key = storage_key(schema, account_descriptor, &account_components)?;
        if !claimed_records.insert(key.clone()) {
            return Err("duplicate economic position would burn twice".into());
        }
        let before = record(storage, &key)?;
        let fields = typed::named_fields(
            before,
            account_descriptor.value_type,
            &schema.registry,
            &data_path,
        )?;
        if !fields.iter().any(|f| f == "free")
            || !fields.iter().any(|f| f == "reserved")
            || fields.iter().any(|f| {
                ![
                    "free",
                    "reserved",
                    "frozen",
                    "misc_frozen",
                    "fee_frozen",
                    "flags",
                ]
                .contains(&f.as_str())
            })
        {
            return Err("unsupported balance fields require an exact-version adapter".into());
        }
        let mut free_path = data_path.clone();
        free_path.push("free");
        let free = typed::u128_field(
            before,
            account_descriptor.value_type,
            &schema.registry,
            &free_path,
        )?
        .0;
        let mut encumbered = false;
        for field in fields
            .iter()
            .filter(|f| ["reserved", "frozen", "misc_frozen", "fee_frozen"].contains(&f.as_str()))
        {
            let mut path = data_path.clone();
            path.push(field);
            encumbered |= typed::u128_field(
                before,
                account_descriptor.value_type,
                &schema.registry,
                &path,
            )?
            .0 != 0;
        }
        if let Some(clearance) = &burn.unencumbered_evidence_hash {
            evidence(clearance)?;
        }
        if encumbered || burn.unencumbered_evidence_hash.is_none() {
            quarantine.push(QuarantineEntry { position_id: format!("balance:{}", hx(&key)), owner: hx(&attacker), reason: "Reserved/frozen balance or missing third-party/debt clearance; no balance or issuance changes generated".into(), retained_value_hash: Some(hx(&hash(before))) });
            blockers.push(format!(
                "balance {} requires reviewed encumbrance settlement",
                hx(&key)
            ));
            continue;
        }
        // There is no burn when free is already zero. Do not manufacture
        // identical account/issuance edits or a lifecycle transition.
        if free == 0 {
            continue;
        }
        let issuance_key = storage_key(schema, issuance_descriptor, &issuance_components)?;
        let issuance_record = record(storage, &issuance_key)?;
        let checkpoint_issuance = typed::u128_field(
            issuance_record,
            issuance_descriptor.value_type,
            &schema.registry,
            &[],
        )?
        .0;
        let entry =
            burned_by_asset
                .entry(asset)
                .or_insert((issuance_key.clone(), checkpoint_issuance, 0));
        if entry.0 != issuance_key || entry.1 != checkpoint_issuance {
            return Err("asset has conflicting issuance sources".into());
        }
        entry.2 = entry
            .2
            .checked_add(free)
            .ok_or("burn aggregation overflow")?;
        if entry.2 > checkpoint_issuance {
            return Err("attacker burn exceeds total issuance".into());
        }
        let after = typed::set_u128_field(
            before,
            account_descriptor.value_type,
            &schema.registry,
            &free_path,
            0,
        )?;
        if free != 0 {
            mutations.insert(key.clone(), (before.to_vec(), after));
        }
        burns.push(BurnSummary {
            owner: hx(&attacker),
            asset: hx(&asset),
            amount: free.to_string(),
            balance_key: hx(&key),
            issuance_key: hx(&issuance_key),
        });
        // Reference ownership is not derivable from the balance scalar alone.
        // Keep the original System refs/account records until the lifecycle
        // adapter establishes which pallet supplied each provider/consumer.
        if free != 0 {
            blockers.push(format!(
                "zeroed balance {} requires account-reference lifecycle review",
                hx(&key)
            ));
        }
    }
    for (asset, (key, checkpoint_issuance, burned)) in &burned_by_asset {
        let descriptor = descriptor(
            schema,
            if *asset == native_asset {
                "Balances.TotalIssuance"
            } else {
                "Tokens.TotalIssuance"
            },
        )?;
        let before = record(storage, key)?;
        let after = typed::set_u128_field(
            before,
            descriptor.value_type,
            &schema.registry,
            &[],
            checkpoint_issuance - burned,
        )?;
        if mutations
            .insert(key.clone(), (before.to_vec(), after.clone()))
            .is_some()
        {
            return Err("issuance mutation collides with another economic operation".into());
        }
        terminal.issuance.push(IssuanceInvariant {
            asset: *asset,
            current: projection(schema, descriptor, key, &after, &[])?.1,
            checkpoint_issuance: *checkpoint_issuance,
            authenticated_mints: 0,
            burned: *burned,
        });
    }
    if !burned_by_asset.contains_key(&native_asset) {
        terminal
            .issuance
            .push(issuance_baseline(schema, storage, native_asset)?);
    }
    for claim in &plan.extinguish_unissued {
        let attacker = owner(&claim.owner)?;
        checked_attacker(&attacker, &attackers)?;
        evidence(&claim.ownership_evidence_hash)?;
        evidence(&claim.unissued_evidence_hash)?;
        let descriptor = descriptor(schema, &claim.storage)?;
        if crate::inventory::sensitive(descriptor, &schema.registry) {
            return Err(
                "authorization-bearing claim requires a dedicated cancellation adapter".into(),
            );
        }
        let key = hex_bytes(&claim.key)?;
        if !claimed_records.insert(key.clone()) || mutations.contains_key(&key) {
            return Err("overlapping claim/balance interest would be extinguished twice".into());
        }
        let preimages = claim
            .key_preimages
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .map(|p| hex_bytes(p))
                    .collect::<Result<Vec<_>>>()
            })
            .transpose()?;
        let (_, owners) = crate::inventory::transform_key(
            &key,
            descriptor,
            &schema.registry,
            &BTreeMap::new(),
            preimages.as_deref(),
        )?;
        if owners != BTreeSet::from([attacker]) {
            return Err("claim key is not exclusively attributed to the explicit attacker".into());
        }
        let before = record(storage, &key)?;
        let path: Vec<_> = claim.amount_path.iter().map(String::as_str).collect();
        let claim_amount =
            typed::u128_field(before, descriptor.value_type, &schema.registry, &path)?.0;
        // Metadata and an evidence hash alone cannot distinguish an unissued
        // reward from a token balance or a claim backed by a shared reserve.
        // Known deployed rewards also maintain aggregate totals. Until the
        // corresponding exact-prefix adapter checks those relations, preserve
        // the record and block instead of zeroing an arbitrary u128 field.
        quarantine.push(QuarantineEntry {
            position_id: format!("claim:{}", hx(&key)),
            owner: hx(&attacker),
            reason: format!(
                "{} units require a storage-specific unissued/backing and aggregate-total adapter",
                claim_amount
            ),
            retained_value_hash: Some(hx(&hash(before))),
        });
        blockers.push(format!(
            "{} {}: unissued/backing semantics are not yet proven",
            claim.storage, claim.key
        ));
    }
    let mut positions = BTreeSet::new();
    let mut underlying = BTreeSet::new();
    let mut collateral_keys = BTreeSet::new();
    for position in &plan.quarantine {
        let attacker = owner(&position.owner)?;
        checked_attacker(&attacker, &attackers)?;
        evidence(&position.ownership_evidence_hash)?;
        let asset = id(&position.asset)?;
        if position.position_id.is_empty() || !positions.insert(position.position_id.clone()) {
            return Err("duplicate/empty quarantined position identity".into());
        }
        for source in &position.underlying_position_ids {
            if source.is_empty() || !underlying.insert(source.clone()) {
                return Err(
                    "LP/farming/debt positions overlap the same underlying interest".into(),
                );
            }
        }
        let mut retained_hash = None;
        if let Some(collateral) = &position.collateral {
            evidence(&collateral.settlement_evidence_hash)?;
            let descriptor = descriptor(schema, &collateral.storage)?;
            let key = hex_bytes(&collateral.key)?;
            if !collateral_keys.insert(key.clone())
                || mutations.contains_key(&key)
                || claimed_records.contains(&key)
            {
                return Err(
                    "collateral backing is duplicated or overlaps a burn/extinction".into(),
                );
            }
            let before = record(storage, &key)?;
            let path: Vec<_> = collateral.amount_path.iter().map(String::as_str).collect();
            let (retained, projection) = projection(schema, descriptor, &key, before, &path)?;
            let minimum = amount(&collateral.minimum)?;
            if retained < minimum {
                return Err("retained collateral cannot cover reviewed debt backing".into());
            }
            retained_hash = Some(hx(&hash(before)));
            terminal.collateral.push(CollateralInvariant {
                asset,
                retained: vec![projection],
                minimum,
            });
        }
        let reason = match position.kind {
            PositionKind::LiquidityPool => "LP withdrawal must run the exact pool adapter before burning proportional underlying assets",
            PositionKind::Farming => "Farming shares/lazy rewards require reconciliation with the single underlying position",
            PositionKind::Debt => "Debt settlement/custody enforcement must preserve required collateral before residual burn",
            PositionKind::SharedOwnership => "Third-party beneficial ownership requires a reviewed attribution/settlement adapter",
            PositionKind::OutgoingRefund => "No refund or backing burn without counterpart finality and all redeemable-authorization evidence",
        };
        quarantine.push(QuarantineEntry {
            position_id: position.position_id.clone(),
            owner: hx(&attacker),
            reason: reason.into(),
            retained_value_hash: retained_hash,
        });
        blockers.push(format!("{}: {reason}", position.position_id));
    }
    let edits: Vec<_> = mutations
        .into_iter()
        .map(|(key, (before, after))| GuardedStorageEdit {
            scope: StorageScope::Top,
            key,
            before: Some(hash(&before)),
            after: Some(after),
        })
        .collect();
    let report = EconomicReport {
        schema: "sora-nexus-economic-candidates-v1",
        metadata_hash: schema.metadata_hash.clone(),
        burns,
        extinguished_unissued: extinguished,
        quarantine,
        blockers,
        production_ready: false,
        edits_scale: hx(&edits.encode()),
        terminal_audit_scale: hx(&terminal.encode()),
    };
    Ok(EconomicCompilation {
        report,
        edits,
        terminal_audit: terminal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Hasher;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};
    use sp_core::crypto::AccountId32;
    #[derive(Encode, TypeInfo)]
    struct TokenData {
        free: u128,
        reserved: u128,
        frozen: u128,
    }
    fn coverage_fixture() -> (MetadataSchema, Storage, EconomicPlan, InventoryOptions) {
        let (schema, storage, mut plan) = setup(0);
        plan.attackers = vec![hx(&common::recovery::ATTACKER_ACCOUNT)];
        plan.direct_burns.clear();
        let options = InventoryOptions {
            dispositions: schema
                .descriptors
                .iter()
                .map(|descriptor| crate::inventory::StorageDisposition {
                    trie: descriptor.trie.clone(),
                    storage: descriptor.name.clone(),
                    action: crate::inventory::Disposition::RetainHistorical,
                    reason: "Public native coverage test; no mapped owners".into(),
                    evidence_hash: hx(&[7; 32]),
                })
                .collect(),
            ..Default::default()
        };
        (schema, storage, plan, options)
    }
    #[test]
    fn coverage_requires_fixed_registry_and_watches_additional_h160_without_inferring_ownership() {
        let (_, _, mut plan, mut options) = coverage_fixture();
        plan.attackers.clear();
        assert!(require_attacker_watch(&plan, &mut options)
            .unwrap_err()
            .contains("fixed incident"));
        plan.attackers.push(hx(&common::recovery::ATTACKER_ACCOUNT));
        plan.attacker_evm_addresses.push(hx(&[9; 20]));
        require_attacker_watch(&plan, &mut options).unwrap();
        assert!(options
            .watched_accounts
            .contains(&hx(&common::recovery::ATTACKER_EVM)));
        assert!(options.watched_accounts.contains(&hx(&[9; 20])));
        plan.attacker_evm_addresses.push(hx(&[9; 20]));
        assert!(require_attacker_watch(&plan, &mut options).is_err());
        plan.attacker_evm_addresses = vec![hx(&[9; 21])];
        assert!(require_attacker_watch(&plan, &mut options).is_err());
    }
    #[test]
    fn coverage_certificate_binds_native_scan_and_cannot_be_promoted_from_changed_report() {
        let (schema, storage, plan, mut options) = coverage_fixture();
        let unwatched =
            crate::inventory::inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert!(validate_attacker_coverage(&plan, &unwatched).is_err());
        require_attacker_watch(&plan, &mut options).unwrap();
        let mut inventory =
            crate::inventory::inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        let coverage = validate_attacker_coverage(&plan, &inventory).unwrap();
        assert!(coverage.no_observed_direct_typed_identity_references);
        assert!(!coverage.production_ready);
        assert!(!coverage.beneficial_or_derived_ownership_established);
        assert!(!coverage.external_network_registry_complete);
        inventory.top_entries += 1;
        assert!(validate_attacker_coverage(&plan, &inventory).is_err());
    }
    #[test]
    fn coverage_rejects_omitted_interests_even_with_no_burn_operations_and_rejects_hidden_hits() {
        let (schema, storage, mut plan, mut options) = coverage_fixture();
        plan.attackers.push(hx(&[3; 32]));
        require_attacker_watch(&plan, &mut options).unwrap();
        let mut inventory =
            crate::inventory::inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert!(validate_attacker_coverage(&plan, &inventory)
            .unwrap_err()
            .contains("unresolved attacker typed reference"));
        inventory.watched_references.clear();
        assert!(validate_attacker_coverage(&plan, &inventory).is_err());
    }
    #[test]
    fn coverage_detects_h160_claim_keys_and_unknown_storage_prevents_certificate() {
        let (mut schema, mut storage, plan, mut options) = coverage_fixture();
        let mut registry = Registry::new();
        let address = registry.register_type(&meta_type::<sp_core::H160>()).id;
        let balance = registry.register_type(&meta_type::<u128>()).id;
        schema.registry = registry.into();
        schema.descriptors = vec![Descriptor {
            trie: "top".into(),
            name: "Rewards.ValOwners".into(),
            prefix: vec![11; 32],
            key_types: vec![address],
            hashers: vec![Hasher::Blake2_128Concat],
            value_type: balance,
        }];
        options.dispositions.truncate(1);
        options.dispositions[0].storage = "Rewards.ValOwners".into();
        storage.top.clear();
        let key = storage_key(
            &schema,
            &schema.descriptors[0],
            &[common::recovery::ATTACKER_EVM.to_vec()],
        )
        .unwrap();
        storage.top.insert(key, 17u128.encode());
        require_attacker_watch(&plan, &mut options).unwrap();
        let inventory =
            crate::inventory::inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert!(validate_attacker_coverage(&plan, &inventory)
            .unwrap_err()
            .contains("unresolved attacker typed reference"));
        storage.top.clear();
        options.dispositions.clear();
        storage.top.insert(vec![88; 32], vec![1]);
        let inventory =
            crate::inventory::inventory(&schema, &storage, &BTreeMap::new(), &options).unwrap();
        assert!(validate_attacker_coverage(&plan, &inventory).is_err());
    }
    #[test]
    fn mapping_itself_protects_both_old_and_new_even_when_protected_owners_are_omitted() {
        let mapping = vec![common::recovery::AccountReplacement {
            old: [1; 32].into(),
            new: [2; 32].into(),
        }];
        for attacker in [1u8, 2] {
            let mut plan = EconomicPlan {
                attackers: vec![hx(&[attacker; 32])],
                ..Default::default()
            };
            assert!(plan
                .protect_replacements(&mapping)
                .unwrap_err()
                .contains("overlaps"));
        }
        let mut plan = EconomicPlan {
            attackers: vec![hx(&[3; 32])],
            ..Default::default()
        };
        plan.protect_replacements(&mapping).unwrap();
        assert_eq!(plan.protected_owners, vec![hx(&[1; 32]), hx(&[2; 32])]);
        plan.protect_replacements(&mapping).unwrap();
        assert_eq!(plan.protected_owners.len(), 2);
    }
    fn setup(reserved: u128) -> (MetadataSchema, Storage, EconomicPlan) {
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<AccountId32>()).id;
        let asset = registry.register_type(&meta_type::<[u8; 32]>()).id;
        let data = registry.register_type(&meta_type::<TokenData>()).id;
        let balance = registry.register_type(&meta_type::<u128>()).id;
        let accounts = Descriptor {
            trie: "top".into(),
            name: "Tokens.Accounts".into(),
            prefix: vec![1; 32],
            key_types: vec![account, asset],
            hashers: vec![Hasher::Blake2_128Concat, Hasher::Twox64Concat],
            value_type: data,
        };
        let issuance = Descriptor {
            trie: "top".into(),
            name: "Tokens.TotalIssuance".into(),
            prefix: vec![2; 32],
            key_types: vec![asset],
            hashers: vec![Hasher::Twox64Concat],
            value_type: balance,
        };
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![
                accounts,
                issuance,
                Descriptor {
                    trie: "top".into(),
                    name: "Balances.TotalIssuance".into(),
                    prefix: vec![9; 32],
                    key_types: vec![],
                    hashers: vec![],
                    value_type: balance,
                },
            ],
            metadata_hash: hx(&[8; 32]),
        };
        let mut storage = Storage::default();
        storage.top.insert(vec![9; 32], 1_000_000u128.encode());
        storage.top.insert(
            storage_key(&schema, &schema.descriptors[0], &[vec![3; 32], vec![4; 32]]).unwrap(),
            TokenData {
                free: 70,
                reserved,
                frozen: 0,
            }
            .encode(),
        );
        storage.top.insert(
            storage_key(&schema, &schema.descriptors[1], &[vec![4; 32]]).unwrap(),
            1000u128.encode(),
        );
        let plan = EconomicPlan {
            attackers: vec![hx(&[3; 32])],
            direct_burns: vec![DirectBurn {
                owner: hx(&[3; 32]),
                asset: hx(&[4; 32]),
                kind: BalanceKind::Tokens,
                ownership_evidence_hash: hx(&[5; 32]),
                unencumbered_evidence_hash: Some(hx(&[6; 32])),
            }],
            ..Default::default()
        };
        (schema, storage, plan)
    }
    #[test]
    fn no_attacker_balance_still_audits_native_supply_and_mapping_assets() {
        let (schema, storage, mut plan) = setup(0);
        plan.direct_burns.clear();
        let compiled = compile(&schema, &storage, &plan).unwrap();
        assert!(compiled.edits.is_empty());
        assert_eq!(compiled.terminal_audit.issuance.len(), 1);
        assert_eq!(
            compiled.terminal_audit.issuance[0].checkpoint_issuance,
            1_000_000
        );
        assert_eq!(compiled.terminal_audit.issuance[0].burned, 0);
        let token = issuance_baseline(&schema, &storage, [4; 32]).unwrap();
        assert_eq!(token.checkpoint_issuance, 1000);
        assert_eq!(token.burned, 0);
    }
    #[test]
    fn zero_free_burn_never_emits_identical_balance_or_issuance_edits() {
        let (schema, mut storage, plan) = setup(0);
        let key =
            storage_key(&schema, &schema.descriptors[0], &[vec![3; 32], vec![4; 32]]).unwrap();
        storage.top.insert(
            key,
            TokenData {
                free: 0,
                reserved: 0,
                frozen: 0,
            }
            .encode(),
        );
        let compiled = compile(&schema, &storage, &plan).unwrap();
        assert!(compiled.edits.is_empty());
        assert!(compiled.report.burns.is_empty());
        assert!(compiled.report.blockers.is_empty());
        assert_eq!(compiled.terminal_audit.issuance.len(), 1);
    }
    #[test]
    fn direct_free_burn_has_exact_matching_issuance_reduction() {
        let (schema, storage, plan) = setup(0);
        let compiled = compile(&schema, &storage, &plan).unwrap();
        assert_eq!(compiled.edits.len(), 2);
        assert_eq!(compiled.terminal_audit.issuance[0].burned, 70);
        assert_eq!(
            compiled.terminal_audit.issuance[0].checkpoint_issuance,
            1000
        );
        let supply = compiled
            .edits
            .iter()
            .find(|e| e.key.starts_with(&[2; 32]))
            .unwrap();
        assert_eq!(supply.after.as_ref().unwrap(), &930u128.encode());
        assert!(!compiled.report.production_ready);
    }
    #[test]
    fn reserved_and_missing_clearance_quarantine_without_any_issuance_change() {
        let (schema, storage, plan) = setup(1);
        let compiled = compile(&schema, &storage, &plan).unwrap();
        assert!(compiled.edits.is_empty());
        assert_eq!(compiled.report.quarantine.len(), 1);
        let (schema, storage, mut plan) = setup(0);
        plan.direct_burns[0].unencumbered_evidence_hash = None;
        assert!(compile(&schema, &storage, &plan).unwrap().edits.is_empty());
    }
    #[test]
    fn duplicate_burn_shared_owner_and_issuance_underflow_are_rejected() {
        let (schema, mut storage, mut plan) = setup(0);
        plan.protected_owners.push(hx(&[3; 32]));
        assert!(compile(&schema, &storage, &plan).is_err());
        plan.protected_owners.clear();
        plan.direct_burns.push(DirectBurn {
            owner: hx(&[3; 32]),
            asset: hx(&[4; 32]),
            kind: BalanceKind::Tokens,
            ownership_evidence_hash: hx(&[5; 32]),
            unencumbered_evidence_hash: Some(hx(&[6; 32])),
        });
        assert!(compile(&schema, &storage, &plan).is_err());
        plan.direct_burns.pop();
        storage.top.insert(
            storage_key(&schema, &schema.descriptors[1], &[vec![4; 32]]).unwrap(),
            60u128.encode(),
        );
        assert!(compile(&schema, &storage, &plan).is_err());
    }
    #[test]
    fn debt_collateral_is_unchanged_and_cannot_also_be_burned() {
        let (schema, storage, mut plan) = setup(0);
        let key =
            storage_key(&schema, &schema.descriptors[0], &[vec![3; 32], vec![4; 32]]).unwrap();
        plan.quarantine.push(QuarantinedPosition {
            position_id: "debt-1".into(),
            owner: hx(&[3; 32]),
            asset: hx(&[4; 32]),
            kind: PositionKind::Debt,
            ownership_evidence_hash: hx(&[5; 32]),
            underlying_position_ids: vec!["collateral-1".into()],
            collateral: Some(CollateralReference {
                storage: "Tokens.Accounts".into(),
                key: hx(&key),
                amount_path: vec!["free".into()],
                minimum: "60".into(),
                settlement_evidence_hash: hx(&[6; 32]),
            }),
        });
        assert!(compile(&schema, &storage, &plan).is_err());
        plan.direct_burns.clear();
        let compiled = compile(&schema, &storage, &plan).unwrap();
        assert!(compiled.edits.is_empty());
        assert_eq!(compiled.terminal_audit.collateral[0].minimum, 60);
        plan.quarantine[0].collateral.as_mut().unwrap().minimum = "71".into();
        assert!(compile(&schema, &storage, &plan).is_err());
    }

    #[test]
    fn evidence_hash_cannot_relabel_an_issued_balance_as_an_unissued_claim() {
        let (schema, storage, mut plan) = setup(0);
        let key =
            storage_key(&schema, &schema.descriptors[0], &[vec![3; 32], vec![4; 32]]).unwrap();
        plan.direct_burns.clear();
        plan.extinguish_unissued.push(EntitlementExtinction {
            owner: hx(&[3; 32]),
            storage: "Tokens.Accounts".into(),
            key: hx(&key),
            key_preimages: None,
            amount_path: vec!["free".into()],
            ownership_evidence_hash: hx(&[5; 32]),
            unissued_evidence_hash: hx(&[6; 32]),
        });
        let result = compile(&schema, &storage, &plan).unwrap();
        assert!(result.edits.is_empty());
        assert!(result.report.extinguished_unissued.is_empty());
        assert_eq!(result.report.quarantine.len(), 1);
        assert!(!result.report.blockers.is_empty());
    }
    #[test]
    fn lp_math_preserves_rounding_and_cannot_overflow_but_does_not_generate_pool_edits() {
        assert_eq!(
            proportional_lp_entitlement(&[100, 200], 1, 3).unwrap(),
            vec![33, 66]
        );
        assert_eq!(
            proportional_lp_entitlement(&[u128::MAX], u128::MAX, u128::MAX).unwrap(),
            vec![u128::MAX]
        );
        assert!(proportional_lp_entitlement(&[10], 2, 1).is_err());
        assert!(proportional_lp_entitlement(&[10], 0, 0).is_err());
    }
}
