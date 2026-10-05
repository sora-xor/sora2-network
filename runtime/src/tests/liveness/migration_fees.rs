// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use super::*;
use crate::{
    Assets, EthBridge, Executive, IrohaMigration, RuntimeCall, SignedExtra, UncheckedExtrinsic,
    XorFee,
};
use common::{AssetInfoProvider, VAL};
use ed25519_dalek_iroha::{Digest, Keypair, PublicKey, SecretKey};
use frame_support::{
    assert_err,
    dispatch::{GetDispatchInfo, Pays},
};
use sha3::Sha3_256;
use sp_core::sr25519;
use sp_runtime::{generic::Era, DispatchResult, FixedPointNumber, FixedU128};

const ADDRESS: &str = "paid-migration@sora";
const CLAIM: Balance = 300 * UNIT;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn raw_map(name: &[u8], key: impl Encode, value: impl Encode) {
    let encoded = key.encode();
    let mut storage_key = frame_support::storage::storage_prefix(b"IrohaMigration", name).to_vec();
    storage_key.extend(sp_core::blake2_128(&encoded));
    storage_key.extend(encoded);
    sp_io::storage::set(&storage_key, &value.encode());
}

fn migration_storage() -> Vec<(Vec<u8>, Vec<u8>)> {
    let prefix = sp_io::hashing::twox_128(b"IrohaMigration").to_vec();
    let mut key = prefix.clone();
    let mut state = Vec::new();
    while let Some(next) = sp_io::storage::next_key(&key) {
        if !next.starts_with(&prefix) {
            break;
        }
        state.push((next.clone(), sp_io::storage::get(&next).unwrap().to_vec()));
        key = next;
    }
    state
}

fn val_balance(who: &AccountId) -> Balance {
    Assets::free_balance(&VAL, who).unwrap()
}

fn fixture(fail_settlement: bool) -> (sr25519::Pair, AccountId, RuntimeCall) {
    System::set_block_number(1);
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1)
    ));
    let signer = sr25519::Pair::from_seed(&[79; 32]);
    let who = AccountId::from(signer.public());
    // Keep the account alive so zero-XOR rejection reaches fee admission,
    // rather than only failing CheckNonce for a missing account.
    System::inc_providers(&who);
    let escrow = EthBridge::bridge_account(0).unwrap();
    assert_ok!(Assets::update_balance(
        RuntimeOrigin::root(),
        escrow.clone(),
        VAL,
        CLAIM as i128,
    ));
    raw_map(b"Balances", ADDRESS.to_string(), CLAIM);
    let secret = SecretKey::from_bytes(&[71; 32]).unwrap();
    let key = Keypair {
        public: PublicKey::from(&secret),
        secret,
    };
    let public = hex(key.public.as_bytes());
    raw_map(
        b"PublicKeys",
        ADDRESS.to_string(),
        vec![(false, public.clone())],
    );
    let message = format!(
        "SORA2-IROHA-MIGRATION-V2\ngenesis_hash=0x{}\niroha_address={}\niroha_public_key={}\nsubstrate_account=0x{}",
        hex(&crate::IrohaMigrationGenesisHash::get().encode()), ADDRESS, public, hex(&who.encode()),
    );
    let mut digest = Sha3_256::default();
    digest.update(message.as_bytes());
    let signature = hex(&key.sign_prehashed(digest, None).unwrap().to_bytes());
    let call = RuntimeCall::IrohaMigration(iroha_migration::Call::migrate {
        iroha_address: ADDRESS.to_string(),
        iroha_public_key: public,
        iroha_signature: signature,
    });
    if fail_settlement {
        // Ownership and the backed VAL entitlement are valid. The referral
        // conflict fails after the VAL transfer, exercising atomic rollback.
        let referrer = AccountId::from([80; 32]);
        raw_map(
            b"Referrers",
            ADDRESS.to_string(),
            "existing-referrer@sora".to_string(),
        );
        raw_map(
            b"MigratedAccounts",
            "existing-referrer@sora".to_string(),
            referrer.clone(),
        );
        referrals::Referrers::<Runtime>::insert(&who, referrer);
    }
    assert!(IrohaMigration::needs_migration(&ADDRESS.to_string()));
    assert_eq!(Balances::free_balance(&who), 0);
    assert_eq!(val_balance(&who), 0);
    (signer, escrow, call)
}

fn wire(call: RuntimeCall, signer: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = AccountId::from(signer.public());
    let extra: SignedExtra = (
        frame_system::CheckSpecVersion::new(),
        frame_system::CheckTxVersion::new(),
        frame_system::CheckGenesis::new(),
        frame_system::CheckEra::from(Era::Immortal),
        frame_system::CheckNonce::from(System::account_nonce(&who)),
        frame_system::CheckWeight::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| signer.sign(bytes));
    UncheckedExtrinsic::new_signed(call, who, crate::Signature::Sr25519(signature), extra)
}

fn reset() {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    System::reset_events();
}

fn apply_paid(call: RuntimeCall, signer: &sr25519::Pair) -> DispatchResult {
    reset();
    assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
    let who = AccountId::from(signer.public());
    let extrinsic = wire(call, signer);
    let quote = XorFee::query_info(
        &extrinsic,
        &extrinsic.function,
        extrinsic.encoded_size() as u32,
    )
    .partial_fee;
    assert!(quote > 0);
    let before = Balances::free_balance(&who);
    let nonce = System::account_nonce(&who);
    let result = Executive::apply_extrinsic(extrinsic).expect("funded migration is admitted");
    let charged = before - Balances::free_balance(&who);
    assert!(
        charged > 0,
        "successful migrations must also retain their XOR fee"
    );
    assert!(charged <= quote);
    assert_eq!(System::account_nonce(&who), nonce + 1);
    let emitted_fee = System::events()
        .into_iter()
        .find_map(|record| match record.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who: fee_payer,
                    actual_fee,
                    tip,
                },
            ) if fee_payer == who => {
                assert_eq!(tip, 0);
                Some(actual_fee)
            }
            _ => None,
        })
        .expect("the payment extension emits the actual XOR fee");
    assert_eq!(emitted_fee, charged);
    result
}

fn assert_zero_xor_rejected(call: RuntimeCall, signer: &sr25519::Pair, escrow: &AccountId) {
    let who = AccountId::from(signer.public());
    let claim_before = migration_storage();
    let escrow_before = val_balance(escrow);
    let nonce_before = System::account_nonce(&who);
    assert_eq!(Balances::free_balance(&who), 0);
    for source in [
        TransactionSource::External,
        TransactionSource::Local,
        TransactionSource::InBlock,
    ] {
        reset();
        assert_err!(
            Executive::validate_transaction(source, wire(call.clone(), signer), Default::default()),
            InvalidTransaction::Payment,
        );
        assert_eq!(migration_storage(), claim_before);
        assert_eq!(val_balance(escrow), escrow_before);
        assert_eq!(val_balance(&who), 0);
        assert_eq!(System::account_nonce(&who), nonce_before);
    }
    reset();
    assert_err!(
        Executive::apply_extrinsic(wire(call, signer)),
        InvalidTransaction::Payment,
    );
    assert_eq!(migration_storage(), claim_before);
    assert_eq!(val_balance(escrow), escrow_before);
    assert_eq!(val_balance(&who), 0);
    assert_eq!(System::account_nonce(&who), nonce_before);
    assert_eq!(Balances::free_balance(&who), 0);
    assert!(System::events().is_empty());
}

#[test]
fn migration_success_delivers_val_and_keeps_xor_fee() {
    ext().execute_with(|| {
        let (signer, escrow, call) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&who, 100_000 * UNIT));
        let escrow_before = val_balance(&escrow);
        assert_ok!(apply_paid(call, &signer));
        assert_eq!(val_balance(&who), CLAIM);
        assert_eq!(val_balance(&escrow), escrow_before - CLAIM);
        assert!(!IrohaMigration::needs_migration(&ADDRESS.to_string()));
        assert!(System::events().iter().any(|record| matches!(
            &record.event,
            RuntimeEvent::IrohaMigration(iroha_migration::Event::Migrated(address, account))
                if address == ADDRESS && account == &who
        )));
    });
}

#[test]
fn migration_settlement_failure_keeps_fee_and_rolls_back_claim() {
    ext().execute_with(|| {
        let (signer, escrow, call) = fixture(true);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&who, 100_000 * UNIT));
        let claim_before = migration_storage();
        let escrow_before = val_balance(&escrow);
        assert_eq!(
            apply_paid(call, &signer).unwrap_err(),
            iroha_migration::Error::<Runtime>::ReferralMigrationFailed.into(),
        );
        assert_eq!(migration_storage(), claim_before);
        assert_eq!(val_balance(&escrow), escrow_before);
        assert_eq!(val_balance(&who), 0);
        assert!(IrohaMigration::needs_migration(&ADDRESS.to_string()));
        assert!(!System::events()
            .iter()
            .any(|record| matches!(record.event, RuntimeEvent::IrohaMigration(_))));
    });
}

#[test]
fn migration_requires_xor_before_valid_claim_can_execute() {
    ext().execute_with(|| {
        let (signer, escrow, call) = fixture(false);
        assert_zero_xor_rejected(call, &signer, &escrow);
    });
}

#[test]
fn migration_invalid_proof_and_replay_pay_without_duplicate_val() {
    ext().execute_with(|| {
        let (signer, escrow, call) = fixture(false);
        let who = AccountId::from(signer.public());
        drop(Balances::deposit_creating(&who, 100_000 * UNIT));
        let claim_before = migration_storage();
        let escrow_before = val_balance(&escrow);
        let mut invalid = call.clone();
        if let RuntimeCall::IrohaMigration(iroha_migration::Call::migrate {
            iroha_signature, ..
        }) = &mut invalid
        {
            *iroha_signature = "00".repeat(64);
        }
        assert_eq!(
            apply_paid(invalid, &signer).unwrap_err(),
            iroha_migration::Error::<Runtime>::SignatureVerificationFailed.into(),
        );
        assert_eq!(migration_storage(), claim_before);
        assert_eq!(val_balance(&escrow), escrow_before);
        assert_eq!(val_balance(&who), 0);
        assert_ok!(apply_paid(call.clone(), &signer));
        let completed = migration_storage();
        assert_eq!(
            apply_paid(call, &signer).unwrap_err(),
            iroha_migration::Error::<Runtime>::AccountAlreadyMigrated.into(),
        );
        assert_eq!(migration_storage(), completed);
        assert_eq!(val_balance(&who), CLAIM);
        assert_eq!(val_balance(&escrow), escrow_before - CLAIM);
        assert_eq!(System::account_nonce(&who), 3);
    });
}

#[test]
fn wrapped_migration_requires_xor_and_keeps_success_fee() {
    ext().execute_with(|| {
        let (signer, escrow, call) = fixture(false);
        let who = AccountId::from(signer.public());
        let wrapped = RuntimeCall::Utility(pallet_utility::Call::batch_all { calls: vec![call] });
        assert_zero_xor_rejected(wrapped.clone(), &signer, &escrow);
        drop(Balances::deposit_creating(&who, 100_000 * UNIT));
        let escrow_before = val_balance(&escrow);
        assert_ok!(apply_paid(wrapped, &signer));
        assert_eq!(val_balance(&who), CLAIM);
        assert_eq!(val_balance(&escrow), escrow_before - CLAIM);
        assert!(!IrohaMigration::needs_migration(&ADDRESS.to_string()));
    });
}
