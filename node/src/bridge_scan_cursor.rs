// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use codec::DecodeAll;
use framenode_runtime::{
    eth_bridge::{eth_scan_cursor_key, DEPOSIT_TOPIC},
    NetworkId,
};
use serde::Deserialize;
use sp_core::{offchain::OffchainStorage, H160, H256};
use sp_runtime::offchain::STORAGE_PREFIX;
use std::collections::{BTreeMap, BTreeSet};

/// How to recover a missing first-unscanned Ethereum height on this node.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ScanStart {
    Genesis,
    Backup {
        sha256: H256,
    },
    /// A new operator-audited derivation, never a recovered peer cursor.
    Reconstructed {
        sha256: H256,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CursorBackup {
    format: String,
    pub genesis_hash: H256,
    pub source_number: u32,
    pub source_hash: H256,
    next_heights: BTreeMap<NetworkId, u64>,
}

/// Evidence commitment for a newly reconstructed scan start. This authenticates
/// the operator-reviewed document and its native context, not the completeness
/// of Ethereum log providers or the correctness of the operator's reconciliation.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CursorReconstruction {
    format: String,
    pub genesis_hash: H256,
    pub audit_anchor_number: u32,
    pub audit_anchor_hash: H256,
    runtime_code_hash: H256,
    scanner: String,
    deposit_topic: H256,
    #[serde(deserialize_with = "unique_network_map")]
    pub configured_network_contracts: BTreeMap<NetworkId, H160>,
    #[serde(deserialize_with = "unique_network_map")]
    next_heights: BTreeMap<NetworkId, u64>,
    audit_manifest_sha256: H256,
}

// BTreeMap's default deserializer silently replaces duplicate keys. An audited
// document must have exactly one unambiguous value per parsed network ID.
fn unique_network_map<'de, D, V>(deserializer: D) -> Result<BTreeMap<NetworkId, V>, D::Error>
where
    D: serde::Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct NetworkMap<V>(std::marker::PhantomData<V>);

    impl<'de, V: Deserialize<'de>> serde::de::Visitor<'de> for NetworkMap<V> {
        type Value = BTreeMap<NetworkId, V>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a map with unique network IDs")
        }

        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut map: M,
        ) -> Result<Self::Value, M::Error> {
            let mut result = BTreeMap::new();
            while let Some((network, value)) = map.next_entry::<NetworkId, V>()? {
                if result.insert(network, value).is_some() {
                    return Err(serde::de::Error::custom(format!(
                        "Duplicate HASHI network ID {network}"
                    )));
                }
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(NetworkMap(std::marker::PhantomData))
}

/// Frozen storage layout for the scanner named by the reconstruction format.
/// No runtime execution or RPC is used to turn evidence into a cursor.
pub(super) fn contract_storage_key(network: NetworkId) -> Vec<u8> {
    use codec::Encode;
    let encoded = network.encode();
    [
        sp_io::hashing::twox_128(b"EthBridge").as_slice(),
        sp_io::hashing::twox_128(b"BridgeContractAddress").as_slice(),
        sp_io::hashing::twox_64(&encoded).as_slice(),
        encoded.as_slice(),
    ]
    .concat()
}

impl CursorReconstruction {
    pub(super) fn verify_history(
        &self,
        genesis: H256,
        finalized_number: u32,
        canonical_hash: Option<H256>,
    ) -> Result<(), String> {
        if self.genesis_hash != genesis
            || self.audit_anchor_number > finalized_number
            || canonical_hash != Some(self.audit_anchor_hash)
        {
            return Err(
                "HASHI reconstruction audit anchor is outside this node's finalized chain".into(),
            );
        }
        Ok(())
    }

    /// Check at both the audit anchor and the current best block. Refuse when
    /// state has been pruned or runtime/contract configuration has changed.
    pub(super) fn verify_state(
        &self,
        runtime_code_hash: Option<H256>,
        mut contract: impl FnMut(NetworkId) -> Result<Option<Vec<u8>>, String>,
    ) -> Result<(), String> {
        if runtime_code_hash != Some(self.runtime_code_hash) {
            return Err(
                "HASHI reconstruction runtime code is missing or differs from the audit".into(),
            );
        }
        for (network, expected) in &self.configured_network_contracts {
            let raw = contract(*network)?.ok_or_else(|| {
                format!("HASHI reconstruction contract is missing for network {network}")
            })?;
            let actual = H160::decode_all(&mut raw.as_slice())
                .map_err(|_| format!("Malformed HASHI contract for network {network}"))?;
            if *expected == H160::zero() || actual != *expected {
                return Err(format!(
                    "HASHI reconstruction contract differs for network {network}"
                ));
            }
        }
        Ok(())
    }
}

/// Existing well-formed cursors are authoritative. Resolve every missing cursor
/// before writing any configuration or exposing the activation marker.
pub(super) fn prepare(
    storage: &impl OffchainStorage,
    networks: impl Iterator<Item = NetworkId>,
    start: Option<&ScanStart>,
    mut load_file: impl FnMut(&str) -> Result<Vec<u8>, String>,
    verify_anchor: impl FnOnce(&CursorBackup) -> Result<(), String>,
    verify_reconstruction: impl FnOnce(&CursorReconstruction) -> Result<(), String>,
) -> Result<BTreeMap<NetworkId, u64>, String> {
    let networks: BTreeSet<_> = networks.collect();
    let mut missing = BTreeMap::new();
    for &network in &networks {
        if let Some(bytes) = storage.get(STORAGE_PREFIX, &eth_scan_cursor_key(network)) {
            u64::decode_all(&mut &bytes[..]).map_err(|_| {
                format!("Malformed retained HASHI Ethereum scan cursor for network {network}; restore the healthy offchain database before bootstrap")
            })?;
        } else {
            missing.insert(network, 0);
        }
    }
    if missing.is_empty() {
        return Ok(missing);
    }
    match start {
        Some(ScanStart::Genesis) => Ok(missing),
        Some(ScanStart::Backup { sha256 }) => {
            let bytes = load_file("eth-scan-cursors.json")?;
            if H256::from(sp_io::hashing::sha2_256(&bytes)) != *sha256 {
                return Err("HASHI scan backup SHA-256 differs from recovery_scan_start".into());
            }
            let backup: CursorBackup = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Invalid HASHI scan backup: {e}"))?;
            if backup.format != "hashi-scan-cursors-v1" {
                return Err("Unrecognized HASHI scan backup format".into());
            }
            verify_anchor(&backup)?;
            for (network, height) in &mut missing {
                *height = *backup.next_heights.get(network)
                    .ok_or_else(|| format!("HASHI scan backup lacks network {network}"))?;
            }
            Ok(missing)
        }
        Some(ScanStart::Reconstructed { sha256 }) => {
            let bytes = load_file("eth-scan-reconstruction.json")?;
            if H256::from(sp_io::hashing::sha2_256(&bytes)) != *sha256 {
                return Err("HASHI reconstruction SHA-256 differs from recovery_scan_start".into());
            }
            let reconstruction: CursorReconstruction = serde_json::from_slice(&bytes)
                .map_err(|e| format!("Invalid HASHI scan reconstruction: {e}"))?;
            if reconstruction.format != "hashi-scan-reconstruction-v1"
                || reconstruction.scanner != "eth-bridge-main-contract-deposits-v1"
                || reconstruction.deposit_topic != DEPOSIT_TOPIC
            {
                return Err("Unrecognized HASHI reconstruction format or scanner scope".into());
            }
            if reconstruction.next_heights.keys().copied().collect::<BTreeSet<_>>() != networks
                || reconstruction.configured_network_contracts.keys().copied().collect::<BTreeSet<_>>() != networks
            {
                return Err("HASHI reconstruction must bind exactly the configured networks".into());
            }
            let audit = load_file("eth-scan-reconstruction-audit.json")?;
            if H256::from(sp_io::hashing::sha2_256(&audit)) != reconstruction.audit_manifest_sha256 {
                return Err("HASHI reconstruction audit manifest SHA-256 differs".into());
            }
            let audit: serde_json::Value = serde_json::from_slice(&audit)
                .map_err(|e| format!("Invalid HASHI reconstruction audit manifest: {e}"))?;
            if !audit.as_object().is_some_and(|object| !object.is_empty()) {
                return Err("HASHI reconstruction audit manifest must be a nonempty JSON object".into());
            }
            verify_reconstruction(&reconstruction)?;
            for (network, height) in &mut missing {
                *height = reconstruction.next_heights[network];
            }
            Ok(missing)
        }
        None => Err(format!(
            "Missing HASHI Ethereum scan cursor for networks {:?}; configure recovery_scan_start in bridge/eth.json with a trusted pre-outage backup, an explicit audited reconstruction, or genesis replay",
            missing.keys().collect::<Vec<_>>()
        )),
    }
}

impl CursorBackup {
    pub(super) fn verify_history(
        &self,
        genesis: H256,
        finalized_number: u32,
        canonical_hash: Option<H256>,
    ) -> Result<(), String> {
        if self.genesis_hash != genesis
            || self.source_number > finalized_number
            || canonical_hash != Some(self.source_hash)
        {
            return Err("HASHI scan backup is outside this node's finalized chain; sync through its anchor and verify the backup".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::Encode;
    use sp_core::offchain::storage::InMemOffchainStorage;

    fn backup() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "format": "hashi-scan-cursors-v1",
            "genesisHash": H256::repeat_byte(1),
            "sourceNumber": 20, "sourceHash": H256::repeat_byte(2),
            "nextHeights": {"0": 5, "42": 17}
        }))
        .unwrap()
    }

    fn anchored(backup: &CursorBackup) -> Result<(), String> {
        backup.verify_history(H256::repeat_byte(1), 30, Some(H256::repeat_byte(2)))
    }

    #[test]
    fn restores_only_missing_networks_from_a_verified_backup() {
        let data = backup();
        let policy = ScanStart::Backup {
            sha256: H256::from(sp_io::hashing::sha2_256(&data)),
        };
        let mut db = InMemOffchainStorage::default();
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(0), &3u64.encode());
        assert_eq!(
            prepare(
                &db,
                [0, 42].into_iter(),
                Some(&policy),
                |_| Ok(data.clone()),
                anchored,
                |_| unreachable!(),
            )
            .unwrap(),
            [(42, 17)].into_iter().collect()
        );
        assert_eq!(
            prepare(
                &db,
                [0].into_iter(),
                None,
                |_| unreachable!(),
                |_| unreachable!(),
                |_| unreachable!(),
            )
            .unwrap(),
            BTreeMap::new()
        );
        assert_eq!(
            prepare(
                &db,
                [0, 42].into_iter(),
                Some(&ScanStart::Genesis),
                |_| unreachable!(),
                |_| unreachable!(),
                |_| unreachable!(),
            )
            .unwrap(),
            [(42, 0)].into_iter().collect()
        );
    }

    #[test]
    fn rejects_missing_unreadable_and_untrusted_cursors_before_writing() {
        let data = backup();
        let policy = ScanStart::Backup {
            sha256: H256::from(sp_io::hashing::sha2_256(&data)),
        };
        let mut db = InMemOffchainStorage::default();
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(0), &3u64.encode());
        assert!(prepare(
            &db,
            [0, 42, 43].into_iter(),
            Some(&policy),
            |_| Ok(data.clone()),
            anchored,
            |_| unreachable!(),
        )
        .is_err());
        assert!(prepare(
            &db,
            [42].into_iter(),
            None,
            |_| unreachable!(),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        assert!(prepare(
            &db,
            [42].into_iter(),
            Some(&policy),
            |_| Err("missing file".into()),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        assert!(prepare(
            &db,
            [42].into_iter(),
            Some(&policy),
            |_| Ok(vec![]),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        let malformed = b"not JSON".to_vec();
        let malformed_policy = ScanStart::Backup {
            sha256: sp_io::hashing::sha2_256(&malformed).into(),
        };
        assert!(prepare(
            &db,
            [42].into_iter(),
            Some(&malformed_policy),
            |_| Ok(malformed.clone()),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        let verify = |genesis, number, hash| {
            prepare(
                &db,
                [42].into_iter(),
                Some(&policy),
                |_| Ok(data.clone()),
                |b| b.verify_history(genesis, number, hash),
                |_| unreachable!(),
            )
        };
        assert!(verify(H256::repeat_byte(9), 30, Some(H256::repeat_byte(2))).is_err());
        assert!(verify(H256::repeat_byte(1), 19, Some(H256::repeat_byte(2))).is_err());
        assert!(verify(H256::repeat_byte(1), 30, Some(H256::repeat_byte(9))).is_err());
        assert!(verify(H256::repeat_byte(1), 30, None).is_err());
        assert_eq!(
            db.get(STORAGE_PREFIX, &eth_scan_cursor_key(0)),
            Some(3u64.encode())
        );
        assert_eq!(db.get(STORAGE_PREFIX, &eth_scan_cursor_key(42)), None);
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(42), &[0; 9]);
        assert!(prepare(
            &db,
            [42].into_iter(),
            Some(&ScanStart::Genesis),
            |_| unreachable!(),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        assert_eq!(
            db.get(STORAGE_PREFIX, &eth_scan_cursor_key(42)),
            Some(vec![0; 9])
        );
    }

    const AUDIT: &[u8] = br#"{"origin":"operator-reviewed reconstruction","limitations":["provider-observed log coverage"]}"#;

    fn reconstruction() -> serde_json::Value {
        serde_json::json!({
            "format": "hashi-scan-reconstruction-v1",
            "genesisHash": H256::repeat_byte(1),
            "auditAnchorNumber": 20,
            "auditAnchorHash": H256::repeat_byte(2),
            "runtimeCodeHash": H256::repeat_byte(3),
            "scanner": "eth-bridge-main-contract-deposits-v1",
            "depositTopic": DEPOSIT_TOPIC,
            "configuredNetworkContracts": {"0": H160::repeat_byte(4), "42": H160::repeat_byte(5)},
            "nextHeights": {"0": 5, "42": 17},
            "auditManifestSha256": H256::from(sp_io::hashing::sha2_256(AUDIT))
        })
    }

    fn verified_reconstruction(reconstruction: &CursorReconstruction) -> Result<(), String> {
        reconstruction.verify_history(H256::repeat_byte(1), 30, Some(H256::repeat_byte(2)))?;
        reconstruction.verify_state(Some(H256::repeat_byte(3)), |network| {
            Ok(Some(
                H160::repeat_byte(if network == 0 { 4 } else { 5 }).encode(),
            ))
        })
    }

    fn prepare_reconstruction(
        db: &impl OffchainStorage,
        document: serde_json::Value,
        audit: Option<&[u8]>,
    ) -> Result<BTreeMap<NetworkId, u64>, String> {
        let bytes = serde_json::to_vec(&document).unwrap();
        prepare_reconstruction_bytes(db, &bytes, audit)
    }

    fn prepare_reconstruction_bytes(
        db: &impl OffchainStorage,
        bytes: &[u8],
        audit: Option<&[u8]>,
    ) -> Result<BTreeMap<NetworkId, u64>, String> {
        prepare(
            db,
            [0, 42].into_iter(),
            Some(&ScanStart::Reconstructed {
                sha256: sp_io::hashing::sha2_256(&bytes).into(),
            }),
            |filename| match filename {
                "eth-scan-reconstruction.json" => Ok(bytes.to_vec()),
                "eth-scan-reconstruction-audit.json" => {
                    audit.map(Vec::from).ok_or("missing audit".into())
                }
                _ => unreachable!("unexpected evidence path"),
            },
            |_| unreachable!("reconstruction must not use backup verification"),
            verified_reconstruction,
        )
    }

    #[test]
    fn audited_reconstruction_only_resolves_missing_cursors_without_writes() {
        let mut db = InMemOffchainStorage::default();
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(0), &3u64.encode());
        assert_eq!(
            prepare_reconstruction(&db, reconstruction(), Some(AUDIT)).unwrap(),
            [(42, 17)].into_iter().collect()
        );
        assert_eq!(
            db.get(STORAGE_PREFIX, &eth_scan_cursor_key(0)),
            Some(3u64.encode())
        );
        assert_eq!(db.get(STORAGE_PREFIX, &eth_scan_cursor_key(42)), None);
    }

    #[test]
    fn reconstructed_mode_preserves_complete_and_rejects_malformed_existing_cursors() {
        let mut db = InMemOffchainStorage::default();
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(0), &3u64.encode());
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(42), &4u64.encode());
        assert!(prepare(
            &db,
            [0, 42].into_iter(),
            Some(&ScanStart::Reconstructed {
                sha256: H256::zero()
            }),
            |_| unreachable!(),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .unwrap()
        .is_empty());
        db.set(STORAGE_PREFIX, &eth_scan_cursor_key(42), &[0; 9]);
        assert!(prepare_reconstruction(&db, reconstruction(), Some(AUDIT)).is_err());
        assert_eq!(
            db.get(STORAGE_PREFIX, &eth_scan_cursor_key(42)),
            Some(vec![0; 9])
        );
    }

    #[test]
    fn reconstructed_mode_requires_exact_document_and_audit_bytes() {
        let db = InMemOffchainStorage::default();
        let bytes = serde_json::to_vec(&reconstruction()).unwrap();
        assert!(prepare(
            &db,
            [0, 42].into_iter(),
            Some(&ScanStart::Reconstructed {
                sha256: H256::zero()
            }),
            |_| Ok(bytes.clone()),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        assert!(prepare_reconstruction(&db, reconstruction(), None).is_err());
        assert!(prepare_reconstruction(&db, reconstruction(), Some(b"{} ")).is_err());
        for invalid in [b"{}".as_slice(), b"[]", b"null", b"not json"] {
            let mut document = reconstruction();
            document["auditManifestSha256"] =
                serde_json::json!(H256::from(sp_io::hashing::sha2_256(invalid)));
            assert!(prepare_reconstruction(&db, document, Some(invalid)).is_err());
        }
    }

    #[test]
    fn reconstructed_mode_rejects_wrong_scope_schema_and_networks() {
        let db = InMemOffchainStorage::default();
        let mut invalid = Vec::new();
        for (field, value) in [
            ("format", serde_json::json!("hashi-scan-cursors-v1")),
            ("scanner", serde_json::json!("all-ethereum-events")),
            ("depositTopic", serde_json::json!(H256::zero())),
            ("nextHeights", serde_json::json!({"0":5})),
            ("nextHeights", serde_json::json!({"0":5,"42":17,"43":18})),
            ("nextHeights", serde_json::json!({"0":5,"42":-1})),
            (
                "configuredNetworkContracts",
                serde_json::json!({"0":H160::repeat_byte(4)}),
            ),
            ("unexpected", serde_json::json!(true)),
        ] {
            let mut document = reconstruction();
            document[field] = value;
            invalid.push(document);
        }
        let mut missing_origin = reconstruction();
        missing_origin
            .as_object_mut()
            .unwrap()
            .remove("auditManifestSha256");
        invalid.push(missing_origin);
        for document in invalid {
            assert!(prepare_reconstruction(&db, document, Some(AUDIT)).is_err());
        }
        for network in [0, 42] {
            assert_eq!(db.get(STORAGE_PREFIX, &eth_scan_cursor_key(network)), None);
        }
        assert_eq!(
            db.get(
                STORAGE_PREFIX,
                framenode_runtime::eth_bridge::STORAGE_PEER_MARKER_KEY
            ),
            None
        );
    }

    #[test]
    fn reconstructed_and_backup_documents_are_not_interchangeable() {
        let db = InMemOffchainStorage::default();
        let reconstructed = serde_json::to_vec(&reconstruction()).unwrap();
        assert!(prepare(
            &db,
            [0, 42].into_iter(),
            Some(&ScanStart::Backup {
                sha256: sp_io::hashing::sha2_256(&reconstructed).into()
            }),
            |_| Ok(reconstructed.clone()),
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .is_err());
        assert!(prepare_reconstruction(
            &db,
            serde_json::from_slice(&backup()).unwrap(),
            Some(AUDIT)
        )
        .is_err());
        let parsed: ScanStart = serde_json::from_value(serde_json::json!({
            "reconstructed": {"sha256": H256::repeat_byte(7)}
        }))
        .unwrap();
        assert!(
            matches!(parsed, ScanStart::Reconstructed { sha256 } if sha256 == H256::repeat_byte(7))
        );
    }

    #[test]
    fn reconstructed_mode_requires_canonical_finalized_anchor_and_runtime() {
        let db = InMemOffchainStorage::default();
        for (field, value) in [
            ("genesisHash", serde_json::json!(H256::repeat_byte(8))),
            ("auditAnchorNumber", serde_json::json!(31)),
            ("auditAnchorHash", serde_json::json!(H256::repeat_byte(8))),
            ("runtimeCodeHash", serde_json::json!(H256::repeat_byte(8))),
        ] {
            let mut document = reconstruction();
            document[field] = value;
            assert!(prepare_reconstruction(&db, document, Some(AUDIT)).is_err());
        }
        let document: CursorReconstruction = serde_json::from_value(reconstruction()).unwrap();
        assert!(document
            .verify_history(H256::repeat_byte(1), 30, None)
            .is_err());
        assert!(document.verify_state(None, |_| unreachable!()).is_err());
    }

    #[test]
    fn reconstructed_mode_rejects_changed_missing_and_malformed_contract_state() {
        let document: CursorReconstruction = serde_json::from_value(reconstruction()).unwrap();
        for value in [None, Some(vec![0; 21]), Some(H160::repeat_byte(9).encode())] {
            assert!(document
                .verify_state(Some(H256::repeat_byte(3)), |_| Ok(value.clone()))
                .is_err());
        }
        assert!(document
            .verify_state(Some(H256::repeat_byte(3)), |_| Err("pruned state".into()))
            .is_err());
        let mut zero = reconstruction();
        zero["configuredNetworkContracts"]["0"] = serde_json::json!(H160::zero());
        let zero: CursorReconstruction = serde_json::from_value(zero).unwrap();
        assert!(zero
            .verify_state(Some(H256::repeat_byte(3)), |_| Ok(Some(
                H160::zero().encode()
            )))
            .is_err());
    }

    #[test]
    fn reconstructed_contract_storage_key_matches_scanner_layout() {
        let actual: String = contract_storage_key(0)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, "8e8f4d4777099074419cbb4597dac1d35f273d84158fdb601f0b6e26e58596f6b4def25cfda6ef3a00000000");
    }

    #[test]
    fn reconstructed_mode_rejects_duplicate_network_ids_in_raw_json() {
        let db = InMemOffchainStorage::default();
        let document = reconstruction();
        let raw = serde_json::to_string(&document).unwrap();
        for field in ["nextHeights", "configuredNetworkContracts"] {
            let needle = format!("\"{field}\":{}", document[field]);
            let original = document[field]["0"].clone();
            let different = if field == "nextHeights" {
                serde_json::json!(99999999)
            } else {
                serde_json::json!(H160::repeat_byte(6))
            };
            for duplicate in [original.clone(), different] {
                for duplicate_key in ["0", "00"] {
                    let replacement = format!(
                        "\"{field}\":{{\"0\":{original},\"{duplicate_key}\":{duplicate},\"42\":{}}}",
                        document[field]["42"]
                    );
                    let malformed = raw.replace(&needle, &replacement);
                    assert_ne!(raw, malformed);
                    assert!(
                        prepare_reconstruction_bytes(&db, malformed.as_bytes(), Some(AUDIT))
                            .is_err()
                    );
                }
            }
        }
        for network in [0, 42] {
            assert_eq!(db.get(STORAGE_PREFIX, &eth_scan_cursor_key(network)), None);
        }
    }

    #[test]
    fn reconstructed_mode_rejects_out_of_range_and_fractional_heights() {
        let db = InMemOffchainStorage::default();
        let raw = serde_json::to_string(&reconstruction()).unwrap();
        for value in [
            "18446744073709551616",
            "17.5",
            "-1",
            "1e100",
            "null",
            "\"17\"",
        ] {
            let malformed = raw.replace(
                "\"nextHeights\":{\"0\":5,\"42\":17}",
                &format!("\"nextHeights\":{{\"0\":5,\"42\":{value}}}"),
            );
            assert_ne!(raw, malformed);
            assert!(prepare_reconstruction_bytes(&db, malformed.as_bytes(), Some(AUDIT)).is_err());
        }
    }
}
