//! Full account/issuance reconciliation with exact, unchanged legacy XYK markers.
//! This proves present-state derivations, not execution of the historical migration.
use super::{need, next, prefix, read};
use crate::{
    inventory::{Descriptor, Hasher, MetadataSchema},
    typed, Result,
};
use codec::{DecodeWithMemLimit, Encode};
use common::{recovery, ToFeeAccount};
use scale_info::{TypeDef, TypeDefPrimitive};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

type Id = [u8; 32];
type Tech = common::TechAccountId<
    sp_runtime::AccountId32,
    common::TechAssetId<common::PredefinedAssetId>,
    u32,
>;
const MAX_ROWS: usize = 2_000_000;
const MAX_BYTES: usize = 512 * 1024 * 1024;

fn descriptor<'a>(
    schema: &'a MetadataSchema,
    name: &str,
    hashers: &[Hasher],
) -> Result<&'a Descriptor> {
    let found: Vec<_> = schema
        .descriptors
        .iter()
        .filter(|d| d.name == name)
        .collect();
    need(found.len() == 1, "ledger descriptor absent or duplicated")?;
    let d = found[0];
    need(
        d.trie == "top"
            && d.prefix == prefix(name)
            && d.hashers == hashers
            && d.key_types.len() == hashers.len(),
        "ledger descriptor identity differs",
    )?;
    Ok(d)
}
fn u128_type(schema: &MetadataSchema, id: u32) -> bool {
    matches!(
        schema.registry.resolve(id).map(|t| &t.type_def),
        Some(TypeDef::Primitive(TypeDefPrimitive::U128))
    )
}
fn asset_type(schema: &MetadataSchema, id: u32) -> bool {
    schema.registry.resolve(id).is_some_and(|t| {
        t.path.segments.last().is_some_and(|s| s == "AssetId32")
            && t.path.segments.iter().any(|s| s == "common")
    })
}
fn id(raw: &[u8]) -> Result<Id> {
    raw.try_into().map_err(|_| "ledger identity width".into())
}
fn amount(raw: &[u8]) -> Result<u128> {
    Ok(u128::from_le_bytes(
        raw.try_into().map_err(|_| "ledger amount width")?,
    ))
}
fn add(target: &mut u128, value: u128) -> Result<()> {
    *target = target.checked_add(value).ok_or("ledger sum overflow")?;
    Ok(())
}
fn concat_id(raw: &[u8], hasher: Hasher) -> Result<Id> {
    let width = match hasher {
        Hasher::Blake2_128Concat => 16,
        Hasher::Twox64Concat => 8,
        Hasher::Identity => 0,
        _ => return Err("unsupported ledger hasher".into()),
    };
    need(raw.len() == width + 32, "ledger key width differs")?;
    let decoded = id(&raw[width..])?;
    need(hasher.apply(&decoded) == raw, "ledger concat hash differs")?;
    Ok(decoded)
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
    let mut count = 0;
    while let Some(current) = key.filter(|k| k.starts_with(&d.prefix)) {
        let value = read(None, &current)?;
        budget.rows = budget.rows.checked_add(1).ok_or("ledger row overflow")?;
        budget.bytes = budget
            .bytes
            .checked_add(current.len())
            .and_then(|n| n.checked_add(value.len()))
            .ok_or("ledger byte overflow")?;
        need(
            budget.rows <= MAX_ROWS
                && budget.bytes <= MAX_BYTES
                && current.len() <= 4096
                && value.len() <= 512 * 1024,
            "ledger scan bound",
        )?;
        consume(&current[d.prefix.len()..], &value)?;
        count += 1;
        key = next(None, &current);
        need(
            key.as_ref().is_none_or(|n| n > &current),
            "nonmonotonic ledger prefix",
        )?;
    }
    Ok(count)
}
#[derive(Clone, PartialEq, Eq, Encode)]
struct Marker {
    stale_issuance: u128,
    record: Vec<u8>,
    pool: Id,
    technical: Vec<u8>,
    properties_key: Vec<u8>,
    properties_value: Vec<u8>,
    fee_technical: Vec<u8>,
}
struct Pool {
    key: Vec<u8>,
    value: Vec<u8>,
    base: Id,
    target: Id,
    fee: Id,
}
#[derive(Default)]
struct State {
    native_sum: u128,
    native_issuance: u128,
    native_rows: usize,
    token_rows: usize,
    issuance_rows: usize,
    token_sums: BTreeMap<Id, (usize, u128)>,
    issuance: BTreeMap<Id, u128>,
    markers: BTreeMap<Id, Marker>,
    rows: usize,
    bytes: usize,
}
/// No public constructor/Deserialize. Only the same verified checkpoint creates this baseline.
pub(crate) struct Baseline {
    state: State,
    metadata_hash: String,
}

fn marker_record(pool: Id) -> Vec<u8> {
    // Historical pool-xyk get_marking_asset_repr: Arity3(GenericU128(tag),
    // Extra(LstId(XYKPool)), Extra(AccountId(pool))). Source order is fixed.
    let mut raw = vec![3, 2];
    raw.extend(common::hash_to_u128_pair(b"Marking asset").0.encode());
    raw.extend([8, 1, 0, 8, 2]);
    raw.extend(pool);
    raw
}
fn marker_id(raw: &[u8]) -> Id {
    let mut input = b"From AssetRecord".to_vec();
    input.extend(raw);
    let mut asset = sp_io::hashing::keccak_256(&input);
    asset[0] = 0;
    asset
}
fn technical_id(raw: &[u8]) -> Id {
    let mut account = [0; 32];
    account[..16].copy_from_slice(&common::TECH_ACCOUNT_MAGIC_PREFIX);
    account[16..].copy_from_slice(&sp_io::hashing::twox_128(raw));
    account
}
fn decode_technical(raw: &[u8]) -> Result<Tech> {
    let mut input = raw;
    let value =
        Tech::decode_with_mem_limit(&mut input, 2 * 1024 * 1024).map_err(|e| e.to_string())?;
    need(
        input.is_empty() && value.encode() == raw,
        "noncanonical marker technical record",
    )?;
    Ok(value)
}
fn classify(
    asset: Id,
    stale_issuance: u128,
    record: &[u8],
    pools: &BTreeMap<Id, Pool>,
    technical: &BTreeMap<Id, Vec<u8>>,
    pool_sums: &BTreeMap<Id, u128>,
    pool_issuance: &BTreeMap<Id, u128>,
) -> Result<Marker> {
    need(
        record.len() == 55,
        "orphan issuance lacks exact legacy marker record",
    )?;
    let pool = id(&record[23..])?;
    need(
        record == marker_record(pool) && marker_id(record) == asset,
        "orphan issuance marker tag/hash differs",
    )?;
    let property = pools
        .get(&pool)
        .ok_or("legacy marker lacks unique current pool")?;
    let raw = technical
        .get(&pool)
        .ok_or("legacy marker technical account absent")?;
    need(
        technical_id(raw) == pool,
        "legacy marker technical account hash differs",
    )?;
    let tech = decode_technical(raw)?;
    let common::TechAccountId::Pure(_, common::TechPurpose::XykLiquidityKeeper(pair)) = &tech
    else {
        return Err("legacy marker account is not a pure XYK keeper".into());
    };
    let base: common::AssetId32<common::PredefinedAssetId> = pair.base_asset_id.into();
    let target: common::AssetId32<common::PredefinedAssetId> = pair.target_asset_id.into();
    need(
        base.code == property.base && target.code == property.target,
        "legacy marker technical pair differs",
    )?;
    let fee = tech
        .to_fee_account()
        .ok_or("legacy marker fee derivation absent")?
        .encode();
    need(
        technical_id(&fee) == property.fee && technical.get(&property.fee) == Some(&fee),
        "legacy marker fee account/registration differs",
    )?;
    need(
        pool_issuance.get(&pool).copied() == Some(pool_sums.get(&pool).copied().unwrap_or(0)),
        "legacy marker pool shares differ from live issuance",
    )?;
    Ok(Marker {
        stale_issuance,
        record: record.to_vec(),
        pool,
        technical: raw.clone(),
        properties_key: property.key.clone(),
        properties_value: property.value.clone(),
        fee_technical: fee,
    })
}
fn inspect(schema: &MetadataSchema) -> Result<State> {
    let mut state = State::default();
    let mut budget = Budget::default();
    let accounts = descriptor(schema, "System.Account", &[Hasher::Blake2_128Concat])?;
    let native = descriptor(schema, "Balances.TotalIssuance", &[])?;
    need(
        typed::account_type(&schema.registry, accounts.key_types[0])
            && u128_type(schema, native.value_type),
        "native ledger metadata differs",
    )?;
    state.native_rows = rows(accounts, &mut budget, |key, raw| {
        concat_id(key, Hasher::Blake2_128Concat)?;
        typed::decode_exact(raw, accounts.value_type, &schema.registry)?;
        for field in ["free", "reserved"] {
            add(
                &mut state.native_sum,
                typed::u128_field(raw, accounts.value_type, &schema.registry, &["data", field])?.0,
            )?;
        }
        Ok(())
    })?;
    let count = rows(native, &mut budget, |key, raw| {
        need(key.is_empty(), "native issuance key differs")?;
        state.native_issuance = amount(raw)?;
        Ok(())
    })?;
    need(
        count == 1 && state.native_sum == state.native_issuance,
        "complete native account sum differs from issuance",
    )?;
    let accounts = descriptor(
        schema,
        "Tokens.Accounts",
        &[Hasher::Blake2_128Concat, Hasher::Twox64Concat],
    )?;
    let issuance = descriptor(schema, "Tokens.TotalIssuance", &[Hasher::Twox64Concat])?;
    need(
        typed::account_type(&schema.registry, accounts.key_types[0])
            && asset_type(schema, accounts.key_types[1])
            && asset_type(schema, issuance.key_types[0])
            && u128_type(schema, issuance.value_type),
        "token ledger metadata differs",
    )?;
    state.token_rows = rows(accounts, &mut budget, |key, raw| {
        need(key.len() == 88, "token account double key differs")?;
        concat_id(&key[..48], Hasher::Blake2_128Concat)?;
        let asset = concat_id(&key[48..], Hasher::Twox64Concat)?;
        typed::decode_exact(raw, accounts.value_type, &schema.registry)?;
        let value = state.token_sums.entry(asset).or_default();
        value.0 += 1;
        for field in ["free", "reserved"] {
            add(
                &mut value.1,
                typed::u128_field(raw, accounts.value_type, &schema.registry, &[field])?.0,
            )?;
        }
        Ok(())
    })?;
    state.issuance_rows = rows(issuance, &mut budget, |key, raw| {
        need(
            state
                .issuance
                .insert(concat_id(key, Hasher::Twox64Concat)?, amount(raw)?)
                .is_none(),
            "duplicate token issuance",
        )?;
        Ok(())
    })?;
    let records = descriptor(schema, "Assets.AssetRecordAssetId", &[Hasher::Twox64Concat])?;
    let techs = descriptor(
        schema,
        "Technical.TechAccounts",
        &[Hasher::Blake2_128Concat],
    )?;
    let props = descriptor(
        schema,
        "PoolXYK.Properties",
        &[Hasher::Blake2_128Concat, Hasher::Blake2_128Concat],
    )?;
    let providers = descriptor(
        schema,
        "PoolXYK.PoolProviders",
        &[Hasher::Identity, Hasher::Identity],
    )?;
    let issued = descriptor(schema, "PoolXYK.TotalIssuances", &[Hasher::Identity])?;
    need(
        asset_type(schema, records.key_types[0])
            && typed::account_type(&schema.registry, techs.key_types[0])
            && props.key_types.iter().all(|t| asset_type(schema, *t))
            && providers
                .key_types
                .iter()
                .chain(&issued.key_types)
                .all(|t| typed::account_type(&schema.registry, *t))
            && u128_type(schema, providers.value_type)
            && u128_type(schema, issued.value_type),
        "marker metadata key/value identity differs",
    )?;
    need(
        matches!(schema.registry.resolve(props.value_type).map(|t| &t.type_def),
            Some(TypeDef::Tuple(t)) if t.fields.len() == 2 && t.fields.iter().all(|f| typed::account_type(&schema.registry, f.id))),
        "pool property metadata must contain two accounts",
    )?;
    let (mut record_rows, mut technical, mut pools, mut pool_sums, mut pool_issuance) = (
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    rows(records, &mut budget, |key, raw| {
        typed::decode_exact(raw, records.value_type, &schema.registry)?;
        record_rows.insert(concat_id(key, Hasher::Twox64Concat)?, raw.to_vec());
        Ok(())
    })?;
    rows(techs, &mut budget, |key, raw| {
        typed::decode_exact(raw, techs.value_type, &schema.registry)?;
        technical.insert(concat_id(key, Hasher::Blake2_128Concat)?, raw.to_vec());
        Ok(())
    })?;
    rows(props, &mut budget, |key, raw| {
        need(
            key.len() == 96 && raw.len() == 64,
            "pool property codec differs",
        )?;
        typed::decode_exact(raw, props.value_type, &schema.registry)?;
        need(
            pools
                .insert(
                    id(&raw[..32])?,
                    Pool {
                        key: key.to_vec(),
                        value: raw.to_vec(),
                        base: concat_id(&key[..48], Hasher::Blake2_128Concat)?,
                        target: concat_id(&key[48..], Hasher::Blake2_128Concat)?,
                        fee: id(&raw[32..])?,
                    },
                )
                .is_none(),
            "ambiguous pool properties account",
        )?;
        Ok(())
    })?;
    rows(providers, &mut budget, |key, raw| {
        need(key.len() == 64, "pool provider key differs")?;
        add(pool_sums.entry(id(&key[..32])?).or_default(), amount(raw)?)
    })?;
    rows(issued, &mut budget, |key, raw| {
        pool_issuance.insert(id(key)?, amount(raw)?);
        Ok(())
    })?;
    for (asset, (_, sum)) in &state.token_sums {
        need(
            state.issuance.get(asset).copied().unwrap_or(0) == *sum,
            "complete token account sum differs from issuance",
        )?;
    }
    for (asset, issuance) in &state.issuance {
        if *issuance != 0 && !state.token_sums.contains_key(asset) {
            let record = record_rows
                .get(asset)
                .ok_or("unclassified orphan token issuance")?;
            let marker = classify(
                *asset,
                *issuance,
                record,
                &pools,
                &technical,
                &pool_sums,
                &pool_issuance,
            )?;
            state.markers.insert(*asset, marker);
        }
    }
    state.rows = budget.rows;
    state.bytes = budget.bytes;
    Ok(state)
}
pub(super) fn checkpoint(
    ext: &mut sp_io::TestExternalities,
    schema: &MetadataSchema,
) -> Result<Baseline> {
    let root = crate::replay::root(ext);
    need(
        root == sp_core::H256::from(recovery::CHECKPOINT_STATE_ROOT),
        "ledger checkpoint root differs",
    )?;
    let state = ext.execute_with(|| inspect(schema))?;
    need(
        crate::replay::root(ext) == root,
        "ledger checkpoint scan mutated state",
    )?;
    Ok(Baseline {
        state,
        metadata_hash: schema.metadata_hash.clone(),
    })
}
fn reconcile(before: &State, after: &State) -> Result<()> {
    need(
        before.markers == after.markers,
        "legacy marker identity/stale liability changed during recovery",
    )?;
    for asset in before.markers.keys() {
        need(
            !after.token_sums.contains_key(asset),
            "legacy marker token accounts resurrected",
        )?;
    }
    Ok(())
}
pub(super) fn verify(schema: &MetadataSchema, before: &Baseline) -> Result<Value> {
    let after = inspect(schema)?;
    reconcile(&before.state, &after)?;
    let markers: Vec<_> = after.markers.iter().map(|(asset, marker)| json!({"asset":"0x".to_owned()+&hex::encode(asset),
        "pool":"0x".to_owned()+&hex::encode(marker.pool),"retainedLegacyIssuance":marker.stale_issuance.to_string()})).collect();
    Ok(
        json!({"schema":"sora-nexus-full-account-ledger-equations-v1","checkpointStateRoot":"0x".to_owned()+&hex::encode(recovery::CHECKPOINT_STATE_ROOT),
        "checkpointMetadataHash":before.metadata_hash,"finalMetadataHash":schema.metadata_hash,
        "nativeAccountSumEqualsIssuance":true,"allTokenAccountSumsReconciled":true,"unclassifiedOrphanIssuanceRows":0,
        "exactLegacyMarkerDerivationsVerified":true,"legacyMarkerRecordsAndIssuanceUnchanged":true,"legacyMarkerAccountsRemainAbsent":true,
        "currentMarkerPoolSharesEqualIssuance":true,"legacyMarkerRows":markers,
        "legacyMarkerIdentitySha256":hex::encode(Sha256::digest(after.markers.encode())),
        "checkpointNativeAccountRows":before.state.native_rows,"finalNativeAccountRows":after.native_rows,
        "checkpointTokenAccountRows":before.state.token_rows,"finalTokenAccountRows":after.token_rows,
        "checkpointTokenIssuanceRows":before.state.issuance_rows,"finalTokenIssuanceRows":after.issuance_rows,
        "checkpointNativeIssuance":before.state.native_issuance.to_string(),"finalNativeIssuance":after.native_issuance.to_string(),
        "checkpointRows":before.state.rows,"finalRows":after.rows,"checkpointBytes":before.state.bytes,"finalBytes":after.bytes,
        "historicalMigrationExecutionVerified":false,"checkpointToFinalOwnershipConservationVerified":false,
        "completeFinancialCoverageVerified":false,"completeStorageSemantics":false,"productionReady":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    type MarkerFixture = (
        Id,
        Vec<u8>,
        BTreeMap<Id, Pool>,
        BTreeMap<Id, Vec<u8>>,
        BTreeMap<Id, u128>,
        BTreeMap<Id, u128>,
    );
    fn fixture() -> MarkerFixture {
        let pair = common::TradingPair {
            base_asset_id: common::TechAssetId::Wrapped(common::PredefinedAssetId::XOR),
            target_asset_id: common::TechAssetId::Escaped([7; 32]),
        };
        let tech = Tech::Pure(0, common::TechPurpose::XykLiquidityKeeper(pair));
        let raw = tech.encode();
        let pool = technical_id(&raw);
        let fee = tech.to_fee_account().unwrap().encode();
        let fee_id = technical_id(&fee);
        let record = marker_record(pool);
        let asset = marker_id(&record);
        let base: common::AssetId32<common::PredefinedAssetId> = pair.base_asset_id.into();
        let target: common::AssetId32<common::PredefinedAssetId> = pair.target_asset_id.into();
        let property = Pool {
            key: vec![1],
            value: vec![2],
            base: base.code,
            target: target.code,
            fee: fee_id,
        };
        (
            asset,
            record,
            BTreeMap::from([(pool, property)]),
            BTreeMap::from([(pool, raw), (fee_id, fee)]),
            BTreeMap::from([(pool, 5)]),
            BTreeMap::from([(pool, 5)]),
        )
    }
    #[test]
    fn exact_retained_marker_keccak_vector_matches_source_codec() {
        let record = hex::decode("0302526c06e75e4e78c034a1f9a35d454207080100080254734f90f971a02c609b2d684e61b557d2a6aede6a757a97d472f5dbb5a1a71d").unwrap();
        assert_eq!(marker_record(id(&record[23..]).unwrap()), record);
        assert_eq!(
            hex::encode(marker_id(&record)),
            "0002232a4ecaabc044838587f893f596a38f41400d0a0e34b550a5907dbbaafb"
        );
    }
    #[test]
    fn marker_requires_exact_hash_technical_pair_fee_and_current_pool_shares() {
        let (asset, record, mut pools, mut techs, mut shares, issued) = fixture();
        let good = classify(asset, 100, &record, &pools, &techs, &shares, &issued).unwrap();
        assert_eq!(good.stale_issuance, 100); // Old marker issuance need not equal today's live pool supply.
        assert!(classify([9; 32], 100, &record, &pools, &techs, &shares, &issued).is_err());
        let mut bad = record.clone();
        bad[2] ^= 1;
        assert!(classify(marker_id(&bad), 100, &bad, &pools, &techs, &shares, &issued).is_err());
        shares.insert(good.pool, 4);
        assert!(classify(asset, 100, &record, &pools, &techs, &shares, &issued).is_err());
        shares.insert(good.pool, 5);
        pools.get_mut(&good.pool).unwrap().target = [8; 32];
        assert!(classify(asset, 100, &record, &pools, &techs, &shares, &issued).is_err());
        pools.get_mut(&good.pool).unwrap().target = [7; 32];
        techs.remove(&pools[&good.pool].fee);
        assert!(classify(asset, 100, &record, &pools, &techs, &shares, &issued).is_err());
    }
    #[test]
    fn no_orphan_classification_can_hide_a_changed_liability_or_resurrection() {
        let (asset, record, pools, techs, shares, issued) = fixture();
        let marker = classify(asset, 100, &record, &pools, &techs, &shares, &issued).unwrap();
        let before = State {
            markers: BTreeMap::from([(asset, marker.clone())]),
            ..Default::default()
        };
        let mut after = State {
            markers: before.markers.clone(),
            ..Default::default()
        };
        reconcile(&before, &after).unwrap();
        after.markers.get_mut(&asset).unwrap().stale_issuance = 99;
        assert!(reconcile(&before, &after).is_err());
        after.markers = before.markers.clone();
        after.token_sums.insert(asset, (1, 0));
        assert!(reconcile(&before, &after).is_err());
        after.token_sums.clear();
        after.markers.clear();
        assert!(reconcile(&before, &after).is_err());
        after.markers = before.markers.clone();
        after.markers.insert([9; 32], marker);
        assert!(reconcile(&before, &after).is_err());
    }
    #[test]
    fn concat_hashes_and_checked_ledger_sums_are_not_optional() {
        let account = [8; 32];
        let mut raw = Hasher::Blake2_128Concat.apply(&account);
        assert_eq!(concat_id(&raw, Hasher::Blake2_128Concat).unwrap(), account);
        raw[0] ^= 1;
        assert!(concat_id(&raw, Hasher::Blake2_128Concat).is_err());
        let mut total = u128::MAX;
        assert!(add(&mut total, 1).is_err());
    }
}
