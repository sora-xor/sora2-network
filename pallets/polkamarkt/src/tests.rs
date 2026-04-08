use crate::{
    BinaryOutcome, ConditionInput, CreatorLockedBond, Error, Event, MarketBondLock,
    MarketCreatorFees, MarketPools, MarketPositionTotals, MarketPositions, MarketResolution,
    MarketStatus, OpengovConditions, PendingXorBuybackCollateral, RelayNetwork,
};
use frame_support::{assert_noop, assert_ok, traits::StorageVersion};
use frame_system::Pallet as System;
use sp_runtime::Perbill;

use super::mock::*;
use super::mock::{
    balance_of, last_buyback_call, last_plaza_condition, new_test_ext, reset_plaza_notifications,
    run_to_block, xor_burned, BlockNumber, GovernanceBondMinimumConst, RuntimeEvent, RuntimeOrigin,
    TradeFeeBpsConst, BUYBACK_ASSET, CANONICAL_ASSET, FEE_COLLECTOR, MAINTENANCE_ACCOUNT,
    USDC_ASSET,
};

type Polkamarkt = crate::Pallet<Test>;

const TEST_BOND: Balance = 2_000;

fn default_condition() -> ConditionInput {
    ConditionInput {
        question: b"Will SORA win?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"https://example.com".to_vec(),
    }
}

fn bond_alice() {
    assert_ok!(Polkamarkt::bond_governance(
        RuntimeOrigin::signed(ALICE),
        TEST_BOND,
    ));
}

fn create_market(seed_liquidity: Balance, close_block: BlockNumber) {
    assert_ok!(Polkamarkt::create_condition(
        RuntimeOrigin::signed(ALICE),
        default_condition(),
    ));
    assert_ok!(Polkamarkt::create_market(
        RuntimeOrigin::signed(ALICE),
        0,
        close_block,
        seed_liquidity,
    ));
}

fn setup_market(seed_liquidity: Balance, close_block: BlockNumber) {
    run_to_block(1);
    bond_alice();
    create_market(seed_liquidity, close_block);
}

fn trade_fee(amount: Balance) -> Balance {
    Perbill::from_rational(TradeFeeBpsConst::get(), 10_000u32) * amount
}

fn fee_split(total_fee: Balance) -> (Balance, Balance, Balance) {
    let creator = total_fee * 10 / 100;
    let buyback = total_fee * 20 / 100;
    let pool = total_fee - creator - buyback;
    (pool, creator, buyback)
}

#[test]
fn creator_bond_escrows_collateral() {
    new_test_ext().execute_with(|| {
        assert_ok!(Polkamarkt::bond_governance(
            RuntimeOrigin::signed(ALICE),
            TEST_BOND,
        ));

        assert_eq!(crate::GovernanceBonds::<Test>::get(ALICE), TEST_BOND);
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            1_000_000_000_000 - TEST_BOND
        );
        assert_eq!(balance_of(MAINTENANCE_ACCOUNT, CANONICAL_ASSET), TEST_BOND);
    });
}

#[test]
fn create_market_seeds_pool_locks_bond_and_accrues_creation_buyback() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);

        let pool = MarketPools::<Test>::get(0).expect("market pool");
        assert_eq!(pool.collateral, 100_000);
        assert_eq!(pool.yes, 100_000);
        assert_eq!(pool.no, 100_000);
        assert_eq!(
            CreatorLockedBond::<Test>::get(ALICE),
            GovernanceBondMinimumConst::get()
        );
        assert_eq!(
            MarketBondLock::<Test>::get(0),
            Some(GovernanceBondMinimumConst::get())
        );
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 70);
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 280);
    });
}

#[test]
fn create_market_leaves_noncanonical_balances_untouched() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        let alice_usdc_before = balance_of(ALICE, USDC_ASSET);
        let pallet_usdc_before = balance_of(Polkamarkt::account_id(), USDC_ASSET);
        let fee_collector_usdc_before = balance_of(FEE_COLLECTOR, USDC_ASSET);

        create_market(100_000, 10);

        assert_eq!(balance_of(ALICE, USDC_ASSET), alice_usdc_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), USDC_ASSET),
            pallet_usdc_before
        );
        assert_eq!(
            balance_of(FEE_COLLECTOR, USDC_ASSET),
            fee_collector_usdc_before
        );
    });
}

#[test]
fn buy_updates_pool_positions_and_fee_buckets() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();

        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));

        let fee = trade_fee(10_000);
        let pricing_input = 10_000 - fee;
        let (pool_fee, creator_fee, buyback_fee) = fee_split(fee);
        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        let totals = MarketPositionTotals::<Test>::get(0);
        let pool = MarketPools::<Test>::get(0).expect("pool");

        assert_eq!(position.net_collateral_paid, pricing_input);
        assert!(position.yes_shares > 0);
        assert_eq!(position.no_shares, 0);
        assert_eq!(totals.total_yes_shares, position.yes_shares);
        assert_eq!(totals.total_net_collateral_paid, pricing_input);
        assert_eq!(pool.collateral, 100_000 + pricing_input + pool_fee);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fee);
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            buyback_before + buyback_fee
        );
    });
}

#[test]
fn sell_reduces_shares_and_net_collateral_paid() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let before_position = MarketPositions::<Test>::get(0, BOB).expect("position");
        let before_balance = balance_of(BOB, CANONICAL_ASSET);
        let before_creator_fees = MarketCreatorFees::<Test>::get(0);

        assert_ok!(Polkamarkt::sell(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            5_000,
            0,
        ));

        let after_position = MarketPositions::<Test>::get(0, BOB).expect("position");
        assert_eq!(
            after_position.yes_shares,
            before_position.yes_shares - 5_000
        );
        assert!(after_position.net_collateral_paid < before_position.net_collateral_paid);
        assert!(balance_of(BOB, CANONICAL_ASSET) > before_balance);
        assert!(MarketCreatorFees::<Test>::get(0) > before_creator_fees);
    });
}

#[test]
fn trading_is_rejected_after_close() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
        assert_ok!(Polkamarkt::sync_market_status(
            RuntimeOrigin::signed(BOB),
            0
        ));

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0),
            Error::<Test>::MarketNotOpen
        );
        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Locked
        );
        assert!(System::<Test>::events().iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Polkamarkt(Event::MarketLocked { market_id }) if market_id == 0
            )
        }));
    });
}

#[test]
fn resolve_market_auto_locks_finalizes_and_clears_opengov_metadata() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        assert_ok!(Polkamarkt::create_opengov_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
            crate::OpengovProposalInput {
                network: RelayNetwork::Polkadot,
                parachain_id: 1,
                track_id: 7,
                referendum_index: 11,
                plaza_tag: b"pm-7-11".to_vec(),
            },
        ));
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10,
            100_000,
        ));
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let winning_shares = MarketPositions::<Test>::get(0, BOB).unwrap().yes_shares;

        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));

        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Resolved
        );
        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::Yes));
        assert_eq!(CreatorLockedBond::<Test>::get(ALICE), 0);
        assert!(OpengovConditions::<Test>::get(0).is_none());
        assert!(System::<Test>::events().iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Polkamarkt(Event::OpengovConditionCleared { condition_id })
                    if condition_id == 0
            )
        }));

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + winning_shares
        );

        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_creator_liquidity(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        assert!(balance_of(ALICE, CANONICAL_ASSET) > alice_before);
    });
}

#[test]
fn cancel_market_auto_locks_and_refunds_net_collateral_paid() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        assert_ok!(Polkamarkt::sell(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            5_000,
            0,
        ));
        let expected_refund = MarketPositions::<Test>::get(0, BOB)
            .unwrap()
            .net_collateral_paid;

        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));
        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Cancelled
        );

        let before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), before + expected_refund);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
    });
}

#[test]
fn creator_can_claim_trading_fees() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));

        let amount = MarketCreatorFees::<Test>::get(0);
        let before = balance_of(ALICE, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_creator_fees(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), before + amount);
    });
}

#[test]
fn buyback_sweep_burns_accrued_collateral() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));

        let pending = PendingXorBuybackCollateral::<Test>::get();
        let before = xor_burned();
        assert_ok!(Polkamarkt::sweep_xor_buyback_and_burn(
            RuntimeOrigin::signed(BOB),
        ));
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
        assert_eq!(xor_burned(), before + pending);
    });
}

#[test]
fn buyback_sweep_uses_pallet_account_and_configured_assets() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));

        let pending = PendingXorBuybackCollateral::<Test>::get();
        assert_ok!(Polkamarkt::sweep_xor_buyback_and_burn(
            RuntimeOrigin::signed(ALICE),
        ));
        assert_eq!(
            last_buyback_call(),
            Some((
                Polkamarkt::account_id(),
                CANONICAL_ASSET,
                BUYBACK_ASSET,
                pending,
            ))
        );
    });
}

#[test]
fn sync_market_status_is_permissionless_and_idempotent() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);

        assert_ok!(Polkamarkt::sync_market_status(
            RuntimeOrigin::signed(BOB),
            0
        ));
        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Locked
        );

        let events_before = System::<Test>::events().len();
        assert_ok!(Polkamarkt::sync_market_status(
            RuntimeOrigin::signed(ALICE),
            0
        ));
        assert_eq!(System::<Test>::events().len(), events_before);
    });
}

#[test]
fn genesis_sets_current_storage_version() {
    new_test_ext().execute_with(|| {
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(1));
    });
}

#[test]
fn opengov_condition_broadcasts_to_plaza_hook() {
    new_test_ext().execute_with(|| {
        reset_plaza_notifications();
        run_to_block(1);
        bond_alice();
        assert_ok!(Polkamarkt::create_opengov_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
            crate::OpengovProposalInput {
                network: RelayNetwork::Polkadot,
                parachain_id: 1,
                track_id: 7,
                referendum_index: 11,
                plaza_tag: b"pm-7-11".to_vec(),
            },
        ));
        assert_eq!(last_plaza_condition(), Some(0));
        assert!(System::<Test>::events().iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Polkamarkt(Event::PolkadotPlazaBroadcast { condition_id, .. })
                    if condition_id == 0
            )
        }));
    });
}
