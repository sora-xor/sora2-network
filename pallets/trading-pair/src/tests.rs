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

use crate::mock::*;
use crate::{Error, Pallet};
use common::{
    DexId, EnsureTradingPairExists, LiquiditySourceType, TradingPair, TradingPairSourceManager,
    DOT, KSM, KUSD, VXOR, XOR, XSTUSD,
};
use frame_support::assert_noop;
use frame_support::assert_ok;

type TradingPairPallet = Pallet<Runtime>;

#[test]
fn should_register_trading_pair() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(TradingPairPallet::register(
            RuntimeOrigin::signed(ALICE),
            DEX_ID,
            XOR,
            DOT
        ));
    });
}

#[test]
fn should_register_with_another_dex_id() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_ok!(TradingPairPallet::register(
            RuntimeOrigin::signed(ALICE),
            DexId::PolkaswapXstUsd,
            XSTUSD,
            DOT
        ));

        assert_ok!(TradingPairPallet::register(
            RuntimeOrigin::signed(ALICE),
            DexId::PolkaswapKUSD,
            KUSD,
            DOT
        ));

        assert_ok!(TradingPairPallet::register(
            RuntimeOrigin::signed(ALICE),
            DexId::PolkaswapVXOR,
            VXOR,
            DOT
        ));
    });
}

#[test]
fn should_not_register_trading_pair_with_wrong_base_asset() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DexId::Polkaswap, DOT, XOR),
            Error::<Runtime>::ForbiddenBaseAssetId
        );

        assert_noop!(
            TradingPairPallet::register(
                RuntimeOrigin::signed(ALICE),
                DexId::PolkaswapXstUsd,
                XOR,
                DOT
            ),
            Error::<Runtime>::ForbiddenBaseAssetId
        );

        assert_noop!(
            TradingPairPallet::register(
                RuntimeOrigin::signed(ALICE),
                DexId::PolkaswapKUSD,
                XOR,
                DOT
            ),
            Error::<Runtime>::ForbiddenBaseAssetId
        );

        assert_noop!(
            TradingPairPallet::register(
                RuntimeOrigin::signed(ALICE),
                DexId::PolkaswapVXOR,
                XOR,
                DOT
            ),
            Error::<Runtime>::ForbiddenBaseAssetId
        );
    });
}

#[test]
fn should_not_register_trading_pair_with_same_assets() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_noop!(
            TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, XOR),
            Error::<Runtime>::IdenticalAssetIds
        );
    });
}

#[test]
fn should_list_registered_pairs() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        assert_eq!(
            TradingPairPallet::list_trading_pairs(&DEX_ID).expect("Failed to list trading pairs."),
            vec![]
        );
        assert!(
            !TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &DOT)
                .expect("Failed to query pair state.")
        );
        assert!(
            !TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert_noop!(
            TradingPairPallet::ensure_trading_pair_exists(&DEX_ID, &XOR, &DOT),
            Error::<Runtime>::TradingPairDoesntExist
        );
        assert_noop!(
            TradingPairPallet::ensure_trading_pair_exists(&DEX_ID, &XOR, &KSM),
            Error::<Runtime>::TradingPairDoesntExist
        );

        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_eq!(
            TradingPairPallet::list_trading_pairs(&DEX_ID).expect("Failed to list trading pairs."),
            vec![TradingPair {
                base_asset_id: XOR,
                target_asset_id: DOT
            }]
        );
        assert!(
            TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &DOT)
                .expect("Failed to query pair state.")
        );
        assert!(
            !TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert_ok!(TradingPairPallet::ensure_trading_pair_exists(
            &DEX_ID, &XOR, &DOT
        ));
        assert_noop!(
            TradingPairPallet::ensure_trading_pair_exists(&DEX_ID, &XOR, &KSM),
            Error::<Runtime>::TradingPairDoesntExist
        );

        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, KSM)
            .expect("Failed to register pair.");
        assert_eq!(
            TradingPairPallet::list_trading_pairs(&DEX_ID).expect("Failed to list trading pairs."),
            vec![
                TradingPair {
                    base_asset_id: XOR,
                    target_asset_id: DOT
                },
                TradingPair {
                    base_asset_id: XOR,
                    target_asset_id: KSM
                },
            ]
        );
        assert!(
            TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert!(
            TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &KSM)
                .expect("Failed to query pair state.")
        );
        assert_ok!(TradingPairPallet::ensure_trading_pair_exists(
            &DEX_ID, &XOR, &DOT
        ));
        assert_ok!(TradingPairPallet::ensure_trading_pair_exists(
            &DEX_ID, &XOR, &KSM
        ));
    });
}

#[test]
fn should_enable_sources_for_pair_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, KSM)
            .expect("Failed to register pair.");
        // check initial states after trading pair registration
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );

        // pre check for enabled sources
        assert!(!TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));

        // enable source on one pair and check both trading pairs
        TradingPairPallet::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::XykPool]
        );
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert!(TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));

        // enable source for another pair
        TradingPairPallet::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::XykPool]
        );
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::XykPool]
        );
        assert!(TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
        assert!(TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
    });
}

#[test]
fn should_disable_sources_for_pair_correctly() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, KSM)
            .expect("Failed to register pair.");

        TradingPairPallet::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool,
        )
        .expect("Failed to enable source for pair.");

        TradingPairPallet::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool,
        )
        .expect("Failed to enable source for pair.");

        // check initial states after trading pair registration
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::XykPool]
        );
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::XykPool]
        );

        // pre check for enabled sources
        assert!(TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
        assert!(TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));

        // enable source on one pair and check both trading pairs
        TradingPairPallet::disable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![LiquiditySourceType::XykPool]
        );
        assert!(!TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
        assert!(TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));

        // enable source for another pair
        TradingPairPallet::disable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool,
        )
        .expect("Failed to enable source for pair.");
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert_eq!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &KSM)
                .expect("Failed to list enabled sources for pair.")
                .into_iter()
                .collect::<Vec<_>>(),
            vec![]
        );
        assert!(!TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
        assert!(!TradingPairPallet::is_source_enabled_for_trading_pair(
            &DEX_ID,
            &XOR,
            &KSM,
            LiquiditySourceType::XykPool
        )
        .expect("Failed to query pair state."));
    });
}

#[test]
fn duplicate_enabled_source_should_not_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_ok!(TradingPairPallet::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::MockPool,
        ));
        assert_ok!(TradingPairPallet::enable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::MockPool,
        ));
    });
}

#[test]
fn duplicate_disabled_source_should_not_fail() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_ok!(TradingPairPallet::disable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::MockPool,
        ));
        assert_ok!(TradingPairPallet::disable_source_for_trading_pair(
            &DEX_ID,
            &XOR,
            &DOT,
            LiquiditySourceType::MockPool,
        ));
    });
}

#[test]
fn should_not_enable_source_for_unregistered_pair() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_noop!(
            TradingPairPallet::enable_source_for_trading_pair(
                &DEX_ID,
                &XOR,
                &KSM,
                LiquiditySourceType::MockPool,
            ),
            Error::<Runtime>::TradingPairDoesntExist
        );
    });
}

#[test]
fn should_not_disable_source_for_unregistered_pair() {
    let mut ext = ExtBuilder::default().build();
    ext.execute_with(|| {
        TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT)
            .expect("Failed to register pair.");
        assert_noop!(
            TradingPairPallet::disable_source_for_trading_pair(
                &DEX_ID,
                &XOR,
                &KSM,
                LiquiditySourceType::MockPool,
            ),
            Error::<Runtime>::TradingPairDoesntExist
        );
    });
}

#[test]
fn should_fail_with_nonexistent_dex() {
    let mut ext = ExtBuilder::without_initialized_dex().build();
    ext.execute_with(|| {
        assert_noop!(
            TradingPairPallet::register(RuntimeOrigin::signed(ALICE), DEX_ID, XOR, DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::ensure_trading_pair_exists(&DEX_ID, &XOR, &DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::list_trading_pairs(&DEX_ID),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::is_trading_pair_enabled(&DEX_ID, &XOR, &DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::list_enabled_sources_for_trading_pair(&DEX_ID, &XOR, &DOT),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::is_source_enabled_for_trading_pair(
                &DEX_ID,
                &XOR,
                &DOT,
                LiquiditySourceType::MockPool
            ),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::enable_source_for_trading_pair(
                &DEX_ID,
                &XOR,
                &DOT,
                LiquiditySourceType::MockPool
            ),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
        assert_noop!(
            TradingPairPallet::disable_source_for_trading_pair(
                &DEX_ID,
                &XOR,
                &DOT,
                LiquiditySourceType::MockPool
            ),
            dex_manager::Error::<Runtime>::DEXDoesNotExist
        );
    });
}
