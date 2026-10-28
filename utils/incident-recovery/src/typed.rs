//! Metadata-directed AccountId substitution. Hashes, signatures, amounts and
//! opaque bytes are never rewritten merely because their bytes match an account.

use crate::Result;
use codec::{Compact, Decode};
use scale_info::{PortableRegistry, TypeDef, TypeDefPrimitive};
use scale_value::{Composite, Primitive, Value, ValueDef};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub fn account_type(registry: &PortableRegistry, id: u32) -> bool {
    registry.resolve(id).is_some_and(|ty| {
        let path = &ty.path.segments;
        path.last().is_some_and(|name| name == "AccountId32")
            && path
                .iter()
                .any(|name| name == "sp_core" || name == "sp_runtime")
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityReference {
    pub identity: String,
    pub kind: &'static str,
    pub type_id: u32,
    pub type_path: String,
    pub path: Vec<String>,
}

/// Finds only metadata-typed AccountId32/H160 values, never matching arbitrary
/// bytes. H160 is reported as an address reference, not proof of ownership.
pub fn watched_references(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
    watched: &BTreeSet<Vec<u8>>,
) -> Result<Vec<IdentityReference>> {
    let value = decode_exact(bytes, type_id, registry)?;
    let mut references = Vec::new();
    fn visit(
        value: &Value<u32>,
        registry: &PortableRegistry,
        watched: &BTreeSet<Vec<u8>>,
        path: &mut Vec<String>,
        out: &mut Vec<IdentityReference>,
    ) -> Result<()> {
        let ty = registry
            .resolve(value.context)
            .ok_or("reference metadata type missing")?;
        let kind = if account_type(registry, value.context) {
            Some("account_id32")
        } else if ty.path.segments.last().is_some_and(|s| s == "H160") {
            Some("h160_address_reference")
        } else {
            None
        };
        if let Some(kind) = kind {
            let encoded = encode(value, registry)?;
            if (kind == "account_id32" && encoded.len() != 32)
                || (kind != "account_id32" && encoded.len() != 20)
            {
                return Err("typed identity width mismatch".into());
            }
            if watched.contains(&encoded) {
                out.push(IdentityReference {
                    identity: format!("0x{}", hex::encode(encoded)),
                    kind,
                    type_id: value.context,
                    type_path: ty.path.segments.join("::"),
                    path: path.clone(),
                });
            }
            return Ok(());
        }
        let composite = match &value.value {
            ValueDef::Composite(c) => Some(c),
            ValueDef::Variant(v) => {
                path.push(format!("variant:{}", v.name));
                Some(&v.values)
            }
            _ => None,
        };
        if let Some(composite) = composite {
            match composite {
                Composite::Named(fields) => {
                    for (name, value) in fields {
                        path.push(name.clone());
                        visit(value, registry, watched, path, out)?;
                        path.pop();
                    }
                }
                Composite::Unnamed(fields) => {
                    for (index, value) in fields.iter().enumerate() {
                        path.push(index.to_string());
                        visit(value, registry, watched, path, out)?;
                        path.pop();
                    }
                }
            }
        }
        if matches!(value.value, ValueDef::Variant(_)) {
            path.pop();
        }
        Ok(())
    }
    visit(&value, registry, watched, &mut vec![], &mut references)?;
    Ok(references)
}

/// Human-readable exact typed values for a small watched-reference report.
/// Amounts remain decimal strings so JSON consumers cannot round u128 values.
pub fn decoded_json(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
) -> Result<serde_json::Value> {
    fn convert(value: &Value<u32>, registry: &PortableRegistry) -> Result<serde_json::Value> {
        if account_type(registry, value.context)
            || registry.resolve(value.context).is_some_and(|t| {
                t.path
                    .segments
                    .last()
                    .is_some_and(|s| s == "H160" || s == "H256" || s == "AssetId32")
            })
        {
            return Ok(serde_json::Value::String(format!(
                "0x{}",
                hex::encode(encode(value, registry)?)
            )));
        }
        fn composite(
            value: &Composite<u32>,
            registry: &PortableRegistry,
        ) -> Result<serde_json::Value> {
            match value {
                Composite::Named(fields) => Ok(serde_json::Value::Object(
                    fields
                        .iter()
                        .map(|(key, value)| Ok((key.clone(), convert(value, registry)?)))
                        .collect::<Result<_>>()?,
                )),
                Composite::Unnamed(fields) => Ok(serde_json::Value::Array(
                    fields
                        .iter()
                        .map(|value| convert(value, registry))
                        .collect::<Result<_>>()?,
                )),
            }
        }
        Ok(match &value.value {
            ValueDef::Composite(fields) => composite(fields, registry)?,
            ValueDef::Variant(variant) => {
                serde_json::json!({"variant":variant.name,"fields":composite(&variant.values, registry)?})
            }
            ValueDef::Primitive(Primitive::U128(number)) => {
                serde_json::Value::String(number.to_string())
            }
            ValueDef::Primitive(Primitive::I128(number)) => {
                serde_json::Value::String(number.to_string())
            }
            ValueDef::Primitive(Primitive::Bool(value)) => serde_json::Value::Bool(*value),
            ValueDef::Primitive(Primitive::String(value)) => {
                serde_json::Value::String(value.clone())
            }
            ValueDef::Primitive(Primitive::Char(value)) => {
                serde_json::Value::String(value.to_string())
            }
            ValueDef::Primitive(Primitive::U256(value))
            | ValueDef::Primitive(Primitive::I256(value)) => {
                serde_json::Value::String(format!("0x{}", hex::encode(value)))
            }
            ValueDef::BitSequence(bits) => serde_json::Value::String(format!("{bits:?}")),
        })
    }
    convert(&decode_exact(bytes, type_id, registry)?, registry)
}

pub const MAX_TYPED_BYTES: usize = 1024 * 1024;
const MAX_TYPED_NODES: usize = 65_536;
const MAX_TYPED_DEPTH: usize = 64;

pub(crate) fn encode(value: &Value<u32>, registry: &PortableRegistry) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    scale_value::scale::encode_as_type(value, value.context, registry, &mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

fn take(input: &mut &[u8], count: usize) -> Result<()> {
    *input = input.get(count..).ok_or("truncated typed storage")?;
    Ok(())
}

/// Allocation-free SCALE shape validation before scale-value can reserve a Vec
/// from an untrusted compact count (including Vec<()> with tiny encoded input).
fn preflight(
    input: &mut &[u8],
    id: u32,
    registry: &PortableRegistry,
    depth: usize,
    nodes: &mut usize,
) -> Result<()> {
    if depth > MAX_TYPED_DEPTH || *nodes == 0 {
        return Err("typed decoding depth/node budget exceeded".into());
    }
    *nodes -= 1;
    let ty = registry
        .resolve(id)
        .ok_or("typed registry reference missing")?;
    match &ty.type_def {
        TypeDef::Composite(c) => {
            for field in &c.fields {
                preflight(input, field.ty.id, registry, depth + 1, nodes)?;
            }
        }
        TypeDef::Variant(v) => {
            let index = *input.first().ok_or("missing enum variant")?;
            take(input, 1)?;
            let variant = v
                .variants
                .iter()
                .find(|v| v.index == index)
                .ok_or("unknown enum variant")?;
            for field in &variant.fields {
                preflight(input, field.ty.id, registry, depth + 1, nodes)?;
            }
        }
        TypeDef::Sequence(s) => {
            let count = Compact::<u32>::decode(input).map_err(|e| e.to_string())?.0 as usize;
            if count > *nodes {
                return Err("typed sequence exceeds allocation budget".into());
            }
            for _ in 0..count {
                preflight(input, s.type_param.id, registry, depth + 1, nodes)?;
            }
        }
        TypeDef::Array(a) => {
            if a.len as usize > *nodes {
                return Err("typed array exceeds allocation budget".into());
            }
            for _ in 0..a.len {
                preflight(input, a.type_param.id, registry, depth + 1, nodes)?;
            }
        }
        TypeDef::Tuple(t) => {
            for field in &t.fields {
                preflight(input, field.id, registry, depth + 1, nodes)?;
            }
        }
        TypeDef::Compact(_) => {
            Compact::<u128>::decode(input).map_err(|e| e.to_string())?;
        }
        TypeDef::BitSequence(bits) => {
            let count = Compact::<u32>::decode(input).map_err(|e| e.to_string())?.0 as usize;
            let width = match registry
                .resolve(bits.bit_store_type.id)
                .map(|t| &t.type_def)
            {
                Some(TypeDef::Primitive(TypeDefPrimitive::U8)) => 1,
                Some(TypeDef::Primitive(TypeDefPrimitive::U16)) => 2,
                Some(TypeDef::Primitive(TypeDefPrimitive::U32)) => 4,
                Some(TypeDef::Primitive(TypeDefPrimitive::U64)) => 8,
                _ => return Err("unsupported bit-store type".into()),
            };
            let length = count
                .div_ceil(width * 8)
                .checked_mul(width)
                .ok_or("bit sequence overflow")?;
            if length > MAX_TYPED_BYTES {
                return Err("bit sequence exceeds allocation budget".into());
            }
            take(input, length)?;
        }
        TypeDef::Primitive(primitive) => {
            let width = match primitive {
                TypeDefPrimitive::Bool | TypeDefPrimitive::U8 | TypeDefPrimitive::I8 => 1,
                TypeDefPrimitive::U16 | TypeDefPrimitive::I16 => 2,
                TypeDefPrimitive::Char | TypeDefPrimitive::U32 | TypeDefPrimitive::I32 => 4,
                TypeDefPrimitive::U64 | TypeDefPrimitive::I64 => 8,
                TypeDefPrimitive::U128 | TypeDefPrimitive::I128 => 16,
                TypeDefPrimitive::U256 | TypeDefPrimitive::I256 => 32,
                TypeDefPrimitive::Str => {
                    Compact::<u32>::decode(input).map_err(|e| e.to_string())?.0 as usize
                }
            };
            take(input, width)?;
        }
    }
    Ok(())
}

fn bounded_prefix(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
) -> Result<(Value<u32>, usize)> {
    if bytes.len() > MAX_TYPED_BYTES {
        return Err(
            "typed record exceeds 1 MiB; explicit opaque retention or dedicated adapter required"
                .into(),
        );
    }
    let mut checked = bytes;
    let mut nodes = MAX_TYPED_NODES;
    preflight(&mut checked, type_id, registry, 0, &mut nodes)?;
    let consumed = bytes.len() - checked.len();
    let mut input = &bytes[..consumed];
    let value = scale_value::scale::decode_as_type(&mut input, type_id, registry)
        .map_err(|e| e.to_string())?;
    if !input.is_empty() || encode(&value, registry)? != bytes[..consumed] {
        return Err("typed storage did not roundtrip exactly".into());
    }
    Ok((value, consumed))
}

pub(crate) fn decode_exact(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
) -> Result<Value<u32>> {
    let (value, consumed) = bounded_prefix(bytes, type_id, registry)?;
    if consumed != bytes.len() {
        return Err("typed storage did not roundtrip exactly".into());
    }
    Ok(value)
}

fn named_field_mut<'a>(
    value: &'a mut Value<u32>,
    path: &[&str],
    registry: &PortableRegistry,
) -> Result<(&'a mut Value<u32>, usize)> {
    if path.is_empty() {
        return Ok((value, 0));
    }
    // Restrict projections to named structs. In particular, a Vec's SCALE
    // length prefix and an enum discriminant must never be omitted from offsets.
    if !matches!(
        registry.resolve(value.context).map(|ty| &ty.type_def),
        Some(TypeDef::Composite(_))
    ) {
        return Err("scalar projection only traverses named composite fields".into());
    }
    let ValueDef::Composite(Composite::Named(fields)) = &mut value.value else {
        return Err("scalar projection field is not a named composite".into());
    };
    let mut offset = 0usize;
    for (name, field) in fields {
        if name == path[0] {
            let (target, relative) = named_field_mut(field, &path[1..], registry)?;
            return Ok((
                target,
                offset
                    .checked_add(relative)
                    .ok_or("projection offset overflow")?,
            ));
        }
        offset = offset
            .checked_add(encode(field, registry)?.len())
            .ok_or("projection offset overflow")?;
    }
    Err("scalar projection field missing".into())
}

/// Locate only a metadata-proven, non-compact u128 field in an exact record.
pub fn u128_field(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
    path: &[&str],
) -> Result<(u128, usize)> {
    let mut value = decode_exact(bytes, type_id, registry)?;
    let (field, offset) = named_field_mut(&mut value, path, registry)?;
    if !matches!(
        registry.resolve(field.context).map(|ty| &ty.type_def),
        Some(TypeDef::Primitive(TypeDefPrimitive::U128))
    ) {
        return Err("amount projection is not an uncompressed u128".into());
    }
    let ValueDef::Primitive(Primitive::U128(amount)) = field.value else {
        return Err("u128 metadata/value mismatch".into());
    };
    if bytes.get(offset..offset + 16) != Some(amount.to_le_bytes().as_slice()) {
        return Err("scalar projection differs from exact SCALE bytes".into());
    }
    Ok((amount, offset))
}

pub fn set_u128_field(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
    path: &[&str],
    amount: u128,
) -> Result<Vec<u8>> {
    let (_, offset) = u128_field(bytes, type_id, registry, path)?;
    let mut value = decode_exact(bytes, type_id, registry)?;
    let (field, _) = named_field_mut(&mut value, path, registry)?;
    field.value = ValueDef::Primitive(Primitive::U128(amount));
    let encoded = encode(&value, registry)?;
    let mut expected = bytes.to_vec();
    expected[offset..offset + 16].copy_from_slice(&amount.to_le_bytes());
    if encoded != expected {
        return Err("scalar edit changed unrelated SCALE fields".into());
    }
    Ok(encoded)
}

pub fn named_fields(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
    path: &[&str],
) -> Result<Vec<String>> {
    let mut value = decode_exact(bytes, type_id, registry)?;
    let (field, _) = named_field_mut(&mut value, path, registry)?;
    match &field.value {
        ValueDef::Composite(Composite::Named(fields)) => {
            Ok(fields.iter().map(|(name, _)| name.clone()).collect())
        }
        _ => Err("expected named fields".into()),
    }
}

fn walk(
    value: &mut Value<u32>,
    registry: &PortableRegistry,
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
    accounts: &mut BTreeSet<[u8; 32]>,
) -> Result<()> {
    if account_type(registry, value.context) {
        let account: [u8; 32] = encode(value, registry)?
            .try_into()
            .map_err(|_| "AccountId32 metadata did not encode 32 bytes")?;
        accounts.insert(account);
        if let Some(replacement) = mappings.get(&account) {
            let mut input = replacement.as_slice();
            *value = scale_value::scale::decode_as_type(&mut input, value.context, registry)
                .map_err(|e| e.to_string())?;
            if !input.is_empty() {
                return Err("replacement account decode left trailing bytes".into());
            }
        }
        return Ok(());
    }
    let composite = match &mut value.value {
        ValueDef::Composite(c) => Some(c),
        ValueDef::Variant(v) => Some(&mut v.values),
        _ => None,
    };
    if let Some(composite) = composite {
        match composite {
            Composite::Named(values) => {
                for (_, value) in values {
                    walk(value, registry, mappings, accounts)?;
                }
            }
            Composite::Unnamed(values) => {
                for value in values {
                    walk(value, registry, mappings, accounts)?;
                }
            }
        }
    }
    Ok(())
}

pub fn transform_value(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
) -> Result<(Vec<u8>, BTreeSet<[u8; 32]>)> {
    let (encoded, accounts, consumed) = transform_prefix(bytes, type_id, registry, mappings)?;
    if consumed != bytes.len() {
        return Err("typed storage has trailing bytes".into());
    }
    Ok((encoded, accounts))
}

/// Decode one SCALE key component without interpreting the next map component.
/// Exact re-encoding is checked before any AccountId replacement is applied.
pub fn transform_prefix(
    bytes: &[u8],
    type_id: u32,
    registry: &PortableRegistry,
    mappings: &BTreeMap<[u8; 32], [u8; 32]>,
) -> Result<(Vec<u8>, BTreeSet<[u8; 32]>, usize)> {
    let (mut value, consumed) = bounded_prefix(bytes, type_id, registry)?;
    let mut accounts = BTreeSet::new();
    walk(&mut value, registry, mappings, &mut accounts)?;
    Ok((encode(&value, registry)?, accounts, consumed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::{Decode, Encode};
    use scale_info::{meta_type, Registry, TypeInfo};
    use sp_core::{crypto::AccountId32, H160, H256};

    #[derive(Encode, Decode, TypeInfo, Debug, PartialEq)]
    struct Position {
        owner: AccountId32,
        historical_hash: H256,
        amount: u128,
        opaque: Vec<u8>,
    }

    #[test]
    fn watched_identities_are_typed_references_not_hash_or_opaque_matches() {
        #[derive(Encode, TypeInfo)]
        struct References {
            owner: AccountId32,
            destination: H160,
            hash: H256,
            opaque: Vec<u8>,
        }
        let value = References {
            owner: [1; 32].into(),
            destination: [2; 20].into(),
            hash: [1; 32].into(),
            opaque: vec![1; 32],
        };
        let mut registry = Registry::new();
        let id = registry.register_type(&meta_type::<References>()).id;
        let registry = PortableRegistry::from(registry);
        let references = watched_references(
            &value.encode(),
            id,
            &registry,
            &BTreeSet::from([vec![1; 32], vec![2; 20]]),
        )
        .unwrap();
        assert_eq!(references.len(), 2);
        assert_eq!(references[0].path, vec!["owner"]);
        assert_eq!(references[1].path, vec!["destination"]);
        assert_eq!(references[1].kind, "h160_address_reference");
    }

    #[test]
    fn allocation_bombs_and_huge_records_fail_before_value_tree_allocation() {
        let mut registry = Registry::new();
        let units = registry.register_type(&meta_type::<Vec<()>>()).id;
        let data = registry.register_type(&meta_type::<Vec<u8>>()).id;
        let portable = PortableRegistry::from(registry);
        let bomb = Compact(u32::MAX).encode();
        assert!(transform_value(&bomb, units, &portable, &BTreeMap::new())
            .unwrap_err()
            .contains("allocation budget"));
        assert!(transform_value(
            &vec![0; MAX_TYPED_BYTES + 1],
            data,
            &portable,
            &BTreeMap::new()
        )
        .unwrap_err()
        .contains("1 MiB"));
    }

    #[test]
    fn scalar_projection_requires_exact_uncompressed_type_and_preserves_others() {
        let original = Position {
            owner: AccountId32::from([1; 32]),
            historical_hash: H256::from([2; 32]),
            amount: 123,
            opaque: vec![3; 10],
        };
        let mut registry = Registry::new();
        let id = registry.register_type(&meta_type::<Position>()).id;
        let portable = PortableRegistry::from(registry);
        let bytes = original.encode();
        assert_eq!(
            u128_field(&bytes, id, &portable, &["amount"]).unwrap(),
            (123, 64)
        );
        let changed = set_u128_field(&bytes, id, &portable, &["amount"], 0).unwrap();
        assert_eq!(&changed[..64], &bytes[..64]);
        assert_eq!(&changed[80..], &bytes[80..]);
        assert!(u128_field(&bytes, id, &portable, &["opaque"]).is_err());
    }

    #[test]
    fn only_typed_accounts_move_identical_hash_and_opaque_bytes_do_not() {
        let original = Position {
            owner: AccountId32::from([1; 32]),
            historical_hash: H256::from([1; 32]),
            amount: 123,
            opaque: vec![1; 32],
        };
        let mut registry = Registry::new();
        let id = registry.register_type(&meta_type::<Position>()).id;
        let portable = PortableRegistry::from(registry);
        let (bytes, accounts) = transform_value(
            &original.encode(),
            id,
            &portable,
            &BTreeMap::from([([1; 32], [2; 32])]),
        )
        .unwrap();
        let result = Position::decode(&mut bytes.as_slice()).unwrap();
        assert_eq!(result.owner, AccountId32::from([2; 32]));
        assert_eq!(result.historical_hash, original.historical_hash);
        assert_eq!(result.opaque, original.opaque);
        assert_eq!(result.amount, 123);
        assert_eq!(accounts, BTreeSet::from([[1; 32]]));
    }
}
