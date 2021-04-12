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

#![warn(unused_extern_crates)]

//! Service implementation. Specialized wrapper over substrate service.

use codec::Encode;
use framenode_runtime::eth_bridge::{
    self, PeerConfig, STORAGE_ETH_NODE_PARAMS, STORAGE_NETWORK_IDS_KEY, STORAGE_PEER_SECRET_KEY,
    STORAGE_SUB_NODE_URL_KEY,
};
use framenode_runtime::opaque::Block;
use framenode_runtime::{self, Runtime, RuntimeApi};
use sc_client_api::{Backend, ExecutorProvider, RemoteBackend};
use sc_executor::native_executor_instance;
pub use sc_executor::NativeExecutor;
use sc_finality_grandpa::SharedVoterState;
use sc_service::error::Error as ServiceError;
use sc_service::{Configuration, TaskManager};
use sp_core::offchain::OffchainStorage;
use sp_core::{Pair, Public};
use sp_inherents::InherentDataProviders;
use sp_keystore::SyncCryptoStore;
use sp_runtime::offchain::STORAGE_PREFIX;
use std::collections::BTreeSet;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;

// Our native executor instance.
native_executor_instance!(
    pub Executor,
    framenode_runtime::api::dispatch,
    framenode_runtime::native_version,
    frame_benchmarking::benchmarking::HostFunctions,
);

type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullGrandpaBlockImport =
    sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;

pub fn new_partial(
    config: &Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sp_consensus::DefaultImportQueue<Block, FullClient>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>,
            sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
            sc_consensus_babe::BabeLink<Block>,
        ),
    >,
    ServiceError,
> {
    if config.keystore_remote.is_some() {
        return Err(ServiceError::Other(format!(
            "Remote Keystores are not supported."
        )));
    }
    let inherent_data_providers = sp_inherents::InherentDataProviders::new();

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
    let client = Arc::new(client);
    let mut bridge_peer_secret_key = None;

    if let Some(first_pk_raw) =
        SyncCryptoStore::keys(&*keystore_container.sync_keystore(), eth_bridge::KEY_TYPE)
            .unwrap()
            .first()
            .map(|x| x.1.clone())
    {
        let pk = eth_bridge::crypto::Public::from_slice(&first_pk_raw[..]);
        if let Some(keystore) = keystore_container.local_keystore() {
            if let Ok(kep) = keystore.key_pair::<eth_bridge::crypto::Pair>(&pk) {
                let seed = kep.to_raw_vec();
                bridge_peer_secret_key = Some(seed);
            }
        }
    } else {
        log::debug!("Ethereum bridge peer key not found.")
    }

    if let Some(sk) = bridge_peer_secret_key {
        let mut storage = backend.offchain_storage().unwrap();
        storage.set(STORAGE_PREFIX, STORAGE_PEER_SECRET_KEY, &sk.encode());

        let path = config
            .network
            .net_config_path
            .clone()
            .or(config.database.path().map(|x| x.to_owned()))
            .expect("Expected network or database path.");
        let bridge_path = path
            .ancestors()
            .skip(1)
            .next()
            .map(|x| {
                let mut x = x.to_owned();
                x.push("bridge/eth.json");
                x
            })
            .unwrap();
        let file = File::open(&bridge_path).expect(&format!(
            "Ethereum bridge node config not found. Expected path: {:?}",
            bridge_path
        ));
        let peer_config: PeerConfig<<Runtime as eth_bridge::Config>::NetworkId> =
            serde_json::from_reader(&file).expect("Invalid ethereum bridge node config.");
        let mut network_ids = BTreeSet::new();
        for (net_id, params) in peer_config.networks {
            // TODO: optimize storage key construction.
            let string = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, net_id);
            storage.set(STORAGE_PREFIX, string.as_bytes(), &params.encode());
            network_ids.insert(net_id);
        }
        storage.set(
            STORAGE_PREFIX,
            STORAGE_NETWORK_IDS_KEY,
            &network_ids.encode(),
        );
        let rpc_addr = config
            .rpc_http
            .as_ref()
            .expect("HTTP RPC should be enabled for ethereum bridge. Please enable it via `--rpc-port <port>`.");
        storage.set(
            STORAGE_PREFIX,
            STORAGE_SUB_NODE_URL_KEY,
            &format!("http://{}", rpc_addr).encode(),
        );
        log::info!("Ethereum bridge peer initialized");
    }

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_handle(),
        client.clone(),
    );

    let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
        client.clone(),
        &(client.clone() as Arc<_>),
        select_chain.clone(),
    )?;

    let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
        sc_consensus_babe::Config::get_or_compute(&*client)?,
        grandpa_block_import.clone(),
        client.clone(),
    )?;

    let import_queue = sc_consensus_babe::import_queue(
        babe_link.clone(),
        babe_block_import.clone(),
        Some(Box::new(grandpa_block_import)),
        client.clone(),
        select_chain.clone(),
        inherent_data_providers.clone(),
        &task_manager.spawn_handle(),
        config.prometheus_registry(),
        sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
    )?;

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        inherent_data_providers,
        other: (babe_block_import, grandpa_link, babe_link),
    })
}

/// Builds a new service for a full client.
pub fn new_full(mut config: Configuration) -> Result<TaskManager, ServiceError> {
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        inherent_data_providers,
        other: (babe_block_import, grandpa_link, babe_link),
    } = new_partial(&config)?;

    if let Some(url) = &config.keystore_remote {
        return Err(ServiceError::Other(format!(
            "Error hooking up remote keystore for {}: Remote Keystore not supported.",
            url
        )));
    }

    config
        .network
        .extra_sets
        .push(sc_finality_grandpa::grandpa_peers_set_config());

    let (network, network_status_sinks, system_rpc_tx, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            on_demand: None,
            block_announce_validator_builder: None,
        })?;

    if config.offchain_worker.enabled {
        sc_service::build_offchain_workers(
            &config,
            backend.clone(),
            task_manager.spawn_handle(),
            client.clone(),
            network.clone(),
        )
        .expect("failed to build offchain workers");
    }

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();

        Box::new(move |deny_unsafe, _| {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
                deny_unsafe,
            };

            crate::rpc::create_full(deps)
        })
    };

    let (_rpc_handlers, telemetry_connection_notifier) =
        sc_service::spawn_tasks(sc_service::SpawnTasksParams {
            network: network.clone(),
            client: client.clone(),
            keystore: keystore_container.sync_keystore(),
            task_manager: &mut task_manager,
            transaction_pool: transaction_pool.clone(),
            rpc_extensions_builder,
            on_demand: None,
            remote_blockchain: None,
            backend,
            network_status_sinks,
            system_rpc_tx,
            config,
        })?;

    if role.is_authority() {
        let proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool,
            prometheus_registry.as_ref(),
        );

        let backoff_authoring_blocks =
            Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
        let can_author_with =
            sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());
        let babe_config = sc_consensus_babe::BabeParams {
            keystore: keystore_container.sync_keystore(),
            client: client.clone(),
            select_chain,
            env: proposer,
            block_import: babe_block_import,
            sync_oracle: network.clone(),
            inherent_data_providers: inherent_data_providers.clone(),
            force_authoring,
            babe_link,
            can_author_with,
            backoff_authoring_blocks,
        };

        let babe = sc_consensus_babe::start_babe(babe_config)?;

        task_manager
            .spawn_essential_handle()
            .spawn_blocking("babe-proposer", babe);
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore = if role.is_authority() {
        Some(keystore_container.sync_keystore())
    } else {
        None
    };

    let grandpa_config = sc_finality_grandpa::Config {
        // FIXME #1578 make this available through chainspec
        gossip_duration: Duration::from_millis(333),
        justification_period: 512,
        name: Some(name),
        observer_enabled: false,
        keystore,
        is_authority: role.is_network_authority(),
    };

    if enable_grandpa {
        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_config = sc_finality_grandpa::GrandpaParams {
            config: grandpa_config,
            link: grandpa_link,
            network,
            telemetry_on_connect: telemetry_connection_notifier.map(|x| x.on_connect_stream()),
            voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry,
            shared_voter_state: SharedVoterState::empty(),
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "sc_finality_grandpa-voter",
            sc_finality_grandpa::run_grandpa_voter(grandpa_config)?,
        );
    }

    network_starter.start_network();
    Ok(task_manager)
}

/// Builds a new service for a light client.
pub fn new_light(mut config: Configuration) -> Result<TaskManager, ServiceError> {
    let (client, backend, keystore_container, mut task_manager, on_demand) =
        sc_service::new_light_parts::<Block, RuntimeApi, Executor>(&config)?;

    config
        .network
        .extra_sets
        .push(sc_finality_grandpa::grandpa_peers_set_config());

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
        config.transaction_pool.clone(),
        config.prometheus_registry(),
        task_manager.spawn_handle(),
        client.clone(),
        on_demand.clone(),
    ));

    let (grandpa_block_import, _) = sc_finality_grandpa::block_import(
        client.clone(),
        &(client.clone() as Arc<_>),
        select_chain.clone(),
    )?;

    let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
        sc_consensus_babe::Config::get_or_compute(&*client)?,
        grandpa_block_import.clone(),
        client.clone(),
    )?;

    let import_queue = sc_consensus_babe::import_queue(
        babe_link,
        babe_block_import,
        Some(Box::new(grandpa_block_import)),
        client.clone(),
        select_chain.clone(),
        InherentDataProviders::new(),
        &task_manager.spawn_handle(),
        config.prometheus_registry(),
        sp_consensus::NeverCanAuthor,
    )?;

    let (network, network_status_sinks, system_rpc_tx, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            on_demand: Some(on_demand.clone()),
            block_announce_validator_builder: None,
        })?;

    if config.offchain_worker.enabled {
        sc_service::build_offchain_workers(
            &config,
            backend.clone(),
            task_manager.spawn_handle(),
            client.clone(),
            network.clone(),
        );
    }

    sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        remote_blockchain: Some(backend.remote_blockchain()),
        transaction_pool,
        task_manager: &mut task_manager,
        on_demand: Some(on_demand),
        rpc_extensions_builder: Box::new(|_, _| ()),
        config,
        client,
        keystore: keystore_container.sync_keystore(),
        backend,
        network,
        network_status_sinks,
        system_rpc_tx,
    })?;

    network_starter.start_network();

    Ok(task_manager)
}
