// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{AccountId, Balance, Balances, Runtime, RuntimeCall};
use frame_support::traits::{Currency, WithdrawReasons};
use sp_runtime::{DispatchError, DispatchResult};

/// Sponsorship covers one authenticated migration call, never a wrapper or tip.
pub fn sponsor(
    who: &AccountId,
    call: &RuntimeCall,
    fee: Balance,
    tip: Balance,
) -> Option<AccountId> {
    let sponsor = match call {
        RuntimeCall::IrohaMigration(iroha_migration::Call::migrate {
            iroha_address,
            iroha_public_key,
            iroha_signature,
        }) => iroha_migration::Pallet::<Runtime>::fee_sponsor(
            who,
            iroha_address,
            iroha_public_key,
            iroha_signature,
            fee,
            tip,
        ),
        _ => None,
    }?;
    // A third-party promise must never block a claimant who can pay directly.
    // Choose the sponsor only while its native balance can actually fund this
    // quote and retain a live account for a possible successful refund.
    let free = Balances::free_balance(&sponsor);
    let remaining = free.checked_sub(fee)?;
    if remaining == 0 || remaining < Balances::minimum_balance() {
        return None;
    }
    Balances::ensure_can_withdraw(
        &sponsor,
        fee,
        WithdrawReasons::TRANSACTION_PAYMENT,
        remaining,
    )
    .ok()?;
    Some(sponsor)
}

pub fn consume(
    who: &AccountId,
    call: &RuntimeCall,
    sponsor: &AccountId,
    fee: Balance,
) -> DispatchResult {
    match call {
        RuntimeCall::IrohaMigration(iroha_migration::Call::migrate {
            iroha_address,
            iroha_public_key,
            ..
        }) => iroha_migration::Pallet::<Runtime>::consume_fee_sponsorship(
            who,
            iroha_address,
            iroha_public_key,
            sponsor,
            fee,
        ),
        _ => Err(DispatchError::Other("Unsupported sponsored call")),
    }
}
