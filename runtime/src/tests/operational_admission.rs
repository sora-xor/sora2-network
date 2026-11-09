// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Signed Operational calls through the real transaction pipeline. A call its
//! signer cannot make succeed is invalid in the pool and before dispatch: it is
//! never included, charges nothing and leaves the nonce unchanged. Operational
//! calls that can succeed are admitted as before.

use crate::{
    AccountId, Balances, Council, CouncilCollective, Executive, OriginCaller, Runtime, RuntimeCall,
    RuntimeEvent, RuntimeOrigin, Signature, SignedExtra, System, TechnicalCollective,
    TechnicalCommittee, UncheckedExtrinsic, XorFee,
};
use codec::Encode;
use common::balance;
use frame_support::{
    assert_ok,
    dispatch::{DispatchClass, GetDispatchInfo},
    traits::Currency,
    weights::Weight,
};
use framenode_chain_spec::ext;
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{
    generic::Era,
    traits::Hash as _,
    transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError,
    },
    DispatchResult, FixedPointNumber, FixedU128,
};

fn pair(seed: u8) -> sr25519::Pair {
    sr25519::Pair::from_seed(&[seed; 32])
}

fn account(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public())
}

fn stranger() -> sr25519::Pair {
    pair(71)
}

/// The council, in the sorted order `set_members` stores.
fn members() -> Vec<sr25519::Pair> {
    let mut members = vec![pair(72), pair(73), pair(74)];
    members.sort_by_key(account);
    members
}

fn signed(call: RuntimeCall, pair: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = account(pair);
    let extra: SignedExtra = (
        frame_system::AuthorizeCall::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),
        frame_system::CheckWeight::<Runtime>::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|payload| pair.sign(payload));
    UncheckedExtrinsic::new_signed(call, who, Signature::Sr25519(signature), extra)
}

/// Pool validation, discarding the validation block Executive initializes.
fn validate(call: RuntimeCall, pair: &sr25519::Pair) -> TransactionValidity {
    sp_io::storage::start_transaction();
    let validity = Executive::validate_transaction(
        TransactionSource::External,
        signed(call, pair),
        Default::default(),
    );
    sp_io::storage::rollback_transaction();
    validity
}

fn fresh_block() {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    System::reset_events();
}

/// Transaction weight in the block. The block's first application also runs
/// the `on_poll` hooks, which add Mandatory weight whatever the extrinsic.
fn transaction_weight() -> (Weight, Weight) {
    let weight = frame_system::BlockWeight::<Runtime>::get();
    (
        *weight.get(DispatchClass::Normal),
        *weight.get(DispatchClass::Operational),
    )
}

/// The Operational call is invalid in the pool and in the block. An Err from
/// apply_extrinsic makes the block builder drop it, so it is never included:
/// no fee, no nonce, no transaction weight and no events.
fn rejected(call: RuntimeCall, pair: &sr25519::Pair, reason: InvalidTransaction) {
    fresh_block();
    let who = account(pair);
    assert_eq!(call.get_dispatch_info().class, DispatchClass::Operational);
    assert_eq!(
        validate(call.clone(), pair),
        Err(TransactionValidityError::Invalid(reason)),
        "{call:?} must be invalid in the pool"
    );
    let balance = Balances::free_balance(&who);
    let nonce = System::account_nonce(&who);
    let weight = transaction_weight();
    assert_eq!(
        Executive::apply_extrinsic(signed(call, pair)),
        Err(TransactionValidityError::Invalid(reason))
    );
    assert_eq!(Balances::free_balance(&who), balance);
    assert_eq!(System::account_nonce(&who), nonce);
    assert_eq!(transaction_weight(), weight);
    assert!(System::events().is_empty());
}

/// The Operational call is valid in the pool and applied. Returns its result.
fn admitted(call: RuntimeCall, pair: &sr25519::Pair) -> DispatchResult {
    fresh_block();
    let who = account(pair);
    assert_eq!(call.get_dispatch_info().class, DispatchClass::Operational);
    assert!(
        validate(call.clone(), pair).is_ok(),
        "{call:?} must be valid in the pool"
    );
    let nonce = System::account_nonce(&who);
    let result = Executive::apply_extrinsic(signed(call, pair))
        .expect("an admitted Operational call is applied");
    assert_eq!(System::account_nonce(&who), nonce + 1);
    result
}

/// Every account holds XOR, so a charged fee would show as a balance change.
fn setup() {
    System::set_block_number(1);
    for pair in members().iter().chain([stranger()].iter()) {
        let _ = Balances::make_free_balance_be(&account(pair), balance!(10));
    }
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
    set_council(members().iter().map(account).collect(), None);
    fresh_block();
}

fn set_council(new_members: Vec<AccountId>, prime: Option<AccountId>) {
    let old_count = pallet_collective::Members::<Runtime, CouncilCollective>::get().len() as u32;
    assert_ok!(Council::set_members(
        RuntimeOrigin::root(),
        new_members,
        prime,
        old_count
    ));
}

fn set_storage() -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::set_storage {
        items: vec![(b"operational-admission".to_vec(), vec![1])],
    })
}

fn kill_storage() -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::kill_storage {
        keys: vec![b"operational-admission".to_vec()],
    })
}

/// A proposal any origin can execute.
fn remark(text: &[u8]) -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark {
        remark: text.to_vec(),
    })
}

fn propose(threshold: u32, proposal: &RuntimeCall) -> RuntimeCall {
    RuntimeCall::Council(pallet_collective::Call::propose {
        threshold,
        proposal: Box::new(proposal.clone()),
        length_bound: proposal.encoded_size() as u32,
    })
}

fn vote(proposal: H256, index: u32, approve: bool) -> RuntimeCall {
    RuntimeCall::Council(pallet_collective::Call::vote {
        proposal,
        index,
        approve,
    })
}

fn close(proposal_hash: H256, index: u32, weight: Weight, length: u32) -> RuntimeCall {
    RuntimeCall::Council(pallet_collective::Call::close {
        proposal_hash,
        index,
        proposal_weight_bound: weight,
        length_bound: length,
    })
}

/// Close with exactly the bounds the stored proposal needs.
fn exact_close(proposal: &RuntimeCall, index: u32) -> RuntimeCall {
    close(
        <Runtime as frame_system::Config>::Hashing::hash_of(proposal),
        index,
        proposal.get_dispatch_info().call_weight,
        proposal.encoded_size() as u32,
    )
}

/// A council motion opened outside the transaction pipeline. The proposer's
/// aye counts only once they vote.
fn motion(proposer: &sr25519::Pair, threshold: u32, proposal: &RuntimeCall) -> (H256, u32) {
    let index = pallet_collective::ProposalCount::<Runtime, CouncilCollective>::get();
    assert_ok!(Council::propose(
        RuntimeOrigin::signed(account(proposer)),
        threshold,
        Box::new(proposal.clone()),
        proposal.encoded_size() as u32,
    ));
    (
        <Runtime as frame_system::Config>::Hashing::hash_of(proposal),
        index,
    )
}

fn council_event(event: pallet_collective::Event<Runtime, CouncilCollective>) -> bool {
    let found = System::events()
        .iter()
        .any(|record| record.event == RuntimeEvent::Council(event.clone()));
    if !found {
        println!("{event:?} not among {:#?}", System::events());
    }
    found
}

#[test]
fn root_and_none_origin_operational_calls_are_rejected_before_inclusion() {
    ext().execute_with(|| {
        setup();
        let code_hash = H256::repeat_byte(1);
        let raw_solution = pallet_election_provider_multi_phase::RawSolution {
            solution: Default::default(),
            score: Default::default(),
            round: 1,
        };
        for call in [
            RuntimeCall::System(frame_system::Call::set_heap_pages { pages: 64 }),
            RuntimeCall::System(frame_system::Call::set_code { code: vec![1] }),
            RuntimeCall::System(frame_system::Call::set_code_without_checks { code: vec![1] }),
            set_storage(),
            kill_storage(),
            RuntimeCall::System(frame_system::Call::kill_prefix {
                prefix: b"operational-admission".to_vec(),
                subkeys: 1,
            }),
            RuntimeCall::System(frame_system::Call::authorize_upgrade { code_hash }),
            RuntimeCall::System(frame_system::Call::authorize_upgrade_without_checks { code_hash }),
            RuntimeCall::Council(pallet_collective::Call::set_members {
                new_members: vec![account(&stranger())],
                prime: None,
                old_count: 3,
            }),
            RuntimeCall::TechnicalCommittee(pallet_collective::Call::set_members {
                new_members: vec![account(&stranger())],
                prime: None,
                old_count: 0,
            }),
            RuntimeCall::Democracy(pallet_democracy::Call::emergency_cancel { ref_index: 0 }),
            RuntimeCall::Democracy(pallet_democracy::Call::blacklist {
                proposal_hash: code_hash,
                maybe_ref_index: None,
            }),
            RuntimeCall::ElectionProviderMultiPhase(
                pallet_election_provider_multi_phase::Call::submit_unsigned {
                    raw_solution: Box::new(raw_solution),
                    witness: Default::default(),
                },
            ),
        ] {
            rejected(call.clone(), &stranger(), InvalidTransaction::BadSigner);
            // Council membership confers none of these origins either.
            rejected(call, &members()[0], InvalidTransaction::BadSigner);
        }
    });
}

#[test]
fn wrappers_are_admitted_only_when_every_call_they_dispatch_is() {
    ext().execute_with(|| {
        setup();
        let member = &members()[0];
        let (hash, index) = motion(member, 3, &remark(b"wrapped"));
        let other = account(&members()[1]);
        let root = || Box::new(OriginCaller::system(frame_system::RawOrigin::Root));
        for call in [
            RuntimeCall::Utility(pallet_utility::Call::batch {
                calls: vec![set_storage()],
            }),
            RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![set_storage(), kill_storage()],
            }),
            RuntimeCall::Utility(pallet_utility::Call::force_batch {
                calls: vec![set_storage()],
            }),
            RuntimeCall::Utility(pallet_utility::Call::if_else {
                main: Box::new(set_storage()),
                fallback: Box::new(kill_storage()),
            }),
            RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                index: 0,
                call: Box::new(set_storage()),
            }),
            RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                other_signatories: vec![other.clone()],
                call: Box::new(set_storage()),
            }),
            RuntimeCall::Utility(pallet_utility::Call::dispatch_as {
                as_origin: root(),
                call: Box::new(set_storage()),
            }),
            RuntimeCall::Utility(pallet_utility::Call::dispatch_as_fallible {
                as_origin: root(),
                call: Box::new(set_storage()),
            }),
            RuntimeCall::Utility(pallet_utility::Call::with_weight {
                call: Box::new(set_storage()),
                weight: Weight::from_parts(1, 0),
            }),
            // A Mandatory call makes a batch Operational and needs the None origin.
            RuntimeCall::Utility(pallet_utility::Call::batch {
                calls: vec![RuntimeCall::Timestamp(pallet_timestamp::Call::set {
                    now: 1,
                })],
            }),
            // One call the member can make does not carry one it cannot.
            RuntimeCall::Utility(pallet_utility::Call::force_batch {
                calls: vec![vote(hash, index, false), set_storage()],
            }),
        ] {
            rejected(call, member, InvalidTransaction::BadSigner);
        }

        // A batch with a Normal call is Normal and keeps its ordinary admission.
        let mixed = RuntimeCall::Utility(pallet_utility::Call::batch {
            calls: vec![remark(b"normal"), set_storage()],
        });
        assert_eq!(mixed.get_dispatch_info().class, DispatchClass::Normal);
        assert_ok!(validate(mixed, &stranger()));
    });
}

#[test]
fn only_collective_members_vote_propose_and_execute() {
    ext().execute_with(|| {
        setup();
        let proposal = remark(b"members only");
        let hash = <Runtime as frame_system::Config>::Hashing::hash_of(&proposal);
        let index = pallet_collective::ProposalCount::<Runtime, CouncilCollective>::get();
        let execute = RuntimeCall::Council(pallet_collective::Call::execute {
            proposal: Box::new(remark(b"executed")),
            length_bound: remark(b"executed").encoded_size() as u32,
        });

        for call in [
            propose(2, &proposal),
            vote(hash, index, true),
            execute.clone(),
            RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![vote(hash, index, true)],
            }),
            RuntimeCall::TechnicalCommittee(pallet_collective::Call::vote {
                proposal: hash,
                index: 0,
                approve: true,
            }),
        ] {
            rejected(call, &stranger(), InvalidTransaction::BadSigner);
        }

        let council = members();
        let [first, second, third] = &council[..] else {
            unreachable!()
        };
        assert_ok!(admitted(propose(2, &proposal), first));
        assert_ok!(admitted(vote(hash, index, true), second));
        assert_ok!(admitted(
            RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![vote(hash, index, false)],
            }),
            third
        ));
        assert_ok!(admitted(execute, third));
        assert!(council_event(pallet_collective::Event::MemberExecuted {
            proposal_hash: <Runtime as frame_system::Config>::Hashing::hash_of(&remark(
                b"executed"
            )),
            result: Ok(()),
        }));

        // The technical committee checks its own membership.
        let old_count =
            pallet_collective::Members::<Runtime, TechnicalCollective>::get().len() as u32;
        assert_ok!(TechnicalCommittee::set_members(
            RuntimeOrigin::root(),
            vec![account(&stranger())],
            None,
            old_count,
        ));
        rejected(
            RuntimeCall::TechnicalCommittee(pallet_collective::Call::propose {
                threshold: 2,
                proposal: Box::new(proposal.clone()),
                length_bound: proposal.encoded_size() as u32,
            }),
            first,
            InvalidTransaction::BadSigner,
        );
        assert_ok!(admitted(
            RuntimeCall::TechnicalCommittee(pallet_collective::Call::propose {
                threshold: 2,
                proposal: Box::new(proposal.clone()),
                length_bound: proposal.encoded_size() as u32,
            }),
            &stranger(),
        ));
    });
}

#[test]
fn anyone_closes_a_motion_that_closes_now_within_its_bounds() {
    ext().execute_with(|| {
        setup();
        let council = members();
        let [first, second, _] = &council[..] else {
            unreachable!()
        };
        let proposal = remark(b"approved early");
        let (hash, index) = motion(first, 2, &proposal);
        for member in [first, second] {
            assert_ok!(Council::vote(
                RuntimeOrigin::signed(account(member)),
                hash,
                index,
                true
            ));
        }
        let weight = proposal.get_dispatch_info().call_weight;
        let length = proposal.encoded_size() as u32;
        assert!(weight.ref_time() > 0);

        for call in [
            close(H256::repeat_byte(9), index, weight, length),
            close(hash, index + 1, weight, length),
            close(hash, index, weight, length - 1),
            close(
                hash,
                index,
                Weight::from_parts(weight.ref_time() - 1, weight.proof_size()),
                length,
            ),
            // An unknown motion stating a large weight bound.
            close(
                H256::repeat_byte(9),
                index,
                Weight::from_parts(1_000_000_000_000, 0),
                length,
            ),
        ] {
            rejected(call, &stranger(), InvalidTransaction::Call);
        }

        assert_ok!(admitted(exact_close(&proposal, index), &stranger()));
        assert!(council_event(pallet_collective::Event::Closed {
            proposal_hash: hash,
            yes: 2,
            no: 0,
        }));
        assert!(council_event(pallet_collective::Event::Executed {
            proposal_hash: hash,
            result: Ok(()),
        }));
        // The motion is gone, so a second close cannot succeed.
        rejected(
            exact_close(&proposal, index),
            &stranger(),
            InvalidTransaction::Call,
        );
    });
}

#[test]
fn a_batch_may_close_after_its_deciding_vote() {
    ext().execute_with(|| {
        setup();
        let council = members();
        let [first, second, _] = &council[..] else {
            unreachable!()
        };
        for kind in 0..3u8 {
            let proposal = remark(&[kind]);
            let (hash, index) = motion(first, 2, &proposal);
            assert_ok!(Council::vote(
                RuntimeOrigin::signed(account(first)),
                hash,
                index,
                true
            ));
            // The deciding vote changes an existing nay to an aye.
            assert_ok!(Council::vote(
                RuntimeOrigin::signed(account(second)),
                hash,
                index,
                false
            ));
            let batch = |calls| {
                RuntimeCall::Utility(match kind {
                    0 => pallet_utility::Call::batch { calls },
                    1 => pallet_utility::Call::batch_all { calls },
                    _ => pallet_utility::Call::force_batch { calls },
                })
            };
            let deciding = vote(hash, index, true);
            let finish = exact_close(&proposal, index);
            rejected(
                batch(vec![finish.clone(), deciding.clone()]),
                second,
                InvalidTransaction::Call,
            );
            rejected(
                batch(vec![vote(hash, index + 1, true), finish.clone()]),
                second,
                InvalidTransaction::Call,
            );
            rejected(
                batch(vec![
                    deciding.clone(),
                    close(hash, index, Weight::zero(), 0),
                ]),
                second,
                InvalidTransaction::Call,
            );
            rejected(
                batch(vec![deciding.clone(), finish.clone()]),
                &stranger(),
                InvalidTransaction::BadSigner,
            );
            assert_ok!(admitted(batch(vec![deciding, finish]), second));
            assert!(council_event(pallet_collective::Event::Closed {
                proposal_hash: hash,
                yes: 2,
                no: 0
            }));
            assert!(council_event(pallet_collective::Event::Executed {
                proposal_hash: hash,
                result: Ok(())
            }));
        }
    });
}

#[test]
fn technical_committee_can_close_an_early_disapproval_in_its_voting_batch() {
    ext().execute_with(|| {
        setup();
        let council = members();
        let [first, second, _] = &council[..] else {
            unreachable!()
        };
        let old_count =
            pallet_collective::Members::<Runtime, TechnicalCollective>::get().len() as u32;
        assert_ok!(TechnicalCommittee::set_members(
            RuntimeOrigin::root(),
            council.iter().map(account).collect(),
            None,
            old_count
        ));
        let proposal = remark(b"committee vote");
        let (hash, index) = motion(first, 2, &proposal);
        assert_ok!(Council::vote(
            RuntimeOrigin::signed(account(first)),
            hash,
            index,
            false
        ));
        let tc_index = pallet_collective::ProposalCount::<Runtime, TechnicalCollective>::get();
        assert_ok!(TechnicalCommittee::propose(
            RuntimeOrigin::signed(account(first)),
            2,
            Box::new(proposal.clone()),
            proposal.encoded_size() as u32
        ));
        assert_ok!(TechnicalCommittee::vote(
            RuntimeOrigin::signed(account(first)),
            hash,
            tc_index,
            false
        ));
        let nay = RuntimeCall::TechnicalCommittee(pallet_collective::Call::vote {
            proposal: hash,
            index: tc_index,
            approve: false,
        });
        let batch = |calls| RuntimeCall::Utility(pallet_utility::Call::batch_all { calls });
        assert_eq!(index, tc_index);
        // A matching hash and index do not mix two collective instances.
        rejected(
            batch(vec![nay.clone(), close(hash, index, Weight::zero(), 0)]),
            second,
            InvalidTransaction::Call,
        );
        let finish = RuntimeCall::TechnicalCommittee(pallet_collective::Call::close {
            proposal_hash: hash,
            index: tc_index,
            proposal_weight_bound: Weight::zero(),
            length_bound: 0,
        });
        assert_ok!(admitted(batch(vec![nay, finish]), second));
        assert!(System::events().iter().any(|record| record.event
            == RuntimeEvent::TechnicalCommittee(pallet_collective::Event::Disapproved {
                proposal_hash: hash
            })));
    });
}

#[test]
fn an_undecided_motion_closes_only_when_its_vote_ends() {
    ext().execute_with(|| {
        setup();
        let council = members();
        let [first, second, third] = &council[..] else {
            unreachable!()
        };
        // The prime's aye becomes the default vote of members who abstain.
        set_council(
            members().iter().map(account).collect(),
            Some(account(first)),
        );
        let proposal = remark(b"approved by default");
        let (hash, index) = motion(first, 3, &proposal);
        assert_ok!(Council::vote(
            RuntimeOrigin::signed(account(first)),
            hash,
            index,
            true
        ));
        let end = System::block_number() + crate::CouncilCollectiveMotionDuration::get();

        // Pool validation runs in the next block, so stop two blocks short.
        System::set_block_number(end - 2);
        rejected(
            exact_close(&proposal, index),
            &stranger(),
            InvalidTransaction::Call,
        );

        System::set_block_number(end);
        rejected(
            close(
                hash,
                index,
                proposal.get_dispatch_info().call_weight,
                proposal.encoded_size() as u32 - 1,
            ),
            &stranger(),
            InvalidTransaction::Call,
        );
        assert_ok!(admitted(exact_close(&proposal, index), &stranger()));
        assert!(council_event(pallet_collective::Event::Executed {
            proposal_hash: hash,
            result: Ok(()),
        }));

        // Enough nays settle a motion before its end; closing executes nothing.
        let rejected_proposal = remark(b"disapproved early");
        let (hash, index) = motion(first, 3, &rejected_proposal);
        for member in [second, third] {
            assert_ok!(Council::vote(
                RuntimeOrigin::signed(account(member)),
                hash,
                index,
                false
            ));
        }
        assert_ok!(admitted(close(hash, index, Weight::zero(), 0), &stranger()));
        assert!(council_event(pallet_collective::Event::Disapproved {
            proposal_hash: hash,
        }));
    });
}

#[test]
fn wrapped_calls_are_judged_for_the_account_they_dispatch_as() {
    ext().execute_with(|| {
        setup();
        let who = account(&stranger());
        let cosigner = account(&members()[0]);
        let mut signatories = vec![who.clone(), cosigner.clone()];
        signatories.sort();
        let multisig = pallet_multisig::Pallet::<Runtime>::multi_account_id(&signatories, 1);
        let derivative = pallet_utility::derivative_account_id(who.clone(), 3);
        let mut council: Vec<_> = members().iter().map(account).collect();
        council.extend([multisig, derivative.clone()]);
        council.sort();
        set_council(council, None);
        let (hash, index) = motion(&members()[0], 3, &remark(b"derived voters"));

        assert_ok!(admitted(
            RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                other_signatories: vec![cosigner.clone()],
                call: Box::new(vote(hash, index, true)),
            }),
            &stranger(),
        ));
        assert_ok!(admitted(
            RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                index: 3,
                call: Box::new(vote(hash, index, true)),
            }),
            &stranger(),
        ));
        assert!(council_event(pallet_collective::Event::Voted {
            account: derivative,
            proposal_hash: hash,
            voted: true,
            yes: 2,
            no: 0,
        }));

        // Another derivative or co-signatory set is a different, non-member account.
        rejected(
            RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                index: 4,
                call: Box::new(vote(hash, index, true)),
            }),
            &stranger(),
            InvalidTransaction::BadSigner,
        );
        rejected(
            RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                other_signatories: vec![account(&members()[1])],
                call: Box::new(vote(hash, index, true)),
            }),
            &stranger(),
            InvalidTransaction::BadSigner,
        );
        // Co-signatory lists the pallet refuses never dispatch.
        for other_signatories in [vec![], vec![who.clone()], vec![cosigner.clone(), cosigner]] {
            rejected(
                RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                    other_signatories,
                    call: Box::new(vote(hash, index, true)),
                }),
                &stranger(),
                InvalidTransaction::Call,
            );
        }
    });
}

#[test]
fn anyone_applies_the_authorized_upgrade_with_its_code() {
    ext().execute_with(|| {
        setup();
        let code = b"authorized runtime".to_vec();
        let apply = |code: &[u8]| {
            RuntimeCall::System(frame_system::Call::apply_authorized_upgrade {
                code: code.to_vec(),
            })
        };
        rejected(apply(&code), &stranger(), InvalidTransaction::Call);

        assert_ok!(System::authorize_upgrade_without_checks(
            RuntimeOrigin::root(),
            <Runtime as frame_system::Config>::Hashing::hash(&code),
        ));
        rejected(
            apply(b"another runtime"),
            &stranger(),
            InvalidTransaction::Call,
        );
        assert_ok!(admitted(apply(&code), &stranger()));
        assert_eq!(
            sp_io::storage::get(sp_core::storage::well_known_keys::CODE).map(|code| code.to_vec()),
            Some(code.clone())
        );
        // Applying consumes the authorization.
        rejected(apply(&code), &stranger(), InvalidTransaction::Call);
    });
}
