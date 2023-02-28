use std::io::Write;

use hex_literal::hex;
use remote_externalities::{Builder, Mode, OfflineConfig, OnlineConfig, SnapshotConfig, Transport};
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

const INCLUDED_PREFIXES: [[u8; 32]; 1] = [hex_literal::hex!(
    "26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da9"
)];

#[derive(Debug, Clone, clap::Parser)]
pub struct ForkOffCmd {
    /// Shared parameters of substrate cli.
    #[allow(missing_docs)]
    #[clap(flatten)]
    pub shared_params: sc_cli::SharedParams,
    #[clap(long)]
    url: String,
    #[clap(long)]
    snapshot: Option<String>,
    #[clap(long)]
    raw: bool,
}

impl ForkOffCmd {
    pub async fn run(&self, mut cfg: Configuration) -> Result<(), sc_cli::Error> {
        let transport: Transport = self.url.clone().into();
        let maybe_state_snapshot: Option<SnapshotConfig> = self.snapshot.clone().map(|s| s.into());
        let ext: remote_externalities::TestExternalities =
            Builder::<framenode_runtime::Block>::default()
                .mode(if let Some(state_snapshot) = maybe_state_snapshot {
                    Mode::OfflineOrElseOnline(
                        OfflineConfig {
                            state_snapshot: state_snapshot.clone(),
                        },
                        OnlineConfig {
                            transport,
                            state_snapshot: Some(state_snapshot),
                            ..Default::default()
                        },
                    )
                } else {
                    Mode::Online(OnlineConfig {
                        transport,
                        ..Default::default()
                    })
                })
                .build()
                .await
                .unwrap();
        let skipped_prefixes = SKIPPED_PALLETS
            .iter()
            .map(|p| {
                let prefix = sp_core::twox_128(p.as_bytes()).to_vec();
                prefix
            })
            .collect::<std::collections::BTreeSet<_>>();
        let included_prefixes = INCLUDED_PREFIXES
            .iter()
            .map(|x| x.to_vec())
            .collect::<std::collections::BTreeSet<_>>();
        let mut storage = cfg.chain_spec.as_storage_builder().build_storage()?;
        storage.top = storage
            .top
            .into_iter()
            .filter(|(k, _)| {
                if k.len() < 32 || skipped_prefixes.contains(&k[..16]) {
                    true
                } else {
                    false
                }
            })
            .collect();
        let kv = ext.as_backend().essence().pairs();
        for (k, v) in kv {
            if k.len() >= 32
                && (!skipped_prefixes.contains(&k[..16]) || included_prefixes.contains(&k[..32]))
            {
                storage.top.insert(k, v);
            } else {
                log::debug!("Skipped {}", to_hex(&k, false));
            }
        }
        // Delete System.LastRuntimeUpgrade to ensure that the on_runtime_upgrade event is triggered
        storage.top.remove(
            hex!("26aa394eea5630e07c48ae0c9558cef7f9cce9c888469bb1a0dceaa129672ef8").as_slice(),
        );
        // To prevent the validator set from changing mid-test, set Staking.ForceEra to ForceNone ('0x02')
        storage.top.insert(
            hex!("5f3e4907f716ac89b6347d15ececedcaf7dad0317324aecae8744b87fc95f2f3").to_vec(),
            vec![2],
        );
        cfg.chain_spec.set_storage(storage);
        let json = sc_service::chain_ops::build_spec(&*cfg.chain_spec, self.raw)?;
        if std::io::stdout().write_all(json.as_bytes()).is_err() {
            let _ = std::io::stderr().write_all(b"Error writing to stdout\n");
        }
        Ok(())
    }
}

impl CliConfiguration for ForkOffCmd {
    fn shared_params(&self) -> &sc_cli::SharedParams {
        &self.shared_params
    }
}
