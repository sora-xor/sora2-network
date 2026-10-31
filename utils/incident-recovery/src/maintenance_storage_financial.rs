//! Independent stored-position equations over the executing final runtime's trie.
//! No imported JSON assertion or accounting plan can supply these totals.
use super::{need, next, prefix, read};
use crate::{
    inventory::{Descriptor, Hasher, MetadataSchema},
    typed, Result,
};
use codec::DecodeAll;
use scale_info::{PortableRegistry, TypeDef, TypeDefPrimitive};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

type Identity = [u8; 32];
type AssetPair = (Identity, Identity);
const MAX_ROWS: usize = 2_000_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;
const MAX_INDEX_IDS: usize = 4096;

fn descriptor<'a>(schema: &'a MetadataSchema, name: &str, count: usize) -> Result<&'a Descriptor> {
    let rows: Vec<_> = schema
        .descriptors
        .iter()
        .filter(|d| d.name == name)
        .collect();
    need(
        rows.len() == 1,
        "financial equation metadata descriptor absent/duplicate",
    )?;
    let d = rows[0];
    need(
        d.trie == "top"
            && d.prefix == prefix(name)
            && d.key_types.len() == count
            && d.hashers == vec![Hasher::Identity; count],
        "financial equation storage identity/hasher differs",
    )?;
    Ok(d)
}
fn primitive_u128(registry: &PortableRegistry, id: u32) -> bool {
    matches!(
        registry.resolve(id).map(|t| &t.type_def),
        Some(TypeDef::Primitive(TypeDefPrimitive::U128))
    )
}
fn asset_type(registry: &PortableRegistry, id: u32) -> bool {
    registry.resolve(id).is_some_and(|t| {
        t.path.segments.last().is_some_and(|n| n == "AssetId32")
            && t.path.segments.iter().any(|n| n == "common")
    })
}
fn fields(
    registry: &PortableRegistry,
    id: u32,
    expected: &[&str],
) -> Result<BTreeMap<String, u32>> {
    let Some(TypeDef::Composite(c)) = registry.resolve(id).map(|t| &t.type_def) else {
        return Err("financial equation expected named struct".into());
    };
    need(
        c.fields
            .iter()
            .map(|f| f.name.as_deref())
            .eq(expected.iter().map(|n| Some(*n))),
        "financial equation named fields differ",
    )?;
    Ok(c.fields
        .iter()
        .map(|f| (f.name.clone().expect("checked named field"), f.ty.id))
        .collect())
}
fn sequence_u128(registry: &PortableRegistry, id: u32, depth: usize) -> bool {
    if depth > 4 {
        return false;
    }
    match registry.resolve(id).map(|t| &t.type_def) {
        Some(TypeDef::Sequence(s)) => primitive_u128(registry, s.type_param.id),
        Some(TypeDef::Composite(c)) if c.fields.len() == 1 && c.fields[0].name.is_none() => {
            sequence_u128(registry, c.fields[0].ty.id, depth + 1)
        }
        _ => false,
    }
}
fn array32(raw: &[u8]) -> Result<Identity> {
    raw.try_into()
        .map_err(|_| "financial identity width differs".into())
}
fn hex32(value: &Value) -> Result<Identity> {
    let text = value
        .as_str()
        .ok_or("financial typed identity is not a string")?;
    let bytes = hex::decode(text.strip_prefix("0x").ok_or("financial identity prefix")?)
        .map_err(|e| e.to_string())?;
    need(
        text == format!("0x{}", hex::encode(&bytes)),
        "noncanonical financial identity",
    )?;
    array32(&bytes)
}
fn amount(raw: &[u8]) -> Result<u128> {
    Ok(u128::from_le_bytes(
        raw.try_into()
            .map_err(|_| "financial amount width differs")?,
    ))
}
fn checked_add<K: Ord>(map: &mut BTreeMap<K, u128>, key: K, value: u128) -> Result<()> {
    let entry = map.entry(key).or_default();
    *entry = entry
        .checked_add(value)
        .ok_or("financial aggregate overflow")?;
    Ok(())
}
#[derive(Default)]
struct Budget {
    rows: usize,
    bytes: usize,
}
fn rows(
    d: &Descriptor,
    budget: &mut Budget,
    mut consume: impl FnMut(&[u8], &[u8]) -> Result<()>,
) -> Result<usize> {
    let mut key = if sp_io::storage::exists(&d.prefix) {
        Some(d.prefix.clone())
    } else {
        next(None, &d.prefix)
    };
    let mut count = 0usize;
    while let Some(current) = key.filter(|k| k.starts_with(&d.prefix)) {
        let value = read(None, &current)?;
        budget.rows = budget
            .rows
            .checked_add(1)
            .ok_or("financial row count overflow")?;
        budget.bytes = budget
            .bytes
            .checked_add(current.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or("financial byte count overflow")?;
        need(
            budget.rows <= MAX_ROWS && budget.bytes <= MAX_BYTES && current.len() <= 4096,
            "financial prefix scan bound",
        )?;
        consume(&current[d.prefix.len()..], &value)?;
        count += 1;
        key = next(None, &current);
        need(
            key.as_ref().is_none_or(|new| new > &current),
            "nonmonotonic financial prefix scan",
        )?;
    }
    Ok(count)
}
fn reconcile_lp(
    shares: &BTreeMap<Identity, u128>,
    issuance: &BTreeMap<Identity, u128>,
) -> Result<()> {
    for (pool, total) in shares {
        need(
            issuance.get(pool) == Some(total),
            "LP provider shares differ from issued pool shares",
        )?;
    }
    for (pool, total) in issuance {
        need(
            shares.get(pool).copied().unwrap_or(0) == *total,
            "LP issuance has unmatched provider shares",
        )?;
    }
    Ok(())
}
fn liquidity(schema: &MetadataSchema, budget: &mut Budget) -> Result<Value> {
    let providers = descriptor(schema, "PoolXYK.PoolProviders", 2)?;
    let issued = descriptor(schema, "PoolXYK.TotalIssuances", 1)?;
    need(
        providers
            .key_types
            .iter()
            .chain(&issued.key_types)
            .all(|id| typed::account_type(&schema.registry, *id))
            && primitive_u128(&schema.registry, providers.value_type)
            && primitive_u128(&schema.registry, issued.value_type),
        "LP equation account/amount metadata differs",
    )?;
    let mut shares = BTreeMap::new();
    let mut issuance = BTreeMap::new();
    let provider_rows = rows(providers, budget, |key, value| {
        need(key.len() == 64, "LP provider double account key differs")?;
        checked_add(&mut shares, array32(&key[..32])?, amount(value)?)
    })?;
    let issuance_rows = rows(issued, budget, |key, value| {
        need(
            issuance.insert(array32(key)?, amount(value)?).is_none(),
            "duplicate LP issuance",
        )?;
        Ok(())
    })?;
    reconcile_lp(&shares, &issuance)?;
    Ok(
        json!({"providerRows":provider_rows,"issuanceRows":issuance_rows,"poolsWithProviders":shares.len(),"perPoolProviderSumEqualsIssuance":true,"underlyingAssetReservesVerified":false,"checkpointOwnershipConservationVerified":false}),
    )
}
#[derive(Default)]
struct CdpTotals {
    owners: BTreeMap<Identity, BTreeSet<u128>>,
    collateral: BTreeMap<AssetPair, u128>,
    debt: BTreeMap<AssetPair, u128>,
}
impl CdpTotals {
    fn insert(
        &mut self,
        id: u128,
        owner: Identity,
        pair: AssetPair,
        collateral: u128,
        debt: u128,
    ) -> Result<()> {
        need(
            self.owners.entry(owner).or_default().insert(id),
            "duplicate CDP owner/id",
        )?;
        checked_add(&mut self.collateral, pair, collateral)?;
        checked_add(&mut self.debt, pair, debt)
    }
    fn reconcile(
        &self,
        owners: &BTreeMap<Identity, BTreeSet<u128>>,
        collateral: &BTreeMap<AssetPair, (u128, u128)>,
    ) -> Result<()> {
        // Empty index rows are tolerated only as empty; duplicated/extra IDs and
        // absent or wrong owners are never hidden by aggregate totals.
        for (owner, ids) in &self.owners {
            need(
                owners.get(owner) == Some(ids),
                "CDP owner index differs from positions",
            )?;
        }
        for (owner, ids) in owners {
            need(
                self.owners.get(owner).cloned().unwrap_or_default() == *ids,
                "CDP owner index contains absent/wrong-owner position",
            )?;
        }
        for pair in self.collateral.keys() {
            need(
                collateral.contains_key(pair),
                "CDP collateral/debt aggregate record absent",
            )?;
        }
        for (pair, (locked, supply)) in collateral {
            need(
                self.collateral.get(pair).copied().unwrap_or(0) == *locked,
                "CDP collateral sum differs from collateral aggregate",
            )?;
            need(
                self.debt.get(pair).copied().unwrap_or(0) == *supply,
                "stored CDP debt sum differs from collateral stablecoin supply",
            )?;
        }
        Ok(())
    }
}
fn kensetsu(schema: &MetadataSchema, budget: &mut Budget) -> Result<Value> {
    let cdps = descriptor(schema, "Kensetsu.CDPDepository", 1)?;
    let owners = descriptor(schema, "Kensetsu.CdpOwnerIndex", 1)?;
    let collateral = descriptor(schema, "Kensetsu.CollateralInfos", 1)?;
    need(
        primitive_u128(&schema.registry, cdps.key_types[0])
            && typed::account_type(&schema.registry, owners.key_types[0])
            && sequence_u128(&schema.registry, owners.value_type, 0),
        "CDP id/owner-index metadata differs",
    )?;
    let cdp_fields = fields(
        &schema.registry,
        cdps.value_type,
        &[
            "owner",
            "collateral_asset_id",
            "collateral_amount",
            "stablecoin_asset_id",
            "debt",
            "interest_coefficient",
        ],
    )?;
    need(
        typed::account_type(&schema.registry, cdp_fields["owner"])
            && asset_type(&schema.registry, cdp_fields["collateral_asset_id"])
            && asset_type(&schema.registry, cdp_fields["stablecoin_asset_id"])
            && primitive_u128(&schema.registry, cdp_fields["collateral_amount"])
            && primitive_u128(&schema.registry, cdp_fields["debt"]),
        "CDP owner/asset/amount metadata differs",
    )?;
    let pair_fields = fields(
        &schema.registry,
        collateral.key_types[0],
        &["collateral_asset_id", "stablecoin_asset_id"],
    )?;
    need(
        pair_fields
            .values()
            .all(|id| asset_type(&schema.registry, *id)),
        "CDP collateral pair metadata differs",
    )?;
    let aggregate_fields = fields(
        &schema.registry,
        collateral.value_type,
        &[
            "risk_parameters",
            "total_collateral",
            "stablecoin_supply",
            "last_fee_update_time",
            "interest_coefficient",
        ],
    )?;
    need(
        primitive_u128(&schema.registry, aggregate_fields["total_collateral"])
            && primitive_u128(&schema.registry, aggregate_fields["stablecoin_supply"]),
        "CDP aggregate amount metadata differs",
    )?;
    let mut totals = CdpTotals::default();
    let mut owner_rows = BTreeMap::new();
    let mut aggregates = BTreeMap::new();
    let cdp_count = rows(cdps, budget, |key, raw| {
        let id = amount(key)?;
        let decoded = typed::decoded_json(raw, cdps.value_type, &schema.registry)?;
        let owner = hex32(&decoded["owner"])?;
        let pair = (
            hex32(&decoded["collateral_asset_id"])?,
            hex32(&decoded["stablecoin_asset_id"])?,
        );
        let locked = typed::u128_field(
            raw,
            cdps.value_type,
            &schema.registry,
            &["collateral_amount"],
        )?
        .0;
        let debt = typed::u128_field(raw, cdps.value_type, &schema.registry, &["debt"])?.0;
        totals.insert(id, owner, pair, locked, debt)
    })?;
    let index_count = rows(owners, budget, |key, raw| {
        let owner = array32(key)?;
        // Both metadata and SCALE must agree; bounded wrapper encodes the same sequence.
        typed::decode_exact(raw, owners.value_type, &schema.registry)?;
        let mut input = raw;
        let ids = Vec::<u128>::decode_all(&mut input).map_err(|e| e.to_string())?;
        need(
            ids.len() <= MAX_INDEX_IDS,
            "CDP owner-index inspection bound",
        )?;
        let unique: BTreeSet<_> = ids.iter().copied().collect();
        need(unique.len() == ids.len(), "duplicate CDP id in owner index")?;
        need(
            owner_rows.insert(owner, unique).is_none(),
            "duplicate CDP owner index",
        )?;
        Ok(())
    })?;
    let aggregate_count = rows(collateral, budget, |key, raw| {
        let decoded = typed::decoded_json(key, collateral.key_types[0], &schema.registry)?;
        let pair = (
            hex32(&decoded["collateral_asset_id"])?,
            hex32(&decoded["stablecoin_asset_id"])?,
        );
        let locked = typed::u128_field(
            raw,
            collateral.value_type,
            &schema.registry,
            &["total_collateral"],
        )?
        .0;
        let debt = typed::u128_field(
            raw,
            collateral.value_type,
            &schema.registry,
            &["stablecoin_supply"],
        )?
        .0;
        need(
            aggregates.insert(pair, (locked, debt)).is_none(),
            "duplicate CDP collateral aggregate",
        )?;
        Ok(())
    })?;
    totals.reconcile(&owner_rows, &aggregates)?;
    Ok(
        json!({"positionRows":cdp_count,"ownerIndexRows":index_count,"collateralAggregateRows":aggregate_count,"ownerIndexBijectionVerified":true,"storedCollateralAndDebtSumsVerified":true,"unaccruedFeeValuationVerified":false,"depositoryBackingVerified":false,"protocolBadDebtCoverageVerified":false,"checkpointLiabilityConservationVerified":false}),
    )
}
pub(super) fn verify(schema: &MetadataSchema) -> Result<Value> {
    let mut budget = Budget::default();
    let liquidity = liquidity(schema, &mut budget)?;
    let kensetsu = kensetsu(schema, &mut budget)?;
    Ok(
        json!({"schema":"sora-nexus-read-only-cross-record-financial-equations-v1","liquidity":liquidity,"kensetsu":kensetsu,"rows":budget.rows,"bytes":budget.bytes,"completeFinancialCoverageVerified":false,"completeStorageSemantics":false,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lp_shares_require_exact_issuance_and_allow_only_zero_empty_pools() {
        let shares = BTreeMap::from([([1; 32], 30)]);
        let mut issued = BTreeMap::from([([1; 32], 30), ([2; 32], 0)]);
        reconcile_lp(&shares, &issued).unwrap();
        issued.insert([2; 32], 1);
        assert!(reconcile_lp(&shares, &issued).is_err());
        issued.remove(&[2; 32]);
        issued.insert([1; 32], 29);
        assert!(reconcile_lp(&shares, &issued).is_err());
        assert!(reconcile_lp(&shares, &BTreeMap::new()).is_err());
    }
    #[test]
    fn cdp_owner_and_amount_equations_detect_shifted_or_missing_liabilities() {
        let pair = ([2; 32], [3; 32]);
        let mut totals = CdpTotals::default();
        totals.insert(1, [1; 32], pair, 10, 4).unwrap();
        totals.insert(2, [1; 32], pair, 20, 5).unwrap();
        let owners = BTreeMap::from([([1; 32], BTreeSet::from([1, 2]))]);
        let aggregates = BTreeMap::from([(pair, (30, 9))]);
        totals.reconcile(&owners, &aggregates).unwrap();
        let wrong_owner = BTreeMap::from([([9; 32], BTreeSet::from([1, 2]))]);
        assert!(totals.reconcile(&wrong_owner, &aggregates).is_err());
        let missing = BTreeMap::from([([1; 32], BTreeSet::from([1]))]);
        assert!(totals.reconcile(&missing, &aggregates).is_err());
        let extra = BTreeMap::from([([1; 32], BTreeSet::from([1, 2, 3]))]);
        assert!(totals.reconcile(&extra, &aggregates).is_err());
        for value in [(29, 9), (30, 8)] {
            assert!(totals
                .reconcile(&owners, &BTreeMap::from([(pair, value)]))
                .is_err());
        }
        assert!(totals.reconcile(&owners, &BTreeMap::new()).is_err());
    }
    #[test]
    fn financial_totals_never_saturate_overflow() {
        let mut sums = BTreeMap::from([(1, u128::MAX)]);
        assert!(checked_add(&mut sums, 1, 1).is_err());
        let mut totals = CdpTotals::default();
        totals
            .insert(1, [1; 32], ([2; 32], [3; 32]), 0, u128::MAX)
            .unwrap();
        assert!(totals.insert(2, [1; 32], ([2; 32], [3; 32]), 0, 1).is_err());
    }
    #[test]
    fn lp_schema_and_full_prefix_walk_are_enforced() {
        use scale_info::{meta_type, Registry};
        let mut registry = Registry::new();
        let account = registry
            .register_type(&meta_type::<sp_core::crypto::AccountId32>())
            .id;
        let amount = registry.register_type(&meta_type::<u128>()).id;
        let mut schema = MetadataSchema {
            registry: registry.into(),
            metadata_hash: "0x00".into(),
            descriptors: vec![
                Descriptor {
                    trie: "top".into(),
                    name: "PoolXYK.PoolProviders".into(),
                    prefix: prefix("PoolXYK.PoolProviders"),
                    key_types: vec![account; 2],
                    hashers: vec![Hasher::Identity; 2],
                    value_type: amount,
                },
                Descriptor {
                    trie: "top".into(),
                    name: "PoolXYK.TotalIssuances".into(),
                    prefix: prefix("PoolXYK.TotalIssuances"),
                    key_types: vec![account],
                    hashers: vec![Hasher::Identity],
                    value_type: amount,
                },
            ],
        };
        sp_io::TestExternalities::default().execute_with(|| {
            for (owner, value) in [(1, 10u128), (2, 20u128)] {
                sp_io::storage::set(
                    &[
                        prefix("PoolXYK.PoolProviders"),
                        vec![7; 32],
                        vec![owner; 32],
                    ]
                    .concat(),
                    &value.to_le_bytes(),
                );
            }
            let key = [prefix("PoolXYK.TotalIssuances"), vec![7; 32]].concat();
            sp_io::storage::set(&key, &30u128.to_le_bytes());
            let result = liquidity(&schema, &mut Budget::default()).unwrap();
            assert_eq!(result["providerRows"], 2);
            sp_io::storage::set(&key, &29u128.to_le_bytes());
            assert!(liquidity(&schema, &mut Budget::default()).is_err());
        });
        schema.descriptors[0].hashers[0] = Hasher::Blake2_256;
        assert!(descriptor(&schema, "PoolXYK.PoolProviders", 2).is_err());
    }
    #[test]
    fn full_typed_cdp_scan_joins_positions_index_and_aggregates() {
        use codec::Encode;
        use scale_info::{meta_type, Registry, TypeInfo};
        type Asset = common::AssetId32<()>;
        type Account = sp_core::crypto::AccountId32;
        #[derive(Encode, TypeInfo)]
        struct Cdp {
            owner: Account,
            collateral_asset_id: Asset,
            collateral_amount: u128,
            stablecoin_asset_id: Asset,
            debt: u128,
            interest_coefficient: u128,
        }
        #[derive(Encode, TypeInfo)]
        struct Pair {
            collateral_asset_id: Asset,
            stablecoin_asset_id: Asset,
        }
        #[derive(Encode, TypeInfo)]
        struct Aggregate {
            risk_parameters: u8,
            total_collateral: u128,
            stablecoin_supply: u128,
            last_fee_update_time: u64,
            interest_coefficient: u128,
        }
        let mut registry = Registry::new();
        let account = registry.register_type(&meta_type::<Account>()).id;
        let number = registry.register_type(&meta_type::<u128>()).id;
        let cdp = registry.register_type(&meta_type::<Cdp>()).id;
        let ids = registry.register_type(&meta_type::<Vec<u128>>()).id;
        let pair = registry.register_type(&meta_type::<Pair>()).id;
        let aggregate = registry.register_type(&meta_type::<Aggregate>()).id;
        let schema = MetadataSchema {
            registry: registry.into(),
            metadata_hash: "0x00".into(),
            descriptors: [
                ("Kensetsu.CDPDepository", number, cdp),
                ("Kensetsu.CdpOwnerIndex", account, ids),
                ("Kensetsu.CollateralInfos", pair, aggregate),
            ]
            .into_iter()
            .map(|(name, key, value)| Descriptor {
                trie: "top".into(),
                name: name.into(),
                prefix: prefix(name),
                key_types: vec![key],
                hashers: vec![Hasher::Identity],
                value_type: value,
            })
            .collect(),
        };
        let a = Asset::from_bytes([2; 32]);
        let b = Asset::from_bytes([3; 32]);
        let pair = Pair {
            collateral_asset_id: a,
            stablecoin_asset_id: b,
        };
        sp_io::TestExternalities::default().execute_with(|| {
            for id in [1u128, 2] {
                let row = Cdp {
                    owner: Account::from([1; 32]),
                    collateral_asset_id: a,
                    collateral_amount: 10,
                    stablecoin_asset_id: b,
                    debt: 4,
                    interest_coefficient: 1,
                };
                sp_io::storage::set(
                    &[prefix("Kensetsu.CDPDepository"), id.encode()].concat(),
                    &row.encode(),
                );
            }
            let owner_key = [prefix("Kensetsu.CdpOwnerIndex"), vec![1; 32]].concat();
            let aggregate_key = [prefix("Kensetsu.CollateralInfos"), pair.encode()].concat();
            sp_io::storage::set(&owner_key, &vec![1u128, 2].encode());
            let row = Aggregate {
                risk_parameters: 0,
                total_collateral: 20,
                stablecoin_supply: 8,
                last_fee_update_time: 0,
                interest_coefficient: 1,
            };
            sp_io::storage::set(&aggregate_key, &row.encode());
            let result = kensetsu(&schema, &mut Budget::default()).unwrap();
            assert_eq!(result["positionRows"], 2);
            assert_eq!(result["checkpointLiabilityConservationVerified"], false);
            sp_io::storage::set(&owner_key, &vec![1u128, 1, 2].encode());
            assert!(kensetsu(&schema, &mut Budget::default()).is_err());
            sp_io::storage::set(&owner_key, &vec![1u128, 2].encode());
            let wrong = Aggregate {
                stablecoin_supply: 7,
                ..row
            };
            sp_io::storage::set(&aggregate_key, &wrong.encode());
            assert!(kensetsu(&schema, &mut Budget::default()).is_err());
        });
    }
}
