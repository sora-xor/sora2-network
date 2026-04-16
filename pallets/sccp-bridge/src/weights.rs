// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#![allow(unused_parens)]

use core::marker::PhantomData;
use frame_support::traits::Get;
use frame_support::weights::{constants::RocksDbWeight, Weight};

/// Weight functions needed for sccp-bridge.
pub trait WeightInfo {
    fn import_registry_asset() -> Weight;
    fn activate_route() -> Weight;
    fn pause_route() -> Weight;
    fn resume_route() -> Weight;
    fn record_outbound() -> Weight;
    fn finalize_inbound() -> Weight;
    fn submit_message_proof(proof_bytes: u32, public_inputs: u32, bundle_bytes: u32) -> Weight;
}

/// Hand-authored pre-benchmark weights for SCCP bridge.
pub struct SubstrateWeight<T>(PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn import_registry_asset() -> Weight {
        Weight::from_parts(15_000_000, 0).saturating_add(T::DbWeight::get().writes(1))
    }

    fn activate_route() -> Weight {
        Weight::from_parts(20_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }

    fn pause_route() -> Weight {
        Weight::from_parts(18_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }

    fn resume_route() -> Weight {
        Weight::from_parts(18_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(1, 1))
    }

    fn record_outbound() -> Weight {
        Weight::from_parts(35_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 2))
    }

    fn finalize_inbound() -> Weight {
        Weight::from_parts(30_000_000, 0).saturating_add(T::DbWeight::get().reads_writes(2, 2))
    }

    fn submit_message_proof(proof_bytes: u32, public_inputs: u32, bundle_bytes: u32) -> Weight {
        // Conservative placeholder until verifier-specific benchmarking is available.
        let total_bytes = u64::from(proof_bytes)
            .saturating_add(u64::from(public_inputs))
            .saturating_add(u64::from(bundle_bytes));

        Weight::from_parts(60_000_000, 0)
            .saturating_add(Weight::from_parts(500_000, 0).saturating_mul(total_bytes))
            .saturating_add(T::DbWeight::get().reads_writes(4, 3))
    }
}

impl WeightInfo for () {
    fn import_registry_asset() -> Weight {
        Weight::from_parts(15_000_000, 0).saturating_add(RocksDbWeight::get().writes(1))
    }

    fn activate_route() -> Weight {
        Weight::from_parts(20_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
    }

    fn pause_route() -> Weight {
        Weight::from_parts(18_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
    }

    fn resume_route() -> Weight {
        Weight::from_parts(18_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(1, 1))
    }

    fn record_outbound() -> Weight {
        Weight::from_parts(35_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
    }

    fn finalize_inbound() -> Weight {
        Weight::from_parts(30_000_000, 0).saturating_add(RocksDbWeight::get().reads_writes(2, 2))
    }

    fn submit_message_proof(proof_bytes: u32, public_inputs: u32, bundle_bytes: u32) -> Weight {
        let total_bytes = u64::from(proof_bytes)
            .saturating_add(u64::from(public_inputs))
            .saturating_add(u64::from(bundle_bytes));

        Weight::from_parts(60_000_000, 0)
            .saturating_add(Weight::from_parts(500_000, 0).saturating_mul(total_bytes))
            .saturating_add(RocksDbWeight::get().reads_writes(4, 3))
    }
}
