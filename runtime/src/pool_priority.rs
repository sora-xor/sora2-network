// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Transaction pool priority: a guardian's direct bridge halt comes first.
//!
//! Uncapped, three kinds of transaction reach `TransactionPriority::MAX` in
//! this runtime:
//!
//! - signed Operational calls light enough to saturate `get_priority`, which
//!   scales the 18-decimal fee by `OperationalFeeMultiplier` and by the number
//!   of such calls a block holds: a guardian's halt and a council member's
//!   vote among them;
//! - ImOnline heartbeats, through `ImOnlineUnsignedPriority`;
//! - the election miner's unsigned solution, which adds its minimal stake to
//!   `MinerTxPriority`.
//!
//! The pool orders equal priorities by longevity and arrival, so a guardian's
//! fee-free halt, which cannot add a tip, could wait behind legitimate
//! operational traffic, for example a council member who keeps switching a
//! valid vote. A guardian's direct `BridgeHalt::halt` therefore takes
//! [`GUARDIAN_HALT`] alone, and every other transaction is capped at
//! [`BELOW_GUARDIAN_HALT`]. The cap merges only the two highest values, so
//! other transactions keep their order.

use crate::{Runtime, RuntimeCall, RuntimeOrigin};
use frame_support::traits::OriginTrait;
use sp_runtime::transaction_validity::{
    TransactionPriority, TransactionSource, TransactionValidity, TransactionValidityError,
};

/// The priority of a guardian's direct halt, which no other transaction reaches.
pub const GUARDIAN_HALT: TransactionPriority = TransactionPriority::MAX;

/// The highest priority of every other transaction.
pub const BELOW_GUARDIAN_HALT: TransactionPriority = GUARDIAN_HALT - 1;

/// The priority of a transaction that `AuthorizedHeartbeatPayment` validated.
///
/// A direct halt gets this far only from a signer that both
/// `operational_admission` and the halt arm of
/// `xor_fee_impls::validate_bridge_fee_exemption` admit: a guardian, while the
/// bridges run. A wrapped halt pays and is capped like any other call.
pub fn validated(
    origin: &RuntimeOrigin,
    call: &RuntimeCall,
    priority: TransactionPriority,
) -> TransactionPriority {
    match (origin.as_system_ref(), call) {
        (
            Some(frame_system::RawOrigin::Signed(_)),
            RuntimeCall::BridgeHalt(bridge_halt::Call::halt { .. }),
        ) => GUARDIAN_HALT,
        _ => priority.min(BELOW_GUARDIAN_HALT),
    }
}

/// The runtime's unsigned validation, capped below a guardian's halt.
pub struct UnsignedBelowGuardianHalt;

#[allow(deprecated)] // Executive still validates bare extrinsics through ValidateUnsigned.
impl sp_runtime::traits::ValidateUnsigned for UnsignedBelowGuardianHalt {
    type Call = RuntimeCall;

    fn pre_dispatch(call: &RuntimeCall) -> Result<(), TransactionValidityError> {
        <Runtime as sp_runtime::traits::ValidateUnsigned>::pre_dispatch(call)
    }

    fn validate_unsigned(source: TransactionSource, call: &RuntimeCall) -> TransactionValidity {
        <Runtime as sp_runtime::traits::ValidateUnsigned>::validate_unsigned(source, call).map(
            |mut validity| {
                validity.priority = validity.priority.min(BELOW_GUARDIAN_HALT);
                validity
            },
        )
    }
}
