// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use codec::Decode;
use frame_support::dispatch::GetDispatchInfo;
use sp_runtime::traits::Header as HeaderT;

/// Construct the real four report calls and test the weight admission gate that
/// both signed and unsigned transactions pass through. Proof contents need not
/// pass cryptographic validation here: that later validation does not affect the
/// call's declared weight. A small proof therefore isolates a weight
/// rejection rather than block-length exhaustion or bad-proof dispatch errors.
fn assert_report_admission(session: u32, validator_count: u32, expected: bool) {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();

    let membership = sp_session::MembershipProof {
        session,
        trie_nodes: vec![],
        validator_count,
    };
    let first_header = crate::Header::new(
        1,
        sp_core::H256::repeat_byte(1),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    let second_header = crate::Header::new(
        1,
        sp_core::H256::repeat_byte(2),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    let babe_proof = sp_consensus_babe::EquivocationProof {
        offender: sp_core::sr25519::Pair::from_seed(&[1; 32]).public().into(),
        slot: 42u64.into(),
        first_header,
        second_header,
    };

    // GRANDPA keeps its proof fields private, and its finality-grandpa
    // constructor types are not runtime dependencies. Decode the documented
    // SCALE structure: set id, Prevote variant, round, authority, two signed
    // prevotes (target hash, target number, signature).
    let authority = sp_core::ed25519::Pair::from_seed(&[1; 32]).public();
    let signature = sp_core::ed25519::Signature::from_raw([0; 64]);
    let encoded_grandpa = (
        1u64,
        0u8,
        42u64,
        authority,
        ((sp_core::H256::repeat_byte(1), 1u32), signature.clone()),
        ((sp_core::H256::repeat_byte(2), 1u32), signature),
    )
        .encode();
    let grandpa_proof =
        crate::fg_primitives::EquivocationProof::<sp_core::H256, crate::BlockNumber>::decode(
            &mut encoded_grandpa.as_slice(),
        )
        .expect("the GRANDPA proof fixture has the real SCALE layout");

    let calls = vec![
        (
            "BABE signed",
            crate::RuntimeCall::Babe(pallet_babe::Call::report_equivocation {
                equivocation_proof: Box::new(babe_proof.clone()),
                key_owner_proof: membership.clone(),
            }),
        ),
        (
            "BABE unsigned",
            crate::RuntimeCall::Babe(pallet_babe::Call::report_equivocation_unsigned {
                equivocation_proof: Box::new(babe_proof),
                key_owner_proof: membership.clone(),
            }),
        ),
        (
            "GRANDPA signed",
            crate::RuntimeCall::Grandpa(pallet_grandpa::Call::report_equivocation {
                equivocation_proof: Box::new(grandpa_proof.clone()),
                key_owner_proof: membership.clone(),
            }),
        ),
        (
            "GRANDPA unsigned",
            crate::RuntimeCall::Grandpa(pallet_grandpa::Call::report_equivocation_unsigned {
                equivocation_proof: Box::new(grandpa_proof),
                key_owner_proof: membership,
            }),
        ),
    ];
    let results = calls
        .into_iter()
        .map(|(label, call)| {
            let info = call.get_dispatch_info();
            let limit = crate::BlockWeights::get().get(info.class).max_extrinsic;
            let admission =
                frame_system::CheckWeight::<Runtime>::do_validate(&info, call.encoded_size());
            (label, info.class, info.total_weight(), limit, admission)
        })
        .collect::<Vec<_>>();

    assert!(results.iter().all(|(_, _, _, _, admission)| {
            if expected {
                admission.is_ok()
            } else {
                matches!(admission, Err(error) if *error == InvalidTransaction::ExhaustsResources.into())
            }
        }), "expected report admission={expected}: {results:#?}");
}

#[test]
fn repro_signed_and_unsigned_equivocation_reports_fit_runtime_admission() {
    ext().execute_with(|| {
        let _validators = Validators::new();
        let work = crate::liveness::ExposureWork::<Runtime>::get()
            .expect("the fixture's real session hook refreshes the complete exposure bound");
        assert_eq!(work.max_nomination_edges, VALIDATORS);
        assert_eq!(work.max_validators, VALIDATORS);
        assert_eq!(crate::MaxNominators::get(), 12_500);
        assert_report_admission(FIRST_SESSION, VALIDATORS, true);
    });
}

// Copied counts from the read-only finalized mainnet snapshot at block 27,623,828
// (0xbc0793b9e85d4eec012edf4647c1d167712f88d8564f58aaedc2a35cffaad127).
// Source: misc/slash-investigation/exposure-work-snapshot.json. These are workload
// summaries, not a mainnet state replay: accounts and balances below are synthetic.
// Each row is (era, start session, validators, total nomination rows, largest exposure).
const CAPTURED_ERAS: &[(u32, u32, u32, u32, u32)] = &[
    (7609, 46958, 28, 113, 30),
    (7610, 46964, 28, 113, 30),
    (7611, 46970, 28, 112, 26),
    (7612, 46976, 28, 111, 28),
    (7613, 46982, 28, 111, 18),
    (7614, 46988, 28, 111, 18),
    (7615, 46994, 28, 111, 26),
    (7616, 47000, 28, 112, 22),
    (7617, 47006, 25, 107, 19),
    (7618, 47012, 25, 107, 27),
    (7619, 47018, 25, 107, 28),
    (7620, 47024, 25, 107, 21),
    (7621, 47030, 25, 107, 20),
    (7622, 47036, 25, 107, 18),
    (7623, 47042, 25, 107, 29),
    (7624, 47048, 25, 107, 19),
    (7625, 47054, 25, 107, 28),
    (7626, 47060, 25, 107, 26),
    (7627, 47066, 25, 107, 20),
    (7628, 47072, 25, 107, 20),
    (7629, 47078, 25, 107, 21),
    (7630, 47084, 25, 107, 20),
    (7631, 47090, 25, 107, 20),
    (7632, 47096, 25, 106, 29),
    (7633, 47102, 25, 106, 28),
    (7634, 47108, 25, 106, 21),
    (7635, 47114, 25, 106, 26),
    (7636, 47120, 25, 106, 26),
    (7637, 47126, 25, 106, 27),
];

fn nomination_rows(count: u32) -> Vec<IndividualExposure<AccountId, Balance>> {
    (0..count)
        .map(|index| {
            let mut account = [0; 32];
            account[..4].copy_from_slice(&(index + 1).to_le_bytes());
            IndividualExposure {
                who: AccountId::from(account),
                value: NOMINATED_STAKE,
            }
        })
        .collect()
}

#[test]
fn captured_mainnet_historical_work_admits_all_four_equivocation_calls() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let mut overview_count = 0;
        for &(era, start_session, validator_count, total_rows, largest) in CAPTURED_ERAS {
            pallet_staking::ErasStartSessionIndex::<Runtime>::insert(era, start_session);
            for index in 0..validator_count {
                // Preserve both the era's aggregate workload and the largest
                // individual exposure; repeated nominators across validators
                // remain separate rows, exactly as slashing processes them.
                let remainder = total_rows - largest;
                let others = if index == 0 {
                    largest
                } else {
                    remainder / (validator_count - 1)
                        + u32::from(index - 1 < remainder % (validator_count - 1))
                };
                let account = AccountId::from([index as u8 + 150; 32]);
                EraInfo::<Runtime>::set_exposure(
                    era,
                    &account,
                    Exposure {
                        total: OWN_STAKE + Balance::from(others) * NOMINATED_STAKE,
                        own: OWN_STAKE,
                        others: nomination_rows(others),
                    },
                );
                overview_count += 1;
            }
            assert_eq!(
                pallet_staking::ErasStakersOverview::<Runtime>::iter_prefix(era)
                    .map(|(_, overview)| overview.nominator_count)
                    .sum::<u32>(),
                total_rows,
            );
        }
        assert_eq!(CAPTURED_ERAS.len(), 29);
        assert_eq!(overview_count, 749);
        pallet_staking::ActiveEra::<Runtime>::put(ActiveEraInfo {
            index: 7637,
            start: Some(0),
        });
        pallet_staking::CurrentEra::<Runtime>::put(7637);
        pallet_staking::BondedEras::<Runtime>::put(
            CAPTURED_ERAS
                .iter()
                .map(|&(era, session, _, _, _)| (era, session))
                .collect::<Vec<_>>(),
        );
        validators.start_session(47126);
        let work = crate::liveness::ExposureWork::<Runtime>::get()
            .expect("the complete captured workload fits the bounded metadata scan");
        assert_eq!(work.max_nomination_edges, 113);
        assert_eq!(work.max_validators, 28);
        assert_eq!(crate::MaxNominators::get(), 12_500);

        // Both the historical 28-validator set and current 25-validator set fit.
        // Membership proof fixtures remain admission-only, as described above.
        assert_report_admission(46958, 28, true);
        assert_report_admission(47126, 25, true);
    });
}

#[test]
fn missing_or_stale_exposure_bound_keeps_reports_above_admission_limit() {
    ext().execute_with(|| {
        let _validators = Validators::new();
        assert_report_admission(FIRST_SESSION, VALIDATORS, true);
        pallet_session::CurrentIndex::<Runtime>::put(FIRST_SESSION + 1);
        assert_report_admission(FIRST_SESSION, VALIDATORS, false);

        pallet_session::CurrentIndex::<Runtime>::put(FIRST_SESSION);
        crate::liveness::ExposureWork::<Runtime>::kill();
        assert_report_admission(FIRST_SESSION, VALIDATORS, false);
    });
}

#[test]
fn real_large_exposure_work_is_not_undercharged_to_force_admission() {
    ext().execute_with(|| {
        let validators = Validators::new();
        let rows = crate::MaxNominators::get();
        EraInfo::<Runtime>::set_exposure(
            ERA,
            &validators.accounts[0],
            Exposure {
                total: OWN_STAKE + Balance::from(rows) * NOMINATED_STAKE,
                own: OWN_STAKE,
                others: nomination_rows(rows),
            },
        );
        validators.start_session(FIRST_SESSION);
        let work = crate::liveness::ExposureWork::<Runtime>::get()
            .expect("large exposures have bounded metadata even when their work exceeds a block");
        assert_eq!(work.max_nomination_edges, rows + VALIDATORS - 1);
        assert_eq!(
            Staking::eras_stakers(ERA, &validators.accounts[0])
                .others
                .len(),
            rows as usize
        );
        // Truthful admission must reject this synchronous workload until
        // financial slashing supports resumable processing across blocks.
        assert_report_admission(FIRST_SESSION, VALIDATORS, false);
    });
}

/// Supply the elected identities which the shared fixture installs directly.
/// The SDK's actual historical session manager builds the trie root; this test
/// manager substitutes only for election machinery, never proof verification.
struct FixtureHistoricalValidators;

impl pallet_session::SessionManager<AccountId> for FixtureHistoricalValidators {
    fn new_session(_: u32) -> Option<Vec<AccountId>> {
        Some(Session::validators())
    }

    fn start_session(_: u32) {}
    fn end_session(_: u32) {}
}

impl pallet_session::historical::SessionManager<AccountId, Exposure<AccountId, Balance>>
    for FixtureHistoricalValidators
{
    fn new_session(_: u32) -> Option<Vec<Identification>> {
        Some(
            Session::validators()
                .into_iter()
                .map(|account| (account, Exposure::default()))
                .collect(),
        )
    }

    fn start_session(_: u32) {}
    fn end_session(_: u32) {}
}

#[test]
fn valid_signed_babe_reports_pass_admission_and_dispatch_in_a_batch() {
    use frame_support::traits::{KeyOwnerProofSystem, OnFinalize, OnInitialize};
    use sp_consensus_babe::digests::{PreDigest, SecondaryPlainPreDigest, SecondaryVRFPreDigest};
    use sp_core::crypto::{VrfPublic, VrfSecret};
    use sp_runtime::{generic::Digest, traits::Dispatchable, DigestItem};

    ext().execute_with(|| {
        let validators = Validators::new();
        let keys = validators
            .accounts
            .iter()
            .enumerate()
            .map(|(index, account)| {
                let seed = [index as u8 + 1; 32];
                let keys = crate::opaque::SessionKeys {
                    babe: sp_core::sr25519::Pair::from_seed(&seed).public().into(),
                    grandpa: sp_core::ed25519::Pair::from_seed(&seed).public().into(),
                    im_online: validators.keys[index].clone(),
                    beefy: sp_core::ecdsa::Pair::from_seed(&seed).public().into(),
                };
                pallet_session::NextKeys::<Runtime>::insert(account, &keys);
                (account.clone(), keys)
            })
            .collect::<Vec<_>>();
        pallet_session::QueuedKeys::<Runtime>::put(&keys);
        pallet_session::QueuedChanged::<Runtime>::put(false);
        let authorities = frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::force_from(
            keys.iter()
                .map(|(_, keys)| (keys.babe.clone(), 1))
                .collect(),
            None,
        );
        pallet_babe::Authorities::<Runtime>::put(&authorities);
        pallet_babe::NextAuthorities::<Runtime>::put(&authorities);

        let offence_session = FIRST_SESSION + 1;
        type SeedHistoricalRoot =
            pallet_session::historical::NoteHistoricalRoot<Runtime, FixtureHistoricalValidators>;
        assert!(
            <SeedHistoricalRoot as pallet_session::SessionManager<AccountId>>::new_session(
                offence_session,
            )
            .is_some()
        );
        assert!(
            pallet_session::historical::HistoricalSessions::<Runtime>::contains_key(
                offence_session,
            )
        );
        pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
        pallet_babe::EpochIndex::<Runtime>::put(u64::from(FIRST_SESSION));

        let rotate = |block: u32, session: u32| {
            for index in 0..validators.accounts.len() {
                validators.note_author(index);
            }
            let slot = 1 + u64::from(session) * crate::EpochDuration::get();
            pallet_babe::CurrentSlot::<Runtime>::put(sp_consensus_babe::Slot::from(slot - 1));
            pallet_babe::Initialized::<Runtime>::kill();
            <crate::Authorship as OnFinalize<crate::BlockNumber>>::on_finalize(block - 1);
            let digest = Digest {
                logs: vec![DigestItem::PreRuntime(
                    sp_consensus_babe::BABE_ENGINE_ID,
                    PreDigest::SecondaryPlain(SecondaryPlainPreDigest {
                        authority_index: VALIDATORS - 1,
                        slot: slot.into(),
                    })
                    .encode(),
                )],
            };
            System::initialize(&block, &Default::default(), &digest);
            <crate::AllPalletsWithSystem as OnInitialize<crate::BlockNumber>>::on_initialize(block);
            assert_eq!(Session::current_index(), session);
            assert_eq!(crate::Babe::epoch_index(), u64::from(session));
        };
        rotate(2, offence_session);
        let randomness = pallet_babe::Randomness::<Runtime>::get();
        let first_slot = 1 + u64::from(offence_session) * crate::EpochDuration::get();
        let mut calls = vec![];
        for index in 0..2u32 {
            let pair = sp_core::sr25519::Pair::from_seed(&[index as u8 + 1; 32]);
            let offender: pallet_babe::AuthorityId = pair.public().into();
            let membership = <crate::Historical as KeyOwnerProofSystem<_>>::prove((
                sp_consensus_babe::KEY_TYPE,
                offender.clone(),
            ))
            .expect("Historical generates a real membership proof for installed session keys");
            assert_eq!(membership.session, offence_session);
            assert_eq!(membership.validator_count, VALIDATORS);

            // Match BABE's deterministic secondary-slot assignment, then use a
            // genuine VRF and seal signatures for two distinct headers. These
            // are real evidence proofs, unlike the admission-only fixtures above.
            let slot = (first_slot..first_slot + crate::EpochDuration::get())
                .find(|slot| {
                    let digest = (randomness, sp_consensus_babe::Slot::from(*slot))
                        .using_encoded(sp_core::blake2_256);
                    (sp_core::U256::from_big_endian(&digest) % sp_core::U256::from(VALIDATORS))
                        .as_u32()
                        == index
                })
                .expect("the epoch contains a secondary slot for each seeded test authority");
            let sign_data = sp_consensus_babe::make_vrf_sign_data(
                &randomness,
                slot.into(),
                u64::from(offence_session),
            );
            let vrf_signature = pair.vrf_sign(&sign_data);
            assert!(pair.public().vrf_verify(&sign_data, &vrf_signature));
            let make_header = |marker: u8| {
                let mut header = crate::Header::new(
                    2,
                    sp_core::H256::repeat_byte(marker),
                    Default::default(),
                    Default::default(),
                    Digest {
                        logs: vec![DigestItem::PreRuntime(
                            sp_consensus_babe::BABE_ENGINE_ID,
                            PreDigest::SecondaryVRF(SecondaryVRFPreDigest {
                                authority_index: index,
                                slot: slot.into(),
                                vrf_signature: vrf_signature.clone(),
                            })
                            .encode(),
                        )],
                    },
                );
                let signature = pair.sign(header.hash().as_ref()).encode();
                header.digest_mut().push(DigestItem::Seal(
                    sp_consensus_babe::BABE_ENGINE_ID,
                    signature,
                ));
                header
            };
            let proof = sp_consensus_babe::EquivocationProof {
                offender,
                slot: slot.into(),
                first_header: make_header(1),
                second_header: make_header(2),
            };
            assert!(sp_consensus_babe::check_equivocation_proof(proof.clone()));
            calls.push(crate::RuntimeCall::Babe(
                pallet_babe::Call::report_equivocation {
                    equivocation_proof: Box::new(proof),
                    key_owner_proof: membership,
                },
            ));
        }

        // Rotate again: dispatch must verify the archived trie root, not the
        // current-session shortcut in Historical::check_proof.
        <crate::Babe as OnFinalize<crate::BlockNumber>>::on_finalize(2);
        rotate(3, offence_session + 1);
        assert!(
            pallet_session::historical::HistoricalSessions::<Runtime>::contains_key(
                offence_session,
            )
        );
        let batch = crate::RuntimeCall::Utility(pallet_utility::Call::batch_all { calls });
        let info = batch.get_dispatch_info();
        let len = batch.encoded_size();
        // Isolate transaction capacity from the session-ending hook's reserved
        // mandatory weight, while retaining the actual consensus/session state.
        frame_system::BlockSize::<Runtime>::kill();
        frame_system::BlockWeight::<Runtime>::kill();
        let (_, next_len) = frame_system::CheckWeight::<Runtime>::do_validate(&info, len)
            .expect("both valid reports fit the real outer batch admission gate");
        assert_ok!(frame_system::CheckWeight::<Runtime>::do_prepare(
            &info, len, next_len
        ));
        assert_ok!(batch.dispatch(RuntimeOrigin::signed(validators.nominator.clone())));
        assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 2);
        assert_eq!(Session::disabled_validators(), vec![0, 1]);
        assert_eq!(queued_slash_count(), 2);
        for index in 0..2 {
            let (fraction, amount) = pallet_staking::ValidatorSlashInEra::<Runtime>::get(
                ERA,
                &validators.accounts[index],
            )
            .expect("each cryptographically verified report reached financial slashing");
            assert_eq!(fraction, Perbill::from_rational(3u32, VALIDATORS).square());
            assert_eq!(amount, fraction * OWN_STAKE);
        }
        <crate::Babe as OnFinalize<crate::BlockNumber>>::on_finalize(3);
    });
}
