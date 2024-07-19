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

use crate::cli::{Cli, Subcommand};
use crate::service;
use sc_cli::{ChainSpec, RuntimeVersion, SubstrateCli};
use sc_executor::sp_wasm_interface::ExtendedHostFunctions;
use sc_executor::NativeExecutionDispatch;
use sc_service::PartialComponents;

type HostFunctionsOf<E> = ExtendedHostFunctions<
    sp_io::SubstrateHostFunctions,
    <E as NativeExecutionDispatch>::ExtendHostFunctions,
>;

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
            use frame_benchmarking_cli::BenchmarkCmd;
            use sc_chain_spec::ChainType;
            use sc_service::Error;
            let runner = cli.create_runner(cmd)?;
            set_default_ss58_version();
            let chain_spec = &runner.config().chain_spec;

            match cmd {
                BenchmarkCmd::Storage(cmd) => runner.sync_run(|mut config| {
                    let PartialComponents {
                        client, backend, ..
                    } = service::new_partial(&mut config, None)?;
                    let db = backend.expose_db();
                    let storage = backend.expose_storage();
                    cmd.run(config, client, db, storage)
                }),
                BenchmarkCmd::Block(cmd) => runner.sync_run(|mut config| {
                    let PartialComponents { client, .. } = service::new_partial(&mut config, None)?;

                    cmd.run(client)
                }),
                BenchmarkCmd::Pallet(cmd) => {
                    if !matches!(chain_spec.chain_type(), ChainType::Development) {
                        return Err(Error::Other("Available only for dev chain".into()).into());
                    }

                    runner.sync_run(|config| {
                        cmd.run::<framenode_runtime::Block, HostFunctionsOf<service::ExecutorDispatch>>(config)
                    })
                }
                #[allow(unreachable_patterns)]
                _ => Err(Error::Other("Command not implemented".into()).into()),
            }
        }
        #[cfg(feature = "try-runtime")]
        Some(Subcommand::TryRuntime(cmd)) => {
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

            runner.async_run(|_config| {
                Ok((
                    cmd.try_run::<framenode_runtime::Block, HostFunctionsOf<service::ExecutorDispatch>>(),
                    task_manager,
                ))
            })
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
                return service::new_full(config, cli.disable_beefy, None)
                    .map_err(sc_cli::Error::Service);
                // Disable BEEFY on production.
                // BEEFY is still work in progress and probably will contain breaking changes, so it's better to enable it when it's ready
                // Also before enabling it we need to ensure that validators updated their session keys
                #[cfg(not(feature = "wip"))] // Bridges
                service::new_full(config, true, None).map_err(sc_cli::Error::Service)
            })
        }
    }
}
