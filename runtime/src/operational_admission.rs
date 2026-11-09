// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Admission for signed Operational-class calls.
//!
//! The pool priority of most Operational transactions saturates:
//! `get_priority` scales the fee by `OperationalFeeMultiplier` and by the
//! number of such transactions a block holds, which exceeds `u64::MAX` with
//! 18-decimal balances, and `pool_priority` caps it just below a guardian's
//! halt. A call that can only fail would therefore tie with legitimate
//! operational traffic. Pool validation and pre-dispatch admit a signed
//! Operational call only when its signer can make it succeed:
//!
//! - council and technical committee members vote, propose and execute;
//! - anyone closes a motion that closes now within the bounds it states;
//! - anyone applies the authorized runtime upgrade;
//! - a bridge guardian halts the bridge workers, once per halt cooldown;
//! - wrappers qualify when every call they may dispatch qualifies for the
//!   account it is dispatched as.
//!
//! Every other Operational call needs Root or the None origin: System storage,
//! code and upgrade authorization calls, collective `set_members`, Democracy
//! `emergency_cancel` and `blacklist`, Utility `dispatch_as`, `with_weight` and
//! `dispatch_as_fallible`, ElectionProviderMultiPhase `submit_unsigned` and
//! EthereumBeaconClient `force_checkpoint`. Mandatory calls such as
//! `Timestamp::set` make a batch Operational and also need the None origin.
//! A signed transaction carrying any of them is rejected before inclusion.
//!
//! The checks read only storage that the admitted call reads again when it is
//! dispatched, so the call's own weight covers them.

use crate::{AccountId, BlockNumber, Hash, Runtime, RuntimeCall};
use codec::{Decode, DecodeAll, Encode};
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    migrations::MultiStepMigrator,
    weights::Weight,
};
use pallet_collective::{DefaultVote, MemberCount, ProposalIndex};
use sp_runtime::{
    traits::Hash as _,
    transaction_validity::{InvalidTransaction, TransactionValidityError},
};
use sp_std::vec::Vec;

/// Wrapper nesting followed before rejecting, as in `BridgeMultisigCallFilter`.
const MAX_DEPTH: u32 = 32;

/// Reject a signed Operational call that `who` cannot make succeed. Calls of
/// other classes are left to the existing checks.
pub fn validate(
    who: &AccountId,
    call: &RuntimeCall,
    class: DispatchClass,
) -> Result<(), TransactionValidityError> {
    if class == DispatchClass::Operational {
        admit(who, call, 0)
    } else {
        Ok(())
    }
}

fn admit(who: &AccountId, call: &RuntimeCall, depth: u32) -> Result<(), TransactionValidityError> {
    if depth >= MAX_DEPTH {
        return Err(InvalidTransaction::Call.into());
    }
    let next = depth + 1;
    match call {
        // A batch or if_else is Operational only when every call in it is
        // Operational or Mandatory. Each runs with the signer's origin.
        RuntimeCall::Utility(
            pallet_utility::Call::batch { calls }
            | pallet_utility::Call::batch_all { calls }
            | pallet_utility::Call::force_batch { calls },
        ) => calls.iter().try_for_each(|call| admit(who, call, next)),
        RuntimeCall::Utility(pallet_utility::Call::if_else { main, fallback }) => {
            admit(who, main, next)?;
            admit(who, fallback, next)
        }
        // Outside `wip`, xorless_call is Normal and never dispatches its call.
        #[cfg(feature = "wip")]
        RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. }) => admit(who, call, next),
        RuntimeCall::Utility(pallet_utility::Call::as_derivative { index, call }) => admit(
            &pallet_utility::derivative_account_id(who.clone(), *index),
            call,
            next,
        ),
        RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
            other_signatories,
            call,
        }) => {
            let multisig =
                threshold_one_multisig(who, other_signatories).ok_or(InvalidTransaction::Call)?;
            admit(&multisig, call, next)
        }
        RuntimeCall::Council(
            pallet_collective::Call::vote { .. }
            | pallet_collective::Call::propose { .. }
            | pallet_collective::Call::execute { .. },
        ) => member::<crate::CouncilCollective>(who),
        RuntimeCall::TechnicalCommittee(
            pallet_collective::Call::vote { .. }
            | pallet_collective::Call::propose { .. }
            | pallet_collective::Call::execute { .. },
        ) => member::<crate::TechnicalCollective>(who),
        RuntimeCall::Council(pallet_collective::Call::close {
            proposal_hash,
            index,
            proposal_weight_bound,
            length_bound,
        }) => closable::<crate::CouncilCollective>(
            proposal_hash,
            *index,
            *proposal_weight_bound,
            *length_bound,
        ),
        RuntimeCall::TechnicalCommittee(pallet_collective::Call::close {
            proposal_hash,
            index,
            proposal_weight_bound,
            length_bound,
        }) => closable::<crate::TechnicalCollective>(
            proposal_hash,
            *index,
            *proposal_weight_bound,
            *length_bound,
        ),
        RuntimeCall::System(frame_system::Call::apply_authorized_upgrade { code }) => {
            authorized_upgrade(code)
        }
        // A direct halt is also checked by the fee exemption, which makes it free.
        RuntimeCall::BridgeHalt(bridge_halt::Call::halt { .. }) => {
            crate::BridgeHalt::validate_halt(who)
        }
        #[cfg(feature = "private-net")]
        RuntimeCall::Sudo(
            pallet_sudo::Call::sudo { .. }
            | pallet_sudo::Call::sudo_unchecked_weight { .. }
            | pallet_sudo::Call::sudo_as { .. },
        ) => {
            if pallet_sudo::Key::<Runtime>::get().as_ref() == Some(who) {
                Ok(())
            } else {
                Err(InvalidTransaction::BadSigner.into())
            }
        }
        _ => Err(InvalidTransaction::BadSigner.into()),
    }
}

fn member<I: 'static>(who: &AccountId) -> Result<(), TransactionValidityError>
where
    Runtime: pallet_collective::Config<I>,
{
    if pallet_collective::Pallet::<Runtime, I>::is_member(who) {
        Ok(())
    } else {
        Err(InvalidTransaction::BadSigner.into())
    }
}

/// The SCALE layout of `pallet_collective::Votes`, whose fields are private.
/// The collective close tests fail if the two diverge.
#[derive(Decode)]
struct Motion {
    index: ProposalIndex,
    threshold: MemberCount,
    ayes: Vec<AccountId>,
    nays: Vec<AccountId>,
    end: BlockNumber,
}

/// `pallet_collective::Pallet::do_close` without its effects: whether the
/// motion closes in this block, and whether an approval fits the bounds.
fn closable<I: 'static>(
    proposal_hash: &Hash,
    index: ProposalIndex,
    proposal_weight_bound: Weight,
    length_bound: u32,
) -> Result<(), TransactionValidityError>
where
    Runtime: pallet_collective::Config<I, Proposal = RuntimeCall>,
{
    let cannot_close = || TransactionValidityError::from(InvalidTransaction::Call);
    let voting = sp_io::storage::get(&pallet_collective::Voting::<Runtime, I>::hashed_key_for(
        proposal_hash,
    ))
    .ok_or_else(cannot_close)?;
    let Ok(motion) = Motion::decode_all(&mut &voting[..]) else {
        // Never block governance on an unexpected layout; dispatch decides.
        return Ok(());
    };
    if motion.index != index {
        return Err(cannot_close());
    }
    let seats = pallet_collective::Members::<Runtime, I>::get().len() as MemberCount;
    let mut yes = motion.ayes.len() as MemberCount;
    let no = motion.nays.len() as MemberCount;
    if yes < motion.threshold {
        if seats.saturating_sub(no) < motion.threshold {
            // Disapproved early: closing executes nothing.
            return Ok(());
        }
        if frame_system::Pallet::<Runtime>::block_number() < motion.end {
            return Err(cannot_close());
        }
        let prime_vote =
            pallet_collective::Prime::<Runtime, I>::get().map(|prime| motion.ayes.contains(&prime));
        let default_aye = <Runtime as pallet_collective::Config<I>>::DefaultVote::default_vote(
            prime_vote, yes, no, seats,
        );
        if default_aye {
            yes = yes.saturating_add(seats.saturating_sub(yes.saturating_add(no)));
        }
        if yes < motion.threshold {
            // Disapproved at the end of the vote.
            return Ok(());
        }
    }
    // An approval executes the stored proposal only within the stated bounds.
    let proposal = pallet_collective::ProposalOf::<Runtime, I>::hashed_key_for(proposal_hash);
    let length = sp_io::storage::read(&proposal, &mut [], 0).ok_or_else(cannot_close)?;
    let proposal =
        pallet_collective::ProposalOf::<Runtime, I>::get(proposal_hash).ok_or_else(cannot_close)?;
    if length > length_bound
        || !proposal
            .get_dispatch_info()
            .call_weight
            .all_lte(proposal_weight_bound)
    {
        return Err(cannot_close());
    }
    Ok(())
}

/// Whether `code` is the authorized upgrade, which any signer may apply.
fn authorized_upgrade(code: &[u8]) -> Result<(), TransactionValidityError> {
    let unauthorized = || TransactionValidityError::from(InvalidTransaction::Call);
    let authorization =
        frame_system::Pallet::<Runtime>::authorized_upgrade().ok_or_else(unauthorized)?;
    // Wasm builds cannot read the authorized hash directly; it encodes first,
    // followed by the version-check flag.
    let Ok((code_hash, _check_version)) = <(Hash, bool)>::decode(&mut &authorization.encode()[..])
    else {
        return Ok(());
    };
    if <Runtime as frame_system::Config>::Hashing::hash(code) != code_hash
        || <Runtime as frame_system::Config>::MultiBlockMigrator::ongoing()
    {
        return Err(unauthorized());
    }
    Ok(())
}

/// The account `pallet_multisig::as_multi_threshold_1` dispatches as, or `None`
/// when it rejects the co-signatories.
fn threshold_one_multisig(who: &AccountId, other_signatories: &[AccountId]) -> Option<AccountId> {
    let max = <Runtime as pallet_multisig::Config>::MaxSignatories::get() as usize;
    if other_signatories.is_empty()
        || other_signatories.len() >= max
        || !other_signatories.windows(2).all(|pair| pair[0] < pair[1])
        || other_signatories.contains(who)
    {
        return None;
    }
    let mut signatories = other_signatories.to_vec();
    signatories.insert(
        signatories.partition_point(|signatory| signatory < who),
        who.clone(),
    );
    Some(pallet_multisig::Pallet::<Runtime>::multi_account_id(
        &signatories,
        1,
    ))
}
