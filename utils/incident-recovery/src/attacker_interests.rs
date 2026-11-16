//! Read-only economic-interest triage from the complete in-memory native watch.
//! A typed identity reference is not an ownership or settlement certificate.

use super::{attacker_registry, hx, EconomicPlan};
use crate::{inventory::InventoryReport, typed::IdentityReference, Result};
use codec::Encode;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const MAX_REFERENCE_ROWS: usize = 4096;
const MAX_REFERENCES: usize = 16384;

/// The catalog organizes review work; unknown stores and child tries remain in
/// `other_storage`. Its names never authorize a burn or waive a native hit.
struct Family {
    name: &'static str,
    stores: &'static [&'static str],
    required_positive_adapter: &'static str,
}

const FAMILIES: &[Family] = &[
    Family {
        name: "balances_and_account_lifecycle",
        stores: &[
            "System.Account", "Tokens.Accounts", "Balances.Locks", "Balances.Holds",
            "Balances.Reserves", "Tokens.Locks", "Tokens.Reserves",
        ],
        required_positive_adapter: "Attribute owned free/reserved/frozen assets, preserve third-party backing and pallet reference ownership, and couple any burn to exact issuance reduction; scalar zeroing alone is insufficient.",
    },
    Family {
        name: "lp_shares_and_locks",
        stores: &[
            "PoolXYK.PoolProviders", "PoolXYK.AccountPools", "PoolXYK.Properties",
            "PoolXYK.Reserves", "PoolXYK.TotalIssuances", "CeresLiquidityLocker.LockerData",
        ],
        required_positive_adapter: "Establish the owner and encumbrances of each LP share, execute exact pool withdrawal/rounding and cache/index accounting, reconcile farming once, and burn only withdrawn underlying assets without redistributing other LP interests.",
    },
    Family {
        name: "farming_and_lazy_rewards",
        stores: &[
            "Farming.PoolFarmers", "Farming.Pools", "DemeterFarmingPlatform.UserInfos",
            "DemeterFarmingPlatform.Pools", "DemeterFarmingPlatform.TokenInfos",
            "PswapDistribution.ShareholderAccounts", "PswapDistribution.TotalShares",
        ],
        required_positive_adapter: "Reconcile exact checkpoint lazy reward time/weights, pool aggregates and reserve attribution together with the single underlying LP/staked interest; a cached farmer entry is not a second token balance.",
    },
    Family {
        name: "rewards_and_claims",
        stores: &[
            "Rewards.ValOwners", "Rewards.PswapFarmOwners", "Rewards.PswapWaifuOwners",
            "Rewards.UmiNftReceivers", "Rewards.EthAddresses", "Rewards.TotalValRewards",
            "Rewards.TotalClaimableVal", "Rewards.ReservesAcc", "VestedRewards.Rewards",
            "VestedRewards.TotalRewards", "VestedRewards.CrowdloanUserInfos",
            "VestedRewards.CrowdloanInfos", "VestedRewards.ClaimSchedules",
            "CeresStaking.Stakers", "CeresStaking.TotalDeposited", "CeresStaking.RewardsRemaining",
            "IrohaMigration.Balances", "Referrals.ReferrerBalances", "CeresTokenLocker.TokenLockerData",
        ],
        required_positive_adapter: "Distinguish unissued entitlements from already issued reserve-backed claims, preserve innocent aggregate claims and provider/consumer references, and extinguish only attributable claims/backing. TS/SS and external claim history require their separate ledger evidence.",
    },
    Family {
        name: "debt_and_shared_collateral",
        stores: &[
            "Kensetsu.CDPDepository", "Kensetsu.CdpOwnerIndex", "Kensetsu.CollateralInfos",
            "Kensetsu.StablecoinInfos", "ApolloPlatform.UserLendingInfo",
            "ApolloPlatform.UserBorrowingInfo", "ApolloPlatform.UserTotalCollateral",
            "ApolloPlatform.PoolData", "ApolloPlatform.LendingRewards", "ApolloPlatform.BorrowingRewards",
        ],
        required_positive_adapter: "Settle exact debt and aggregate pool liabilities before residual collateral burns; otherwise enforce non-withdrawable recovery custody. Preserve lender/counterparty ownership and pre-checkpoint debt/fees without manufacturing bad debt.",
    },
    Family {
        name: "staking_and_shared_exposure",
        stores: &[
            "Staking.Bonded", "Staking.Ledger", "Staking.Payee", "Staking.Nominators",
            "Staking.Validators", "Staking.ErasStakers", "Staking.ErasStakersClipped",
            "Staking.ErasStakersPaged", "Staking.ErasStakersOverview", "Staking.ErasRewardPoints",
            "Staking.UnappliedSlashes", "Staking.SlashingSpans", "Staking.SpanSlash",
        ],
        required_positive_adapter: "Separate attacker-owned stake/rewards from innocent nominators and historical shared liabilities; preserve exposure and offence evidence and apply exact lock/reference/issuance accounting.",
    },
    Family {
        name: "bridge_requests_and_refund_entitlements",
        stores: &[
            "EthBridge.Requests", "EthBridge.AccountRequests", "EthBridge.RequestStatuses",
            "EthBridge.RequestApprovals", "EthBridge.RequestApprovers", "EthBridge.BridgeAccount",
            "BridgeProxy.Transactions", "BridgeProxy.Senders", "BridgeProxy.LockedAssets",
            "BridgeMultisig.Multisigs", "BridgeMultisig.Calls", "BridgeMultisig.Accounts",
        ],
        required_positive_adapter: "Establish request ownership and independent non-redemption/authorization evidence before any refund; extinguish attacker entitlements without exposing funds, preserve pooled backing and replay/status records, and never infer a refund from BridgeMultisig nominal deposits.",
    },
    Family {
        name: "issuance_aggregates",
        stores: &[
            "Balances.TotalIssuance", "Balances.InactiveIssuance", "Tokens.TotalIssuance",
        ],
        required_positive_adapter: "Join every changed asset's exact checkpoint issuance, authenticated mints, approved burns and final state; inventory counts alone prove neither conservation nor a permissible economic mutation.",
    },
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreAssessment {
    pub storage: String,
    pub captured_rows: usize,
    pub decoded_rows: usize,
    pub typed_reference_rows: usize,
    /// A zero row count means no records were observed; it does not claim the
    /// deployed metadata defined the catalog name or the store had a default.
    pub captured_prefix_present: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyAssessment {
    pub family: &'static str,
    pub stores: Vec<StoreAssessment>,
    pub captured_rows: usize,
    pub typed_reference_rows: usize,
    pub positive_ownership_established: bool,
    pub required_positive_adapter: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InterestReference {
    pub family: &'static str,
    pub trie: String,
    pub storage: String,
    pub key: String,
    pub value_hash: String,
    pub references: Vec<IdentityReference>,
    pub disposition: &'static str,
}

/// Constructed only from an intact same-process `InventoryReport`. This type
/// deliberately has no Deserialize implementation or release-gate authority.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackerInterestAssessment {
    pub schema: &'static str,
    pub metadata_hash: String,
    pub attacker_registry: Vec<String>,
    pub attacker_registry_sha256: String,
    pub native_inventory: crate::inventory::NativeWatchSummary,
    pub families: Vec<FamilyAssessment>,
    pub references: Vec<InterestReference>,
    pub total_typed_reference_rows: usize,
    pub no_observed_direct_typed_identity_references: bool,
    pub claimed_economic_operations: usize,
    pub economic_plan_consistent_with_observed_absence: bool,
    pub beneficial_or_derived_ownership_established: bool,
    pub external_network_assets_inspected: bool,
    pub external_claim_entitlements_verified: bool,
    pub issuance_disposition_verified: bool,
    pub attacker_assets_debt_and_issuance_verified: bool,
    pub production_ready: bool,
    pub limitation: &'static str,
}

pub fn assess_attacker_interests(
    plan: &EconomicPlan,
    inventory: &InventoryReport,
) -> Result<AttackerInterestAssessment> {
    let registry = attacker_registry(plan)?;
    let native_inventory = inventory.require_native_watch(&registry)?;
    let canonical: BTreeSet<_> = registry.iter().map(|identity| hx(identity)).collect();
    let catalog: BTreeMap<_, _> = FAMILIES
        .iter()
        .flat_map(|family| family.stores.iter().map(move |name| (*name, family.name)))
        .collect();
    if catalog.len()
        != FAMILIES
            .iter()
            .map(|family| family.stores.len())
            .sum::<usize>()
    {
        return Err("economic-interest catalog duplicates a storage family".into());
    }
    let mut references = Vec::new();
    let mut reference_count = 0usize;
    let mut by_store = BTreeMap::<(&str, &str), usize>::new();
    for record in &inventory.watched_references {
        let matching: Vec<_> = record
            .references
            .iter()
            .filter(|reference| canonical.contains(&reference.identity))
            .cloned()
            .collect();
        if matching.is_empty() {
            continue;
        }
        reference_count = reference_count
            .checked_add(matching.len())
            .ok_or("economic-interest reference count overflow")?;
        if references.len() >= MAX_REFERENCE_ROWS || reference_count > MAX_REFERENCES {
            return Err("economic-interest assessment exceeds bounded reference output; inspect the complete native inventory without treating this truncated assessment as evidence".into());
        }
        *by_store.entry((&record.trie, &record.storage)).or_default() += 1;
        let family = if record.trie == "top" {
            catalog
                .get(record.storage.as_str())
                .copied()
                .unwrap_or("other_storage")
        } else {
            "other_storage"
        };
        references.push(InterestReference {
            family,
            trie: record.trie.clone(),
            storage: record.storage.clone(),
            key: record.key.clone(),
            value_hash: record.value_hash.clone(),
            references: matching,
            disposition: "Typed reference only; exact ownership/backing adapter required before economic disposition. No value or issuance was changed.",
        });
    }
    let mut captured = BTreeMap::new();
    for prefix in &inventory.prefixes {
        if captured
            .insert((prefix.trie.as_str(), prefix.storage.as_str()), prefix)
            .is_some()
        {
            return Err("native inventory contains duplicate storage prefix reports".into());
        }
    }
    let mut families = Vec::new();
    for family in FAMILIES {
        let mut stores = Vec::new();
        let mut captured_rows = 0usize;
        let mut typed_reference_rows = 0usize;
        for name in family.stores {
            let prefix = captured.get(&("top", *name));
            let rows = prefix.map_or(0, |prefix| prefix.entries);
            let hits = by_store.get(&("top", *name)).copied().unwrap_or(0);
            captured_rows = captured_rows
                .checked_add(rows)
                .ok_or("economic-interest row count overflow")?;
            typed_reference_rows = typed_reference_rows
                .checked_add(hits)
                .ok_or("economic-interest hit count overflow")?;
            stores.push(StoreAssessment {
                storage: (*name).into(),
                captured_rows: rows,
                decoded_rows: prefix.map_or(0, |prefix| prefix.decoded_entries),
                typed_reference_rows: hits,
                captured_prefix_present: prefix.is_some(),
            });
        }
        families.push(FamilyAssessment {
            family: family.name,
            stores,
            captured_rows,
            typed_reference_rows,
            positive_ownership_established: false,
            required_positive_adapter: family.required_positive_adapter,
        });
    }
    // Retain all uncatalogued and child storage, even when no named family
    // contains the positive reference. This is not an allowlist of relevant rows.
    let other_stores: Vec<_> = inventory
        .prefixes
        .iter()
        .filter(|prefix| prefix.trie != "top" || !catalog.contains_key(prefix.storage.as_str()))
        .map(|prefix| StoreAssessment {
            storage: format!("{}/{}", prefix.trie, prefix.storage),
            captured_rows: prefix.entries,
            decoded_rows: prefix.decoded_entries,
            typed_reference_rows: by_store
                .get(&(prefix.trie.as_str(), prefix.storage.as_str()))
                .copied()
                .unwrap_or(0),
            captured_prefix_present: true,
        })
        .collect();
    families.push(FamilyAssessment {
        family: "other_storage",
        captured_rows: other_stores.iter().map(|store| store.captured_rows).sum(),
        typed_reference_rows: other_stores.iter().map(|store| store.typed_reference_rows).sum(),
        stores: other_stores,
        positive_ownership_established: false,
        required_positive_adapter: "Resolve every remaining typed control, ownership, authorization or historical reference by its exact source semantics; unlisted or child storage never becomes an exception to coverage.",
    });
    let total_typed_reference_rows = references.len();
    let claimed_economic_operations = plan
        .direct_burns
        .len()
        .checked_add(plan.extinguish_unissued.len())
        .and_then(|count| count.checked_add(plan.quarantine.len()))
        .ok_or("economic-operation count overflow")?;
    let registry_hash: [u8; 32] = Sha256::digest(
        (
            b"sora-nexus/attacker-typed-registry/v1".as_slice(),
            &registry,
        )
            .encode(),
    )
    .into();
    Ok(AttackerInterestAssessment {
        schema: "sora-nexus-attacker-interest-assessment-v1",
        metadata_hash: inventory.metadata_hash.clone(),
        attacker_registry: canonical.into_iter().collect(),
        attacker_registry_sha256: hx(&registry_hash),
        native_inventory,
        families,
        references,
        total_typed_reference_rows,
        no_observed_direct_typed_identity_references: total_typed_reference_rows == 0,
        claimed_economic_operations,
        economic_plan_consistent_with_observed_absence: total_typed_reference_rows != 0 || claimed_economic_operations == 0,
        beneficial_or_derived_ownership_established: false,
        external_network_assets_inspected: false,
        external_claim_entitlements_verified: false,
        issuance_disposition_verified: false,
        attacker_assets_debt_and_issuance_verified: false,
        production_ready: false,
        limitation: "Fresh complete native typed traversal of the exact supplied identities only. Family counts do not establish ownership, settlement, aggregate conservation, derived-account completeness, external-network assets or burn-ledger claims. Opaque retained history remains opaque. Positive references and claimed positions require exact adapters; neither absence nor this report clears the attacker accounting release gate.",
    })
}
