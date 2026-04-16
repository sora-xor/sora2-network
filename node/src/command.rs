// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(feature = "runtime-benchmarks")]
use crate::benchmarking::{inherent_benchmark_data, AssetTransferBuilder, RemarkBuilder};
use crate::cli::{Cli, Subcommand};
use crate::service;
#[cfg(feature = "runtime-benchmarks")]
use frame_benchmarking_cli::{BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE};
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;
#[cfg(feature = "runtime-benchmarks")]
use sp_keyring::Sr25519Keyring;

fn set_default_ss58_version() {
    sp_core::crypto::set_default_ss58_version(sp_core::crypto::Ss58AddressFormat::from(
        framenode_runtime::SS58Prefix::get() as u16,
    ));
}

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "SORA".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        env!("CARGO_PKG_DESCRIPTION").into()
    }

    fn author() -> String {
        env!("CARGO_PKG_AUTHORS").into()
    }

    fn support_url() -> String {
        "https://github.com/sora-xor/sora2-network/issues/new".into()
    }

    fn copyright_start_year() -> i32 {
        2017
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
        #[cfg(feature = "private-net")]
        let chain_spec = match id {
            "" | "local" => Some(framenode_chain_spec::local_testnet_config(3, 3)),
            "docker-local" => Some(framenode_chain_spec::local_testnet_config(1, 1)),
            // dev doesn't use json chain spec to make development easier
            // "dev" => framenode_chain_spec::dev_net(),
            // "dev-coded" => Ok(framenode_chain_spec::dev_net_coded()),
            "dev" => Some(framenode_chain_spec::dev_net_coded()),
            "predev" => Some(framenode_chain_spec::predev_net_coded()),
            "test" => Some(framenode_chain_spec::test_net()?),
            "test-coded" => Some(framenode_chain_spec::staging_net_coded(true)),
            "staging" => Some(framenode_chain_spec::staging_net()?),
            "staging-coded" => Some(framenode_chain_spec::staging_net_coded(false)),
            "bridge-staging" => Some(framenode_chain_spec::bridge_staging_net()?),
            "bridge-staging-coded" => Some(framenode_chain_spec::bridge_staging_net_coded()),
            "bridge-dev" => Some(framenode_chain_spec::bridge_dev_net_coded()),
            _ => None,
        };

        #[cfg(not(feature = "private-net"))]
        let mut chain_spec = None;

        #[cfg(not(feature = "private-net"))]
        if id == "main" {
            chain_spec = Some(framenode_chain_spec::main_net()?);
        }

        #[cfg(feature = "main-net-coded")]
        if id == "main-coded" {
            chain_spec = Some(framenode_chain_spec::main_net_coded());
        }

        let chain_spec = if let Some(chain_spec) = chain_spec {
            chain_spec
        } else {
            framenode_chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(id))?
        };

        Ok(Box::new(chain_spec))
    }
}

#[cfg(all(test, feature = "private-net"))]
mod tests {
    use super::Cli;
    use clap::Parser;
    use sc_cli::SubstrateCli;

    #[test]
    fn private_chain_specs_load_with_default_node_features() {
        let cli = Cli::parse_from(["framenode"]);

        <Cli as SubstrateCli>::load_spec(&cli, "local")
            .expect("local chainspec should load with embedded wasm");
        <Cli as SubstrateCli>::load_spec(&cli, "dev")
            .expect("dev chainspec should load with embedded wasm");
    }
}

#[cfg(all(test, not(feature = "private-net")))]
mod default_chain_spec_tests {
    use super::Cli;
    use clap::Parser;
    use sc_cli::SubstrateCli;

    #[test]
    fn main_chain_spec_loads_with_default_node_features() {
        let cli = Cli::parse_from(["framenode"]);

        <Cli as SubstrateCli>::load_spec(&cli, "main")
            .expect("main chainspec should load with embedded json");
    }
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::from_args();

    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.async_run(|mut config| {
                let PartialComponents {
                    client,
                    task_manager,
                    import_queue,
                    ..
                } = service::new_partial(&mut config, None)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.async_run(|mut config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = service::new_partial(&mut config, None)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.async_run(|mut config| {
                let PartialComponents {
                    client,
                    task_manager,
                    ..
                } = service::new_partial(&mut config, None)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.async_run(|mut config| {
                let PartialComponents {
                    client,
                    task_manager,
                    import_queue,
                    ..
                } = service::new_partial(&mut config, None)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.sync_run(|config| cmd.run(config.database))
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            runner.async_run(|mut config| {
                let PartialComponents {
                    client,
                    task_manager,
                    backend,
                    ..
                } = service::new_partial(&mut config, None)?;
                Ok((cmd.run(client, backend, None), task_manager))
            })
        }
        #[cfg(feature = "runtime-benchmarks")]
        Some(Subcommand::Benchmark(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();

            match cmd {
                BenchmarkCmd::Storage(cmd) => runner.sync_run(|mut config| {
                    let partial = service::new_partial(&mut config, None)?;
                    let db = partial.backend.expose_db();
                    let storage = partial.backend.expose_storage();
                    let shared_trie_cache = partial.backend.expose_shared_trie_cache();
                    cmd.run(config, partial.client, db, storage, shared_trie_cache)
                }),
                BenchmarkCmd::Block(cmd) => runner.sync_run(|mut config| {
                    let partial = service::new_partial(&mut config, None)?;
                    cmd.run(partial.client)
                }),
                BenchmarkCmd::Pallet(cmd) => runner.sync_run(|config| {
                    cmd.run_with_spec::<sp_runtime::traits::HashingFor<framenode_runtime::Block>, ()>(Some(config.chain_spec))
                }),
                BenchmarkCmd::Overhead(cmd) => runner.sync_run(|mut config| {
                    let partial = service::new_partial(&mut config, None)?;
                    let ext_builder = RemarkBuilder::new(partial.client.clone());
                    cmd.run(
                        config.chain_spec.name().into(),
                        partial.client,
                        inherent_benchmark_data()?,
                        Vec::new(),
                        &ext_builder,
                        false,
                    )
                }),
                BenchmarkCmd::Extrinsic(cmd) => runner.sync_run(|mut config| {
                    let partial = service::new_partial(&mut config, None)?;
                    let ext_factory = ExtrinsicFactory(vec![
                        Box::new(RemarkBuilder::new(partial.client.clone())),
                        Box::new(AssetTransferBuilder::new(
                            partial.client.clone(),
                            Sr25519Keyring::Alice.to_account_id(),
                            core::cmp::max(framenode_runtime::ExistentialDeposit::get(), 1_u128),
                        )),
                    ]);

                    cmd.run(
                        partial.client,
                        inherent_benchmark_data()?,
                        Vec::new(),
                        &ext_factory,
                    )
                }),
                BenchmarkCmd::Machine(cmd) =>
                    runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone())),
            }
        }
        #[cfg(feature = "private-net")]
        Some(Subcommand::ForkOff(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();

            use sc_service::TaskManager;
            let registry = &runner
                .config()
                .prometheus_config
                .as_ref()
                .map(|cfg| &cfg.registry);
            let task_manager = TaskManager::new(runner.config().tokio_handle.clone(), *registry)
                .map_err(|e| sc_cli::Error::Service(sc_service::Error::Prometheus(e)))?;

            runner.async_run(|config| Ok((cmd.run(config), task_manager)))
        }
        None => {
            let runner = cli.create_runner(&cli.run)?;
            set_default_ss58_version();
            runner.run_node_until_exit(|config| async move {
                #[cfg(feature = "wip")] // Bridges
                return match config.network.network_backend {
                    sc_network::config::NetworkBackendType::Libp2p => {
                        service::new_full::<sc_network::NetworkWorker<_, _>>(
                            config,
                            cli.disable_beefy,
                            None,
                        )
                        .map_err(sc_cli::Error::Service)
                    }
                    sc_network::config::NetworkBackendType::Litep2p => {
                        service::new_full::<sc_network::Litep2pNetworkBackend>(
                            config,
                            cli.disable_beefy,
                            None,
                        )
                        .map_err(sc_cli::Error::Service)
                    }
                };
                // Disable BEEFY on production.
                // BEEFY is still work in progress and probably will contain breaking changes, so it's better to enable it when it's ready
                // Also before enabling it we need to ensure that validators updated their session keys
                #[cfg(not(feature = "wip"))] // Bridges
                match config.network.network_backend {
                    sc_network::config::NetworkBackendType::Libp2p => {
                        service::new_full::<sc_network::NetworkWorker<_, _>>(config, true, None)
                            .map_err(sc_cli::Error::Service)
                    }
                    sc_network::config::NetworkBackendType::Litep2p => {
                        service::new_full::<sc_network::Litep2pNetworkBackend>(config, true, None)
                            .map_err(sc_cli::Error::Service)
                    }
                }
            })
        }
    }
}
