//! Native channel catch-up regression coverage. These fixtures use only public
//! deterministic peer keys and the normal verifier, channel and application paths.
//!
//! "Pending" here means an authenticated external transfer whose native channel
//! nonce has not yet been consumed at the checkpoint. Channel bridges do not
//! persist HASHI-style `LoadIncoming` requests: a valid submission authenticates,
//! consumes its nonce and dispatches its application together. Existing on-chain
//! HASHI Pending loads have separate regression coverage in the parent module.

#[path = "incident_recovery_incoming_post_complete_channel_tests.rs"]
mod post_complete;

use super::*;
use bridge_types::{
    substrate::BridgeMessage,
    traits::{BridgeAssetRegistry, BridgeOriginOutput},
    types::{AssetKind, AuxiliaryDigest, AuxiliaryDigestItem, CallOriginOutput, MessageDirection},
    GenericAssetId, GenericBalance,
};
use common::{recovery::AccountReplacement, AssetInfoProvider};
use frame_support::{assert_ok, traits::BuildGenesisConfig};
use sp_core::{ecdsa, Pair};
use sp_runtime::traits::{Hash, Keccak256};

const AMOUNT: u128 = 1_000_000_000_000_000_000;

#[derive(Clone, Copy, Debug)]
enum ChannelKind {
    Substrate,
    Parachain,
    Ton,
    #[cfg(feature = "wip")]
    Evm,
}

fn kinds() -> Vec<ChannelKind> {
    vec![
        ChannelKind::Substrate,
        ChannelKind::Parachain,
        #[cfg(feature = "wip")]
        ChannelKind::Evm,
    ]
}

fn old() -> AccountId {
    AccountId::new([7; 32])
}

fn replacement() -> AccountId {
    AccountId::new([8; 32])
}

fn origin(network: SubNetworkId) -> RuntimeOrigin {
    dispatch::RawOrigin::new(CallOriginOutput::new(
        network,
        H256::zero(),
        GenericTimepoint::Unknown,
        (),
    ))
    .into()
}

fn sign(channel: &mut IncomingChannel) {
    let (network, commitment, proof, substrate) = match channel {
        IncomingChannel::Substrate {
            network,
            commitment,
            proof,
        } => ((*network).into(), commitment, proof, true),
        IncomingChannel::Generic {
            network,
            commitment,
            proof,
        } => (*network, commitment, proof, false),
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
            unreachable!()
        }
    };
    let keys: Vec<_> = (1u8..=4)
        .map(|seed| ecdsa::Pair::from_seed(&[seed; 32]))
        .collect();
    let peers: BTreeSet<_> = keys.iter().map(Pair::public).collect();
    multisig_verifier::PeerKeys::<Runtime>::insert(
        network,
        sp_runtime::BoundedBTreeSet::<_, <Runtime as multisig_verifier::Config>::MaxPeers>::try_from(peers)
            .unwrap(),
    );
    if substrate {
        let digest = AuxiliaryDigest {
            logs: vec![AuxiliaryDigestItem::Commitment(
                ThisNetworkId::get(),
                commitment.hash(),
            )],
        };
        let hash = Keccak256::hash_of(&digest);
        *proof = MultiProof::Multisig(multisig_verifier::Proof {
            digest,
            proof: keys
                .iter()
                .map(|key| key.sign_prehashed(hash.as_fixed_bytes()))
                .collect(),
        });
    } else {
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
            hash,
        ));
        *proof = MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof {
            proof: keys
                .iter()
                .map(|key| key.sign_prehashed(hash.as_fixed_bytes()))
                .collect(),
        });
    }
}

fn empty_proof() -> MultiProof {
    MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof { proof: vec![] })
}

fn fixture(kind: ChannelKind, recipient: AccountId) -> (RecoveryPolicy, IncomingOperation) {
    fixture_with_asset_kind(kind, recipient, AssetKind::Sidechain)
}

fn fixture_with_asset_kind(
    kind: ChannelKind,
    recipient: AccountId,
    asset_kind: AssetKind,
) -> (RecoveryPolicy, IncomingOperation) {
    System::set_block_number(recovery::CHECKPOINT_NUMBER + 1);
    let network = match kind {
        ChannelKind::Substrate => SubNetworkId::Liberland.into(),
        ChannelKind::Parachain => SubNetworkId::Polkadot.into(),
        ChannelKind::Ton => bridge_types::ton::TonNetworkId::Mainnet.into(),
        #[cfg(feature = "wip")]
        ChannelKind::Evm => GenericNetworkId::EVM(H256::from_low_u64_be(1)),
    };
    let owner = crate::BridgeProxy::bridge_account(network).unwrap();
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
    let mut channel = match kind {
        ChannelKind::Substrate | ChannelKind::Parachain => {
            let (network, call) = if matches!(kind, ChannelKind::Substrate) {
                let network = SubNetworkId::Liberland;
                assert_ok!(crate::SubstrateBridgeApp::authorize_asset_registration(
                    RuntimeOrigin::root(),
                    network,
                    common::DAI,
                    GenericAssetId::Sora(common::DAI.into()),
                    asset_kind,
                ));
                assert_ok!(crate::SubstrateBridgeApp::finalize_asset_registration(
                    origin(network),
                    common::DAI,
                    GenericAssetId::Sora(common::DAI.into()),
                    asset_kind,
                    18,
                ));
                assert_ok!(<crate::BridgeProxy as BridgeAssetRegistry<
                    AccountId,
                    crate::AssetId,
                >>::manage_asset(
                    network.into(), common::DAI,
                ));
                (
                    network,
                    BridgeCall::SubstrateApp(SubstrateAppCall::Transfer {
                        asset_id: GenericAssetId::Sora(common::DAI.into()),
                        sender: GenericAccount::Liberland(AccountId::new([6; 32])),
                        recipient: GenericAccount::Sora(recipient),
                        amount: GenericBalance::Substrate(AMOUNT),
                    }),
                )
            } else {
                let network = SubNetworkId::Polkadot;
                assert_ok!(crate::ParachainBridgeApp::register_asset_inner(
                    network,
                    common::DAI,
                    bridge_types::substrate::PARENT_PARACHAIN_ASSET,
                    AssetKind::Sidechain,
                    18,
                    vec![],
                    0,
                ));
                assert_ok!(crate::ParachainBridgeApp::finalize_asset_registration(
                    origin(network),
                    common::DAI,
                    AssetKind::Sidechain,
                ));
                (
                    network,
                    BridgeCall::ParachainApp(ParachainAppCall::Transfer {
                        asset_id: common::DAI.into(),
                        sender: Some(xcm::VersionedLocation::V4(xcm::v4::Location::here())),
                        recipient,
                        amount: AMOUNT,
                    }),
                )
            };
            IncomingChannel::Substrate {
                network,
                commitment: GenericCommitment::Sub(bridge_types::substrate::Commitment {
                    nonce: 1,
                    messages: vec![BridgeMessage {
                        payload: call.encode().try_into().unwrap(),
                        timepoint: GenericTimepoint::Sora(1),
                    }]
                    .try_into()
                    .unwrap(),
                }),
                proof: empty_proof(),
            }
        }
        ChannelKind::Ton => {
            use bridge_types::ton::{TonAddress, TonNetworkId, TonTransactionId};
            let network = TonNetworkId::Mainnet;
            let app = TonAddress::new(0, H256::repeat_byte(31));
            let channel = TonAddress::new(0, H256::repeat_byte(32));
            // Retained historical bindings remain decodable after TON is disabled.
            jetton_app::GenesisConfig::<Runtime> {
                app: Some((network, app)),
                assets: vec![(common::DAI, TonAddress::empty(), AssetKind::Sidechain, 18)],
            }
            .build();
            bridge_channel::inbound::TONChannelAddresses::<Runtime>::insert(network, channel);
            let call = BridgeCall::JettonApp(JettonAppCall::Transfer {
                token: TonAddress::empty().into(),
                sender: TonAddress::new(0, H256::repeat_byte(6)).into(),
                recipient,
                amount: AMOUNT.into(),
            });
            IncomingChannel::Generic {
                network: network.into(),
                commitment: GenericCommitment::TON(bridge_types::ton::Commitment::Inbound(
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
                )),
                proof: empty_proof(),
            }
        }
        #[cfg(feature = "wip")]
        ChannelKind::Evm => {
            let network = H256::from_low_u64_be(1);
            let app = bridge_types::H160::repeat_byte(31);
            let channel = bridge_types::H160::repeat_byte(32);
            bridge_channel::inbound::EVMChannelAddresses::<Runtime>::insert(network, channel);
            assert_ok!(crate::EVMFungibleApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                network,
                app,
                common::DAI,
                18,
            ));
            let call = BridgeCall::FAApp(FAAppCall::Transfer {
                token: bridge_types::H160::zero(),
                sender: bridge_types::H160::repeat_byte(6),
                recipient,
                amount: H256::from_low_u64_be(AMOUNT as u64),
            });
            IncomingChannel::Generic {
                network: network.into(),
                commitment: GenericCommitment::EVM(bridge_types::evm::Commitment::Inbound(
                    bridge_types::evm::InboundCommitment {
                        channel,
                        source: app,
                        nonce: 1,
                        block_number: 1,
                        payload: call.encode().try_into().unwrap(),
                    },
                )),
                proof: empty_proof(),
            }
        }
    };
    sign(&mut channel);
    let network = match channel {
        IncomingChannel::Substrate { network, .. } => network.into(),
        IncomingChannel::Generic { network, .. } => network,
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
            unreachable!()
        }
    };
    let attackers = if matches!(network, GenericNetworkId::EVM(_)) {
        vec![GenericAccount::EVM(bridge_types::H160::from(
            recovery::ATTACKER_EVM,
        ))]
    } else {
        vec![]
    };
    let operation = IncomingOperation {
        index: 0,
        channel,
        registries: vec![
            NetworkAttackerRegistry {
                network,
                no_known_attackers: attackers.is_empty(),
                accounts: attackers,
                attestation: [1; 32],
            },
            NetworkAttackerRegistry {
                network: ThisNetworkId::get(),
                no_known_attackers: false,
                accounts: vec![GenericAccount::Sora(AccountId::new(
                    recovery::ATTACKER_ACCOUNT,
                ))],
                attestation: [2; 32],
            },
        ],
        finalized_source_evidence: [3; 32],
    };
    let policy = RecoveryPolicy {
        format_version: 2,
        authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![AccountReplacement {
            old: old(),
            new: replacement(),
        }],
        council: vec![AccountId::new([5; 32])],
        chunk_hashes: vec![operation.commitment()],
        audit_chunk_hashes: vec![[4; 32]],
        evidence_commitment: [5; 32],
        terminal_audit_commitment: [6; 32],
    };
    (policy, operation)
}

fn native_nonce(operation: &IncomingOperation) -> u64 {
    match operation.channel {
        IncomingChannel::Substrate { network, .. } => {
            substrate_bridge_channel::inbound::ChannelNonces::<Runtime>::get(network)
        }
        IncomingChannel::Generic { network, .. } => {
            bridge_channel::inbound::ChannelNonces::<Runtime>::get(network)
        }
        IncomingChannel::Hashi { .. } | IncomingChannel::HashiAuthenticatedSource { .. } => {
            unreachable!()
        }
    }
}

fn assert_no_credit(operation: &IncomingOperation, entry: &ExpectedTransfer) {
    assert_eq!(
        crate::Assets::free_balance(&common::DAI, &old()).unwrap(),
        0
    );
    assert_eq!(
        crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
        0
    );
    assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), 0);
    assert_eq!(
        crate::BridgeProxy::locked_assets(operation.network(), common::DAI),
        0
    );
    assert_eq!(
        crate::BridgeProxy::sender(operation.network(), entry.id),
        None
    );
    assert!(crate::BridgeProxy::transactions((operation.network(), old()), entry.id).is_none());
    assert!(
        crate::BridgeProxy::transactions((operation.network(), replacement()), entry.id).is_none()
    );
    assert!(!frame_system::Account::<Runtime>::contains_key(old()));
}

#[test]
fn recovery_channels_authenticated_pending_receipts_credit_only_replacement_once() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let signed_bytes = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(operation.encode(), signed_bytes, "{kind:?}");
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &old()).unwrap(),
                    0
                );
                assert!(!frame_system::Account::<Runtime>::contains_key(old()));
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                    AMOUNT
                );
                assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), AMOUNT);
                assert_eq!(
                    crate::BridgeProxy::locked_assets(operation.network(), common::DAI),
                    AMOUNT
                );
                assert_eq!(
                    crate::BridgeProxy::sender(operation.network(), entry.id),
                    Some(replacement())
                );
                assert!(
                    crate::BridgeProxy::transactions((operation.network(), old()), entry.id)
                        .is_none()
                );
                let request = crate::BridgeProxy::transactions(
                    (operation.network(), replacement()),
                    entry.id,
                )
                .unwrap();
                assert_eq!(request.amount(), AMOUNT);
                assert_eq!(request.source(), &entry.sender);
                assert_eq!(request.status(), MessageStatus::Done);
                assert_eq!(request.direction(), MessageDirection::Inbound);
                let receipt =
                    sp_io::storage::get(&receipt_key(operation.network(), entry.id)).unwrap();
                let (_, _, _, consumed): ([u8; 32], [u8; 32], [u8; 32], ExpectedTransfer) =
                    decode_bounded(&receipt).unwrap();
                assert_eq!(consumed.original, old());
                assert_eq!(consumed.beneficiary, replacement());
                assert_eq!(consumed.payload_hash, entry.payload_hash);
                assert!(consumed.delivered && !consumed.excluded);
                if matches!(kind, ChannelKind::Substrate) {
                    assert_eq!(
                        substrate_bridge_app::DeferredTransferResults::<Runtime>::get(
                            SubNetworkId::Liberland,
                            entry.id
                        ),
                        Some(MessageStatus::Done)
                    );
                    assert_eq!(
                        substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(
                            SubNetworkId::Liberland
                        ),
                        0
                    );
                }
                // A repeated manifest entry also reaches the native next-nonce gate:
                // it cannot mint twice even with a fresh valid recovery cursor.
                let mut repeated = operation.clone();
                repeated.index = 1;
                let mut repeated_policy = policy.clone();
                repeated_policy.chunk_hashes.push(repeated.commitment());
                sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &repeated_policy.commitment());
                let mut status = recovery::storage::status().unwrap().unwrap();
                status.phase = RecoveryPhase::Applying;
                sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
                assert_eq!(
                    apply_operation(&repeated_policy, &repeated),
                    Err(RecoveryError::WrongPreimage)
                );
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                    AMOUNT
                );
                assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), AMOUNT);
                assert_eq!(
                    sp_io::storage::get(&receipt_key(operation.network(), entry.id)).unwrap(),
                    receipt
                );
            });
        });
    }
}

#[test]
fn recovery_channels_disabled_ton_rejects_authenticated_historical_receipt() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation) = fixture(ChannelKind::Ton, old());
        let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
        let app = crate::JettonApp::app_info();
        let token = crate::JettonApp::token_address(common::DAI);
        assert!(app.is_some() && token.is_some());
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            let before = batch_storage_without_status();
            assert_eq!(
                apply_operation(&policy, &operation),
                Err(RecoveryError::WrongPreimage)
            );
            assert_eq!(native_nonce(&operation), 0);
            assert_no_credit(&operation, &entry);
            assert_eq!(batch_storage_without_status(), before);
            assert_eq!(crate::JettonApp::app_info(), app);
            assert_eq!(crate::JettonApp::token_address(common::DAI), token);
            assert!(!sp_io::storage::exists(&receipt_key(
                operation.network(),
                entry.id
            )));
            assert!(!sp_io::storage::exists(ACTIVE_KEY));
            assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
        });
    });
}

#[test]
fn recovery_channels_bad_authentication_cannot_create_old_or_replacement_credit() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = fixture(kind, old());
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            let proof = match &mut operation.channel {
                IncomingChannel::Substrate { proof, .. }
                | IncomingChannel::Generic { proof, .. } => proof,
                IncomingChannel::Hashi { .. }
                | IncomingChannel::HashiAuthenticatedSource { .. } => unreachable!(),
            };
            match proof {
                MultiProof::Multisig(proof) => proof.proof.clear(),
                MultiProof::EVMMultisig(proof) => proof.proof.clear(),
                #[cfg(any(feature = "wip", feature = "runtime-benchmarks"))]
                _ => unreachable!(),
            }
            policy.chunk_hashes[0] = operation.commitment();
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_eq!(
                    apply_operation(&policy, &operation),
                    Err(RecoveryError::WrongPreimage)
                );
                assert_eq!(native_nonce(&operation), 0);
                assert_no_credit(&operation, &entry);
                assert!(!sp_io::storage::exists(&receipt_key(
                    operation.network(),
                    entry.id
                )));
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Failed
                );
            });
        });
    }
}

#[test]
fn recovery_channels_authenticated_attacker_exclusion_consumes_without_credit() {
    for kind in kinds() {
        for exclude_source in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let recipient = if exclude_source {
                    old()
                } else {
                    AccountId::new(recovery::ATTACKER_ACCOUNT)
                };
                let (mut policy, mut operation) = fixture(kind, recipient.clone());
                if exclude_source {
                    let sender = prepare(&operation, &policy)
                        .unwrap()
                        .transfers
                        .remove(0)
                        .sender;
                    operation.registries[0]
                        .accounts
                        .push(normalized_account(&sender).unwrap());
                    operation.registries[0].no_known_attackers = false;
                    policy.chunk_hashes[0] = operation.commitment();
                }
                let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
                assert!(entry.excluded);
                let signed_bytes = operation.encode();
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    assert_ok!(apply_operation(&policy, &operation));
                    assert_eq!(operation.encode(), signed_bytes);
                    assert_eq!(native_nonce(&operation), 1);
                    assert_no_credit(&operation, &entry);
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &recipient).unwrap(),
                        0
                    );
                    assert!(!frame_system::Account::<Runtime>::contains_key(&recipient));
                    let receipt =
                        sp_io::storage::get(&receipt_key(operation.network(), entry.id)).unwrap();
                    let (_, _, _, consumed): ([u8; 32], [u8; 32], [u8; 32], ExpectedTransfer) =
                        decode_bounded(&receipt).unwrap();
                    assert!(consumed.delivered && consumed.excluded);
                    if matches!(kind, ChannelKind::Substrate) {
                        assert_eq!(
                            substrate_bridge_app::DeferredTransferResults::<Runtime>::get(
                                SubNetworkId::Liberland,
                                entry.id
                            ),
                            Some(MessageStatus::Done)
                        );
                    }
                });
            });
        }
    }
}

#[test]
fn recovery_thischain_exclusion_preserves_pooled_custody_and_consumes_claim_once() {
    use bridge_types::traits::BridgeAssetLocker;

    // Exclusion extinguishes this authenticated claim. It does not prove title
    // to pooled checkpoint backing and must not be reported as a token burn.
    for exclude_source in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let recipient = if exclude_source {
                old()
            } else {
                AccountId::new(recovery::ATTACKER_ACCOUNT)
            };
            let (mut policy, mut operation) = fixture_with_asset_kind(
                ChannelKind::Substrate,
                recipient.clone(),
                AssetKind::Thischain,
            );
            let network = SubNetworkId::Liberland;
            assert_eq!(
                crate::SubstrateBridgeApp::asset_kind(network, common::DAI),
                Some(AssetKind::Thischain)
            );

            let innocent = AccountId::new([46; 32]);
            let custody = crate::BridgeProxy::bridge_account(network.into()).unwrap();
            let backing = 5 * AMOUNT;
            let liquid = 2 * AMOUNT;
            System::inc_providers(&innocent);
            assert_ok!(crate::Assets::mint_to(
                &common::DAI,
                &custody,
                &innocent,
                backing + liquid,
            ));
            // Populate the real technical custody account and LockedAssets by
            // the ordinary Thischain lock routine before maintenance starts.
            assert_ok!(
                <crate::BridgeProxy as BridgeAssetLocker<AccountId>>::lock_asset(
                    network.into(),
                    AssetKind::Thischain,
                    &innocent,
                    &common::DAI,
                    &backing,
                )
            );
            if exclude_source {
                let sender = prepare(&operation, &policy)
                    .unwrap()
                    .transfers
                    .remove(0)
                    .sender;
                operation.registries[0]
                    .accounts
                    .push(normalized_account(&sender).unwrap());
                operation.registries[0].no_known_attackers = false;
                policy.chunk_hashes[0] = operation.commitment();
            }
            let entry = prepare(&operation, &policy).unwrap().transfers.remove(0);
            assert!(entry.excluded);
            let signed_bytes = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(operation.encode(), signed_bytes);
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(
                    crate::Assets::total_issuance(&common::DAI).unwrap(),
                    backing + liquid
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &custody).unwrap(),
                    backing
                );
                assert_eq!(
                    crate::BridgeProxy::locked_assets(GenericNetworkId::from(network), common::DAI),
                    backing
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &innocent).unwrap(),
                    liquid
                );
                for account in [recipient.clone(), old(), replacement()] {
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &account).unwrap(),
                        0
                    );
                    assert!(!frame_system::Account::<Runtime>::contains_key(account));
                }
                assert!(
                    crate::BridgeProxy::sender(GenericNetworkId::from(network), entry.id).is_none()
                );
                assert!(crate::BridgeProxy::transactions(
                    (GenericNetworkId::from(network), entry.beneficiary.clone()),
                    entry.id
                )
                .is_none());
                assert_eq!(
                    substrate_bridge_app::DeferredTransferResults::<Runtime>::get(
                        network, entry.id
                    ),
                    Some(MessageStatus::Done)
                );
                assert_eq!(
                    substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(network),
                    0
                );
                assert!(
                    substrate_bridge_channel::outbound::Pallet::<Runtime>::recovery_queue(network)
                        .is_empty()
                );
                let receipt = sp_io::storage::get(&receipt_key(network.into(), entry.id)).unwrap();
                let (_, _, _, consumed): ([u8; 32], [u8; 32], [u8; 32], ExpectedTransfer) =
                    decode_bounded(&receipt).unwrap();
                assert!(consumed.delivered && consumed.excluded);
                assert_eq!(consumed.original, entry.original);
                assert_eq!(consumed.beneficiary, entry.beneficiary);

                // Even a fresh authorized work cursor cannot consume this
                // identity twice or replace its ACK/custody/issuance evidence.
                let mut repeated = operation.clone();
                repeated.index = 1;
                let mut repeated_policy = policy.clone();
                repeated_policy.chunk_hashes.push(repeated.commitment());
                sp_io::storage::set(ACTIVE_POLICY_STORAGE_KEY, &repeated_policy.commitment());
                let mut status = recovery::storage::status().unwrap().unwrap();
                status.phase = RecoveryPhase::Applying;
                sp_io::storage::set(STATUS_STORAGE_KEY, &status.encode());
                let before_repeat = batch_storage_without_status();
                assert_eq!(
                    apply_operation(&repeated_policy, &repeated),
                    Err(RecoveryError::WrongPreimage)
                );
                assert_eq!(batch_storage_without_status(), before_repeat);
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(
                    sp_io::storage::get(&receipt_key(network.into(), entry.id)).unwrap(),
                    receipt
                );
            });
        });
    }
}

/// Capture every non-status top-storage value: a failed committed chunk may record
/// Failed, but no earlier message's financial, query, nonce, event or ACK writes.
fn batch_storage_without_status() -> alloc::collections::BTreeMap<Vec<u8>, Vec<u8>> {
    let mut out = alloc::collections::BTreeMap::new();
    let mut last = Vec::new();
    while let Some(key) = sp_io::storage::next_key(&last) {
        if key.as_slice() != STATUS_STORAGE_KEY {
            out.insert(key.clone(), sp_io::storage::get(&key).unwrap().to_vec());
        }
        last = key;
    }
    out
}

#[test]
fn recovery_channels_late_application_failure_rolls_back_prior_mapped_credit_and_ack() {
    for kind in [ChannelKind::Substrate, ChannelKind::Parachain] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = fixture(kind, old());
            let IncomingChannel::Substrate {
                commitment: GenericCommitment::Sub(batch),
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let first = batch.messages[0].clone();
            batch.messages.try_push(first).unwrap();
            sign(&mut operation.channel);
            policy.chunk_hashes[0] = operation.commitment();
            let good = prepare(&operation, &policy).unwrap();
            assert_eq!(good.transfers.len(), 2);
            // First prove both original messages are independently admissible.
            // This outer test transaction removes the control run completely.
            sp_io::storage::start_transaction();
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                    AMOUNT * 2
                );
                assert_eq!(
                    crate::Assets::total_issuance(&common::DAI).unwrap(),
                    AMOUNT * 2
                );
                assert_eq!(native_nonce(&operation), 1);
                for entry in &good.transfers {
                    assert!(sp_io::storage::exists(&receipt_key(
                        operation.network(),
                        entry.id
                    )));
                }
                if matches!(kind, ChannelKind::Substrate) {
                    for entry in &good.transfers {
                        assert_eq!(
                            substrate_bridge_app::DeferredTransferResults::<Runtime>::get(
                                SubNetworkId::Liberland,
                                entry.id
                            ),
                            Some(MessageStatus::Done)
                        );
                    }
                }
            });
            sp_io::storage::rollback_transaction();

            // Only message 2 changes: its proof and asset descriptors remain valid,
            // but its mint overflows after the first message's mapped credit and
            // deferred ACK have been written. Preflight must not replace this
            // actual late-application rollback check with an early registry error.
            let IncomingChannel::Substrate {
                commitment: GenericCommitment::Sub(batch),
                ..
            } = &mut operation.channel
            else {
                unreachable!()
            };
            let mut call = decode_call(&batch.messages[1].payload).unwrap();
            match &mut call {
                BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { amount, .. }) => {
                    *amount = GenericBalance::Substrate(u128::MAX)
                }
                BridgeCall::ParachainApp(ParachainAppCall::Transfer { amount, .. }) => {
                    *amount = u128::MAX
                }
                _ => unreachable!(),
            }
            batch.messages[1].payload = call.encode().try_into().unwrap();
            sign(&mut operation.channel);
            policy.chunk_hashes[0] = operation.commitment();
            let entries = prepare(&operation, &policy).unwrap().transfers;
            assert_eq!(entries[0].payload_hash, good.transfers[0].payload_hash);
            assert_ne!(entries[1].payload_hash, good.transfers[1].payload_hash);
            let signed_bytes = operation.encode();
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                let before = batch_storage_without_status();
                assert_eq!(
                    apply_operation(&policy, &operation),
                    Err(RecoveryError::WrongPreimage)
                );
                let status = recovery::storage::status().unwrap().unwrap();
                assert_eq!(status.phase, RecoveryPhase::Failed);
                assert_eq!(status.next_chunk, 0);
                assert_eq!(status.failure, Some(RecoveryError::WrongPreimage));
                assert_eq!(
                    batch_storage_without_status(),
                    before,
                    "{kind:?}: leaked partial batch writes"
                );
                assert_eq!(operation.encode(), signed_bytes);
                assert_eq!(native_nonce(&operation), 0);
                assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), 0);
                for entry in &entries {
                    assert_no_credit(&operation, entry);
                    assert!(!sp_io::storage::exists(&receipt_key(
                        operation.network(),
                        entry.id
                    )));
                }
                assert!(!sp_io::storage::exists(ACTIVE_KEY));
                assert!(!sp_io::storage::exists(FAILED_KEY));
                assert!(!sp_io::storage::exists(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY));
            });
        });
    }
}

#[test]
fn recovery_channels_typed_audits_allow_delay_and_consumption_registry_cleanup() {
    for kind in kinds() {
        for excluded in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let recipient = if excluded {
                    AccountId::new(recovery::ATTACKER_ACCOUNT)
                } else {
                    old()
                };
                let (mut policy, operation) = fixture(kind, recipient);
                let audit = IncomingAudit {
                    index: 0,
                    operation_index: operation.index,
                    operation_commitment: operation.commitment(),
                    requests: expected_requests(&operation, &policy).unwrap(),
                    prior_policy: None,
                };
                policy.audit_chunk_hashes = vec![audit.commitment()];
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    System::set_block_number(System::block_number() + 52);
                    apply_operation(&policy, &operation).unwrap();
                    assert_eq!(native_nonce(&operation), 1);
                    // Registry cleanup is permitted after consumption. Audit must use
                    // the independently derived consumption-time descriptor instead.
                    let pallet = match kind {
                        ChannelKind::Substrate => b"SubstrateBridgeApp".as_slice(),
                        ChannelKind::Parachain => b"ParachainBridgeApp".as_slice(),
                        ChannelKind::Ton => b"JettonApp".as_slice(),
                        #[cfg(feature = "wip")]
                        ChannelKind::Evm => b"EVMFungibleApp".as_slice(),
                    };
                    let prefix =
                        frame_support::storage::storage_prefix(pallet, b"SidechainPrecision");
                    let mut key = prefix.to_vec();
                    while let Some(next) = sp_io::storage::next_key(&key) {
                        if !next.starts_with(&prefix) {
                            break;
                        }
                        sp_io::storage::clear(&next);
                        key = next;
                    }
                    System::set_block_number(System::block_number() + 100);
                    audit_encoded_with_policy(&policy, &audit.encode()).unwrap();
                    assert_eq!(
                        sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
                            .unwrap()
                            .to_vec(),
                        0u32.encode()
                    );
                });
            });
        }
    }
}

fn set_substrate_messages(operation: &mut IncomingOperation, calls: Vec<BridgeCall>) {
    let IncomingChannel::Substrate {
        commitment: GenericCommitment::Sub(batch),
        ..
    } = &mut operation.channel
    else {
        panic!("Substrate fixture required");
    };
    batch.messages = calls
        .into_iter()
        .map(|call| BridgeMessage {
            payload: call.encode().try_into().unwrap(),
            timepoint: GenericTimepoint::Sora(2),
        })
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    sign(&mut operation.channel);
}

fn transfer_call(operation: &IncomingOperation) -> BridgeCall {
    let IncomingChannel::Substrate {
        commitment: GenericCommitment::Sub(batch),
        ..
    } = &operation.channel
    else {
        panic!("Substrate fixture required");
    };
    decode_call(&batch.messages[0].payload).unwrap()
}

fn make_audit(policy: &RecoveryPolicy, operation: &IncomingOperation, index: u32) -> IncomingAudit {
    IncomingAudit {
        index,
        operation_index: operation.index,
        operation_commitment: operation.commitment(),
        requests: expected_requests(operation, policy).unwrap(),
        prior_policy: None,
    }
}

#[test]
fn recovery_channels_mixed_controls_are_journaled_without_refunds_or_registration_effects() {
    use bridge_types::substrate::DataSignerCall;
    for tamper in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = fixture(ChannelKind::Substrate, old());
            let transfer = transfer_call(&operation);
            let result_id = H256::repeat_byte(99);
            let peer = ecdsa::Pair::from_seed(&[9; 32]).public();
            set_substrate_messages(
                &mut operation,
                vec![
                    BridgeCall::SubstrateApp(SubstrateAppCall::ReportTransferResult {
                        message_id: result_id,
                        message_status: MessageStatus::Failed,
                    }),
                    BridgeCall::SubstrateApp(SubstrateAppCall::RegisterAsset {
                        asset_id: GenericAssetId::Sora(common::DAI.into()),
                        sidechain_asset: GenericAssetId::Sora(common::DAI.into()),
                    }),
                    transfer,
                    BridgeCall::DataSigner(DataSignerCall::AddPeer { peer }),
                ],
            );
            policy.chunk_hashes = vec![operation.commitment()];
            let expected = prepare(&operation, &policy).unwrap();
            assert_eq!(expected.transfers.len(), 1);
            assert_eq!(expected.controls.len(), 3);
            let audit = make_audit(&policy, &operation, 0);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                <crate::BridgeProxy as bridge_types::traits::MessageStatusNotifier<
                    crate::AssetId,
                    AccountId,
                    crate::Balance,
                >>::outbound_request(
                    operation.network(),
                    result_id,
                    replacement(),
                    GenericAccount::Liberland(AccountId::new([16; 32])),
                    common::DAI,
                    AMOUNT,
                    MessageStatus::Committed,
                );
                let outgoing_before = crate::BridgeProxy::transactions(
                    (operation.network(), replacement()),
                    result_id,
                )
                .unwrap()
                .encode();
                let initial_registration =
                    crate::SubstrateBridgeApp::asset_kind(SubNetworkId::Liberland, common::DAI);
                let initial_precision = crate::SubstrateBridgeApp::sidechain_precision(
                    SubNetworkId::Liberland,
                    common::DAI,
                );
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                    AMOUNT
                );
                assert_eq!(
                    crate::Assets::free_balance(&common::DAI, &old()).unwrap(),
                    0
                );
                assert_eq!(
                    crate::SubstrateBridgeApp::asset_kind(SubNetworkId::Liberland, common::DAI),
                    initial_registration
                );
                assert_eq!(
                    crate::SubstrateBridgeApp::sidechain_precision(
                        SubNetworkId::Liberland,
                        common::DAI
                    ),
                    initial_precision
                );
                assert_eq!(
                    crate::BridgeProxy::transactions(
                        (operation.network(), replacement()),
                        result_id
                    )
                    .unwrap()
                    .encode(),
                    outgoing_before
                );
                assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), AMOUNT);
                assert!(crate::SubstrateBridgeOutboundChannel::recovery_queue(
                    SubNetworkId::Liberland
                )
                .is_empty());
                assert_eq!(
                    substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(
                        SubNetworkId::Liberland
                    ),
                    0
                );
                for expected_control in &expected.controls {
                    let bytes = sp_io::storage::get(&control_receipt_key(
                        expected_control.network,
                        expected_control.id,
                    ))
                    .unwrap();
                    let (_, _, _, actual): ([u8; 32], [u8; 32], [u8; 32], ExpectedControl) =
                        decode_bounded(&bytes).unwrap();
                    assert!(actual.delivered);
                    assert_eq!(actual.payload, expected_control.payload);
                    assert_eq!(actual.disposition, ControlDisposition::Deferred);
                    assert_eq!(actual.outcome, Some(ControlOutcome::Deferred));
                }
                if tamper {
                    sp_io::storage::clear(&control_receipt_key(
                        expected.controls[0].network,
                        expected.controls[0].id,
                    ));
                    assert!(audit_encoded_with_policy(&policy, &audit.encode()).is_err());
                    assert_eq!(
                        sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
                            .unwrap()
                            .to_vec(),
                        1u32.encode()
                    );
                } else {
                    assert_ok!(audit_encoded_with_policy(&policy, &audit.encode()));
                }
            });
        });
    }
}

fn sign_existing_peers(channel: &mut IncomingChannel, seeds: &[u8]) {
    let IncomingChannel::Substrate {
        commitment, proof, ..
    } = channel
    else {
        panic!("Substrate fixture required");
    };
    let digest = AuxiliaryDigest {
        logs: vec![AuxiliaryDigestItem::Commitment(
            ThisNetworkId::get(),
            commitment.hash(),
        )],
    };
    let hash = Keccak256::hash_of(&digest);
    *proof = MultiProof::Multisig(multisig_verifier::Proof {
        digest,
        proof: seeds
            .iter()
            .map(|seed| ecdsa::Pair::from_seed(&[*seed; 32]).sign_prehashed(hash.as_fixed_bytes()))
            .collect(),
    });
}

#[test]
fn recovery_channels_control_only_peer_rotation_preserves_next_batch_authentication() {
    use bridge_types::substrate::{DataSignerCall, MultisigVerifierCall};
    for old_keys in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = fixture(ChannelKind::Substrate, old());
            let transfer = transfer_call(&operation);
            let new_peer = ecdsa::Pair::from_seed(&[9; 32]).public();
            let retired_peer = ecdsa::Pair::from_seed(&[1; 32]).public();
            set_substrate_messages(
                &mut operation,
                vec![
                    BridgeCall::MultisigVerifier(MultisigVerifierCall::AddPeer { peer: new_peer }),
                    BridgeCall::MultisigVerifier(MultisigVerifierCall::RemovePeer {
                        peer: retired_peer,
                    }),
                ],
            );
            let expected = prepare(&operation, &policy).unwrap();
            assert!(expected.transfers.is_empty());
            assert_eq!(expected.controls.len(), 2);
            let mut next = operation.clone();
            next.index = 1;
            set_substrate_messages(&mut next, vec![transfer]);
            let IncomingChannel::Substrate {
                commitment: GenericCommitment::Sub(batch),
                ..
            } = &mut next.channel
            else {
                unreachable!()
            };
            batch.nonce = 2;
            sign_existing_peers(
                &mut next.channel,
                if old_keys {
                    &[1, 2, 3, 4]
                } else {
                    &[2, 3, 4, 9]
                },
            );
            policy.chunk_hashes = vec![operation.commitment(), next.commitment()];
            let first_audit = make_audit(&policy, &operation, 0);
            assert!(first_audit.requests.is_empty());
            let next_audit = make_audit(&policy, &next, 1);
            policy.audit_chunk_hashes = vec![first_audit.commitment(), next_audit.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                assert_ok!(apply_operation(&policy, &operation));
                let peers =
                    multisig_verifier::PeerKeys::<Runtime>::get(operation.network()).unwrap();
                assert!(peers.contains(&new_peer));
                assert!(!peers.contains(&retired_peer));
                for (entry, ack) in expected.controls.iter().zip([
                    DataSignerCall::AddPeer { peer: new_peer },
                    DataSignerCall::RemovePeer { peer: retired_peer },
                ]) {
                    assert_eq!(
                        sp_io::storage::get(&multisig_verifier::recovery_peer_ack_key(
                            SubNetworkId::Liberland,
                            entry.id
                        ))
                        .unwrap()
                        .to_vec(),
                        ack.encode()
                    );
                }
                assert!(crate::SubstrateBridgeOutboundChannel::recovery_queue(
                    SubNetworkId::Liberland
                )
                .is_empty());
                assert_eq!(
                    substrate_bridge_channel::outbound::ChannelNonces::<Runtime>::get(
                        SubNetworkId::Liberland
                    ),
                    0
                );
                System::set_block_number(System::block_number() + 1);
                if old_keys {
                    assert!(apply_operation(&policy, &next).is_err());
                    assert_eq!(native_nonce(&next), 1);
                    assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), 0);
                } else {
                    assert_ok!(apply_operation(&policy, &next));
                    assert_eq!(native_nonce(&next), 2);
                    assert_eq!(
                        crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                        AMOUNT
                    );
                    assert_ok!(audit_encoded_with_policy(&policy, &first_audit.encode()));
                    assert_ok!(audit_encoded_with_policy(&policy, &next_audit.encode()));
                }
            });
        });
    }
}

fn control_result(entry: &ExpectedControl) -> ExpectedControl {
    let bytes = sp_io::storage::get(&control_receipt_key(entry.network, entry.id)).unwrap();
    let (_, _, _, actual): ([u8; 32], [u8; 32], [u8; 32], ExpectedControl) =
        decode_bounded(&bytes).unwrap();
    actual
}

#[test]
fn recovery_channels_failed_peer_control_consumes_and_preserves_later_valid_messages() {
    use bridge_types::substrate::MultisigVerifierCall;
    for forged_ack in [false, true] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = fixture(ChannelKind::Substrate, old());
            let transfer = transfer_call(&operation);
            set_substrate_messages(
                &mut operation,
                vec![
                    BridgeCall::MultisigVerifier(MultisigVerifierCall::AddPeer {
                        peer: ecdsa::Pair::from_seed(&[1; 32]).public(),
                    }),
                    transfer,
                    BridgeCall::SubstrateApp(SubstrateAppCall::ReportTransferResult {
                        message_id: H256::repeat_byte(99),
                        message_status: MessageStatus::Failed,
                    }),
                ],
            );
            policy.chunk_hashes = vec![operation.commitment()];
            let expected = prepare(&operation, &policy).unwrap();
            let audit = make_audit(&policy, &operation, 0);
            policy.audit_chunk_hashes = vec![audit.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                let peers = multisig_verifier::PeerKeys::<Runtime>::get(operation.network());
                assert_ok!(apply_operation(&policy, &operation));
                assert_eq!(native_nonce(&operation), 1);
                assert_eq!(multisig_verifier::PeerKeys::<Runtime>::get(operation.network()), peers);
                assert_eq!(crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(), AMOUNT);
                assert_eq!(control_result(&expected.controls[0]).outcome, Some(ControlOutcome::Failed));
                assert_eq!(control_result(&expected.controls[1]).outcome, Some(ControlOutcome::Deferred));
                let ack_key = multisig_verifier::recovery_peer_ack_key(SubNetworkId::Liberland, expected.controls[0].id);
                assert!(!sp_io::storage::exists(&ack_key));
                assert!(!sp_io::storage::exists(FAILED_KEY));
                // Native dispatch preserves the actual application error event.
                assert!(System::events().iter().any(|record| matches!(&record.event,
                    crate::RuntimeEvent::SubstrateDispatch(dispatch::Event::MessageDispatched(id, Err(_)))
                        if id.hash() == expected.controls[0].id)));
                if forged_ack {
                    sp_io::storage::set(&ack_key, &bridge_types::substrate::DataSignerCall::AddPeer {
                        peer: ecdsa::Pair::from_seed(&[1; 32]).public(),
                    }.encode());
                    assert!(audit_encoded_with_policy(&policy, &audit.encode()).is_err());
                } else {
                    assert_ok!(audit_encoded_with_policy(&policy, &audit.encode()));
                    assert!(apply_operation(&policy, &operation).is_err());
                    assert_eq!(native_nonce(&operation), 1);
                }
            });
        });
    }
}

fn finalization_call(kind: ChannelKind) -> BridgeCall {
    match kind {
        ChannelKind::Substrate => {
            BridgeCall::SubstrateApp(SubstrateAppCall::FinalizeAssetRegistration {
                asset_id: GenericAssetId::Sora(common::DAI.into()),
                sidechain_asset: GenericAssetId::Sora(common::DAI.into()),
                asset_kind: AssetKind::Sidechain,
                precision: 18,
            })
        }
        ChannelKind::Parachain => {
            BridgeCall::ParachainApp(ParachainAppCall::FinalizeAssetRegistration {
                asset_id: common::DAI.into(),
                asset_kind: AssetKind::Sidechain,
            })
        }
        _ => unreachable!(),
    }
}

fn remove_registration(kind: ChannelKind) {
    // Both native pallets use Identity for these two map keys. Keep their
    // implementation-private aliases private; remove only this fixture's rows.
    let clear = |pallet: &[u8], item: &[u8], network: SubNetworkId, asset: Vec<u8>| {
        let mut key = frame_support::storage::storage_prefix(pallet, item).to_vec();
        key.extend(network.encode());
        key.extend(asset);
        sp_io::storage::clear(&key);
    };
    match kind {
        ChannelKind::Substrate => {
            for item in [
                b"AssetKinds".as_slice(),
                b"SidechainPrecision",
                b"SidechainAssetId",
            ] {
                clear(
                    b"SubstrateBridgeApp",
                    item,
                    SubNetworkId::Liberland,
                    common::DAI.encode(),
                );
            }
            clear(
                b"SubstrateBridgeApp",
                b"ThischainAssetId",
                SubNetworkId::Liberland,
                GenericAssetId::Sora(common::DAI.into()).encode(),
            );
            assert!(
                crate::SubstrateBridgeApp::asset_kind(SubNetworkId::Liberland, common::DAI)
                    .is_none()
            );
            assert_ok!(crate::SubstrateBridgeApp::authorize_asset_registration(
                RuntimeOrigin::root(),
                SubNetworkId::Liberland,
                common::DAI,
                GenericAssetId::Sora(common::DAI.into()),
                AssetKind::Sidechain,
            ));
        }
        ChannelKind::Parachain => {
            // Native finalization requires the preceding registration's precision.
            clear(
                b"ParachainBridgeApp",
                b"AssetKinds",
                SubNetworkId::Polkadot,
                common::DAI.encode(),
            );
            assert!(
                crate::ParachainBridgeApp::asset_kind(SubNetworkId::Polkadot, common::DAI)
                    .is_none()
            );
            // Reconstruct the locally authorized pending state from before the
            // fixture's initial finalization, alongside its retained precision.
            parachain_bridge_app::PendingAssetKinds::<Runtime>::insert(
                SubNetworkId::Polkadot,
                common::DAI,
                AssetKind::Sidechain,
            );
        }
        _ => unreachable!(),
    }
}

#[test]
fn recovery_channels_native_finalization_precedes_first_asset_transfer_in_same_batch() {
    for kind in [ChannelKind::Substrate, ChannelKind::Parachain] {
        for bad_proof in [false, true] {
            sp_io::TestExternalities::default().execute_with(|| {
                let (mut policy, mut operation) = fixture(kind, old());
                let transfer = transfer_call(&operation);
                remove_registration(kind);
                set_substrate_messages(&mut operation, vec![finalization_call(kind), transfer]);
                if bad_proof {
                    if let IncomingChannel::Substrate { proof, .. } = &mut operation.channel {
                        *proof = empty_proof();
                    }
                }
                policy.chunk_hashes = vec![operation.commitment()];
                let before = batch_storage_without_status();
                let audit = make_audit(&policy, &operation, 0);
                // Derivation invokes the actual finalization in a rollback layer:
                // neither registration nor events/context survive the preview.
                assert_eq!(batch_storage_without_status(), before);
                assert_eq!(audit.requests.len(), 1);
                assert_eq!(audit.requests[0].as_ref().unwrap().amount, AMOUNT);
                policy.audit_chunk_hashes = vec![audit.commitment()];
                let control = prepare(&operation, &policy).unwrap().controls.remove(0);
                crate::incident_recovery::with_test_policy(&policy, || {
                    recovery::storage::initialize(&policy).unwrap();
                    let before = batch_storage_without_status();
                    if bad_proof {
                        assert!(apply_operation(&policy, &operation).is_err());
                        assert_eq!(native_nonce(&operation), 0);
                        assert_eq!(batch_storage_without_status(), before);
                    } else {
                        assert_ok!(apply_operation(&policy, &operation));
                        assert_eq!(native_nonce(&operation), 1);
                        assert_eq!(
                            crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                            AMOUNT
                        );
                        assert_eq!(
                            crate::Assets::free_balance(&common::DAI, &old()).unwrap(),
                            0
                        );
                        assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), AMOUNT);
                        assert_eq!(
                            control_result(&control).outcome,
                            Some(ControlOutcome::Applied)
                        );
                        assert_ok!(audit_encoded_with_policy(&policy, &audit.encode()));
                    }
                });
            });
        }
    }
}

#[test]
fn recovery_channels_registration_preview_weight_covers_success_and_failure_paths() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (_, mut operation) = fixture(ChannelKind::Substrate, old());
        for kind in [ChannelKind::Substrate, ChannelKind::Parachain] {
            set_substrate_messages(&mut operation, vec![finalization_call(kind)]);
            let payload = finalization_call(kind).encode();
            let expected = SubstrateDispatch::dispatch_weight(&payload)
                .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().writes(1));
            assert_eq!(audit::registration_preview_weight(&operation), expected);
            assert!(expected
                .saturating_mul(3)
                .all_lte(operation.required_weight()));
        }
    });
}

#[test]
fn recovery_channels_native_registration_does_not_anticipate_a_later_message() {
    for kind in [ChannelKind::Substrate, ChannelKind::Parachain] {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation) = fixture(kind, old());
            let transfer = transfer_call(&operation);
            remove_registration(kind);
            set_substrate_messages(&mut operation, vec![transfer, finalization_call(kind)]);
            policy.chunk_hashes = vec![operation.commitment()];
            let before = batch_storage_without_status();
            assert!(expected_requests(&operation, &policy).is_err());
            assert_eq!(batch_storage_without_status(), before);
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                let before = batch_storage_without_status();
                assert!(apply_operation(&policy, &operation).is_err());
                assert_eq!(native_nonce(&operation), 0);
                assert_eq!(batch_storage_without_status(), before);
            });
        });
    }
}

#[test]
fn recovery_channels_failed_registration_consumes_while_following_valid_transfer_executes() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, mut operation) = fixture(ChannelKind::Substrate, old());
        let transfer = transfer_call(&operation);
        // Liberland has the Substrate asset registration, but no Parachain
        // precision prerequisite. The native Parachain finalization must fail.
        assert!(crate::ParachainBridgeApp::sidechain_precision(
            SubNetworkId::Liberland,
            common::DAI
        )
        .is_none());
        set_substrate_messages(
            &mut operation,
            vec![finalization_call(ChannelKind::Parachain), transfer],
        );
        policy.chunk_hashes = vec![operation.commitment()];
        let control = prepare(&operation, &policy).unwrap().controls.remove(0);
        let audit = make_audit(&policy, &operation, 0);
        policy.audit_chunk_hashes = vec![audit.commitment()];
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            assert_ok!(apply_operation(&policy, &operation));
            assert_eq!(native_nonce(&operation), 1);
            assert_eq!(
                control_result(&control).outcome,
                Some(ControlOutcome::Failed)
            );
            assert!(
                crate::ParachainBridgeApp::asset_kind(SubNetworkId::Liberland, common::DAI)
                    .is_none()
            );
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                AMOUNT
            );
            assert_ok!(audit_encoded_with_policy(&policy, &audit.encode()));
        });
    });
}

#[test]
fn recovery_channels_finalization_enables_a_later_authenticated_batch() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, mut operation) = fixture(ChannelKind::Substrate, old());
        let transfer = transfer_call(&operation);
        remove_registration(ChannelKind::Substrate);
        set_substrate_messages(
            &mut operation,
            vec![finalization_call(ChannelKind::Substrate)],
        );
        let mut next = operation.clone();
        next.index = 1;
        set_substrate_messages(&mut next, vec![transfer.clone()]);
        if let IncomingChannel::Substrate {
            commitment: GenericCommitment::Sub(batch),
            ..
        } = &mut next.channel
        {
            batch.nonce = 2;
        }
        sign_existing_peers(&mut next.channel, &[1, 2, 3, 4]);
        policy.chunk_hashes = vec![operation.commitment(), next.commitment()];
        let first = make_audit(&policy, &operation, 0);
        // Use the same ordered native preview to derive the later batch's request
        // with its prerequisite, while preserving the original operation identity.
        let mut preview = next.clone();
        set_substrate_messages(
            &mut preview,
            vec![finalization_call(ChannelKind::Substrate), transfer],
        );
        let next_audit = IncomingAudit {
            index: 1,
            operation_index: next.index,
            operation_commitment: next.commitment(),
            requests: expected_requests(&preview, &policy).unwrap(),
            prior_policy: None,
        };
        policy.audit_chunk_hashes = vec![first.commitment(), next_audit.commitment()];
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            assert_ok!(apply_operation(&policy, &operation));
            assert_eq!(native_nonce(&operation), 1);
            assert_eq!(crate::Assets::total_issuance(&common::DAI).unwrap(), 0);
            System::set_block_number(System::block_number() + 1);
            assert_ok!(apply_operation(&policy, &next));
            assert_eq!(native_nonce(&next), 2);
            assert_eq!(
                crate::Assets::free_balance(&common::DAI, &replacement()).unwrap(),
                AMOUNT
            );
            assert_ok!(audit_encoded_with_policy(&policy, &first.encode()));
            assert_ok!(audit_encoded_with_policy(&policy, &next_audit.encode()));
        });
    });
}

#[test]
fn recovery_channels_successful_peer_change_requires_its_deferred_ack() {
    use bridge_types::substrate::MultisigVerifierCall;
    sp_io::TestExternalities::default().execute_with(|| {
        let (mut policy, mut operation) = fixture(ChannelKind::Substrate, old());
        set_substrate_messages(
            &mut operation,
            vec![BridgeCall::MultisigVerifier(
                MultisigVerifierCall::AddPeer {
                    peer: ecdsa::Pair::from_seed(&[9; 32]).public(),
                },
            )],
        );
        policy.chunk_hashes = vec![operation.commitment()];
        let control = prepare(&operation, &policy).unwrap().controls.remove(0);
        let audit = make_audit(&policy, &operation, 0);
        policy.audit_chunk_hashes = vec![audit.commitment()];
        crate::incident_recovery::with_test_policy(&policy, || {
            recovery::storage::initialize(&policy).unwrap();
            assert_ok!(apply_operation(&policy, &operation));
            assert_eq!(
                control_result(&control).outcome,
                Some(ControlOutcome::Applied)
            );
            sp_io::storage::clear(&multisig_verifier::recovery_peer_ack_key(
                SubNetworkId::Liberland,
                control.id,
            ));
            assert!(audit_encoded_with_policy(&policy, &audit.encode()).is_err());
        });
    });
}

#[test]
fn recovery_registries_require_known_evm_attacker_on_every_evm_chain() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, template) = fixture(ChannelKind::Substrate, old());
        for network in [
            GenericNetworkId::EVM(H256::from_low_u64_be(1)),
            GenericNetworkId::EVM(H256::from_low_u64_be(42161)),
            GenericNetworkId::EVMLegacy(0),
            GenericNetworkId::EVMLegacy(9),
        ] {
            let mut operation = template.clone();
            operation.channel = match network {
                GenericNetworkId::EVMLegacy(network) => IncomingChannel::Hashi {
                    network,
                    request: vec![],
                    approvals: vec![],
                },
                _ => IncomingChannel::Generic {
                    network,
                    commitment: match &template.channel {
                        IncomingChannel::Substrate { commitment, .. } => commitment.clone(),
                        _ => unreachable!(),
                    },
                    proof: empty_proof(),
                },
            };
            operation.registries[0].network = network;
            operation.registries[0].accounts.clear();
            operation.registries[0].no_known_attackers = true;
            assert_eq!(
                validate_registries(&operation, &policy),
                Err(RecoveryError::WrongPreimage)
            );
            operation.registries[0].accounts = vec![GenericAccount::EVM(bridge_types::H160::from(
                recovery::ATTACKER_EVM,
            ))];
            operation.registries[0].no_known_attackers = false;
            assert_ok!(validate_registries(&operation, &policy));
        }
    });
}

#[test]
fn recovery_channels_unattributed_transfers_require_evidence_before_nonce_consumption() {
    for case in 0..3 {
        sp_io::TestExternalities::default().execute_with(|| {
            let kind = if case == 2 {
                ChannelKind::Parachain
            } else {
                ChannelKind::Substrate
            };
            let (mut policy, mut operation) = fixture(kind, old());
            let mut call = transfer_call(&operation);
            match &mut call {
                BridgeCall::SubstrateApp(SubstrateAppCall::Transfer { sender, .. }) => {
                    *sender = if case == 0 {
                        GenericAccount::Unknown
                    } else {
                        GenericAccount::Root
                    };
                }
                BridgeCall::ParachainApp(ParachainAppCall::Transfer { sender, .. }) => {
                    *sender = None
                }
                _ => unreachable!(),
            }
            set_substrate_messages(&mut operation, vec![call]);
            policy.chunk_hashes = vec![operation.commitment()];
            crate::incident_recovery::with_test_policy(&policy, || {
                recovery::storage::initialize(&policy).unwrap();
                let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                assert_eq!(
                    apply_operation(&policy, &operation),
                    Err(RecoveryError::WrongPreimage)
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
                assert_eq!(native_nonce(&operation), 0);
                assert_eq!(
                    recovery::storage::status().unwrap().unwrap().phase,
                    RecoveryPhase::Applying
                );
            });
        });
    }
}
