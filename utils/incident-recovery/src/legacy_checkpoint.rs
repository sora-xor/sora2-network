//! Exact removed storage codecs, opt-in and bound to the executed checkpoint
//! metadata. Source revisions and semantic decisions live beside the schema.
use super::{Descriptor, Hasher, MetadataSchema};
use crate::{hex_bytes, Result};
use codec::{Decode, Encode};
use scale_info::{
    form::PortableForm, Field, Path, PortableType, Type, TypeDef, TypeDefComposite,
    TypeDefSequence, TypeDefTuple, TypeDefVariant, Variant,
};
use serde::Deserialize;

pub(super) struct PreimageEnvelope {
    pub provider: Option<(usize, [u8; 32])>,
    pub summary: serde_json::Value,
}
pub(super) fn validate_marker(descriptor: &Descriptor, value: &[u8]) -> Result<()> {
    if descriptor.trie == "top" && descriptor.name == "Staking.TimeSinceGenesis" {
        let mut input = value;
        let duration = std::time::Duration::decode(&mut input).map_err(|e| e.to_string())?;
        if !input.is_empty() || duration.encode() != value {
            return Err("legacy staking Duration did not roundtrip exactly".into());
        }
    }
    Ok(())
}
/// Only the outer deposit envelope is inspected. Opaque historical call bytes
/// are never interpreted as a current RuntimeCall, even when very large.
pub(super) fn preimage(
    schema: &MetadataSchema,
    descriptor: &Descriptor,
    value: &[u8],
) -> Result<Option<PreimageEnvelope>> {
    if descriptor.trie != "top" || descriptor.name != "Democracy.Preimages" {
        return Ok(None);
    }
    if schema.metadata_hash != "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8"
        || !schema
            .registry
            .resolve(descriptor.value_type)
            .is_some_and(|t| {
                t.path
                    .segments
                    .last()
                    .is_some_and(|p| p == "LegacyDemocracyPreimageStatus")
            })
    {
        return Err("legacy preimage codec is not source-pinned".into());
    }
    if value.is_empty() || value.len() > 64 * 1024 * 1024 {
        return Err("legacy preimage envelope length exceeds bound".into());
    }
    if value[0] == 0 {
        if value.len() != 5 {
            return Err("legacy Missing preimage has invalid width".into());
        }
        return Ok(Some(PreimageEnvelope {
            provider: None,
            summary: serde_json::json!({"variant":"Missing","expiry":u32::from_le_bytes(value[1..5].try_into().map_err(|_|"expiry width")?)}),
        }));
    }
    if value[0] != 1 {
        return Err("legacy preimage variant is invalid".into());
    }
    let mut input = &value[1..];
    let length = codec::Compact::<u32>::decode(&mut input).map_err(|e| e.to_string())?;
    let encoded_length = length.encode();
    let offset = 1 + encoded_length.len() + length.0 as usize;
    if value.len() - 1 - input.len() != encoded_length.len()
        || !value[1..].starts_with(&encoded_length)
        || offset > value.len()
    {
        return Err("legacy preimage compact length is invalid".into());
    }
    let suffix = &value[offset..];
    if !((suffix.len() == 53 && suffix[52] == 0) || (suffix.len() == 57 && suffix[52] == 1)) {
        return Err("legacy preimage provider/deposit/expiry envelope differs".into());
    }
    let provider: [u8; 32] = suffix[..32].try_into().map_err(|_| "provider width")?;
    let deposit = u128::from_le_bytes(suffix[32..48].try_into().map_err(|_| "deposit width")?);
    let since = u32::from_le_bytes(suffix[48..52].try_into().map_err(|_| "since width")?);
    let expiry = if suffix[52] == 0 {
        None
    } else {
        Some(u32::from_le_bytes(
            suffix[53..57].try_into().map_err(|_| "expiry width")?,
        ))
    };
    Ok(Some(PreimageEnvelope {
        provider: Some((offset, provider)),
        summary: serde_json::json!({"variant":"Available","opaqueCallBytes":length.0,"provider":format!("0x{}",hex::encode(provider)),"deposit":deposit.to_string(),"since":since,"expiry":expiry}),
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Schema {
    metadata_blake2: String,
    original_type_count: usize,
    types: Vec<Definition>,
    storage: Vec<Storage>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Storage {
    trie: String,
    name: String,
    prefix: String,
    key_types: Vec<u32>,
    hashers: Vec<Hasher>,
    value_type: u32,
}
#[derive(Deserialize)]
struct Definition {
    id: u32,
    name: String,
    #[serde(flatten)]
    shape: Shape,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Shape {
    Tuple { fields: Vec<u32> },
    Sequence { r#type: u32 },
    Composite { fields: Vec<SourceField> },
    Variant { variants: Vec<SourceVariant> },
}
#[derive(Deserialize)]
struct SourceField {
    name: Option<String>,
    r#type: u32,
}
#[derive(Deserialize)]
struct SourceVariant {
    name: String,
    index: u8,
    fields: Vec<SourceField>,
}
fn fields(source: Vec<SourceField>) -> Vec<Field<PortableForm>> {
    source
        .into_iter()
        .map(|f| Field::new(f.name, f.r#type.into(), None, vec![]))
        .collect()
}

pub(super) fn apply(schema: &MetadataSchema) -> Result<MetadataSchema> {
    // Do not reuse these legacy field layouts against another metadata registry.
    let source: Schema = serde_json::from_str(include_str!(
        "../../../misc/incident_recovery/checkpoint_legacy_schema.json"
    ))
    .map_err(|e| e.to_string())?;
    if schema.metadata_hash != source.metadata_blake2
        || schema.registry.types.len() != source.original_type_count
    {
        return Err("legacy schema requires exact executed checkpoint metadata".into());
    }
    let mut result = schema.clone();
    for definition in source.types {
        if definition.id as usize != result.registry.types.len() {
            return Err("legacy type IDs are not contiguous".into());
        }
        let shape: TypeDef<PortableForm> = match definition.shape {
            Shape::Tuple { fields } => {
                TypeDefTuple::new_portable(fields.into_iter().map(Into::into)).into()
            }
            Shape::Sequence { r#type } => TypeDefSequence::new(r#type.into()).into(),
            Shape::Composite { fields: f } => TypeDefComposite::new(fields(f)).into(),
            Shape::Variant { variants } => TypeDefVariant::new(
                variants
                    .into_iter()
                    .map(|v| Variant::new(v.name, fields(v.fields), v.index, vec![])),
            )
            .into(),
        };
        let ty = Type::new(
            Path {
                segments: vec!["sora_nexus_checkpoint_legacy".into(), definition.name],
            },
            vec![],
            shape,
            vec![],
        );
        result
            .registry
            .types
            .push(PortableType::new(definition.id, ty));
    }
    for storage in source.storage {
        let extra = storage;
        result.descriptors.push(Descriptor {
            trie: extra.trie,
            name: extra.name,
            prefix: hex_bytes(&extra.prefix)?,
            key_types: extra.key_types,
            hashers: extra.hashers,
            value_type: extra.value_type,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scale_info::{meta_type, PortableRegistry, Registry, TypeInfo};
    #[allow(dead_code)]
    #[derive(TypeInfo)]
    enum LegacyDemocracyPreimageStatus {
        Missing(u32),
        Available {
            data: Vec<u8>,
            provider: sp_core::crypto::AccountId32,
            deposit: u128,
            since: u32,
            expiry: Option<u32>,
        },
    }
    #[test]
    fn large_legacy_preimage_still_exposes_depositor_and_rejects_malformed_suffix() {
        let mut registry = Registry::new();
        let id = registry
            .register_type(&meta_type::<LegacyDemocracyPreimageStatus>())
            .id;
        let schema = MetadataSchema {
            registry: PortableRegistry::from(registry),
            descriptors: vec![],
            metadata_hash: "0x0c1d4bf5c3bc5037ad854932443c94faae6c327ecf1f38b3e77d38668cc2b4b8"
                .into(),
        };
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Democracy.Preimages".into(),
            prefix: vec![1; 32],
            key_types: vec![],
            hashers: vec![],
            value_type: id,
        };
        let data = vec![9u8; 1024 * 1024 + 1];
        let mut encoded = vec![1];
        encoded.extend(data.encode());
        encoded.extend([7u8; 32].encode());
        encoded.extend(19u128.encode());
        encoded.extend(100u32.encode());
        encoded.extend(Option::<u32>::None.encode());
        let decoded = preimage(&schema, &descriptor, &encoded).unwrap().unwrap();
        assert_eq!(decoded.provider.unwrap().1, [7; 32]);
        assert_eq!(decoded.summary["deposit"], "19");
        assert!(preimage(&schema, &descriptor, &encoded[..encoded.len() - 1]).is_err());
        *encoded.last_mut().unwrap() = 3;
        assert!(preimage(&schema, &descriptor, &encoded).is_err());
    }
    #[test]
    fn legacy_duration_rejects_noncanonical_nanoseconds() {
        let descriptor = Descriptor {
            trie: "top".into(),
            name: "Staking.TimeSinceGenesis".into(),
            prefix: vec![1; 32],
            key_types: vec![],
            hashers: vec![],
            value_type: 0,
        };
        assert!(validate_marker(&descriptor, &(123u64, 999_999_999u32).encode()).is_ok());
        assert!(validate_marker(&descriptor, &(123u64, 1_000_000_000u32).encode()).is_err());
    }
}
