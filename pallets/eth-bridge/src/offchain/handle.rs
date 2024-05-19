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

use crate::contract::ContractEvent;
use crate::offchain::SignedTransactionData;
use crate::requests::{
    IncomingMarkAsDoneRequest, IncomingMetaRequestKind, IncomingRequest,
    IncomingTransactionRequestKind, LoadIncomingMetaRequest, LoadIncomingRequest,
    LoadIncomingTransactionRequest, OffchainRequest, OutgoingRequest,
};
use crate::types::SubstrateBlockLimited;
use crate::{
    Call, Config, Error, Pallet, RequestStatuses, Requests, RequestsQueue, CONFIRMATION_INTERVAL,
    MAX_FAILED_SEND_SIGNED_TX_RETRIES, MAX_GET_LOGS_ITEMS, MAX_PENDING_TX_BLOCKS_PERIOD,
    MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE, RE_HANDLE_TXS_PERIOD,
    STORAGE_FAILED_PENDING_TRANSACTIONS_KEY, STORAGE_PEER_SECRET_KEY,
    STORAGE_PENDING_TRANSACTIONS_KEY, STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY,
    SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK, SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION,
};
use alloc::vec::Vec;
use bridge_multisig::MultiChainHeight;
use codec::{Decode, Encode};
use frame_support::sp_runtime::app_crypto::ecdsa;
use frame_support::sp_runtime::offchain::storage::StorageValueRef;
use frame_support::sp_runtime::traits::{One, Saturating, Zero};
use frame_support::traits::Get;
use frame_support::{ensure, fail};
use frame_system::offchain::{CreateSignedTransaction, Signer};
use frame_system::pallet_prelude::BlockNumberFor;
use log::{debug, error, info, trace, warn};
use sp_core::RuntimeDebug;
use sp_core::H256;
use sp_io::hashing::blake2_256;
use sp_std::collections::btree_map::BTreeMap;

impl<T: Config> Pallet<T> {
    /// Encodes the given outgoing request to Ethereum ABI, then signs the data by off-chain worker's
    /// key and sends the approve as a signed transaction.
    fn handle_outgoing_request(request: OutgoingRequest<T>, hash: H256) -> Result<(), Error<T>> {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            error!("No local account available");
            return Err(<Error<T>>::NoLocalAccountForSigning);
        }
        let encoded_request = request.to_eth_abi(hash)?;
        let secret_s = StorageValueRef::persistent(STORAGE_PEER_SECRET_KEY);
        let sk = secp256k1::SecretKey::parse_slice(
            &secret_s
                .get::<Vec<u8>>()
                .ok()
                .flatten()
                .expect("Off-chain worker secret key is not specified."),
        )
        .expect("Invalid off-chain worker secret key.");
        // Signs `abi.encodePacked(tokenAddress, amount, to, txHash, from)`.
        let (signature, public) = Self::sign_message(encoded_request.as_raw(), &sk);
        let call = Call::approve_request {
            ocw_public: ecdsa::Public::from_raw(public.serialize_compressed()),
            hash,
            signature_params: signature,
            network_id: request.network_id(),
        };
        let result = Self::send_signed_transaction(&signer, &call);

        match result {
            Some((account, res)) => {
                Self::add_pending_extrinsic(call, &account, res.is_ok());
                match res {
                    Ok(_) => trace!("Signed transaction sent"),
                    Err(e) => {
                        error!(
                            "[{:?}] Failed in handle_outgoing_transfer: {:?}",
                            account.id, e
                        );
                        return Err(<Error<T>>::FailedToSendSignedTransaction);
                    }
                }
            }
            _ => {
                error!("Failed in handle_outgoing_transfer");
                return Err(<Error<T>>::NoLocalAccountForSigning);
            }
        };
        Ok(())
    }

    /// Procedure for handling incoming requests.
    ///
    /// For an incoming request, a premise for its finalization will be block confirmation in PoW
    /// consensus. Since the confirmation is probabilistic, we need to choose a relatively large
    /// number of how many blocks should be mined after a corresponding transaction
    /// (`CONFIRMATION_INTERVAL`).
    ///
    /// An off-chain worker keeps track of already handled requests in local storage.
    fn handle_pending_incoming_requests(
        request: IncomingRequest<T>,
        hash: H256,
    ) -> Result<(), Error<T>> {
        let network_id = request.network_id();
        // TODO (optimization): send results as batch.
        // Load the transaction receipt again to check that it still exists.
        let exists = request.check_existence()?;
        ensure!(exists, Error::<T>::EthTransactionIsFailed);
        Self::send_finalize_incoming_request(hash, request.timepoint(), network_id)
    }

    /// Removes all extrinsics presented in the `block` from pending transactions storage. If some
    /// extrinsic weren't sent or finalized for a long time
    /// (`SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION`), it's re-sent.
    fn handle_substrate_block(
        block: SubstrateBlockLimited,
        current_height: BlockNumberFor<T>,
    ) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        let s_pending_txs = StorageValueRef::persistent(STORAGE_PENDING_TRANSACTIONS_KEY);
        if let Some(mut txs) = s_pending_txs
            .get::<BTreeMap<H256, SignedTransactionData<T>>>()
            .ok()
            .flatten()
        {
            debug!("Pending txs count: {}", txs.len());
            for ext in block.extrinsics {
                let vec = ext.encode();
                let hash = H256(blake2_256(&vec));
                // Transaction has been finalized, remove it from pending txs.
                txs.remove(&hash);
            }
            let signer = Self::get_signer()?;
            // Re-send all transactions that weren't sent or finalized.
            let mut resent_num = 0;
            let mut failed_txs_tmp = Vec::new();
            let txs = txs
                .into_iter()
                .filter_map(|(mut hash, mut tx)| {
                    let not_sent_or_too_long_finalization = tx
                        .submitted_at
                        .map(|submitted_height| {
                            current_height
                                > submitted_height
                                    + SUBSTRATE_MAX_BLOCK_NUM_EXPECTING_UNTIL_FINALIZATION.into()
                        })
                        .unwrap_or(true);
                    if not_sent_or_too_long_finalization {
                        let should_resend = resent_num < MAX_SUCCESSFUL_SENT_SIGNED_TX_PER_ONCE;
                        if should_resend {
                            if tx.resend(&signer) {
                                resent_num += 1;
                                // Update key = extrinsic hash.
                            } else {
                                let key = format!(
                                    "eth-bridge-ocw::pending-transactions-retries-v2-{:?}",
                                    tx.extrinsic_hash
                                );
                                let mut s_retries = StorageValueRef::persistent(key.as_bytes());
                                let mut retries: u16 =
                                    s_retries.get().ok().flatten().unwrap_or_default();
                                retries = retries.saturating_add(1);
                                // If re-send limit exceeded - remove.
                                if retries > MAX_FAILED_SEND_SIGNED_TX_RETRIES {
                                    failed_txs_tmp.push(tx);
                                    s_retries.clear();
                                    return None;
                                } else {
                                    s_retries.set(&retries);
                                }
                            }
                        }
                        hash = tx.extrinsic_hash;
                    }
                    Some((hash, tx))
                })
                .collect::<BTreeMap<H256, SignedTransactionData<T>>>();
            if !failed_txs_tmp.is_empty() {
                let s_failed_pending_txs =
                    StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY);
                let mut failed_txs = s_failed_pending_txs
                    .get::<BTreeMap<H256, SignedTransactionData<T>>>()
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                for tx in failed_txs_tmp {
                    failed_txs.insert(tx.extrinsic_hash, tx);
                }
                s_failed_pending_txs.set(&failed_txs);
            }
            s_pending_txs.set(&txs);
        }
        Ok(())
    }

    /// Handles a special case of an incoming request - marking as done.
    ///
    /// This special flow (unlike `parse_incoming_request`) only queries the contract's `used`
    /// variable to check if the request was actually made.
    fn handle_mark_as_done_incoming_request(
        pre_request: LoadIncomingMetaRequest<T>,
        pre_request_hash: H256,
    ) -> Result<(), Error<T>> {
        let network_id = pre_request.network_id;
        let at_height = Self::load_current_height(network_id)?;
        let timepoint = pre_request.timepoint;
        let request = IncomingRequest::MarkAsDone(IncomingMarkAsDoneRequest {
            outgoing_request_hash: pre_request_hash,
            initial_request_hash: pre_request.hash,
            author: pre_request.author,
            at_height,
            timepoint,
            network_id,
        });
        let is_used = request.check_existence()?;
        ensure!(is_used, Error::<T>::RequestNotFinalizedOnSidechain);
        Self::send_register_incoming_request(request, timepoint, network_id)
    }

    /// Handles the given off-chain request.
    ///
    /// The function delegates further handling depending on request type.
    /// There are 4 flows. 3 for incoming request: handle 'mark as done', handle 'cancel outgoing
    /// request' and for the rest, and only one for all outgoing requests.
    fn handle_offchain_request(request: OffchainRequest<T>) -> Result<(), Error<T>> {
        debug!("Handling request: {:?}", request.hash());
        match request {
            OffchainRequest::LoadIncoming(request) => {
                let network_id = request.network_id();
                let timepoint = request.timepoint();
                match request {
                    LoadIncomingRequest::Transaction(request) => {
                        let tx_hash = request.hash;
                        let kind = request.kind;
                        debug!("Loading approved tx {}", tx_hash);
                        let tx = Self::load_tx_receipt(tx_hash, network_id)?;
                        let mut incoming_request = Self::parse_incoming_request(tx, request)?;
                        // TODO: this flow was used to transfer XOR for free with the `request_from_sidechain`
                        // extrinsic. This could be used to spam network with transactions. Now it's unneeded,
                        // since all incoming transactions are loaded automatically. The extrinsic and related
                        // code should be considered for deletion.
                        if kind == IncomingTransactionRequestKind::TransferXOR {
                            if let IncomingRequest::Transfer(transfer) = &mut incoming_request {
                                ensure!(
                                    transfer.asset_id == common::XOR.into(),
                                    Error::<T>::ExpectedXORTransfer
                                );
                            } else {
                                fail!(Error::<T>::ExpectedXORTransfer)
                            }
                        }
                        Self::send_register_incoming_request(
                            incoming_request,
                            timepoint,
                            network_id,
                        )
                    }
                    LoadIncomingRequest::Meta(request, hash) => {
                        let kind = request.kind;
                        match kind {
                            IncomingMetaRequestKind::MarkAsDone => {
                                Self::handle_mark_as_done_incoming_request(request, hash)
                            }
                            IncomingMetaRequestKind::CancelOutgoingRequest => {
                                let tx_hash = request.hash;
                                let tx = Self::load_tx_receipt(tx_hash, network_id)?;
                                let incoming_request =
                                    Self::parse_cancel_incoming_request(tx, request, hash)?;
                                Self::send_register_incoming_request(
                                    incoming_request,
                                    timepoint,
                                    network_id,
                                )
                            }
                        }
                    }
                }
            }
            OffchainRequest::Outgoing(request, hash) => {
                Self::handle_outgoing_request(request, hash)
            }
            OffchainRequest::Incoming(request, hash) => {
                Self::handle_pending_incoming_requests(request, hash)
            }
        }
    }

    /// Parses the logs emitted on the Sidechain's contract to an `IncomingRequest` and imports it
    /// to Thischain.
    fn handle_logs(
        from_block: u64,
        to_block: u64,
        new_to_handle_height: &mut u64,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let logs = Self::load_transfers_logs(network_id, from_block, to_block)?;
        for log in logs {
            // Check address to be sure what it came from our contract
            if Self::ensure_known_contract(log.address.0.into(), network_id).is_err() {
                continue;
            }
            // We assume that all events issued by our contracts are valid and, therefore, ignore
            // the invalid ones.
            let event = match Self::parse_deposit_event(&log) {
                Ok(v) => v,
                Err(e) => {
                    info!("Skipped {:?}, error: {:?}", log, e);
                    continue;
                }
            };
            let tx_hash = H256(
                log.transaction_hash
                    .ok_or(Error::<T>::EthTransactionIsPending)?
                    .0,
            );
            if RequestStatuses::<T>::get(network_id, tx_hash).is_some() {
                // Skip already submitted requests.
                trace!("Skipped already submitted request: {:?}", tx_hash);
                continue;
            }
            let at_height = log
                .block_number
                .ok_or(Error::<T>::EthTransactionIsPending)?
                .as_u64();
            let transaction_index = log
                .transaction_index
                .ok_or(Error::<T>::EthTransactionIsPending)?
                .as_u64();
            let timepoint = bridge_multisig::Pallet::<T>::sidechain_timepoint(
                at_height,
                transaction_index as u32, // TODO: can it exceed u32?
            );
            info!("Got log [{}], {:?}", at_height, log);
            let load_incoming_transaction_request = LoadIncomingTransactionRequest::new(
                event.destination.clone(),
                tx_hash,
                timepoint,
                IncomingTransactionRequestKind::Transfer,
                network_id,
            );
            let inc_request_result = IncomingRequest::try_from_contract_event(
                ContractEvent::Deposit(event),
                load_incoming_transaction_request.clone(),
                at_height,
            );
            Self::send_import_incoming_request(
                LoadIncomingRequest::Transaction(load_incoming_transaction_request),
                inc_request_result.map_err(|e| e.into()),
                network_id,
            )?;
            // Update 'handled height' value when all logs at `at_height` were processed.
            if at_height > *new_to_handle_height {
                *new_to_handle_height = at_height;
                // Return to not overload the node.
                return Ok(());
            }
        }
        // All logs in the given range were processed.
        *new_to_handle_height = to_block + 1;
        Ok(())
    }

    fn handle_failed_transactions_queue() {
        let network_id = T::GetEthNetworkId::get();
        let s_failed_pending_txs =
            StorageValueRef::persistent(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY);
        let mut failed_txs = s_failed_pending_txs
            .get::<BTreeMap<H256, SignedTransactionData<T>>>()
            .ok()
            .flatten()
            .unwrap_or_default();
        let mut to_remove = Vec::new();
        for (key, tx) in &failed_txs {
            let tx_call = bridge_multisig::Call::<T>::decode(&mut &tx.call.encode()[1..]);
            if let Ok(tx_call) = tx_call {
                let maybe_call = match &tx_call {
                    bridge_multisig::Call::as_multi_threshold_1 { call, .. } => {
                        Call::<T>::decode(&mut &call.encode()[1..])
                    }
                    bridge_multisig::Call::as_multi { call, .. } => {
                        Call::<T>::decode(&mut &call[1..])
                    }
                    _ => continue,
                };
                match maybe_call {
                    Ok(Call::<T>::import_incoming_request {
                        load_incoming_request,
                        ..
                    }) => {
                        let tx_hash = load_incoming_request.hash();

                        if RequestStatuses::<T>::get(network_id, tx_hash).is_some() {
                            to_remove.push(*key);
                            // Skip already submitted requests.
                            continue;
                        }
                        let _ = Self::send_transaction::<bridge_multisig::Call<T>>(tx_call);
                    }
                    _ => (),
                };
            }
        }
        if !to_remove.is_empty() {
            for key in to_remove {
                failed_txs.remove(&key);
            }
            s_failed_pending_txs.set(&failed_txs);
        }
    }

    pub(crate) fn handle_substrate() -> Result<BlockNumberFor<T>, Error<T>>
    where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        let substrate_finalized_block = match Self::load_substrate_finalized_header() {
            Ok(v) => v,
            Err(e) => {
                info!(
                "Failed to load substrate finalized block ({:?}). Skipping off-chain procedure.",
                e
            );
                return Err(e);
            }
        };

        if substrate_finalized_block.number.as_u64() % (RE_HANDLE_TXS_PERIOD as u64) == 0 {
            Self::handle_failed_transactions_queue();
        }

        let substrate_finalized_height = <BlockNumberFor<T>>::from(
            u32::try_from(substrate_finalized_block.number).expect("cannot cast block height"),
        );
        let s_sub_to_handle_from_height =
            StorageValueRef::persistent(STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY);
        let from_block_opt = s_sub_to_handle_from_height
            .get::<BlockNumberFor<T>>()
            .map_err(|_| Error::<T>::ReadStorageError)?;
        if from_block_opt.is_none() {
            s_sub_to_handle_from_height.set(&substrate_finalized_height);
        }
        let mut from_block = from_block_opt.unwrap_or(substrate_finalized_height);
        let to_block = from_block + SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK.into();
        while from_block <= substrate_finalized_height && from_block < to_block {
            log::debug!(
                "Handle substrate block: {:?}, finalized block: {:?}",
                from_block,
                substrate_finalized_height
            );
            match Self::load_substrate_block(from_block)
                .and_then(|block| Self::handle_substrate_block(block, from_block))
            {
                Ok(_) => {}
                Err(e) => {
                    info!(
                        "Failed to handle substrate block ({:?}). Skipping off-chain procedure.",
                        e
                    );
                    return Ok(substrate_finalized_height);
                }
            };
            from_block += BlockNumberFor::<T>::one();
            // Will not process block with height bigger than finalized height
            s_sub_to_handle_from_height.set(&from_block);
        }
        Ok(substrate_finalized_height)
    }

    fn handle_ethereum(network_id: T::NetworkId) -> Result<u64, Error<T>> {
        let string = format!("eth-bridge-ocw::eth-height-{:?}", network_id);
        let s_eth_height = StorageValueRef::persistent(string.as_bytes());
        let current_eth_height = match Self::load_current_height(network_id) {
            Ok(v) => v,
            Err(e) => {
                info!(
                    "Failed to load current ethereum height. Skipping off-chain procedure. {:?}",
                    e
                );
                return Err(e);
            }
        };
        s_eth_height.set(&current_eth_height);

        let string = format!("eth-bridge-ocw::eth-to-handle-from-height-{:?}", network_id);
        let s_eth_to_handle_from_height = StorageValueRef::persistent(string.as_bytes());
        let from_block_opt = s_eth_to_handle_from_height.get::<u64>().ok().flatten();
        if from_block_opt.is_none() {
            s_eth_to_handle_from_height.set(&current_eth_height);
        }
        trace!(
            "Handle network {:?}: current height {}, from height {:?}",
            network_id,
            current_eth_height,
            from_block_opt
        );
        let from_block = from_block_opt.unwrap_or(current_eth_height);
        // The upper bound of range of blocks to download logs for. Limit the value to
        // `MAX_GET_LOGS_ITEMS` if the OCW is lagging behind Ethereum to avoid downloading too many
        // logs.
        let to_block_opt = current_eth_height
            .checked_sub(CONFIRMATION_INTERVAL)
            .map(|to_block| (from_block + MAX_GET_LOGS_ITEMS).min(to_block));
        if let Some(to_block) = to_block_opt {
            if to_block >= from_block {
                let mut new_height = from_block;
                let err_opt =
                    Self::handle_logs(from_block, to_block, &mut new_height, network_id).err();
                if new_height != from_block {
                    s_eth_to_handle_from_height.set(&new_height);
                }
                if let Some(err) = err_opt {
                    warn!("Failed to load handle logs: {:?}.", err);
                }
            }
        }
        Ok(current_eth_height)
    }

    fn handle_pending_multisig_calls(network_id: T::NetworkId, current_eth_height: u64) {
        for ms in bridge_multisig::Multisigs::<T>::iter_values() {
            let from_block = match ms.when.height {
                MultiChainHeight::Sidechain(sh)
                    if current_eth_height.saturating_sub(sh)
                        >= MAX_PENDING_TX_BLOCKS_PERIOD as u64 =>
                {
                    let string = format!(
                        "eth-bridge-ocw::eth-to-re-handle-from-height-{:?}-{}-{}",
                        network_id, sh, ms.when.index
                    );
                    let s_eth_to_handle_from_height =
                        StorageValueRef::persistent(string.as_bytes());
                    let handled = s_eth_to_handle_from_height
                        .get::<bool>()
                        .ok()
                        .flatten()
                        .unwrap_or(false);
                    if handled {
                        continue;
                    }
                    s_eth_to_handle_from_height.set(&true);
                    sh
                }
                _ => {
                    continue;
                }
            };
            debug!("Re-handling ethereum height {}", from_block);
            // +1 block should be ok, because MAX_PENDING_TX_BLOCKS_PERIOD > CONFIRMATION_INTERVAL.
            let err_opt = Self::handle_logs(from_block, from_block + 1, &mut 0, network_id).err();
            if let Some(err) = err_opt {
                warn!("Failed to re-handle logs: {:?}.", err);
            }
        }
    }

    fn is_peer_for_network(network_id: T::NetworkId) -> bool {
        let keystore_accounts = Self::get_keystore_accounts();
        let peers = Self::peers(network_id);
        for account in keystore_accounts {
            if peers.contains(&account) {
                return true;
            }
        }
        false
    }

    /// Retrieves latest needed information about networks and handles corresponding
    /// requests queues.
    ///
    /// At first, it loads current Sidechain height and current finalized Thischain height.
    /// Then it handles each request in the requests queue if it was submitted at least at
    /// the finalized height. The same is done with incoming requests queue. All handled requests
    /// are added to local storage to not be handled twice by the off-chain worker.
    pub(crate) fn handle_network(
        network_id: T::NetworkId,
        substrate_finalized_height: BlockNumberFor<T>,
    ) where
        T: CreateSignedTransaction<<T as Config>::RuntimeCall>,
    {
        if !Self::is_peer_for_network(network_id) {
            log::debug!("Node is not peer for network {:?}, skipping", network_id);
            return;
        }
        let current_eth_height = match Self::handle_ethereum(network_id) {
            Ok(v) => v,
            Err(_e) => {
                return;
            }
        };

        if substrate_finalized_height % RE_HANDLE_TXS_PERIOD.into() == BlockNumberFor::<T>::zero() {
            Self::handle_pending_multisig_calls(network_id, current_eth_height);
        }

        for request_hash in RequestsQueue::<T>::get(network_id) {
            let request = match Requests::<T>::get(network_id, request_hash) {
                Some(v) => v,
                _ => continue, // TODO: remove from queue
            };
            if request.should_be_skipped() {
                log::debug!("Temporary skip request: {:?}", request_hash);
                continue;
            }
            let request_submission_height: BlockNumberFor<T> =
                Self::request_submission_height(network_id, &request_hash);
            let number = BlockNumberFor::<T>::from(MAX_PENDING_TX_BLOCKS_PERIOD);
            let diff = substrate_finalized_height.saturating_sub(request_submission_height);
            let should_reapprove = diff >= number && diff % number == BlockNumberFor::<T>::zero();
            if !should_reapprove && substrate_finalized_height < request_submission_height {
                continue;
            }
            let handled_key = format!("eth-bridge-ocw::handled-request-{:?}", request_hash);
            let s_handled_request = StorageValueRef::persistent(handled_key.as_bytes());
            let height_opt = s_handled_request.get::<BlockNumberFor<T>>().ok().flatten();

            let need_to_handle = match height_opt {
                Some(height) => should_reapprove || request_submission_height > height,
                None => true,
            };
            let confirmed = match &request {
                OffchainRequest::Incoming(request, _) => {
                    current_eth_height.saturating_sub(request.at_height()) >= CONFIRMATION_INTERVAL
                }
                _ => true,
            };
            if need_to_handle && confirmed {
                let timepoint = request.timepoint();
                let error = Self::handle_offchain_request(request).err();
                let mut is_handled = true;
                if let Some(e) = error {
                    error!(
                        "An error occurred while processing off-chain request: {:?}",
                        e
                    );
                    if e.should_retry() {
                        is_handled = false;
                    } else if e.should_abort() {
                        if let Err(abort_err) =
                            Self::send_abort_request(request_hash, e, timepoint, network_id)
                        {
                            error!(
                                "An error occurred while trying to send abort request: {:?}",
                                abort_err
                            );
                        }
                    }
                }
                if is_handled {
                    s_handled_request.set(&request_submission_height);
                }
            }
        }
    }
}
