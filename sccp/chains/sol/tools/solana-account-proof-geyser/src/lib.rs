pub mod config;
pub mod types;
pub mod utils;

use std::collections::HashMap;
use std::io;
use std::str::FromStr;
use std::thread;

use borsh::to_vec;
use crossbeam_channel::{unbounded, Sender};
use log::error;
use agave_geyser_plugin_interface::geyser_plugin_interface::{
    GeyserPlugin, GeyserPluginError, ReplicaAccountInfoVersions, ReplicaBlockInfoVersions,
    ReplicaEntryInfoVersions, ReplicaTransactionInfoVersions, Result as PluginResult, SlotStatus,
};
use solana_sdk::clock::Slot;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

use crate::config::Config;
use crate::types::{
    AccountHashAccumulator, AccountInfo, BlockInfo, GeyserMessage, SlotInfo, TransactionInfo,
    TransactionSigAccumulator, Update,
};
use crate::utils::{
    assemble_account_delta_inclusion_proof, calculate_root_and_proofs, compute_bank_hash,
    hash_solana_account,
};

pub const SLOT_HASH_ACCOUNT: &str = "SysvarS1otHashes111111111111111111111111111";

fn handle_confirmed_slot(
    slot: u64,
    block_accumulator: &mut HashMap<u64, BlockInfo>,
    processed_slot_account_accumulator: &mut AccountHashAccumulator,
    processed_transaction_accumulator: &mut TransactionSigAccumulator,
    monitored_pubkeys: &[Pubkey],
) -> anyhow::Result<Update> {
    let Some(block) = block_accumulator.remove(&slot) else {
        anyhow::bail!("block metadata not available for slot {slot}");
    };
    let Some(num_sigs) = processed_transaction_accumulator.remove(&slot) else {
        anyhow::bail!("transaction signature count not available for slot {slot}");
    };
    let Some(account_hashes_data) = processed_slot_account_accumulator.remove(&slot) else {
        anyhow::bail!("account hashes not available for slot {slot}");
    };

    let proof_pubkeys: Vec<Pubkey> = monitored_pubkeys
        .iter()
        .copied()
        .filter(|pubkey| account_hashes_data.contains_key(pubkey))
        .collect();
    if proof_pubkeys.is_empty() {
        anyhow::bail!("no monitored accounts were modified in slot {slot}");
    }

    let mut account_hashes: Vec<(Pubkey, Hash)> = account_hashes_data
        .iter()
        .map(|(pubkey, (_, hash, _))| (*pubkey, *hash))
        .collect();
    let (account_delta_root, account_proofs) =
        calculate_root_and_proofs(&mut account_hashes, &proof_pubkeys);
    let proofs = assemble_account_delta_inclusion_proof(
        &account_hashes_data,
        &account_proofs,
        &proof_pubkeys,
    )?;

    let parent_bankhash =
        Hash::from_str(&block.parent_bankhash).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    let blockhash =
        Hash::from_str(&block.blockhash).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    let root = compute_bank_hash(parent_bankhash, account_delta_root, num_sigs, blockhash);

    Ok(Update {
        slot,
        root,
        proof: crate::types::BankHashProof {
            proofs,
            num_sigs,
            parent_slot: block.parent_slot,
            account_delta_root,
            parent_bankhash,
            blockhash,
        },
    })
}

fn custom_error(message: &'static str) -> GeyserPluginError {
    GeyserPluginError::Custom(Box::new(io::Error::new(io::ErrorKind::Other, message)))
}

fn handle_processed_slot(
    slot: u64,
    raw_slot_account_accumulator: &mut AccountHashAccumulator,
    processed_slot_account_accumulator: &mut AccountHashAccumulator,
    raw_transaction_accumulator: &mut TransactionSigAccumulator,
    processed_transaction_accumulator: &mut TransactionSigAccumulator,
) {
    transfer_slot(
        slot,
        raw_slot_account_accumulator,
        processed_slot_account_accumulator,
    );
    transfer_slot(
        slot,
        raw_transaction_accumulator,
        processed_transaction_accumulator,
    );
}

fn transfer_slot<V>(slot: u64, raw: &mut HashMap<u64, V>, processed: &mut HashMap<u64, V>) {
    if let Some(entry) = raw.remove(&slot) {
        processed.insert(slot, entry);
    }
}

fn process_messages(
    geyser_receiver: crossbeam_channel::Receiver<GeyserMessage>,
    tx: broadcast::Sender<Update>,
    monitored_pubkeys: Vec<Pubkey>,
) {
    let mut raw_slot_account_accumulator: AccountHashAccumulator = HashMap::new();
    let mut processed_slot_account_accumulator: AccountHashAccumulator = HashMap::new();
    let mut raw_transaction_accumulator: TransactionSigAccumulator = HashMap::new();
    let mut processed_transaction_accumulator: TransactionSigAccumulator = HashMap::new();
    let mut block_accumulator: HashMap<u64, BlockInfo> = HashMap::new();

    loop {
        match geyser_receiver.recv() {
            Ok(GeyserMessage::AccountMessage(account)) => {
                let account_hash = hash_solana_account(
                    account.lamports,
                    account.owner.as_ref(),
                    account.executable,
                    account.rent_epoch,
                    &account.data,
                    account.pubkey.as_ref(),
                );
                let slot_entry = raw_slot_account_accumulator
                    .entry(account.slot)
                    .or_insert_with(HashMap::new);
                let account_entry = slot_entry
                    .entry(account.pubkey)
                    .or_insert_with(|| (0, Hash::default(), AccountInfo::default()));
                if account.write_version > account_entry.0 {
                    *account_entry = (account.write_version, Hash::from(account_hash), account);
                }
            }
            Ok(GeyserMessage::TransactionMessage(txn)) => {
                *raw_transaction_accumulator.entry(txn.slot).or_insert(0) += txn.num_sigs;
            }
            Ok(GeyserMessage::BlockMessage(block)) => {
                block_accumulator.insert(block.slot, block);
            }
            Ok(GeyserMessage::SlotMessage(slot_info)) => match slot_info.status {
                SlotStatus::Processed => handle_processed_slot(
                    slot_info.slot,
                    &mut raw_slot_account_accumulator,
                    &mut processed_slot_account_accumulator,
                    &mut raw_transaction_accumulator,
                    &mut processed_transaction_accumulator,
                ),
                SlotStatus::Confirmed => match handle_confirmed_slot(
                    slot_info.slot,
                    &mut block_accumulator,
                    &mut processed_slot_account_accumulator,
                    &mut processed_transaction_accumulator,
                    &monitored_pubkeys,
                ) {
                    Ok(update) => {
                        if let Err(err) = tx.send(update) {
                            error!(
                                "failed to publish confirmed-slot witness {}: {:?}",
                                slot_info.slot, err
                            );
                        }
                    }
                    Err(err) => {
                        error!(
                            "failed to build confirmed-slot witness {}: {:?}",
                            slot_info.slot, err
                        );
                    }
                },
                _ => {}
            },
            Err(err) => {
                error!("error receiving geyser message: {:?}", err);
            }
        }
    }
}

#[derive(Debug)]
pub struct PluginInner {
    geyser_sender: Sender<GeyserMessage>,
}

impl PluginInner {
    fn send_message(&self, message: GeyserMessage) {
        if let Err(err) = self.geyser_sender.send(message) {
            error!("failed to send geyser message: {:?}", err);
        }
    }
}

#[derive(Debug, Default)]
pub struct Plugin {
    inner: Option<PluginInner>,
}

impl Plugin {
    fn with_inner<F>(&self, f: F) -> PluginResult<()>
    where
        F: FnOnce(&PluginInner) -> PluginResult<()>,
    {
        let inner = self.inner.as_ref().expect("plugin initialized");
        f(inner)
    }
}

impl GeyserPlugin for Plugin {
    fn name(&self) -> &'static str {
        "SolanaAccountProofGeyser"
    }

    fn on_load(&mut self, config_file: &str, _is_reload: bool) -> PluginResult<()> {
        let config = Config::load_from_file(config_file).map_err(|err| {
            GeyserPluginError::ConfigFileReadError {
                msg: err.to_string(),
            }
        })?;
        solana_logger::setup_with_default("error");

        let (geyser_sender, geyser_receiver) = unbounded();
        let mut monitored_pubkeys: Vec<Pubkey> = config
            .account_list
            .iter()
            .map(|value| {
                Pubkey::from_str(value).map_err(|err| GeyserPluginError::Custom(Box::new(err)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let slot_hash_pubkey =
            Pubkey::from_str(SLOT_HASH_ACCOUNT).expect("slot hashes sysvar constant is valid");
        if !monitored_pubkeys.contains(&slot_hash_pubkey) {
            monitored_pubkeys.push(slot_hash_pubkey);
        }

        let (tx, _rx) = broadcast::channel(64);
        let tx_process_messages = tx.clone();
        thread::spawn(move || {
            process_messages(geyser_receiver, tx_process_messages, monitored_pubkeys)
        });

        thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
            runtime.block_on(async move {
                let listener = TcpListener::bind(&config.bind_address)
                    .await
                    .expect("bind geyser TCP listener");
                loop {
                    let (mut socket, _) = match listener.accept().await {
                        Ok(connection) => connection,
                        Err(err) => {
                            error!("failed to accept witness client connection: {:?}", err);
                            continue;
                        }
                    };
                    let mut rx = tx.subscribe();
                    tokio::spawn(async move {
                        loop {
                            match rx.recv().await {
                                Ok(update) => match to_vec(&update) {
                                    Ok(data) => {
                                        if let Err(err) = socket.write_all(&data).await {
                                            error!(
                                                "failed to write witness update to client: {:?}",
                                                err
                                            );
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        error!("failed to borsh-encode witness update: {:?}", err);
                                    }
                                },
                                Err(err) => {
                                    error!("broadcast receive error: {:?}", err);
                                    break;
                                }
                            }
                        }
                    });
                }
            });
        });

        self.inner = Some(PluginInner { geyser_sender });
        Ok(())
    }

    fn on_unload(&mut self) {
        if let Some(inner) = self.inner.take() {
            drop(inner.geyser_sender);
        }
    }

    fn update_account(
        &self,
        account: ReplicaAccountInfoVersions,
        slot: Slot,
        _is_startup: bool,
    ) -> PluginResult<()> {
        self.with_inner(|inner| {
            let account = match account {
                ReplicaAccountInfoVersions::V0_0_3(account) => account,
                _ => {
                    return Err(custom_error(
                        "unsupported ReplicaAccountInfoVersions variant",
                    ))
                }
            };

            let pubkey = Pubkey::try_from(account.pubkey)
                .map_err(|err| GeyserPluginError::Custom(Box::new(err)))?;
            let owner = Pubkey::try_from(account.owner)
                .map_err(|err| GeyserPluginError::Custom(Box::new(err)))?;

            inner.send_message(GeyserMessage::AccountMessage(AccountInfo {
                pubkey,
                lamports: account.lamports,
                owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                data: account.data.to_vec(),
                write_version: account.write_version,
                slot,
            }));
            Ok(())
        })
    }

    fn notify_end_of_startup(&self) -> PluginResult<()> {
        Ok(())
    }

    fn update_slot_status(
        &self,
        slot: Slot,
        _parent: Option<u64>,
        status: &SlotStatus,
    ) -> PluginResult<()> {
        self.with_inner(|inner| {
            inner.send_message(GeyserMessage::SlotMessage(SlotInfo {
                slot,
                status: status.clone(),
            }));
            Ok(())
        })
    }

    fn notify_transaction(
        &self,
        transaction: ReplicaTransactionInfoVersions<'_>,
        slot: Slot,
    ) -> PluginResult<()> {
        self.with_inner(|inner| {
            let num_sigs = match transaction {
                ReplicaTransactionInfoVersions::V0_0_2(transaction) => {
                    transaction.transaction.signatures().len() as u64
                }
                ReplicaTransactionInfoVersions::V0_0_3(transaction) => {
                    transaction.transaction.signatures.len() as u64
                }
                ReplicaTransactionInfoVersions::V0_0_1(transaction) => {
                    transaction.transaction.signatures().len() as u64
                }
            };
            inner.send_message(GeyserMessage::TransactionMessage(TransactionInfo {
                slot,
                num_sigs,
            }));
            Ok(())
        })
    }

    fn notify_entry(&self, _entry: ReplicaEntryInfoVersions<'_>) -> PluginResult<()> {
        Ok(())
    }

    fn notify_block_metadata(&self, blockinfo: ReplicaBlockInfoVersions<'_>) -> PluginResult<()> {
        self.with_inner(|inner| {
            let blockinfo = match blockinfo {
                ReplicaBlockInfoVersions::V0_0_1(info) => BlockInfo {
                    slot: info.slot,
                    parent_slot: 0,
                    parent_bankhash: String::default(),
                    blockhash: info.blockhash.to_string(),
                },
                ReplicaBlockInfoVersions::V0_0_2(info) => info.into(),
                ReplicaBlockInfoVersions::V0_0_3(info) => info.into(),
                ReplicaBlockInfoVersions::V0_0_4(info) => info.into(),
            };
            inner.send_message(GeyserMessage::BlockMessage(blockinfo));
            Ok(())
        })
    }

    fn account_data_notifications_enabled(&self) -> bool {
        true
    }

    fn account_data_snapshot_notifications_enabled(&self) -> bool {
        false
    }

    fn transaction_notifications_enabled(&self) -> bool {
        true
    }

    fn entry_notifications_enabled(&self) -> bool {
        false
    }
}

#[no_mangle]
#[allow(improper_ctypes_definitions)]
pub unsafe extern "C" fn _create_plugin() -> *mut dyn GeyserPlugin {
    let plugin = Plugin::default();
    let plugin: Box<dyn GeyserPlugin> = Box::new(plugin);
    Box::into_raw(plugin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::hash_solana_account;

    #[test]
    fn handle_confirmed_slot_builds_bank_hash_proofs_for_monitored_accounts() {
        let slot = 19u64;
        let burn_record_pubkey = Pubkey::new_unique();
        let slot_hashes_pubkey = Pubkey::from_str(SLOT_HASH_ACCOUNT).unwrap();
        let burn_record_account = AccountInfo {
            pubkey: burn_record_pubkey,
            lamports: 10,
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
            data: vec![1, 2, 3, 4],
            write_version: 1,
            slot,
        };
        let slot_hashes_account = AccountInfo {
            pubkey: slot_hashes_pubkey,
            lamports: 10,
            owner: Pubkey::new_unique(),
            executable: false,
            rent_epoch: 0,
            data: vec![5, 6, 7, 8],
            write_version: 2,
            slot,
        };
        let burn_hash = Hash::from(hash_solana_account(
            burn_record_account.lamports,
            burn_record_account.owner.as_ref(),
            burn_record_account.executable,
            burn_record_account.rent_epoch,
            &burn_record_account.data,
            burn_record_account.pubkey.as_ref(),
        ));
        let slot_hash_hash = Hash::from(hash_solana_account(
            slot_hashes_account.lamports,
            slot_hashes_account.owner.as_ref(),
            slot_hashes_account.executable,
            slot_hashes_account.rent_epoch,
            &slot_hashes_account.data,
            slot_hashes_account.pubkey.as_ref(),
        ));

        let mut block_accumulator = HashMap::from([(
            slot,
            BlockInfo {
                slot,
                parent_slot: slot.saturating_sub(1),
                parent_bankhash: Hash::new_unique().to_string(),
                blockhash: Hash::new_unique().to_string(),
            },
        )]);
        let block = block_accumulator.get(&slot).unwrap().clone();
        let mut processed_slot_account_accumulator = HashMap::from([(
            slot,
            HashMap::from([
                (
                    burn_record_pubkey,
                    (burn_record_account.write_version, burn_hash, burn_record_account.clone()),
                ),
                (
                    slot_hashes_pubkey,
                    (
                        slot_hashes_account.write_version,
                        slot_hash_hash,
                        slot_hashes_account.clone(),
                    ),
                ),
            ]),
        )]);
        let mut processed_transaction_accumulator = HashMap::from([(slot, 7u64)]);
        let monitored_pubkeys = vec![burn_record_pubkey, slot_hashes_pubkey];

        let update = handle_confirmed_slot(
            slot,
            &mut block_accumulator,
            &mut processed_slot_account_accumulator,
            &mut processed_transaction_accumulator,
            &monitored_pubkeys,
        )
        .unwrap();

        let mut expected_hashes = vec![
            (burn_record_pubkey, burn_hash),
            (slot_hashes_pubkey, slot_hash_hash),
        ];
        let (expected_delta_root, _) =
            calculate_root_and_proofs(&mut expected_hashes, &monitored_pubkeys);
        let expected_root = compute_bank_hash(
            Hash::from_str(&block.parent_bankhash).unwrap(),
            expected_delta_root,
            7,
            Hash::from_str(&block.blockhash).unwrap(),
        );

        assert_eq!(update.slot, slot);
        assert_eq!(update.root, expected_root);
        assert_eq!(update.proof.account_delta_root, expected_delta_root);
        assert_eq!(update.proof.proofs.len(), 2);
        assert!(update
            .proof
            .proofs
            .iter()
            .any(|proof| proof.0 == burn_record_pubkey));
        assert!(update
            .proof
            .proofs
            .iter()
            .any(|proof| proof.0 == slot_hashes_pubkey));
        assert!(!block_accumulator.contains_key(&slot));
        assert!(!processed_slot_account_accumulator.contains_key(&slot));
        assert!(!processed_transaction_accumulator.contains_key(&slot));
    }
}
