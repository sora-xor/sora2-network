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
#[cfg(feature = "try-runtime")]
mod remote;
mod xor_fee;

use crate::{genesis_config_presets, Currencies, Referrals, RuntimeOrigin};
use assets::GetTotalBalance;
use common::mock::{alice, bob};
use common::prelude::constants::SMALL_FEE;
use common::{fixed, SymbolName, KUSD, TBCD, VAL, XOR};
use frame_support::genesis_builder_helper;
use frame_support::traits::{GetStorageVersion, OnRuntimeUpgrade, StorageVersion};
use frame_support::{assert_err, assert_ok};
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

#[test]
fn pswap_buy_back_fractions_split_kusd_10_xor_90() {
    let fractions = <crate::Runtime as pswap_distribution::Config>::GetBuyBackFractions::get();
    let kusd_fraction = fractions
        .iter()
        .find_map(|(asset_id, fraction)| (*asset_id == KUSD.into()).then_some(*fraction));
    let xor_fraction = fractions
        .iter()
        .find_map(|(asset_id, fraction)| (*asset_id == XOR.into()).then_some(*fraction));

    assert_eq!(kusd_fraction, Some(sp_runtime::Permill::from_percent(4)));
    assert_eq!(xor_fraction, Some(sp_runtime::Permill::from_percent(36)));
    assert!(!fractions
        .iter()
        .any(|(asset_id, _)| *asset_id == TBCD.into()));
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

    #[cfg(feature = "private-net")]
    assert_eq!(balances.len(), 6);

    #[cfg(not(feature = "private-net"))]
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
    {
        let sudo_key = preset["sudo"]["key"]
            .as_str()
            .expect("private-net benchmark preset should include sudo");
        assert!(
            balances
                .iter()
                .any(|entry| entry[0].as_str() == Some(sudo_key)),
            "private-net benchmark preset should fund the sudo account"
        );
    }
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
        kensetsu::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(6)
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
        StorageVersion::new(4)
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
    assert_eq!(
        ::xor_fee::Pallet::<crate::Runtime>::in_code_storage_version(),
        StorageVersion::new(3)
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

pub(crate) fn demeter_storage_version_bridge_reaches_v3() {
    sp_io::TestExternalities::new_empty().execute_with(|| {
        demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::put(
            demeter_farming_platform::StorageVersion::V2,
        );

        crate::migrations::DemeterFarmingPlatformStorageVersionV3::on_runtime_upgrade();

        assert!(
            demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::get()
                == demeter_farming_platform::StorageVersion::V3
        );
    });

    sp_io::TestExternalities::new_empty().execute_with(|| {
        demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::put(
            demeter_farming_platform::StorageVersion::V3,
        );

        let weight =
            crate::migrations::DemeterFarmingPlatformStorageVersionV3::on_runtime_upgrade();

        assert!(
            demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::get()
                == demeter_farming_platform::StorageVersion::V3
        );
        assert_eq!(weight, frame_support::weights::Weight::zero());
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
pub(crate) fn demeter_storage_version_bridge_try_runtime_hooks() {
    sp_io::TestExternalities::new_empty().execute_with(|| {
        demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::put(
            demeter_farming_platform::StorageVersion::V2,
        );
        let state =
            crate::migrations::DemeterFarmingPlatformStorageVersionV3::pre_upgrade().unwrap();
        crate::migrations::DemeterFarmingPlatformStorageVersionV3::on_runtime_upgrade();
        crate::migrations::DemeterFarmingPlatformStorageVersionV3::post_upgrade(state).unwrap();
    });

    sp_io::TestExternalities::new_empty().execute_with(|| {
        demeter_farming_platform::PalletStorageVersion::<crate::Runtime>::put(
            demeter_farming_platform::StorageVersion::V3,
        );
        let state =
            crate::migrations::DemeterFarmingPlatformStorageVersionV3::pre_upgrade().unwrap();
        crate::migrations::DemeterFarmingPlatformStorageVersionV3::on_runtime_upgrade();
        crate::migrations::DemeterFarmingPlatformStorageVersionV3::post_upgrade(state).unwrap();
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

pub(crate) fn queue_ethereum_xor_thischain_add_asset_migration_queues_once() {
    use eth_bridge::requests::{OffchainRequest, OutgoingRequest, RequestStatus};

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let xor_asset_id: crate::AssetId = XOR.into();

        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        assert!(eth_bridge::migration::is_legacy_ethereum_xor_decommissioned::<crate::Runtime>());
        assert!(
            eth_bridge::Pallet::<crate::Runtime>::registered_asset(net_id, &xor_asset_id).is_none()
        );

        let initial_queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);
        assert!(!crate::migrations::ethereum_xor_thischain_add_asset_queued());

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();

        assert!(crate::migrations::ethereum_xor_thischain_add_asset_queued());
        let queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);
        assert_eq!(queue.len(), initial_queue.len() + 1);
        let request_hash = *queue.last().unwrap();
        assert_eq!(
            eth_bridge::RequestStatuses::<crate::Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
        match eth_bridge::Requests::<crate::Runtime>::get(net_id, request_hash) {
            Some(OffchainRequest::Outgoing(OutgoingRequest::AddAsset(req), _)) => {
                assert_eq!(req.network_id, net_id);
                assert_eq!(req.asset_id, xor_asset_id);
            }
            other => panic!("unexpected queued request: {:?}", other),
        }

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();
        assert_eq!(
            eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id),
            queue
        );
    });
}

pub(crate) fn queue_ethereum_xor_thischain_add_asset_migration_handles_full_queue() {
    use eth_bridge::requests::{OffchainRequest, OutgoingRequest, OutgoingTransfer, RequestStatus};

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let xor_asset_id: crate::AssetId = XOR.into();

        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        assert!(eth_bridge::migration::is_legacy_ethereum_xor_decommissioned::<crate::Runtime>());

        let max = crate::MaxEthBridgeRequestsPerQueue::get() as usize;
        for nonce in 0..max {
            let request_hash = sp_core::H256::from_low_u64_be(nonce as u64 + 1);
            let request = OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<
                crate::Runtime,
            > {
                from: alice(),
                to: sp_core::H160::from([7; 20]),
                asset_id: VAL.into(),
                amount: 1,
                nonce: nonce as crate::Index,
                network_id: net_id,
                timepoint: Default::default(),
            }));
            eth_bridge::Requests::<crate::Runtime>::insert(net_id, request_hash, request);
            eth_bridge::RequestStatuses::<crate::Runtime>::insert(
                net_id,
                request_hash,
                RequestStatus::Pending,
            );
            eth_bridge::RequestsQueue::<crate::Runtime>::mutate(net_id, |queue| {
                queue.push(request_hash)
            });
        }

        let full_queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);
        assert_eq!(full_queue.len(), max);
        assert_err!(
            crate::EthBridge::add_asset(crate::RuntimeOrigin::root(), xor_asset_id.clone(), net_id),
            eth_bridge::Error::<crate::Runtime>::RequestsQueueFull
        );
        assert_eq!(
            eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id),
            full_queue
        );

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();

        assert!(crate::migrations::ethereum_xor_thischain_add_asset_queued());
        let queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);
        assert_eq!(queue.len(), max + 1);
        assert_eq!(&queue[..max], full_queue.as_slice());
        let request_hash = *queue.last().unwrap();
        assert_eq!(
            eth_bridge::RequestStatuses::<crate::Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
        match eth_bridge::Requests::<crate::Runtime>::get(net_id, request_hash) {
            Some(OffchainRequest::Outgoing(OutgoingRequest::AddAsset(req), _)) => {
                assert_eq!(req.network_id, net_id);
                assert_eq!(req.asset_id, xor_asset_id);
            }
            other => panic!("unexpected queued request: {:?}", other),
        }
    });
}

pub(crate) fn queue_ethereum_xor_thischain_add_asset_migration_handles_adversarial_states() {
    use eth_bridge::requests::{
        AssetKind, OffchainRequest, OutgoingAddAsset, OutgoingRequest, RequestStatus,
    };

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let initial_queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();

        assert!(!eth_bridge::migration::is_legacy_ethereum_xor_decommissioned::<crate::Runtime>());
        assert!(!crate::migrations::ethereum_xor_thischain_add_asset_queued());
        assert_eq!(
            eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id),
            initial_queue
        );
    });

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let xor_asset_id: crate::AssetId = XOR.into();

        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        let stale_request =
            OffchainRequest::outgoing(OutgoingRequest::AddAsset(OutgoingAddAsset::<
                crate::Runtime,
            > {
                author: crate::EthBridge::authority_account().unwrap(),
                asset_id: xor_asset_id.clone(),
                nonce: 999u32 as crate::Index,
                network_id: net_id,
                timepoint: Default::default(),
            }));
        let stale_hash = match &stale_request {
            OffchainRequest::Outgoing(_, hash) => *hash,
            _ => unreachable!("stale add-asset request must be outgoing"),
        };
        eth_bridge::Requests::<crate::Runtime>::insert(net_id, stale_hash, stale_request);
        eth_bridge::RequestStatuses::<crate::Runtime>::insert(
            net_id,
            stale_hash,
            RequestStatus::Failed(sp_runtime::DispatchError::Other("stale add asset")),
        );
        eth_bridge::RequestsQueue::<crate::Runtime>::mutate(net_id, |queue| queue.push(stale_hash));
        let stale_queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);

        assert!(
            !eth_bridge::Pallet::<crate::Runtime>::is_add_asset_request_pending(
                net_id,
                xor_asset_id.clone()
            )
        );

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();

        assert!(crate::migrations::ethereum_xor_thischain_add_asset_queued());
        let queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);
        assert_eq!(queue.len(), stale_queue.len() + 1);
        assert_eq!(&queue[..stale_queue.len()], stale_queue.as_slice());
        let request_hash = *queue.last().unwrap();
        assert_eq!(
            eth_bridge::RequestStatuses::<crate::Runtime>::get(net_id, request_hash),
            Some(RequestStatus::Pending)
        );
        match eth_bridge::Requests::<crate::Runtime>::get(net_id, request_hash) {
            Some(OffchainRequest::Outgoing(OutgoingRequest::AddAsset(req), _)) => {
                assert_eq!(req.network_id, net_id);
                assert_eq!(req.asset_id, xor_asset_id);
            }
            other => panic!("unexpected queued request: {:?}", other),
        }
    });

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let xor_asset_id: crate::AssetId = XOR.into();

        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        crate::EthBridge::add_asset(crate::RuntimeOrigin::root(), xor_asset_id.clone(), net_id)
            .unwrap();
        let queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);
        assert!(queue.iter().any(|hash| {
            matches!(
                eth_bridge::Requests::<crate::Runtime>::get(net_id, hash),
                Some(OffchainRequest::Outgoing(OutgoingRequest::AddAsset(req), _))
                    if req.asset_id == xor_asset_id
            )
        }));

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();

        assert!(crate::migrations::ethereum_xor_thischain_add_asset_queued());
        assert_eq!(
            eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id),
            queue
        );
    });

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let xor_asset_id: crate::AssetId = XOR.into();

        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        OutgoingAddAsset::<crate::Runtime> {
            author: crate::EthBridge::authority_account().unwrap(),
            asset_id: xor_asset_id.clone(),
            nonce: Default::default(),
            network_id: net_id,
            timepoint: Default::default(),
        }
        .finalize()
        .unwrap();
        let queue = eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id);

        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();

        assert!(crate::migrations::ethereum_xor_thischain_add_asset_queued());
        assert_eq!(
            eth_bridge::Pallet::<crate::Runtime>::registered_asset(net_id, &xor_asset_id),
            Some(AssetKind::Thischain)
        );
        assert_eq!(
            eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id),
            queue
        );
    });
}

#[cfg(feature = "try-runtime")]
pub(crate) fn legacy_ethereum_xor_decommission_try_runtime_hooks() {
    use eth_bridge::requests::{OffchainRequest, OutgoingRequest, OutgoingTransfer, RequestStatus};

    ext().execute_with(|| {
        let state = crate::migrations::DecommissionLegacyEthereumXor::pre_upgrade().unwrap();
        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        crate::migrations::DecommissionLegacyEthereumXor::post_upgrade(state).unwrap();
    });

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let request = OffchainRequest::outgoing(OutgoingRequest::Transfer(OutgoingTransfer::<
            crate::Runtime,
        > {
            from: alice(),
            to: sp_core::H160::from([7; 20]),
            asset_id: XOR.into(),
            amount: 1,
            nonce: 1,
            network_id: net_id,
            timepoint: Default::default(),
        }));
        let request_hash = sp_core::H256::repeat_byte(42);
        eth_bridge::Requests::<crate::Runtime>::insert(net_id, request_hash, request);
        eth_bridge::RequestStatuses::<crate::Runtime>::insert(
            net_id,
            request_hash,
            RequestStatus::ApprovalsReady,
        );
        eth_bridge::RequestsQueue::<crate::Runtime>::mutate(net_id, |queue| {
            queue.push(request_hash)
        });

        assert!(eth_bridge::Requests::<crate::Runtime>::contains_key(
            net_id,
            request_hash
        ));
        assert_eq!(
            eth_bridge::RequestStatuses::<crate::Runtime>::get(net_id, request_hash),
            Some(RequestStatus::ApprovalsReady)
        );
        assert!(matches!(
            eth_bridge::Requests::<crate::Runtime>::get(net_id, request_hash),
            Some(OffchainRequest::Outgoing(
                OutgoingRequest::Transfer(OutgoingTransfer { asset_id, .. }),
                _
            )) if asset_id == XOR.into()
        ));
        assert_eq!(
            eth_bridge::migration::legacy_ethereum_xor_decommission_blockers::<crate::Runtime>(),
            1
        );
        assert!(crate::migrations::DecommissionLegacyEthereumXor::pre_upgrade().is_err());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        }));
        assert!(result.is_err());
        assert!(!eth_bridge::migration::is_legacy_ethereum_xor_decommissioned::<crate::Runtime>());
        assert_eq!(
            eth_bridge::migration::legacy_ethereum_xor_decommission_blockers::<crate::Runtime>(),
            1
        );
    });
}

#[cfg(feature = "try-runtime")]
pub(crate) fn queue_ethereum_xor_thischain_add_asset_try_runtime_hooks() {
    use eth_bridge::requests::{OffchainRequest, OutgoingRequest};

    ext().execute_with(|| {
        let net_id = crate::GetEthNetworkId::get();
        let xor_asset_id: crate::AssetId = XOR.into();

        let decommission_state =
            crate::migrations::DecommissionLegacyEthereumXor::pre_upgrade().unwrap();
        crate::migrations::DecommissionLegacyEthereumXor::on_runtime_upgrade();
        crate::migrations::DecommissionLegacyEthereumXor::post_upgrade(decommission_state).unwrap();

        let state = crate::migrations::QueueEthereumXorThischainAddAsset::pre_upgrade().unwrap();
        crate::migrations::QueueEthereumXorThischainAddAsset::on_runtime_upgrade();
        crate::migrations::QueueEthereumXorThischainAddAsset::post_upgrade(state).unwrap();

        assert!(crate::migrations::ethereum_xor_thischain_add_asset_queued());
        assert!(
            eth_bridge::Pallet::<crate::Runtime>::is_add_asset_request_pending(
                net_id,
                xor_asset_id.clone()
            )
        );
        let request_hash = *eth_bridge::RequestsQueue::<crate::Runtime>::get(net_id)
            .last()
            .unwrap();
        match eth_bridge::Requests::<crate::Runtime>::get(net_id, request_hash) {
            Some(OffchainRequest::Outgoing(OutgoingRequest::AddAsset(req), _)) => {
                assert_eq!(req.network_id, net_id);
                assert_eq!(req.asset_id, xor_asset_id);
            }
            other => panic!("unexpected queued request: {:?}", other),
        }
    });
}

#[cfg(feature = "try-runtime")]
pub(crate) async fn remote_try_runtime_upgrade_rehearsal() {
    remote::remote_try_runtime_upgrade_rehearsal().await;
}
