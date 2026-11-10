use super::*;
use crate::mock::{
    new_tester, AccountId, BridgeProxy, Currencies, RuntimeCall, RuntimeOrigin, System, Test,
    BASE_EVM_NETWORK_ID,
};
use codec::Encode;
use common::{AssetInfoProvider, XOR};
use frame_support::{
    assert_noop, assert_ok,
    dispatch::{GetDispatchInfo, Pays},
};
use sp_keyring::sr25519::Keyring;

fn withdrawal(owner: AccountId) -> (GenericNetworkId, H256) {
    let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
    assert_ok!(Currencies::update_balance(
        RuntimeOrigin::root(),
        owner.clone(),
        XOR,
        1_000
    ));
    assert_ok!(BridgeProxy::burn(
        RuntimeOrigin::signed(owner.clone()),
        network,
        XOR,
        GenericAccount::EVM(H160::repeat_byte(9)),
        1_000,
    ));
    let id = Transactions::<Test>::iter_prefix((network, owner))
        .next()
        .unwrap()
        .0;
    (network, id)
}

fn balance(account: &AccountId) -> Balance {
    assets::Pallet::<Test>::free_balance(&XOR, account).unwrap()
}

fn mark_failed_without_custody(owner: AccountId) -> (GenericNetworkId, H256, AccountId) {
    let (network, id) = withdrawal(owner);
    let custody = BridgeProxy::bridge_account(network).unwrap();
    assert_ok!(Currencies::update_balance(
        RuntimeOrigin::root(),
        custody.clone(),
        XOR,
        -1_000
    ));
    BridgeProxy::update_status(
        network,
        id,
        MessageStatus::Failed,
        GenericTimepoint::EVM(10),
    );
    (network, id, custody)
}

#[test]
fn paid_retry_resolves_owner_preserves_evidence_and_refunds_exactly_once() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let recipient: AccountId = Keyring::Dave.into();
        let payer: AccountId = Keyring::Charlie.into();
        let (network, id, custody) = mark_failed_without_custody(owner.clone());
        sp_io::storage::set(
            b"proxy-test:refund-mapping",
            &(owner.clone(), recipient.clone()).encode(),
        );
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert!(
            BridgeProxy::retry_refund(RuntimeOrigin::signed(payer.clone()), network, id).is_err()
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        assert_eq!(LockedAssets::<Test>::get(network, XOR), 1_000);
        assert_ok!(Currencies::update_balance(
            RuntimeOrigin::root(),
            custody,
            XOR,
            1_000
        ));
        System::set_block_number(20);
        let call = RuntimeCall::BridgeProxy(crate::Call::retry_refund {
            network_id: network,
            message_id: id,
        });
        assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
        assert!(call.get_dispatch_info().total_weight().ref_time() > 0);
        assert_eq!(
            crate::Call::<Test>::retry_refund {
                network_id: network,
                message_id: id
            }
            .encode()[0],
            4
        );
        assert_ok!(BridgeProxy::retry_refund(
            RuntimeOrigin::signed(payer.clone()),
            network,
            id
        ));
        assert_eq!(balance(&owner), 0);
        assert_eq!(balance(&recipient), 1_000);
        assert_eq!(balance(&payer), 0);
        assert_eq!(LockedAssets::<Test>::get(network, XOR), 0);
        assert_eq!(Senders::<Test>::get(network, id), Some(owner.clone()));
        let request = Transactions::<Test>::get((network, owner), id).unwrap();
        assert_eq!(request.source, GenericAccount::Sora(Keyring::Alice.into()));
        assert_eq!(request.status, MessageStatus::Refunded);
        assert_eq!(request.end_timepoint, GenericTimepoint::Sora(20));
        assert_noop!(
            BridgeProxy::retry_refund(RuntimeOrigin::signed(payer), network, id),
            Error::<Test>::RefundNotAvailable
        );
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::EVM(30),
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
    });
}

#[test]
fn automatic_refund_uses_the_same_fallible_owner_resolution() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let recipient: AccountId = Keyring::Dave.into();
        let (network, id) = withdrawal(owner.clone());
        sp_io::storage::set(b"proxy-test:refund-mapping", &[1]);
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::EVM(10),
        );
        assert_eq!(
            Transactions::<Test>::get((network, owner.clone()), id)
                .unwrap()
                .status,
            MessageStatus::Failed
        );
        assert_eq!(balance(&owner), 0);
        assert_eq!(LockedAssets::<Test>::get(network, XOR), 1_000);
        assert_noop!(
            BridgeProxy::retry_refund(RuntimeOrigin::signed(recipient.clone()), network, id),
            DispatchError::Other("Invalid refund mapping")
        );
        sp_io::storage::set(
            b"proxy-test:refund-mapping",
            &(owner.clone(), recipient.clone()).encode(),
        );
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::EVM(11),
        );
        assert_eq!(balance(&recipient), 1_000);
        assert_eq!(balance(&owner), 0);
        assert_eq!(
            Transactions::<Test>::get((network, owner), id)
                .unwrap()
                .status,
            MessageStatus::Refunded
        );
    });
}

#[test]
fn retries_reject_halts_both_quarantine_markers_and_non_failed_outbound_records() {
    for marker in 0..3 {
        new_tester().execute_with(|| {
            let owner: AccountId = Keyring::Alice.into();
            let (network, id, _) = mark_failed_without_custody(owner.clone());
            let key = match marker {
                0 => common::recovery::BRIDGE_HALT_STORAGE_KEY.to_vec(),
                1 => bridge_types::recovery::message_key(network, id),
                _ => bridge_types::recovery::historical_message_key(network, id),
            };
            sp_io::storage::set(&key, &[0xff]);
            assert_noop!(
                BridgeProxy::retry_refund(RuntimeOrigin::signed(owner), network, id),
                Error::<Test>::RecoveryHalted
            );
        });
    }
    for (direction, status) in [
        (MessageDirection::Inbound, MessageStatus::Failed),
        (MessageDirection::Outbound, MessageStatus::Done),
        (MessageDirection::Outbound, MessageStatus::Refunded),
        (MessageDirection::Outbound, MessageStatus::Committed),
        (MessageDirection::Outbound, MessageStatus::InQueue),
        (MessageDirection::Outbound, MessageStatus::Approved),
    ] {
        new_tester().execute_with(|| {
            let owner: AccountId = Keyring::Alice.into();
            let (network, id) = withdrawal(owner.clone());
            Transactions::<Test>::mutate((network, owner.clone()), id, |record| {
                let record = record.as_mut().unwrap();
                record.direction = direction;
                record.status = status;
            });
            assert_noop!(
                BridgeProxy::retry_refund(RuntimeOrigin::signed(owner), network, id),
                Error::<Test>::RefundNotAvailable
            );
        });
    }
}

#[test]
fn retry_requires_a_payer_and_an_existing_refundable_route() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        let (network, id, _) = mark_failed_without_custody(owner.clone());
        assert_noop!(
            BridgeProxy::retry_refund(RuntimeOrigin::none(), network, id),
            DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeProxy::retry_refund(RuntimeOrigin::root(), network, id),
            DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeProxy::retry_refund(
                RuntimeOrigin::signed(owner.clone()),
                network,
                H256::repeat_byte(99)
            ),
            Error::<Test>::UnknownRequest
        );
        Transactions::<Test>::mutate((network, owner.clone()), id, |record| {
            record.as_mut().unwrap().asset_id = common::TBCD
        });
        assert_noop!(
            BridgeProxy::retry_refund(RuntimeOrigin::signed(owner.clone()), network, id),
            Error::<Test>::PathIsNotAvailable
        );
        Transactions::<Test>::remove((network, owner.clone()), id);
        assert_noop!(
            BridgeProxy::retry_refund(RuntimeOrigin::signed(owner), network, id),
            Error::<Test>::UnknownRequest
        );
    });
}
