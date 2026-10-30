//! Conservative resource charges for the experimental BEEFY light client.
//!
//! Composed charges calibrated with compiled Wasmtime 36.0.20 on the operator's
//! selected Apple M4 Max reference, October 31, 2026. The measured FRAME fits and
//! all raw observations are retained separately; these charges include additional
//! margin over the observations, including shared-host outliers. See
//! `RESOURCE_BOUNDS.md` for the artifact, exact command and supported dimensions.
//! Recalibrate if the engine, reference hardware or limits change. Units are ps.

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::Weight};

pub trait WeightInfo {
    fn initialize() -> Weight;
    fn submit_signature_commitment(
        sampling_attempts: u32,
        signatures: u32,
        proof_nodes: u32,
        commitment_bytes: u32,
    ) -> Weight;
    fn verify(proof_nodes: u32, digest_logs: u32) -> Weight;
}

pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn initialize() -> Weight {
        Weight::from_parts(100_000_000, 16_384).saturating_add(T::DbWeight::get().writes(3))
    }

    fn submit_signature_commitment(
        sampling_attempts: u32,
        signatures: u32,
        proof_nodes: u32,
        commitment_bytes: u32,
    ) -> Weight {
        // Covers storage, fixed leaf hashing, bounded claims scans, and dispatch.
        Weight::from_parts(5_000_000_000, 65_536)
            .saturating_add(
                Weight::from_parts(8_000_000, 0).saturating_mul(sampling_attempts.into()),
            )
            .saturating_add(Weight::from_parts(2_600_000_000, 0).saturating_mul(signatures.into()))
            .saturating_add(Weight::from_parts(14_000_000, 0).saturating_mul(proof_nodes.into()))
            .saturating_add(Weight::from_parts(120_000, 0).saturating_mul(commitment_bytes.into()))
            .saturating_add(T::DbWeight::get().reads(9))
            .saturating_add(T::DbWeight::get().writes(6))
    }

    fn verify(proof_nodes: u32, digest_logs: u32) -> Weight {
        Weight::from_parts(500_000_000, 16_384)
            .saturating_add(Weight::from_parts(14_000_000, 0).saturating_mul(proof_nodes.into()))
            .saturating_add(Weight::from_parts(20_000_000, 0).saturating_mul(digest_logs.into()))
            .saturating_add(T::DbWeight::get().reads(2))
    }
}
