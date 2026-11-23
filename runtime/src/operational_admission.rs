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
//! The payment extension reserves admission weight for every inspected child.
//! Utility's refund for skipped dispatches leaves this extension charge intact.

use crate::{AccountId, BlockNumber, Hash, Runtime, RuntimeCall};
use alloc::collections::{BTreeMap, BTreeSet};
use codec::{Decode, DecodeAll, Encode};
use frame_support::{
    dispatch::{DispatchClass, GetDispatchInfo},
    migrations::MultiStepMigrator,
    traits::Get,
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
        admit(who, call, 0, &mut BatchVotes::default())
    } else {
        Ok(())
    }
}

/// Reserve the admission pass as extension weight, independently of what
/// Utility ultimately dispatches. Use benchmarked SDK operations that cover
/// the same storage accesses, decoding, hashing and per-child traversal.
pub fn weight(call: &RuntimeCall) -> Weight {
    match call {
        RuntimeCall::Utility(
            pallet_utility::Call::batch { .. }
            | pallet_utility::Call::batch_all { .. }
            | pallet_utility::Call::force_batch { .. }
            | pallet_utility::Call::if_else { .. }
            | pallet_utility::Call::as_derivative { .. },
        )
        | RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 { .. })
            if call.get_dispatch_info().class != DispatchClass::Operational =>
        {
            Weight::zero()
        }
        #[cfg(feature = "private-net")]
        RuntimeCall::Sudo(_) if call.get_dispatch_info().class != DispatchClass::Operational => {
            Weight::zero()
        }
        #[cfg(feature = "wip")]
        RuntimeCall::XorFee(xor_fee::Call::xorless_call { .. })
            if call.get_dispatch_info().class != DispatchClass::Operational =>
        {
            Weight::zero()
        }
        #[cfg(feature = "private-net")]
        RuntimeCall::Sudo(_) if call.get_dispatch_info().class != DispatchClass::Operational => {
            Weight::zero()
        }
        _ => inspected_weight(call, 0),
    }
}

fn inspected_weight(call: &RuntimeCall, depth: u32) -> Weight {
    use pallet_utility::WeightInfo as _;
    type Utility = <Runtime as pallet_utility::Config>::WeightInfo;
    if depth >= MAX_DEPTH {
        return Weight::zero();
    }
    let next = depth + 1;
    match call {
        RuntimeCall::Utility(
            pallet_utility::Call::batch { calls }
            | pallet_utility::Call::batch_all { calls }
            | pallet_utility::Call::force_batch { calls },
        ) => calls
            .iter()
            .fold(Utility::batch(calls.len() as u32), |weight, call| {
                weight.saturating_add(inspected_weight(call, next))
            }),
        RuntimeCall::Utility(pallet_utility::Call::if_else { main, fallback }) => {
            Utility::if_else()
                .saturating_add(inspected_weight(main, next))
                .saturating_add(inspected_weight(fallback, next))
        }
        RuntimeCall::Utility(pallet_utility::Call::as_derivative { call, .. }) => {
            Utility::as_derivative().saturating_add(inspected_weight(call, next))
        }
        RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
            other_signatories,
            call,
        }) => {
            // Validate ordering and hash the signatory list, whose entries are AccountId32.
            use frame_system::WeightInfo as _;
            <Runtime as frame_system::Config>::SystemWeightInfo::remark_with_event(
                (other_signatories.len() as u32)
                    .saturating_add(1)
                    .saturating_mul(32),
            )
            .saturating_add(inspected_weight(call, next))
        }
        #[cfg(feature = "wip")]
        RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. }) => {
            Utility::batch(1).saturating_add(inspected_weight(call, next))
        }
        RuntimeCall::Council(call) => collective_weight::<crate::CouncilCollective>(call),
        RuntimeCall::TechnicalCommittee(call) => {
            collective_weight::<crate::TechnicalCollective>(call)
        }
        RuntimeCall::System(frame_system::Call::apply_authorized_upgrade { code }) => {
            use frame_system::WeightInfo as _;
            <Runtime as frame_system::Config>::SystemWeightInfo::remark_with_event(code.len() as u32)
                .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(2))
                .saturating_add(Weight::from_parts(0, 6_000))
        }
        RuntimeCall::BridgeHalt(bridge_halt::Call::halt { .. }) => {
            use bridge_halt::weights::WeightInfo as _;
            <Runtime as bridge_halt::Config>::WeightInfo::halt()
        }
        #[cfg(feature = "private-net")]
        RuntimeCall::Sudo(
            pallet_sudo::Call::sudo { .. }
            | pallet_sudo::Call::sudo_unchecked_weight { .. }
            | pallet_sudo::Call::sudo_as { .. },
        ) => <Runtime as frame_system::Config>::DbWeight::get()
            .reads(1)
            .saturating_add(Weight::from_parts(0, 3_000)),
        _ => Weight::zero(),
    }
}

fn collective_weight<I: 'static>(call: &pallet_collective::Call<Runtime, I>) -> Weight
where
    Runtime: pallet_collective::Config<I, Proposal = RuntimeCall>,
{
    use pallet_collective::WeightInfo as _;
    let members = <Runtime as pallet_collective::Config<I>>::MaxMembers::get();
    match call {
        pallet_collective::Call::propose {
            threshold,
            proposal,
            ..
        } if *threshold > 1 => {
            // Project the content-addressed identity of a newly proposed motion.
            use frame_system::WeightInfo as _;
            <() as pallet_collective::WeightInfo>::execute(0, members).saturating_add(
                <Runtime as frame_system::Config>::SystemWeightInfo::remark_with_event(
                    proposal.encoded_size() as u32,
                ),
            )
        }
        pallet_collective::Call::vote { .. }
        | pallet_collective::Call::propose { .. }
        | pallet_collective::Call::execute { .. } => {
            // execute(0, m) includes the single bounded membership lookup.
            <()>::execute(0, members)
        }
        pallet_collective::Call::close { length_bound, .. } => {
            // A stored proposal had to fit in one extrinsic. A caller can
            // overestimate its length; charge at most the block length for the
            // bytes inspected, and never include the proposal's execution weight.
            let block_length = <Runtime as frame_system::Config>::BlockLength::get();
            let bytes = (*length_bound).min(*block_length.max.get(DispatchClass::Operational));
            let proposals = <Runtime as pallet_collective::Config<I>>::MaxProposals::get();
            <()>::close_approved(bytes, members, proposals)
                .max(<()>::close_early_approved(bytes, members, proposals))
        }
        _ => Weight::zero(),
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Collective {
    Council,
    TechnicalCommittee,
}

/// Definite votes of each dispatch origin, until a preceding call can execute
/// arbitrary governance work. In that case current storage can no longer prove
/// a later collective call impossible. The collective still checks the actual
/// membership, tallies and bounds when the batch dispatches in order.
#[derive(Default)]
struct BatchVotes<'a> {
    // Branches borrow the preceding projections, so nested conditionals never
    // repeatedly clone an entire batch of votes. Only changed entries are merged.
    parent: Option<&'a BatchVotes<'a>>,
    votes: BTreeMap<(Collective, Hash, ProposalIndex, AccountId), Option<bool>>,
    new_motions: BTreeSet<(Collective, Hash)>,
    uncertain: bool,
}

impl BatchVotes<'_> {
    fn branch(&self) -> BatchVotes<'_> {
        BatchVotes {
            parent: Some(self),
            uncertain: self.uncertain,
            ..Default::default()
        }
    }

    fn new_motion(&self, instance: Collective, hash: Hash) -> bool {
        self.new_motions.contains(&(instance, hash))
            || self
                .parent
                .is_some_and(|parent| parent.new_motion(instance, hash))
    }

    fn project(
        &self,
        instance: Collective,
        hash: Hash,
        index: ProposalIndex,
        ballots: &mut BTreeMap<AccountId, Option<bool>>,
    ) {
        if let Some(parent) = self.parent {
            parent.project(instance, hash, index, ballots);
        }
        let first = (instance, hash, index, AccountId::new([0; 32]));
        let last = (instance, hash, index, AccountId::new([u8::MAX; 32]));
        for ((_, _, _, who), vote) in self.votes.range(first..=last) {
            ballots.insert(who.clone(), *vote);
        }
    }
    fn may_change(&mut self) {
        self.uncertain = true;
        self.votes.clear();
    }

    fn observe_execution(&mut self, proposal: &RuntimeCall) {
        if !matches!(
            proposal,
            RuntimeCall::System(
                frame_system::Call::remark { .. } | frame_system::Call::remark_with_event { .. }
            )
        ) {
            self.may_change();
        }
    }
}

fn admit(
    who: &AccountId,
    call: &RuntimeCall,
    depth: u32,
    batch: &mut BatchVotes,
) -> Result<(), TransactionValidityError> {
    if depth >= MAX_DEPTH {
        return Err(InvalidTransaction::Call.into());
    }
    let next = depth + 1;
    match call {
        RuntimeCall::Utility(
            pallet_utility::Call::batch { calls }
            | pallet_utility::Call::batch_all { calls }
            | pallet_utility::Call::force_batch { calls },
        ) => calls
            .iter()
            .try_for_each(|call| admit(who, call, next, batch)),
        RuntimeCall::Utility(pallet_utility::Call::if_else { main, fallback }) => {
            let mut primary = batch.branch();
            let mut alternative = batch.branch();
            admit(who, main, next, &mut primary)?;
            admit(who, fallback, next, &mut alternative)?;
            let uncertain = primary.uncertain || alternative.uncertain;
            let (primary_votes, mut other_votes) = (primary.votes, alternative.votes);
            let (primary_motions, other_motions) = (primary.new_motions, alternative.new_motions);
            if uncertain {
                batch.may_change();
            } else {
                // Preserve decisions shared by both branches. A vote that might
                // not execute affects only that voter and its particular motion.
                for (key, vote) in primary_votes {
                    let same_vote = other_votes.remove(&key) == Some(vote);
                    batch.votes.insert(key, if same_vote { vote } else { None });
                }
                for key in other_votes.into_keys() {
                    batch.votes.insert(key, None);
                }
            }
            batch.new_motions.extend(primary_motions);
            batch.new_motions.extend(other_motions);
            Ok(())
        }
        #[cfg(feature = "wip")]
        RuntimeCall::XorFee(xor_fee::Call::xorless_call { call, .. }) => {
            admit(who, call, next, batch)
        }
        RuntimeCall::Utility(pallet_utility::Call::as_derivative { index, call }) => admit(
            &pallet_utility::derivative_account_id(who.clone(), *index),
            call,
            next,
            batch,
        ),
        RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
            other_signatories,
            call,
        }) => {
            let multisig =
                threshold_one_multisig(who, other_signatories).ok_or(InvalidTransaction::Call)?;
            admit(&multisig, call, next, batch)
        }
        RuntimeCall::Council(call) => {
            collective::<crate::CouncilCollective>(who, call, Collective::Council, batch)
        }
        RuntimeCall::TechnicalCommittee(call) => collective::<crate::TechnicalCollective>(
            who,
            call,
            Collective::TechnicalCommittee,
            batch,
        ),
        RuntimeCall::System(frame_system::Call::apply_authorized_upgrade { code }) => {
            authorized_upgrade(code)
        }
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
                batch.may_change();
                Ok(())
            } else {
                Err(InvalidTransaction::BadSigner.into())
            }
        }
        _ => Err(InvalidTransaction::BadSigner.into()),
    }
}

fn collective<I: 'static>(
    who: &AccountId,
    call: &pallet_collective::Call<Runtime, I>,
    instance: Collective,
    batch: &mut BatchVotes,
) -> Result<(), TransactionValidityError>
where
    Runtime: pallet_collective::Config<I, Proposal = RuntimeCall>,
{
    use pallet_collective::Call;
    match call {
        Call::vote {
            proposal,
            index,
            approve,
        } => {
            if !batch.uncertain {
                member::<I>(who)?;
                batch
                    .votes
                    .insert((instance, *proposal, *index, who.clone()), Some(*approve));
            }
            Ok(())
        }
        Call::propose {
            threshold,
            proposal,
            ..
        } => {
            if !batch.uncertain {
                member::<I>(who)?;
            }
            if *threshold < 2 {
                batch.observe_execution(proposal);
            } else {
                // An ordinary proposal creates a new motion without changing
                // the membership or voting state of existing motions.
                let hash = <Runtime as frame_system::Config>::Hashing::hash_of(proposal);
                batch.new_motions.insert((instance, hash));
            }
            Ok(())
        }
        Call::execute { proposal, .. } => {
            if !batch.uncertain {
                member::<I>(who)?;
            }
            batch.observe_execution(proposal);
            Ok(())
        }
        Call::close {
            proposal_hash,
            index,
            proposal_weight_bound,
            length_bound,
        } => {
            if batch.uncertain {
                return Ok(());
            }
            closable::<I>(
                proposal_hash,
                *index,
                *proposal_weight_bound,
                *length_bound,
                instance,
                batch,
            )
        }
        // These calls require a privileged origin regardless of earlier work.
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
    instance: Collective,
    batch: &mut BatchVotes,
) -> Result<(), TransactionValidityError>
where
    Runtime: pallet_collective::Config<I, Proposal = RuntimeCall>,
{
    let cannot_close = || TransactionValidityError::from(InvalidTransaction::Call);
    let Some(voting) = sp_io::storage::get(
        &pallet_collective::Voting::<Runtime, I>::hashed_key_for(proposal_hash),
    ) else {
        if batch.new_motion(instance, *proposal_hash) {
            batch.may_change();
            return Ok(());
        }
        return Err(cannot_close());
    };
    let Ok(motion) = Motion::decode_all(&mut &voting[..]) else {
        // Never block governance on an unexpected layout; dispatch decides.
        batch.may_change();
        return Ok(());
    };
    if motion.index != index {
        return Err(cannot_close());
    }
    // A signer or derived origin may have changed its vote in this batch.
    // Each key belongs to this exact instance and motion. Range over just its
    // voters instead of rescanning all votes for unrelated motions.
    let mut ballots: BTreeMap<_, _> = motion
        .ayes
        .into_iter()
        .map(|who| (who, Some(true)))
        .chain(motion.nays.into_iter().map(|who| (who, Some(false))))
        .collect();
    batch.project(instance, *proposal_hash, index, &mut ballots);
    if ballots.values().any(Option::is_none) {
        // A conditional vote may have succeeded or failed. Dispatch decides the
        // outcome, including whether executing this close changes other motions.
        batch.may_change();
        return Ok(());
    }
    let seats = pallet_collective::Members::<Runtime, I>::get().len() as MemberCount;
    let mut yes = ballots
        .values()
        .filter(|approve| **approve == Some(true))
        .count() as MemberCount;
    let no = ballots.len() as MemberCount - yes;
    if yes < motion.threshold {
        if seats.saturating_sub(no) < motion.threshold {
            // Disapproved early: closing executes nothing.
            return Ok(());
        }
        if frame_system::Pallet::<Runtime>::block_number() < motion.end {
            return Err(cannot_close());
        }
        let prime_vote = pallet_collective::Prime::<Runtime, I>::get()
            .map(|prime| ballots.get(&prime) == Some(&Some(true)));
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
    let proposal: RuntimeCall = bounded_proposal(&proposal, length_bound)?;
    if !proposal
        .get_dispatch_info()
        .call_weight
        .all_lte(proposal_weight_bound)
    {
        return Err(cannot_close());
    }
    batch.observe_execution(&proposal);
    Ok(())
}

fn bounded_proposal<P: Decode>(
    key: &[u8],
    length_bound: u32,
) -> Result<P, TransactionValidityError> {
    // Read the encoded length first, as pallet_collective::validate_and_get_proposal
    // does. An under-bounded close must never force a full decode.
    let length = sp_io::storage::read(key, &mut [], 0).ok_or(InvalidTransaction::Call)?;
    if length > length_bound {
        return Err(InvalidTransaction::Call.into());
    }
    frame_support::storage::unhashed::get(key).ok_or_else(|| InvalidTransaction::Call.into())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_bounded_proposal_is_rejected_before_decoding() {
        struct MustNotDecode;
        impl Decode for MustNotDecode {
            fn decode<I: codec::Input>(_: &mut I) -> Result<Self, codec::Error> {
                panic!("admission decoded a proposal despite its invalid length bound");
            }
        }
        sp_io::TestExternalities::default().execute_with(|| {
            let key = b"oversized council proposal";
            sp_io::storage::set(key, &vec![0; 64 * 1024]);
            assert_eq!(
                bounded_proposal::<MustNotDecode>(key, 0).err(),
                Some(TransactionValidityError::Invalid(InvalidTransaction::Call))
            );
        });
    }
}
