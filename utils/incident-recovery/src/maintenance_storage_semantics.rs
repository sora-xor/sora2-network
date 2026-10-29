//! Same-process post-activation inspection, never an imported JSON certificate.
//! Complete history/derived ownership and semantic interpretation remain separate.
use super::{BlockWitness, Manifest};
use crate::{inventory, replay, typed, Result};
use codec::DecodeAll;
use common::recovery::{self, RecoveryAction, RecoveryInherent, RecoveryPolicy};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::Path,
};

const MAX_ENTRIES: usize = 4_000_000;
const MAX_BYTES: usize = 64 * 1024 * 1024 * 1024;
const MAX_VALUE: usize = 32 * 1024 * 1024;
const MAX_CHILDREN: usize = 4096;
const MAX_EXAMPLES: usize = 256;

#[path = "maintenance_storage_financial.rs"]
mod financial;
#[path = "maintenance_storage_ledger.rs"]
mod ledger;
pub(super) type LedgerBaseline = ledger::Baseline;
pub(super) fn capture_ledger(
    ext: &mut sp_io::TestExternalities,
    schema: &inventory::MetadataSchema,
) -> Result<LedgerBaseline> {
    ledger::checkpoint(ext, schema)
}

fn need(value: bool, message: &str) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn read(child: Option<&[u8]>, key: &[u8]) -> Result<Vec<u8>> {
    let length = match child {
        None => sp_io::storage::read(key, &mut [], 0),
        Some(id) => sp_io::default_child_storage::read(id, key, &mut [], 0),
    }
    .ok_or("enumerated storage value disappeared")? as usize;
    need(length <= MAX_VALUE, "final storage value bound")?;
    let value = match child {
        None => sp_io::storage::get(key).map(|value| value.to_vec()),
        Some(id) => sp_io::default_child_storage::get(id, key),
    }
    .ok_or("enumerated storage value disappeared")?;
    need(
        value.len() == length,
        "final storage value changed during read",
    )?;
    Ok(value)
}
fn next(child: Option<&[u8]>, key: &[u8]) -> Option<Vec<u8>> {
    match child {
        None => sp_io::storage::next_key(key),
        Some(id) => sp_io::default_child_storage::next_key(id, key),
    }
}

// Exact current storage semantics, not a name-fragment heuristic. These records
// still determine mutable ownership/control or future financial settlement.
// In particular, staking exposure/reward/slash history is NOT an immutable proof.
fn live_family(name: &str) -> Option<&'static str> {
    Some(match name {
        "System.Account" | "Tokens.Accounts" => "liquidOwnership",
        "Soratopia.LastCheckIn" => "soratopiaEligibility",
        "Staking.Bonded"
        | "Staking.Ledger"
        | "Staking.Payee"
        | "Staking.Validators"
        | "Staking.Nominators"
        | "Staking.VirtualStakers"
        | "Staking.Invulnerables"
        | "Staking.LastValidatorEra"
        | "Staking.VoterSnapshotStatus"
        | "Staking.ElectableStashes" => "stakingControlAndPositions",
        "Staking.ErasStakers"
        | "Staking.ErasStakersClipped"
        | "Staking.ErasStakersOverview"
        | "Staking.ErasStakersPaged"
        | "Staking.ErasValidatorPrefs"
        | "Staking.ErasRewardPoints"
        | "Staking.ClaimedRewards"
        | "Staking.ValidatorSlashInEra"
        | "Staking.NominatorSlashInEra"
        | "Staking.SlashingSpans"
        | "Staking.SpanSlash"
        | "Staking.UnappliedSlashes"
        | "Staking.AsyncUnappliedSlashes"
        | "Staking.OffenceQueue"
        | "Staking.ProcessingOffence"
        | "Staking.CancelledSlashes"
        | "Staking.ErasValidatorIncentiveWeight" => "stakingSettlement",
        "Session.NextKeys"
        | "Session.QueuedKeys"
        | "Session.Validators"
        | "Session.KeyOwner"
        | "Council.Members"
        | "Council.Prime"
        | "Council.Voting"
        | "Council.CostOf"
        | "TechnicalCommittee.Members"
        | "TechnicalCommittee.Prime"
        | "TechnicalCommittee.Voting"
        | "TechnicalCommittee.CostOf"
        | "TechnicalMembership.Members"
        | "TechnicalMembership.Prime"
        | "Democracy.VotingOf"
        | "Democracy.DepositOf"
        | "Assets.AssetOwners"
        | "Permissions.Owners"
        | "Permissions.Permissions"
        | "Referrals.Referrers"
        | "Referrals.Referrals" => "liveControl",
        "PoolXYK.PoolProviders"
        | "PoolXYK.AccountPools"
        | "PoolXYK.TotalIssuances"
        | "PoolXYK.Properties"
        | "DemeterFarmingPlatform.UserInfos"
        | "DemeterFarmingPlatform.TokenInfos" => "liquidityAndFarming",
        "Kensetsu.CDPDepository"
        | "Kensetsu.CdpOwnerIndex"
        | "ApolloPlatform.UserLendingInfo"
        | "ApolloPlatform.UserBorrowingInfo"
        | "ApolloPlatform.UserTotalCollateral" => "debtAndCollateral",
        "VestedRewards.Rewards"
        | "VestedRewards.PendingClaims"
        | "VestedRewards.ClaimSchedules"
        | "VestedRewards.VestingSchedules"
        | "VestedRewards.CrowdloanInfos"
        | "VestedRewards.CrowdloanUserInfos"
        | "PswapDistribution.ShareholderAccounts"
        | "Referrals.ReferrerBalances" => "unsettledClaims",
        _ => return None,
    })
}

fn prefix(name: &str) -> Vec<u8> {
    let (pallet, item) = name
        .split_once('.')
        .expect("fixed storage name has separator");
    [
        sp_io::hashing::twox_128(pallet.as_bytes()),
        sp_io::hashing::twox_128(item.as_bytes()),
    ]
    .concat()
}

fn validate_live_descriptors(schema: &inventory::MetadataSchema) -> Result<()> {
    let mut seen = BTreeSet::new();
    for d in &schema.descriptors {
        if live_family(&d.name).is_some() {
            need(
                d.trie == "top" && d.prefix == prefix(&d.name) && seen.insert(d.name.clone()),
                "reviewed live storage descriptor identity differs",
            )?;
        }
    }
    Ok(())
}

const RETIRED_LIVE: &str =
    "retired identity remains in live ownership/control or settlement storage";

struct Row {
    name: String,
    retired: usize,
    issues: Vec<String>,
    family: Option<&'static str>,
    explicit_legacy_codec: bool,
}

fn typed_row(
    schema: &inventory::MetadataSchema,
    d: &inventory::Descriptor,
    key: &[u8],
    value: &[u8],
    retired: &BTreeSet<[u8; 32]>,
) -> Result<Row> {
    let family = live_family(&d.name);
    let mut accounts = BTreeSet::new();
    let mut issues = Vec::new();
    // Key and value decoding are independent: an opaque map key cannot hide a
    // typed owner in its value. Decode failures remain explicit coverage gaps.
    match inventory::transform_key(key, d, &schema.registry, &BTreeMap::new(), None) {
        Ok((_, found)) => accounts.extend(found),
        Err(error) => issues.push(format!("key: {error}")),
    }
    match typed::transform_value(value, d.value_type, &schema.registry, &BTreeMap::new()) {
        Ok((_, found)) => accounts.extend(found),
        Err(error) => issues.push(format!("value: {error}")),
    }
    if d.name == "Permissions.Permissions" {
        // The holder is the first Blake2_256 key. Its preimage is known for each
        // retired account; scopes need not be inverted to reject an old holder.
        need(
            d.key_types.len() == 2
                && d.hashers == [inventory::Hasher::Blake2_256, inventory::Hasher::Blake2_256]
                && typed::account_type(&schema.registry, d.key_types[0])
                && key.len() == 96,
            "permissions holder key no longer has reviewed hashed account/scope codec",
        )?;
        for old in retired {
            if key[32..64] == sp_io::hashing::blake2_256(old) {
                accounts.insert(*old);
            }
        }
    }
    let count = accounts.intersection(retired).count();
    need(family.is_none() || count == 0, RETIRED_LIVE)?;
    Ok(Row {
        name: d.name.clone(),
        retired: count,
        issues,
        family,
        explicit_legacy_codec: false,
    })
}

// Final138 still consumes this storage_alias queue, which FRAME metadata omits.
// Its exact local legacy.rs layout is intentionally explicit and bounded; no
// historical-name waiver is applied to a still-payable/slashable record.
#[derive(codec::Decode, codec::Encode)]
struct HistoricalSlashRecord {
    offence_era: u32,
    fraction: sp_runtime::Perbill,
    slash: LegacySlashRecord,
}
#[derive(codec::Decode, codec::Encode)]
struct LegacySlashRecord {
    validator: sp_core::crypto::AccountId32,
    own: u128,
    others: Vec<(sp_core::crypto::AccountId32, u128)>,
    reporters: Vec<sp_core::crypto::AccountId32>,
    payout: u128,
}
fn historical_slash_row(key: &[u8], value: &[u8], retired: &BTreeSet<[u8; 32]>) -> Result<Row> {
    use codec::Encode;
    need(
        key.len() == 60
            && key.starts_with(&prefix("Staking.HistoricalSlashes"))
            && key[32..40] == sp_io::hashing::twox_64(&key[40..44])
            && key[44..52] == sp_io::hashing::twox_64(&key[52..60]),
        "historical slash queue key is not canonical era/index",
    )?;
    need(
        value.len() <= 512 * 1024,
        "historical slash queue value bound",
    )?;
    let mut input = value;
    let record = HistoricalSlashRecord::decode_all(&mut input)
        .map_err(|e| format!("historical slash queue codec: {e}"))?;
    need(
        record.encode() == value
            && record.slash.others.len() <= 2048
            && record.slash.reporters.len() <= 2048,
        "historical slash queue canonical/bounded record differs",
    )?;
    let accounts: BTreeSet<[u8; 32]> = std::iter::once(&record.slash.validator)
        .chain(record.slash.others.iter().map(|(who, _)| who))
        .chain(record.slash.reporters.iter())
        .map(|who| *AsRef::<[u8; 32]>::as_ref(who))
        .collect();
    need(accounts.is_disjoint(retired), RETIRED_LIVE)?;
    Ok(Row {
        name: "Staking.HistoricalSlashes".into(),
        retired: 0,
        issues: Vec::new(),
        family: Some("stakingSettlement"),
        explicit_legacy_codec: true,
    })
}

/// Enumerate the complete final trie and all child tries. Decode final138 metadata
/// and the explicitly reviewed legacy slash alias. Other unknown/legacy/hash-only
/// keys remain gaps; arbitrary bytes and historical labels cannot waive them.
fn scan(schema: &inventory::MetadataSchema, retired: &BTreeSet<[u8; 32]>) -> Result<Value> {
    validate_live_descriptors(schema)?;
    let mut queue = vec![None::<Vec<u8>>];
    let mut seen_children = BTreeSet::new();
    let mut entries = 0usize;
    let mut total = 0usize;
    let mut decoded = 0usize;
    let mut legacy_decoded = 0usize;
    let mut old_references = 0usize;
    let mut reviewed_rows = BTreeMap::<String, usize>::new();
    let mut reviewed_partial_rows = BTreeMap::<String, usize>::new();
    let mut issues = BTreeMap::<String, usize>::new();
    let mut examples = Vec::new();
    let mut digest = Sha256::new();
    digest.update(b"sora-nexus/final-typed-storage-scan/v1");
    let mut index = 0usize;
    while index < queue.len() {
        let child = queue[index].clone();
        index += 1;
        let trie = child
            .as_ref()
            .map(|id| {
                format!(
                    "0x{}",
                    hex::encode([b":child_storage:default:".as_slice(), id.as_slice()].concat())
                )
            })
            .unwrap_or_else(|| "top".into());
        digest.update((trie.len() as u64).to_le_bytes());
        digest.update(trie.as_bytes());
        let mut key = if match child.as_deref() {
            None => sp_io::storage::exists(b""),
            Some(id) => sp_io::default_child_storage::exists(id, b""),
        } {
            Some(Vec::new())
        } else {
            next(child.as_deref(), b"")
        };
        while let Some(current) = key {
            let value = read(child.as_deref(), &current)?;
            entries = entries.checked_add(1).ok_or("entry count overflow")?;
            total = total
                .checked_add(current.len())
                .and_then(|n| n.checked_add(value.len()))
                .ok_or("storage byte overflow")?;
            need(
                entries <= MAX_ENTRIES && total <= MAX_BYTES && current.len() <= 4096,
                "final storage scan bound",
            )?;
            digest.update((current.len() as u64).to_le_bytes());
            digest.update(&current);
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(&value);
            if child.is_none() && current.starts_with(b":child_storage:") {
                let id = current
                    .strip_prefix(b":child_storage:default:")
                    .ok_or("unsupported final child storage kind")?;
                need(
                    !id.is_empty()
                        && id.len() <= 1024
                        && value.len() == 32
                        && seen_children.insert(id.to_vec()),
                    "invalid/duplicate final child root",
                )?;
                need(
                    seen_children.len() <= MAX_CHILDREN,
                    "final child trie bound",
                )?;
                queue.push(Some(id.to_vec()));
            } else {
                let descriptors: Vec<_> = schema
                    .descriptors
                    .iter()
                    .filter(|d| d.trie == trie && current.starts_with(&d.prefix))
                    .collect();
                let outcome = (|| -> Result<Row> {
                    if child.is_none() && current.starts_with(&prefix("Staking.HistoricalSlashes"))
                    {
                        need(
                            descriptors.is_empty(),
                            "historical slash alias unexpectedly entered metadata",
                        )?;
                        return historical_slash_row(&current, &value, retired);
                    }
                    need(
                        descriptors.len() == 1,
                        "unknown or overlapping final metadata storage",
                    )?;
                    typed_row(schema, descriptors[0], &current, &value, retired)
                })();
                match outcome {
                    Ok(row) => {
                        old_references += row.retired;
                        if row.issues.is_empty() {
                            if row.explicit_legacy_codec {
                                legacy_decoded += 1;
                            } else {
                                decoded += 1;
                            }
                            if row.family.is_some() {
                                *reviewed_rows.entry(row.name.clone()).or_default() += 1;
                            }
                        } else {
                            if row.family.is_some() {
                                *reviewed_partial_rows.entry(row.name.clone()).or_default() += 1;
                            }
                            for error in &row.issues {
                                *issues.entry(error.clone()).or_default() += 1;
                            }
                        }
                        if (row.retired > 0 || !row.issues.is_empty())
                            && examples.len() < MAX_EXAMPLES
                        {
                            examples.push(json!({"trie":trie,"storage":row.name,"key":"0x".to_owned()+&hex::encode(&current),"retiredReferences":row.retired,"issues":row.issues,"reviewedLiveFamily":row.family,"dispositionVerified":false}));
                        }
                    }
                    Err(error) if error == RETIRED_LIVE => return Err(error),
                    Err(error) => {
                        *issues.entry(error.clone()).or_default() += 1;
                        if examples.len() < MAX_EXAMPLES {
                            examples.push(json!({"trie":trie,"key":"0x".to_owned()+&hex::encode(&current),"error":error}));
                        }
                    }
                }
            }
            key = next(child.as_deref(), &current);
            need(
                key.as_ref().is_none_or(|new| new > &current),
                "nonmonotonic final storage iteration",
            )?;
        }
    }
    Ok(
        json!({"entries":entries,"bytes":total,"childTries":seen_children.len(),"metadataDecodedEntries":decoded,"explicitLegacyDecodedEntries":legacy_decoded,"retiredTypedReferences":old_references,"issues":issues,"examples":examples,"completeTrieEnumerated":true,"allEntriesTyped":issues.is_empty(),"typedScanSha256":hex::encode(digest.finalize()),"reviewedLiveDescriptors":schema.descriptors.iter().filter(|d|live_family(&d.name).is_some()).map(|d|d.name.as_str()).collect::<Vec<_>>(),"reviewedLiveTypedRows":reviewed_rows,"reviewedLivePartiallyTypedRows":reviewed_partial_rows,"historicalReferencesDispositionVerified":false,"derivedAccountDiscoveryComplete":false,"completeMutableOwnershipVerified":false}),
    )
}

pub(super) struct Context<'a> {
    pub policy: &'a RecoveryPolicy,
    pub manifest: &'a Manifest,
    pub base: &'a Path,
    pub checkpoint: &'a crate::VerificationReport,
    pub checkpoint_metadata: &'a Value,
    pub checkpoint_ledger: &'a LedgerBaseline,
    pub last: Option<&'a BlockWitness>,
    pub output: &'a Path,
}

pub(super) fn verify(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    context: Context<'_>,
) -> Result<Value> {
    let Context {
        policy,
        manifest,
        base,
        checkpoint,
        checkpoint_metadata,
        checkpoint_ledger,
        last,
        output,
    } = context;
    let last = last.ok_or("storage semantics requires an executed final block")?;
    need(
        last.resulting_spec == 138 && last.phase == "Complete",
        "final runtime did not complete",
    )?;
    let root = replay::root(ext);
    need(
        format!("{root:#x}") == last.state_root,
        "post-state root differs from final imported witness",
    )?;
    let code = replay::artifact(base, &manifest.final_runtime, 32 * 1024 * 1024)?;
    need(
        replay::code(ext)? == code
            && last.executing_code_blake2 == hex::encode(sp_io::hashing::blake2_256(&code)),
        "final runtime must execute, not only install",
    )?;
    let version = replay::version(executor, ext, &code)?;
    need(
        version.spec_version == 138 && version.transaction_version == 134,
        "final runtime version differs",
    )?;
    let metadata = Vec::<u8>::decode_all(
        &mut replay::call(executor, ext, &code, "Metadata_metadata", &[])?.as_slice(),
    )
    .map_err(|e| e.to_string())?;
    let schema = inventory::MetadataSchema::decode(&metadata)?;
    let metadata_path = output.join("post-state-metadata.scale");
    let mut metadata_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_path)
        .map_err(|e| e.to_string())?;
    metadata_file
        .write_all(&metadata)
        .and_then(|_| metadata_file.sync_all())
        .map_err(|e| e.to_string())?;

    let mut terminal = None;
    let mut terminal_pages = Vec::new();
    for step in &manifest.steps {
        if let Some(reference) = &step.recovery {
            let raw = replay::artifact(base, reference, recovery::MAX_INHERENT_BYTES)?;
            let envelope = RecoveryInherent::decode_remark(&raw)
                .map_err(|e| format!("terminal envelope: {e:?}"))?;
            match envelope.action {
                RecoveryAction::TerminalAudit(audit) => {
                    need(
                        terminal_pages.is_empty() && terminal.replace(audit).is_none(),
                        "mixed/multiple terminal audits in normal activation",
                    )?;
                }
                RecoveryAction::TerminalAuditPage(page) => {
                    need(terminal.is_none(), "mixed terminal audit formats")?;
                    terminal_pages.push(page);
                }
                _ => {}
            }
        }
    }
    if !terminal_pages.is_empty() {
        terminal = Some(
            recovery::invariants::join_terminal_pages(&terminal_pages)
                .map_err(|e| format!("normal terminal pages: {e:?}"))?,
        );
    }
    let terminal = terminal.ok_or("normal activation terminal audit absent")?;
    ext.execute_with(|| recovery::invariants::verify_completed_read_only(policy, &terminal))
        .map_err(|e| format!("post-activation committed financial/retirement check: {e:?}"))?;
    let retired: BTreeSet<[u8; 32]> = policy
        .replacements
        .iter()
        .map(|pair| *AsRef::<[u8; 32]>::as_ref(&pair.old))
        .collect();
    let scan = ext.execute_with(|| scan(&schema, &retired))?;
    let financial = ext.execute_with(|| financial::verify(&schema))?;
    let account_ledger = ext.execute_with(|| ledger::verify(&schema, checkpoint_ledger))?;
    need(
        replay::root(ext) == root,
        "post-state semantic verification mutated executed state",
    )?;
    Ok(
        json!({"schema":"sora-nexus-fresh-post-state-semantics-v1","checkpointStateRoot":checkpoint.state_root,"captureManifestSha256":checkpoint.capture_manifest_sha256,"checkpointMetadataWitness":checkpoint_metadata,"finalStateRoot":format!("{root:#x}"),"finalBlockHash":last.hash,"executingCodeBlake2":last.executing_code_blake2,"policyCommitment":hex::encode(policy.commitment()),"terminalAuditCommitment":hex::encode(policy.terminal_audit_commitment),"finalMetadataSha256":hex::encode(Sha256::digest(&metadata)),"finalMetadataBlake2":schema.metadata_hash,"finalMetadataExecutedAsWasm":true,"finalMetadataNativeExecution":false,"committedTerminalFinancialInvariantsRechecked":true,"retirementAbsenceWitnessesRechecked":terminal.absent.len(),"issuanceEquationsRechecked":terminal.issuance.len(),"collateralEquationsRechecked":terminal.collateral.len(),"zeroClaimsRechecked":terminal.zero_claims.len(),"fullFinancialLiabilityCoverageVerified":false,"retiredKeyDispatchRejectionVerified":false,"readOnlyStateRootMatches":true,"scan":scan,"crossRecordFinancialEquations":financial,"accountLedgerEquations":account_ledger,"completeStorageSemantics":false,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, Registry};
    fn schema() -> inventory::MetadataSchema {
        let mut registry = Registry::new();
        let id = registry
            .register_type(&meta_type::<sp_core::crypto::AccountId32>())
            .id;
        inventory::MetadataSchema {
            registry: registry.into(),
            metadata_hash: "0x00".into(),
            descriptors: vec![inventory::Descriptor {
                trie: "top".into(),
                name: "System.Account".into(),
                prefix: prefix("System.Account"),
                key_types: vec![],
                hashers: vec![],
                value_type: id,
            }],
        }
    }
    #[test]
    fn old_liquid_account_is_rejected_without_bytes_search() {
        let schema = schema();
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(&prefix("System.Account"), &[7; 32]);
            assert!(scan(&schema, &BTreeSet::from([[7; 32]])).is_err());
            assert_eq!(
                scan(&schema, &BTreeSet::from([[8; 32]])).unwrap()["retiredTypedReferences"],
                0
            );
        });
    }
    #[test]
    fn unknown_and_child_storage_remain_explicit_gaps_and_are_fully_enumerated() {
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            sp_io::storage::set(b"unknown", &[7; 32]);
            sp_io::default_child_storage::set(b"child", b"", &[8]);
            sp_io::default_child_storage::set(b"child", b"k", &[9]);
        });
        ext.commit_all().unwrap();
        let result = ext
            .execute_with(|| scan(&schema(), &BTreeSet::from([[7; 32]])))
            .unwrap();
        assert_eq!(result["childTries"], 1);
        assert!(result["entries"].as_u64().unwrap() >= 4);
        assert_eq!(result["allEntriesTyped"], false);
        assert_eq!(result["completeMutableOwnershipVerified"], false);
    }
    fn account_value_schema(name: &str) -> inventory::MetadataSchema {
        let mut result = schema();
        result.descriptors[0].name = name.into();
        result.descriptors[0].prefix = prefix(name);
        result
    }
    #[test]
    fn concrete_live_controls_positions_and_settlement_reject_old_typed_values() {
        for name in [
            "Staking.Payee",
            "Staking.Ledger",
            "Session.KeyOwner",
            "Council.Members",
            "PoolXYK.PoolProviders",
            "DemeterFarmingPlatform.UserInfos",
            "Kensetsu.CDPDepository",
            "ApolloPlatform.UserBorrowingInfo",
            "VestedRewards.PendingClaims",
            "Staking.ErasStakers",
            "Staking.ErasRewardPoints",
            "Staking.ClaimedRewards",
            "Staking.AsyncUnappliedSlashes",
        ] {
            let schema = account_value_schema(name);
            sp_io::TestExternalities::default().execute_with(|| {
                sp_io::storage::set(&prefix(name), &[7; 32]);
                assert_eq!(
                    scan(&schema, &BTreeSet::from([[7; 32]])).unwrap_err(),
                    RETIRED_LIVE,
                    "{name}"
                );
                let result = scan(&schema, &BTreeSet::from([[8; 32]])).unwrap();
                assert_eq!(result["reviewedLiveTypedRows"][name], 1);
                assert_eq!(result["completeMutableOwnershipVerified"], false);
            });
        }
    }
    #[test]
    fn immutable_proof_history_is_not_waived_as_economic_history() {
        let schema = account_value_schema("Historical.HistoricalSessions");
        sp_io::TestExternalities::default().execute_with(|| {
            sp_io::storage::set(&prefix("Historical.HistoricalSessions"), &[7; 32]);
            let result = scan(&schema, &BTreeSet::from([[7; 32]])).unwrap();
            assert_eq!(result["retiredTypedReferences"], 1);
            assert_eq!(result["historicalReferencesDispositionVerified"], false);
            assert_eq!(result["reviewedLiveTypedRows"], json!({}));
        });
        assert_eq!(
            live_family("Staking.ErasStakersClipped"),
            Some("stakingSettlement")
        );
        assert_eq!(live_family("Staking.HistoricalUnknown"), None);
    }
    fn opaque_schema(name: &str, holder_key: bool) -> inventory::MetadataSchema {
        let mut registry = Registry::new();
        let account = registry
            .register_type(&meta_type::<sp_core::crypto::AccountId32>())
            .id;
        let number = registry.register_type(&meta_type::<u32>()).id;
        let numbers = registry.register_type(&meta_type::<Vec<u32>>()).id;
        inventory::MetadataSchema {
            registry: registry.into(),
            metadata_hash: "0x00".into(),
            descriptors: vec![inventory::Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix: prefix(name),
                key_types: vec![if holder_key { account } else { number }, number],
                hashers: vec![inventory::Hasher::Blake2_256; 2],
                value_type: if holder_key { numbers } else { account },
            }],
        }
    }
    #[test]
    fn soratopia_check_in_rejects_retired_owner_from_actual_typed_map_key() {
        let mut registry = Registry::new();
        let account = registry
            .register_type(&meta_type::<sp_core::crypto::AccountId32>())
            .id;
        let block = registry.register_type(&meta_type::<u32>()).id;
        let schema = inventory::MetadataSchema {
            registry: registry.into(),
            metadata_hash: "0x00".into(),
            descriptors: vec![inventory::Descriptor {
                trie: "top".into(),
                name: "Soratopia.LastCheckIn".into(),
                prefix: prefix("Soratopia.LastCheckIn"),
                key_types: vec![account],
                hashers: vec![inventory::Hasher::Blake2_128Concat],
                value_type: block,
            }],
        };
        let key = [
            prefix("Soratopia.LastCheckIn"),
            inventory::Hasher::Blake2_128Concat.apply(&[7; 32]),
        ]
        .concat();
        sp_io::TestExternalities::default().execute_with(|| {
            // The descriptor alone does not assert that a checkpoint row exists.
            assert!(scan(&schema, &BTreeSet::from([[7; 32]])).is_ok());
            sp_io::storage::set(&key, &28_191_816u32.to_le_bytes());
            assert_eq!(
                scan(&schema, &BTreeSet::from([[7; 32]])).unwrap_err(),
                RETIRED_LIVE
            );
            let result = scan(&schema, &BTreeSet::from([[8; 32]])).unwrap();
            assert_eq!(result["reviewedLiveTypedRows"]["Soratopia.LastCheckIn"], 1);
            // A malformed value must not hide the typed retired map key.
            sp_io::storage::set(&key, &[0]);
            assert_eq!(
                scan(&schema, &BTreeSet::from([[7; 32]])).unwrap_err(),
                RETIRED_LIVE
            );
        });
    }
    #[test]
    fn opaque_key_cannot_hide_typed_permission_owner_value() {
        let schema = opaque_schema("Permissions.Owners", false);
        let key = [prefix("Permissions.Owners"), vec![1; 64]].concat();
        sp_io::TestExternalities::default().execute_with(|| {
            sp_io::storage::set(&key, &[7; 32]);
            assert_eq!(
                scan(&schema, &BTreeSet::from([[7; 32]])).unwrap_err(),
                RETIRED_LIVE
            );
            let result = scan(&schema, &BTreeSet::from([[8; 32]])).unwrap();
            assert_eq!(
                result["reviewedLivePartiallyTypedRows"]["Permissions.Owners"],
                1
            );
            assert_eq!(result["allEntriesTyped"], false);
        });
    }
    #[test]
    fn exact_hashed_permission_holder_is_checked_without_inverting_scope() {
        let mut schema = opaque_schema("Permissions.Permissions", true);
        let key = [
            prefix("Permissions.Permissions"),
            sp_io::hashing::blake2_256(&[7; 32]).to_vec(),
            vec![2; 32],
        ]
        .concat();
        assert_eq!(
            typed_row(
                &schema,
                &schema.descriptors[0],
                &key,
                &[0],
                &BTreeSet::from([[7; 32]])
            )
            .err()
            .unwrap(),
            RETIRED_LIVE
        );
        let row = typed_row(
            &schema,
            &schema.descriptors[0],
            &key,
            &[0],
            &BTreeSet::from([[8; 32]]),
        )
        .unwrap();
        assert_eq!(row.retired, 0);
        assert!(!row.issues.is_empty()); // General scope/key completeness was not inferred.
        schema.descriptors[0].hashers[0] = inventory::Hasher::Twox256;
        assert!(typed_row(
            &schema,
            &schema.descriptors[0],
            &key,
            &[0],
            &BTreeSet::new()
        )
        .is_err());
    }
    #[test]
    fn live_descriptor_requires_exact_top_prefix_and_unique_name() {
        for kind in 0..3 {
            let mut schema = schema();
            match kind {
                0 => schema.descriptors[0].prefix[0] ^= 1,
                1 => schema.descriptors[0].trie = "child".into(),
                _ => schema.descriptors.push(schema.descriptors[0].clone()),
            }
            assert!(validate_live_descriptors(&schema).is_err());
        }
    }
    #[test]
    fn retired_map_key_is_checked_even_when_value_codec_fails() {
        let mut schema = schema();
        let id = schema.descriptors[0].value_type;
        schema.descriptors[0].key_types = vec![id];
        schema.descriptors[0].hashers = vec![inventory::Hasher::Twox64Concat];
        let key = [
            prefix("System.Account"),
            inventory::Hasher::Twox64Concat.apply(&[7; 32]),
        ]
        .concat();
        assert_eq!(
            typed_row(
                &schema,
                &schema.descriptors[0],
                &key,
                &[0],
                &BTreeSet::from([[7; 32]])
            )
            .err()
            .unwrap(),
            RETIRED_LIVE
        );
    }
    #[test]
    fn matching_hash_bytes_are_not_treated_as_an_account() {
        let mut registry = Registry::new();
        let id = registry.register_type(&meta_type::<sp_core::H256>()).id;
        let schema = inventory::MetadataSchema {
            registry: registry.into(),
            metadata_hash: "0x00".into(),
            descriptors: vec![inventory::Descriptor {
                trie: "top".into(),
                name: "Unknown.Commitment".into(),
                prefix: prefix("Unknown.Commitment"),
                key_types: vec![],
                hashers: vec![],
                value_type: id,
            }],
        };
        let row = typed_row(
            &schema,
            &schema.descriptors[0],
            &prefix("Unknown.Commitment"),
            &[7; 32],
            &BTreeSet::from([[7; 32]]),
        )
        .unwrap();
        assert_eq!(row.retired, 0);
        assert!(row.family.is_none());
    }
    fn historical_record() -> HistoricalSlashRecord {
        HistoricalSlashRecord {
            offence_era: 7809,
            fraction: sp_runtime::Perbill::from_percent(5),
            slash: LegacySlashRecord {
                validator: sp_core::crypto::AccountId32::from([1; 32]),
                own: 10,
                others: vec![(sp_core::crypto::AccountId32::from([2; 32]), 20)],
                reporters: vec![sp_core::crypto::AccountId32::from([3; 32])],
                payout: 2,
            },
        }
    }
    fn historical_key() -> Vec<u8> {
        [
            prefix("Staking.HistoricalSlashes"),
            inventory::Hasher::Twox64Concat.apply(&7810u32.to_le_bytes()),
            inventory::Hasher::Twox64Concat.apply(&4u64.to_le_bytes()),
        ]
        .concat()
    }
    #[test]
    fn metadata_omitted_historical_slash_liabilities_reject_every_old_recipient() {
        use codec::Encode;
        let value = historical_record().encode();
        let key = historical_key();
        for old in [1, 2, 3] {
            assert_eq!(
                historical_slash_row(&key, &value, &BTreeSet::from([[old; 32]]))
                    .err()
                    .unwrap(),
                RETIRED_LIVE
            );
        }
        assert!(
            historical_slash_row(&key, &value, &BTreeSet::from([[4; 32]]))
                .unwrap()
                .issues
                .is_empty()
        );
        let mut altered = key.clone();
        altered[32] ^= 1;
        assert!(historical_slash_row(&altered, &value, &BTreeSet::new()).is_err());
        assert!(historical_slash_row(&key, &value[..value.len() - 1], &BTreeSet::new()).is_err());
        sp_io::TestExternalities::default().execute_with(|| {
            sp_io::storage::set(&key, &value);
            assert_eq!(
                scan(&schema(), &BTreeSet::from([[2; 32]])).unwrap_err(),
                RETIRED_LIVE
            );
            assert_eq!(
                scan(&schema(), &BTreeSet::from([[4; 32]])).unwrap()["reviewedLiveTypedRows"]
                    ["Staking.HistoricalSlashes"],
                1
            );
        });
    }
    #[cfg(feature = "bridge-fixture")]
    #[test]
    fn explicit_historical_slash_codec_matches_compiled_async_staking_type() {
        use codec::Encode;
        let bytes = historical_record().encode();
        let native=pallet_staking::legacy::HistoricalSlash::<sp_core::crypto::AccountId32,u128>::decode_all(&mut bytes.as_slice()).unwrap();
        assert_eq!(native.encode(), bytes);
        assert_eq!(
            native.slash.others[0].0,
            sp_core::crypto::AccountId32::from([2; 32])
        );
        assert_eq!(
            native.slash.reporters,
            vec![sp_core::crypto::AccountId32::from([3; 32])]
        );
    }
}
