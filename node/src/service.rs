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
    self, PeerConfig, STORAGE_ETH_NODE_PARAMS, STORAGE_NETWORK_IDS_KEY, STORAGE_PEER_MARKER_KEY,
    STORAGE_PEER_SECRET_KEY, STORAGE_SUB_NODE_URL_KEY,
};
use framenode_runtime::opaque::Block;
use framenode_runtime::{self, BeefyId, Runtime, RuntimeApi};
use log::debug;
use prometheus_endpoint::Registry;
use sc_client_api::{Backend, BlockBackend};
use sc_executor::WasmExecutor;
use sc_network::NetworkBackend;
use sc_network_sync::strategy::warp::WarpSyncConfig;
use sc_service::config::PrometheusConfig;
use sc_service::error::Error as ServiceError;
use sc_service::{Configuration, TaskManager};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sp_consensus_babe::inherents::BabeCreateInherentDataProviders;
use sp_core::offchain::OffchainStorage;
use sp_core::ByteArray;
use sp_keystore::Keystore;
use sp_runtime::offchain::STORAGE_PREFIX;
use sp_runtime::traits::{Block as BlockT, IdentifyAccount};
use std::collections::BTreeSet;
use std::fs::File;
use std::sync::Arc;
use std::time::Duration;
use telemetry::{Telemetry, TelemetryWorker, TelemetryWorkerHandle};

type HostFunctions = sp_io::SubstrateHostFunctions;
type FullClient = sc_service::TFullClient<Block, RuntimeApi, WasmExecutor<HostFunctions>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type TransactionPool = sc_transaction_pool::TransactionPoolHandle<Block, FullClient>;
type FullGrandpaBlockImport =
    sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;
type FullBeefyBlockImport = beefy_gadget::import::BeefyBlockImport<
    Block,
    FullBackend,
    FullClient,
    FullGrandpaBlockImport,
    BeefyId,
>;
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

// If we're using prometheus, use a registry with a prefix of `polkadot`.
fn set_prometheus_registry(config: &mut Configuration) -> Result<(), ServiceError> {
    if let Some(PrometheusConfig { registry, .. }) = config.prometheus_config.as_mut() {
        *registry = Registry::new_custom(Some("polkadot".into()), None)?;
    }

    Ok(())
}

/// The native executor instance for benchmarking commands.
#[cfg(feature = "runtime-benchmarks")]
pub struct ExecutorDispatch;

#[cfg(feature = "runtime-benchmarks")]
impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
    type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        framenode_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        framenode_runtime::native_version()
    }
}

pub fn new_partial(
    config: &mut Configuration,
    telemetry_worker_handle: Option<TelemetryWorkerHandle>,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sc_consensus::DefaultImportQueue<Block>,
        TransactionPool,
        (
            impl Fn(
                sc_rpc::SubscriptionTaskExecutor,
            ) -> Result<crate::rpc::RpcExtension, sc_service::Error>,
            (
                sc_consensus_babe::BabeBlockImport<
                    Block,
                    FullClient,
                    FullBeefyBlockImport,
                    BabeCreateInherentDataProviders<Block>,
                    FullSelectChain,
                >,
                sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
                sc_consensus_babe::BabeLink<Block>,
                beefy_gadget::BeefyVoterLinks<Block, BeefyId>,
            ),
            sc_finality_grandpa::SharedVoterState,
            Option<Telemetry>,
        ),
    >,
    ServiceError,
> {
    set_prometheus_registry(config)?;

    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(move |endpoints| -> Result<_, telemetry::Error> {
            let (worker, mut worker_handle) = if let Some(worker_handle) = telemetry_worker_handle {
                (None, worker_handle)
            } else {
                let worker = TelemetryWorker::new(16)?;
                let worker_handle = worker.handle();
                (Some(worker), worker_handle)
            };
            let telemetry = worker_handle.new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_wasm_executor(&config.executor);

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            &config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);
    let mut bridge_peer_storage_marker = None;

    if let Some(first_pk_raw) = keystore_container
        .keystore()
        .keys(eth_bridge::KEY_TYPE)
        .unwrap_or_default()
        .first()
        .cloned()
    {
        let pk = eth_bridge::offchain::crypto::Public::from_slice(&first_pk_raw[..])
            .expect("should have correct size");
        let sub_public = sp_core::ecdsa::Public::from(pk.clone());
        let public = secp256k1::PublicKey::parse_compressed(&sub_public.0).unwrap();
        let address = common::eth::public_key_to_eth_address(&public);
        let account = sp_runtime::MultiSigner::Ecdsa(sub_public.clone()).into_account();
        log::warn!(
            "Peer info: address: {:?}, account: {:?}, {}, public: {:?}",
            address,
            account,
            account,
            sub_public
        );
        bridge_peer_storage_marker = Some(first_pk_raw);
    } else {
        log::debug!("Ethereum bridge peer key not found.")
    }

    if let Some(marker) = bridge_peer_storage_marker {
        let mut storage = backend.offchain_storage().unwrap();
        // Keep a non-secret OCW activation marker in offchain DB for bridge workers.
        storage.set(STORAGE_PREFIX, STORAGE_PEER_MARKER_KEY, &marker.encode());
        // Legacy compatibility key for nodes/runtimes still reading the old storage path.
        storage.set(STORAGE_PREFIX, STORAGE_PEER_SECRET_KEY, &marker.encode());

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
            .rpc
            .addr
            .as_ref()
            .and_then(|endpoints| endpoints.first().map(|endpoint| endpoint.listen_addr))
            .unwrap_or_else(|| std::net::SocketAddr::from(([127, 0, 0, 1], config.rpc.port)));
        storage.set(
            STORAGE_PREFIX,
            STORAGE_SUB_NODE_URL_KEY,
            &format!("http://{}", rpc_addr).encode(),
        );

        config
            .prometheus_registry()
            .and_then(|registry| {
                crate::eth_bridge_metrics::Metrics::register(
                    registry,
                    backend.clone(),
                    std::time::Duration::from_secs(6),
                )
                .map_err(|e| {
                    log::error!("Failed to register metrics: {:?}", e);
                })
                .ok()
            })
            .and_then(|metrics| {
                task_manager.spawn_essential_handle().spawn_blocking(
                    "eth-bridge-metrics",
                    Some("eth-bridge-metrics"),
                    metrics.run(),
                );
                Some(())
            });

        log::info!("Ethereum bridge peer initialized");
    }
    config
        .prometheus_registry()
        .and_then(|registry| {
            crate::data_feed_metrics::Metrics::register(
                Arc::new(registry.clone()),
                client.clone(),
                Duration::from_secs(6),
            )
            .map_err(|e| {
                log::error!("Failed to register metrics: {:?}", e);
            })
            .ok()
        })
        .and_then(|metrics| {
            task_manager.spawn_essential_handle().spawn_blocking(
                "data-feed-metrics",
                Some("data-feed-metrics"),
                metrics.run(),
            );
            Some(())
        });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = Arc::from(
        sc_transaction_pool::Builder::new(
            task_manager.spawn_essential_handle(),
            client.clone(),
            config.role.is_authority().into(),
        )
        .with_options(config.transaction_pool.clone())
        .with_prometheus(config.prometheus_registry())
        .build(),
    );

    let telemetry = telemetry.map(|(worker, telemetry)| {
        if let Some(worker) = worker {
            task_manager
                .spawn_handle()
                .spawn("telemetry", Some("telemetry"), worker.run());
        }
        telemetry
    });

    let grandpa_hard_forks = Vec::new();

    let (grandpa_block_import, grandpa_link) =
        sc_finality_grandpa::block_import_with_authority_set_hard_forks(
            client.clone(),
            GRANDPA_JUSTIFICATION_PERIOD,
            &(client.clone() as Arc<_>),
            select_chain.clone(),
            grandpa_hard_forks,
            telemetry.as_ref().map(|x| x.handle()),
        )?;

    let (beefy_block_import, beefy_voter_links, beefy_rpc_links) =
        beefy_gadget::beefy_block_import_and_links(
            grandpa_block_import.clone(),
            backend.clone(),
            client.clone(),
            config.prometheus_registry().cloned(),
        );

    let babe_config = sc_consensus_babe::configuration(&*client)?;
    let slot_duration = babe_config.slot_duration();
    let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
        babe_config,
        beefy_block_import,
        client.clone(),
        Arc::new(move |_, _| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
            let slot =
                sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                    *timestamp,
                    slot_duration,
                );
            Ok((slot, timestamp))
        }) as BabeCreateInherentDataProviders<Block>,
        select_chain.clone(),
        OffchainTransactionPoolFactory::new(transaction_pool.clone()),
    )?;

    let (import_queue, _babe_worker_handle) =
        sc_consensus_babe::import_queue(sc_consensus_babe::ImportQueueParams {
            link: babe_link.clone(),
            block_import: babe_block_import.clone(),
            justification_import: Some(Box::new(grandpa_block_import)),
            client: client.clone(),
            slot_duration,
            spawner: &task_manager.spawn_essential_handle(),
            registry: config.prometheus_registry(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        })?;

    let import_setup = (
        babe_block_import.clone(),
        grandpa_link,
        babe_link.clone(),
        beefy_voter_links,
    );
    let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();
    let rpc_setup = shared_voter_state.clone();

    let rpc_extensions_builder = {
        let client = client.clone();
        let pool = transaction_pool.clone();
        let backend = backend.clone();

        move |subscription_executor| -> Result<crate::rpc::RpcExtension, sc_service::Error> {
            let deps = crate::rpc::FullDeps {
                client: client.clone(),
                pool: pool.clone(),
                backend: backend.clone(),
                deny_unsafe: sc_rpc::DenyUnsafe::No,
                beefy: crate::rpc::BeefyDeps {
                    beefy_finality_proof_stream: beefy_rpc_links.from_voter_justif_stream.clone(),
                    beefy_best_block_stream: beefy_rpc_links.from_voter_best_beefy_stream.clone(),
                    subscription_executor,
                },
            };

            let rpc = crate::rpc::create_full(deps)?;

            #[cfg(feature = "wip")]
            let rpc = crate::rpc::add_wip_rpc(rpc, client.clone())?;

            #[cfg(feature = "stage")]
            let rpc = crate::rpc::add_stage_rpc(rpc)?;

            Ok(rpc)
        }
    };

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (rpc_extensions_builder, import_setup, rpc_setup, telemetry),
    })
}

/// Create a new full node of arbitrary runtime and executor.
///
/// This is an advanced feature and not recommended for general use. Generally, `build_full` is
/// a better choice.
pub fn new_full(
    mut config: Configuration,
    disable_beefy: bool,
    telemetry_worker_handle: Option<TelemetryWorkerHandle>,
) -> Result<TaskManager, ServiceError> {
    // Increase the default value by 2 to make wasm being able to use 128MB, each heap page is 64KiB
    config.executor.default_heap_pages = Some(1024 * 2);

    debug!("using: {:#?}", config);

    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (rpc_extensions_builder, import_setup, rpc_setup, mut telemetry),
    } = new_partial(&mut config, telemetry_worker_handle)?;

    let genesis_hash = client
        .block_hash(0)
        .ok()
        .flatten()
        .expect("Genesis block exists; qed");
    type FullNetwork = sc_network::Litep2pNetworkBackend;
    let metrics = <FullNetwork as NetworkBackend<Block, <Block as BlockT>::Hash>>::register_notification_metrics(
        config.prometheus_config.as_ref().map(|cfg| &cfg.registry),
    );
    let mut net_config = sc_network::config::FullNetworkConfiguration::<
        Block,
        <Block as BlockT>::Hash,
        FullNetwork,
    >::new(
        &config.network,
        config
            .prometheus_config
            .as_ref()
            .map(|cfg| cfg.registry.clone()),
    );
    let peer_store_handle = net_config.peer_store_handle();

    let grandpa_protocol_name =
        sc_finality_grandpa::protocol_standard_name(&genesis_hash, &config.chain_spec);
    let (grandpa_protocol_config, grandpa_notification_service) =
        sc_finality_grandpa::grandpa_peers_set_config::<Block, FullNetwork>(
            grandpa_protocol_name.clone(),
            metrics.clone(),
            Arc::clone(&peer_store_handle),
        );
    net_config.add_notification_protocol(grandpa_protocol_config);

    let beefy_gossip_proto_name =
        beefy_gadget::gossip_protocol_name(&genesis_hash, config.chain_spec.fork_id());
    let (beefy_on_demand_justifications_handler, beefy_req_resp_cfg) =
        beefy_gadget::communication::request_response::BeefyJustifsRequestHandler::new::<
            _,
            FullNetwork,
        >(
            &genesis_hash,
            config.chain_spec.fork_id(),
            client.clone(),
            config.prometheus_registry().cloned(),
        );
    let (beefy_notification_config, beefy_notification_service) =
        beefy_gadget::communication::beefy_peers_set_config::<Block, FullNetwork>(
            beefy_gossip_proto_name.clone(),
            metrics.clone(),
            Arc::clone(&peer_store_handle),
        );

    let beefy_notification_service = if disable_beefy {
        None
    } else {
        net_config.add_notification_protocol(beefy_notification_config);
        net_config.add_request_response_protocol(beefy_req_resp_cfg);
        Some(beefy_notification_service)
    };

    let warp_sync = Arc::new(sc_finality_grandpa::warp_proof::NetworkProvider::new(
        backend.clone(),
        import_setup.1.shared_authority_set().clone(),
        vec![],
    ));

    let (network, system_rpc_tx, tx_handler_controller, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_config: Some(WarpSyncConfig::WithProvider(warp_sync)),
            block_relay: None,
            metrics,
        })?;

    if config.offchain_worker.enabled {
        let offchain_workers =
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                is_validator: config.role.is_authority(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                enable_http_requests: true,
                custom_extensions: |_| vec![],
            })?;
        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-worker",
            offchain_workers.run(client.clone(), task_manager.spawn_handle()),
        );
    }

    let is_offchain_indexing_enabled = config.offchain_worker.indexing_enabled;
    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    let (block_import, link_half, babe_link, beefy_links) = import_setup;

    let _rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        network: network.clone(),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        task_manager: &mut task_manager,
        transaction_pool: transaction_pool.clone(),
        rpc_builder: Box::new(rpc_extensions_builder),
        backend: backend.clone(),
        system_rpc_tx,
        config,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        telemetry: telemetry.as_mut(),
        tracing_execute_block: None,
    })?;

    if role.is_authority() {
        let mut proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );
        // Increase default block size to be able to run runtime upgrade with larger runtime wasm
        proposer.set_default_block_size_limit(sc_basic_authorship::DEFAULT_BLOCK_SIZE_LIMIT * 4);

        let backoff_authoring_blocks =
            Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
        let slot_duration = babe_link.config().slot_duration();

        let babe_config = sc_consensus_babe::BabeParams {
            keystore: keystore_container.keystore(),
            client: client.clone(),
            select_chain,
            env: proposer,
            block_import,
            sync_oracle: sync_service.clone(),
            justification_sync_link: sync_service.clone(),
            force_authoring,
            babe_link,
            block_proposal_slot_portion: sc_consensus_babe::SlotProportion::new(2f32 / 3f32),
            max_block_proposal_slot_portion: None,
            backoff_authoring_blocks,
            create_inherent_data_providers: move |_parent, ()| {
                // let client_clone = client_clone.clone();
                // let overseer_handle = overseer_handle.clone();
                async move {
                    let time = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
                        sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                            *time,
                            slot_duration //slot_duration.slot_duration(),
                        );

                    Ok((slot, time))
                }
            },
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        };

        let babe = sc_consensus_babe::start_babe(babe_config)?;

        task_manager.spawn_essential_handle().spawn_blocking(
            "babe-proposer",
            Some("babe-proposer"),
            babe,
        );
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore = if role.is_authority() {
        Some(keystore_container.keystore())
    } else {
        None
    };

    if !disable_beefy {
        let justifications_protocol_name = beefy_on_demand_justifications_handler.protocol_name();
        let network_params = beefy_gadget::BeefyNetworkParams {
            network: Arc::new(network.clone()),
            sync: sync_service.clone(),
            notification_service: beefy_notification_service
                .expect("BEEFY notification service should exist when BEEFY is enabled"),
            gossip_protocol_name: beefy_gossip_proto_name,
            justifications_protocol_name,
            _phantom: core::marker::PhantomData::<Block>,
        };
        let payload_provider = sp_beefy::mmr::MmrRootProvider::new(client.clone());
        let beefy_params = beefy_gadget::BeefyParams {
            client: client.clone(),
            backend: backend.clone(),
            payload_provider,
            runtime: client.clone(),
            is_authority: role.is_authority(),
            key_store: keystore.clone(),
            network_params,
            min_block_delta: 8,
            prometheus_registry: prometheus_registry.clone(),
            links: beefy_links,
            on_demand_justifications_handler: beefy_on_demand_justifications_handler,
        };

        let gadget = beefy_gadget::start_beefy_gadget::<_, _, _, _, _, _, _, _>(beefy_params);

        task_manager
            .spawn_essential_handle() // FIXME: use `spawn_handle` in non-test case
            .spawn_blocking("beefy-gadget", Some("beefy-gadget"), gadget);

        if is_offchain_indexing_enabled {
            task_manager.spawn_handle().spawn_blocking(
                "mmr-gadget",
                None,
                mmr_gadget::MmrGadget::start(
                    client.clone(),
                    backend.clone(),
                    sp_mmr_primitives::INDEXING_PREFIX.to_vec(),
                ),
            );
        }
    }

    let grandpa_config = sc_finality_grandpa::Config {
        // FIXME #1578 make this available through chainspec
        protocol_name: grandpa_protocol_name,
        gossip_duration: Duration::from_millis(333),
        justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
        name: Some(name),
        observer_enabled: false,
        keystore,
        local_role: role,
        telemetry: telemetry.as_ref().map(|x| x.handle()),
    };

    if enable_grandpa {
        let shared_voter_state = rpc_setup;

        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_config = sc_finality_grandpa::GrandpaParams {
            config: grandpa_config,
            link: link_half,
            network: network.clone(),
            sync: sync_service.clone(),
            notification_service: grandpa_notification_service,
            voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry,
            shared_voter_state,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool),
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "sc_finality_grandpa-voter",
            Some("sc_finality_grandpa-voter"),
            sc_finality_grandpa::run_grandpa_voter(grandpa_config)?,
        );
    }

    Ok(task_manager)
}
