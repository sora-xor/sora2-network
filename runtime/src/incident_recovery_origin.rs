//! Permanent retirement at every generated RuntimeCall dispatch boundary.
//!
//! The optional construct_runtime hook runs for ordinary and unfiltered dispatch,
//! including derivative/multisig origins and calls restored by the scheduler.
//! It neither remaps a caller nor expands Root, None or collective privileges.

use crate::RuntimeOrigin;
use frame_support::{dispatch::DispatchResult, traits::OriginTrait, weights::Weight};

pub fn check(origin: &RuntimeOrigin) -> DispatchResult {
    if let Some(account) = origin.as_signer() {
        if crate::incident_recovery::account_retired(account) {
            return Err(sp_runtime::DispatchError::Other("Retired recovery account"));
        }
    }
    Ok(())
}

/// Six raw-policy reads cover declared/dispatch weight probes and fail-closed map admission;
/// at most twelve static-map comparisons cover the maximum 4,096 replacements.
/// No writes are performed. The proof/CPU allowance is deliberately conservative.
/// The charge is state-independent: dispatch information is also queried before
/// recovery admission and outside externalities. The historical spec-134 runtime
/// does not include this hook and retains its deployed weights.
pub fn weight() -> Weight {
    <crate::Runtime as frame_system::Config>::DbWeight::get()
        .reads(6)
        .saturating_add(Weight::from_parts(2_000_000, 16_384))
}

#[cfg(test)]
mod post_complete_bridge_tests {
    use super::*;
    use crate::{AccountId, MultiProof, Runtime, RuntimeCall, System};
    use codec::Encode;
    use common::recovery::{self, AccountReplacement, RecoveryPhase, RecoveryPolicy};
    use sp_runtime::traits::Dispatchable;

    #[test]
    fn recovery_post_complete_retired_signer_cannot_submit_bridge_messages() {
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::from([21; 32]);
            let policy = RecoveryPolicy {
                format_version: 2,
                authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
                checkpoint_hash: recovery::CHECKPOINT_HASH,
                checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
                replacements: vec![AccountReplacement {
                    old: old.clone(),
                    new: AccountId::from([22; 32]),
                }],
                council: vec![AccountId::from([23; 32])],
                chunk_hashes: vec![[1; 32]],
                audit_chunk_hashes: vec![[2; 32]],
                evidence_commitment: [3; 32],
                terminal_audit_commitment: [4; 32],
            };
            crate::incident_recovery::with_test_policy(&policy, || {
                System::set_block_number(recovery::CHECKPOINT_NUMBER + 2);
                let mut status = recovery::storage::initialize(&policy).unwrap();
                status.phase = RecoveryPhase::Complete;
                status.next_chunk = 1;
                sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
                sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
                let call = RuntimeCall::SubstrateBridgeInboundChannel(
                    substrate_bridge_channel::inbound::Call::submit {
                        network_id: bridge_types::SubNetworkId::Liberland,
                        commitment: bridge_types::GenericCommitment::Sub(
                            bridge_types::substrate::Commitment {
                                nonce: 1,
                                messages: Default::default(),
                            },
                        ),
                        proof: MultiProof::Multisig(multisig_verifier::Proof {
                            digest: bridge_types::types::AuxiliaryDigest { logs: vec![] },
                            proof: vec![],
                        }),
                    },
                );
                let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                let error = call.dispatch(RuntimeOrigin::signed(old)).unwrap_err();
                assert_eq!(
                    error.error,
                    sp_runtime::DispatchError::Other("Retired recovery account")
                );
                assert_eq!(root, sp_io::storage::root(sp_runtime::StateVersion::V1));
                assert_eq!(
                    substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(
                        bridge_types::SubNetworkId::Liberland
                    ),
                    0
                );
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountId, Runtime, RuntimeCall, RuntimeEvent, System};
    use codec::Encode;
    use common::recovery::{AccountReplacement, RecoveryPolicy, POLICY_STORAGE_KEY};
    use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};
    use sp_runtime::traits::Dispatchable;

    fn remark() -> RuntimeCall {
        RuntimeCall::System(frame_system::Call::remark_with_event {
            remark: b"retired origin must not reach this event".to_vec(),
        })
    }

    fn with_retired(old: AccountId, run: impl FnOnce()) {
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: common::recovery::CHECKPOINT_HASH,
            checkpoint_state_root: common::recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![AccountReplacement {
                old,
                new: AccountId::from([251; 32]),
            }],
            council: vec![AccountId::from([252; 32])],
            chunk_hashes: vec![[1; 32]],
            audit_chunk_hashes: vec![[2; 32]],
            evidence_commitment: [3; 32],
            terminal_audit_commitment: [4; 32],
        };
        crate::incident_recovery::with_test_policy(&policy, || {
            sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
            System::set_block_number(common::recovery::CHECKPOINT_NUMBER + 2);
            run();
        });
    }

    fn assert_no_remark() {
        assert!(!System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::System(frame_system::Event::Remarked { .. })
        )));
    }

    #[test]
    fn recovery_origin_ordinary_and_bypass_reject_before_state_changes() {
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::from([21; 32]);
            with_retired(old.clone(), || {
                for bypass in [false, true] {
                    let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
                    let result = if bypass {
                        remark().dispatch_bypass_filter(RuntimeOrigin::signed(old.clone()))
                    } else {
                        remark().dispatch(RuntimeOrigin::signed(old.clone()))
                    };
                    let error = result.unwrap_err();
                    assert_eq!(
                        error.error,
                        sp_runtime::DispatchError::Other("Retired recovery account")
                    );
                    assert_eq!(error.post_info.actual_weight, Some(weight()));
                    assert_eq!(root, sp_io::storage::root(sp_runtime::StateVersion::V1));
                }
                assert_no_remark();
            });
        });
    }

    #[test]
    fn recovery_origin_derivative_cannot_reanimate_retired_identity() {
        sp_io::TestExternalities::default().execute_with(|| {
            let controller = AccountId::from([11; 32]);
            let retired = pallet_utility::derivative_account_id(controller.clone(), 7);
            with_retired(retired, || {
                let call = RuntimeCall::Utility(pallet_utility::Call::as_derivative {
                    index: 7,
                    call: Box::new(remark()),
                });
                let declared = call.get_dispatch_info().call_weight;
                let error = call
                    .dispatch(RuntimeOrigin::signed(controller))
                    .unwrap_err();
                assert_eq!(
                    error.error,
                    sp_runtime::DispatchError::Other("Retired recovery account")
                );
                let actual = error.post_info.actual_weight.unwrap();
                assert!(actual.all_gte(weight().saturating_mul(2)));
                assert!(actual.all_lte(declared));
                assert_no_remark();
            });
        });
    }

    #[test]
    fn recovery_origin_multisig_cannot_reanimate_retired_identity() {
        sp_io::TestExternalities::default().execute_with(|| {
            let controller = AccountId::from([11; 32]);
            let other = AccountId::from([12; 32]);
            let retired =
                crate::Multisig::multi_account_id(&[controller.clone(), other.clone()], 1);
            with_retired(retired, || {
                let call = RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
                    other_signatories: vec![other],
                    call: Box::new(remark()),
                });
                let error = call
                    .dispatch(RuntimeOrigin::signed(controller))
                    .unwrap_err();
                assert_eq!(
                    error.error,
                    sp_runtime::DispatchError::Other("Retired recovery account")
                );
                assert_no_remark();
            });
        });
    }

    #[test]
    fn recovery_origin_root_dispatch_as_and_saved_scheduler_origin_are_guarded() {
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::from([21; 32]);
            with_retired(old.clone(), || {
                let call = RuntimeCall::Utility(pallet_utility::Call::dispatch_as_fallible {
                    as_origin: Box::new(frame_system::RawOrigin::Signed(old.clone()).into()),
                    call: Box::new(remark()),
                });
                assert_eq!(
                    call.dispatch_bypass_filter(RuntimeOrigin::root())
                        .unwrap_err()
                        .error,
                    sp_runtime::DispatchError::Other("Retired recovery account")
                );
                // Scheduler restores this caller and dispatches RuntimeCall directly;
                // it does not execute a transaction extension or verify a signature.
                let saved: crate::OriginCaller = frame_system::RawOrigin::Signed(old).into();
                assert_eq!(
                    remark().dispatch(saved.into()).unwrap_err().error,
                    sp_runtime::DispatchError::Other("Retired recovery account")
                );
                assert_no_remark();
            });
        });
    }

    #[test]
    fn recovery_origin_active_and_privileged_origins_keep_native_semantics() {
        sp_io::TestExternalities::default().execute_with(|| {
            with_retired(AccountId::from([21; 32]), || {
                assert!(check(&RuntimeOrigin::root()).is_ok());
                assert!(check(&RuntimeOrigin::none()).is_ok());
                let collective: RuntimeOrigin =
                    crate::OriginCaller::Council(pallet_collective::RawOrigin::Members(2, 3))
                        .into();
                assert!(check(&collective).is_ok());
                assert!(remark()
                    .dispatch(RuntimeOrigin::signed(AccountId::from([22; 32])))
                    .is_ok());
                assert!(System::events().iter().any(|record| matches!(
                    record.event,
                    RuntimeEvent::System(frame_system::Event::Remarked { .. })
                )));
            });
        });
    }

    #[test]
    fn recovery_origin_weights_and_call_encoding_do_not_depend_on_policy_state() {
        let native = frame_system::Call::<Runtime>::remark_with_event {
            remark: vec![1, 2, 3],
        };
        let call = RuntimeCall::System(native.clone());
        // Tooling can ask for dispatch information without a storage context.
        let declared = call.get_dispatch_info();
        let encoded_before = call.encode();
        assert_eq!(
            declared.call_weight,
            native
                .get_dispatch_info()
                .call_weight
                .saturating_add(weight())
        );
        sp_io::TestExternalities::default().execute_with(|| {
            System::set_block_number(common::recovery::CHECKPOINT_NUMBER);
            assert_eq!(call.get_dispatch_info(), declared);
            assert!(call
                .clone()
                .dispatch(RuntimeOrigin::signed(AccountId::from([21; 32])))
                .is_ok());
            with_retired(AccountId::from([21; 32]), || {
                assert_eq!(call.encode(), encoded_before);
                assert_eq!(call.get_dispatch_info(), declared);
                assert_eq!(
                    call.get_dispatch_info().call_weight,
                    native
                        .get_dispatch_info()
                        .call_weight
                        .saturating_add(weight())
                );
            });
        });
    }

    #[test]
    fn recovery_origin_malformed_policy_fails_closed_for_signed_only() {
        sp_io::TestExternalities::default().execute_with(|| {
            sp_io::storage::set(POLICY_STORAGE_KEY, &[0]);
            assert!(check(&RuntimeOrigin::signed(AccountId::from([21; 32]))).is_err());
            assert!(check(&RuntimeOrigin::root()).is_ok());
            assert!(check(&RuntimeOrigin::none()).is_ok());
        });
    }
}
