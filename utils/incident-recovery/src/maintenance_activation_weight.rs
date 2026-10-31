//! Recorded FRAME weight, decoded with the exact executing Wasm's metadata.
//! This checks configured accounting, not elapsed time or benchmark accuracy.
use super::{replay, storage_key, value, Result};
use codec::{Compact, DecodeAll, Encode};
use frame_metadata::{RuntimeMetadata, RuntimeMetadataPrefixed};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::{io::Write, path::Path};

const FILE: &str = "activation-metadata.scale";
const MAX_METADATA: usize = 16 * 1024 * 1024;
const CLASSES: [&str; 3] = ["normal", "operational", "mandatory"];

pub(super) struct Observer {
    schema: crate::inventory::MetadataSchema,
    weight_type: u32,
    pub(super) limits: Value,
}

fn weight(value: &Value) -> Result<[u64; 2]> {
    let fields = value.as_object().ok_or("weight is not a named structure")?;
    if fields.len() != 2 {
        return Err("weight fields differ".into());
    }
    let number = |name: &str| -> Result<u64> {
        let text = fields
            .get(name)
            .and_then(Value::as_str)
            .ok_or("weight component absent")?;
        let number: u64 = text.parse().map_err(|_| "invalid weight component")?;
        if number.to_string() != text {
            return Err("noncanonical weight component".into());
        }
        Ok(number)
    };
    Ok([number("ref_time")?, number("proof_size")?])
}

fn report_weight(value: [u64; 2]) -> Value {
    json!({"refTime":value[0].to_string(),"proofSize":value[1].to_string()})
}

impl Observer {
    fn metadata(bytes: &[u8], code_hash: [u8; 32]) -> Result<Self> {
        if bytes.len() > MAX_METADATA {
            return Err("activation metadata exceeds bound".into());
        }
        let schema = crate::inventory::MetadataSchema::decode(bytes)?;
        let mut input = bytes;
        let metadata =
            RuntimeMetadataPrefixed::decode_all(&mut input).map_err(|e| e.to_string())?;
        let constants: Vec<_> = match metadata.1 {
            RuntimeMetadata::V14(m) => m
                .pallets
                .into_iter()
                .map(|p| (p.name, p.constants))
                .collect(),
            RuntimeMetadata::V15(m) => m
                .pallets
                .into_iter()
                .map(|p| (p.name, p.constants))
                .collect(),
            _ => return Err("unsupported activation metadata".into()),
        };
        let mut systems = constants.into_iter().filter(|(name, _)| name == "System");
        let (_, constants) = systems.next().ok_or("System metadata missing")?;
        if systems.next().is_some() {
            return Err("duplicate System metadata".into());
        }
        let mut constants = constants.into_iter().filter(|c| c.name == "BlockWeights");
        let constant = constants.next().ok_or("System.BlockWeights missing")?;
        if constants.next().is_some() || constant.value.len() > 1024 {
            return Err("duplicate or oversized System.BlockWeights".into());
        }
        let decoded =
            crate::typed::decoded_json(&constant.value, constant.ty.id, &schema.registry)?;
        let maximum = weight(&decoded["max_block"])?;
        if maximum.contains(&0) {
            return Err("zero activation max_block component".into());
        }
        let key = storage_key(b"System", b"BlockWeight");
        let mut descriptors = schema
            .descriptors
            .iter()
            .filter(|d| d.name == "System.BlockWeight");
        let descriptor = descriptors
            .next()
            .ok_or("System.BlockWeight metadata missing")?;
        if descriptors.next().is_some()
            || descriptor.trie != "top"
            || descriptor.prefix != key
            || !descriptor.key_types.is_empty()
            || !descriptor.hashers.is_empty()
        {
            return Err("System.BlockWeight is not the exact plain storage value".into());
        }
        let weight_type = descriptor.value_type;
        let limits = json!({"schema":"sora-nexus-activation-block-weight-limits-v1",
            "executingCodeBlake2":hex::encode(code_hash),"metadataFile":FILE,
            "metadataSha256":hex::encode(Sha256::digest(bytes)),
            "metadataBlake2":"0x".to_owned()+&hex::encode(sp_io::hashing::blake2_256(bytes)),
            "constant":"System.BlockWeights","constantScale":"0x".to_owned()+&hex::encode(&constant.value),
            "maxBlock":report_weight(maximum),"storage":"System.BlockWeight",
            "storageKey":"0x".to_owned()+&hex::encode(key),
            "metadataExecutedAsWasm":true,"metadataNativeExecution":false,
            "elapsedPerformanceMeasured":false});
        Ok(Self {
            schema,
            weight_type,
            limits,
        })
    }

    pub(super) fn start(
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        output: &Path,
    ) -> Result<Self> {
        let root = replay::root(ext);
        if replay::code(ext)? != code {
            return Err(
                "activation metadata executing code differs from stored final runtime".into(),
            );
        }
        let encoded = replay::call(executor, ext, code, "Metadata_metadata", &[])?;
        if encoded.len() > MAX_METADATA + 5 {
            return Err("activation metadata API result exceeds bound".into());
        }
        let metadata = Vec::<u8>::decode_all(&mut encoded.as_slice()).map_err(|e| e.to_string())?;
        if metadata.encode() != encoded || replay::root(ext) != root {
            return Err("activation metadata call changed state or encoding".into());
        }
        let observer = Self::metadata(&metadata, sp_io::hashing::blake2_256(code))?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(FILE))
            .map_err(|e| e.to_string())?;
        file.write_all(&metadata)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(observer)
    }

    pub(super) fn observe(&self, ext: &mut sp_io::TestExternalities) -> Result<Value> {
        let raw = value(ext, &storage_key(b"System", b"BlockWeight"))
            .ok_or("persisted System.BlockWeight absent after finalization")?;
        if raw.len() > 54 {
            return Err("persisted block weight exceeds compact u64 bound".into());
        }
        let weights = <[(Compact<u64>, Compact<u64>); 3]>::decode_all(&mut raw.as_slice())
            .map_err(|e| e.to_string())?;
        if weights.encode() != raw {
            return Err("noncanonical persisted block weight".into());
        }
        let decoded = crate::typed::decoded_json(&raw, self.weight_type, &self.schema.registry)?;
        if decoded.as_object().map(|o| o.len()) != Some(3) {
            return Err("persisted weight class structure differs from metadata".into());
        }
        let mut total = [0u128; 2];
        let mut classes = serde_json::Map::new();
        for (name, (time, proof)) in CLASSES.into_iter().zip(weights) {
            let parts = [time.0, proof.0];
            if weight(&decoded[name])? != parts {
                return Err("persisted weight encoding differs from exact metadata".into());
            }
            for (total, part) in total.iter_mut().zip(parts) {
                *total += u128::from(part);
            }
            classes.insert(name.into(), report_weight(parts));
        }
        Ok(json!({"schema":"sora-nexus-recorded-block-weight-v1",
            "storageScale":"0x".to_owned()+&hex::encode(raw),"classes":classes,
            "total":{"refTime":total[0].to_string(),"proofSize":total[1].to_string()},
            "accountingOnly":true,"elapsedPerformanceMeasured":false}))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use frame_metadata::v14;
    use scale_info::{meta_type, TypeInfo};

    #[derive(Encode, TypeInfo, Clone, Copy)]
    struct Weight {
        #[codec(compact)]
        ref_time: u64,
        #[codec(compact)]
        proof_size: u64,
    }
    #[derive(Encode, TypeInfo)]
    struct PerClass<T> {
        normal: T,
        operational: T,
        mandatory: T,
    }
    #[derive(Encode, TypeInfo)]
    struct ClassLimit {
        base_extrinsic: Weight,
        max_extrinsic: Option<Weight>,
        max_total: Option<Weight>,
        reserved: Option<Weight>,
    }
    #[derive(Encode, TypeInfo)]
    struct Limits {
        base_block: Weight,
        max_block: Weight,
        per_class: PerClass<ClassLimit>,
    }

    fn metadata(maximum: u64) -> Vec<u8> {
        let weight = Weight {
            ref_time: maximum,
            proof_size: maximum,
        };
        let class = || ClassLimit {
            base_extrinsic: weight,
            max_extrinsic: None,
            max_total: None,
            reserved: None,
        };
        let limits = Limits {
            base_block: weight,
            max_block: weight,
            per_class: PerClass {
                normal: class(),
                operational: class(),
                mandatory: class(),
            },
        };
        let system = v14::PalletMetadata {
            name: "System",
            storage: Some(v14::PalletStorageMetadata {
                prefix: "System",
                entries: vec![v14::StorageEntryMetadata {
                    name: "BlockWeight",
                    modifier: v14::StorageEntryModifier::Default,
                    ty: v14::StorageEntryType::Plain(meta_type::<PerClass<Weight>>()),
                    default: vec![],
                    docs: vec![],
                }],
            }),
            calls: None,
            event: None,
            constants: vec![v14::PalletConstantMetadata {
                name: "BlockWeights",
                ty: meta_type::<Limits>(),
                value: limits.encode(),
                docs: vec![],
            }],
            error: None,
            index: 0,
        };
        let metadata = v14::RuntimeMetadataV14::new(
            vec![system],
            v14::ExtrinsicMetadata {
                ty: meta_type::<()>(),
                version: 4,
                signed_extensions: vec![],
            },
            meta_type::<u16>(),
        );
        RuntimeMetadataPrefixed(
            frame_metadata::META_RESERVED,
            RuntimeMetadata::V14(metadata),
        )
        .encode()
    }

    pub(in super::super) fn observer() -> Observer {
        Observer::metadata(&metadata(100), [2; 32]).unwrap()
    }
    pub(in super::super) fn install(ext: &mut sp_io::TestExternalities, parts: [u64; 6]) {
        let raw = parts.map(Compact).encode();
        ext.execute_with(|| sp_io::storage::set(&storage_key(b"System", b"BlockWeight"), &raw));
    }

    #[test]
    fn exact_metadata_decodes_each_class_and_preserves_root() {
        let observer = observer();
        assert_eq!(
            observer.limits["maxBlock"],
            json!({"refTime":"100","proofSize":"100"})
        );
        let mut ext = sp_io::TestExternalities::default();
        install(&mut ext, [10, 1, 20, 2, 70, 3]);
        let root = replay::root(&mut ext);
        let row = observer.observe(&mut ext).unwrap();
        assert_eq!(
            row["classes"]["mandatory"],
            json!({"refTime":"70","proofSize":"3"})
        );
        assert_eq!(row["total"], json!({"refTime":"100","proofSize":"6"}));
        assert_eq!(replay::root(&mut ext), root);
    }

    #[test]
    fn weight_sum_never_saturates_away_overflow() {
        let mut ext = sp_io::TestExternalities::default();
        install(&mut ext, [u64::MAX; 6]);
        assert_eq!(
            observer().observe(&mut ext).unwrap()["total"]["refTime"],
            (3 * u128::from(u64::MAX)).to_string()
        );
    }

    #[test]
    fn absent_corrupt_and_trailing_storage_are_rejected() {
        let observer = observer();
        let mut ext = sp_io::TestExternalities::default();
        assert!(observer.observe(&mut ext).is_err());
        for raw in [vec![], vec![0; 7], vec![1, 0, 0, 0, 0, 0, 0], vec![255; 55]] {
            ext.execute_with(|| sp_io::storage::set(&storage_key(b"System", b"BlockWeight"), &raw));
            assert!(observer.observe(&mut ext).is_err());
        }
    }

    #[test]
    fn metadata_constant_and_storage_shape_fail_closed() {
        assert!(Observer::metadata(&metadata(0), [2; 32]).is_err());
        let raw = metadata(100);
        let mut altered = RuntimeMetadataPrefixed::decode_all(&mut raw.as_slice()).unwrap();
        if let RuntimeMetadata::V14(ref mut m) = altered.1 {
            m.pallets[0].storage.as_mut().unwrap().entries[0].name = "WrongWeight".into();
        }
        assert!(Observer::metadata(&altered.encode(), [2; 32]).is_err());
        let mut extra = raw;
        extra.push(0);
        assert!(Observer::metadata(&extra, [2; 32]).is_err());
    }
}
