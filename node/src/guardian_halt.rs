// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Restarting this node's HASHI bridge workers after a guardian halt.
//!
//! A durable node-local observation remains stopped through reorgs, disappearance
//! of the chain signal and automatic daemon restarts. DevOps releases the exact
//! observed epoch using --bridge-resume-after-observed-halt <digest>. No on-chain
//! transaction or governance is needed, and the flag never enables OCW mode.

pub(crate) mod observed;
pub(crate) mod worker_guard;

use crate::service::{FullBackend, FullClient};
use codec::DecodeAll;
use common::bridge_halt::{GuardianHalt, GUARDIAN_HALT_STORAGE_KEY};
use sc_client_api::StorageProvider;
use sp_core::storage::StorageKey;

pub(crate) fn halt_at(
    client: &FullClient,
    hash: sp_core::H256,
) -> Result<Option<GuardianHalt>, String> {
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

/// Start the durable local observer before constructing any worker. Numeric
/// flags are retained as idempotent compatibility checks, never fresh authority.
pub(crate) fn on_startup(
    client: std::sync::Arc<FullClient>,
    backend: std::sync::Arc<FullBackend>,
    numeric: Option<u32>,
    requested: Option<sp_core::H256>,
) -> (
    std::sync::Arc<observed::Monitor>,
    impl std::future::Future<Output = ()> + Send + 'static,
) {
    observed::start(client, backend, numeric, requested)
}
