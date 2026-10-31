// Public-fixture native admission tests for a later, explicitly authorized reopening.
// Only these local tests clear the latch. No production call or recovery context
// is added. Legacy native runs inject their public test policy; production legacy
// runtimes still reject ordinary recovery delivery (see the overlay stub test).

fn delayed_hashi_complete_and_enable(policy: &RecoveryPolicy) {
    recovery::storage::initialize(policy).unwrap();
    let mut status = recovery::storage::status().unwrap().unwrap();
    status.phase = RecoveryPhase::Complete;
    status.next_chunk = policy.chunk_hashes.len() as u32;
    sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
    // Test-only simulation of a later, separately authorized bridge reopening.
    sp_io::storage::clear(BRIDGE_HALT_STORAGE_KEY);
}

fn delayed_hashi_approvals(
    operation: &IncomingOperation,
    keys: &[sr25519::Pair],
    inner: RuntimeCall,
) -> Vec<HashiPeerApproval> {
    let IncomingChannel::Hashi { approvals, .. } = &operation.channel else {
        unreachable!()
    };
    approvals
        .iter()
        .zip(keys)
        .map(|(template, key)| {
            let mut approval = template.clone();
            let mut call = decode_runtime_call(&approval.call).unwrap();
            let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                call: bytes,
                max_weight,
                maybe_timepoint,
                ..
            }) = &mut call
            else {
                unreachable!()
            };
            *bytes = inner.encode();
            *max_weight = inner.get_dispatch_info().total_weight();
            *maybe_timepoint = Some(crate::BridgeMultisig::thischain_timepoint());
            approval.call = call.encode();
            let era =
                sp_runtime::generic::Era::mortal(64, u64::from(recovery::CHECKPOINT_NUMBER + 1));
            approval.extra = (
                era,
                codec::Compact(System::account_nonce(&approval.signer)),
                codec::Compact(0u128),
            )
                .encode();
            sign_hashi(&mut approval, key);
            approval
        })
        .collect()
}

fn delayed_hashi_submit(approval: &HashiPeerApproval) -> Result<(), sp_runtime::DispatchError> {
    use sp_runtime::traits::{Checkable, Dispatchable};
    let error = || sp_runtime::DispatchError::Other("delayed HASHI fixture admission");
    let call = decode_runtime_call(&approval.call).map_err(|_| error())?;
    let (extra, _, nonce) = hashi_extra(&approval.extra).map_err(|_| error())?;
    // Native signed-payload verification, protocol fee/admission and nonce gates.
    // This deliberately does NOT call the committed recovery operation executor.
    let checked = crate::UncheckedExtrinsic::new_signed(
        call,
        approval.signer.clone(),
        approval.signature.clone(),
        extra,
    )
    .check(&frame_system::ChainContext::<Runtime>::default())
    .map_err(|_| error())?;
    let sp_runtime::generic::ExtrinsicFormat::Signed(signer, _) = checked.format else {
        return Err(error());
    };
    if signer != approval.signer
        || !crate::xor_fee_impls::validate_bridge_fee_exemption(&signer, &checked.function)
            .map_err(|_| error())?
    {
        return Err(error());
    }
    frame_system::CheckNonce::<Runtime>::validate_nonce_for_account(&signer, nonce)
        .map_err(|_| error())?;
    frame_system::CheckNonce::<Runtime>::prepare_nonce_for_account(&signer, nonce)
        .map_err(|_| error())?;
    checked
        .function
        .dispatch(RuntimeOrigin::signed(signer))
        .map(|_| ())
        .map_err(|error| error.error)
}

fn delayed_hashi_no_context() {
    for key in [
        ACTIVE_KEY,
        FAILED_KEY,
        BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
        eth_bridge::recovery::HASHI_RESOLUTION_KEY,
    ] {
        assert!(!sp_io::storage::exists(key));
    }
    assert_eq!(
        recovery::storage::status().unwrap().unwrap().phase,
        RecoveryPhase::Complete
    );
}

#[test]
fn delayed_hashi_pending_load_registers_and_finalizes_original_receipt_after_complete_once() {
    use common::AssetInfoProvider;
    for mapped_author in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let old = AccountId::new([7; 32]);
            let new = AccountId::new([8; 32]);
            let innocent = AccountId::new([19; 32]);
            let (author, to) = if mapped_author {
                (old.clone(), innocent.clone())
            } else {
                (innocent.clone(), old.clone())
            };
            let (policy, operation, keys) = hashi_fixture_with_parties(author.clone(), to);
            let (load, index_owner) = checkpoint_pending_hashi_load(&operation, mapped_author);
            let IncomingChannel::Hashi {
                request, network, ..
            } = &operation.channel
            else {
                unreachable!()
            };
            let original: eth_bridge::requests::IncomingRequest<Runtime> =
                decode_bounded(request).unwrap();
            let wrapper = eth_bridge::requests::OffchainRequest::incoming(original.clone());
            let original_bytes = wrapper.encode();
            let register = RuntimeCall::EthBridge(eth_bridge::Call::register_incoming_request {
                incoming_request: original.clone(),
            });
            let finalize = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                hash: wrapper.hash(),
                network_id: *network,
            });
            crate::incident_recovery::with_test_policy(&policy, || {
                delayed_hashi_complete_and_enable(&policy);
                for approval in delayed_hashi_approvals(&operation, &keys, register) {
                    assert_ok!(delayed_hashi_submit(&approval));
                }
                let beneficiary = if mapped_author { &innocent } else { &new };
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, beneficiary).unwrap(),
                    0
                );
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(*network, wrapper.hash()),
                    Some(eth_bridge::requests::RequestStatus::Pending)
                );
                let approvals = delayed_hashi_approvals(&operation, &keys, finalize.clone());
                for approval in &approvals {
                    assert_ok!(delayed_hashi_submit(approval));
                }
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, beneficiary).unwrap(),
                    1_000_000_000_000_000_000
                );
                assert_eq!(crate::Assets::free_balance(&common::DAI, &old).unwrap(), 0);
                assert_eq!(
                    eth_bridge::Requests::<Runtime>::get(*network, wrapper.hash())
                        .unwrap()
                        .encode(),
                    original_bytes
                );
                assert_eq!(
                    eth_bridge::Requests::<Runtime>::get(*network, load.hash()),
                    Some(load.clone())
                );
                assert_eq!(
                    eth_bridge::RequestSubmissionHeight::<Runtime>::get(*network, load.hash()),
                    recovery::CHECKPOINT_NUMBER
                );
                assert_eq!(
                    eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(*network, load.hash()),
                    wrapper.hash()
                );
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(*network, wrapper.hash()),
                    Some(eth_bridge::requests::RequestStatus::Done)
                );
                assert!(eth_bridge::RequestsQueue::<Runtime>::get(*network).is_empty());
                assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                assert_eq!(
                    eth_bridge::AccountRequests::<Runtime>::get(&index_owner),
                    vec![(*network, load.hash()), (*network, wrapper.hash())]
                );
                assert_eq!(
                    crate::BridgeProxy::sender(GenericNetworkId::EVMLegacy(*network), load.hash()),
                    Some(beneficiary.clone())
                );
                assert!(crate::BridgeProxy::transactions(
                    (GenericNetworkId::EVMLegacy(*network), old.clone()),
                    load.hash()
                )
                .is_none());
                let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert!(delayed_hashi_submit(&approvals[0]).is_err());
                // Freshly signed peer nonces cannot reopen an already consumed native receipt.
                let retry = delayed_hashi_approvals(&operation, &keys, finalize);
                assert!(delayed_hashi_submit(&retry[0]).is_err());
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
                delayed_hashi_no_context();
            });
        });
    }
}

#[test]
fn delayed_hashi_fresh_import_preserves_authentication_and_mapped_indexes_after_complete() {
    use common::AssetInfoProvider;
    sp_io::TestExternalities::default().execute_with(|| {
        let old = AccountId::new([7; 32]);
        let new = AccountId::new([8; 32]);
        let (policy, operation, keys) = hashi_fixture_with_author(old.clone());
        let IncomingChannel::Hashi {
            request,
            approvals,
            network,
        } = &operation.channel
        else {
            unreachable!()
        };
        let original: eth_bridge::requests::IncomingRequest<Runtime> =
            decode_bounded(request).unwrap();
        let wrapper = eth_bridge::requests::OffchainRequest::incoming(original);
        let transfer = hashi_transfer(request).unwrap();
        crate::incident_recovery::with_test_policy(&policy, || {
            delayed_hashi_complete_and_enable(&policy);
            // Peers sign using the ordinary post-recovery native dispatch weight.
            let outer = decode_runtime_call(&approvals[0].call).unwrap();
            let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi { call, .. }) = outer
            else {
                unreachable!()
            };
            let approvals =
                delayed_hashi_approvals(&operation, &keys, decode_runtime_call(&call).unwrap());
            // The original signed bytes cannot be rewritten to the replacement before verification.
            let mut tampered = approvals[0].clone();
            let mut outer = decode_runtime_call(&tampered.call).unwrap();
            let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi { call, .. }) =
                &mut outer
            else {
                unreachable!()
            };
            let mut inner = decode_runtime_call(call).unwrap();
            let RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
                incoming_request_result: Ok(eth_bridge::requests::IncomingRequest::Transfer(value)),
                ..
            }) = &mut inner
            else {
                unreachable!()
            };
            value.to = new.clone();
            *call = inner.encode();
            tampered.call = outer.encode();
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(delayed_hashi_submit(&tampered).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            for approval in &approvals {
                assert_ok!(delayed_hashi_submit(approval));
            }
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &new).unwrap(),
                transfer.amount
            );
            assert_eq!(crate::Assets::free_balance(&common::DAI, &old).unwrap(), 0);
            assert_eq!(
                eth_bridge::Requests::<Runtime>::get(*network, wrapper.hash()),
                Some(wrapper.clone())
            );
            assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
            assert_eq!(
                eth_bridge::AccountRequests::<Runtime>::get(&new),
                vec![(*network, transfer.tx_hash), (*network, wrapper.hash())]
            );
            let outer = decode_runtime_call(&approvals[0].call).unwrap();
            let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi { call, .. }) = outer
            else {
                unreachable!()
            };
            let retry =
                delayed_hashi_approvals(&operation, &keys, decode_runtime_call(&call).unwrap());
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(delayed_hashi_submit(&retry[0]).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            delayed_hashi_no_context();
        });
    });
}

#[test]
fn delayed_hashi_policy_mismatch_precedes_prepare_and_preserves_retained_pending_state() {
    for prepared in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, _) =
                hashi_fixture_with_kind(false, eth_bridge::requests::AssetKind::Thischain);
            let IncomingChannel::Hashi {
                request, network, ..
            } = &operation.channel
            else {
                unreachable!()
            };
            let original: eth_bridge::requests::IncomingRequest<Runtime> =
                decode_bounded(request).unwrap();
            let wrapper = eth_bridge::requests::OffchainRequest::incoming(original.clone());
            let bridge = crate::EthBridge::bridge_account(*network).unwrap();
            if prepared {
                assert_ok!(crate::EthBridge::register_incoming_request(
                    RuntimeOrigin::signed(bridge.clone()),
                    original.clone()
                ));
            }
            crate::incident_recovery::with_test_policy(&policy, || {
                delayed_hashi_complete_and_enable(&policy);
                sp_io::storage::set(POLICY_STORAGE_KEY, &[99; 32]);
                let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                let result = if prepared {
                    // The native inner failure path normally cancels; policy errors must
                    // not reach it and release already reserved checkpoint collateral.
                    crate::EthBridge::finalize_incoming_request(
                        RuntimeOrigin::signed(bridge),
                        wrapper.hash(),
                        *network,
                    )
                } else {
                    crate::EthBridge::register_incoming_request(
                        RuntimeOrigin::signed(bridge),
                        original,
                    )
                };
                assert!(result.is_err());
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
                delayed_hashi_no_context();
            });
        });
    }
}

#[test]
fn delayed_hashi_prepared_original_transfer_finalizes_once_after_complete() {
    use common::AssetInfoProvider;
    sp_io::TestExternalities::default().execute_with(|| {
        let old = AccountId::new([7; 32]);
        let new = AccountId::new([8; 32]);
        let (policy, operation, keys) = hashi_fixture_with_author(old.clone());
        let IncomingChannel::Hashi {
            request, network, ..
        } = &operation.channel
        else {
            unreachable!()
        };
        let transfer = hashi_transfer(request).unwrap();
        let original: eth_bridge::requests::IncomingRequest<Runtime> =
            decode_bounded(request).unwrap();
        let wrapper = eth_bridge::requests::OffchainRequest::incoming(original.clone());
        let bridge = crate::EthBridge::bridge_account(*network).unwrap();
        // Authentic native preparation represented in checkpoint state. The typed
        // compiler subsequently moves query ownership, preserving this payload.
        assert_ok!(crate::EthBridge::register_incoming_request(
            RuntimeOrigin::signed(bridge),
            original
        ));
        let index = eth_bridge::AccountRequests::<Runtime>::take(&old);
        eth_bridge::AccountRequests::<Runtime>::insert(&new, &index);
        crate::incident_recovery::with_test_policy(&policy, || {
            delayed_hashi_complete_and_enable(&policy);
            let inner = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
                hash: wrapper.hash(),
                network_id: *network,
            });
            for approval in delayed_hashi_approvals(&operation, &keys, inner.clone()) {
                assert_ok!(delayed_hashi_submit(&approval));
            }
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &new).unwrap(),
                transfer.amount
            );
            assert_eq!(crate::Assets::free_balance(&common::DAI, &old).unwrap(), 0);
            assert_eq!(
                eth_bridge::Requests::<Runtime>::get(*network, wrapper.hash()),
                Some(wrapper.clone())
            );
            assert_eq!(eth_bridge::AccountRequests::<Runtime>::get(&new), index);
            assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
            assert_eq!(
                crate::BridgeProxy::sender(GenericNetworkId::EVMLegacy(*network), transfer.tx_hash),
                Some(new)
            );
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            let retry = delayed_hashi_approvals(&operation, &keys, inner);
            assert!(delayed_hashi_submit(&retry[0]).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            delayed_hashi_no_context();
        });
    });
}

#[test]
fn delayed_hashi_complete_does_not_itself_allow_ordinary_delivery() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, _) = hashi_fixture_with_author(AccountId::new([7; 32]));
        let IncomingChannel::Hashi { approvals, .. } = &operation.channel else {
            unreachable!()
        };
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            let mut status = recovery::storage::status().unwrap().unwrap();
            status.phase = RecoveryPhase::Complete;
            sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(delayed_hashi_submit(&approvals[0]).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            assert!(sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY));
            delayed_hashi_no_context();
        });
    });
}

#[test]
fn delayed_hashi_import_policy_error_cannot_register_or_abort_a_different_kind_load() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, _) = hashi_fixture_with_author(AccountId::new([7; 32]));
        let IncomingChannel::Hashi {
            request, network, ..
        } = &operation.channel
        else {
            unreachable!()
        };
        let transfer = hashi_transfer(request).unwrap();
        let original: eth_bridge::requests::IncomingRequest<Runtime> =
            decode_bounded(request).unwrap();
        let bridge = crate::EthBridge::bridge_account(*network).unwrap();
        let load = eth_bridge::requests::LoadIncomingRequest::Transaction(
            eth_bridge::requests::LoadIncomingTransactionRequest::new(
                transfer.author.clone(),
                transfer.tx_hash,
                transfer.timepoint,
                eth_bridge::requests::IncomingTransactionRequestKind::AddAsset,
                *network,
            ),
        );
        crate::incident_recovery::with_test_policy(&policy, || {
            delayed_hashi_complete_and_enable(&policy);
            sp_io::storage::set(POLICY_STORAGE_KEY, &[99; 32]);
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(crate::EthBridge::import_incoming_request(
                RuntimeOrigin::signed(bridge),
                load,
                Ok(original)
            )
            .is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
            delayed_hashi_no_context();
        });
    });
}

#[test]
fn delayed_hashi_load_query_mapping_preserves_native_kind_and_decommission_admission() {
    use common::AssetInfoProvider;
    use eth_bridge::requests::IncomingTransactionRequestKind;
    for kind in [
        IncomingTransactionRequestKind::TransferXOR,
        IncomingTransactionRequestKind::AddAsset,
    ] {
        for decommissioned in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let old = AccountId::new([7; 32]);
                let new = AccountId::new([8; 32]);
                let (policy, operation, keys) = hashi_fixture_with_author(old.clone());
                let IncomingChannel::Hashi {
                    request, network, ..
                } = &operation.channel
                else {
                    unreachable!()
                };
                let transfer = hashi_transfer(request).unwrap();
                let original: eth_bridge::requests::IncomingRequest<Runtime> =
                    decode_bounded(request).unwrap();
                let wrapper = eth_bridge::requests::OffchainRequest::incoming(original.clone());
                // Native import allows a load kind distinct from its Transfer
                // result. Query ownership must not depend on that pairing.
                // This public DAI fixture never revives decommissioned XOR.
                let load = eth_bridge::requests::LoadIncomingRequest::Transaction(
                    eth_bridge::requests::LoadIncomingTransactionRequest::new(
                        transfer.author.clone(),
                        transfer.tx_hash,
                        transfer.timepoint,
                        kind,
                        *network,
                    ),
                );
                let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
                    load_incoming_request: load.clone(),
                    incoming_request_result: Ok(original),
                });
                crate::incident_recovery::with_test_policy(&policy, || {
                    delayed_hashi_complete_and_enable(&policy);
                    sp_io::storage::set(
                        &frame_support::storage::storage_prefix(
                            b"EthBridge",
                            b"LegacyEthereumXorDecommissioned",
                        ),
                        &decommissioned.encode(),
                    );
                    let approvals = delayed_hashi_approvals(&operation, &keys, inner);
                    let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                    if decommissioned && kind == IncomingTransactionRequestKind::TransferXOR {
                        assert!(delayed_hashi_submit(&approvals[0]).is_err());
                        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
                        assert_eq!(crate::Assets::free_balance(&common::DAI, &new).unwrap(), 0);
                    } else {
                        for approval in &approvals {
                            assert_ok!(delayed_hashi_submit(approval));
                        }
                        assert_eq!(
                            crate::Assets::free_balance(&common::DAI, &new).unwrap(),
                            transfer.amount
                        );
                        assert_eq!(
                            eth_bridge::Requests::<Runtime>::get(*network, transfer.tx_hash),
                            Some(eth_bridge::requests::OffchainRequest::LoadIncoming(load))
                        );
                        assert_eq!(
                            eth_bridge::AccountRequests::<Runtime>::get(&new),
                            vec![(*network, transfer.tx_hash), (*network, wrapper.hash())]
                        );
                    }
                    assert!(!eth_bridge::AccountRequests::<Runtime>::contains_key(&old));
                    assert_eq!(crate::Assets::free_balance(&common::DAI, &old).unwrap(), 0);
                    delayed_hashi_no_context();
                });
            });
        }
    }
}
