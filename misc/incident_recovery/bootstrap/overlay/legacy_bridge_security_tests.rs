//! Export-only SORA Nexus tests of the configured legacy bridge adapters.
//!
//! Public fixture policies exercise routing mechanics only. The production-path
//! tests deliberately install no TEST_POLICY: spec 134/135 must reject ordinary
//! policy-present delivery even if the independent halt marker is absent.
//! Native bridge-account origins and signing keys are public local fixtures;
//! their successful checks do not attest external-chain finality.

use super::tests::{fixture, hashi_fixture_with_kind};
use super::*;
use bridge_types::traits::{BridgeAssetRegistry, BridgeOriginOutput, MessageStatusNotifier};
use bridge_types::types::{AssetKind, CallOriginOutput};
use bridge_types::{GenericAssetId, H160};
use common::AssetInfoProvider;
use eth_bridge::requests::{
    AssetKind as HashiAssetKind, IncomingRequest, IncomingTransactionRequestKind, IncomingTransfer,
    LoadIncomingRequest, LoadIncomingTransactionRequest, OffchainRequest, RequestStatus,
};
use frame_support::{assert_ok, traits::BuildGenesisConfig};
use sp_core::{ecdsa, Pair};
use sp_runtime::traits::{Hash, Keccak256};

fn state_root() -> Vec<u8> {
    sp_io::storage::root(sp_runtime::StateVersion::V1)
}

fn hashi_fixture(kind: HashiAssetKind) -> (RecoveryPolicy, IncomingTransfer<Runtime>) {
    let (policy, operation, _) = hashi_fixture_with_kind(false, kind);
    let IncomingChannel::Hashi { request, .. } = operation.channel else {
        unreachable!()
    };
    let IncomingRequest::Transfer(transfer) =
        IncomingRequest::<Runtime>::decode(&mut request.as_slice()).unwrap()
    else {
        unreachable!()
    };
    (policy, transfer)
}

fn import(
    transfer: &IncomingTransfer<Runtime>,
) -> frame_support::dispatch::DispatchResultWithPostInfo {
    let bridge = crate::EthBridge::bridge_account(transfer.network_id).unwrap();
    crate::EthBridge::import_incoming_request(
        RuntimeOrigin::signed(bridge),
        LoadIncomingRequest::Transaction(LoadIncomingTransactionRequest::new(
            transfer.author.clone(),
            transfer.tx_hash,
            transfer.timepoint,
            IncomingTransactionRequestKind::Transfer,
            transfer.network_id,
        )),
        Ok(IncomingRequest::Transfer(transfer.clone())),
    )
}

#[test]
fn legacy_security_hashi_no_policy_retains_historical_recipient() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, mut transfer) = hashi_fixture(HashiAssetKind::Sidechain);
        transfer.from = H160::from(recovery::ATTACKER_EVM);
        transfer.to = AccountId::new(recovery::ATTACKER_ACCOUNT);
        assert!(!sp_io::storage::exists(POLICY_STORAGE_KEY));
        let original = OffchainRequest::incoming(IncomingRequest::Transfer(transfer.clone()));
        assert_ok!(import(&transfer));
        assert_eq!(
            crate::Assets::free_balance(&common::DAI, &transfer.to).unwrap(),
            transfer.amount
        );
        assert_eq!(
            crate::Assets::free_balance(
                &common::DAI,
                &AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT)
            )
            .unwrap(),
            0
        );
        assert_eq!(
            eth_bridge::Requests::<Runtime>::get(transfer.network_id, original.hash()),
            Some(original)
        );
    });
}

#[test]
fn legacy_security_production_policy_rejects_hashi_without_cancel_or_state_change() {
    for prepared in [false, true] {
        for malformed in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, transfer) = hashi_fixture(HashiAssetKind::Thischain);
                let bridge = crate::EthBridge::bridge_account(transfer.network_id).unwrap();
                let incoming = IncomingRequest::Transfer(transfer.clone());
                let original = OffchainRequest::incoming(incoming.clone());
                if prepared {
                    assert_ok!(crate::EthBridge::register_incoming_request(
                        RuntimeOrigin::signed(bridge.clone()),
                        incoming
                    ));
                }
                let value = if malformed {
                    vec![0xff]
                } else {
                    policy.commitment().to_vec()
                };
                sp_io::storage::set(POLICY_STORAGE_KEY, &value);
                assert!(!sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY));
                // No public fixture policy override: an accidentally removed halt
                // cannot open the production legacy ownership resolver.
                let before = state_root();
                let result = if prepared {
                    crate::EthBridge::finalize_incoming_request(
                        RuntimeOrigin::signed(bridge),
                        original.hash(),
                        transfer.network_id,
                    )
                } else {
                    import(&transfer)
                };
                assert!(result.is_err());
                assert_eq!(state_root(), before);
                assert!(transfer.finalize().is_err());
                assert_eq!(state_root(), before);
                if prepared {
                    assert_eq!(
                        eth_bridge::RequestStatuses::<Runtime>::get(
                            transfer.network_id,
                            original.hash()
                        ),
                        Some(RequestStatus::Pending)
                    );
                }
            });
        }
    }
}

#[test]
fn legacy_security_zero_source_rejects_before_import_and_prepared_cancellation() {
    for prepared in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, mut transfer) = hashi_fixture(HashiAssetKind::Thischain);
            transfer.from = H160::zero();
            let bridge = crate::EthBridge::bridge_account(transfer.network_id).unwrap();
            let incoming = IncomingRequest::Transfer(transfer.clone());
            let original = OffchainRequest::incoming(incoming.clone());
            if prepared {
                assert_ok!(crate::EthBridge::register_incoming_request(
                    RuntimeOrigin::signed(bridge.clone()),
                    incoming
                ));
            }
            crate::incident_recovery::with_test_policy(&policy, || {
                sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
                let before = state_root();
                let error = if prepared {
                    crate::EthBridge::finalize_incoming_request(
                        RuntimeOrigin::signed(bridge),
                        original.hash(),
                        transfer.network_id,
                    )
                } else {
                    import(&transfer)
                }
                .unwrap_err();
                assert_eq!(
                    error.error,
                    eth_bridge::Error::<Runtime>::InvalidAddress.into()
                );
                assert_eq!(state_root(), before);
                assert_eq!(
                    transfer.finalize(),
                    Err(eth_bridge::Error::<Runtime>::InvalidAddress.into())
                );
                assert_eq!(state_root(), before);
            });
        });
    }
}

#[test]
fn legacy_security_halted_finalization_requires_exact_single_use_resolution() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, mut transfer) = hashi_fixture(HashiAssetKind::Thischain);
        transfer.from = H160::from(recovery::ATTACKER_EVM);
        assert_ok!(transfer.prepare());
        sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
        sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
        let block = System::block_number();
        let beneficiary = AccountId::new([99; 32]);
        let resolution = eth_bridge::recovery::HashiResolution {
            block,
            request_hash: sp_io::hashing::blake2_256(&transfer.encode()),
            beneficiary: beneficiary.clone(),
            excluded: false,
            consumed: false,
        };
        for invalid in 0..4 {
            sp_io::storage::clear(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY);
            sp_io::storage::clear(eth_bridge::recovery::HASHI_RESOLUTION_KEY);
            let mut context = resolution.clone();
            if invalid != 0 {
                let context_block = if invalid == 1 { block - 1 } else { block };
                sp_io::storage::set(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, &context_block.encode());
            }
            if invalid == 3 {
                context.request_hash = [0; 32];
            }
            if invalid != 2 {
                sp_io::storage::set(
                    eth_bridge::recovery::HASHI_RESOLUTION_KEY,
                    &context.encode(),
                );
            }
            let before = state_root();
            assert!(transfer.finalize().is_err());
            assert_eq!(state_root(), before);
        }
        sp_io::storage::set(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, &block.encode());
        sp_io::storage::set(
            eth_bridge::recovery::HASHI_RESOLUTION_KEY,
            &resolution.encode(),
        );
        assert_eq!(transfer.finalize(), Ok(transfer.tx_hash));
        assert_eq!(
            crate::Assets::free_balance(&common::DAI, &beneficiary).unwrap(),
            transfer.amount
        );
        assert_eq!(
            crate::Assets::free_balance(
                &common::DAI,
                &AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT)
            )
            .unwrap(),
            0
        );
        let retained = sp_io::storage::get(eth_bridge::recovery::HASHI_RESOLUTION_KEY).unwrap();
        assert!(
            eth_bridge::recovery::HashiResolution::<AccountId>::decode(&mut retained.as_ref())
                .unwrap()
                .consumed
        );
        let before = state_root();
        assert!(transfer.finalize().is_err());
        assert_eq!(state_root(), before);
    });
}

#[test]
fn legacy_security_fixture_hashi_routes_attacker_proceeds_without_changing_request() {
    for kind in [HashiAssetKind::Sidechain, HashiAssetKind::Thischain] {
        // sender, beneficiary, EVM registry, local registry, mapped innocent,
        // and unchanged innocent recipient all use the configured native route.
        for case in 0..6 {
            sp_io::TestExternalities::default().execute_with(|| {
                let (policy, mut transfer) = hashi_fixture(kind);
                let migration = AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT);
                let expected = match case {
                    0 => {
                        transfer.from = H160::from(recovery::ATTACKER_EVM);
                        migration.clone()
                    }
                    1 => {
                        transfer.to = AccountId::new(recovery::ATTACKER_ACCOUNT);
                        migration.clone()
                    }
                    2 => {
                        sp_io::storage::set(
                            &recovery::attacker_registry_evm_key(transfer.from.as_fixed_bytes()),
                            &[1],
                        );
                        migration.clone()
                    }
                    3 => {
                        transfer.to = AccountId::new([31; 32]);
                        sp_io::storage::set(
                            &recovery::attacker_registry_account_key(&[31; 32]),
                            &[1],
                        );
                        migration.clone()
                    }
                    4 => AccountId::new([8; 32]),
                    _ => {
                        transfer.to = AccountId::new([32; 32]);
                        transfer.to.clone()
                    }
                };
                transfer.should_take_fee = kind.is_owned();
                let fee = if transfer.should_take_fee {
                    IncomingTransfer::<Runtime>::fee_amount()
                } else {
                    0
                };
                assert!(transfer.amount > fee);
                let original =
                    OffchainRequest::incoming(IncomingRequest::Transfer(transfer.clone()));
                let signed = transfer.encode();
                let bridge = crate::EthBridge::bridge_account(transfer.network_id).unwrap();
                let custody = crate::Assets::total_balance(&common::DAI, &bridge).unwrap();
                let issuance = crate::Assets::total_issuance(&common::DAI).unwrap();
                crate::incident_recovery::with_test_policy(&policy, || {
                    sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
                    assert_ok!(import(&transfer));
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &expected).unwrap(),
                        transfer.amount - fee
                    );
                    if expected != transfer.to {
                        assert_eq!(
                            crate::Assets::free_balance(&common::DAI, &transfer.to).unwrap(),
                            0
                        );
                    }
                    assert_eq!(
                        crate::Assets::total_issuance(&common::DAI).unwrap(),
                        if kind.is_owned() {
                            issuance - fee
                        } else {
                            issuance + transfer.amount
                        }
                    );
                    assert_eq!(
                        crate::Assets::total_balance(&common::DAI, &bridge).unwrap(),
                        if kind.is_owned() {
                            custody - transfer.amount
                        } else {
                            custody
                        }
                    );
                    assert_eq!(transfer.encode(), signed);
                    assert_eq!(
                        eth_bridge::Requests::<Runtime>::get(transfer.network_id, original.hash()),
                        Some(original.clone())
                    );
                    assert_eq!(
                        eth_bridge::RequestStatuses::<Runtime>::get(
                            transfer.network_id,
                            original.hash()
                        ),
                        Some(RequestStatus::Done)
                    );
                    assert_eq!(
                        crate::BridgeProxy::sender(
                            GenericNetworkId::EVMLegacy(transfer.network_id),
                            transfer.tx_hash
                        ),
                        Some(expected)
                    );
                    let before = state_root();
                    assert!(crate::EthBridge::finalize_incoming_request(
                        RuntimeOrigin::signed(bridge),
                        original.hash(),
                        transfer.network_id
                    )
                    .is_err());
                    assert_eq!(state_root(), before);
                });
            });
        }
    }
}

fn refund_fixture() -> (RecoveryPolicy, GenericNetworkId, AccountId) {
    let (policy, _, _) = fixture(AccountId::new([7; 32]));
    let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
    let custody = crate::BridgeProxy::bridge_account(network).unwrap();
    System::inc_providers(&custody);
    assert_ok!(crate::Assets::register_asset_id(
        custody.clone(),
        common::DAI,
        common::AssetSymbol(b"DAI".to_vec()),
        common::AssetName(b"Dai".to_vec()),
        18,
        0,
        true,
        common::AssetType::Regular,
        None,
        None
    ));
    assert_ok!(<crate::BridgeProxy as BridgeAssetRegistry<
        AccountId,
        crate::AssetId,
    >>::manage_asset(network, common::DAI));
    // The historical app has no pending-authorization storage or new root call.
    let origin: RuntimeOrigin = dispatch::RawOrigin::new(CallOriginOutput::new(
        SubNetworkId::Liberland,
        H256::zero(),
        GenericTimepoint::Unknown,
        (),
    ))
    .into();
    assert_ok!(crate::SubstrateBridgeApp::finalize_asset_registration(
        origin,
        common::DAI,
        GenericAssetId::Sora(common::DAI.into()),
        AssetKind::Sidechain,
        18
    ));
    (policy, network, custody)
}

#[test]
fn legacy_security_refund_production_policy_and_halt_fail_before_asset_changes() {
    for mode in 0..3 {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, network, _) = refund_fixture();
            let original = AccountId::new(recovery::ATTACKER_ACCOUNT);
            let value = if mode == 0 {
                vec![0xff]
            } else {
                policy.commitment().to_vec()
            };
            sp_io::storage::set(POLICY_STORAGE_KEY, &value);
            if mode == 2 {
                sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
            }
            let before = state_root();
            assert!(crate::BridgeProxy::refund(
                network,
                H256::repeat_byte(98),
                GenericAccount::Sora(original),
                common::DAI,
                1_000
            )
            .is_err());
            assert_eq!(state_root(), before);
        });
    }
}

#[test]
fn legacy_security_refund_no_policy_preserves_historical_attacker_account() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, network, _) = refund_fixture();
        let original = AccountId::new(recovery::ATTACKER_ACCOUNT);
        assert_ok!(crate::BridgeProxy::refund(
            network,
            H256::repeat_byte(98),
            GenericAccount::Sora(original.clone()),
            common::DAI,
            1_000
        ));
        assert_eq!(
            crate::Assets::free_balance(&common::DAI, &original).unwrap(),
            1_000
        );
        assert_eq!(
            crate::Assets::free_balance(
                &common::DAI,
                &AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT)
            )
            .unwrap(),
            0
        );
    });
}

#[test]
fn legacy_security_fixture_status_refund_routes_owner_and_preserves_source() {
    let migration = AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT);
    for (source, beneficiary) in [
        (AccountId::new([7; 32]), AccountId::new([8; 32])),
        (
            AccountId::new(recovery::ATTACKER_ACCOUNT),
            migration.clone(),
        ),
        (
            AccountId::new(recovery::ATTACKER_COLLECTION_ACCOUNT),
            migration.clone(),
        ),
        (AccountId::new([31; 32]), migration.clone()),
        (AccountId::new([32; 32]), AccountId::new([32; 32])),
    ] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, network, _) = refund_fixture();
            let id = H256::repeat_byte(98);
            crate::BridgeProxy::outbound_request(
                network,
                id,
                source.clone(),
                GenericAccount::Liberland(AccountId::new([33; 32])),
                common::DAI,
                1_000,
                MessageStatus::InQueue,
            );
            sp_io::storage::set(&recovery::attacker_registry_account_key(&[31; 32]), &[1]);
            crate::incident_recovery::with_test_policy(&policy, || {
                sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
                crate::BridgeProxy::update_status(
                    network,
                    id,
                    MessageStatus::Failed,
                    GenericTimepoint::Unknown,
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &beneficiary).unwrap(),
                    1_000
                );
                if source != beneficiary {
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &source).unwrap(),
                        0
                    );
                }
                let request =
                    crate::BridgeProxy::transactions((network, source.clone()), id).unwrap();
                assert_eq!(request.source(), &GenericAccount::Sora(source.clone()));
                assert_eq!(request.status(), MessageStatus::Refunded);
                assert_eq!(
                    crate::BridgeProxy::sender(network, id),
                    Some(source.clone())
                );
                let before = state_root();
                crate::BridgeProxy::update_status(
                    network,
                    id,
                    MessageStatus::Failed,
                    GenericTimepoint::Unknown,
                );
                assert_eq!(
                    state_root(),
                    before,
                    "terminal refund cannot be repeated under policy"
                );
            });
        });
    }
}

#[test]
fn legacy_security_fixture_refund_rejects_corrupt_policy_and_rolls_back_mint_failure() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, network, custody) = refund_fixture();
        let original = AccountId::new([7; 32]);
        crate::incident_recovery::with_test_policy(&policy, || {
            sp_io::storage::set(POLICY_STORAGE_KEY, &[0xff]);
            let before = state_root();
            assert!(crate::BridgeProxy::refund(
                network,
                H256::repeat_byte(98),
                GenericAccount::Sora(original.clone()),
                common::DAI,
                1_000
            )
            .is_err());
            assert_eq!(state_root(), before);
            sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
            // The underlying sidechain mint must fail after before_asset_unlock
            // has attempted to change lock accounting; the refund rolls it back.
            assert_ok!(crate::Assets::set_non_mintable_from(&common::DAI, &custody));
            let before = state_root();
            assert_eq!(
                crate::BridgeProxy::refund(
                    network,
                    H256::repeat_byte(99),
                    GenericAccount::Sora(original),
                    common::DAI,
                    1
                ),
                Err(assets::Error::<Runtime>::AssetSupplyIsNotMintable.into())
            );
            assert_eq!(state_root(), before);
        });
    });
}

fn ton_fixture() -> (RecoveryPolicy, GenericNetworkId, Commitment, MultiProof) {
    use bridge_types::ton::{TonAddress, TonNetworkId, TonTransactionId};
    let (policy, _, _) = fixture(AccountId::new([7; 32]));
    let network = TonNetworkId::Mainnet;
    let owner = crate::BridgeProxy::bridge_account(network.into()).unwrap();
    System::inc_providers(&owner);
    assert_ok!(crate::Assets::register_asset_id(
        owner,
        common::DAI,
        common::AssetSymbol(b"DAI".to_vec()),
        common::AssetName(b"Dai".to_vec()),
        18,
        0,
        true,
        common::AssetType::Regular,
        None,
        None,
    ));
    let app = TonAddress::new(0, H256::repeat_byte(31));
    let channel = TonAddress::new(0, H256::repeat_byte(32));
    jetton_app::GenesisConfig::<Runtime> {
        app: Some((network, app)),
        assets: vec![(common::DAI, TonAddress::empty(), AssetKind::Sidechain, 18)],
    }
    .build();
    bridge_channel::inbound::TONChannelAddresses::<Runtime>::insert(network, channel);
    let call = BridgeCall::JettonApp(JettonAppCall::Transfer {
        token: TonAddress::empty().into(),
        sender: TonAddress::new(0, H256::repeat_byte(6)).into(),
        recipient: AccountId::new([7; 32]),
        amount: 1_000u128.into(),
    });
    let commitment: Commitment = GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(
        bridge_types::ton::InboundCommitment {
            channel,
            source: app,
            nonce: 1,
            transaction_id: TonTransactionId {
                lt: 1,
                hash: H256::repeat_byte(33),
            },
            payload: call.encode().try_into().unwrap(),
        },
    ));
    let network = GenericNetworkId::TON(network);
    let keys: Vec<_> = (1u8..=4)
        .map(|seed| ecdsa::Pair::from_seed(&[seed; 32]))
        .collect();
    // Incoming proof verification reads the verifier's own network registry;
    // the signer's registry below tracks the independent approval lifecycle.
    let peers: BTreeSet<_> = keys.iter().map(Pair::public).collect();
    multisig_verifier::PeerKeys::<Runtime>::insert(
        network,
        sp_runtime::BoundedBTreeSet::<_, <Runtime as multisig_verifier::Config>::MaxPeers>::try_from(
            peers,
        )
        .unwrap(),
    );
    let hash = Keccak256::hash_of(&(network, ThisNetworkId::get(), commitment.hash()));
    assert_ok!(crate::BridgeDataSigner::force_set_peers(
        RuntimeOrigin::root(),
        network,
        keys.iter()
            .map(Pair::public)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
    ));
    assert_ok!(crate::BridgeDataSigner::register_pending_approval(
        RuntimeOrigin::root(),
        network,
        hash
    ));
    let proof = MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof {
        proof: keys
            .iter()
            .map(|key| key.sign_prehashed(hash.as_fixed_bytes()))
            .collect(),
    });
    (policy, network, commitment, proof)
}

#[test]
fn legacy_security_ton_halt_and_production_policy_cannot_move_value() {
    for halted in [true, false] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, network, commitment, proof) = ton_fixture();
            // Deliberately no TEST_POLICY override or simulated final reopening.
            sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
            if halted {
                sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
            } else {
                // Even an accidentally absent independent halt cannot make the
                // production legacy permanent resolver admit ordinary delivery.
                assert!(!sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY));
                let GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(message)) =
                    &commitment
                else {
                    unreachable!()
                };
                let before = state_root();
                assert_eq!(
                    RecoveryGenericMessageDispatch::dispatch(
                        network,
                        MessageId::basic(network, ThisNetworkId::get(), 1),
                        GenericTimepoint::TON(message.transaction_id),
                        &message.payload,
                        GenericAdditionalInboundData::TON(
                            bridge_types::ton::AdditionalTONInboundData {
                                source: message.source
                            }
                        ),
                    ),
                    MessageDispatchOutcome::Failed
                );
                assert_eq!(state_root(), before);
            }
            let before = state_root();
            let result = BridgeInboundChannel::submit(
                RuntimeOrigin::none(),
                network,
                commitment.clone(),
                proof.clone(),
            );
            if halted {
                assert_eq!(
                    result.unwrap_err().error,
                    bridge_channel::inbound::Error::<Runtime>::RecoveryHalted.into()
                );
                assert_eq!(state_root(), before);
                assert_eq!(
                    bridge_channel::inbound::ChannelNonces::<Runtime>::get(network),
                    0
                );
            } else {
                // Historical channels consume an authenticated nonce on local
                // dispatch failure. Do not misreport this as a no-write reject.
                assert_ok!(result);
                assert_eq!(
                    bridge_channel::inbound::ChannelNonces::<Runtime>::get(network),
                    1
                );
                let before = state_root();
                assert!(BridgeInboundChannel::submit(
                    RuntimeOrigin::none(),
                    network,
                    commitment,
                    proof
                )
                .is_err());
                assert_eq!(state_root(), before);
            }
            for account in [
                AccountId::new([7; 32]),
                AccountId::new([8; 32]),
                AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT),
            ] {
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &account).unwrap(),
                    0
                );
                assert!(crate::BridgeProxy::transactions(
                    (network, account),
                    MessageId::basic(network, ThisNetworkId::get(), 1).hash()
                )
                .is_none());
            }
            assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), 0);
            assert_eq!(crate::BridgeProxy::locked_assets(network, common::DAI), 0);
            assert_eq!(
                crate::BridgeProxy::sender(
                    network,
                    MessageId::basic(network, ThisNetworkId::get(), 1).hash()
                ),
                None
            );
        });
    }
}
