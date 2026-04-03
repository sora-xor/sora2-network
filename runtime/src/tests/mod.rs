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

mod liquidity_proxy;
mod referrals;
mod remote;
mod xor_fee;

use crate::{genesis_config_presets, Currencies, Referrals, RuntimeOrigin};
use assets::GetTotalBalance;
use common::mock::{alice, bob};
use common::prelude::constants::SMALL_FEE;
use common::{fixed, SymbolName, XOR};
use frame_support::assert_ok;
use frame_support::genesis_builder_helper;
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use framenode_chain_spec::ext;
use serde_json::Value;

fn benchmark_preset_json() -> Value {
    let preset_id = genesis_config_presets::BENCHMARK_RUNTIME_PRESET.to_owned();
    let preset =
        genesis_config_presets::get_preset(&preset_id).expect("benchmark preset should serialize");
    serde_json::from_slice(&preset).expect("benchmark preset is valid json")
}

fn merge_json_patch(base: &mut Value, patch: Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    base.remove(&key);
                } else {
                    merge_json_patch(base.entry(key).or_insert(Value::Null), value);
                }
            }
        }
        (slot, replacement) => *slot = replacement,
    }
}

fn ensure_phantom_field(config: &mut Value, key: &str) {
    let root = config
        .as_object_mut()
        .expect("runtime genesis config should be a json object");
    let section = root
        .entry(key.to_owned())
        .or_insert_with(|| Value::Object(Default::default()));
    let section = section
        .as_object_mut()
        .expect("genesis section should be a json object");
    section.entry("phantom".to_owned()).or_insert(Value::Null);
}

fn merged_benchmark_genesis_json() -> Vec<u8> {
    let mut default_config: Value = serde_json::from_slice(
        &genesis_builder_helper::get_preset::<crate::RuntimeGenesisConfig>(&None, |_| None)
            .expect("default runtime genesis config should serialize"),
    )
    .expect("default runtime genesis config is valid json");
    merge_json_patch(&mut default_config, benchmark_preset_json());
    ensure_phantom_field(&mut default_config, "dexapi");
    serde_json::to_vec(&default_config).expect("merged benchmark genesis should serialize")
}

fn value_for_key<'a>(
    object: &'a serde_json::Map<String, Value>,
    snake_case: &str,
    camel_case: &str,
) -> &'a Value {
    object
        .get(snake_case)
        .or_else(|| object.get(camel_case))
        .unwrap_or_else(|| panic!("missing `{snake_case}`/`{camel_case}` in preset"))
}

pub(crate) fn get_total_balance() {
    ext().execute_with(|| {
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            alice(),
            XOR.into(),
            (SMALL_FEE * 3) as i128
        ));
        Referrals::reserve(RuntimeOrigin::signed(alice()), SMALL_FEE).unwrap();
        assert_eq!(
            crate::GetTotalBalance::total_balance(&XOR, &alice()),
            Ok(SMALL_FEE)
        );

        assert_eq!(crate::GetTotalBalance::total_balance(&XOR, &bob()), Ok(0));
    });
}

pub(crate) fn benchmark_genesis_preset_is_available() {
    let preset_id = genesis_config_presets::BENCHMARK_RUNTIME_PRESET.to_owned();
    let preset_names = genesis_config_presets::preset_names();
    assert!(preset_names.contains(&preset_id));

    let preset =
        genesis_config_presets::get_preset(&preset_id).expect("benchmark preset should serialize");
    let preset = String::from_utf8(preset).expect("preset is valid json");
    assert!(preset.contains("\"balances\""));
    assert!(preset.contains("\"staking\""));
}

pub(crate) fn benchmark_genesis_preset_has_expected_shape() {
    let preset = benchmark_preset_json();

    let balances = preset["balances"]["balances"]
        .as_array()
        .expect("balances preset is an array");
    assert_eq!(balances.len(), 5);

    let assets = preset["assets"]
        .as_object()
        .expect("assets preset is an object");
    assert!(
        !assets.is_empty(),
        "benchmark preset should customize at least one asset field"
    );

    let session_keys = preset["session"]["keys"]
        .as_array()
        .expect("session keys preset is an array");
    assert_eq!(session_keys.len(), 1);

    let staking = preset["staking"]
        .as_object()
        .expect("staking preset is an object");
    assert_eq!(
        value_for_key(staking, "validator_count", "validatorCount").as_u64(),
        Some(1),
    );
    assert_eq!(
        value_for_key(staking, "minimum_validator_count", "minimumValidatorCount").as_u64(),
        Some(1),
    );
    assert_eq!(
        value_for_key(staking, "slash_reward_fraction", "slashRewardFraction")
            .as_u64()
            .expect("slash reward fraction should be numeric"),
        100_000_000,
    );
    assert_eq!(
        staking["stakers"]
            .as_array()
            .expect("stakers preset is an array")
            .len(),
        1,
    );

    #[cfg(feature = "private-net")]
    assert!(
        preset["sudo"]["key"].is_string(),
        "private-net benchmark preset should include sudo"
    );
}

pub(crate) fn benchmark_genesis_preset_builds_state() {
    sp_io::TestExternalities::new_empty().execute_with(|| {
        genesis_builder_helper::build_state::<crate::RuntimeGenesisConfig>(
            merged_benchmark_genesis_json(),
        )
        .expect("benchmark preset should build runtime state when applied to the default config");
    });
}

pub(crate) fn unknown_benchmark_genesis_preset_is_rejected() {
    let preset_id = "does-not-exist".to_owned();
    assert!(genesis_config_presets::get_preset(&preset_id).is_none());
    assert!(
        genesis_builder_helper::get_preset::<crate::RuntimeGenesisConfig>(&Some(preset_id), |id| {
            genesis_config_presets::get_preset(id)
        })
        .is_none()
    );
}

pub(crate) fn runtime_upgrade_storage_versions_match_expected_code_versions() {
    assert_eq!(
        band::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(2)
    );
    assert_eq!(
        farming::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(2)
    );
    assert_eq!(
        oracle_proxy::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(1)
    );
    assert_eq!(
        pool_xyk::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(3)
    );
    assert_eq!(
        pswap_distribution::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(2)
    );
    assert_eq!(
        pallet_polkamarkt::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(1)
    );
    assert_eq!(
        vested_rewards::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(4)
    );
    assert_eq!(
        bridge_channel::inbound::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(1)
    );
    assert_eq!(
        substrate_bridge_channel::inbound::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(1)
    );
    assert_eq!(
        substrate_bridge_channel::outbound::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(1)
    );
    #[cfg(feature = "wip")]
    assert_eq!(
        ::xor_fee::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(2)
    );
    #[cfg(not(feature = "wip"))]
    assert_eq!(
        ::xor_fee::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(1)
    );
}

pub(crate) fn runtime_upgrade_version_only_migrations_bump_zero_to_one() {
    ext().execute_with(|| {
        StorageVersion::new(0).put::<oracle_proxy::Pallet<crate::Runtime>>();
        StorageVersion::new(0).put::<bridge_channel::inbound::Pallet<crate::Runtime>>();
        StorageVersion::new(0).put::<substrate_bridge_channel::inbound::Pallet<crate::Runtime>>();
        StorageVersion::new(0).put::<substrate_bridge_channel::outbound::Pallet<crate::Runtime>>();

        crate::migrations::OracleProxyStorageVersionV1::on_runtime_upgrade();
        crate::migrations::BridgeInboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::on_runtime_upgrade();

        assert_eq!(
            oracle_proxy::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(1)
        );
        assert_eq!(
            bridge_channel::inbound::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(1)
        );
        assert_eq!(
            substrate_bridge_channel::inbound::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(1)
        );
        assert_eq!(
            substrate_bridge_channel::outbound::Pallet::<crate::Runtime>::on_chain_storage_version(
            ),
            StorageVersion::new(1)
        );
    });
}

pub(crate) fn band_migrate_to_v2_if_needed_handles_expected_versions() {
    use band::migrations::storages::{BandRateV1, SymbolRatesV1};

    let symbol: crate::Symbol = SymbolName::usd();
    ext().execute_with(|| {
        crate::System::set_block_number(1);
        StorageVersion::new(1).put::<band::Pallet<crate::Runtime>>();
        SymbolRatesV1::<crate::Runtime>::insert(
            symbol.clone(),
            Some(BandRateV1 {
                value: 0,
                last_updated: 0,
                request_id: 0,
                dynamic_fee: fixed!(0),
            }),
        );

        crate::migrations::BandMigrateToV2IfNeeded::on_runtime_upgrade();

        assert_eq!(
            band::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(2)
        );
        assert_eq!(
            band::SymbolRates::<crate::Runtime>::get(symbol.clone())
                .expect("band rate should still exist after migration")
                .last_updated_block,
            1
        );
        assert!(band::SymbolCheckBlock::<crate::Runtime>::get(
            1 + crate::GetBandRateStaleBlockPeriod::get(),
            symbol
        ));
    });

    ext().execute_with(|| {
        let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
        StorageVersion::new(2).put::<band::Pallet<crate::Runtime>>();
        let weight = crate::migrations::BandMigrateToV2IfNeeded::on_runtime_upgrade();
        assert_eq!(
            band::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(2)
        );
        assert_eq!(weight, db_weight.reads(1));
    });

    ext().execute_with(|| {
        let db_weight = <crate::Runtime as frame_system::Config>::DbWeight::get();
        StorageVersion::new(0).put::<band::Pallet<crate::Runtime>>();
        let weight = crate::migrations::BandMigrateToV2IfNeeded::on_runtime_upgrade();
        assert_eq!(
            band::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(0)
        );
        assert_eq!(weight, db_weight.reads(1));
    });
}

pub(crate) fn staking_storage_version_bridge_reaches_v16() {
    for starting_version in [13, 14, 15] {
        ext().execute_with(|| {
            StorageVersion::new(starting_version).put::<pallet_staking::Pallet<crate::Runtime>>();
            crate::migrations::StakingStorageVersionV16::on_runtime_upgrade();
            assert_eq!(
                pallet_staking::Pallet::<crate::Runtime>::on_chain_storage_version(),
                StorageVersion::new(16),
                "staking bridge migration should finish at v16 from v{starting_version}"
            );
        });
    }

    ext().execute_with(|| {
        let in_code = pallet_staking::Pallet::<crate::Runtime>::in_code_storage_version();
        StorageVersion::new(16).put::<pallet_staking::Pallet<crate::Runtime>>();
        crate::migrations::StakingStorageVersionV16::on_runtime_upgrade();
        assert_eq!(
            pallet_staking::Pallet::<crate::Runtime>::on_chain_storage_version(),
            in_code
        );
    });
}

#[cfg(feature = "try-runtime")]
pub(crate) fn band_migrate_to_v2_if_needed_try_runtime_hooks() {
    use band::migrations::storages::{BandRateV1, SymbolRatesV1};

    let symbol: crate::Symbol = SymbolName::usd();
    ext().execute_with(|| {
        crate::System::set_block_number(1);
        StorageVersion::new(1).put::<band::Pallet<crate::Runtime>>();
        SymbolRatesV1::<crate::Runtime>::insert(
            symbol.clone(),
            Some(BandRateV1 {
                value: 0,
                last_updated: 0,
                request_id: 0,
                dynamic_fee: fixed!(0),
            }),
        );
        let state = crate::migrations::BandMigrateToV2IfNeeded::pre_upgrade().unwrap();
        crate::migrations::BandMigrateToV2IfNeeded::on_runtime_upgrade();
        crate::migrations::BandMigrateToV2IfNeeded::post_upgrade(state).unwrap();
        assert_eq!(
            band::Pallet::<crate::Runtime>::on_chain_storage_version(),
            StorageVersion::new(2)
        );
    });

    ext().execute_with(|| {
        StorageVersion::new(2).put::<band::Pallet<crate::Runtime>>();
        let state = crate::migrations::BandMigrateToV2IfNeeded::pre_upgrade().unwrap();
        crate::migrations::BandMigrateToV2IfNeeded::on_runtime_upgrade();
        crate::migrations::BandMigrateToV2IfNeeded::post_upgrade(state).unwrap();
    });
}

#[cfg(feature = "try-runtime")]
pub(crate) fn runtime_upgrade_version_only_migrations_try_runtime_hooks() {
    ext().execute_with(|| {
        StorageVersion::new(0).put::<oracle_proxy::Pallet<crate::Runtime>>();
        let state = crate::migrations::OracleProxyStorageVersionV1::pre_upgrade().unwrap();
        crate::migrations::OracleProxyStorageVersionV1::on_runtime_upgrade();
        crate::migrations::OracleProxyStorageVersionV1::post_upgrade(state).unwrap();

        StorageVersion::new(0).put::<bridge_channel::inbound::Pallet<crate::Runtime>>();
        let state = crate::migrations::BridgeInboundChannelStorageVersionV1::pre_upgrade().unwrap();
        crate::migrations::BridgeInboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::BridgeInboundChannelStorageVersionV1::post_upgrade(state).unwrap();

        StorageVersion::new(0).put::<substrate_bridge_channel::inbound::Pallet<crate::Runtime>>();
        let state = crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::pre_upgrade()
            .unwrap();
        crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::post_upgrade(state)
            .unwrap();

        StorageVersion::new(0).put::<substrate_bridge_channel::outbound::Pallet<crate::Runtime>>();
        let state =
            crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::pre_upgrade()
                .unwrap();
        crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::post_upgrade(state)
            .unwrap();
    });

    ext().execute_with(|| {
        StorageVersion::new(1).put::<oracle_proxy::Pallet<crate::Runtime>>();
        let state = crate::migrations::OracleProxyStorageVersionV1::pre_upgrade().unwrap();
        crate::migrations::OracleProxyStorageVersionV1::on_runtime_upgrade();
        crate::migrations::OracleProxyStorageVersionV1::post_upgrade(state).unwrap();

        StorageVersion::new(1).put::<bridge_channel::inbound::Pallet<crate::Runtime>>();
        let state = crate::migrations::BridgeInboundChannelStorageVersionV1::pre_upgrade().unwrap();
        crate::migrations::BridgeInboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::BridgeInboundChannelStorageVersionV1::post_upgrade(state).unwrap();

        StorageVersion::new(1).put::<substrate_bridge_channel::inbound::Pallet<crate::Runtime>>();
        let state = crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::pre_upgrade()
            .unwrap();
        crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::SubstrateBridgeInboundChannelStorageVersionV1::post_upgrade(state)
            .unwrap();

        StorageVersion::new(1).put::<substrate_bridge_channel::outbound::Pallet<crate::Runtime>>();
        let state =
            crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::pre_upgrade()
                .unwrap();
        crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::on_runtime_upgrade();
        crate::migrations::SubstrateBridgeOutboundChannelStorageVersionV1::post_upgrade(state)
            .unwrap();
    });
}

#[cfg(feature = "try-runtime")]
pub(crate) fn staking_storage_version_bridge_try_runtime_hooks() {
    for starting_version in [13, 14, 15, 16] {
        ext().execute_with(|| {
            StorageVersion::new(starting_version).put::<pallet_staking::Pallet<crate::Runtime>>();
            let state = crate::migrations::StakingStorageVersionV16::pre_upgrade().unwrap();
            crate::migrations::StakingStorageVersionV16::on_runtime_upgrade();
            crate::migrations::StakingStorageVersionV16::post_upgrade(state).unwrap();
        });
    }
}

#[cfg(feature = "try-runtime")]
pub(crate) fn bridge_peer_isolation_audit_try_runtime_hooks() {
    use bridge_types::{GenericNetworkId, SubNetworkId};
    use frame_support::BoundedBTreeSet;
    use sp_core::{bounded::BoundedVec, ecdsa};
    use sp_std::collections::btree_set::BTreeSet;

    fn peer(byte: u8) -> ecdsa::Public {
        let mut raw = [byte; 33];
        raw[32] = raw[32].saturating_add(1);
        ecdsa::Public::from_raw(raw)
    }

    ext().execute_with(|| {
        let mainnet = GenericNetworkId::Sub(SubNetworkId::Mainnet);
        let clean_peers: BoundedVec<ecdsa::Public, crate::BridgeMaxPeers> =
            vec![peer(1), peer(2), peer(3), peer(4)].try_into().unwrap();

        assert_ok!(crate::BridgeDataSigner::register_network(
            crate::RuntimeOrigin::root(),
            mainnet,
            clean_peers.clone(),
        ));
        assert_ok!(crate::MultisigVerifier::initialize(
            crate::RuntimeOrigin::root(),
            mainnet,
            clean_peers,
        ));

        let state = crate::migrations::BridgePeerIsolationAudit::pre_upgrade().unwrap();
        crate::migrations::BridgePeerIsolationAudit::on_runtime_upgrade();
        crate::migrations::BridgePeerIsolationAudit::post_upgrade(state).unwrap();
    });

    ext().execute_with(|| {
        let overlapping_peer = peer(20);
        let mainnet: BoundedBTreeSet<ecdsa::Public, crate::BridgeMaxPeers> =
            vec![peer(21), peer(22), peer(23), overlapping_peer]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .try_into()
                .unwrap();
        let kusama: BoundedBTreeSet<ecdsa::Public, crate::BridgeMaxPeers> =
            vec![peer(30), peer(31), peer(32), overlapping_peer]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .try_into()
                .unwrap();

        multisig_verifier::PeerKeys::<crate::Runtime>::insert(
            GenericNetworkId::Sub(SubNetworkId::Mainnet),
            mainnet,
        );
        multisig_verifier::PeerKeys::<crate::Runtime>::insert(
            GenericNetworkId::Sub(SubNetworkId::Kusama),
            kusama,
        );

        assert!(crate::migrations::BridgePeerIsolationAudit::pre_upgrade().is_err());
    });
}

pub(crate) async fn remote_try_runtime_upgrade_rehearsal() {
    remote::remote_try_runtime_upgrade_rehearsal().await;
}
