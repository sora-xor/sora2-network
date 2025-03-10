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

//! Autogenerated weights for multicollateral_bonding_curve_pool
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
// --pallet=multicollateral_bonding_curve_pool
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --header=./misc/file_header.txt
// --template=./misc/pallet-weight-template.hbs
// --output=./pallets/multicollateral-bonding-curve-pool/src/weights.rs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for multicollateral_bonding_curve_pool.
pub trait WeightInfo {
	fn initialize_pool() -> Weight;
	fn set_reference_asset() -> Weight;
	fn set_optional_reward_multiplier() -> Weight;
	fn on_initialize(n: u32, ) -> Weight;
	fn set_price_change_config() -> Weight;
	fn set_price_bias() -> Weight;
	fn quote() -> Weight;
	fn step_quote(a: u32, ) -> Weight;
	fn exchange() -> Weight;
	fn can_exchange() -> Weight;
	fn check_rewards() -> Weight;
}

/// Weights for multicollateral_bonding_curve_pool using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:1 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: PriceTools FastPriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools FastPriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: TradingPair EnabledSources (r:1 w:1)
	/// Proof Skipped: TradingPair EnabledSources (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (max_values: Some(1), max_size: None, mode: Measured)
	fn initialize_pool() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2072`
		//  Estimated: `27869`
		// Minimum execution time: 115_540_000 picoseconds.
		Weight::from_parts(116_403_000, 27869)
			.saturating_add(T::DbWeight::get().reads(7_u64))
			.saturating_add(T::DbWeight::get().writes(5_u64))
	}
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:1 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	fn set_reference_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `830`
		//  Estimated: `7440`
		// Minimum execution time: 45_623_000 picoseconds.
		Weight::from_parts(46_532_000, 7440)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:1 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (max_values: None, max_size: None, mode: Measured)
	fn set_optional_reward_multiplier() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1220`
		//  Estimated: `12800`
		// Minimum execution time: 61_305_000 picoseconds.
		Weight::from_parts(61_834_000, 12800)
			.saturating_add(T::DbWeight::get().reads(4_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: MulticollateralBondingCurvePool FreeReservesAccountId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool FreeReservesAccountId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PendingFreeReserves (r:11 w:11)
	/// Proof Skipped: MulticollateralBondingCurvePool PendingFreeReserves (max_values: None, max_size: None, mode: Measured)
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: XSTPool EnabledSynthetics (r:2 w:0)
	/// Proof Skipped: XSTPool EnabledSynthetics (max_values: None, max_size: None, mode: Measured)
	/// Storage: DEXAPI EnabledSourceTypes (r:1 w:0)
	/// Proof Skipped: DEXAPI EnabledSourceTypes (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PoolXYK Properties (r:1 w:0)
	/// Proof Skipped: PoolXYK Properties (max_values: None, max_size: None, mode: Measured)
	/// Storage: OrderBook OrderBooks (r:1 w:0)
	/// Proof: OrderBook OrderBooks (max_values: None, max_size: Some(238), added: 2713, mode: MaxEncodedLen)
	/// Storage: TradingPair LockedLiquiditySources (r:1 w:0)
	/// Proof Skipped: TradingPair LockedLiquiditySources (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Tokens Accounts (r:2 w:0)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: ExtendedAssets SoulboundAsset (r:2 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:2 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Technical TechAccounts (r:1 w:0)
	/// Proof Skipped: Technical TechAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: Assets AssetOwners (r:1 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// The range of component `n` is `[0, 10]`.
	fn on_initialize(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4471 + n * (66 ±0)`
		//  Estimated: `741139 + n * (2452 ±1)`
		// Minimum execution time: 15_439_000 picoseconds.
		Weight::from_parts(254_129_650, 741139)
			// Standard Error: 1_436_592
			.saturating_add(Weight::from_parts(19_914_536, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().reads(19_u64))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(n.into())))
			.saturating_add(T::DbWeight::get().writes(8_u64))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(n.into())))
			.saturating_add(Weight::from_parts(0, 2452).saturating_mul(n.into()))
	}
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	fn set_price_change_config() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 19_571_000 picoseconds.
		Weight::from_parts(19_917_000, 0)
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	fn set_price_bias() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 17_590_000 picoseconds.
		Weight::from_parts(17_847_000, 0)
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool BaseFee (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool BaseFee (max_values: Some(1), max_size: None, mode: Measured)
	fn quote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2919`
		//  Estimated: `28353`
		// Minimum execution time: 62_539_000 picoseconds.
		Weight::from_parts(64_590_000, 28353)
			.saturating_add(T::DbWeight::get().reads(8_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool BaseFee (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool BaseFee (max_values: Some(1), max_size: None, mode: Measured)
	/// The range of component `a` is `[10, 1000]`.
	fn step_quote(a: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2919`
		//  Estimated: `28353`
		// Minimum execution time: 332_918_000 picoseconds.
		Weight::from_parts(27_159_923, 28353)
			// Standard Error: 21_023
			.saturating_add(Weight::from_parts(30_471_574, 0).saturating_mul(a.into()))
			.saturating_add(T::DbWeight::get().reads(8_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReservesAcc (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReservesAcc (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool BaseFee (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool BaseFee (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:3 w:3)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: Assets AssetOwners (r:1 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPswapRewardsSupply (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPswapRewardsSupply (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (max_values: None, max_size: None, mode: Measured)
	/// Storage: VestedRewards Rewards (r:1 w:1)
	/// Proof Skipped: VestedRewards Rewards (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:3 w:3)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: VestedRewards TotalRewards (r:1 w:1)
	/// Proof: VestedRewards TotalRewards (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Technical TechAccounts (r:1 w:0)
	/// Proof Skipped: Technical TechAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:2 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:2 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AlwaysDistributeCoefficient (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool AlwaysDistributeCoefficient (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool FreeReservesAccountId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool FreeReservesAccountId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PendingFreeReserves (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool PendingFreeReserves (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:2 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool CollateralReserves (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool CollateralReserves (max_values: None, max_size: None, mode: Measured)
	fn exchange() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6531`
		//  Estimated: `828575`
		// Minimum execution time: 408_583_000 picoseconds.
		Weight::from_parts(410_643_000, 828575)
			.saturating_add(T::DbWeight::get().reads(31_u64))
			.saturating_add(T::DbWeight::get().writes(10_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	fn can_exchange() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `390`
		//  Estimated: `885`
		// Minimum execution time: 9_757_000 picoseconds.
		Weight::from_parts(9_891_000, 885)
			.saturating_add(T::DbWeight::get().reads(1_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReservesAcc (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReservesAcc (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:0)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: Assets AssetOwners (r:1 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPswapRewardsSupply (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPswapRewardsSupply (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (max_values: None, max_size: None, mode: Measured)
	fn check_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3973`
		//  Estimated: `60174`
		// Minimum execution time: 107_809_000 picoseconds.
		Weight::from_parts(108_305_000, 60174)
			.saturating_add(T::DbWeight::get().reads(13_u64))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:1 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: PriceTools FastPriceInfos (r:1 w:1)
	/// Proof Skipped: PriceTools FastPriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: TradingPair EnabledSources (r:1 w:1)
	/// Proof Skipped: TradingPair EnabledSources (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (max_values: Some(1), max_size: None, mode: Measured)
	fn initialize_pool() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2072`
		//  Estimated: `27869`
		// Minimum execution time: 115_540_000 picoseconds.
		Weight::from_parts(116_403_000, 27869)
			.saturating_add(RocksDbWeight::get().reads(7_u64))
			.saturating_add(RocksDbWeight::get().writes(5_u64))
	}
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:1 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	fn set_reference_asset() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `830`
		//  Estimated: `7440`
		// Minimum execution time: 45_623_000 picoseconds.
		Weight::from_parts(46_532_000, 7440)
			.saturating_add(RocksDbWeight::get().reads(2_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:1 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (max_values: None, max_size: None, mode: Measured)
	fn set_optional_reward_multiplier() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1220`
		//  Estimated: `12800`
		// Minimum execution time: 61_305_000 picoseconds.
		Weight::from_parts(61_834_000, 12800)
			.saturating_add(RocksDbWeight::get().reads(4_u64))
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: MulticollateralBondingCurvePool FreeReservesAccountId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool FreeReservesAccountId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PendingFreeReserves (r:11 w:11)
	/// Proof Skipped: MulticollateralBondingCurvePool PendingFreeReserves (max_values: None, max_size: None, mode: Measured)
	/// Storage: DEXManager DEXInfos (r:1 w:0)
	/// Proof Skipped: DEXManager DEXInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: XSTPool EnabledSynthetics (r:2 w:0)
	/// Proof Skipped: XSTPool EnabledSynthetics (max_values: None, max_size: None, mode: Measured)
	/// Storage: DEXAPI EnabledSourceTypes (r:1 w:0)
	/// Proof Skipped: DEXAPI EnabledSourceTypes (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PoolXYK Properties (r:1 w:0)
	/// Proof Skipped: PoolXYK Properties (max_values: None, max_size: None, mode: Measured)
	/// Storage: OrderBook OrderBooks (r:1 w:0)
	/// Proof: OrderBook OrderBooks (max_values: None, max_size: Some(238), added: 2713, mode: MaxEncodedLen)
	/// Storage: TradingPair LockedLiquiditySources (r:1 w:0)
	/// Proof Skipped: TradingPair LockedLiquiditySources (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: System Account (r:1 w:0)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: Tokens Accounts (r:2 w:0)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: ExtendedAssets SoulboundAsset (r:2 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:2 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: Technical TechAccounts (r:1 w:0)
	/// Proof Skipped: Technical TechAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: Assets AssetOwners (r:1 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// The range of component `n` is `[0, 10]`.
	fn on_initialize(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `4471 + n * (66 ±0)`
		//  Estimated: `741139 + n * (2452 ±1)`
		// Minimum execution time: 15_439_000 picoseconds.
		Weight::from_parts(254_129_650, 741139)
			// Standard Error: 1_436_592
			.saturating_add(Weight::from_parts(19_914_536, 0).saturating_mul(n.into()))
			.saturating_add(RocksDbWeight::get().reads(19_u64))
			.saturating_add(RocksDbWeight::get().reads((1_u64).saturating_mul(n.into())))
			.saturating_add(RocksDbWeight::get().writes(8_u64))
			.saturating_add(RocksDbWeight::get().writes((1_u64).saturating_mul(n.into())))
			.saturating_add(Weight::from_parts(0, 2452).saturating_mul(n.into()))
	}
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	fn set_price_change_config() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 19_571_000 picoseconds.
		Weight::from_parts(19_917_000, 0)
			.saturating_add(RocksDbWeight::get().writes(2_u64))
	}
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	fn set_price_bias() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 17_590_000 picoseconds.
		Weight::from_parts(17_847_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool BaseFee (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool BaseFee (max_values: Some(1), max_size: None, mode: Measured)
	fn quote() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2919`
		//  Estimated: `28353`
		// Minimum execution time: 62_539_000 picoseconds.
		Weight::from_parts(64_590_000, 28353)
			.saturating_add(RocksDbWeight::get().reads(8_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool BaseFee (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool BaseFee (max_values: Some(1), max_size: None, mode: Measured)
	/// The range of component `a` is `[10, 1000]`.
	fn step_quote(a: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2919`
		//  Estimated: `28353`
		// Minimum execution time: 332_918_000 picoseconds.
		Weight::from_parts(27_159_923, 28353)
			// Standard Error: 21_023
			.saturating_add(Weight::from_parts(30_471_574, 0).saturating_mul(a.into()))
			.saturating_add(RocksDbWeight::get().reads(8_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReservesAcc (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReservesAcc (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool BaseFee (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool BaseFee (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:3 w:3)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: Assets AssetOwners (r:1 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPswapRewardsSupply (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPswapRewardsSupply (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (max_values: None, max_size: None, mode: Measured)
	/// Storage: VestedRewards Rewards (r:1 w:1)
	/// Proof Skipped: VestedRewards Rewards (max_values: None, max_size: None, mode: Measured)
	/// Storage: System Account (r:3 w:3)
	/// Proof: System Account (max_values: None, max_size: Some(128), added: 2603, mode: MaxEncodedLen)
	/// Storage: VestedRewards TotalRewards (r:1 w:1)
	/// Proof: VestedRewards TotalRewards (max_values: Some(1), max_size: Some(16), added: 511, mode: MaxEncodedLen)
	/// Storage: Technical TechAccounts (r:1 w:0)
	/// Proof Skipped: Technical TechAccounts (max_values: None, max_size: None, mode: Measured)
	/// Storage: ExtendedAssets SoulboundAsset (r:2 w:0)
	/// Proof: ExtendedAssets SoulboundAsset (max_values: None, max_size: Some(322091), added: 324566, mode: MaxEncodedLen)
	/// Storage: Assets AssetInfosV2 (r:2 w:0)
	/// Proof Skipped: Assets AssetInfosV2 (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AlwaysDistributeCoefficient (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool AlwaysDistributeCoefficient (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool FreeReservesAccountId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool FreeReservesAccountId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PendingFreeReserves (r:1 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool PendingFreeReserves (max_values: None, max_size: None, mode: Measured)
	/// Storage: Permissions Permissions (r:2 w:0)
	/// Proof Skipped: Permissions Permissions (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool CollateralReserves (r:0 w:1)
	/// Proof Skipped: MulticollateralBondingCurvePool CollateralReserves (max_values: None, max_size: None, mode: Measured)
	fn exchange() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `6531`
		//  Estimated: `828575`
		// Minimum execution time: 408_583_000 picoseconds.
		Weight::from_parts(410_643_000, 828575)
			.saturating_add(RocksDbWeight::get().reads(31_u64))
			.saturating_add(RocksDbWeight::get().writes(10_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	fn can_exchange() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `390`
		//  Estimated: `885`
		// Minimum execution time: 9_757_000 picoseconds.
		Weight::from_parts(9_891_000, 885)
			.saturating_add(RocksDbWeight::get().reads(1_u64))
	}
	/// Storage: MulticollateralBondingCurvePool EnabledTargets (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool EnabledTargets (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReservesAcc (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReservesAcc (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPrice (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPrice (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeStep (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeStep (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool PriceChangeRate (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool PriceChangeRate (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: Tokens Accounts (r:1 w:0)
	/// Proof: Tokens Accounts (max_values: None, max_size: Some(136), added: 2611, mode: MaxEncodedLen)
	/// Storage: Assets AssetOwners (r:1 w:0)
	/// Proof Skipped: Assets AssetOwners (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool ReferenceAssetId (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool ReferenceAssetId (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: PriceTools PriceInfos (r:2 w:0)
	/// Proof Skipped: PriceTools PriceInfos (max_values: None, max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool IncentivisedCurrenciesNum (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool InitialPswapRewardsSupply (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool InitialPswapRewardsSupply (max_values: Some(1), max_size: None, mode: Measured)
	/// Storage: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (r:1 w:0)
	/// Proof Skipped: MulticollateralBondingCurvePool AssetsWithOptionalRewardMultiplier (max_values: None, max_size: None, mode: Measured)
	fn check_rewards() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `3973`
		//  Estimated: `60174`
		// Minimum execution time: 107_809_000 picoseconds.
		Weight::from_parts(108_305_000, 60174)
			.saturating_add(RocksDbWeight::get().reads(13_u64))
	}
}
