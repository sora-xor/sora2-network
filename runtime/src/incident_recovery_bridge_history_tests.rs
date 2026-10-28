//! Native refund/status suppression after terminal bridge ownership replacement.
use crate::{AccountId, AssetId, Balances, BridgeProxy, Runtime, System};
use bridge_types::{
    recovery::HistoricalMessageTombstone,
    traits::MessageStatusNotifier,
    types::{MessageDirection, MessageStatus},
    GenericAccount, GenericNetworkId, GenericTimepoint, SubNetworkId, H256,
};
use codec::{Decode, Encode};
use common::recovery::{
    self, AccountReplacement, GuardedStorageEdit, RecoveryChunk, RecoveryPolicy, StorageScope,
};
use frame_support::{assert_noop, assert_ok, traits::Currency};

#[derive(Encode, Decode)]
struct Record {
    source: GenericAccount,
    dest: GenericAccount,
    asset: AssetId,
    amount: u128,
    status: MessageStatus,
    start: GenericTimepoint,
    end: GenericTimepoint,
    direction: MessageDirection,
}
fn prefix(item: &[u8]) -> Vec<u8> {
    [
        sp_io::hashing::twox_128(b"BridgeProxy"),
        sp_io::hashing::twox_128(item),
    ]
    .concat()
}
fn transaction(network: GenericNetworkId, owner: &AccountId, id: H256) -> Vec<u8> {
    let first = (network, owner).encode();
    [
        prefix(b"Transactions"),
        sp_io::hashing::blake2_128(&first).to_vec(),
        first,
        sp_io::hashing::blake2_128(id.as_bytes()).to_vec(),
        id.encode(),
    ]
    .concat()
}
fn sender(network: GenericNetworkId, id: H256) -> Vec<u8> {
    [
        prefix(b"Senders"),
        sp_io::hashing::twox_64(&network.encode()).to_vec(),
        network.encode(),
        sp_io::hashing::blake2_128(id.as_bytes()).to_vec(),
        id.encode(),
    ]
    .concat()
}

#[test]
fn opaque_legacy_inbound_marker_blocks_status_and_both_record_constructors_without_global_halt() {
    sp_io::TestExternalities::default().execute_with(|| {
        System::set_block_number(1);
        let owner = AccountId::new([81; 32]);
        let replacement = AccountId::new([82; 32]);
        let network = GenericNetworkId::Sub(SubNetworkId::Polkadot);
        let id = H256::repeat_byte(83);
        let original = Record {
            source: GenericAccount::Sora(owner.clone()),
            dest: GenericAccount::Unknown,
            asset: common::XOR,
            amount: 500,
            status: MessageStatus::InQueue,
            start: GenericTimepoint::EVM(0),
            end: GenericTimepoint::EVM(0),
            direction: MessageDirection::Inbound,
        }
        .encode();
        let request = transaction(network, &owner, id);
        let index = sender(network, id);
        sp_io::storage::set(&request, &original);
        sp_io::storage::set(&index, &owner.encode());
        let marker = (
            *b"snxanom1",
            recovery::CHECKPOINT_HASH,
            sp_io::hashing::blake2_256(&owner.encode()),
            sp_io::hashing::blake2_256(&original),
            [84u8; 32],
            [85u8; 32],
        )
            .encode();
        sp_io::storage::set(
            &bridge_types::recovery::historical_message_key(network, id),
            &marker,
        );
        sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
        let root = sp_io::storage::root(sp_core::storage::StateVersion::V0);
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::Sora(2),
        );
        BridgeProxy::inbound_request(
            network,
            id,
            GenericAccount::Root,
            replacement.clone(),
            common::XOR,
            999,
            GenericTimepoint::Sora(2),
            MessageStatus::Done,
        );
        BridgeProxy::outbound_request(
            network,
            id,
            replacement,
            GenericAccount::Root,
            common::XOR,
            999,
            MessageStatus::InQueue,
        );
        assert_noop!(
            BridgeProxy::refund(network, id, GenericAccount::Sora(owner), common::XOR, 500),
            bridge_proxy::Error::<Runtime>::RecoveryHalted
        );
        assert_eq!(sp_io::storage::get(&request).unwrap().to_vec(), original);
        assert_eq!(
            sp_io::storage::root(sp_core::storage::StateVersion::V0),
            root
        );
    });
}

#[test]
fn terminal_history_marker_blocks_native_status_refund_and_recreation_after_global_halt_removed() {
    sp_io::TestExternalities::default().execute_with(|| {
        System::set_block_number(1);
        let old = AccountId::new([71; 32]);
        let new = AccountId::new([72; 32]);
        let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
        let id = H256::repeat_byte(73);
        drop(Balances::deposit_creating(&new, 1_000));
        let external = GenericAccount::EVM(bridge_types::H160::repeat_byte(71));
        BridgeProxy::outbound_request(
            network,
            id,
            old.clone(),
            external.clone(),
            common::XOR,
            500,
            MessageStatus::Done,
        );
        let source = transaction(network, &old, id);
        let target = transaction(network, &new, id);
        let index = sender(network, id);
        let original = sp_io::storage::get(&source).unwrap();
        let mut moved = Record::decode(&mut original.as_ref()).unwrap();
        assert_eq!(moved.encode(), original.to_vec());
        moved.source = GenericAccount::Sora(new.clone());
        let marker = HistoricalMessageTombstone {
            tag: HistoricalMessageTombstone::TAG,
            source_record_hash: sp_io::hashing::blake2_256(&original),
            original_owner: *AsRef::<[u8; 32]>::as_ref(&old),
            effective_owner: *AsRef::<[u8; 32]>::as_ref(&new),
            evidence: [74; 32],
            refund_authorized: false,
        };
        assert!(marker.is_valid());
        let marker_key = bridge_types::recovery::historical_message_key(network, id);
        let mut edits = vec![
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: source.clone(),
                before: Some(sp_io::hashing::blake2_256(&original)),
                after: None,
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: target.clone(),
                before: None,
                after: Some(moved.encode()),
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: index,
                before: Some(sp_io::hashing::blake2_256(&old.encode())),
                after: Some(new.encode()),
            },
            GuardedStorageEdit {
                scope: StorageScope::Top,
                key: marker_key,
                before: None,
                after: Some(marker.encode()),
            },
        ];
        edits.sort_by(|a, b| a.key.cmp(&b.key));
        let chunk = RecoveryChunk { index: 0, edits };
        assert_ok!(chunk.validate());
        let policy = RecoveryPolicy {
            format_version: 1,
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![AccountReplacement {
                old: old.clone(),
                new: new.clone(),
            }],
            council: vec![AccountId::new([75; 32])],
            chunk_hashes: vec![chunk.commitment()],
            audit_chunk_hashes: vec![[76; 32]],
            evidence_commitment: [77; 32],
            terminal_audit_commitment: [78; 32],
        };
        assert_ok!(recovery::storage::initialize(&policy));
        assert_ok!(recovery::storage::apply_chunk(&policy, &chunk));
        sp_io::storage::clear(recovery::BRIDGE_HALT_STORAGE_KEY);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        let issuance = Balances::total_issuance();
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::Sora(200),
        );
        assert_noop!(
            BridgeProxy::refund(
                network,
                id,
                GenericAccount::Sora(new.clone()),
                common::XOR,
                500
            ),
            bridge_proxy::Error::<Runtime>::RecoveryHalted
        );
        BridgeProxy::outbound_request(
            network,
            id,
            new.clone(),
            GenericAccount::Sora(old.clone()),
            common::XOR,
            999,
            MessageStatus::InQueue,
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        assert_eq!(Balances::total_issuance(), issuance);
        assert_eq!(Balances::free_balance(&new), 1_000);
        assert_eq!(BridgeProxy::sender(network, id), Some(new.clone()));
        assert!(BridgeProxy::transactions((network, old), id).is_none());
        let retained = BridgeProxy::transactions((network, new), id).unwrap();
        assert_eq!(retained.status(), MessageStatus::Done);
        assert_eq!(retained.amount(), 500);
        let raw = sp_io::storage::get(&target).unwrap();
        let decoded = Record::decode(&mut raw.as_ref()).unwrap();
        assert_eq!(decoded.dest, external);
    });
}

#[test]
fn malformed_historical_marker_fails_closed_without_requiring_global_halt() {
    sp_io::TestExternalities::default().execute_with(|| {
        let network = GenericNetworkId::Sub(SubNetworkId::Liberland);
        let id = H256::repeat_byte(93);
        sp_io::storage::set(
            &bridge_types::recovery::historical_message_key(network, id),
            b"invalid",
        );
        assert!(bridge_types::recovery::message_quarantined(network, id));
        assert_noop!(
            BridgeProxy::refund(
                network,
                id,
                GenericAccount::Sora(AccountId::new([1; 32])),
                common::XOR,
                1
            ),
            bridge_proxy::Error::<Runtime>::RecoveryHalted
        );
    });
}
