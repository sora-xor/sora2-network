// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::{
    equivocation_fees::{report_call, report_fixture, Consensus},
    *,
};
use crate::{BridgeMultisig, RuntimeCall};
use codec::Decode;
use frame_support::{dispatch::GetDispatchInfo, traits::Contains};

fn bridge_account_with_signers(signers: Vec<AccountId>) -> AccountId {
    let id =
        crate::EthBridge::bridge_account(0).expect("the runtime fixture has Ethereum bridge 0");
    bridge_multisig::Accounts::<Runtime>::insert(
        &id,
        bridge_multisig::MultisigAccount::new(signers),
    );
    id
}

fn report_wrappers(call: RuntimeCall, id: &AccountId) -> Vec<RuntimeCall> {
    vec![
        call.clone(),
        RuntimeCall::Utility(pallet_utility::Call::batch {
            calls: vec![call.clone()],
        }),
        RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![call.clone()],
        }),
        RuntimeCall::Utility(pallet_utility::Call::force_batch {
            calls: vec![call.clone()],
        }),
        RuntimeCall::Utility(pallet_utility::Call::as_derivative {
            index: 0,
            call: Box::new(call.clone()),
        }),
        RuntimeCall::Multisig(pallet_multisig::Call::as_multi_threshold_1 {
            other_signatories: vec![AccountId::from([88; 32])],
            call: Box::new(call.clone()),
        }),
        RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi_threshold_1 {
            id: id.clone(),
            call: Box::new(call.clone()),
            timepoint: Default::default(),
        }),
        RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
            id: id.clone(),
            maybe_timepoint: None,
            call: call.encode(),
            store_call: true,
            max_weight: call.get_dispatch_info().call_weight,
        }),
        RuntimeCall::Utility(pallet_utility::Call::batch_all {
            calls: vec![RuntimeCall::Multisig(
                pallet_multisig::Call::as_multi_threshold_1 {
                    other_signatories: vec![AccountId::from([88; 32])],
                    call: Box::new(call),
                },
            )],
        }),
    ]
}

#[test]
fn bridge_multisig_cannot_execute_direct_or_wrapped_equivocation_reports_for_free() {
    for consensus in [Consensus::Babe, Consensus::Grandpa] {
        for legacy_unsigned_name in [false, true] {
            ext().execute_with(|| {
                let _validators = report_fixture();
                let signer = AccountId::from([87; 32]);
                let id = bridge_account_with_signers(vec![signer.clone()]);
                let call = report_call(consensus, legacy_unsigned_name, 0, true);
                for wrapped in report_wrappers(call, &id) {
                    assert!(
                        !<Runtime as bridge_multisig::Config>::CallFilter::contains(&wrapped),
                        "the bridge fee exemption must exclude every report wrapper: {wrapped:?}",
                    );
                    let result = BridgeMultisig::as_multi_threshold_1(
                        RuntimeOrigin::signed(signer.clone()),
                        id.clone(),
                        Box::new(wrapped),
                        Default::default(),
                    );
                    assert_eq!(
                        result.unwrap_err().error,
                        frame_system::Error::<Runtime>::CallFiltered.into(),
                    );
                    assert_no_offence_or_slash();
                }
            });
        }
    }
}

#[test]
fn bridge_multisig_filter_preserves_normal_bridge_dispatch() {
    ext().execute_with(|| {
        let _validators = report_fixture();
        let signer = AccountId::from([87; 32]);
        let id = bridge_account_with_signers(vec![signer.clone()]);
        let call = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
            hash: Default::default(),
            network_id: 0,
        });
        assert!(<Runtime as bridge_multisig::Config>::CallFilter::contains(
            &call
        ));
        let result = BridgeMultisig::as_multi_threshold_1(
            RuntimeOrigin::signed(signer.clone()),
            id.clone(),
            Box::new(call),
            Default::default(),
        );
        assert_eq!(
            result.unwrap_err().error,
            eth_bridge::Error::<Runtime>::UnknownRequest.into(),
            "the permitted call reached the bridge's request lookup",
        );

        let remark = RuntimeCall::System(frame_system::Call::remark {
            remark: b"bridge dispatch".to_vec(),
        });
        assert!(<Runtime as bridge_multisig::Config>::CallFilter::contains(
            &remark
        ));
        assert_ok!(BridgeMultisig::as_multi_threshold_1(
            RuntimeOrigin::signed(signer),
            id,
            Box::new(remark),
            Default::default(),
        ));
    });
}

#[test]
fn previously_stored_bridge_report_stays_pending_after_membership_change_and_is_filtered_at_dispatch(
) {
    for consensus in [Consensus::Babe, Consensus::Grandpa] {
        ext().execute_with(|| {
            let _validators = report_fixture();
            let signer = AccountId::from([87; 32]);
            let removed = AccountId::from([88; 32]);
            let id = bridge_account_with_signers(vec![signer.clone(), removed.clone()]);
            let call = RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![report_call(consensus, true, 0, true)],
            });
            let encoded = call.encode();
            let hash = sp_io::hashing::blake2_256(&encoded);
            let timepoint = bridge_multisig::BridgeTimepoint::default();
            bridge_multisig::Calls::<Runtime>::insert(&hash, (encoded, signer.clone(), 0));
            bridge_multisig::Multisigs::<Runtime>::insert(
                &id,
                &hash,
                bridge_multisig::Multisig {
                    when: timepoint,
                    deposit: 0,
                    depositor: signer.clone(),
                    approvals: vec![signer],
                },
            );
            System::reset_events();
            assert_ok!(BridgeMultisig::remove_signatory(
                RuntimeOrigin::signed(id.clone()),
                removed
            ));
            assert!(!System::events().iter().any(|event| matches!(
                &event.event,
                RuntimeEvent::BridgeMultisig(bridge_multisig::Event::MultisigExecuted(..))
            )));
            assert!(bridge_multisig::Multisigs::<Runtime>::contains_key(
                &id, hash
            ));
            let call = bridge_multisig::Calls::<Runtime>::get(hash).unwrap().0;
            let call = RuntimeCall::decode(&mut &call[..]).unwrap();
            let result = BridgeMultisig::as_multi_threshold_1(
                RuntimeOrigin::signed(AccountId::from([87; 32])),
                id,
                Box::new(call),
                timepoint,
            );
            assert_eq!(
                result.unwrap_err().error,
                frame_system::Error::<Runtime>::CallFiltered.into()
            );
            assert_no_offence_or_slash();
        });
    }
}

#[test]
fn bridge_multisig_filter_checks_opaque_calls_stored_hashes_and_nesting_bounds() {
    ext().execute_with(|| {
        let _validators = report_fixture();
        let signer = AccountId::from([87; 32]);
        let id = bridge_account_with_signers(vec![signer.clone()]);
        let remark = RuntimeCall::System(frame_system::Call::remark {
            remark: b"stored bridge call".to_vec(),
        });
        let mut trailing = remark.encode();
        trailing.push(0);
        for opaque in [vec![], vec![255], trailing] {
            let wrapper = RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                id: id.clone(),
                maybe_timepoint: None,
                call: opaque,
                store_call: true,
                max_weight: Default::default(),
            });
            assert!(!<Runtime as bridge_multisig::Config>::CallFilter::contains(
                &wrapper
            ));
            assert_eq!(
                BridgeMultisig::as_multi_threshold_1(
                    RuntimeOrigin::signed(signer.clone()),
                    id.clone(),
                    Box::new(wrapper),
                    Default::default(),
                )
                .unwrap_err()
                .error,
                frame_system::Error::<Runtime>::CallFiltered.into(),
            );
        }

        let hash = [42; 32];
        let approval = RuntimeCall::BridgeMultisig(bridge_multisig::Call::approve_as_multi {
            id,
            maybe_timepoint: None,
            call_hash: hash,
            max_weight: Default::default(),
        });
        bridge_multisig::Calls::<Runtime>::remove(hash);
        assert!(!<Runtime as bridge_multisig::Config>::CallFilter::contains(
            &approval
        ));
        let report = report_call(Consensus::Babe, true, 0, true);
        bridge_multisig::Calls::<Runtime>::insert(hash, (report.encode(), signer.clone(), 0u128));
        assert!(!<Runtime as bridge_multisig::Config>::CallFilter::contains(
            &approval
        ));
        bridge_multisig::Calls::<Runtime>::insert(hash, (remark.encode(), signer, 0u128));
        assert!(<Runtime as bridge_multisig::Config>::CallFilter::contains(
            &approval
        ));

        let mut nested = remark;
        for _ in 0..32 {
            nested = RuntimeCall::Utility(pallet_utility::Call::batch_all {
                calls: vec![nested],
            });
        }
        assert!(!<Runtime as bridge_multisig::Config>::CallFilter::contains(
            &nested
        ));
        assert_no_offence_or_slash();
    });
}
