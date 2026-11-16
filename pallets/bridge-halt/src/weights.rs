// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Weights for bridge-halt.
//!
//! These are hand-set upper bounds, not benchmark output. Each call reads and
//! writes at most two small items. Regenerate on reference hardware with:
//!
//! ```text
//! framenode benchmark pallet --chain=local --steps=50 --repeat=20 \
//!     --pallet=bridge_halt --extrinsic='*' --wasm-execution=compiled \
//!     --header=./misc/file_header.txt --template=./misc/pallet-weight-template.hbs \
//!     --output=./pallets/bridge-halt/src/weights.rs
//! ```

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use core::marker::PhantomData;
use frame_support::{traits::Get, weights::{constants::RocksDbWeight, Weight}};

/// Weight functions needed for bridge-halt.
pub trait WeightInfo {
	fn halt() -> Weight;
	fn add_guardian() -> Weight;
	fn remove_guardian() -> Weight;
}

/// Weights for bridge-halt using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: BridgeHalt Guardians (r:1 w:0), max 1_029 bytes
	/// Storage: BridgeHalt LastHalt (r:1 w:1), max 304 bytes
	/// Storage: unhashed `:sora-nexus:bridge-halt:latest:v1` (r:0 w:1), 8 bytes
	/// Admission (`validate_halt`, in validation and again before dispatch) reads
	/// Guardians and LastHalt twice more; counted here (r:4) because no
	/// transaction-extension weight covers them.
	fn halt() -> Weight {
		Weight::from_parts(40_000_000, 8_000)
			.saturating_add(T::DbWeight::get().reads(6_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: BridgeHalt Guardians (r:1 w:1), max 1_029 bytes
	fn add_guardian() -> Weight {
		Weight::from_parts(30_000_000, 3_000)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: BridgeHalt Guardians (r:1 w:1), max 1_029 bytes
	fn remove_guardian() -> Weight {
		Weight::from_parts(30_000_000, 3_000)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn halt() -> Weight {
		Weight::from_parts(40_000_000, 8_000)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	fn add_guardian() -> Weight {
		Weight::from_parts(30_000_000, 3_000)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	fn remove_guardian() -> Weight {
		Weight::from_parts(30_000_000, 3_000)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
