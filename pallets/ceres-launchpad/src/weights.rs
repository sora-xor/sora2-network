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

//! Autogenerated weights for ceres_launchpad
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
// --pallet=ceres_launchpad
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --header=./misc/file_header.txt
// --template=./misc/pallet-weight-template.hbs
// --output=./pallets/ceres-launchpad/src/weights.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for ceres_launchpad.
pub trait WeightInfo {
	fn create_ilo() -> Weight;
	fn contribute() -> Weight;
	fn emergency_withdraw() -> Weight;
	fn finish_ilo() -> Weight;
	fn claim_lp_tokens() -> Weight;
	fn claim() -> Weight;
	fn change_ceres_burn_fee() -> Weight;
	fn change_ceres_contribution_fee() -> Weight;
	fn claim_pswap_rewards() -> Weight;
	fn add_whitelisted_contributor() -> Weight;
	fn remove_whitelisted_contributor() -> Weight;
	fn add_whitelisted_ilo_organizer() -> Weight;
	fn remove_whitelisted_ilo_organizer() -> Weight;
}

/// Weights for ceres_launchpad using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: CeresLaunchpad WhitelistedIloOrganizers (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad WhitelistedIloOrganizers (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: TradingPair EnabledSources (r:1 w:0)
	/// Proof Skipped: TradingPair EnabledSources (max_values: None, max_size: None, mode: Measured)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad CeresBurnFeeAmount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad CeresBurnFeeAmount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens TotalIssuance (r:1 w:1)
	/// Proof: Tokens TotalIssuance (max_values: None, max_size: Some(56), added: 2531, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn create_ilo() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2590`
		//  Estimated: `364458`
		// Minimum execution time: 221_102_000 picoseconds.
		Weight::from_parts(222_019_000, 364458)
			.saturating_add(T::DbWeight::get().reads(13_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: CeresLaunchpad WhitelistedContributors (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad WhitelistedContributors (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad CeresForContributionInILO (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad CeresForContributionInILO (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:0)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad Contributions (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad Contributions (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn contribute() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2592`
		//  Estimated: `354261`
		// Minimum execution time: 154_490_000 picoseconds.
		Weight::from_parts(155_925_000, 354261)
			.saturating_add(T::DbWeight::get().reads(10_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad Contributions (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad Contributions (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:3 w:3)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad PenaltiesAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad PenaltiesAccount (max_values: Some(1), max_size: None, mode: Measured)
	fn emergency_withdraw() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2204`
		//  Estimated: `349614`
		// Minimum execution time: 192_441_000 picoseconds.
		Weight::from_parts(198_324_000, 349614)
			.saturating_add(T::DbWeight::get().reads(9_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad FeePercentOnRaisedFunds (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad FeePercentOnRaisedFunds (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:2 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:2 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:6 w:6)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Assets AssetOwners (r:2 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// Storage: TradingPair EnabledSources (r:1 w:1)
	/// Proof Skipped: TradingPair EnabledSources (max_values: None, max_size: None, mode: Measured)
	/// Storage: Technical TechAccounts (r:2 w:2)
	/// Proof Skipped: Technical TechAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: PswapDistribution SubscribedAccounts (r:1 w:1)
	/// Proof Skipped: PswapDistribution SubscribedAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: Farming Pools (r:1 w:1)
	/// Proof Skipped: Farming Pools (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:5 w:5)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: PoolXYK PoolProviders (r:2 w:2)
	/// Proof Skipped: PoolXYK PoolProviders (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK AccountPools (r:2 w:2)
	/// Proof Skipped: PoolXYK AccountPools (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK TotalIssuances (r:1 w:1)
	/// Proof Skipped: PoolXYK TotalIssuances (max_values: None, max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: PriceTools FastPriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools FastPriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens TotalIssuance (r:1 w:1)
	/// Proof: Tokens TotalIssuance (max_values: None, max_size: Some(56), added: 2531, mode: MaxEncodedLen)
	/// Storage: CeresLiquidityLocker LockerData (r:1 w:1)
	/// Proof Skipped: CeresLiquidityLocker LockerData (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLiquidityLocker FeesOptionOneAccount (r:1 w:0)
	/// Proof Skipped: CeresLiquidityLocker FeesOptionOneAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: DemeterFarmingPlatform UserInfos (r:1 w:1)
	/// Proof Skipped: DemeterFarmingPlatform UserInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresTokenLocker FeeAmount (r:1 w:0)
	/// Proof Skipped: CeresTokenLocker FeeAmount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresTokenLocker FeesAccount (r:1 w:0)
	/// Proof Skipped: CeresTokenLocker FeesAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresTokenLocker TokenLockerData (r:1 w:1)
	/// Proof Skipped: CeresTokenLocker TokenLockerData (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK Reserves (r:0 w:1)
	/// Proof Skipped: PoolXYK Reserves (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK Properties (r:0 w:1)
	/// Proof Skipped: PoolXYK Properties (max_values: None, max_size: None, mode: Measured)
	fn finish_ilo() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5422`
		//  Estimated: `859995`
		// Minimum execution time: 1_319_412_000 picoseconds.
		Weight::from_parts(1_356_507_000, 859995)
			.saturating_add(T::DbWeight::get().reads(41_u64))
			.saturating_add(T::DbWeight::get().writes(30_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK Properties (r:1 w:0)
	/// Proof Skipped: PoolXYK Properties (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK PoolProviders (r:2 w:2)
	/// Proof Skipped: PoolXYK PoolProviders (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK AccountPools (r:1 w:1)
	/// Proof Skipped: PoolXYK AccountPools (max_values: None, max_size: None, mode: Measured)
	fn claim_lp_tokens() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1369`
		//  Estimated: `18354`
		// Minimum execution time: 78_579_000 picoseconds.
		Weight::from_parts(80_317_000, 18354)
			.saturating_add(T::DbWeight::get().reads(6_u64))
			.saturating_add(T::DbWeight::get().writes(4_u64))
	}
	/// Storage: CeresLaunchpad ILOs (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad Contributions (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad Contributions (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2431`
		//  Estimated: `347109`
		// Minimum execution time: 121_432_000 picoseconds.
		Weight::from_parts(125_067_000, 347109)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(3_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad CeresBurnFeeAmount (r:0 w:1)
	/// Proof Skipped: CeresLaunchpad CeresBurnFeeAmount (max_values: Some(1), max_size: None, mode: Measured)
	fn change_ceres_burn_fee() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `647`
		// Minimum execution time: 21_010_000 picoseconds.
		Weight::from_parts(21_455_000, 647)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad CeresForContributionInILO (r:0 w:1)
	/// Proof Skipped: CeresLaunchpad CeresForContributionInILO (max_values: Some(1), max_size: None, mode: Measured)
	fn change_ceres_contribution_fee() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `647`
		// Minimum execution time: 21_350_000 picoseconds.
		Weight::from_parts(21_587_000, 647)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PswapDistribution ShareholderAccounts (r:1 w:1)
	/// Proof Skipped: PswapDistribution ShareholderAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: PswapDistribution ClaimableShares (r:1 w:1)
	/// Proof Skipped: PswapDistribution ClaimableShares (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:3 w:3)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:3 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: VestedRewards Rewards (r:1 w:0)
	/// Proof Skipped: VestedRewards Rewards (max_values: None, max_size: None, mode: Measured)
	fn claim_pswap_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2479`
		//  Estimated: `361018`
		// Minimum execution time: 234_517_000 picoseconds.
		Weight::from_parts(236_386_000, 361018)
			.saturating_add(T::DbWeight::get().reads(12_u64))
			.saturating_add(T::DbWeight::get().writes(7_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedContributors (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedContributors (max_values: Some(1), max_size: None, mode: Measured)
	fn add_whitelisted_contributor() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 22_968_000 picoseconds.
		Weight::from_parts(23_345_000, 1142)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedContributors (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedContributors (max_values: Some(1), max_size: None, mode: Measured)
	fn remove_whitelisted_contributor() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 23_844_000 picoseconds.
		Weight::from_parts(24_157_000, 1142)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedIloOrganizers (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedIloOrganizers (max_values: Some(1), max_size: None, mode: Measured)
	fn add_whitelisted_ilo_organizer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 23_479_000 picoseconds.
		Weight::from_parts(23_861_000, 1142)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedIloOrganizers (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedIloOrganizers (max_values: Some(1), max_size: None, mode: Measured)
	fn remove_whitelisted_ilo_organizer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 24_029_000 picoseconds.
		Weight::from_parts(24_953_000, 1142)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: CeresLaunchpad WhitelistedIloOrganizers (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad WhitelistedIloOrganizers (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: TradingPair EnabledSources (r:1 w:0)
	/// Proof Skipped: TradingPair EnabledSources (max_values: None, max_size: None, mode: Measured)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad CeresBurnFeeAmount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad CeresBurnFeeAmount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens TotalIssuance (r:1 w:1)
	/// Proof: Tokens TotalIssuance (max_values: None, max_size: Some(56), added: 2531, mode: MaxEncodedLen)
	/// Storage: System Account (r:2 w:1)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn create_ilo() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2590`
		//  Estimated: `364458`
		// Minimum execution time: 221_102_000 picoseconds.
		Weight::from_parts(222_019_000, 364458)
			.saturating_add(RocksDbWeight::get().reads(13_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: CeresLaunchpad WhitelistedContributors (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad WhitelistedContributors (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad CeresForContributionInILO (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad CeresForContributionInILO (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:0)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad Contributions (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad Contributions (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:2 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn contribute() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2592`
		//  Estimated: `354261`
		// Minimum execution time: 154_490_000 picoseconds.
		Weight::from_parts(155_925_000, 354261)
			.saturating_add(RocksDbWeight::get().reads(10_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad Contributions (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad Contributions (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:3 w:3)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad PenaltiesAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad PenaltiesAccount (max_values: Some(1), max_size: None, mode: Measured)
	fn emergency_withdraw() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2204`
		//  Estimated: `349614`
		// Minimum execution time: 192_441_000 picoseconds.
		Weight::from_parts(198_324_000, 349614)
			.saturating_add(RocksDbWeight::get().reads(9_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad FeePercentOnRaisedFunds (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad FeePercentOnRaisedFunds (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:2 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:2 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:6 w:6)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Assets AssetOwners (r:2 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// Storage: TradingPair EnabledSources (r:1 w:1)
	/// Proof Skipped: TradingPair EnabledSources (max_values: None, max_size: None, mode: Measured)
	/// Storage: Technical TechAccounts (r:2 w:2)
	/// Proof Skipped: Technical TechAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: PswapDistribution SubscribedAccounts (r:1 w:1)
	/// Proof Skipped: PswapDistribution SubscribedAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: Farming Pools (r:1 w:1)
	/// Proof Skipped: Farming Pools (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:5 w:5)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: PoolXYK PoolProviders (r:2 w:2)
	/// Proof Skipped: PoolXYK PoolProviders (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK AccountPools (r:2 w:2)
	/// Proof Skipped: PoolXYK AccountPools (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK TotalIssuances (r:1 w:1)
	/// Proof Skipped: PoolXYK TotalIssuances (max_values: None, max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: PriceTools FastPriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools FastPriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens TotalIssuance (r:1 w:1)
	/// Proof: Tokens TotalIssuance (max_values: None, max_size: Some(56), added: 2531, mode: MaxEncodedLen)
	/// Storage: CeresLiquidityLocker LockerData (r:1 w:1)
	/// Proof Skipped: CeresLiquidityLocker LockerData (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLiquidityLocker FeesOptionOneAccount (r:1 w:0)
	/// Proof Skipped: CeresLiquidityLocker FeesOptionOneAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: DemeterFarmingPlatform UserInfos (r:1 w:1)
	/// Proof Skipped: DemeterFarmingPlatform UserInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresTokenLocker FeeAmount (r:1 w:0)
	/// Proof Skipped: CeresTokenLocker FeeAmount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresTokenLocker FeesAccount (r:1 w:0)
	/// Proof Skipped: CeresTokenLocker FeesAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresTokenLocker TokenLockerData (r:1 w:1)
	/// Proof Skipped: CeresTokenLocker TokenLockerData (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK Reserves (r:0 w:1)
	/// Proof Skipped: PoolXYK Reserves (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK Properties (r:0 w:1)
	/// Proof Skipped: PoolXYK Properties (max_values: None, max_size: None, mode: Measured)
	fn finish_ilo() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `5422`
		//  Estimated: `859995`
		// Minimum execution time: 1_319_412_000 picoseconds.
		Weight::from_parts(1_356_507_000, 859995)
			.saturating_add(RocksDbWeight::get().reads(41_u64))
			.saturating_add(RocksDbWeight::get().writes(30_u64))
	}
	/// Storage: Timestamp Now (r:1 w:0)
	/// Proof: Timestamp Now (max_values: Some(1), max_size: Some(8), added: 503, mode: MaxEncodedLen)
	/// Storage: CeresLaunchpad ILOs (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK Properties (r:1 w:0)
	/// Proof Skipped: PoolXYK Properties (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK PoolProviders (r:2 w:2)
	/// Proof Skipped: PoolXYK PoolProviders (max_values: None, max_size: None, mode: Measured)
	/// Storage: PoolXYK AccountPools (r:1 w:1)
	/// Proof Skipped: PoolXYK AccountPools (max_values: None, max_size: None, mode: Measured)
	fn claim_lp_tokens() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1369`
		//  Estimated: `18354`
		// Minimum execution time: 78_579_000 picoseconds.
		Weight::from_parts(80_317_000, 18354)
			.saturating_add(RocksDbWeight::get().reads(6_u64))
			.saturating_add(RocksDbWeight::get().writes(4_u64))
	}
	/// Storage: CeresLaunchpad ILOs (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad ILOs (max_values: None, max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad Contributions (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad Contributions (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:2 w:2)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	fn claim() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2431`
		//  Estimated: `347109`
		// Minimum execution time: 121_432_000 picoseconds.
		Weight::from_parts(125_067_000, 347109)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(3_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad CeresBurnFeeAmount (r:0 w:1)
	/// Proof Skipped: CeresLaunchpad CeresBurnFeeAmount (max_values: Some(1), max_size: None, mode: Measured)
	fn change_ceres_burn_fee() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `647`
		// Minimum execution time: 21_010_000 picoseconds.
		Weight::from_parts(21_455_000, 647)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad CeresForContributionInILO (r:0 w:1)
	/// Proof Skipped: CeresLaunchpad CeresForContributionInILO (max_values: Some(1), max_size: None, mode: Measured)
	fn change_ceres_contribution_fee() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `647`
		// Minimum execution time: 21_350_000 picoseconds.
		Weight::from_parts(21_587_000, 647)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PswapDistribution ShareholderAccounts (r:1 w:1)
	/// Proof Skipped: PswapDistribution ShareholderAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: PswapDistribution ClaimableShares (r:1 w:1)
	/// Proof Skipped: PswapDistribution ClaimableShares (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:1 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:1 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:3 w:3)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: System Account (r:3 w:2)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: VestedRewards Rewards (r:1 w:0)
	/// Proof Skipped: VestedRewards Rewards (max_values: None, max_size: None, mode: Measured)
	fn claim_pswap_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2479`
		//  Estimated: `361018`
		// Minimum execution time: 234_517_000 picoseconds.
		Weight::from_parts(236_386_000, 361018)
			.saturating_add(RocksDbWeight::get().reads(12_u64))
			.saturating_add(RocksDbWeight::get().writes(7_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedContributors (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedContributors (max_values: Some(1), max_size: None, mode: Measured)
	fn add_whitelisted_contributor() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 22_968_000 picoseconds.
		Weight::from_parts(23_345_000, 1142)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedContributors (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedContributors (max_values: Some(1), max_size: None, mode: Measured)
	fn remove_whitelisted_contributor() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 23_844_000 picoseconds.
		Weight::from_parts(24_157_000, 1142)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedIloOrganizers (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedIloOrganizers (max_values: Some(1), max_size: None, mode: Measured)
	fn add_whitelisted_ilo_organizer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 23_479_000 picoseconds.
		Weight::from_parts(23_861_000, 1142)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: CeresLaunchpad AuthorityAccount (r:1 w:0)
	/// Proof Skipped: CeresLaunchpad AuthorityAccount (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: CeresLaunchpad WhitelistedIloOrganizers (r:1 w:1)
	/// Proof Skipped: CeresLaunchpad WhitelistedIloOrganizers (max_values: Some(1), max_size: None, mode: Measured)
	fn remove_whitelisted_ilo_organizer() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1142`
		// Minimum execution time: 24_029_000 picoseconds.
		Weight::from_parts(24_953_000, 1142)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
}
