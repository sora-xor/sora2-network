use crate::mock::{new_tester, BridgeProxy, Test, REFERENCE_PRICE_TEST_MODE};
use crate::{ConsumedTransferLimit, LimitedAssets, LockedAssets, TransferLimitUnlockSchedule};
use bridge_types::{traits::BridgeAssetLockChecker, types::AssetKind, GenericNetworkId};
use common::{balance, XOR};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError;

fn network() -> GenericNetworkId {
    GenericNetworkId::EVMLegacy(0)
}

#[test]
fn unavailable_price_rejects_limited_asset_without_partial_accounting() {
    for kind in [AssetKind::Thischain, AssetKind::Sidechain] {
        new_tester().execute_with(|| {
            LimitedAssets::<Test>::insert(XOR, true);
            LockedAssets::<Test>::insert(network(), XOR, balance!(10));
            ConsumedTransferLimit::<Test>::put(balance!(1));
            TransferLimitUnlockSchedule::<Test>::insert(601, balance!(1));
            sp_io::storage::set(REFERENCE_PRICE_TEST_MODE, b"unavailable");
            // Call the trait hook directly, without an outer extrinsic transaction.
            assert_noop!(
                BridgeProxy::before_asset_lock(network(), kind, &XOR, &balance!(1)),
                DispatchError::Other("reference quote unavailable")
            );
        });
    }
}

#[test]
fn unavailable_price_does_not_block_assets_without_a_transfer_limit() {
    new_tester().execute_with(|| {
        sp_io::storage::set(REFERENCE_PRICE_TEST_MODE, b"unavailable");
        assert_ok!(BridgeProxy::before_asset_lock(
            network(),
            AssetKind::Thischain,
            &XOR,
            &balance!(1)
        ));
        assert_eq!(LockedAssets::<Test>::get(network(), XOR), balance!(1));
        assert_eq!(ConsumedTransferLimit::<Test>::get(), 0);
        assert_eq!(TransferLimitUnlockSchedule::<Test>::get(601), 0);
    });
}

#[test]
fn zero_reference_price_rejects_a_limited_asset_without_accounting_changes() {
    new_tester().execute_with(|| {
        LimitedAssets::<Test>::insert(XOR, true);
        LockedAssets::<Test>::insert(network(), XOR, balance!(10));
        sp_io::storage::set(REFERENCE_PRICE_TEST_MODE, b"zero");
        assert_noop!(
            BridgeProxy::before_asset_lock(network(), AssetKind::Thischain, &XOR, &balance!(1)),
            crate::Error::<Test>::InvalidReferencePrice
        );
    });
}

#[test]
fn direct_hook_limit_failure_rolls_back_locked_balance() {
    new_tester().execute_with(|| {
        LimitedAssets::<Test>::insert(XOR, true);
        LockedAssets::<Test>::insert(network(), XOR, balance!(10));
        ConsumedTransferLimit::<Test>::put(BridgeProxy::transfer_limit().max_amount - 1);
        assert_noop!(
            BridgeProxy::before_asset_lock(network(), AssetKind::Thischain, &XOR, &balance!(1)),
            crate::Error::<Test>::TransferLimitReached
        );
    });
}

#[test]
fn unlock_schedule_overflow_rolls_back_both_accounting_updates() {
    new_tester().execute_with(|| {
        LimitedAssets::<Test>::insert(XOR, true);
        LockedAssets::<Test>::insert(network(), XOR, balance!(10));
        TransferLimitUnlockSchedule::<Test>::insert(601, u128::MAX);
        assert_noop!(
            BridgeProxy::before_asset_lock(network(), AssetKind::Thischain, &XOR, &balance!(1)),
            crate::Error::<Test>::Overflow
        );
    });
}

#[test]
fn cancelled_incoming_unlock_restores_only_custody_without_pricing_or_quota() {
    for mode in [b"unavailable".as_slice(), b"zero".as_slice()] {
        for (kind, expected) in [
            (AssetKind::Thischain, balance!(11)),
            (AssetKind::Sidechain, balance!(9)),
        ] {
            new_tester().execute_with(|| {
                LimitedAssets::<Test>::insert(XOR, true);
                LockedAssets::<Test>::insert(network(), XOR, balance!(10));
                ConsumedTransferLimit::<Test>::put(u128::MAX);
                TransferLimitUnlockSchedule::<Test>::insert(601, u128::MAX);
                sp_io::storage::set(REFERENCE_PRICE_TEST_MODE, mode);
                assert_ok!(BridgeProxy::cancel_asset_unlock(
                    network(),
                    kind,
                    &XOR,
                    &balance!(1)
                ));
                assert_eq!(LockedAssets::<Test>::get(network(), XOR), expected);
                assert_eq!(ConsumedTransferLimit::<Test>::get(), u128::MAX);
                assert_eq!(TransferLimitUnlockSchedule::<Test>::get(601), u128::MAX);
            });
        }
    }
}

#[test]
fn cancelled_incoming_unlock_preserves_halt_and_arithmetic_guards() {
    new_tester().execute_with(|| {
        LockedAssets::<Test>::insert(network(), XOR, u128::MAX);
        assert_noop!(
            BridgeProxy::cancel_asset_unlock(network(), AssetKind::Thischain, &XOR, &1),
            crate::Error::<Test>::Overflow
        );
        LockedAssets::<Test>::remove(network(), XOR);
        assert_noop!(
            BridgeProxy::cancel_asset_unlock(network(), AssetKind::Sidechain, &XOR, &1),
            crate::Error::<Test>::NotEnoughLockedLiquidity
        );
        sp_io::storage::set(common::recovery::BRIDGE_HALT_STORAGE_KEY, &[1]);
        assert_noop!(
            BridgeProxy::cancel_asset_unlock(network(), AssetKind::Thischain, &XOR, &1),
            crate::Error::<Test>::RecoveryHalted
        );
    });
}
