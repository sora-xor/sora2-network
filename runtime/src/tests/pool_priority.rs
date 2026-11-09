// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

//! Pool priority through the real transaction pipeline. A guardian's direct
//! halt outranks every other transaction: the operational calls council members
//! can repeat at will, tipped and wrapped calls, and the election miner's
//! unsigned solution all rank below it.

use crate::pool_priority::{UnsignedBelowGuardianHalt, BELOW_GUARDIAN_HALT, GUARDIAN_HALT};
use crate::{
    AccountId, Balance, Balances, BridgeHalt, Council, CouncilCollective, Executive,
    NposCompactSolution24, Runtime, RuntimeCall, RuntimeOrigin, Signature, SignedExtra, System,
    UncheckedExtrinsic, XorFee,
};
use codec::Encode;
use common::{balance, recovery::BRIDGE_HALT_STORAGE_KEY};
use frame_support::{assert_ok, dispatch::GetDispatchInfo, traits::Currency};
use framenode_chain_spec::ext;
use pallet_election_provider_multi_phase as election;
use sp_core::{sr25519, Pair, H256};
use sp_runtime::{
    generic::Era,
    traits::Hash as _,
    transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
    FixedPointNumber, FixedU128,
};

fn pair(seed: u8) -> sr25519::Pair {
    sr25519::Pair::from_seed(&[seed; 32])
}

fn account(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public())
}

fn guardian() -> sr25519::Pair {
    pair(81)
}

fn stranger() -> sr25519::Pair {
    pair(82)
}

/// The council, in the sorted order `set_members` stores.
fn members() -> Vec<sr25519::Pair> {
    let mut members = vec![pair(83), pair(84), pair(85)];
    members.sort_by_key(account);
    members
}

fn signed(call: RuntimeCall, pair: &sr25519::Pair, tip: Balance) -> UncheckedExtrinsic {
    let who = account(pair);
    let extra: SignedExtra = (
        frame_system::AuthorizeCall::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),
        frame_system::CheckWeight::<Runtime>::new(),
        crate::ChargeTxPaymentExtension::from(xor_fee::extension::ChargeTransactionPayment::<
            Runtime,
        >::from(tip)),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|payload| pair.sign(payload));
    UncheckedExtrinsic::new_signed(call, who, Signature::Sr25519(signature), extra)
}

/// Pool validation, discarding the validation block Executive initializes.
fn validate(source: TransactionSource, xt: UncheckedExtrinsic) -> TransactionValidity {
    sp_io::storage::start_transaction();
    let validity = Executive::validate_transaction(source, xt, Default::default());
    sp_io::storage::rollback_transaction();
    validity
}

/// The pool priority of a signed transaction, which must be valid.
fn priority(call: RuntimeCall, pair: &sr25519::Pair, tip: Balance) -> TransactionPriority {
    match validate(TransactionSource::External, signed(call.clone(), pair, tip)) {
        Ok(validity) => validity.priority,
        Err(error) => panic!("{call:?} must be valid in the pool: {error:?}"),
    }
}

/// What `get_priority` gives a signed Operational transaction before the cap.
fn uncapped(call: RuntimeCall, pair: &sr25519::Pair, tip: Balance) -> TransactionPriority {
    let xt = signed(call, pair, tip);
    let info = xt.get_dispatch_info();
    let len = xt.encoded_size();
    let (fee, _) = XorFee::compute_fee(len as u32, &xt.function, &info, tip);
    pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::get_priority(
        &info, len, tip, fee,
    )
}

/// Every account holds XOR, the council has three members and a guardian may
/// halt the bridge workers.
fn setup() {
    System::set_block_number(1);
    for pair in members().iter().chain([guardian(), stranger()].iter()) {
        let _ = Balances::make_free_balance_be(&account(pair), balance!(10));
    }
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
    let old_count = pallet_collective::Members::<Runtime, CouncilCollective>::get().len() as u32;
    assert_ok!(Council::set_members(
        RuntimeOrigin::root(),
        members().iter().map(account).collect(),
        None,
        old_count,
    ));
    assert_ok!(BridgeHalt::add_guardian(
        RuntimeOrigin::root(),
        account(&guardian())
    ));
    sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
}

fn halt() -> RuntimeCall {
    RuntimeCall::BridgeHalt(bridge_halt::Call::halt {
        reason: b"bridge keys leaked".to_vec().try_into().unwrap(),
    })
}

fn remark(text: &[u8]) -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark {
        remark: text.to_vec(),
    })
}

fn hash(proposal: &RuntimeCall) -> H256 {
    <Runtime as frame_system::Config>::Hashing::hash_of(proposal)
}

/// A council motion opened outside the pool. Returns its index.
fn motion(proposer: &sr25519::Pair, threshold: u32, proposal: &RuntimeCall) -> u32 {
    let index = pallet_collective::ProposalCount::<Runtime, CouncilCollective>::get();
    assert_ok!(Council::propose(
        RuntimeOrigin::signed(account(proposer)),
        threshold,
        Box::new(proposal.clone()),
        proposal.encoded_size() as u32,
    ));
    index
}

#[test]
fn a_guardian_halt_outranks_council_votes_closes_and_proposals() {
    ext().execute_with(|| {
        setup();
        let council = members();
        let [first, second, third] = &council[..] else {
            unreachable!()
        };
        let anyone = stranger();

        // Two ayes approve this motion, so anyone may close it now.
        let approved = remark(b"approved");
        let approved_index = motion(first, 2, &approved);
        for member in [first, second] {
            assert_ok!(Council::vote(
                RuntimeOrigin::signed(account(member)),
                hash(&approved),
                approved_index,
                true
            ));
        }
        let close = RuntimeCall::Council(pallet_collective::Call::close {
            proposal_hash: hash(&approved),
            index: approved_index,
            proposal_weight_bound: approved.get_dispatch_info().call_weight,
            length_bound: approved.encoded_size() as u32,
        });
        // A member may switch a valid vote on an open motion again and again.
        let open = remark(b"open");
        let open_index = motion(first, 3, &open);
        assert_ok!(Council::vote(
            RuntimeOrigin::signed(account(third)),
            hash(&open),
            open_index,
            true
        ));
        let switch = RuntimeCall::Council(pallet_collective::Call::vote {
            proposal: hash(&open),
            index: open_index,
            approve: false,
        });
        let proposal = remark(b"proposed");
        let propose = RuntimeCall::Council(pallet_collective::Call::propose {
            threshold: 2,
            proposal: Box::new(proposal.clone()),
            length_bound: proposal.encoded_size() as u32,
        });

        // Uncapped, a member's vote ties with the halt: both saturate `get_priority`.
        assert_eq!(uncapped(halt(), &guardian(), 0), TransactionPriority::MAX);
        assert_eq!(uncapped(switch.clone(), third, 0), TransactionPriority::MAX);
        let halt_priority = priority(halt(), &guardian(), 0);
        assert_eq!(halt_priority, GUARDIAN_HALT);
        assert_eq!(priority(switch.clone(), third, 0), BELOW_GUARDIAN_HALT);

        let wrapped_halt = RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![halt()],
        });
        // Heavier calls such as close and propose never saturated and keep
        // their priority; the rest stop one below the halt.
        for (name, call, signer, tip) in [
            ("vote", switch.clone(), third, 0),
            // A tip cannot lift a call past the cap.
            ("tipped vote", switch, third, balance!(1)),
            ("close", close, &anyone, 0),
            ("propose", propose, first, 0),
            // Only the direct halt ranks first; a wrapped one pays like any call.
            ("wrapped halt", wrapped_halt, &guardian(), 0),
        ] {
            let validated = priority(call.clone(), signer, tip);
            assert_eq!(
                validated,
                uncapped(call, signer, tip).min(BELOW_GUARDIAN_HALT),
                "{name} keeps its priority up to the cap"
            );
            assert!(validated < halt_priority, "{name} ranks below the halt");
        }
        // Lower priorities are unchanged.
        assert_eq!(priority(remark(b"normal"), &anyone, 0), 0);
    });
}

#[test]
#[allow(deprecated)] // Executive still validates bare extrinsics through ValidateUnsigned.
fn the_election_miners_solution_ranks_below_a_guardian_halt() {
    use sp_runtime::traits::ValidateUnsigned;

    ext().execute_with(|| {
        setup();
        election::CurrentPhase::<Runtime>::put(election::Phase::Unsigned((
            true,
            System::block_number(),
        )));
        election::DesiredTargets::<Runtime>::put(1);
        // A winner backed by 18-decimal XOR has a minimal stake above u64::MAX.
        let stake = balance!(1000);
        let call = RuntimeCall::ElectionProviderMultiPhase(election::Call::submit_unsigned {
            raw_solution: Box::new(election::RawSolution {
                solution: NposCompactSolution24 {
                    votes1: vec![(0, 0)],
                    ..Default::default()
                },
                score: sp_npos_elections::ElectionScore {
                    minimal_stake: stake,
                    sum_stake: stake,
                    ..Default::default()
                },
                round: election::Round::<Runtime>::get(),
            }),
            witness: election::SolutionOrSnapshotSize {
                voters: 1,
                targets: 1,
            },
        });

        // On its own, the pallet saturates the solution's priority.
        assert_eq!(
            <Runtime as ValidateUnsigned>::validate_unsigned(TransactionSource::Local, &call)
                .map(|validity| validity.priority),
            Ok(TransactionPriority::MAX)
        );
        let solution = validate(
            TransactionSource::Local,
            UncheckedExtrinsic::new_bare(call.clone()),
        )
        .expect("the local miner's solution is valid in the unsigned phase");
        assert_eq!(solution.priority, BELOW_GUARDIAN_HALT);
        assert_eq!(priority(halt(), &guardian(), 0), GUARDIAN_HALT);

        // Pre-dispatch checks stay the runtime's own, open phase or not.
        assert_ok!(UnsignedBelowGuardianHalt::pre_dispatch(&call));
        election::CurrentPhase::<Runtime>::put(election::Phase::Off);
        let closed = UnsignedBelowGuardianHalt::pre_dispatch(&call);
        assert!(closed.is_err());
        assert_eq!(closed, <Runtime as ValidateUnsigned>::pre_dispatch(&call));
    });
}
