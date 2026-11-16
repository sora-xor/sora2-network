//! Execute the exact candidate through Wasmtime and production host functions.
//! All keys, balances and recovery records below are disposable local fixtures.
//! No RPC, production key, mocked signature, or native dispatch shortcut is used.

use codec::{DecodeAll, Encode};
use common::{recovery, AssetInfoProvider, PSWAP};
use eth_bridge::requests::{AssetKind, OffchainRequest, OutgoingRequest, RequestStatus};
use frame_support::{dispatch::GetDispatchInfo, storage::storage_prefix, traits::Currency};
use framenode_runtime::{
    eth_bridge, AccountId, Assets, Balances, BridgeMultisig, Runtime, RuntimeCall, Signature,
    SignedExtra, System, UncheckedExtrinsic, XorFee,
};
use sc_executor::WasmExecutor;
use sc_executor_common::runtime_blob::RuntimeBlob;
use sp_core::{
    ecdsa, offchain::testing::TestOffchainExt, offchain::OffchainWorkerExt, Pair, H160, H256,
};
use sp_runtime::{
    generic::Era,
    traits::IdentifyAccount,
    transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError,
    },
    BuildStorage, MultiSigner,
};
use std::{collections::BTreeSet, str::FromStr};

fn account(key: &ecdsa::Pair) -> AccountId {
    MultiSigner::Ecdsa(key.public()).into_account()
}

fn network_key(item: &[u8]) -> Vec<u8> {
    [
        storage_prefix(b"EthBridge", item).to_vec(),
        sp_io::hashing::twox_64(&0u32.encode()).to_vec(),
        0u32.encode(),
    ]
    .concat()
}

fn setup(wasm: &[u8]) -> (sp_io::TestExternalities, Vec<ecdsa::Pair>, ecdsa::Pair) {
    let mut storage = framenode_chain_spec::main_net_coded()
        .build_storage()
        .expect("offline mainnet genesis fixture");
    // Native harness compilation may omit its embedded Wasm. The fixture must
    // always execute the separately built, hash-checked candidate.
    storage.top.insert(
        sp_core::storage::well_known_keys::CODE.to_vec(),
        wasm.to_vec(),
    );
    let mut ext = sp_io::TestExternalities::new(storage);
    let (keys, user_key) = ext.execute_with(|| {
        let policy = framenode_runtime::incident_recovery::compiled_policy()
            .expect("verified compiled recovery policy");
        let commitment = policy.commitment();
        assert_eq!(
            H256(commitment),
            H256::from_str("1f4e11427a9c0a61a4868af40c9e0bdf263bb44a9ee59886a537c52164f3cdbb")
                .unwrap()
        );
        let keys: Vec<_> = [81u8, 82, 83]
            .into_iter()
            .map(|seed| ecdsa::Pair::from_seed(&[seed; 32]))
            .collect();
        let peers: BTreeSet<_> = keys.iter().map(account).collect();
        let peers_vec: Vec<_> = peers.iter().cloned().collect();
        System::set_block_number(recovery::CHECKPOINT_NUMBER + 100);
        frame_system::BlockHash::<Runtime>::insert(0, H256(recovery::GENESIS_HASH));
        for peer in &peers {
            let _ = Balances::make_free_balance_be(peer, common::balance!(100));
        }
        let bridge =
            BridgeMultisig::register_multisig_inner(peers_vec[0].clone(), peers_vec).unwrap();
        assert_eq!(
            BridgeMultisig::accounts(&bridge).unwrap().threshold_num(),
            3
        );
        eth_bridge::Peers::<Runtime>::insert(0, peers);
        eth_bridge::BridgeAccount::<Runtime>::insert(0, &bridge);

        // Coherent completed recovery is retained, including its historical
        // halt marker. There is deliberately no peer/governance release record.
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
        for key in recovery::TRANSIENT_CONTEXT_KEYS {
            sp_io::storage::clear(key);
        }
        sp_io::storage::set(recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        let guardian = common::bridge_halt::GuardianHalt {
            number: 1,
            block: System::block_number(),
        };
        sp_io::storage::set(
            common::bridge_halt::GUARDIAN_HALT_STORAGE_KEY,
            &guardian.encode(),
        );
        assert!(bridge_types::recovery::completion().unwrap().is_some());
        // This signal still stops unacknowledged OCW workers. It is not a
        // consensus veto on independently signed normal bridge transactions.
        assert!(!common::bridge_halt::workers_may_run(
            &Ok(Some(guardian)),
            None
        ));

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
        System::inc_providers(&bridge);
        let user_key = ecdsa::Pair::from_seed(&[84; 32]);
        let user = account(&user_key);
        let _ = Balances::make_free_balance_be(&user, common::balance!(100));
        Assets::mint_unchecked(&PSWAP, &user, 1_000).unwrap();
        (keys, user_key)
    });
    (ext, keys, user_key)
}

fn signed(call: RuntimeCall, who: &ecdsa::Pair, signer: &ecdsa::Pair) -> UncheckedExtrinsic {
    let extra: SignedExtra = (
        frame_system::AuthorizeCall::new(),
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(System::account_nonce(account(who))),
        frame_system::CheckWeight::<Runtime>::new(),
        framenode_runtime::charge_tx_payment_extension(),
    );
    let payload = framenode_runtime::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|bytes| signer.sign(bytes));
    UncheckedExtrinsic::new_signed(call, account(who), Signature::Ecdsa(signature), extra)
}

fn execute(
    executor: &WasmExecutor<crate::service::HostFunctions>,
    wasm: &[u8],
    ext: &mut sp_io::TestExternalities,
    method: &str,
    input: &[u8],
) -> Vec<u8> {
    executor
        .uncached_call(
            RuntimeBlob::uncompress_if_needed(wasm).unwrap(),
            &mut ext.ext(),
            false,
            method,
            input,
        )
        .expect("actual candidate executes through production host functions")
}

fn apply(
    executor: &WasmExecutor<crate::service::HostFunctions>,
    wasm: &[u8],
    ext: &mut sp_io::TestExternalities,
    tx: UncheckedExtrinsic,
) -> sp_runtime::ApplyExtrinsicResult {
    ext.execute_with(|| {
        frame_system::BlockSize::<Runtime>::kill();
        frame_system::BlockWeight::<Runtime>::kill();
    });
    let bytes = execute(
        executor,
        wasm,
        ext,
        "BlockBuilder_apply_extrinsic",
        &tx.encode(),
    );
    sp_runtime::ApplyExtrinsicResult::decode_all(&mut &bytes[..]).unwrap()
}

fn validate(
    executor: &WasmExecutor<crate::service::HostFunctions>,
    wasm: &[u8],
    ext: &mut sp_io::TestExternalities,
    tx: UncheckedExtrinsic,
) -> TransactionValidity {
    // Pool validation initializes a synthetic next block; do not retain that
    // overlay between cases or alter the completed recovery fixture.
    ext.execute_with(sp_io::storage::start_transaction);
    let bytes = execute(
        executor,
        wasm,
        ext,
        "TaggedTransactionQueue_validate_transaction",
        &(TransactionSource::External, tx, H256::zero()).encode(),
    );
    ext.execute_with(sp_io::storage::rollback_transaction);
    TransactionValidity::decode_all(&mut &bytes[..]).unwrap()
}

fn approval(request: &OutgoingRequest<Runtime>, hash: H256, peer: &ecdsa::Pair) -> RuntimeCall {
    let eth_bridge::requests::OutgoingRequestEncoded::Transfer(encoded) =
        request.to_eth_abi(hash).unwrap()
    else {
        panic!("outgoing transfer ABI");
    };
    let signature = peer.sign_prehashed(&common::eth::prepare_message(&encoded.raw).serialize());
    RuntimeCall::EthBridge(eth_bridge::Call::approve_request {
        ocw_public: peer.public(),
        hash,
        network_id: 0,
        signature_params: eth_bridge::offchain::SignatureParams {
            r: signature.0[..32].try_into().unwrap(),
            s: signature.0[32..64].try_into().unwrap(),
            v: signature.0[64],
        },
    })
}

#[test]
fn ocw_only_halt_wasm_accepts_signed_operations_without_unhalt_and_keeps_seven_blocked() {
    let wasm =
        std::fs::read(std::env::var("SORA_OCW_ONLY_WASM").expect("explicit candidate Wasm path"))
            .unwrap();
    let expected = std::env::var("SORA_OCW_ONLY_WASM_BLAKE2").expect("explicit candidate hash");
    let hash = H256(sp_io::hashing::blake2_256(&wasm));
    assert_eq!(format!("{hash:x}"), expected);
    assert!(crate::bridge_local_quarantine::reviewed_code(
        H256(recovery::GENESIS_HASH),
        Some(hash)
    ));
    let executor = WasmExecutor::<crate::service::HostFunctions>::builder().build();
    let (mut ext, peers, user) = setup(&wasm);
    let transfer = RuntimeCall::EthBridge(eth_bridge::Call::transfer_to_sidechain {
        asset_id: PSWAP,
        to: H160::repeat_byte(86),
        amount: 100,
        network_id: 0,
    });
    ext.execute_with(|| {
        let tx = signed(transfer.clone(), &user, &user);
        let fee = XorFee::compute_fee(
            tx.encoded_size().try_into().unwrap(),
            &transfer,
            &transfer.get_dispatch_info(),
            0,
        )
        .0;
        assert!(fee > 0);
        let endowed = fee
            .checked_add(Balances::minimum_balance())
            .unwrap()
            .checked_add(common::balance!(1))
            .unwrap();
        let _ = Balances::make_free_balance_be(&account(&user), endowed);
    });
    let invalid = ext.execute_with(|| signed(transfer.clone(), &user, &peers[0]));
    assert_eq!(
        apply(&executor, &wasm, &mut ext, invalid),
        Err(TransactionValidityError::Invalid(
            InvalidTransaction::BadProof
        ))
    );
    ext.execute_with(|| assert_eq!(System::account_nonce(account(&user)), 0));
    // Fee-paying transfers may enter the pool, but incomplete recovery must
    // reject their dispatch before touching bridge funds or request bookkeeping.
    // Roll back the attempted extrinsic's fee/nonce before the positive case.
    let before_incomplete = ext.execute_with(|| {
        let before = (
            Assets::free_balance(&PSWAP, &account(&user)).unwrap(),
            eth_bridge::RequestsQueue::<Runtime>::get(0),
            Balances::free_balance(account(&user)),
            System::account_nonce(account(&user)),
        );
        sp_io::storage::start_transaction();
        sp_io::storage::clear(recovery::INCOMING_AUDIT_PENDING_KEY);
        before
    });
    let incomplete = ext.execute_with(|| signed(transfer.clone(), &user, &user));
    assert_eq!(
        apply(&executor, &wasm, &mut ext, incomplete),
        Ok(Err(eth_bridge::Error::<Runtime>::RecoveryHalted.into()))
    );
    ext.execute_with(|| {
        assert_eq!(
            Assets::free_balance(&PSWAP, &account(&user)).unwrap(),
            before_incomplete.0
        );
        assert_eq!(
            eth_bridge::RequestsQueue::<Runtime>::get(0),
            before_incomplete.1
        );
        sp_io::storage::rollback_transaction();
        assert_eq!(Balances::free_balance(account(&user)), before_incomplete.2);
        assert_eq!(System::account_nonce(account(&user)), before_incomplete.3);
        assert!(bridge_types::recovery::completion().unwrap().is_some());
    });
    let tx = ext.execute_with(|| signed(transfer, &user, &user));
    assert_eq!(apply(&executor, &wasm, &mut ext, tx), Ok(Ok(())));
    let (request, request_hash) = ext.execute_with(|| {
        assert_eq!(Assets::free_balance(&PSWAP, &account(&user)).unwrap(), 900);
        let queue = eth_bridge::RequestsQueue::<Runtime>::get(0);
        assert_eq!(queue.len(), 1);
        let hash = queue[0];
        let OffchainRequest::Outgoing(request, retained_hash) =
            eth_bridge::Requests::<Runtime>::get(0, hash).unwrap()
        else {
            panic!("transfer was queued");
        };
        assert_eq!(hash, retained_hash);
        (request, hash)
    });
    // A valid extrinsic signature from a non-peer must not approve a request.
    let outsider =
        ext.execute_with(|| signed(approval(&request, request_hash, &peers[0]), &user, &user));
    assert!(validate(&executor, &wasm, &mut ext, outsider).is_err());
    for (index, peer) in peers.iter().enumerate() {
        let tx = ext.execute_with(|| signed(approval(&request, request_hash, peer), peer, peer));
        assert_eq!(apply(&executor, &wasm, &mut ext, tx), Ok(Ok(())));
        ext.execute_with(|| {
            assert_eq!(
                eth_bridge::RequestApprovers::<Runtime>::get(0, request_hash).len(),
                index + 1
            );
            assert_eq!(
                eth_bridge::RequestStatuses::<Runtime>::get(0, request_hash),
                Some(if index == 2 {
                    RequestStatus::ApprovalsReady
                } else {
                    RequestStatus::Pending
                })
            );
            assert_eq!(
                sp_io::storage::get(recovery::BRIDGE_HALT_STORAGE_KEY).as_deref(),
                Some(&[1][..])
            );
            assert!(bridge_types::recovery::completion().unwrap().is_some());
        });
    }

    // Disposable aliases model retained requests at the seven exact policy
    // hashes. No chain quarantine markers are created for the local-only test.
    let blocked: Vec<_> = crate::bridge_local_quarantine::REQUESTS
        .iter()
        .map(|hash| H256::from_str(hash).unwrap())
        .collect();
    ext.execute_with(|| {
        for hash in &blocked {
            eth_bridge::Requests::<Runtime>::insert(
                0,
                hash,
                OffchainRequest::Outgoing(request.clone(), *hash),
            );
            eth_bridge::RequestStatuses::<Runtime>::insert(0, hash, RequestStatus::Pending);
            assert!(!bridge_types::recovery::hashi_quarantined(0u32, *hash));
        }
    });
    // Ordinary consensus validation does not inherit an operator's local policy.
    let first = ext.execute_with(|| {
        signed(
            approval(&request, blocked[0], &peers[0]),
            &peers[0],
            &peers[0],
        )
    });
    assert!(validate(&executor, &wasm, &mut ext, first).is_ok());
    ext.register_extension(crate::bridge_local_quarantine::extension());
    let (offchain, _) = TestOffchainExt::new();
    ext.register_extension(OffchainWorkerExt::new(offchain));
    for hash in blocked {
        let tx =
            ext.execute_with(|| signed(approval(&request, hash, &peers[0]), &peers[0], &peers[0]));
        assert!(
            validate(&executor, &wasm, &mut ext, tx).is_err(),
            "local block missing for {hash:x}"
        );
        ext.execute_with(|| {
            assert!(eth_bridge::RequestApprovers::<Runtime>::get(0, hash).is_empty());
            assert!(sp_io::storage::get(&bridge_types::recovery::hashi_key(0u32, hash)).is_none());
        });
    }
}
