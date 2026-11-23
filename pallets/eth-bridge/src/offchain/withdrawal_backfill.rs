// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::{types, WITHDRAWAL_TOPIC};

const LIVE_PASSES: u8 = 7;
const MAX_RANGE: u64 = 4096;
const PAGES_BEFORE_GROWTH: u8 = 32;

#[derive(codec::Decode, Encode)]
struct Backfill {
    // None is a durable completion marker, including for a live scan starting at genesis.
    next_block: Option<u64>,
    before_log_index: Option<u64>,
    range: u64,
    pages_until_growth: u8,
    live_passes_left: u8,
}

impl Backfill {
    fn new(live_from: u64) -> Self {
        Self {
            next_block: live_from.checked_sub(1),
            before_log_index: None,
            range: MAX_RANGE,
            pages_until_growth: PAGES_BEFORE_GROWTH,
            live_passes_left: LIVE_PASSES,
        }
    }

    fn finished_page(&mut self, from: u64) {
        self.next_block = from.checked_sub(1);
        self.before_log_index = None;
        self.pages_until_growth = self.pages_until_growth.saturating_sub(1);
        if self.pages_until_growth == 0 {
            self.range = self.range.saturating_mul(2).min(MAX_RANGE);
            self.pages_until_growth = PAGES_BEFORE_GROWTH;
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Reserve one log query every eighth regular network pass. Pending multisig
    /// rechecks keep their existing slot. Each history turn reads at most one receipt.
    pub(super) fn backfill_withdrawals(
        network_id: T::NetworkId,
        live_from: u64,
        current_eth_height: u64,
    ) -> bool {
        let contract = Self::bridge_contract_address(network_id);
        let key = format!("eth-bridge-ocw::withdrawal-backfill-v1-{network_id:?}-{contract:x}");
        let mut storage = StorageValueRef::persistent(key.as_bytes());
        let mut scan = get_storage_value_or_clear::<Backfill>(&mut storage, "withdrawal backfill")
            .unwrap_or_else(|| {
                let initial = Backfill::new(live_from);
                storage.set(&initial);
                initial
            });
        let Some(to) = scan.next_block else {
            return false;
        };
        // A newly initialized live cursor can be at the EVM head. Retain the
        // entire gap below that cursor while its newest block gains confirmations.
        if current_eth_height.saturating_sub(to) < CONFIRMATION_INTERVAL {
            return false;
        }
        if scan.live_passes_left > 0 {
            scan.live_passes_left -= 1;
            storage.set(&scan);
            return false;
        }

        scan.live_passes_left = LIVE_PASSES;
        let from = to.saturating_sub(scan.range.clamp(1, MAX_RANGE) - 1);
        let result =
            Self::load_bridge_logs(network_id, from, to, vec![types::H256(WITHDRAWAL_TOPIC.0)]);
        match result {
            Ok(logs) => {
                if let Err(error) =
                    Self::import_historical_withdrawal(network_id, &mut scan, from, to, logs)
                {
                    warn!("Retrying withdrawal backfill for network {network_id:?}, blocks {from}..={to}: {error:?}");
                }
            }
            Err(Error::<T>::HttpResponseTooLarge) => {
                scan.range = ((to - from + 1) / 2).max(1);
                scan.pages_until_growth = PAGES_BEFORE_GROWTH;
                warn!("Withdrawal backfill log limit on network {network_id:?}, blocks {from}..={to}; retrying with {} block(s)", scan.range);
            }
            Err(error) => {
                warn!("Withdrawal backfill log query failed on network {network_id:?}, blocks {from}..={to}: {error:?}");
            }
        }
        storage.set(&scan);
        true
    }

    fn import_historical_withdrawal(
        network_id: T::NetworkId,
        scan: &mut Backfill,
        from: u64,
        to: u64,
        logs: Vec<Log>,
    ) -> Result<(), Error<T>> {
        let contract = Self::bridge_contract_address(network_id);
        let mut ordered = Vec::new();
        for log in logs {
            if log.address.0 != contract.0
                || log.is_removed()
                || log.topics.as_slice() != [types::H256(WITHDRAWAL_TOPIC.0)]
                || log.data.0.len() != 32
            {
                continue;
            }
            let height = log
                .block_number
                .ok_or(Error::<T>::EthTransactionIsPending)?
                .as_u64();
            let index = u64::try_from(log.log_index.ok_or(Error::<T>::EthTransactionIsPending)?)
                .map_err(|_| Error::<T>::InvalidUint)?;
            ensure!(
                (from..=to).contains(&height),
                Error::<T>::JsonDeserializationError
            );
            if height == to && scan.before_log_index.is_some_and(|before| index >= before) {
                continue;
            }
            ordered.push(((height, index), log));
        }
        ordered.sort_unstable_by_key(|(position, _)| core::cmp::Reverse(*position));

        let mut seen = BTreeSet::new();
        let mut events = ordered.into_iter().peekable();
        while let Some(((height, index), log)) = events.next() {
            if Self::import_withdrawal_log(&log, network_id, from..=to, &mut seen)?.is_some() {
                info!("Submitted historical Hashi withdrawal from network {network_id:?}, block {height}, log {index}");
                if events.peek().is_some() {
                    scan.next_block = Some(height);
                    scan.before_log_index = Some(index);
                    return Ok(());
                }
                break;
            }
        }
        scan.finished_page(from);
        if scan.next_block.is_none() {
            info!("Completed Hashi withdrawal backfill for network {network_id:?}");
        }
        Ok(())
    }
}
