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

use std::io::Write;

use frame_remote_externalities::{
    Builder, Mode, OfflineConfig, OnlineConfig, RemoteExternalities, SnapshotConfig, Transport,
};
use sc_cli::CliConfiguration;
use sc_service::Configuration;
use sp_core::bytes::to_hex;

const SKIPPED_PALLETS: [&str; 7] = [
    "System",
    "Session",
    "Babe",
    "Grandpa",
    "GrandpaFinality",
    "Authorship",
    "Sudo",
];

const INCLUDED_PREFIXES: [(&str, &str); 1] = [("System", "Accounts")];

#[derive(Debug, Clone, clap::Parser)]
pub struct ForkOffCmd {
    /// Shared parameters of substrate cli.
    #[allow(missing_docs)]
    #[clap(flatten)]
    pub shared_params: sc_cli::SharedParams,

    /// Sora node url
    #[clap(long)]
    url: String,

    /// Save snapshot files to reuse in future. Optional
    #[clap(long)]
    snapshot: Option<String>,

    /// Print chainspec in raw format
    #[clap(long)]
    raw: bool,
}

fn get_storage_prefix(pallet: &str, storage: &str) -> Vec<u8> {
    [
        sp_core::twox_128(pallet.as_bytes()),
        sp_core::twox_128(storage.as_bytes()),
    ]
    .concat()
}

fn get_pallet_prefix(pallet: &str) -> Vec<u8> {
    sp_core::twox_128(pallet.as_bytes()).to_vec()
}

impl ForkOffCmd {
    pub async fn run(&self, mut cfg: Configuration) -> Result<(), sc_cli::Error> {
        // let transport: Transport = self.url.clone().into();
        // let maybe_state_snapshot: Option<SnapshotConfig> = self.snapshot.clone().map(|s| s.into());
        // let ext: RemoteExternalities<framenode_runtime::Block> =
        //     Builder::<framenode_runtime::Block>::default()
        //         .mode(if let Some(state_snapshot) = maybe_state_snapshot {
        //             Mode::OfflineOrElseOnline(
        //                 OfflineConfig {
        //                     state_snapshot: state_snapshot.clone(),
        //                 },
        //                 OnlineConfig {
        //                     transport,
        //                     state_snapshot: Some(state_snapshot),
        //                     ..Default::default()
        //                 },
        //             )
        //         } else {
        //             Mode::Online(OnlineConfig {
        //                 transport,
        //                 ..Default::default()
        //             })
        //         })
        //         .build()
        //         .await
        //         .unwrap();
        // let skipped_prefixes = SKIPPED_PALLETS
        //     .iter()
        //     .cloned()
        //     .map(get_pallet_prefix)
        //     .collect::<std::collections::BTreeSet<_>>();
        // let included_prefixes = INCLUDED_PREFIXES
        //     .iter()
        //     .map(|(p, s)| get_storage_prefix(p, s))
        //     .collect::<std::collections::BTreeSet<_>>();
        // let mut storage = cfg.chain_spec.as_storage_builder().build_storage()?;
        // storage.top = storage
        //     .top
        //     .into_iter()
        //     .filter(|(k, _)| {
        //         if k.len() < 32 || skipped_prefixes.contains(&k[..16]) {
        //             true
        //         } else {
        //             false
        //         }
        //     })
        //     .collect();
        // let kv = ext.as_backend().essence().pairs();
        // for (k, v) in kv {
        //     if k.len() >= 32
        //         && (!skipped_prefixes.contains(&k[..16]) || included_prefixes.contains(&k[..32]))
        //     {
        //         storage.top.insert(k, v);
        //     } else {
        //         log::debug!("Skipped {}", to_hex(&k, false));
        //     }
        // }
        // // Delete System.LastRuntimeUpgrade to ensure that the on_runtime_upgrade event is triggered
        // storage
        //     .top
        //     .remove(&get_storage_prefix("System", "LastRuntimeUpgrade"));
        // // To prevent the validator set from changing mid-test, set Staking.ForceEra to ForceNone ('0x02')
        // storage
        //     .top
        //     .insert(get_storage_prefix("Staking", "ForceEra"), vec![2]);
        // cfg.chain_spec.set_storage(storage);
        // let json = sc_service::chain_ops::build_spec(&*cfg.chain_spec, self.raw)?;
        // if std::io::stdout().write_all(json.as_bytes()).is_err() {
        //     let _ = std::io::stderr().write_all(b"Error writing to stdout\n");
        // }
        Ok(())
    }
}

impl CliConfiguration for ForkOffCmd {
    fn shared_params(&self) -> &sc_cli::SharedParams {
        &self.shared_params
    }
}
