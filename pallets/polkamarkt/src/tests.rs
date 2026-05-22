use crate::{
    BinaryOutcome, ConditionCreators, ConditionInput, ConditionMarket, Error, Event,
    MarketCreatorFees, MarketPools, MarketPositionTotals, MarketPositions, MarketResolution,
    MarketStatus, PendingXorBuybackCollateral,
};
use frame_support::{
    assert_noop, assert_ok,
    storage::{storage_prefix, unhashed},
    traits::{OnRuntimeUpgrade, StorageVersion},
};
use frame_system::Pallet as System;
use sp_runtime::{DispatchError, Perbill};

use super::mock::*;
use super::mock::{
    balance_of, last_buyback_call, new_test_ext, run_to_block, xor_burned, BlockNumber,
    MinCreationFeeConst, RuntimeEvent, RuntimeOrigin, TradeFeeBpsConst, BUYBACK_ASSET,
    CANONICAL_ASSET, FEE_COLLECTOR, USDC_ASSET,
};

type Polkamarkt = crate::Pallet<Test>;

fn default_condition() -> ConditionInput {
    ConditionInput {
        question: b"Will SORA win?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"council-minutes".to_vec(),
    }
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
fn sell_quote_handles_selected_plus_shares_above_u128_max() {
    new_test_ext().execute_with(|| {
        let pool = crate::MarketPool {
            collateral: u128::MAX,
            yes: u128::MAX,
            no: u128::MAX,
        };

        assert_eq!(
            Polkamarkt::quote_sell(&pool, BinaryOutcome::Yes, 10).expect("quote succeeds"),
            4
        );
    });
}

#[test]
fn sell_quote_rejects_zero_reserve_invariant() {
    new_test_ext().execute_with(|| {
        let pool = crate::MarketPool {
            collateral: 100_000,
            yes: 0,
            no: 100_000,
        };

        assert_noop!(
            Polkamarkt::quote_sell(&pool, BinaryOutcome::Yes, 1),
            Error::<Test>::Overflow
        );
    });
}

#[test]
fn create_condition_charges_flat_fee_and_records_creator() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));

        assert!(crate::Conditions::<Test>::get(0).is_some());
        assert_eq!(ConditionCreators::<Test>::get(0), Some(ALICE));
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            alice_before - MinCreationFeeConst::get()
        );
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 8);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 2);
        assert_eq!(balance_of(Polkamarkt::account_id(), CANONICAL_ASSET), 2);
    });
}

#[test]
fn create_condition_rolls_back_when_fee_payment_fails() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        set_balance(ALICE, CANONICAL_ASSET, MinCreationFeeConst::get() - 1);

        assert_noop!(
            Polkamarkt::create_condition(RuntimeOrigin::signed(ALICE), default_condition()),
            DispatchError::Other("insufficient-balance")
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 0);
        assert!(crate::Conditions::<Test>::get(0).is_none());
        assert!(ConditionCreators::<Test>::get(0).is_none());
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            MinCreationFeeConst::get() - 1
        );
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 0);
        assert_eq!(balance_of(Polkamarkt::account_id(), CANONICAL_ASSET), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
    });
}

#[test]
fn create_condition_rejects_bad_origins_before_fee_collection() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_condition(RuntimeOrigin::root(), default_condition()),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::create_condition(RuntimeOrigin::none(), default_condition()),
            DispatchError::BadOrigin
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 0);
        assert!(crate::Conditions::<Test>::get(0).is_none());
        assert!(ConditionCreators::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
    });
}

#[test]
fn create_condition_counter_overflow_rolls_back_pallet_storage() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        crate::NextConditionId::<Test>::put(u32::MAX);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let fee_collector_before = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_condition(RuntimeOrigin::signed(ALICE), default_condition()),
            Error::<Test>::Overflow
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), u32::MAX);
        assert!(crate::Conditions::<Test>::get(u32::MAX).is_none());
        assert!(ConditionCreators::<Test>::get(u32::MAX).is_none());
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_before
        );
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn oversized_metadata_is_rejected_before_fee_collection() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let too_long = vec![b'Q'; MaxMetadataLengthConst::get() as usize + 1];

        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    question: too_long,
                    ..default_condition()
                },
            ),
            Error::<Test>::MetadataTooLong
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 0);
        assert!(crate::Conditions::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
    });
}

#[test]
fn oversized_oracle_and_source_are_rejected_before_fee_collection() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let too_long = vec![b'x'; MaxMetadataLengthConst::get() as usize + 1];

        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    oracle: too_long.clone(),
                    ..default_condition()
                },
            ),
            Error::<Test>::MetadataTooLong
        );
        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    resolution_source: too_long,
                    ..default_condition()
                },
            ),
            Error::<Test>::MetadataTooLong
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 0);
        assert!(crate::Conditions::<Test>::get(0).is_none());
        assert!(ConditionCreators::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
    });
}

#[test]
fn exact_max_metadata_with_local_source_is_accepted_once() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let question = vec![b'q'; MaxMetadataLengthConst::get() as usize];
        let oracle = vec![b'o'; MaxMetadataLengthConst::get() as usize];
        let resolution_source = vec![b's'; MaxMetadataLengthConst::get() as usize];
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            ConditionInput {
                question: question.clone(),
                oracle: oracle.clone(),
                resolution_source: resolution_source.clone(),
            },
        ));

        let stored = crate::Conditions::<Test>::get(0).expect("condition");
        assert_eq!(stored.question.to_vec(), question);
        assert_eq!(stored.oracle.to_vec(), oracle);
        assert_eq!(stored.resolution_source.to_vec(), resolution_source);
        assert_eq!(ConditionCreators::<Test>::get(0), Some(ALICE));
        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            alice_before - MinCreationFeeConst::get()
        );
    });
}

#[test]
fn pending_buyback_saturates_when_creation_fee_is_charged() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        PendingXorBuybackCollateral::<Test>::put(Balance::MAX - 1);

        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));

        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), Balance::MAX);
        assert_eq!(ConditionCreators::<Test>::get(0), Some(ALICE));
    });
}

#[test]
fn create_market_rejects_bad_origins_without_consuming_condition() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::root(), 0, 10, 100_000),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::none(), 0, 10, 100_000),
            DispatchError::BadOrigin
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn create_market_seeds_pool_and_does_not_charge_condition_fee_again() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        let pending_before = PendingXorBuybackCollateral::<Test>::get();
        let fee_collector_before = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10,
            100_000,
        ));

        let pool = MarketPools::<Test>::get(0).expect("market pool");
        assert_eq!(pool.collateral, 100_000);
        assert_eq!(pool.yes, 100_000);
        assert_eq!(pool.no, 100_000);
        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), pending_before);
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_before
        );
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before - 100_000);
    });
}

#[test]
fn create_market_leaves_noncanonical_balances_untouched() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
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
fn non_creator_cannot_create_market_from_condition() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(BOB), 0, 10, 100_000),
            Error::<Test>::NotConditionCreator
        );
    });
}

#[test]
fn legacy_condition_without_recorded_creator_is_not_marketable() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        ConditionCreators::<Test>::remove(0);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10, 100_000),
            Error::<Test>::NotConditionCreator
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
    });
}

#[test]
fn stale_condition_market_index_blocks_creation_without_side_effects() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        ConditionMarket::<Test>::insert(0, 777);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10, 100_000),
            Error::<Test>::ConditionAlreadyUsed
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert_eq!(ConditionMarket::<Test>::get(0), Some(777));
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn condition_cannot_be_reused_for_second_market() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 11, 100_000),
            Error::<Test>::ConditionAlreadyUsed
        );
    });
}

#[test]
fn finalized_condition_cannot_be_reused_for_new_market() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 20, 100_000),
            Error::<Test>::ConditionAlreadyUsed
        );

        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
        assert_eq!(crate::NextMarketId::<Test>::get(), 1);
        assert!(crate::Markets::<Test>::get(1).is_none());
        assert!(MarketPools::<Test>::get(1).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
    });
}

#[test]
fn failed_market_preflight_does_not_consume_condition() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10, 0),
            Error::<Test>::ZeroSeedLiquidity
        );
        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 5, 100_000),
            Error::<Test>::MarketDurationTooShort
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10,
            100_000,
        ));
        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
    });
}

#[test]
fn overflowing_market_close_window_does_not_consume_condition() {
    new_test_ext().execute_with(|| {
        run_to_block(BlockNumber::MAX - 1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, BlockNumber::MAX, 100_000),
            Error::<Test>::Overflow
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
    });
}

#[test]
fn next_market_id_overflow_does_not_seed_or_consume_condition() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        crate::NextMarketId::<Test>::put(u32::MAX);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10, 100_000),
            Error::<Test>::Overflow
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), u32::MAX);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Markets::<Test>::get(u32::MAX).is_none());
        assert!(MarketPools::<Test>::get(u32::MAX).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn failed_seed_transfer_rolls_back_market_id_and_condition_use() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        set_balance(ALICE, CANONICAL_ASSET, 99);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10, 100_000),
            DispatchError::Other("insufficient-balance")
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), 99);

        set_balance(ALICE, CANONICAL_ASSET, 100_000);
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10,
            100_000,
        ));
        assert_eq!(crate::NextMarketId::<Test>::get(), 1);
        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
    });
}

#[test]
fn missing_condition_market_creation_does_not_touch_market_state() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 42, 10, 100_000),
            Error::<Test>::ConditionNotFound
        );

        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(42).is_none());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
    });
}

#[test]
fn invalid_metadata_is_rejected() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    oracle: Vec::new(),
                    ..default_condition()
                },
            ),
            Error::<Test>::InvalidMetadata
        );
        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    resolution_source: Vec::new(),
                    ..default_condition()
                },
            ),
            Error::<Test>::InvalidMetadata
        );
        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    question: vec![0xff, 0xff, 0xff, 0xff],
                    ..default_condition()
                },
            ),
            Error::<Test>::InvalidMetadata
        );
        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    oracle: vec![0xff],
                    ..default_condition()
                },
            ),
            Error::<Test>::InvalidMetadata
        );
        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    resolution_source: vec![0xff],
                    ..default_condition()
                },
            ),
            Error::<Test>::InvalidMetadata
        );
        assert_noop!(
            Polkamarkt::create_condition(
                RuntimeOrigin::signed(ALICE),
                ConditionInput {
                    question: b"no".to_vec(),
                    oracle: Vec::new(),
                    resolution_source: Vec::new(),
                },
            ),
            Error::<Test>::QuestionTooShort
        );
        assert_eq!(crate::NextConditionId::<Test>::get(), 0);
        assert!(crate::Conditions::<Test>::get(0).is_none());
        assert!(ConditionCreators::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
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
fn fee_and_volume_counters_saturate_on_buy() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        MarketCreatorFees::<Test>::insert(0, Balance::MAX - 1);
        PendingXorBuybackCollateral::<Test>::put(Balance::MAX - 1);
        crate::MarketVolume::<Test>::insert(0, Balance::MAX - 1);

        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));

        assert_eq!(MarketCreatorFees::<Test>::get(0), Balance::MAX);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), Balance::MAX);
        assert_eq!(crate::MarketVolume::<Test>::get(0), Balance::MAX);
        assert!(MarketPositions::<Test>::get(0, BOB).is_some());
    });
}

#[test]
fn buy_rejects_position_accounting_overflow_before_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        let position = crate::MarketPosition {
            yes_shares: Balance::MAX,
            no_shares: 0,
            net_collateral_paid: 0,
        };
        MarketPositions::<Test>::insert(0, BOB, position.clone());
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_yes_shares = Balance::MAX;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), Some(position));
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), 0);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn buy_transfer_failure_does_not_mutate_market_state() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        set_balance(BOB, CANONICAL_ASSET, 9_999);
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0,),
            DispatchError::Other("insufficient-balance")
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(crate::MarketVolume::<Test>::get(0), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), 9_999);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn buy_slippage_failure_leaves_pool_balances_and_fee_buckets_untouched() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        let pool_before = MarketPools::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();

        assert_noop!(
            Polkamarkt::buy(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                10_000,
                u128::MAX,
            ),
            Error::<Test>::SlippageToleranceExceeded
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketPositionTotals::<Test>::get(0).total_yes_shares, 0);
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn buy_rejects_zero_and_unknown_market_without_mutation() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        let pool_before = MarketPools::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 0, 0),
            Error::<Test>::InvalidTradeAmount
        );
        assert_noop!(
            Polkamarkt::buy(
                RuntimeOrigin::signed(BOB),
                99,
                BinaryOutcome::Yes,
                10_000,
                0
            ),
            Error::<Test>::MarketUnknown
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn corrupted_pool_buy_quote_overflow_rejects_before_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        MarketPools::<Test>::mutate(0, |pool| {
            let pool = pool.as_mut().expect("pool");
            pool.yes = Balance::MAX;
            pool.no = 1;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                Balance::MAX / 2,
                0,
            ),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn corrupted_pool_buy_update_overflow_rejects_before_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        MarketPools::<Test>::mutate(0, |pool| {
            pool.as_mut().expect("pool").collateral = Balance::MAX;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), 0);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn corrupted_zero_reserve_pool_rejects_buy_without_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        MarketPools::<Test>::mutate(0, |pool| {
            pool.as_mut().expect("pool").no = 0;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), 0);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn missing_pool_rejects_open_market_trades_without_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);
        MarketPools::<Test>::remove(0);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::No, 10_000, 0),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1, 0),
            Error::<Test>::MarketUnknown
        );

        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn unknown_market_paths_do_not_create_or_mutate_state() {
    new_test_ext().execute_with(|| {
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let events_before = System::<Test>::events().len();

        assert_noop!(
            Polkamarkt::sync_market_status(RuntimeOrigin::signed(BOB), 77),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 77, BinaryOutcome::Yes, 1, 0),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 77),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(ALICE), 77),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 77),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::resolve_market(RuntimeOrigin::root(), 77, BinaryOutcome::Yes),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::cancel_market(RuntimeOrigin::root(), 77),
            Error::<Test>::MarketUnknown
        );

        assert!(crate::Markets::<Test>::get(77).is_none());
        assert!(MarketPools::<Test>::get(77).is_none());
        assert!(MarketPositions::<Test>::get(77, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(77), 0);
        assert_eq!(MarketPositionTotals::<Test>::get(77), Default::default());
        assert_eq!(crate::MarketVolume::<Test>::get(77), 0);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(System::<Test>::events().len(), events_before);
    });
}

#[test]
fn orphaned_unknown_market_state_is_not_drained_or_deleted() {
    new_test_ext().execute_with(|| {
        let position = crate::MarketPosition {
            yes_shares: 10,
            no_shares: 0,
            net_collateral_paid: 10,
        };
        MarketPositions::<Test>::insert(77, BOB, position.clone());
        MarketCreatorFees::<Test>::insert(77, 123);
        MarketPositionTotals::<Test>::insert(
            77,
            crate::MarketTotals {
                total_yes_shares: 10,
                total_no_shares: 0,
                total_net_collateral_paid: 10,
            },
        );
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 77),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(ALICE), 77),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 77),
            Error::<Test>::MarketUnknown
        );

        assert_eq!(MarketPositions::<Test>::get(77, BOB), Some(position));
        assert_eq!(MarketCreatorFees::<Test>::get(77), 123);
        assert_eq!(MarketPositionTotals::<Test>::get(77).total_yes_shares, 10);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn signed_extrinsics_reject_bad_origins_without_touching_market_state() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::root(), 0, BinaryOutcome::No, 10_000, 0),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::root(), 0, BinaryOutcome::Yes, 1, 0),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::sync_market_status(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::sweep_xor_buyback_and_burn(RuntimeOrigin::root()),
            DispatchError::BadOrigin
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
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
fn sell_wrong_outcome_or_transfer_failure_preserves_state() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));

        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let volume_before = crate::MarketVolume::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::No, 1, 0),
            Error::<Test>::InsufficientShares
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), volume_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);

        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 0);
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1_000, 0),
            DispatchError::Other("insufficient-balance")
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), volume_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn overselling_existing_outcome_does_not_mutate_position_or_fees() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                position.yes_shares + 1,
                0,
            ),
            Error::<Test>::InsufficientShares
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), Some(position));
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn corrupted_pool_collateral_underflow_on_sell_rolls_back_state() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        MarketPools::<Test>::mutate(0, |pool| {
            pool.as_mut().expect("pool").collateral = 0;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let volume_before = crate::MarketVolume::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1_000, 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), volume_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn deflated_share_totals_reject_sell_without_state_change() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_yes_shares = 999;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let volume_before = crate::MarketVolume::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1_000, 0,),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), volume_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn deflated_net_collateral_totals_reject_sell_without_state_change() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_net_collateral_paid = 0;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let volume_before = crate::MarketVolume::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1_000, 0,),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), volume_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn sell_without_shares_or_with_impossible_slippage_does_not_mutate() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1, 0),
            Error::<Test>::InsufficientShares
        );

        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                1_000,
                u128::MAX,
            ),
            Error::<Test>::SlippageToleranceExceeded
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn dust_sell_that_quotes_zero_collateral_is_rejected_without_state_change() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            1,
            0,
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let volume_before = crate::MarketVolume::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1, 0),
            Error::<Test>::TradeAmountTooSmall
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(crate::MarketVolume::<Test>::get(0), volume_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn finalized_markets_reject_new_trades_without_mutation() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::No, 10_000, 0),
            Error::<Test>::MarketNotOpen
        );
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1, 0),
            Error::<Test>::MarketNotOpen
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn sync_before_close_does_not_emit_events() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        let events_before = System::<Test>::events().len();

        assert_noop!(
            Polkamarkt::sync_market_status(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::MarketNotClosed
        );

        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Open
        );
        assert_eq!(System::<Test>::events().len(), events_before);
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
fn locked_market_rejects_sell_and_claim_without_payout() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::sync_market_status(
            RuntimeOrigin::signed(ALICE),
            0
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 1, 0),
            Error::<Test>::MarketNotOpen
        );
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::MarketNotFinalized
        );

        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Locked
        );
        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn trade_at_close_does_not_execute_trade_or_partial_lock() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
        let pool_before = MarketPools::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let events_before = System::<Test>::events().len();

        assert_eq!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0),
            Err(Error::<Test>::MarketNotOpen.into())
        );

        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Open
        );
        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(crate::MarketVolume::<Test>::get(0), 0);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(System::<Test>::events().len(), events_before);
    });
}

#[test]
fn claim_market_transfer_failure_rolls_back_position_and_totals() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 0);
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            DispatchError::Other("insufficient-balance")
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn deflated_share_totals_reject_claim_without_dropping_position() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_yes_shares = position.yes_shares - 1;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn deflated_net_collateral_totals_reject_cancelled_claim_without_dropping_position() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));
        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_net_collateral_paid = position.net_collateral_paid - 1;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn resolved_market_missing_resolution_rejects_payout_paths_without_mutation() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        MarketResolution::<Test>::remove(0);
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::MarketNotResolved
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::MarketNotResolved
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn missing_pool_rejects_payout_paths_without_dropping_positions() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        MarketPools::<Test>::remove(0);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::MarketUnknown
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::MarketUnknown
        );

        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn zero_position_is_not_claimable_and_is_not_deleted() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let zero_position = crate::MarketPosition {
            yes_shares: 0,
            no_shares: 0,
            net_collateral_paid: 0,
        };
        MarketPositions::<Test>::insert(0, BOB, zero_position.clone());
        let pool_before = MarketPools::<Test>::get(0);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NothingToClaim
        );

        assert_eq!(MarketPositions::<Test>::get(0, BOB), Some(zero_position));
        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn underfunded_resolved_pool_rejects_claim_without_dropping_position() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
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
        MarketPools::<Test>::mutate(0, |pool| {
            pool.as_mut().expect("pool").collateral = winning_shares - 1;
        });
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::Overflow
        );

        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn inflated_totals_block_creator_liquidity_without_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let pool_before = MarketPools::<Test>::get(0).expect("pool");
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_yes_shares = pool_before.collateral + 1;
        });
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );

        assert_eq!(MarketPools::<Test>::get(0), Some(pool_before));
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
    });
}

#[test]
fn inflated_cancelled_totals_block_creator_liquidity_without_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));
        let pool_before = MarketPools::<Test>::get(0).expect("pool");
        MarketPositionTotals::<Test>::mutate(0, |totals| {
            totals.total_net_collateral_paid = pool_before.collateral + 1;
        });
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );

        assert_eq!(MarketPools::<Test>::get(0), Some(pool_before));
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
    });
}

#[test]
fn losing_claim_is_single_use_and_pays_nothing() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::No,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NothingToClaim
        );
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
    });
}

#[test]
fn resolve_market_finalizes_and_allows_claims() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
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
fn finalization_rejects_bad_origin_early_and_duplicate_calls() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);

        assert_noop!(
            Polkamarkt::resolve_market(RuntimeOrigin::signed(ALICE), 0, BinaryOutcome::Yes),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::resolve_market(RuntimeOrigin::root(), 0, BinaryOutcome::Yes),
            Error::<Test>::MarketNotClosed
        );
        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Open
        );

        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        assert_noop!(
            Polkamarkt::resolve_market(RuntimeOrigin::root(), 0, BinaryOutcome::No),
            Error::<Test>::MarketAlreadyFinalized
        );
        assert_noop!(
            Polkamarkt::cancel_market(RuntimeOrigin::root(), 0),
            Error::<Test>::MarketAlreadyFinalized
        );
        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::Yes));
    });
}

#[test]
fn finalization_bad_origin_after_close_does_not_mutate_market() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
        let events_before = System::<Test>::events().len();

        assert_noop!(
            Polkamarkt::resolve_market(RuntimeOrigin::signed(ALICE), 0, BinaryOutcome::Yes),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::cancel_market(RuntimeOrigin::signed(ALICE), 0),
            DispatchError::BadOrigin
        );

        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Open
        );
        assert_eq!(MarketResolution::<Test>::get(0), None);
        assert_eq!(System::<Test>::events().len(), events_before);
    });
}

#[test]
fn sync_after_finalization_is_idempotent_and_emits_no_events() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let events_before = System::<Test>::events().len();

        assert_ok!(Polkamarkt::sync_market_status(
            RuntimeOrigin::signed(BOB),
            0
        ));

        assert_eq!(
            crate::Markets::<Test>::get(0).unwrap().status,
            MarketStatus::Resolved
        );
        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::Yes));
        assert_eq!(System::<Test>::events().len(), events_before);
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
fn creator_liquidity_before_trader_claim_keeps_winning_payout_locked() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
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

        assert_ok!(Polkamarkt::claim_creator_liquidity(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        assert_eq!(
            MarketPools::<Test>::get(0).unwrap().collateral,
            winning_shares
        );

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + winning_shares
        );
        assert_eq!(MarketPools::<Test>::get(0).unwrap().collateral, 0);
    });
}

#[test]
fn creator_liquidity_before_cancelled_refund_keeps_refund_locked() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let expected_refund = MarketPositions::<Test>::get(0, BOB)
            .unwrap()
            .net_collateral_paid;
        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));

        assert_ok!(Polkamarkt::claim_creator_liquidity(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        assert_eq!(
            MarketPools::<Test>::get(0).unwrap().collateral,
            expected_refund
        );

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + expected_refund
        );
        assert_eq!(MarketPools::<Test>::get(0).unwrap().collateral, 0);
    });
}

#[test]
fn cancelled_refund_claim_is_single_use() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let expected_refund = MarketPositions::<Test>::get(0, BOB)
            .unwrap()
            .net_collateral_paid;
        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + expected_refund
        );
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NothingToClaim
        );
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + expected_refund
        );
    });
}

#[test]
fn cancelled_market_ignores_stale_resolution_when_refunding() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let expected_refund = MarketPositions::<Test>::get(0, BOB)
            .unwrap()
            .net_collateral_paid;
        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));
        MarketResolution::<Test>::insert(0, BinaryOutcome::No);

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));

        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + expected_refund
        );
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::No));
    });
}

#[test]
fn creator_fee_claim_transfer_failure_and_duplicate_claims_do_not_pay_twice() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let fee_amount = MarketCreatorFees::<Test>::get(0);
        assert!(fee_amount > 0);

        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(ALICE), 0),
            DispatchError::Other("insufficient-balance")
        );
        assert_eq!(MarketCreatorFees::<Test>::get(0), fee_amount);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);

        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 1_000_000);
        assert_ok!(Polkamarkt::claim_creator_fees(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        let alice_after = balance_of(ALICE, CANONICAL_ASSET);
        assert_eq!(alice_after, alice_before + fee_amount);
        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after);
    });
}

#[test]
fn creator_fee_claim_without_fees_does_not_transfer() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );

        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
    });
}

#[test]
fn non_creator_fee_claim_does_not_clear_accrued_fees() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let fee_amount = MarketCreatorFees::<Test>::get(0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NotMarketCreator
        );

        assert_eq!(MarketCreatorFees::<Test>::get(0), fee_amount);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);

        assert_ok!(Polkamarkt::claim_creator_fees(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
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
fn creator_liquidity_claim_transfer_failure_and_duplicate_claims_do_not_pay_twice() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let pool_before = MarketPools::<Test>::get(0);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 0);
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            DispatchError::Other("insufficient-balance")
        );
        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);

        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 1_000_000);
        assert_ok!(Polkamarkt::claim_creator_liquidity(
            RuntimeOrigin::signed(ALICE),
            0,
        ));
        let alice_after = balance_of(ALICE, CANONICAL_ASSET);
        assert!(alice_after > alice_before);
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after);
    });
}

#[test]
fn claim_and_creator_withdrawal_negative_paths_do_not_payout() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::MarketNotFinalized
        );
        assert_noop!(
            Polkamarkt::claim_creator_fees(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NotMarketCreator
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::MarketNotFinalized
        );
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);

        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );
        assert_noop!(
            Polkamarkt::claim_creator_liquidity(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NotMarketCreator
        );
    });
}

#[test]
fn buyback_sweep_negative_paths_do_not_clear_or_burn_pending_collateral() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Polkamarkt::sweep_xor_buyback_and_burn(RuntimeOrigin::signed(BOB)),
            Error::<Test>::NothingToSweep
        );
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
        assert_eq!(last_buyback_call(), None);
        assert_eq!(xor_burned(), 0);

        PendingXorBuybackCollateral::<Test>::put(50);
        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 0);
        assert_noop!(
            Polkamarkt::sweep_xor_buyback_and_burn(RuntimeOrigin::signed(BOB)),
            DispatchError::Other("insufficient-balance")
        );
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 50);
        assert_eq!(last_buyback_call(), None);
        assert_eq!(xor_burned(), 0);
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
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(3));
    });
}

#[test]
fn migration_clears_legacy_opengov_prefix_and_sets_v2() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(1).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"OpengovConditions");
        let mut key = prefix.to_vec();
        key.extend_from_slice(&[1, 2, 3, 4]);
        unhashed::put_raw(&key, b"legacy");
        assert!(unhashed::contains_prefixed_key(&prefix));

        let _ = crate::migrations::v2::Migrate::<Test>::on_runtime_upgrade();

        assert!(!unhashed::contains_prefixed_key(&prefix));
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(2));
    });
}

#[test]
fn v2_migration_at_v2_clears_legacy_prefix_without_bumping_version() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"OpengovConditions");
        let mut key = prefix.to_vec();
        key.extend_from_slice(&[9, 9, 9, 9]);
        unhashed::put_raw(&key, b"legacy-after-v2");
        assert!(unhashed::contains_prefixed_key(&prefix));

        let _ = crate::migrations::v2::Migrate::<Test>::on_runtime_upgrade();

        assert!(!unhashed::contains_prefixed_key(&prefix));
        assert_eq!(unhashed::get_raw(&key), None);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(2));
    });
}

#[test]
fn v3_migration_clears_legacy_bond_config_and_sets_v3() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"GovernanceBondMinimumOverride");
        let mut key = prefix.to_vec();
        key.extend_from_slice(&[4, 3, 2, 1]);
        unhashed::put_raw(&key, b"legacy-bond-config");
        assert!(unhashed::contains_prefixed_key(&prefix));

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();

        assert!(!unhashed::contains_prefixed_key(&prefix));
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(3));
    });
}

#[test]
#[should_panic(expected = "Polkamarkt v3 migration requires empty legacy governance bond storage")]
fn v3_migration_rejects_legacy_governance_bond_claims() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"GovernanceBonds");
        let mut key = prefix.to_vec();
        key.extend_from_slice(&[4, 3, 2, 1]);
        unhashed::put_raw(&key, b"legacy-bond-claim");
        assert!(unhashed::contains_prefixed_key(&prefix));

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
    });
}
