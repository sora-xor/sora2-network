//! Public consensus inputs derived from the root-verified checkpoint state.

use crate::Result;
use serde_json::{json, Value};
use sp_core::{crypto::ByteArray, storage::Storage};
use std::collections::BTreeSet;

pub fn report(storage: &Storage) -> Result<Value> {
    use framenode_runtime::incident_recovery_consensus::{checkpoint_consensus, encode_operators};
    let mut ext = sp_io::TestExternalities::new(storage.clone());
    ext.execute_with(|| {
        let before = sp_io::storage::root(sp_core::storage::StateVersion::V0);
        let value = checkpoint_consensus().map_err(str::to_owned)?;
        let authorities = value.authorities.iter().map(|(key, weight)| {
            json!([format!("0x{}", hex::encode(key.as_slice())), weight])
        }).collect::<Vec<_>>();
        let approved = value.operators.iter().flat_map(|row| &row.grandpa_keys)
            .map(|key| format!("0x{}", hex::encode(key.as_slice())))
            .collect::<BTreeSet<_>>();
        let operators = encode_operators(&value.operators);
        if sp_io::storage::root(sp_core::storage::StateVersion::V0) != before {
            return Err("checkpoint consensus derivation changed storage".into());
        }
        Ok(json!({
            "schema": "sora-nexus-checkpoint-consensus-v2",
            "checkpointHash": format!("0x{}", hex::encode(common::recovery::CHECKPOINT_HASH)),
            "checkpointNumber": common::recovery::CHECKPOINT_NUMBER,
            "operatorsScale": format!("0x{}", hex::encode(&operators)),
            "operatorsBlake2": format!("0x{}", hex::encode(sp_io::hashing::blake2_256(&operators))),
            "previousGrandpaSetId": value.previous_set,
            "previousGrandpaAuthorities": authorities,
            "grandpaAuthorities": authorities,
            "approvedGrandpaKeys": approved,
            "checkpointSlot": value.slot,
            "checkpointCouncil": value.council.iter().map(|account| format!("0x{}", hex::encode(account.as_ref() as &[u8]))).collect::<Vec<_>>(),
            "signingKeysPreserved": true,
            "manualOperatorApprovalRequired": false,
            "productionReady": false
        }))
    })
}
