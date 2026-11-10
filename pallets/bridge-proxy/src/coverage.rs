use super::*;
use bridge_types::traits::{
    BridgeApp, BridgeAssetLockChecker, BridgeAssetLocker, BridgeAssetRegistry,
    EVMBridgeWithdrawFee, MessageStatusNotifier,
};
use bridge_types::types::{AssetKind, BridgeAppInfo, BridgeAssetInfo};
use bridge_types::H256;
use codec::Decode;
use common::{AssetInfoProvider, AssetName, AssetSymbol};
use frame_support::traits::{OnRuntimeUpgrade, StorageVersion};

fn inbound_context() {
    sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
    sp_io::storage::set(
        common::recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
        &(System::block_number() as u32).to_le_bytes(),
    );
}

#[test]
fn alternate_transfer_and_refund_routes_preserve_all_arguments() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Alice.into();
        let encoded_location = xcm::VersionedLocation::V3(xcm::v3::MultiLocation::here()).encode();
        let location = Decode::decode(&mut &encoded_location[..]).unwrap();
        let routes = [
            (
                0u8,
                crate::mock::RoutedApp::<0>::network(),
                GenericAccount::EVM(H160::repeat_byte(9)),
            ),
            (
                1,
                crate::mock::RoutedApp::<1>::network(),
                GenericAccount::Parachain(location),
            ),
            (
                2,
                crate::mock::RoutedApp::<2>::network(),
                GenericAccount::Liberland(Keyring::Bob.into()),
            ),
        ];
        for (route, network, recipient) in routes {
            assert_ok!(BridgeProxy::burn(
                RawOrigin::Signed(sender.clone()).into(),
                network,
                XOR,
                recipient.clone(),
                123,
            ));
            // Hashi and parachain apps receive their concrete address type;
            // Liberland receives the tagged generic address.
            let recipient_bytes = match recipient {
                GenericAccount::EVM(value) => value.encode(),
                GenericAccount::Parachain(value) => value.encode(),
                value => value.encode(),
            };
            let mut expected = (route, network, XOR, sender.clone()).encode();
            expected.extend(recipient_bytes);
            expected.extend(123u128.encode());
            assert_eq!(
                sp_io::storage::get(b"proxy-test:transfer")
                    .unwrap()
                    .as_ref(),
                expected
            );

            let message = H256::repeat_byte(6);
            assert_ok!(BridgeProxy::refund(
                network,
                message,
                GenericAccount::Sora(sender.clone()),
                XOR,
                123,
            ));
            assert_eq!(
                sp_io::storage::get(b"proxy-test:refund").unwrap().as_ref(),
                (route, network, message, sender.clone(), XOR, 123u128).encode(),
            );
        }
    });
}

#[test]
fn transfer_and_refund_validate_origin_recipient_and_supported_assets() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        assert_noop!(
            BridgeProxy::burn(
                RawOrigin::None.into(),
                network,
                XOR,
                GenericAccount::EVM(H160::zero()),
                1
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        for recipient in [
            GenericAccount::Sora(sender.clone()),
            GenericAccount::Unknown,
            GenericAccount::Root,
        ] {
            assert_noop!(
                BridgeProxy::burn(
                    RawOrigin::Signed(sender.clone()).into(),
                    network,
                    XOR,
                    recipient,
                    1
                ),
                crate::Error::<Test>::WrongAccountKind
            );
        }
        assert_noop!(
            BridgeProxy::burn(
                RawOrigin::Signed(sender).into(),
                network,
                TBCD,
                GenericAccount::EVM(H160::zero()),
                1
            ),
            crate::Error::<Test>::PathIsNotAvailable
        );
        assert_noop!(
            BridgeProxy::refund(
                network,
                H256::zero(),
                GenericAccount::EVM(H160::zero()),
                XOR,
                1
            ),
            crate::Error::<Test>::WrongAccountKind
        );
    });
}

#[test]
fn app_and_asset_discovery_combines_every_configured_route() {
    new_tester().execute_with(|| {
        let mut expected =
            <FungibleApp as BridgeApp<AccountId, H160, crate::mock::AssetId, u128>>::list_apps();
        expected.extend([
            BridgeAppInfo::Sub(crate::mock::RoutedApp::<0>::network()),
            BridgeAppInfo::Sub(crate::mock::RoutedApp::<1>::network()),
            BridgeAppInfo::Sub(crate::mock::RoutedApp::<2>::network()),
        ]);
        assert_eq!(BridgeProxy::list_apps(), expected);
        assert_eq!(
            BridgeProxy::list_supported_assets(BASE_EVM_NETWORK_ID.into()).len(),
            3
        );
        for network in [
            crate::mock::RoutedApp::<0>::network(),
            crate::mock::RoutedApp::<1>::network(),
            crate::mock::RoutedApp::<2>::network(),
        ] {
            assert_eq!(
                BridgeProxy::list_supported_assets(network),
                vec![BridgeAssetInfo::Liberland]
            );
        }
        assert!(BridgeProxy::list_supported_assets(GenericNetworkId::EVMLegacy(999)).is_empty());
    });
}

#[test]
fn request_accessors_and_missing_status_notifications_preserve_evidence() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = create_refundable_transfer(sender.clone(), 123);
        let request = BridgeProxy::transactions((network, sender.clone()), id).unwrap();
        assert_eq!(request.source(), &GenericAccount::Sora(sender.clone()));
        assert_eq!(request.asset_id(), &XOR);
        assert_eq!(request.amount(), 123);
        assert_eq!(request.direction(), MessageDirection::Outbound);
        assert_eq!(request.status(), MessageStatus::InQueue);
        assert_eq!(BridgeProxy::sender(network, id), Some(sender.clone()));

        let missing = H256::repeat_byte(99);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        BridgeProxy::update_status(
            network,
            missing,
            MessageStatus::Failed,
            GenericTimepoint::EVM(10),
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        crate::Senders::<Test>::insert(network, missing, sender.clone());
        BridgeProxy::update_status(
            network,
            missing,
            MessageStatus::Done,
            GenericTimepoint::EVM(10),
        );
        assert!(BridgeProxy::transactions((network, sender), missing).is_none());
        assert!(crate::mock::bridge_logs().iter().any(|message| {
            message.contains("Message status update called for unknown message")
                && message.contains(&format!("{missing:?}"))
        }));
    });
}

#[test]
fn quarantined_requests_cannot_be_created_or_overwritten() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = H256::repeat_byte(3);
        sp_io::storage::set(&bridge_types::recovery::message_key(network, id), &[1]);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        BridgeProxy::inbound_request(
            network,
            id,
            GenericAccount::EVM(H160::zero()),
            sender.clone(),
            XOR,
            20,
            GenericTimepoint::EVM(3),
            MessageStatus::Done,
        );
        BridgeProxy::outbound_request(
            network,
            id,
            sender,
            GenericAccount::EVM(H160::zero()),
            XOR,
            20,
            MessageStatus::InQueue,
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
    });
}

#[test]
fn recovery_context_allows_inbound_status_without_triggering_refunds() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Alice.into();
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let id = H256::repeat_byte(3);
        BridgeProxy::inbound_request(
            network,
            id,
            GenericAccount::EVM(H160::zero()),
            sender.clone(),
            XOR,
            20,
            GenericTimepoint::EVM(3),
            MessageStatus::Committed,
        );
        inbound_context();
        BridgeProxy::update_status(
            network,
            id,
            MessageStatus::Failed,
            GenericTimepoint::Sora(1),
        );
        let request = BridgeProxy::transactions((network, sender), id).unwrap();
        assert_eq!(request.status(), MessageStatus::Failed);
        assert_eq!(request.end_timepoint, GenericTimepoint::Sora(1));
        assert_eq!(BridgeProxy::locked_assets(network, XOR), 0);
    });
}

#[test]
fn recovery_accounting_requires_current_context_and_legacy_owned_route() {
    new_tester().execute_with(|| {
        let legacy = GenericNetworkId::EVMLegacy(0);
        let current = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        for operation in [
            BridgeProxy::recovery_restore_extinguished_hashi_owned,
            BridgeProxy::recovery_extinguish_prepared_sidechain,
        ] {
            assert_noop!(
                operation(legacy, XOR, 10),
                crate::Error::<Test>::RecoveryHalted
            );
        }
        sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        assert_noop!(
            BridgeProxy::recovery_restore_extinguished_hashi_owned(legacy, XOR, 10),
            crate::Error::<Test>::RecoveryHalted
        );
        sp_io::storage::set(
            common::recovery::BRIDGE_INBOUND_CONTEXT_STORAGE_KEY,
            &0u32.to_le_bytes(),
        );
        assert_noop!(
            BridgeProxy::recovery_restore_extinguished_hashi_owned(legacy, XOR, 10),
            crate::Error::<Test>::RecoveryHalted
        );
        inbound_context();
        assert_noop!(
            BridgeProxy::recovery_restore_extinguished_hashi_owned(current, XOR, 10),
            crate::Error::<Test>::RecoveryHalted
        );
        assert_ok!(BridgeProxy::recovery_restore_extinguished_hashi_owned(
            legacy, XOR, 10
        ));
        assert_eq!(BridgeProxy::locked_assets(legacy, XOR), 10);
        assert_eq!(BridgeProxy::consumed_transfer_limit(), 0);
        assert!(crate::TransferLimitUnlockSchedule::<Test>::iter()
            .next()
            .is_none());
        assert_ok!(BridgeProxy::recovery_extinguish_prepared_sidechain(
            legacy, XOR, 4
        ));
        assert_eq!(BridgeProxy::locked_assets(legacy, XOR), 6);
        assert_noop!(
            BridgeProxy::recovery_extinguish_prepared_sidechain(legacy, XOR, 7),
            crate::Error::<Test>::NotEnoughLockedLiquidity
        );
        crate::LockedAssets::<Test>::insert(legacy, XOR, u128::MAX);
        assert_noop!(
            BridgeProxy::recovery_restore_extinguished_hashi_owned(legacy, XOR, 1),
            crate::Error::<Test>::Overflow
        );
        System::set_block_number(u64::from(u32::MAX) + 1);
        assert_noop!(
            BridgeProxy::recovery_extinguish_prepared_sidechain(legacy, XOR, 1),
            crate::Error::<Test>::RecoveryHalted
        );
    });
}

#[test]
fn lock_accounting_overflow_and_recovery_gate_preserve_existing_liquidity() {
    new_tester().execute_with(|| {
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        crate::LockedAssets::<Test>::insert(network, XOR, u128::MAX);
        assert_noop!(
            BridgeProxy::before_asset_lock(network, AssetKind::Thischain, &XOR, &1),
            crate::Error::<Test>::Overflow
        );
        assert_noop!(
            BridgeProxy::before_asset_unlock(network, AssetKind::Sidechain, &XOR, &1),
            crate::Error::<Test>::Overflow
        );
        assert_ok!(BridgeProxy::reset_locked_assets(network, XOR));
        assert!(!crate::LockedAssets::<Test>::contains_key(network, XOR));
        assert_noop!(
            BridgeProxy::before_asset_lock(network, AssetKind::Sidechain, &XOR, &1),
            crate::Error::<Test>::NotEnoughLockedLiquidity
        );
        sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        assert_noop!(
            BridgeProxy::before_asset_lock(network, AssetKind::Thischain, &XOR, &1),
            crate::Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            BridgeProxy::before_asset_unlock(network, AssetKind::Sidechain, &XOR, &1),
            crate::Error::<Test>::RecoveryHalted
        );
        inbound_context();
        assert_ok!(BridgeProxy::before_asset_unlock(
            network,
            AssetKind::Sidechain,
            &XOR,
            &10
        ));
        assert_eq!(BridgeProxy::locked_assets(network, XOR), 10);
    });
}

#[test]
fn sidechain_mint_and_burn_keep_supply_balances_and_locks_in_sync() {
    new_tester().execute_with(|| {
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let owner: AccountId = Keyring::Alice.into();
        assert_ok!(BridgeProxy::unlock_asset(
            network,
            AssetKind::Sidechain,
            &owner,
            &DAI,
            &100
        ));
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&DAI, &owner).unwrap(),
            100
        );
        assert_eq!(BridgeProxy::locked_assets(network, DAI), 100);
        assert_ok!(BridgeProxy::lock_asset(
            network,
            AssetKind::Sidechain,
            &owner,
            &DAI,
            &40
        ));
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&DAI, &owner).unwrap(),
            60
        );
        assert_eq!(BridgeProxy::locked_assets(network, DAI), 60);
    });
}

#[test]
fn fee_custody_round_trip_and_halt_preserve_balances() {
    new_tester().execute_with(|| {
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        let owner: AccountId = Keyring::Alice.into();
        assert_ok!(Currencies::update_balance(
            RawOrigin::Root.into(),
            owner.clone(),
            XOR,
            100
        ));
        let fee_account = BridgeProxy::bridge_fee_account(network).unwrap();
        assert_ne!(fee_account, BridgeProxy::bridge_account(network).unwrap());
        assert_ok!(BridgeProxy::withdraw_fee(network, &owner, &XOR, &40));
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&XOR, &owner).unwrap(),
            60
        );
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&XOR, &fee_account).unwrap(),
            40
        );
        assert_ok!(BridgeProxy::refund_fee(network, &owner, &XOR, &15));
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&XOR, &owner).unwrap(),
            75
        );
        assert_eq!(
            assets::Pallet::<Test>::free_balance(&XOR, &fee_account).unwrap(),
            25
        );
        sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        assert_noop!(
            BridgeProxy::withdraw_fee(network, &owner, &XOR, &1),
            crate::Error::<Test>::RecoveryHalted
        );
        assert_noop!(
            BridgeProxy::refund_fee(network, &owner, &XOR, &1),
            crate::Error::<Test>::RecoveryHalted
        );
    });
}

#[test]
fn fee_route_propagates_app_errors_and_rejects_unsupported_assets() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        assert_noop!(
            BridgeProxy::withdraw_transfer_fee(&owner, BASE_EVM_NETWORK_ID, TBCD),
            crate::Error::<Test>::PathIsNotAvailable
        );
        let expected = FungibleApp::withdraw_transfer_fee(&owner, BASE_EVM_NETWORK_ID, XOR);
        assert!(expected.is_err());
        assert_eq!(
            BridgeProxy::withdraw_transfer_fee(&owner, BASE_EVM_NETWORK_ID, XOR),
            expected
        );
    });
}

#[test]
fn asset_registration_exposes_metadata_and_idempotent_management() {
    new_tester().execute_with(|| {
        let network = GenericNetworkId::EVMLegacy(17);
        let asset = BridgeProxy::register_asset(
            network,
            AssetName(b"Bridge Asset".to_vec()),
            AssetSymbol(b"BRG".to_vec()),
        )
        .unwrap();
        assert!(BridgeProxy::ensure_asset_exists(asset));
        assert!(!BridgeProxy::ensure_asset_exists(TBCD));
        let info = BridgeProxy::get_raw_info(asset);
        assert_eq!(info.name, b"Bridge Asset");
        assert_eq!(info.symbol, b"BRG");
        assert_eq!(info.precision, 18);
        let manager = BridgeProxy::bridge_account(network).unwrap();
        assert_ok!(technical::Pallet::<Test>::ensure_account_registered(
            &manager
        ));
        assert_ok!(technical::Pallet::<Test>::ensure_account_registered(
            &BridgeProxy::bridge_fee_account(network).unwrap()
        ));
        assert_ok!(BridgeProxy::manage_asset(network, DAI));
        assert_ok!(BridgeProxy::manage_asset(network, DAI));
        for permission in [permissions::MINT, permissions::BURN] {
            assert_ok!(permissions::Pallet::<Test>::check_permission_with_scope(
                manager.clone(),
                permission,
                &permissions::Scope::Limited(common::hash(&DAI))
            ));
        }
    });
}

#[test]
fn limit_administration_requires_manager_and_unlock_saturates() {
    new_tester().execute_with(|| {
        let owner: AccountId = Keyring::Alice.into();
        assert_noop!(
            BridgeProxy::add_limited_asset(RawOrigin::Signed(owner.clone()).into(), XOR),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeProxy::remove_limited_asset(RawOrigin::Signed(owner.clone()).into(), XOR),
            sp_runtime::DispatchError::BadOrigin
        );
        assert_noop!(
            BridgeProxy::update_transfer_limit(
                RawOrigin::Signed(owner).into(),
                BridgeProxy::transfer_limit()
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        crate::ConsumedTransferLimit::<Test>::put(10);
        crate::TransferLimitUnlockSchedule::<Test>::insert(5, 20);
        BridgeProxy::on_initialize(4);
        assert_eq!(BridgeProxy::consumed_transfer_limit(), 10);
        assert_eq!(BridgeProxy::transfer_limit_unlock_schedule(5), 20);
        BridgeProxy::on_initialize(5);
        assert_eq!(BridgeProxy::consumed_transfer_limit(), 0);
        assert!(!crate::TransferLimitUnlockSchedule::<Test>::contains_key(5));
    });
}

#[test]
fn migrations_skip_current_versions_and_preserve_account_variants() {
    use crate::migrations::generic_account_v2::{LiberlandGenericAccount, OldGenericAccount};
    frame_support::parameter_types! {
        pub EmptyAssets: Vec<(crate::mock::AssetId, u128)> = vec![];
        pub const LegacyNetwork: GenericNetworkId = GenericNetworkId::EVMLegacy(0);
    }
    new_tester().execute_with(|| {
        StorageVersion::new(2).put::<BridgeProxy>();
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_eq!(LiberlandGenericAccount::<Test>::on_runtime_upgrade(), frame_support::weights::Weight::zero());
        assert_eq!(crate::migrations::init::InitLockedAssets::<Test, EmptyAssets, LegacyNetwork>::on_runtime_upgrade(), frame_support::weights::Weight::zero());
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        for (old, expected) in [
            (OldGenericAccount::EVM(H160::repeat_byte(9)), GenericAccount::EVM(H160::repeat_byte(9))),
            (OldGenericAccount::Unknown, GenericAccount::Unknown),
            (OldGenericAccount::Root, GenericAccount::Root),
        ] {
            let actual: GenericAccount = old.into();
            assert_eq!(actual, expected);
        }
    });
}

#[test]
fn burn_dispatch_weight_covers_the_most_expensive_route_and_discovery() {
    use frame_support::dispatch::GetDispatchInfo;
    new_tester().execute_with(|| {
        let call = crate::Call::<Test>::burn {
            network_id: GenericNetworkId::EVMLegacy(42),
            asset_id: XOR,
            recipient: GenericAccount::EVM(H160::repeat_byte(1)),
            amount: 10,
        };
        let declared = call.get_dispatch_info().call_weight;
        type EvmApp = <Test as crate::Config>::FAApp;
        let discovery = <EvmApp as BridgeApp<AccountId, H160, crate::mock::AssetId, u128>>::is_asset_supported_weight()
            .saturating_add(frame_support::weights::Weight::from_parts(10, 1));
        for route in [
            <EvmApp as BridgeApp<AccountId, H160, crate::mock::AssetId, u128>>::transfer_weight(),
            frame_support::weights::Weight::from_parts(100, 10),
            frame_support::weights::Weight::from_parts(101, 10),
            frame_support::weights::Weight::from_parts(102, 10),
        ] {
            assert!(declared.all_gte(route.saturating_add(discovery)));
        }
    });
}

#[test]
fn limited_lock_rejects_unrepresentable_prices_and_unlock_schedule_overflow() {
    use frame_support::storage::{with_transaction, TransactionOutcome};
    new_tester().execute_with(|| {
        let network = GenericNetworkId::EVM(BASE_EVM_NETWORK_ID);
        assert_ok!(BridgeProxy::add_limited_asset(RawOrigin::Root.into(), XOR));
        // Use the same transactional boundary as the bridge dispatchables.
        let lock = |amount| {
            with_transaction(|| {
                let result =
                    BridgeProxy::before_asset_lock(network, AssetKind::Thischain, &XOR, &amount);
                match result {
                    Ok(()) => TransactionOutcome::Commit(Ok(())),
                    Err(error) => TransactionOutcome::Rollback(Err(error)),
                }
            })
        };
        assert_noop!(lock(u128::MAX), crate::Error::<Test>::Overflow);
        let mut limit = BridgeProxy::transfer_limit();
        limit.max_amount = u128::MAX;
        crate::TransferLimit::<Test>::put(limit.clone());
        let unlock_at = System::block_number().saturating_add(limit.period_blocks);
        crate::TransferLimitUnlockSchedule::<Test>::insert(unlock_at, u128::MAX);
        assert_noop!(lock(balance!(1)), crate::Error::<Test>::Overflow);
        crate::TransferLimitUnlockSchedule::<Test>::remove(unlock_at);
        assert_ok!(lock(balance!(1)));
        assert_eq!(BridgeProxy::locked_assets(network, XOR), balance!(1));
        assert_eq!(BridgeProxy::consumed_transfer_limit(), balance!(2.5));
        assert_eq!(
            BridgeProxy::transfer_limit_unlock_schedule(unlock_at),
            balance!(2.5)
        );
    });
}
