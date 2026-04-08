// This file is part of Substrate.

// Copyright (C) 2020-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Weights for pallet_multisig
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0
//! DATE: 2020-10-27, STEPS: `[50, ]`, REPEAT: 20, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 128

// Executed Command:
// target/release/framenode
// benchmark
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_multisig
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=./multisig/src/weights.rs
// --template=./.maintain/frame-weight-template.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::dispatch::{DispatchErrorWithPostInfo, DispatchResultWithPostInfo, Pays};
use frame_support::{
    traits::Get,
    weights::{constants::RocksDbWeight, Weight},
};
use sp_runtime::DispatchError;
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_multisig.
pub trait WeightInfo {
    fn as_multi_create(s: u32, z: u32) -> Weight;
    fn as_multi_approve(s: u32, z: u32) -> Weight;
    fn as_multi_complete(s: u32, z: u32) -> Weight;
}

/// Weights for pallet_multisig using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    // Storage: Multisig Multisigs (r:1 w:1)
    // Storage: unknown [0x3a65787472696e7369635f696e646578] (r:1 w:0)
    /// The range of component `s` is `[2, 100]`.
    /// The range of component `z` is `[0, 10000]`.
    fn as_multi_create(s: u32, z: u32) -> Weight {
        // Minimum execution time: 52_699 nanoseconds.
        Weight::from_parts(40_874_603 as u64, 0)
            // Standard Error: 546
            .saturating_add(Weight::from_parts(131_727 as u64, 0).saturating_mul(s as u64))
            // Standard Error: 5
            .saturating_add(Weight::from_parts(1_537 as u64, 0).saturating_mul(z as u64))
            .saturating_add(T::DbWeight::get().reads(2 as u64))
            .saturating_add(T::DbWeight::get().writes(1 as u64))
    }
    // Storage: Multisig Multisigs (r:1 w:1)
    /// The range of component `s` is `[3, 100]`.
    /// The range of component `z` is `[0, 10000]`.
    fn as_multi_approve(s: u32, z: u32) -> Weight {
        // Minimum execution time: 39_843 nanoseconds.
        Weight::from_parts(28_912_325 as u64, 0)
            // Standard Error: 734
            .saturating_add(Weight::from_parts(125_761 as u64, 0).saturating_mul(s as u64))
            // Standard Error: 7
            .saturating_add(Weight::from_parts(1_542 as u64, 0).saturating_mul(z as u64))
            .saturating_add(T::DbWeight::get().reads(1 as u64))
            .saturating_add(T::DbWeight::get().writes(1 as u64))
    }
    // Storage: Multisig Multisigs (r:1 w:1)
    // Storage: System Account (r:1 w:1)
    /// The range of component `s` is `[2, 100]`.
    /// The range of component `z` is `[0, 10000]`.
    fn as_multi_complete(s: u32, z: u32) -> Weight {
        // Minimum execution time: 54_980 nanoseconds.
        Weight::from_parts(42_087_213 as u64, 0)
            // Standard Error: 786
            .saturating_add(Weight::from_parts(153_935 as u64, 0).saturating_mul(s as u64))
            // Standard Error: 7
            .saturating_add(Weight::from_parts(1_545 as u64, 0).saturating_mul(z as u64))
            .saturating_add(T::DbWeight::get().reads(2 as u64))
            .saturating_add(T::DbWeight::get().writes(2 as u64))
    }
}

// For backwards compatibility and tests
impl WeightInfo for () {
    // Storage: Multisig Multisigs (r:1 w:1)
    // Storage: unknown [0x3a65787472696e7369635f696e646578] (r:1 w:0)
    /// The range of component `s` is `[2, 100]`.
    /// The range of component `z` is `[0, 10000]`.
    fn as_multi_create(s: u32, z: u32) -> Weight {
        // Minimum execution time: 52_699 nanoseconds.
        Weight::from_parts(40_874_603 as u64, 0)
            // Standard Error: 546
            .saturating_add(Weight::from_parts(131_727 as u64, 0).saturating_mul(s as u64))
            // Standard Error: 5
            .saturating_add(Weight::from_parts(1_537 as u64, 0).saturating_mul(z as u64))
            .saturating_add(RocksDbWeight::get().reads(2 as u64))
            .saturating_add(RocksDbWeight::get().writes(1 as u64))
    }
    // Storage: Multisig Multisigs (r:1 w:1)
    /// The range of component `s` is `[3, 100]`.
    /// The range of component `z` is `[0, 10000]`.
    fn as_multi_approve(s: u32, z: u32) -> Weight {
        // Minimum execution time: 39_843 nanoseconds.
        Weight::from_parts(28_912_325 as u64, 0)
            // Standard Error: 734
            .saturating_add(Weight::from_parts(125_761 as u64, 0).saturating_mul(s as u64))
            // Standard Error: 7
            .saturating_add(Weight::from_parts(1_542 as u64, 0).saturating_mul(z as u64))
            .saturating_add(RocksDbWeight::get().reads(1 as u64))
            .saturating_add(RocksDbWeight::get().writes(1 as u64))
    }
    // Storage: Multisig Multisigs (r:1 w:1)
    // Storage: System Account (r:1 w:1)
    /// The range of component `s` is `[2, 100]`.
    /// The range of component `z` is `[0, 10000]`.
    fn as_multi_complete(s: u32, z: u32) -> Weight {
        // Minimum execution time: 54_980 nanoseconds.
        Weight::from_parts(42_087_213 as u64, 0)
            // Standard Error: 786
            .saturating_add(Weight::from_parts(153_935 as u64, 0).saturating_mul(s as u64))
            // Standard Error: 7
            .saturating_add(Weight::from_parts(1_545 as u64, 0).saturating_mul(z as u64))
            .saturating_add(RocksDbWeight::get().reads(2 as u64))
            .saturating_add(RocksDbWeight::get().writes(2 as u64))
    }
}

#[inline(always)]
pub(super) fn pays_no_with_maybe_weight<E: Into<DispatchError>>(
    result: Result<Option<Weight>, (Option<Weight>, E)>,
) -> DispatchResultWithPostInfo {
    result
        .map_err(|(weight, e)| DispatchErrorWithPostInfo {
            post_info: (weight, Pays::No).into(),
            error: e.into(),
        })
        .map(|weight| (weight, Pays::No).into())
}

#[inline(always)]
pub(super) fn pays_no<T, E: Into<DispatchError>>(
    result: Result<T, E>,
) -> DispatchResultWithPostInfo {
    pays_no_with_maybe_weight(result.map(|_| None).map_err(|e| (None, e)))
}
