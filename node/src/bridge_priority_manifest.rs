//! Read the operator's local schedule of Ethereum blocks for the normal deposit scanner.

use crate::bridge_priority_scan::PriorityScanPlan;
use serde::Deserialize;
use sp_core::{H160, H256};
use std::{fs::File, io::Read, path::Path};

const MAX_BYTES: u64 = 64 * 1024;

// This production feature is compiled for the network-0 deposit contract under spec 142.
pub(crate) const MAINNET_DEPOSIT_CONTRACT: [u8; 20] = [
    0x31, 0x34, 0x16, 0x87, 0x0a, 0x4d, 0xa6, 0xf1, 0x25, 0x05, 0xa5, 0x50, 0xb6, 0x7b, 0xb7, 0x3c,
    0x8e, 0x21, 0xd5, 0xd3,
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    format: String,
    genesis_hash: H256,
    network_id: u32,
    ethereum_chain_id: u64,
    bridge_contract: H160,
    blocks: Vec<u64>,
}

pub(crate) fn load(
    path: &Path,
    expected: H256,
    genesis: H256,
    code_hash: Option<H256>,
    network_zero_contract: Option<&[u8]>,
) -> Result<PriorityScanPlan, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open {path:?}: {e}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read {path:?}: {e}"))?;
    decode(&bytes, expected, genesis, code_hash, network_zero_contract)
}

fn decode(
    bytes: &[u8],
    expected: H256,
    genesis: H256,
    code_hash: Option<H256>,
    network_zero_contract: Option<&[u8]>,
) -> Result<PriorityScanPlan, String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Priority scan manifest exceeds 64 KiB".into());
    }
    if H256::from(sp_io::hashing::sha2_256(bytes)) != expected {
        return Err("Priority scan manifest differs from the supplied SHA-256".into());
    }
    let manifest: Manifest = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid priority scan manifest: {e}"))?;
    if manifest.format != "eth-deposit-priority-blocks-v1"
        || manifest.genesis_hash != genesis
        || manifest.network_id != 0
        || manifest.ethereum_chain_id != 1
        || manifest.bridge_contract.0 != MAINNET_DEPOSIT_CONTRACT
        || network_zero_contract != Some(manifest.bridge_contract.as_bytes())
        || !crate::bridge_local_quarantine::reviewed_priority_code(genesis, code_hash)
    {
        return Err("Priority scan manifest does not match the reviewed mainnet bridge".into());
    }
    PriorityScanPlan::new(expected, manifest.blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    const GENESIS: &str = "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5";
    const CODE: &str = "d601b781566850086b9341552bcde75b43902665a01fc94897da2adf3cac9a26";
    const CONTRACT: &str = "313416870a4da6f12505a550b67bb73c8e21d5d3";

    #[test]
    fn priority_scan_manifest_binds_contents_chain_contract_and_reviewed_code() {
        let genesis = H256::from_str(GENESIS).unwrap();
        let code = Some(H256::from_str(CODE).unwrap());
        let contract = H160::from_str(CONTRACT).unwrap();
        let manifest = serde_json::json!({
            "format": "eth-deposit-priority-blocks-v1", "genesisHash": genesis,
            "networkId": 0, "ethereumChainId": 1, "bridgeContract": contract, "blocks": [13,18]
        });
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let digest = H256::from(sp_io::hashing::sha2_256(&bytes));
        assert!(decode(&bytes, digest, genesis, code, Some(contract.as_bytes())).is_ok());
        for (pin, chain, runtime, addr) in [
            (H256::zero(), genesis, code, Some(contract.as_bytes())),
            (digest, H256::zero(), code, Some(contract.as_bytes())),
            (digest, genesis, None, Some(contract.as_bytes())),
            (
                digest,
                genesis,
                Some(
                    H256::from_str(
                        "c4633932066e1d36ce307c476f76ce8a9833f2c8bc9ea97837febf52ed90da68",
                    )
                    .unwrap(),
                ),
                Some(contract.as_bytes()),
            ),
            (digest, genesis, code, None),
            (digest, genesis, code, Some(&[0u8; 20][..])),
        ] {
            assert!(decode(&bytes, pin, chain, runtime, addr).is_err());
        }
        for (field, value) in [
            ("format", serde_json::json!("unknown")),
            ("networkId", serde_json::json!(1)),
            ("ethereumChainId", serde_json::json!(5)),
            ("approve", serde_json::json!(true)),
            ("blocks", serde_json::json!([13, 13])),
        ] {
            let mut invalid = manifest.clone();
            invalid[field] = value;
            let bytes = serde_json::to_vec(&invalid).unwrap();
            let digest = H256::from(sp_io::hashing::sha2_256(&bytes));
            assert!(decode(&bytes, digest, genesis, code, Some(contract.as_bytes())).is_err());
        }
    }
}
