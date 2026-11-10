// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use codec::DecodeAll;
use framenode_runtime::{eth_bridge::eth_scan_cursor_key, NetworkId};
use serde::Deserialize;
use sp_core::{offchain::OffchainStorage, H256};
use sp_runtime::offchain::STORAGE_PREFIX;
use std::collections::BTreeMap;

/// How to recover a missing first-unscanned Ethereum height on this node.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ScanStart {
    Genesis,
    Backup { sha256: H256 },
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

/// Existing well-formed cursors are authoritative. Resolve every missing cursor
/// before writing any configuration or exposing the activation marker.
pub(super) fn prepare(
    storage: &impl OffchainStorage,
    networks: impl Iterator<Item = NetworkId>,
    start: Option<&ScanStart>,
    load_backup: impl FnOnce() -> Result<Vec<u8>, String>,
    verify_anchor: impl FnOnce(&CursorBackup) -> Result<(), String>,
) -> Result<BTreeMap<NetworkId, u64>, String> {
    let mut missing = BTreeMap::new();
    for network in networks {
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
            let bytes = load_backup()?;
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
        None => Err(format!(
            "Missing HASHI Ethereum scan cursor for networks {:?}; configure recovery_scan_start in bridge/eth.json with a trusted pre-outage backup or genesis replay",
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
                || Ok(data.clone()),
                anchored
            )
            .unwrap(),
            [(42, 17)].into_iter().collect()
        );
        assert_eq!(
            prepare(
                &db,
                [0].into_iter(),
                None,
                || unreachable!(),
                |_| unreachable!()
            )
            .unwrap(),
            BTreeMap::new()
        );
        assert_eq!(
            prepare(
                &db,
                [0, 42].into_iter(),
                Some(&ScanStart::Genesis),
                || unreachable!(),
                |_| unreachable!()
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
            || Ok(data.clone()),
            anchored
        )
        .is_err());
        assert!(prepare(
            &db,
            [42].into_iter(),
            None,
            || unreachable!(),
            |_| unreachable!()
        )
        .is_err());
        assert!(prepare(
            &db,
            [42].into_iter(),
            Some(&policy),
            || Err("missing file".into()),
            |_| unreachable!()
        )
        .is_err());
        assert!(prepare(
            &db,
            [42].into_iter(),
            Some(&policy),
            || Ok(vec![]),
            |_| unreachable!()
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
            || Ok(malformed),
            |_| unreachable!()
        )
        .is_err());
        let verify = |genesis, number, hash| {
            prepare(
                &db,
                [42].into_iter(),
                Some(&policy),
                || Ok(data.clone()),
                |b| b.verify_history(genesis, number, hash),
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
            || unreachable!(),
            |_| unreachable!()
        )
        .is_err());
        assert_eq!(
            db.get(STORAGE_PREFIX, &eth_scan_cursor_key(42)),
            Some(vec![0; 9])
        );
    }
}
