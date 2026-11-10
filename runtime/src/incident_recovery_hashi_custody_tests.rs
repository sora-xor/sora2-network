//! Native unit regressions for extinguished HASHI owned claims. The synthetic
//! opening ledger is not production evidence or proof of attacker-owned tokens.
use super::tests::{hashi_fixture_with_kind, sign_hashi};
use super::*;
use common::AssetInfoProvider;
use frame_support::{assert_ok, dispatch::GetDispatchInfo};
use sp_core::{sr25519, Pair};

const ONE: u128 = 1_000_000_000_000_000_000;
const INNOCENT_RESERVED: u128 = ONE / 2;
fn bridge() -> AccountId {
    AccountId::new([55; 32])
}
fn network() -> GenericNetworkId {
    GenericNetworkId::EVMLegacy(crate::GetEthNetworkId::get())
}
fn kinds() -> [eth_bridge::requests::AssetKind; 2] {
    [
        eth_bridge::requests::AssetKind::Thischain,
        eth_bridge::requests::AssetKind::SidechainOwned,
    ]
}
fn value_key(item: &[u8]) -> Vec<u8> {
    frame_support::storage::storage_prefix(b"BridgeProxy", item).to_vec()
}
fn map_key(item: &[u8], first: &[u8], second: Option<&[u8]>) -> Vec<u8> {
    let mut key = value_key(item);
    key.extend(sp_io::hashing::blake2_128(first));
    key.extend(first);
    if let Some(second) = second {
        key.extend(sp_io::hashing::blake2_128(second));
        key.extend(second);
    }
    key
}
fn locked_key() -> Vec<u8> {
    map_key(
        b"LockedAssets",
        &network().encode(),
        Some(&common::DAI.encode()),
    )
}
fn limit_sentinels() -> Vec<(Vec<u8>, Vec<u8>)> {
    // Explicit test-only preexisting accounting, deliberately nonzero so any
    // new outbound-limit charge or schedule mutation is observable.
    let rows = vec![
        (value_key(b"TransferLimit"), (1000 * ONE, 600u32).encode()),
        (value_key(b"ConsumedTransferLimit"), (7 * ONE).encode()),
        (
            map_key(
                b"TransferLimitUnlockSchedule",
                &(recovery::CHECKPOINT_NUMBER + 600).encode(),
                None,
            ),
            (3 * ONE).encode(),
        ),
        (
            map_key(b"LimitedAssets", &common::DAI.encode(), None),
            true.encode(),
        ),
    ];
    for (key, value) in &rows {
        sp_io::storage::set(key, value);
    }
    rows
}
fn assert_limits(rows: &[(Vec<u8>, Vec<u8>)]) {
    for (key, value) in rows {
        assert_eq!(sp_io::storage::get(key).unwrap().to_vec(), *value);
    }
}
fn request(operation: &IncomingOperation) -> eth_bridge::requests::IncomingRequest<Runtime> {
    let IncomingChannel::Hashi { request, .. } = &operation.channel else {
        unreachable!()
    };
    decode_bounded(request).unwrap()
}
fn prepare_existing(
    policy: &mut RecoveryPolicy,
    operation: &mut IncomingOperation,
    keys: &[sr25519::Pair],
) {
    let request = request(operation);
    let incoming_hash = eth_bridge::requests::OffchainRequest::incoming(request.clone()).hash();
    assert_ok!(crate::EthBridge::register_incoming_request(
        RuntimeOrigin::signed(bridge()),
        request
    ));
    let inner = RuntimeCall::EthBridge(eth_bridge::Call::finalize_incoming_request {
        hash: incoming_hash,
        network_id: crate::GetEthNetworkId::get(),
    });
    let IncomingChannel::Hashi { approvals, .. } = &mut operation.channel else {
        unreachable!()
    };
    for (approval, key) in approvals.iter_mut().zip(keys) {
        let mut call = decode_runtime_call(&approval.call).unwrap();
        let RuntimeCall::BridgeMultisig(bridge_multisig::Call::as_multi {
            call: opaque,
            max_weight,
            ..
        }) = &mut call
        else {
            unreachable!()
        };
        *opaque = inner.encode();
        *max_weight = inner.get_dispatch_info().total_weight();
        approval.call = call.encode();
        sign_hashi(approval, key);
    }
    policy.chunk_hashes = vec![operation.commitment()];
}
fn audit(policy: &mut RecoveryPolicy, operation: &IncomingOperation) -> IncomingAudit {
    let audit = IncomingAudit {
        index: 0,
        operation_index: operation.index,
        operation_commitment: operation.commitment(),
        requests: expected_requests(operation, policy).unwrap(),
        prior_policy: None,
    };
    assert_eq!(audit.requests, vec![None]);
    policy.audit_chunk_hashes = vec![audit.commitment()];
    audit
}
fn native(
    policy: &RecoveryPolicy,
    operation: &IncomingOperation,
) -> Result<(RecoveryStatus, Weight), RecoveryError> {
    crate::incident_recovery::with_test_policy(policy, || apply_operation(policy, operation))
}
fn assert_pool(free: u128, reserved: u128, locked: u128) {
    let account = tokens::Accounts::<Runtime>::get(bridge(), common::DAI);
    assert_eq!((account.free, account.reserved), (free, reserved));
    assert_eq!(
        crate::Assets::total_balance(&common::DAI, &bridge()).unwrap(),
        2 * ONE
    );
    assert_eq!(
        crate::Assets::total_issuance(&common::DAI).unwrap(),
        2 * ONE
    );
    assert_eq!(
        crate::BridgeProxy::locked_assets(network(), common::DAI),
        locked
    );
    for owner in [
        AccountId::new([7; 32]),
        AccountId::new([8; 32]),
        AccountId::new(recovery::ATTACKER_ACCOUNT),
    ] {
        assert_eq!(
            crate::Assets::total_balance(&common::DAI, &owner).unwrap(),
            0
        );
    }
}
#[test]
fn fresh_owned_exclusion_preserves_innocent_reserve_and_consumes_exactly_once() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, operation, keys) = hashi_fixture_with_kind(true, kind);
            assert_ok!(crate::Assets::reserve(
                &common::DAI,
                &bridge(),
                INNOCENT_RESERVED
            ));
            let limits = limit_sentinels();
            let audit = audit(&mut policy, &operation);
            recovery::storage::initialize(&policy).unwrap();
            assert_pool(2 * ONE - INNOCENT_RESERVED, INNOCENT_RESERVED, 2 * ONE);
            assert_ok!(native(&policy, &operation));
            assert_pool(2 * ONE - INNOCENT_RESERVED, INNOCENT_RESERVED, 2 * ONE);
            assert_limits(&limits);
            let eth_bridge::requests::IncomingRequest::Transfer(transfer) = request(&operation)
            else {
                unreachable!()
            };
            let incoming_hash =
                eth_bridge::requests::OffchainRequest::incoming(request(&operation)).hash();
            assert_eq!(
                crate::EthBridge::request_status(transfer.network_id, incoming_hash),
                Some(eth_bridge::requests::RequestStatus::Done)
            );
            assert_eq!(
                eth_bridge::LoadToIncomingRequestHash::<Runtime>::get(
                    transfer.network_id,
                    transfer.tx_hash
                ),
                incoming_hash
            );
            assert!(
                !eth_bridge::RequestsQueue::<Runtime>::get(transfer.network_id)
                    .contains(&incoming_hash)
            );
            assert!(sp_io::storage::exists(&receipt_key(
                network(),
                transfer.tx_hash
            )));
            for key in keys.iter().take(3) {
                assert_eq!(System::account_nonce(AccountId::from(key.public())), 1);
            }
            assert_eq!(
                sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
                    .unwrap()
                    .to_vec(),
                1u32.encode()
            );
            System::set_block_number(recovery::CHECKPOINT_NUMBER + 3);
            assert_ok!(audit_encoded_with_policy(&policy, &audit.encode()));
            assert_eq!(
                sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY)
                    .unwrap()
                    .to_vec(),
                0u32.encode()
            );
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(native(&policy, &operation).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
            assert_pool(2 * ONE - INNOCENT_RESERVED, INNOCENT_RESERVED, 2 * ONE);
            assert_limits(&limits);
        });
    }
}
#[test]
fn prepared_owned_exclusion_releases_only_claim_reserve_and_restores_its_lock_counter() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, keys) = hashi_fixture_with_kind(true, kind);
            assert_ok!(crate::Assets::reserve(
                &common::DAI,
                &bridge(),
                INNOCENT_RESERVED
            ));
            prepare_existing(&mut policy, &mut operation, &keys);
            assert_pool(ONE - INNOCENT_RESERVED, ONE + INNOCENT_RESERVED, ONE);
            let limits = limit_sentinels();
            let audit = audit(&mut policy, &operation);
            recovery::storage::initialize(&policy).unwrap();
            assert_ok!(native(&policy, &operation));
            // Retained prepare already reduced LockedAssets and reserved the
            // claim. Cancellation returns that reserve to free shared custody.
            assert_pool(2 * ONE - INNOCENT_RESERVED, INNOCENT_RESERVED, 2 * ONE);
            assert_limits(&limits);
            System::set_block_number(recovery::CHECKPOINT_NUMBER + 3);
            assert_ok!(audit_encoded_with_policy(&policy, &audit.encode()));
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(native(&policy, &operation).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }
}
fn failed_operation_rolls_back(
    policy: &RecoveryPolicy,
    operation: &IncomingOperation,
    keys: &[sr25519::Pair],
) {
    recovery::storage::initialize(policy).unwrap();
    let initial = recovery::storage::status().unwrap().unwrap();
    let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
    assert_eq!(native(policy, operation), Err(RecoveryError::WrongPreimage));
    let status = recovery::storage::status().unwrap().unwrap();
    assert_eq!(status.phase, RecoveryPhase::Failed);
    assert_eq!(status.next_chunk, initial.next_chunk);
    assert_eq!(status.operation_counter, initial.operation_counter);
    for key in keys.iter().take(3) {
        assert_eq!(System::account_nonce(AccountId::from(key.public())), 0);
    }
    assert!(!sp_io::storage::exists(ACTIVE_KEY));
    assert!(!sp_io::storage::exists(
        eth_bridge::recovery::HASHI_RESOLUTION_KEY
    ));
    // The failure status is the sole permitted write. Compare the entire state,
    // including prepared request, reserve, lock, receipt, votes and pending count.
    sp_io::storage::set(STATUS_STORAGE_KEY, &initial.encode());
    assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
}
#[test]
fn prepared_owned_restore_overflow_rolls_back_unreserve_and_quorum() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, keys) = hashi_fixture_with_kind(true, kind);
            assert_ok!(crate::Assets::reserve(
                &common::DAI,
                &bridge(),
                INNOCENT_RESERVED
            ));
            prepare_existing(&mut policy, &mut operation, &keys);
            // Deliberately corrupt the retained accounting to force the *last*
            // checked-add restoration to fail after native finalize consumed it.
            sp_io::storage::set(&locked_key(), &u128::MAX.encode());
            let limits = limit_sentinels();
            failed_operation_rolls_back(&policy, &operation, &keys);
            assert_limits(&limits);
            assert_eq!(
                tokens::Accounts::<Runtime>::get(bridge(), common::DAI).reserved,
                ONE + INNOCENT_RESERVED
            );
            assert_eq!(
                crate::BridgeProxy::locked_assets(network(), common::DAI),
                u128::MAX
            );
        });
    }
}
#[test]
fn prepared_owned_insufficient_reserve_rolls_back_all_consumption() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (mut policy, mut operation, keys) = hashi_fixture_with_kind(true, kind);
            prepare_existing(&mut policy, &mut operation, &keys);
            // Synthetic corruption retains total supply but cannot unreserve
            // the proven claim. The native FailedToUnreserve must fail closed.
            tokens::Accounts::<Runtime>::mutate(bridge(), common::DAI, |account| {
                account.reserved = ONE / 4;
                account.free = 2 * ONE - ONE / 4;
            });
            failed_operation_rolls_back(&policy, &operation, &keys);
        });
    }
}
#[test]
fn fresh_owned_insufficient_locked_backing_rolls_back_all_consumption() {
    for kind in kinds() {
        sp_io::TestExternalities::default().execute_with(|| {
            let (policy, operation, keys) = hashi_fixture_with_kind(true, kind);
            sp_io::storage::set(&locked_key(), &0u128.encode());
            failed_operation_rolls_back(&policy, &operation, &keys);
        });
    }
}
#[test]
fn owned_counter_restore_cannot_be_called_without_authenticated_recovery_context() {
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, _, _) =
            hashi_fixture_with_kind(true, eth_bridge::requests::AssetKind::Thischain);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(
            crate::BridgeProxy::recovery_restore_extinguished_hashi_owned(
                network(),
                common::DAI,
                ONE
            )
            .is_err()
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        recovery::storage::initialize(&policy).unwrap();
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(
            crate::BridgeProxy::recovery_restore_extinguished_hashi_owned(
                network(),
                common::DAI,
                ONE
            )
            .is_err()
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
    });
}
