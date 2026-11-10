//! TON is disabled without deleting historical assets, channel nonces or proofs.
use crate::{
    BaseCallFilter, BridgeDataSigner, JettonApp, MultiProof, MultiVerifier, MultisigVerifier,
    Runtime, RuntimeCall, RuntimeOrigin, ThisNetworkId,
};
use bridge_types::{
    ton::{TonAddress, TonNetworkId},
    traits::{BridgeApp, Verifier},
    types::{AuxiliaryDigest, AuxiliaryDigestItem},
    GenericNetworkId, SubNetworkId,
};
use frame_support::{assert_noop, assert_ok, traits::Contains};
use sp_core::{ecdsa, Pair, H256};
use sp_runtime::traits::{Hash, Keccak256};

#[test]
fn ton_rejects_all_proofs_even_with_registered_peers_and_pending_approval() {
    sp_io::TestExternalities::default().execute_with(|| {
        let network = GenericNetworkId::TON(TonNetworkId::Mainnet);
        let signer = ecdsa::Pair::from_seed(&[21; 32]); // Public test fixture only.
        let peers = vec![signer.public()];
        assert_ok!(MultisigVerifier::force_set_peers(
            RuntimeOrigin::root(),
            network,
            peers.clone().try_into().unwrap()
        ));
        assert_ok!(BridgeDataSigner::force_set_peers(
            RuntimeOrigin::root(),
            network,
            peers.try_into().unwrap()
        ));
        let commitment = H256::repeat_byte(11);
        let hash = Keccak256::hash_of(&(network, ThisNetworkId::get(), commitment));
        assert_ok!(BridgeDataSigner::register_pending_approval(
            RuntimeOrigin::root(),
            network,
            hash
        ));
        let proof = MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof {
            proof: vec![signer.sign_prehashed(hash.as_fixed_bytes())],
        });
        assert_noop!(
            MultiVerifier::verify(network, commitment, &proof),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
        let digest = AuxiliaryDigest {
            logs: vec![AuxiliaryDigestItem::Commitment(
                ThisNetworkId::get(),
                commitment,
            )],
        };
        let proof = MultiProof::Multisig(multisig_verifier::Proof {
            proof: vec![signer.sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())],
            digest,
        });
        assert_noop!(
            MultiVerifier::verify(network, commitment, &proof),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
        #[cfg(feature = "runtime-benchmarks")]
        assert_noop!(
            MultiVerifier::verify(network, commitment, &MultiProof::Empty),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
    });
}

#[test]
fn ton_direct_calls_discovery_and_trait_transfers_are_disabled() {
    sp_io::TestExternalities::default().execute_with(|| {
        let account = crate::AccountId::new([23; 32]);
        let token = TonAddress::empty();
        let call = RuntimeCall::JettonApp(jetton_app::Call::mint {
            token: token.into(),
            sender: token.into(),
            recipient: account.clone(),
            amount: 1u128.into(),
        });
        assert!(!BaseCallFilter::contains(&call));
        assert_noop!(
            JettonApp::mint(
                RuntimeOrigin::none(),
                token.into(),
                token.into(),
                account.clone(),
                1u128.into()
            ),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
        assert_noop!(
            JettonApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                TonNetworkId::Mainnet,
                token,
                common::XOR.into(),
                9
            ),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
        assert!(JettonApp::list_apps().is_empty());
        assert!(
            JettonApp::list_supported_assets(GenericNetworkId::TON(TonNetworkId::Mainnet))
                .is_empty()
        );
        assert_noop!(
            JettonApp::transfer(
                GenericNetworkId::TON(TonNetworkId::Mainnet),
                common::XOR.into(),
                account.clone(),
                token,
                1
            ),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
        assert_noop!(
            JettonApp::refund(
                GenericNetworkId::TON(TonNetworkId::Mainnet),
                H256::zero(),
                account,
                common::XOR.into(),
                1
            ),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
    });
}

#[test]
fn remote_substrate_digest_does_not_need_a_local_ton_authorization() {
    sp_io::TestExternalities::default().execute_with(|| {
        let network = GenericNetworkId::Sub(SubNetworkId::Polkadot);
        let signer = ecdsa::Pair::from_seed(&[22; 32]); // Public test fixture only.
        assert_ok!(MultisigVerifier::force_set_peers(
            RuntimeOrigin::root(),
            network,
            vec![signer.public()].try_into().unwrap()
        ));
        let commitment = H256::repeat_byte(12);
        let digest = AuxiliaryDigest {
            logs: vec![AuxiliaryDigestItem::Commitment(
                ThisNetworkId::get(),
                commitment,
            )],
        };
        let proof = MultiProof::Multisig(multisig_verifier::Proof {
            proof: vec![signer.sign_prehashed(Keccak256::hash_of(&digest).as_fixed_bytes())],
            digest,
        });
        assert_ok!(MultiVerifier::verify(network, commitment, &proof));
    });
}

#[test]
fn ton_channel_submission_cannot_consume_a_nonce() {
    sp_io::TestExternalities::default().execute_with(|| {
        let network = TonNetworkId::Mainnet;
        let channel = TonAddress::new(0, H256::repeat_byte(31));
        assert_ok!(crate::BridgeInboundChannel::register_ton_channel(
            RuntimeOrigin::root(),
            network,
            channel
        ));
        let commitment = bridge_types::GenericCommitment::TON(
            bridge_types::ton::Commitment::Inbound(bridge_types::ton::InboundCommitment {
                channel,
                source: TonAddress::new(0, H256::repeat_byte(32)),
                nonce: 1,
                transaction_id: bridge_types::ton::TonTransactionId {
                    lt: 33,
                    hash: H256::repeat_byte(34),
                },
                payload: vec![1].try_into().unwrap(),
            }),
        );
        let proof = MultiProof::EVMMultisig(multisig_verifier::MultiEVMProof { proof: vec![] });
        assert_noop!(
            crate::BridgeInboundChannel::submit(
                RuntimeOrigin::none(),
                network.into(),
                commitment,
                proof
            ),
            jetton_app::Error::<Runtime>::BridgeDisabled
        );
        assert_eq!(
            bridge_channel::inbound::ChannelNonces::<Runtime>::get(GenericNetworkId::TON(network)),
            0
        );
    });
}
