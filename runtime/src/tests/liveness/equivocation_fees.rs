// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

#![allow(deprecated)]

use super::*;
use crate::{Executive, RuntimeCall, Signature, SignedExtra, UncheckedExtrinsic, XorFee};
use codec::Decode;
use frame_support::{
    assert_err,
    dispatch::{GetDispatchInfo, Pays},
    traits::KeyOwnerProofSystem,
};
use sp_consensus_babe::digests::{PreDigest, SecondaryVRFPreDigest};
use sp_core::{crypto::VrfSecret, ed25519, sr25519, H256};
use sp_runtime::{
    generic::{Digest, Era},
    traits::{Dispatchable, Header as HeaderT},
    DigestItem, DispatchError, DispatchResult, FixedPointNumber, FixedU128,
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Consensus {
    Babe,
    Grandpa,
}

const CONSENSUS: [Consensus; 2] = [Consensus::Babe, Consensus::Grandpa];

/// Real validator keys and staking exposures make the reports valid at both
/// cryptographic verification and offence processing, including unsigned admission.
pub(super) fn report_fixture() -> Validators {
    let validators = Validators::new();
    for (index, account) in validators.accounts.iter().enumerate() {
        let seed = [index as u8 + 1; 32];
        pallet_session::NextKeys::<Runtime>::insert(
            account,
            crate::opaque::SessionKeys {
                babe: sr25519::Pair::from_seed(&seed).public().into(),
                grandpa: ed25519::Pair::from_seed(&seed).public().into(),
                im_online: validators.keys[index].clone(),
                beefy: sp_core::ecdsa::Pair::from_seed(&seed).public().into(),
            },
        );
    }
    pallet_babe::GenesisSlot::<Runtime>::put(sp_consensus_babe::Slot::from(1));
    pallet_babe::EpochIndex::<Runtime>::put(u64::from(FIRST_SESSION));
    pallet_babe::SkippedEpochs::<Runtime>::kill();
    pallet_grandpa::CurrentSetId::<Runtime>::put(0);
    pallet_grandpa::SetIdSession::<Runtime>::insert(0, FIRST_SESSION);
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
    validators
}

pub(super) fn report_call(
    consensus: Consensus,
    legacy_unsigned_name: bool,
    index: u32,
    valid: bool,
) -> RuntimeCall {
    let seed = [index as u8 + 1; 32];
    match consensus {
        Consensus::Babe => {
            let pair = sr25519::Pair::from_seed(&seed);
            let offender: pallet_babe::AuthorityId = pair.public().into();
            let membership = <crate::Historical as KeyOwnerProofSystem<_>>::prove((
                sp_consensus_babe::KEY_TYPE,
                offender.clone(),
            ))
            .expect("the seeded validator has a real membership proof");
            let randomness = pallet_babe::Randomness::<Runtime>::get();
            let first_slot = 1 + u64::from(FIRST_SESSION) * crate::EpochDuration::get();
            let slot = (first_slot..first_slot + crate::EpochDuration::get())
                .find(|slot| {
                    let digest = (randomness, sp_consensus_babe::Slot::from(*slot))
                        .using_encoded(sp_core::blake2_256);
                    (sp_core::U256::from_big_endian(&digest) % sp_core::U256::from(VALIDATORS))
                        .as_u32()
                        == index
                })
                .expect("the epoch contains a secondary slot for the seeded authority");
            let vrf_signature = pair.vrf_sign(&sp_consensus_babe::make_vrf_sign_data(
                &randomness,
                slot.into(),
                u64::from(FIRST_SESSION),
            ));
            let make_header = |marker: u8| {
                let mut header = crate::Header::new(
                    2,
                    H256::repeat_byte(marker),
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
            let first_header = make_header(1);
            let second_header = if valid {
                make_header(2)
            } else {
                first_header.clone()
            };
            let proof = sp_consensus_babe::EquivocationProof {
                offender,
                slot: slot.into(),
                first_header,
                second_header,
            };
            assert_eq!(
                sp_consensus_babe::check_equivocation_proof(proof.clone()),
                valid
            );
            RuntimeCall::Babe(if legacy_unsigned_name {
                pallet_babe::Call::report_equivocation_unsigned {
                    equivocation_proof: Box::new(proof),
                    key_owner_proof: membership,
                }
            } else {
                pallet_babe::Call::report_equivocation {
                    equivocation_proof: Box::new(proof),
                    key_owner_proof: membership,
                }
            })
        }
        Consensus::Grandpa => {
            let pair = ed25519::Pair::from_seed(&seed);
            let offender: pallet_grandpa::AuthorityId = pair.public().into();
            let membership = <crate::Historical as KeyOwnerProofSystem<_>>::prove((
                crate::fg_primitives::KEY_TYPE,
                offender,
            ))
            .expect("the seeded validator has a real membership proof");
            let set_id = 0u64;
            let round = 42u64;
            let signed_prevote = |marker: u8| {
                let prevote = (H256::repeat_byte(marker), 2u32);
                // SCALE Message::Prevote is variant 0 followed by the prevote.
                let payload =
                    crate::fg_primitives::localized_payload(round, set_id, &(0u8, prevote));
                (prevote, pair.sign(&payload))
            };
            // EquivocationProof's fields are private. Decode its SCALE layout
            // with real localized vote signatures, as in the admission fixture.
            let encoded = (
                set_id,
                0u8,
                round,
                pair.public(),
                signed_prevote(1),
                signed_prevote(if valid { 2 } else { 1 }),
            )
                .encode();
            let proof =
                crate::fg_primitives::EquivocationProof::<H256, crate::BlockNumber>::decode(
                    &mut encoded.as_slice(),
                )
                .expect("the signed prevotes use GRANDPA's real SCALE layout");
            assert_eq!(
                crate::fg_primitives::check_equivocation_proof(proof.clone()),
                valid
            );
            RuntimeCall::Grandpa(if legacy_unsigned_name {
                pallet_grandpa::Call::report_equivocation_unsigned {
                    equivocation_proof: Box::new(proof),
                    key_owner_proof: membership,
                }
            } else {
                pallet_grandpa::Call::report_equivocation {
                    equivocation_proof: Box::new(proof),
                    key_owner_proof: membership,
                }
            })
        }
    }
}

fn reporter_pair() -> sr25519::Pair {
    sr25519::Pair::from_seed(&[77; 32])
}

fn reporter_account(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public())
}

fn signed_extrinsic(call: RuntimeCall, pair: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = reporter_account(pair);
    let extra: SignedExtra = (
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

fn reset_admission() {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
}

/// Execute the signed wire extrinsic through its real payment extension and
/// assert the charged event equals the payer's balance decrease.
fn apply_paid(call: RuntimeCall, pair: &sr25519::Pair) -> DispatchResult {
    reset_admission();
    assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
    let extrinsic = signed_extrinsic(call, pair);
    let quoted_fee = XorFee::query_info(
        &extrinsic,
        &extrinsic.function,
        extrinsic.encoded_size() as u32,
    )
    .partial_fee;
    assert!(quoted_fee > 0);
    let who = reporter_account(pair);
    let before = Balances::free_balance(&who);
    System::reset_events();
    let result = Executive::apply_extrinsic(extrinsic)
        .expect("a funded report passes all signed transaction extensions");
    let charged = before - Balances::free_balance(&who);
    assert!(charged > 0);
    assert!(charged <= quoted_fee);
    let paid = System::events()
        .into_iter()
        .find_map(|event| match event.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who: payer,
                    actual_fee,
                    tip,
                },
            ) if payer == who => {
                assert_eq!(tip, 0);
                Some(actual_fee)
            }
            _ => None,
        })
        .expect("the payment extension emits the actual charged fee");
    assert_eq!(paid, charged);
    result
}

#[test]
fn valid_equivocation_reports_reject_every_unsigned_admission_path() {
    for consensus in CONSENSUS {
        for legacy_unsigned_name in [false, true] {
            ext().execute_with(|| {
                let _validators = report_fixture();
                let call = report_call(consensus, legacy_unsigned_name, 0, true);
                for source in [
                    TransactionSource::External,
                    TransactionSource::Local,
                    TransactionSource::InBlock,
                ] {
                    reset_admission();
                    assert_err!(
                        <Runtime as ValidateUnsigned>::validate_unsigned(source, &call),
                        InvalidTransaction::Call,
                    );
                    assert_err!(
                        Executive::validate_transaction(
                            source,
                            UncheckedExtrinsic::new_bare(call.clone()),
                            Default::default(),
                        ),
                        InvalidTransaction::Call,
                    );
                }
                assert_err!(
                    <Runtime as ValidateUnsigned>::pre_dispatch(&call),
                    InvalidTransaction::Call,
                );
                reset_admission();
                assert_err!(
                    Executive::apply_extrinsic(UncheckedExtrinsic::new_bare(call.clone())),
                    InvalidTransaction::Call,
                );
                assert_err!(
                    call.clone().dispatch(RuntimeOrigin::none()),
                    DispatchError::BadOrigin
                );
                assert_err!(
                    call.dispatch(RuntimeOrigin::root()),
                    DispatchError::BadOrigin
                );
                assert_no_offence_or_slash();
            });
        }
    }
}

#[test]
fn signed_equivocation_report_calls_require_a_funded_fee_payer() {
    for consensus in CONSENSUS {
        for legacy_unsigned_name in [false, true] {
            ext().execute_with(|| {
                let _validators = report_fixture();
                let call = report_call(consensus, legacy_unsigned_name, 0, true);
                let pair = reporter_pair();
                assert_eq!(Balances::free_balance(reporter_account(&pair)), 0);
                assert_err!(
                    Executive::validate_transaction(
                        TransactionSource::External,
                        signed_extrinsic(call.clone(), &pair),
                        Default::default(),
                    ),
                    InvalidTransaction::Payment,
                );
                reset_admission();
                assert_err!(
                    Executive::apply_extrinsic(signed_extrinsic(call, &pair)),
                    InvalidTransaction::Payment,
                );
                assert_no_offence_or_slash();
            });
        }
    }
}

#[test]
fn invalid_signed_equivocation_reports_still_pay_execution_fees() {
    for consensus in CONSENSUS {
        for legacy_unsigned_name in [false, true] {
            ext().execute_with(|| {
                let _validators = report_fixture();
                let pair = reporter_pair();
                let _ = Balances::make_free_balance_be(&reporter_account(&pair), 100_000 * UNIT);
                let result = apply_paid(
                    report_call(consensus, legacy_unsigned_name, 0, false),
                    &pair,
                );
                let expected: DispatchError = match consensus {
                    Consensus::Babe => {
                        pallet_babe::Error::<Runtime>::InvalidEquivocationProof.into()
                    }
                    Consensus::Grandpa => {
                        pallet_grandpa::Error::<Runtime>::InvalidEquivocationProof.into()
                    }
                };
                assert_err!(result, expected);
                assert_no_offence_or_slash();
            });
        }
    }
}

#[test]
fn valid_and_duplicate_signed_equivocation_reports_pay_fees_and_preserve_slashing() {
    for consensus in CONSENSUS {
        for legacy_unsigned_name in [false, true] {
            ext().execute_with(|| {
                let validators = report_fixture();
                let pair = reporter_pair();
                let _ = Balances::make_free_balance_be(&reporter_account(&pair), 100_000 * UNIT);
                let call = report_call(consensus, legacy_unsigned_name, 0, true);
                assert_ok!(apply_paid(call.clone(), &pair));
                assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
                assert_eq!(Session::disabled_validators(), vec![0]);
                let (fraction, amount) = pallet_staking::ValidatorSlashInEra::<Runtime>::get(
                    ERA,
                    &validators.accounts[0],
                )
                .expect("the paid report reached actual staking slash processing");
                assert_eq!(fraction, Perbill::from_rational(3u32, VALIDATORS).square());
                assert_eq!(amount, fraction * OWN_STAKE);
                #[cfg(not(feature = "private-net"))]
                assert_eq!(queued_slash_count(), 1);
                let expected: DispatchError = match consensus {
                    Consensus::Babe => pallet_babe::Error::<Runtime>::DuplicateOffenceReport.into(),
                    Consensus::Grandpa => {
                        pallet_grandpa::Error::<Runtime>::DuplicateOffenceReport.into()
                    }
                };
                assert_err!(apply_paid(call, &pair), expected);
                assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
            });
        }
    }
}

#[test]
fn nested_utility_equivocation_reports_keep_nonzero_fees() {
    for kind in 0..4 {
        ext().execute_with(|| {
            let _validators = report_fixture();
            let pair = reporter_pair();
            let _ = Balances::make_free_balance_be(&reporter_account(&pair), 100_000 * UNIT);
            let calls = vec![
                report_call(Consensus::Babe, true, 0, true),
                report_call(Consensus::Grandpa, true, 1, true),
            ];
            let batch = match kind {
                0 => pallet_utility::Call::batch { calls },
                1 => pallet_utility::Call::batch_all { calls },
                _ => pallet_utility::Call::force_batch { calls },
            };
            let call = if kind == 3 {
                RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                    index: 0,
                    call: Box::new(RuntimeCall::Utility(batch)),
                })
            } else {
                RuntimeCall::Utility(batch)
            };
            assert_ok!(apply_paid(call, &pair));
            assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 2);
            assert_eq!(Session::disabled_validators(), vec![0, 1]);
        });
    }
}
