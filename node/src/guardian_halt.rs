// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Restarting this node's HASHI bridge workers after a guardian halt.
//!
//! A guardian halt stops the HASHI offchain workers on every bridge node. They
//! stay stopped until the node's operator confirms that halt by restarting the
//! node with `--bridge-resume-after-halt <number>`. The node checks the number
//! against its finalized chain and records the confirmed halt in offchain
//! storage, where the workers read it. The next halt stops them again, so the
//! flag can stay in a service definition.

pub(crate) mod worker_guard;

use crate::service::{FullBackend, FullClient};
use codec::{DecodeAll, Encode};
use common::bridge_halt::{GuardianHalt, GUARDIAN_HALT_STORAGE_KEY};
use framenode_runtime::eth_bridge::{STORAGE_PEER_MARKER_KEY, STORAGE_RESUMED_AFTER_HALT_KEY};
use sc_client_api::{Backend, StorageProvider};
use sp_blockchain::HeaderBackend;
use sp_core::offchain::OffchainStorage;
use sp_core::storage::StorageKey;
use sp_runtime::offchain::STORAGE_PREFIX;

/// The newest guardian halt in this node's finalized state.
fn finalized_halt(client: &FullClient) -> Result<Option<GuardianHalt>, String> {
    let info = client.info();
    halt_at(client, info.finalized_hash).map_err(|error| {
        format!(
            "cannot read finalized block #{}: {error}",
            info.finalized_number
        )
    })
}

fn halt_at(client: &FullClient, hash: sp_core::H256) -> Result<Option<GuardianHalt>, String> {
    let value = client
        .storage(hash, &StorageKey(GUARDIAN_HALT_STORAGE_KEY.to_vec()))
        .map_err(|error| error.to_string())?;
    value
        .map(|data| {
            GuardianHalt::decode_all(&mut &data.0[..])
                .map_err(|error| format!("the guardian halt record does not decode: {error}"))
        })
        .transpose()
}

/// Checks the operator's `--bridge-resume-after-halt` number against the newest
/// finalized halt. Only that halt can be confirmed: an older number would leave
/// the workers stopped anyway, and a number the chain has not reached yet must
/// not cover a halt that has not happened.
pub(crate) fn confirm(
    requested: u32,
    finalized: &Result<Option<GuardianHalt>, String>,
) -> Result<GuardianHalt, String> {
    const SYNCING: &str =
        "If the node is still syncing, restart it with the flag once it has caught up.";
    match finalized {
        Err(error) => Err(error.clone()),
        Ok(_) if requested == 0 => Err("guardian halts are numbered from 1.".into()),
        Ok(None) => Err(format!(
            "this node's finalized chain has no guardian halt yet, so there is no halt #{requested}. {SYNCING}"
        )),
        Ok(Some(halt)) if halt.number == requested => Ok(*halt),
        Ok(Some(halt)) if requested < halt.number => Err(format!(
            "halt #{requested} is not the newest: guardian halt #{} (block #{}) came after it. Confirm #{} once it is safe.",
            halt.number, halt.block, halt.number
        )),
        Ok(Some(halt)) => Err(format!(
            "this node's finalized chain has no halt #{requested}; the newest is #{} (block #{}). {SYNCING}",
            halt.number, halt.block
        )),
    }
}

/// Applies `--bridge-resume-after-halt` and tells bridge operators at startup
/// when a guardian halt keeps their workers stopped. Never stops the node: a
/// mistyped number leaves the workers stopped and logs why.
pub(crate) fn on_startup(client: &FullClient, backend: &FullBackend, requested: Option<u32>) {
    let Some(mut storage) = backend.offchain_storage() else {
        if requested.is_some() {
            log::error!(
                "Not restarting the HASHI bridge workers: this node has no offchain storage."
            );
        }
        return;
    };
    let finalized = finalized_halt(client);
    if let Some(requested) = requested {
        match confirm(requested, &finalized) {
            Ok(halt) => {
                storage.set(STORAGE_PREFIX, STORAGE_RESUMED_AFTER_HALT_KEY, &halt.encode());
                // Warn level, like the bridge's "Peer info", so quiet log settings show it.
                log::warn!(
                    "Acknowledged guardian halt #{} (block #{}). The next halt requires a new acknowledgement.",
                    halt.number,
                    halt.block
                );
            }
            Err(reason) => log::error!(
                "Not restarting the HASHI bridge workers for --bridge-resume-after-halt {requested}: {reason}"
            ),
        }
    }

    // Only bridge peers run the workers; other validators get no warning.
    if storage
        .get(STORAGE_PREFIX, STORAGE_PEER_MARKER_KEY)
        .is_none()
    {
        if requested.is_some() {
            log::warn!(
                "HASHI peer activation marker missing. To run this node as a bridge peer, enable --offchain-worker always, install the peer signing key and eth.json, and configure the recovery peer bootstrap."
            );
        }
        return;
    }
    let confirmed = storage
        .get(STORAGE_PREFIX, STORAGE_RESUMED_AFTER_HALT_KEY)
        .and_then(|bytes| GuardianHalt::decode_all(&mut &bytes[..]).ok());
    match &finalized {
        Ok(Some(halt)) if confirmed != Some(*halt) => log::warn!(
            "Guardian halt #{} (block #{}) keeps the HASHI bridge workers stopped. Once it is safe, restart this node with --bridge-resume-after-halt {}.",
            halt.number,
            halt.block,
            halt.number
        ),
        Err(reason) => log::warn!(
            "The HASHI bridge workers stay stopped until the guardian halt can be read: {reason}"
        ),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THIRD: GuardianHalt = GuardianHalt {
        number: 3,
        block: 900,
    };

    #[test]
    fn only_the_newest_finalized_halt_can_be_confirmed() {
        assert_eq!(confirm(3, &Ok(Some(THIRD))), Ok(THIRD));
        // An older halt: the workers would stay stopped, so say which one to confirm.
        let older = confirm(2, &Ok(Some(THIRD))).unwrap_err();
        assert!(
            older.contains("#3") && older.contains("block #900"),
            "{older}"
        );
        // A number the chain has not reached must not cover a future halt.
        assert!(confirm(4, &Ok(Some(THIRD)))
            .unwrap_err()
            .contains("syncing"));
        assert!(confirm(1, &Ok(None))
            .unwrap_err()
            .contains("no guardian halt yet"));
        assert!(confirm(0, &Ok(Some(THIRD))).unwrap_err().contains("from 1"));
        assert_eq!(
            confirm(3, &Err("cannot read".into())),
            Err("cannot read".into())
        );
    }
}
