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

//! Autogenerated weights for hermes_governance_platform
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2025-02-18, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `983eb2919397`, CPU: `Intel(R) Xeon(R) CPU E3-1240 v6 @ 3.70GHz`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("local"), DB CACHE: 1024

// Executed Command:
// /usr/local/bin/framenode
// benchmark
// pallet
// --chain=local
// --steps=50
// --repeat=20
// --pallet=hermes_governance_platform
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --header=./misc/file_header.txt
// --template=./misc/pallet-weight-template.hbs
// --output=./pallets/hermes-governance-platform/src/weights.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for hermes_governance_platform.
pub trait WeightInfo {
	fn vote() -> Weight;
	fn create_poll() -> Weight;
	fn withdraw_funds_voter() -> Weight;
	fn withdraw_funds_creator() -> Weight;
	fn change_min_hermes_for_voting() -> Weight;
	fn change_min_hermes_for_creating_poll() -> Weight;
}

/// Weights for hermes_governance_platform using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesPollData (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform MinimumHermesVotingAmount (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesVotingAmount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesVotings (r:1 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesVotings (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:2 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn vote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1935`
		//  Estimated: `351157`
		// Minimum execution time: 146_609_000 picoseconds.
		Weight::from_parts(150_872_000, 351157)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform HermesPollData (r:0 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	fn create_poll() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1752`
		//  Estimated: `343723`
		// Minimum execution time: 135_282_000 picoseconds.
		Weight::from_parts(139_472_000, 343723)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesPollData (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform HermesVotings (r:1 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesVotings (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn withdraw_funds_voter() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2176`
		//  Estimated: `346847`
		// Minimum execution time: 121_810_000 picoseconds.
		Weight::from_parts(122_763_000, 346847)
			.saturating_add(T::DbWeight::get().reads(8_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesPollData (r:1 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn withdraw_funds_creator() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2066`
		//  Estimated: `341976`
		// Minimum execution time: 113_828_000 picoseconds.
		Weight::from_parts(117_052_000, 341976)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: HermesGovernancePlatform AuthorityAccount (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform MinimumHermesVotingAmount (r:0 w:1)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesVotingAmount (max_values: Some(1), max_size: None, mode: Measured)
	fn change_min_hermes_for_voting() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `142`
		//  Estimated: `779`
		// Minimum execution time: 20_094_000 picoseconds.
		Weight::from_parts(20_766_000, 779)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: HermesGovernancePlatform AuthorityAccount (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (r:0 w:1)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (max_values: Some(1), max_size: None, mode: Measured)
	fn change_min_hermes_for_creating_poll() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `142`
		//  Estimated: `779`
		// Minimum execution time: 20_625_000 picoseconds.
		Weight::from_parts(20_906_000, 779)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesPollData (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform MinimumHermesVotingAmount (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesVotingAmount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesVotings (r:1 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesVotings (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:2 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn vote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1935`
		//  Estimated: `351157`
		// Minimum execution time: 146_609_000 picoseconds.
		Weight::from_parts(150_872_000, 351157)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform HermesPollData (r:0 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	fn create_poll() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1752`
		//  Estimated: `343723`
		// Minimum execution time: 135_282_000 picoseconds.
		Weight::from_parts(139_472_000, 343723)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesPollData (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform HermesVotings (r:1 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesVotings (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn withdraw_funds_voter() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2176`
		//  Estimated: `346847`
		// Minimum execution time: 121_810_000 picoseconds.
		Weight::from_parts(122_763_000, 346847)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: HermesGovernancePlatform HermesPollData (r:1 w:1)
	/// Proof Skipped: HermesGovernancePlatform HermesPollData (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn withdraw_funds_creator() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2066`
		//  Estimated: `341976`
		// Minimum execution time: 113_828_000 picoseconds.
		Weight::from_parts(117_052_000, 341976)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: HermesGovernancePlatform AuthorityAccount (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform MinimumHermesVotingAmount (r:0 w:1)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesVotingAmount (max_values: Some(1), max_size: None, mode: Measured)
	fn change_min_hermes_for_voting() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `142`
		//  Estimated: `779`
		// Minimum execution time: 20_094_000 picoseconds.
		Weight::from_parts(20_766_000, 779)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: HermesGovernancePlatform AuthorityAccount (r:1 w:0)
	/// Proof Skipped: HermesGovernancePlatform AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (r:0 w:1)
	/// Proof Skipped: HermesGovernancePlatform MinimumHermesAmountForCreatingPoll (max_values: Some(1), max_size: None, mode: Measured)
	fn change_min_hermes_for_creating_poll() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `142`
		//  Estimated: `779`
		// Minimum execution time: 20_625_000 picoseconds.
		Weight::from_parts(20_906_000, 779)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
