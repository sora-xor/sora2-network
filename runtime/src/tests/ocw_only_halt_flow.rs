//! Local end-to-end runtime fixture. Every signature uses deterministic test
//! seeds; no production keystore, credentials, RPC or chain state is accessed.

use super::ocw_only_halt::{account, policy, setup, signed};
use crate::{
    AccountId, Assets, Balances, BridgeMultisig, BridgeProxy, Executive, Runtime, RuntimeCall,
    RuntimeEvent, System, XorFee,
};
use bridge_types::{types::MessageStatus, GenericAccount, GenericNetworkId};
use codec::Encode;
use common::{recovery, AssetInfoProvider, PSWAP, XOR};
use eth_bridge::requests::{
    AssetKind, IncomingRequest, IncomingTransactionRequestKind, IncomingTransfer,
    LoadIncomingRequest, LoadIncomingTransactionRequest, OffchainRequest, RequestStatus,
};
use frame_support::{
    assert_ok,
    dispatch::GetDispatchInfo,
    storage::storage_prefix,
    traits::{Currency, GetCallMetadata},
};
use framenode_chain_spec::ext;
use sp_core::{ecdsa, Pair, H160, H256};
use sp_runtime::FixedPointNumber;

fn network_key(item: &[u8]) -> Vec<u8> {
    [
        storage_prefix(b"EthBridge", item).to_vec(),
        sp_io::hashing::twox_64(&0u32.encode()).to_vec(),
        0u32.encode(),
    ]
    .concat()
}

fn prepare_asset_fixture(bridge: &AccountId, user: &AccountId) {
    // Fixture-only configuration of a V3 owned-asset bridge. The flow below
    // then goes through real calls, fees, signatures, custody and replay checks.
    sp_io::storage::set(
        &network_key(b"BridgeStatuses"),
        &eth_bridge::BridgeStatus::Initialized.encode(),
    );
    sp_io::storage::set(
        &network_key(b"BridgeSignatureVersions"),
        &eth_bridge::BridgeSignatureVersion::V3.encode(),
    );
    sp_io::storage::set(
        &network_key(b"BridgeContractAddress"),
        &H160::repeat_byte(66).encode(),
    );
    sp_io::storage::set(
        &[network_key(b"RegisteredAsset"), PSWAP.encode()].concat(),
        &AssetKind::Thischain.encode(),
    );
    sp_io::storage::clear(
        &[
            network_key(b"RegisteredSidechainToken"),
            sp_io::hashing::blake2_128(&PSWAP.encode()).to_vec(),
            PSWAP.encode(),
        ]
        .concat(),
    );
    sp_io::storage::clear(
        &[
            storage_prefix(b"BridgeProxy", b"LimitedAssets").to_vec(),
            sp_io::hashing::blake2_128(&PSWAP.encode()).to_vec(),
            PSWAP.encode(),
        ]
        .concat(),
    );
    let _ = Balances::make_free_balance_be(user, common::balance!(100));
    System::inc_providers(bridge);
    assert_ok!(Assets::mint_unchecked(&PSWAP, user, 1_000));
}

fn submit(call: RuntimeCall, key: &ecdsa::Pair) {
    // Each submission models its own otherwise-empty block's weight budget.
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    let who = account(key);
    let metadata = call.get_call_metadata();
    let label = format!("{}::{}", metadata.pallet_name, metadata.function_name);
    let extrinsic = signed(call.clone(), key);
    let quote = XorFee::compute_fee(
        extrinsic.encoded_size().try_into().unwrap(),
        &call,
        &call.get_dispatch_info(),
        0,
    )
    .0;
    let balance = Balances::free_balance(&who);
    let fee_exemption = crate::xor_fee_impls::validate_bridge_fee_exemption(&who, &call);
    let applied = Executive::apply_extrinsic(extrinsic).unwrap_or_else(|error| {
        panic!("{label} rejected before dispatch: {error:?}; payer={who:?}, native XOR balance={balance}, quoted fee={quote}, exemption={fee_exemption:?}")
    });
    assert!(applied.is_ok(), "{label} dispatch failed: {applied:?}");
}

fn multisig_call(
    bridge: &AccountId,
    inner: RuntimeCall,
    timepoint: bridge_multisig::BridgeTimepoint<u32>,
) -> RuntimeCall {
    let max_weight = inner.get_dispatch_info().total_weight();
    RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
        id: bridge.clone(),
        maybe_timepoint: Some(timepoint),
        call: inner.encode(),
        store_call: true,
        max_weight,
    })
}

fn import_call(
    bridge: &AccountId,
    author: &AccountId,
    recipient: &AccountId,
    from: H160,
    amount: u128,
    byte: u8,
) -> (RuntimeCall, H256, H256) {
    let tx_hash = H256::repeat_byte(byte);
    let timepoint = BridgeMultisig::sidechain_timepoint(u64::from(byte), 0);
    let incoming = IncomingRequest::Transfer(IncomingTransfer::<Runtime> {
        from,
        to: recipient.clone(),
        asset_id: PSWAP,
        asset_kind: AssetKind::Thischain,
        amount,
        author: author.clone(),
        tx_hash,
        at_height: u64::from(byte),
        timepoint,
        network_id: 0,
        should_take_fee: false,
    });
    let native_hash = OffchainRequest::<Runtime>::incoming(incoming.clone()).hash();
    let inner = RuntimeCall::EthBridge(eth_bridge::Call::import_incoming_request {
        load_incoming_request: LoadIncomingRequest::Transaction(
            LoadIncomingTransactionRequest::new(
                author.clone(),
                tx_hash,
                timepoint,
                IncomingTransactionRequestKind::Transfer,
                0,
            ),
        ),
        incoming_request_result: Ok(incoming),
    });
    (
        multisig_call(bridge, inner, timepoint),
        tx_hash,
        native_hash,
    )
}

#[test]
fn ocw_only_halt_signed_burn_approval_import_redirect_and_replay_without_resume_call() {
    let policy = policy();
    ext().execute_with(|| {
        crate::incident_recovery::with_test_policy(&policy, || {
            let (keys, bridge) = setup(&policy);
            let user_key = ecdsa::Pair::from_seed(&[84; 32]);
            let user = account(&user_key);
            let recipient = AccountId::new([85; 32]);
            let redirected = AccountId::new(recovery::ATTACKER_MIGRATION_ACCOUNT);
            for beneficiary in [&recipient, &redirected] {
                System::inc_providers(beneficiary);
            }
            prepare_asset_fixture(&bridge, &user);
            // Neither an emergency signal nor the retained legacy marker is a
            // consensus veto after coherent recovery. No operator ack, governance
            // or peer-quorum release transaction is created by this fixture.
            sp_io::storage::set(
                common::bridge_halt::GUARDIAN_HALT_STORAGE_KEY,
                &common::bridge_halt::GuardianHalt { number: 7, block: System::block_number() }.encode(),
            );
            assert!(bridge_types::recovery::completion().unwrap().is_some());
            assert!(sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            let network = GenericNetworkId::EVMLegacy(0);
            let locked = BridgeProxy::locked_assets(network, PSWAP);
            let issuance = Assets::total_issuance(&PSWAP).unwrap();
            let user_before = Assets::free_balance(&PSWAP, &user).unwrap();
            let custody_before = Assets::free_balance(&PSWAP, &bridge).unwrap();

            let burn = RuntimeCall::BridgeProxy(bridge_proxy::Call::burn {
                network_id: network,
                asset_id: PSWAP,
                recipient: GenericAccount::EVM(H160::repeat_byte(86)),
                amount: 100,
            });
            assert_eq!(crate::xor_fee_impls::validate_bridge_fee_exemption(&user, &burn), Ok(false));
            // This is an ordinary paid user transaction. The genesis fee
            // multiplier is intentionally preserved: 0.007 XOR * 600000,
            // plus encoded-byte fees, exceeds the old 100-XOR endowment.
            // Fund the exact quote and one XOR of reserve, not an arbitrary
            // large balance or a fee-exempt wrapper/multiplier override.
            let (fee_details, _) = XorFee::compute_fee_details(
                signed(burn.clone(), &user_key).encoded_size().try_into().unwrap(),
                &burn,
                &burn.get_dispatch_info(),
                0,
            );
            let inclusion = fee_details.inclusion_fee.as_ref().expect("paid burn inclusion fee");
            assert_eq!(inclusion.adjusted_weight_fee, XorFee::multiplier().saturating_mul_int(common::weights::constants::BIG_FEE));
            let quoted_fee = fee_details.final_fee();
            assert!(quoted_fee > 0);
            let endowed = quoted_fee.checked_add(Balances::minimum_balance()).unwrap().checked_add(common::balance!(1)).unwrap();
            let _ = Balances::make_free_balance_be(&user, endowed);
            assert_eq!(Assets::free_balance(&XOR, &user).unwrap(), endowed);
            System::reset_events();
            submit(burn, &user_key);
            let paid = System::events().iter().find_map(|record| match &record.event {
                RuntimeEvent::TransactionPayment(pallet_transaction_payment::Event::TransactionFeePaid { who, actual_fee, tip }) if who == &user => {
                    assert_eq!(*tip, 0);
                    Some(*actual_fee)
                },
                _ => None,
            }).expect("ordinary bridge burn emits native transaction-payment event");
            assert_eq!(paid, quoted_fee);
            assert_eq!(Balances::free_balance(&user), endowed - paid);
            assert_eq!(Assets::free_balance(&XOR, &user).unwrap(), endowed - paid);
            eprintln!("BridgeProxy::burn charged {paid} native XOR base units; exact quote funded, PSWAP transfer remains separately accounted");
            let queued = eth_bridge::RequestsQueue::<Runtime>::get(0);
            assert_eq!(queued.len(), 1);
            let hash = queued[0];
            let OffchainRequest::Outgoing(request, retained_hash) =
                eth_bridge::Requests::<Runtime>::get(0, hash).unwrap()
            else {
                panic!("queued outgoing transfer");
            };
            assert_eq!(retained_hash, hash);
            assert_eq!(
                Assets::free_balance(&PSWAP, &user).unwrap(),
                user_before - 100
            );
            assert_eq!(
                Assets::total_balance(&PSWAP, &bridge).unwrap(),
                custody_before + 100
            );
            assert_eq!(
                Assets::free_balance(&PSWAP, &bridge).unwrap(),
                custody_before
            );
            assert_eq!(BridgeProxy::locked_assets(network, PSWAP), locked + 100);
            let eth_bridge::requests::OutgoingRequestEncoded::Transfer(encoded) =
                request.to_eth_abi(hash).unwrap()
            else {
                panic!("queued request must encode a transfer");
            };
            let digest = common::eth::prepare_message(&encoded.raw).serialize();
            for (index, key) in keys.iter().enumerate() {
                let signature = key.sign_prehashed(&digest);
                let signature_params = eth_bridge::offchain::SignatureParams {
                    r: signature.0[..32].try_into().unwrap(),
                    s: signature.0[32..64].try_into().unwrap(),
                    v: signature.0[64],
                };
                let call = RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
                    ocw_public: key.public(),
                    hash,
                    signature_params,
                    network_id: 0,
                });
                assert_eq!(
                    crate::xor_fee_impls::validate_bridge_fee_exemption(&account(key), &call),
                    Ok(true)
                );
                submit(call, key);
                assert_eq!(
                    eth_bridge::RequestApprovers::<Runtime>::get(0, hash).len(),
                    index + 1
                );
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, hash),
                    Some(if index == 2 {
                        RequestStatus::ApprovalsReady
                    } else {
                        RequestStatus::Pending
                    })
                );
            }
            assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
            assert_eq!(
                Assets::free_balance(&PSWAP, &bridge).unwrap(),
                custody_before + 100
            );
            assert_eq!(
                BridgeProxy::transactions((network, user.clone()), hash)
                    .unwrap()
                    .status(),
                MessageStatus::Approved
            );

            for (from, amount, byte, beneficiary) in [
                (
                    H160(recovery::ATTACKER_EVM),
                    40u128,
                    87u8,
                    redirected.clone(),
                ),
                (H160::repeat_byte(88), 60u128, 88u8, recipient.clone()),
            ] {
                let balance = Assets::free_balance(&PSWAP, &beneficiary).unwrap();
                let untouched = Assets::free_balance(&PSWAP, &recipient).unwrap();
                let (call, tx_hash, native_hash) =
                    import_call(&bridge, &account(&keys[0]), &recipient, from, amount, byte);
                for (index, key) in keys.iter().enumerate() {
                    assert_eq!(
                        crate::xor_fee_impls::validate_bridge_fee_exemption(&account(key), &call),
                        Ok(true)
                    );
                    submit(call.clone(), key);
                    assert_eq!(
                        Assets::free_balance(&PSWAP, &beneficiary).unwrap(),
                        balance + if index == 2 { amount } else { 0 }
                    );
                }
                if beneficiary != recipient {
                    assert_eq!(Assets::free_balance(&PSWAP, &recipient).unwrap(), untouched);
                }
                assert_eq!(
                    eth_bridge::RequestStatuses::<Runtime>::get(0, native_hash),
                    Some(RequestStatus::Done)
                );
                assert_eq!(
                    eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(0, tx_hash),
                    native_hash
                );
                assert_eq!(
                    BridgeProxy::sender(network, tx_hash),
                    Some(beneficiary.clone())
                );
                assert_eq!(
                    BridgeProxy::transactions((network, beneficiary.clone()), tx_hash)
                        .unwrap()
                        .status(),
                    MessageStatus::Done
                );
                assert!(eth_bridge::RequestsQueue::<Runtime>::get(0).is_empty());
                // Newly signed native nonces do not reopen a consumed receipt.
                assert!(crate::xor_fee_impls::validate_bridge_fee_exemption(
                    &account(&keys[0]),
                    &call
                )
                .is_err());
                frame_system::BlockSize::<Runtime>::kill();
                frame_system::BlockWeight::<Runtime>::kill();
                assert!(Executive::apply_extrinsic(signed(call, &keys[0])).is_err());
                assert_eq!(
                    Assets::free_balance(&PSWAP, &beneficiary).unwrap(),
                    balance + amount
                );
            }
            assert_eq!(BridgeProxy::locked_assets(network, PSWAP), locked);
            assert_eq!(
                Assets::free_balance(&PSWAP, &bridge).unwrap(),
                custody_before
            );
            assert_eq!(Assets::total_issuance(&PSWAP).unwrap(), issuance);
            assert!(bridge_types::recovery::completion().unwrap().is_some());
            assert!(sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY));
            for key in recovery::TRANSIENT_CONTEXT_KEYS {
                assert!(!sp_io::storage::exists(key));
            }
        });
    });
}
