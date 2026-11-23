//! Consensus bridge operations ignore emergency/legacy halt markers after coherent recovery.

use crate::{
    AccountId, Balances, BridgeMultisig, Runtime, RuntimeCall, Signature, SignedExtra, System,
    UncheckedExtrinsic,
};
use codec::Encode;
use common::recovery;
use frame_support::traits::Currency;
use sp_core::{ecdsa, Pair, H256};
use sp_runtime::{generic::Era, traits::IdentifyAccount, MultiSigner};

pub(super) fn keys() -> Vec<ecdsa::Pair> {
    [81u8, 82, 83]
        .into_iter()
        .map(|byte| ecdsa::Pair::from_seed(&[byte; 32]))
        .collect()
}
pub(super) fn account(key: &ecdsa::Pair) -> AccountId {
    MultiSigner::Ecdsa(key.public()).into_account()
}

pub(super) fn policy() -> recovery::RecoveryPolicy {
    recovery::RecoveryPolicy {
        format_version: 2,
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: vec![],
        council: vec![AccountId::new([91; 32])],
        chunk_hashes: vec![[3; 32]],
        audit_chunk_hashes: vec![[4; 32]],
        authorization_domain: recovery::RecoveryDomain::public_fixture(),
        evidence_commitment: [6; 32],
        terminal_audit_commitment: [7; 32],
    }
}

pub(super) fn setup(policy: &recovery::RecoveryPolicy) -> (Vec<ecdsa::Pair>, AccountId) {
    System::set_block_number(recovery::CHECKPOINT_NUMBER + 100);
    frame_system::BlockHash::<Runtime>::insert(0, H256(recovery::GENESIS_HASH));
    let keys = keys();
    let mut peers: Vec<_> = keys.iter().map(account).collect();
    peers.sort();
    for peer in &peers {
        let _ = Balances::make_free_balance_be(peer, common::balance!(100));
    }
    let bridge = BridgeMultisig::register_multisig_inner(peers[0].clone(), peers.clone()).unwrap();
    eth_bridge::Peers::<Runtime>::insert(
        0,
        peers.into_iter().collect::<std::collections::BTreeSet<_>>(),
    );
    eth_bridge::BridgeAccount::<Runtime>::insert(0, &bridge);
    let commitment = policy.commitment();
    sp_io::storage::set(recovery::POLICY_STORAGE_KEY, &commitment);
    sp_io::storage::set(recovery::ACTIVE_POLICY_STORAGE_KEY, &commitment);
    sp_io::storage::set(recovery::RECOVERY_ANCHOR_STORAGE_KEY, &[12; 32]);
    sp_io::storage::set(
        recovery::STATUS_STORAGE_KEY,
        &recovery::RecoveryStatus {
            format_version: 1,
            policy: commitment,
            phase: recovery::RecoveryPhase::Complete,
            next_chunk: 1,
            operation_counter: 0,
            failure: None,
        }
        .encode(),
    );
    sp_io::storage::set(
        recovery::TERMINAL_AUDIT_STORAGE_KEY,
        &(commitment, policy.terminal_audit_commitment, 1u32).encode(),
    );
    sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &0u32.encode());
    sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
    for key in recovery::TRANSIENT_CONTEXT_KEYS {
        sp_io::storage::clear(key);
    }
    (keys, bridge)
}

pub(super) fn signed(call: RuntimeCall, key: &ecdsa::Pair) -> UncheckedExtrinsic {
    let who = account(key);
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
    let signature = payload.using_encoded(|payload| key.sign(payload));
    UncheckedExtrinsic::new_signed(call, who, Signature::Ecdsa(signature), extra)
}

#[cfg(test)]
mod retained_retry_admission {
    use super::*;
    use frame_support::weights::Weight;
    use frame_system::offchain::Signer;
    use sp_core::{crypto::KeyTypeId, ed25519, sr25519};
    use sp_keystore::{Error as KeystoreError, Keystore, KeystoreExt};
    use std::{collections::BTreeMap, str::FromStr};

    // Real RuntimeCall wrappers are exercised below. Any key enumeration or
    // signing access is a test failure, including before pool submission.
    struct NoKeyAccess;

    macro_rules! deny_key_access {
    ($(fn $name:ident($($arg:ident: $ty:ty),*) -> $result:ty;)*) => {
        $(fn $name(&self, $($arg: $ty),*) -> $result {
            $(let _ = $arg;)*
            panic!("rejected retry accessed keystore: {}", stringify!($name));
        })*
    };
}

    impl Keystore for NoKeyAccess {
        deny_key_access! {
            fn sr25519_public_keys(key_type: KeyTypeId) -> Vec<sr25519::Public>;
            fn sr25519_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<sr25519::Public, KeystoreError>;
            fn sr25519_sign(key_type: KeyTypeId, public: &sr25519::Public, msg: &[u8]) -> Result<Option<sr25519::Signature>, KeystoreError>;
            fn sr25519_vrf_sign(key_type: KeyTypeId, public: &sr25519::Public, data: &sr25519::vrf::VrfSignData) -> Result<Option<sr25519::vrf::VrfSignature>, KeystoreError>;
            fn sr25519_vrf_pre_output(key_type: KeyTypeId, public: &sr25519::Public, input: &sr25519::vrf::VrfInput) -> Result<Option<sr25519::vrf::VrfPreOutput>, KeystoreError>;
            fn ed25519_public_keys(key_type: KeyTypeId) -> Vec<ed25519::Public>;
            fn ed25519_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<ed25519::Public, KeystoreError>;
            fn ed25519_sign(key_type: KeyTypeId, public: &ed25519::Public, msg: &[u8]) -> Result<Option<ed25519::Signature>, KeystoreError>;
            fn ecdsa_public_keys(key_type: KeyTypeId) -> Vec<ecdsa::Public>;
            fn ecdsa_generate_new(key_type: KeyTypeId, seed: Option<&str>) -> Result<ecdsa::Public, KeystoreError>;
            fn ecdsa_sign(key_type: KeyTypeId, public: &ecdsa::Public, msg: &[u8]) -> Result<Option<ecdsa::Signature>, KeystoreError>;
            fn ecdsa_sign_prehashed(key_type: KeyTypeId, public: &ecdsa::Public, msg: &[u8; 32]) -> Result<Option<ecdsa::Signature>, KeystoreError>;
            fn insert(key_type: KeyTypeId, suri: &str, public: &[u8]) -> Result<(), ()>;
            fn keys(key_type: KeyTypeId) -> Result<Vec<Vec<u8>>, KeystoreError>;
            fn has_keys(public_keys: &[(Vec<u8>, KeyTypeId)]) -> bool;
        }
    }

    #[test]
    fn unrecognized_retained_runtime_calls_never_access_keys_nonce_pool_or_storage() {
        let mut ext = sp_io::TestExternalities::default();
        let (offchain, _) = sp_runtime::offchain::testing::TestOffchainExt::new();
        let (pool, pool_state) = sp_runtime::offchain::testing::TestTransactionPoolExt::new();
        ext.register_extension(sp_core::offchain::OffchainDbExt::new(offchain.clone()));
        ext.register_extension(sp_core::offchain::OffchainWorkerExt::new(offchain));
        ext.register_extension(sp_core::offchain::TransactionPoolExt::new(pool));
        ext.register_extension(KeystoreExt::new(NoKeyAccess));
        ext.execute_with(|| {
            let hash =
                H256::from_str("d3a1de97f04701cbd06c9e2381ede26625470cf139beef9914c9af2a8169142b")
                    .unwrap();
            // Same existence check that the node's exact seven-request host
            // policy supplies during OCW execution; no signing can bypass it
            // by wrapping a retained approval in an unrelated outer call.
            sp_io::storage::set(&bridge_types::recovery::hashi_key(0u32, hash), &[]);
            let blocked = RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
                ocw_public: ecdsa::Public::from_raw([2; 33]),
                hash,
                signature_params: eth_bridge::offchain::SignatureParams {
                    r: [0; 32],
                    s: [0; 32],
                    v: 27,
                },
                network_id: 0,
            });
            let remark = RuntimeCall::System(frame_system::Call::remark { remark: vec![1] });
            let id = AccountId::new([31; 32]);
            let calls = vec![
                blocked.clone(),
                RuntimeCall::Utility(pallet_utility::Call::batch {
                    calls: vec![blocked.clone()],
                }),
                RuntimeCall::Utility(pallet_utility::Call::batch_all {
                    calls: vec![blocked],
                }),
                remark.clone(),
                RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi_threshold_1 {
                    id: id.clone(),
                    call: Box::new(remark),
                    timepoint: Default::default(),
                }),
                RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                    id: id.clone(),
                    maybe_timepoint: None,
                    call: vec![],
                    store_call: true,
                    max_weight: Weight::zero(),
                }),
                RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
                    id: id.clone(),
                    maybe_timepoint: None,
                    call: vec![255],
                    store_call: true,
                    max_weight: Weight::zero(),
                }),
                RuntimeCall::BridgeMultisig(bridge_multisig::Call::approve_as_multi {
                    id: id.clone(),
                    maybe_timepoint: None,
                    call_hash: [33; 32],
                    max_weight: Weight::zero(),
                }),
            ];
            // Merely constructing a signer enumerates no keys. The old generic
            // fallback reached NoKeyAccess from resend for the Utility cases.
            let fallback =
                Signer::<Runtime, eth_bridge::offchain::crypto::TestAuthId>::any_account();
            for call in calls {
                let cache_hash = H256::repeat_byte(34);
                let mut cached = eth_bridge::offchain::SignedTransactionData::<Runtime>::new(
                    cache_hash, None, call,
                );
                let saved = BTreeMap::from([(cache_hash, &cached)]).encode();
                sp_io::offchain::local_storage_set(
                    sp_core::offchain::StorageKind::PERSISTENT,
                    eth_bridge::STORAGE_PENDING_TRANSACTIONS_KEY,
                    &saved,
                );
                let original = cached.encode();
                let chain = sp_io::storage::root(sp_runtime::StateVersion::V1);
                let nonce = System::account_nonce(&id);
                assert!(!cached.resend(&fallback));
                assert_eq!(cached.encode(), original);
                assert_eq!(System::account_nonce(&id), nonce);
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), chain);
                assert_eq!(
                    sp_io::offchain::local_storage_get(
                        sp_core::offchain::StorageKind::PERSISTENT,
                        eth_bridge::STORAGE_PENDING_TRANSACTIONS_KEY
                    ),
                    Some(saved)
                );
                assert!(pool_state.read().transactions.is_empty());
            }
        });
    }
}
