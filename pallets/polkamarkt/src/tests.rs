use crate::{
    BinaryOutcome, ConditionCreators, ConditionDetails, ConditionDetailsInput, ConditionInput,
    ConditionMarket, DpmCostBasisByAccount, DpmCostBasisTotals, Error, Event, EvidenceInput,
    LiquidityPosition, LiquidityPositionTotals, LiquidityPositions, LiquidityTotals, Market,
    MarketCancellationEvidence, MarketCreatorFees, MarketDpmCollateral, MarketMechanism,
    MarketOrderBookCollateral, MarketPools, MarketPositionTotals, MarketPositions,
    MarketResolution, MarketResolutionEvidence, MarketStatus, Markets, MigratedLegacyPayouts,
    Order, OrderSide, Orders, PendingXorBuybackCollateral,
};
use frame_support::{
    assert_noop, assert_ok,
    storage::{storage_prefix, unhashed},
    traits::{OnRuntimeUpgrade, StorageVersion},
    BoundedVec,
};
use frame_system::Pallet as System;
use sp_runtime::{DispatchError, Perbill};

use super::mock::*;
use super::mock::{
    balance_of, last_buyback_call, new_test_ext, run_to_block, xor_burned, BlockNumber,
    DpmVirtualSharesConst, MinCreationFeeConst, RuntimeEvent, RuntimeOrigin, TradeFeeBpsConst,
    CANONICAL_ASSET, FEE_COLLECTOR, LEGACY_BOND_ESCROW, USDC_ASSET,
};

type Polkamarkt = crate::Pallet<Test>;

fn default_condition() -> ConditionInput {
    ConditionInput {
        question: b"Will SORA win?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"council-minutes".to_vec(),
    }
}

fn default_condition_details() -> ConditionDetailsInput {
    ConditionDetailsInput::default()
}

fn make_legacy_market(
    market_id: crate::MarketId,
    condition_id: crate::ConditionId,
    creator: AccountId,
    seed_liquidity: Balance,
    close_block: BlockNumber,
) {
    let creator_before = balance_of(creator, CANONICAL_ASSET);
    let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);
    set_balance(
        creator,
        CANONICAL_ASSET,
        creator_before.saturating_sub(seed_liquidity),
    );
    set_balance(
        Polkamarkt::account_id(),
        CANONICAL_ASSET,
        pallet_before.saturating_add(seed_liquidity),
    );
    Markets::<Test>::insert(
        market_id,
        Market {
            creator,
            condition_id,
            close_block,
            collateral_asset: CANONICAL_ASSET,
            seed_liquidity,
            mechanism: MarketMechanism::LegacyAmm,
            status: MarketStatus::Open,
        },
    );
    MarketPools::<Test>::insert(
        market_id,
        crate::MarketPool {
            collateral: seed_liquidity,
            yes: seed_liquidity,
            no: seed_liquidity,
        },
    );
    LiquidityPositions::<Test>::insert(
        market_id,
        creator,
        LiquidityPosition {
            shares: seed_liquidity,
            collateral_contributed: seed_liquidity,
        },
    );
    LiquidityPositionTotals::<Test>::insert(
        market_id,
        LiquidityTotals {
            total_shares: seed_liquidity,
            total_collateral_contributed: seed_liquidity,
        },
    );
    ConditionMarket::<Test>::insert(condition_id, market_id);
    crate::NextMarketId::<Test>::mutate(|next_id| {
        if *next_id <= market_id {
            *next_id = market_id.saturating_add(1);
        }
    });
}

fn make_orderbook_market(
    market_id: crate::MarketId,
    condition_id: crate::ConditionId,
    creator: AccountId,
    close_block: BlockNumber,
) {
    Markets::<Test>::insert(
        market_id,
        Market {
            creator,
            condition_id,
            close_block,
            collateral_asset: CANONICAL_ASSET,
            seed_liquidity: 0,
            mechanism: MarketMechanism::OrderBook,
            status: MarketStatus::Open,
        },
    );
    ConditionMarket::<Test>::insert(condition_id, market_id);
    crate::NextMarketId::<Test>::mutate(|next_id| {
        if *next_id <= market_id {
            *next_id = market_id.saturating_add(1);
        }
    });
}

fn legacy_buy_order(
    owner: AccountId,
    market_id: crate::MarketId,
    reserved: Balance,
) -> Order<AccountId, Balance> {
    Order {
        owner,
        market_id,
        outcome: BinaryOutcome::Yes,
        side: OrderSide::Buy,
        price_cents: 50,
        remaining_shares: 0,
        reserved_collateral: reserved,
    }
}

fn insert_v4_legacy_market(
    market_id: crate::MarketId,
    condition_id: crate::ConditionId,
    seed_liquidity: Balance,
) {
    crate::migrations::v4::Markets::<Test>::insert(
        market_id,
        crate::migrations::LegacyMarket {
            creator: ALICE,
            condition_id,
            close_block: 10,
            collateral_asset: CANONICAL_ASSET,
            seed_liquidity,
            status: MarketStatus::Open,
        },
    );
}

fn insert_v5_legacy_market(
    market_id: crate::MarketId,
    condition_id: crate::ConditionId,
    creator: AccountId,
    close_block: BlockNumber,
    collateral_asset: AssetId,
    seed_liquidity: Balance,
    status: MarketStatus,
) {
    crate::migrations::v5::Markets::<Test>::insert(
        market_id,
        crate::migrations::LegacyMarket {
            creator,
            condition_id,
            close_block,
            collateral_asset,
            seed_liquidity,
            status,
        },
    );
}

fn setup_market(_seed_liquidity: Balance, close_block: BlockNumber) {
    setup_dpm_market(close_block);
}

fn setup_orderbook_market(close_block: BlockNumber) {
    run_to_block(1);
    assert_ok!(Polkamarkt::create_condition(
        RuntimeOrigin::signed(ALICE),
        default_condition(),
    ));
    make_orderbook_market(0, 0, ALICE, close_block);
}

fn setup_dpm_market(close_block: BlockNumber) {
    run_to_block(1);
    assert_ok!(Polkamarkt::create_condition_with_details(
        RuntimeOrigin::signed(ALICE),
        default_condition(),
        default_condition_details(),
    ));
    assert_ok!(Polkamarkt::create_market(
        RuntimeOrigin::signed(ALICE),
        0,
        close_block
    ));
}

fn trade_fee(amount: Balance) -> Balance {
    Perbill::from_rational(TradeFeeBpsConst::get(), 10_000u32) * amount
}

fn dpm_fee_split(total_fee: Balance) -> (Balance, Balance) {
    let creator = total_fee * 80 / 100;
    let buyback = total_fee - creator;
    (creator, buyback)
}

fn pro_rata_floor(amount: Balance, numerator: Balance, denominator: Balance) -> Balance {
    amount.saturating_mul(numerator) / denominator
}

fn assert_dpm_accounting_invariants(market_id: crate::MarketId) {
    let mut total_yes = 0;
    let mut total_no = 0;
    let mut total_net = 0;
    for (_, position) in MarketPositions::<Test>::iter_prefix(market_id) {
        total_yes += position.yes_shares;
        total_no += position.no_shares;
        total_net += position.net_collateral_paid;
    }
    let totals = MarketPositionTotals::<Test>::get(market_id);
    assert_eq!(totals.total_yes_shares, total_yes);
    assert_eq!(totals.total_no_shares, total_no);
    assert_eq!(totals.total_net_collateral_paid, total_net);

    let mut basis_yes = 0;
    let mut basis_no = 0;
    for (_, basis) in DpmCostBasisByAccount::<Test>::iter_prefix(market_id) {
        basis_yes += basis.yes;
        basis_no += basis.no;
    }
    let basis_totals = DpmCostBasisTotals::<Test>::get(market_id);
    assert_eq!(basis_totals.yes, basis_yes);
    assert_eq!(basis_totals.no, basis_no);
    assert_eq!(basis_yes + basis_no, total_net);
    assert!(
        MarketDpmCollateral::<Test>::get(market_id)
            <= balance_of(Polkamarkt::account_id(), CANONICAL_ASSET)
    );
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
            Polkamarkt::create_market(RuntimeOrigin::root(), 0, 10),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::none(), 0, 10),
            DispatchError::BadOrigin
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Conditions::<Test>::get(0).is_some());
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
fn create_market_creates_dpm_for_existing_condition_without_second_fee() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let pending_before = PendingXorBuybackCollateral::<Test>::get();
        let fee_collector_before = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);

        assert_ok!(Polkamarkt::create_condition_with_details(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
            ConditionDetailsInput {
                category: b"Crypto".to_vec(),
                tags: b"SORA,governance".to_vec(),
                metadata_uri: b"ipfs://market".to_vec(),
                metadata_hash: Some([9; 32]),
                rules_uri: b"ipfs://rules".to_vec(),
            }
        ));
        let alice_after_condition = balance_of(ALICE, CANONICAL_ASSET);
        let fee_collector_after_condition = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let pending_after_condition = PendingXorBuybackCollateral::<Test>::get();

        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10
        ));

        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), 1);
        assert!(crate::Conditions::<Test>::get(0).is_some());
        assert_eq!(ConditionCreators::<Test>::get(0), Some(ALICE));
        let details = ConditionDetails::<Test>::get(0).expect("details stored");
        assert_eq!(details.category.expect("category").to_vec(), b"Crypto");
        assert_eq!(details.metadata_hash, Some([9; 32]));
        let market = Markets::<Test>::get(0).expect("market");
        assert_eq!(market.creator, ALICE);
        assert_eq!(market.condition_id, 0);
        assert_eq!(market.seed_liquidity, 0);
        assert_eq!(market.mechanism, MarketMechanism::DynamicPariMutuel);
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(MarketDpmCollateral::<Test>::get(0), 0);
        assert!(DpmCostBasisByAccount::<Test>::get(0, ALICE).is_none());
        assert_eq!(DpmCostBasisTotals::<Test>::get(0).yes, 0);
        assert_eq!(DpmCostBasisTotals::<Test>::get(0).no, 0);
        assert!(LiquidityPositions::<Test>::get(0, ALICE).is_none());
        assert_eq!(LiquidityPositionTotals::<Test>::get(0).total_shares, 0);
        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
        assert_eq!(
            pending_after_condition,
            pending_before + MinCreationFeeConst::get() * 20 / 100
        );
        assert_eq!(
            fee_collector_after_condition,
            fee_collector_before + MinCreationFeeConst::get() * 80 / 100
        );
        assert_eq!(
            alice_after_condition,
            alice_before - MinCreationFeeConst::get()
        );
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after_condition);
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_after_condition
        );
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            pending_after_condition
        );
    });
}

#[test]
fn create_market_leaves_noncanonical_balances_untouched() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_usdc_before = balance_of(ALICE, USDC_ASSET);
        let pallet_usdc_before = balance_of(Polkamarkt::account_id(), USDC_ASSET);
        let fee_collector_usdc_before = balance_of(FEE_COLLECTOR, USDC_ASSET);

        setup_dpm_market(10);

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
fn dpm_buy_mints_one_sided_shares_and_records_fee_excluded_collateral() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let pending_before = PendingXorBuybackCollateral::<Test>::get();
        let collateral_in = 10_000;
        let fee = trade_fee(collateral_in);
        let pricing = collateral_in - fee;
        let (creator_fee, buyback_fee) = dpm_fee_split(fee);
        let quote =
            Polkamarkt::quote_buy_market(0, BinaryOutcome::Yes, collateral_in).expect("quote");

        assert_eq!(quote.fee_amount, fee);
        assert_eq!(quote.pricing_collateral, pricing);
        assert!(quote.shares_out > 0);

        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            collateral_in,
            quote.shares_out,
        ));

        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        let basis = DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis");
        let totals = MarketPositionTotals::<Test>::get(0);
        let basis_totals = DpmCostBasisTotals::<Test>::get(0);

        assert_eq!(position.yes_shares, quote.shares_out);
        assert_eq!(position.no_shares, 0);
        assert_eq!(position.net_collateral_paid, pricing);
        assert_eq!(basis.yes, pricing);
        assert_eq!(basis.no, 0);
        assert_eq!(totals.total_yes_shares, quote.shares_out);
        assert_eq!(totals.total_no_shares, 0);
        assert_eq!(basis_totals.yes, pricing);
        assert_eq!(basis_totals.no, 0);
        assert_eq!(MarketDpmCollateral::<Test>::get(0), pricing);
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(MarketOrderBookCollateral::<Test>::get(0), 0);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fee);
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            pending_before + buyback_fee
        );
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before - collateral_in);
    });
}

#[test]
fn dpm_sell_returns_net_collateral_and_reduces_shares_collateral_and_basis() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let position_before = MarketPositions::<Test>::get(0, BOB).expect("position");
        let basis_before = DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis");
        let dpm_collateral_before = MarketDpmCollateral::<Test>::get(0);
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let pending_before = PendingXorBuybackCollateral::<Test>::get();
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let shares_in = position_before.yes_shares / 2;
        let expected_basis_reduction =
            pro_rata_floor(basis_before.yes, shares_in, position_before.yes_shares);
        let quote = Polkamarkt::quote_sell_market(0, BinaryOutcome::Yes, shares_in).expect("quote");
        let (creator_fee, buyback_fee) = dpm_fee_split(quote.fee_amount);

        assert_ok!(Polkamarkt::sell(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            shares_in,
            quote.collateral_out,
        ));

        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        let basis = DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis");
        assert_eq!(position.yes_shares, position_before.yes_shares - shares_in);
        assert_eq!(
            position.net_collateral_paid,
            position_before.net_collateral_paid - expected_basis_reduction
        );
        assert_eq!(basis.yes, basis_before.yes - expected_basis_reduction);
        assert_eq!(
            DpmCostBasisTotals::<Test>::get(0).yes,
            basis_before.yes - expected_basis_reduction
        );
        assert_eq!(
            MarketDpmCollateral::<Test>::get(0),
            dpm_collateral_before - quote.gross_collateral_out
        );
        assert_eq!(
            MarketCreatorFees::<Test>::get(0),
            creator_fees_before + creator_fee
        );
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            pending_before + buyback_fee
        );
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + quote.collateral_out
        );
    });
}

#[test]
fn dpm_market_state_reports_marginal_price_and_implied_probability_separately() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        let initial = Polkamarkt::market_state(0).expect("state");
        assert_eq!(initial.mechanism, MarketMechanism::DynamicPariMutuel);
        assert_eq!(initial.virtual_depth, DpmVirtualSharesConst::get());
        assert_eq!(initial.real_yes_shares, 0);
        assert_eq!(initial.real_no_shares, 0);
        assert_eq!(initial.dpm_collateral, 0);
        assert_eq!(initial.implied_yes_probability_bps, 5_000);
        assert_eq!(initial.implied_no_probability_bps, 5_000);
        assert_eq!(
            initial.marginal_yes_price_bps,
            initial.marginal_no_price_bps
        );
        assert!(initial.marginal_yes_price_bps > initial.implied_yes_probability_bps);

        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let after = Polkamarkt::market_state(0).expect("state");
        assert_eq!(
            after.real_yes_shares,
            MarketPositionTotals::<Test>::get(0).total_yes_shares
        );
        assert_eq!(after.real_no_shares, 0);
        assert_eq!(after.dpm_collateral, MarketDpmCollateral::<Test>::get(0));
        assert!(after.marginal_yes_price_bps > after.marginal_no_price_bps);
        assert!(after.implied_yes_probability_bps > after.implied_no_probability_bps);
        assert_ne!(
            after.marginal_yes_price_bps,
            after.implied_yes_probability_bps
        );
    });
}

#[test]
fn dpm_resolved_winners_receive_collateral_pro_rata_with_last_claimant_dust() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(ALICE),
            0,
            BinaryOutcome::Yes,
            7_000,
            0,
        ));
        let bob_shares = MarketPositions::<Test>::get(0, BOB)
            .expect("bob")
            .yes_shares;
        let alice_shares = MarketPositions::<Test>::get(0, ALICE)
            .expect("alice")
            .yes_shares;
        let total_winning = bob_shares + alice_shares;
        let collateral = MarketDpmCollateral::<Test>::get(0);
        let bob_expected = pro_rata_floor(collateral, bob_shares, total_winning);

        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before + bob_expected);
        assert_eq!(
            MarketDpmCollateral::<Test>::get(0),
            collateral - bob_expected
        );

        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            alice_before + collateral - bob_expected
        );
        assert_eq!(MarketDpmCollateral::<Test>::get(0), 0);
        assert_eq!(MarketPositionTotals::<Test>::get(0).total_yes_shares, 0);
    });
}

#[test]
fn dpm_no_winner_resolution_routes_residual_collateral_to_buyback() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::No,
            10_000,
            0,
        ));
        let residual = MarketDpmCollateral::<Test>::get(0);
        let pending_before = PendingXorBuybackCollateral::<Test>::get();

        run_to_block(10);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));

        assert_eq!(MarketDpmCollateral::<Test>::get(0), 0);
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            pending_before + residual
        );
        assert!(System::<Test>::events().iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Polkamarkt(Event::DpmResidualBurned { market_id, amount })
                    if market_id == 0 && amount == residual
            )
        }));
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
    });
}

#[test]
fn dpm_cancelled_market_refunds_remaining_cost_basis_with_last_claimant_dust() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(ALICE),
            0,
            BinaryOutcome::No,
            5_000,
            0,
        ));
        let bob_before_sell = MarketPositions::<Test>::get(0, BOB).expect("bob");
        let bob_basis_before_sell = DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis");
        let shares_to_sell = bob_before_sell.yes_shares / 2;
        assert_ok!(Polkamarkt::sell(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            shares_to_sell,
            0,
        ));

        let bob_basis = DpmCostBasisByAccount::<Test>::get(0, BOB).expect("bob basis");
        assert!(bob_basis.yes < bob_basis_before_sell.yes);
        let alice_basis = DpmCostBasisByAccount::<Test>::get(0, ALICE).expect("alice basis");
        let bob_refund_basis = bob_basis.yes + bob_basis.no;
        let alice_refund_basis = alice_basis.yes + alice_basis.no;
        let total_refund_basis = bob_refund_basis + alice_refund_basis;
        let collateral = MarketDpmCollateral::<Test>::get(0);
        let bob_expected = pro_rata_floor(collateral, bob_refund_basis, total_refund_basis);

        run_to_block(10);
        assert_ok!(Polkamarkt::cancel_market(RuntimeOrigin::root(), 0));
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before + bob_expected);
        assert_eq!(
            MarketDpmCollateral::<Test>::get(0),
            collateral - bob_expected
        );

        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            alice_before + collateral - bob_expected
        );
        assert_eq!(MarketDpmCollateral::<Test>::get(0), 0);
        assert!(DpmCostBasisByAccount::<Test>::get(0, BOB).is_none());
        assert!(DpmCostBasisByAccount::<Test>::get(0, ALICE).is_none());
    });
}

#[test]
fn dpm_adversarial_sequence_preserves_invariants_and_batches_claims_once() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(20);
        assert_dpm_accounting_invariants(0);

        let bob_yes_quote =
            Polkamarkt::quote_buy_market(0, BinaryOutcome::Yes, 10_000).expect("bob yes quote");
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            bob_yes_quote.shares_out,
        ));
        assert_dpm_accounting_invariants(0);

        let alice_no_quote =
            Polkamarkt::quote_buy_market(0, BinaryOutcome::No, 7_333).expect("alice no quote");
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(ALICE),
            0,
            BinaryOutcome::No,
            7_333,
            alice_no_quote.shares_out,
        ));
        assert_dpm_accounting_invariants(0);

        let bob_position = MarketPositions::<Test>::get(0, BOB).expect("bob position");
        let bob_sell_shares = bob_position.yes_shares / 3;
        let bob_sell_quote = Polkamarkt::quote_sell_market(0, BinaryOutcome::Yes, bob_sell_shares)
            .expect("bob sell quote");
        assert_ok!(Polkamarkt::sell(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            bob_sell_shares,
            bob_sell_quote.collateral_out,
        ));
        assert_dpm_accounting_invariants(0);

        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::No, 1, 0),
            Error::<Test>::InsufficientShares
        );
        assert_dpm_accounting_invariants(0);

        let alice_position = MarketPositions::<Test>::get(0, ALICE).expect("alice position");
        let alice_sell_shares = alice_position.no_shares / 2;
        let alice_sell_quote =
            Polkamarkt::quote_sell_market(0, BinaryOutcome::No, alice_sell_shares)
                .expect("alice sell quote");
        assert_noop!(
            Polkamarkt::sell(
                RuntimeOrigin::signed(ALICE),
                0,
                BinaryOutcome::No,
                alice_sell_shares,
                alice_sell_quote.collateral_out + 1
            ),
            Error::<Test>::SlippageToleranceExceeded
        );
        assert_dpm_accounting_invariants(0);
        assert_ok!(Polkamarkt::sell(
            RuntimeOrigin::signed(ALICE),
            0,
            BinaryOutcome::No,
            alice_sell_shares,
            alice_sell_quote.collateral_out,
        ));
        assert_dpm_accounting_invariants(0);

        run_to_block(20);
        assert_ok!(Polkamarkt::resolve_market(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
        ));

        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let bob_claimable = Polkamarkt::claimable_info(BOB, 0).expect("bob claimable");
        assert!(bob_claimable.claimable_payout > 0);
        let bob_claims = BoundedVec::try_from(vec![99, 0, 0]).expect("bounded claims");
        assert_ok!(Polkamarkt::claim_markets(
            RuntimeOrigin::signed(BOB),
            bob_claims
        ));
        assert_eq!(
            balance_of(BOB, CANONICAL_ASSET),
            bob_before + bob_claimable.claimable_payout
        );
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert!(System::<Test>::events().iter().any(|record| {
            matches!(
                record.event,
                RuntimeEvent::Polkamarkt(Event::MarketClaimsBatched { trader, requested, claimed })
                    if trader == BOB && requested == 3 && claimed == 1
            )
        }));
        assert_dpm_accounting_invariants(0);

        let empty_bob_claims = BoundedVec::try_from(vec![99, 0]).expect("bounded claims");
        assert_noop!(
            Polkamarkt::claim_markets(RuntimeOrigin::signed(BOB), empty_bob_claims),
            Error::<Test>::NothingToClaim
        );
        assert_dpm_accounting_invariants(0);

        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let alice_claimable = Polkamarkt::claimable_info(ALICE, 0).expect("alice claimable");
        assert_eq!(alice_claimable.claimable_payout, 0);
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(ALICE), 0));
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert!(MarketPositions::<Test>::get(0, ALICE).is_none());
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(ALICE), 0),
            Error::<Test>::NothingToClaim
        );
        assert_dpm_accounting_invariants(0);
        assert_eq!(MarketDpmCollateral::<Test>::get(0), 0);
    });
}

#[test]
fn dpm_negative_paths_do_not_mutate_balances_or_ledgers() {
    new_test_ext().execute_with(|| {
        setup_dpm_market(10);
        let quote = Polkamarkt::quote_buy_market(0, BinaryOutcome::Yes, 10_000).expect("quote");
        let bob_before = balance_of(BOB, CANONICAL_ASSET);
        let dpm_before = MarketDpmCollateral::<Test>::get(0);
        assert_noop!(
            Polkamarkt::buy(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                10_000,
                quote.shares_out + 1
            ),
            Error::<Test>::SlippageToleranceExceeded
        );
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(MarketDpmCollateral::<Test>::get(0), dpm_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(DpmCostBasisTotals::<Test>::get(0).yes, 0);

        set_balance(BOB, CANONICAL_ASSET, 9_999);
        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 10_000, 0),
            DispatchError::Other("insufficient-balance")
        );
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), 9_999);
        assert_eq!(MarketDpmCollateral::<Test>::get(0), dpm_before);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert!(DpmCostBasisByAccount::<Test>::get(0, BOB).is_none());
        assert_eq!(DpmCostBasisTotals::<Test>::get(0).yes, 0);
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 2);
        set_balance(BOB, CANONICAL_ASSET, bob_before);

        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let position = MarketPositions::<Test>::get(0, BOB).expect("position");
        let basis = DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis");
        let collateral = MarketDpmCollateral::<Test>::get(0);
        assert_noop!(
            Polkamarkt::sell(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                position.yes_shares + 1,
                0
            ),
            Error::<Test>::InsufficientShares
        );
        assert_eq!(
            MarketPositions::<Test>::get(0, BOB).expect("position"),
            position
        );
        assert_eq!(
            DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis"),
            basis
        );
        assert_eq!(MarketDpmCollateral::<Test>::get(0), collateral);

        let sell_quote =
            Polkamarkt::quote_sell_market(0, BinaryOutcome::Yes, position.yes_shares / 2)
                .expect("sell quote");
        let creator_fees_before = MarketCreatorFees::<Test>::get(0);
        let buyback_before = PendingXorBuybackCollateral::<Test>::get();
        let totals_before = MarketPositionTotals::<Test>::get(0);
        let basis_totals_before = DpmCostBasisTotals::<Test>::get(0);
        let bob_after_buy = balance_of(BOB, CANONICAL_ASSET);
        set_balance(
            Polkamarkt::account_id(),
            CANONICAL_ASSET,
            sell_quote.collateral_out - 1,
        );
        assert_noop!(
            Polkamarkt::sell(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                position.yes_shares / 2,
                0
            ),
            DispatchError::Other("insufficient-balance")
        );
        assert_eq!(
            MarketPositions::<Test>::get(0, BOB).expect("position"),
            position
        );
        assert_eq!(
            DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis"),
            basis
        );
        assert_eq!(MarketPositionTotals::<Test>::get(0), totals_before);
        assert_eq!(DpmCostBasisTotals::<Test>::get(0), basis_totals_before);
        assert_eq!(MarketDpmCollateral::<Test>::get(0), collateral);
        assert_eq!(MarketCreatorFees::<Test>::get(0), creator_fees_before);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), buyback_before);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_after_buy);

        MarketDpmCollateral::<Test>::insert(0, sell_quote.gross_collateral_out - 1);
        assert_noop!(
            Polkamarkt::sell(
                RuntimeOrigin::signed(BOB),
                0,
                BinaryOutcome::Yes,
                position.yes_shares / 2,
                0
            ),
            Error::<Test>::Overflow
        );
        assert_eq!(
            MarketPositions::<Test>::get(0, BOB).expect("position"),
            position
        );
        assert_eq!(
            DpmCostBasisByAccount::<Test>::get(0, BOB).expect("basis"),
            basis
        );
        assert_eq!(
            MarketDpmCollateral::<Test>::get(0),
            sell_quote.gross_collateral_out - 1
        );
    });
}

#[test]
fn raw_orderbook_markets_are_frozen_until_v6_migration() {
    new_test_ext().execute_with(|| {
        setup_orderbook_market(10);
        MarketOrderBookCollateral::<Test>::insert(0, 100);
        MarketPositions::<Test>::insert(
            0,
            BOB,
            crate::MarketPosition {
                yes_shares: 20,
                no_shares: 80,
                net_collateral_paid: 0,
            },
        );
        MarketPositionTotals::<Test>::insert(
            0,
            crate::MarketTotals {
                total_yes_shares: 20,
                total_no_shares: 80,
                total_net_collateral_paid: 0,
            },
        );
        let position_before = MarketPositions::<Test>::get(0, BOB);
        let collateral_before = MarketOrderBookCollateral::<Test>::get(0);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 100, 1),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::No, 10, 0),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_noop!(
            Polkamarkt::quote_buy_market(0, BinaryOutcome::Yes, 100),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_noop!(
            Polkamarkt::quote_sell_market(0, BinaryOutcome::No, 10),
            Error::<Test>::UnsupportedMarketMechanism
        );
        run_to_block(10);
        assert_noop!(
            Polkamarkt::cancel_market(RuntimeOrigin::root(), 0),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_eq!(
            Markets::<Test>::get(0).expect("market").status,
            MarketStatus::Open
        );
        Markets::<Test>::mutate(0, |market| {
            market.as_mut().expect("market").status = MarketStatus::Cancelled;
        });
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
        assert_eq!(MarketOrderBookCollateral::<Test>::get(0), collateral_before);
    });
}

#[test]
fn raw_legacy_amm_markets_are_frozen_until_v6_migration() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        make_legacy_market(0, 0, ALICE, 1_000, 10);
        MarketPositions::<Test>::insert(
            0,
            BOB,
            crate::MarketPosition {
                yes_shares: 10,
                no_shares: 90,
                net_collateral_paid: 100,
            },
        );
        MarketPositionTotals::<Test>::insert(
            0,
            crate::MarketTotals {
                total_yes_shares: 10,
                total_no_shares: 90,
                total_net_collateral_paid: 100,
            },
        );
        let pool_before = MarketPools::<Test>::get(0);
        let position_before = MarketPositions::<Test>::get(0, BOB);

        assert_noop!(
            Polkamarkt::buy(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::Yes, 100, 1),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_noop!(
            Polkamarkt::sell(RuntimeOrigin::signed(BOB), 0, BinaryOutcome::No, 10, 0),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_noop!(
            Polkamarkt::quote_buy_market(0, BinaryOutcome::Yes, 100),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_noop!(
            Polkamarkt::quote_sell_market(0, BinaryOutcome::No, 10),
            Error::<Test>::UnsupportedMarketMechanism
        );
        run_to_block(10);
        assert_noop!(
            Polkamarkt::resolve_market(RuntimeOrigin::root(), 0, BinaryOutcome::No),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_eq!(
            Markets::<Test>::get(0).expect("market").status,
            MarketStatus::Open
        );
        Markets::<Test>::mutate(0, |market| {
            market.as_mut().expect("market").status = MarketStatus::Resolved;
        });
        MarketResolution::<Test>::insert(0, BinaryOutcome::No);
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::UnsupportedMarketMechanism
        );
        assert_eq!(MarketPools::<Test>::get(0), pool_before);
        assert_eq!(MarketPositions::<Test>::get(0, BOB), position_before);
    });
}

#[test]
fn failed_market_preflight_does_not_consume_condition_or_fee() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        let alice_after_condition = balance_of(ALICE, CANONICAL_ASSET);
        let fee_collector_after_condition = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let buyback_after_condition = PendingXorBuybackCollateral::<Test>::get();

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 5),
            Error::<Test>::MarketDurationTooShort
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Conditions::<Test>::get(0).is_some());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after_condition);
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_after_condition
        );
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            buyback_after_condition
        );
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10
        ));
        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
    });
}

#[test]
fn stale_next_condition_market_index_blocks_creation_without_fee() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        ConditionMarket::<Test>::insert(0, 777);
        let alice_after_condition = balance_of(ALICE, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);
        let fee_collector_after_condition = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let buyback_after_condition = PendingXorBuybackCollateral::<Test>::get();

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10),
            Error::<Test>::ConditionAlreadyUsed
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert_eq!(ConditionMarket::<Test>::get(0), Some(777));
        assert!(crate::Conditions::<Test>::get(0).is_some());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after_condition);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_after_condition
        );
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            buyback_after_condition
        );
    });
}

#[test]
fn overflowing_market_close_window_does_not_consume_condition_or_fee() {
    new_test_ext().execute_with(|| {
        run_to_block(BlockNumber::MAX - 1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        let alice_after_condition = balance_of(ALICE, CANONICAL_ASSET);
        let fee_collector_after_condition = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let buyback_after_condition = PendingXorBuybackCollateral::<Test>::get();

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, BlockNumber::MAX),
            Error::<Test>::Overflow
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), 0);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Conditions::<Test>::get(0).is_some());
        assert!(crate::Markets::<Test>::get(0).is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after_condition);
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_after_condition
        );
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            buyback_after_condition
        );
    });
}

#[test]
fn next_market_id_overflow_does_not_bind_condition_or_charge_fee() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        crate::NextMarketId::<Test>::put(u32::MAX);
        let alice_after_condition = balance_of(ALICE, CANONICAL_ASSET);
        let pallet_before = balance_of(Polkamarkt::account_id(), CANONICAL_ASSET);
        let fee_collector_after_condition = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let buyback_after_condition = PendingXorBuybackCollateral::<Test>::get();

        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 10),
            Error::<Test>::Overflow
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), u32::MAX);
        assert!(ConditionMarket::<Test>::get(0).is_none());
        assert!(crate::Conditions::<Test>::get(0).is_some());
        assert!(crate::Markets::<Test>::get(u32::MAX).is_none());
        assert!(MarketPools::<Test>::get(u32::MAX).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_after_condition);
        assert_eq!(
            balance_of(Polkamarkt::account_id(), CANONICAL_ASSET),
            pallet_before
        );
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_after_condition
        );
        assert_eq!(
            PendingXorBuybackCollateral::<Test>::get(),
            buyback_after_condition
        );
    });
}

#[test]
fn create_market_does_not_require_seed_liquidity() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        set_balance(ALICE, CANONICAL_ASSET, MinCreationFeeConst::get());

        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
        ));
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            10
        ));
        assert_eq!(crate::NextConditionId::<Test>::get(), 1);
        assert_eq!(crate::NextMarketId::<Test>::get(), 1);
        assert_eq!(ConditionMarket::<Test>::get(0), Some(0));
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
fn trade_at_close_does_not_execute_trade_or_partial_lock() {
    new_test_ext().execute_with(|| {
        setup_market(100_000, 10);
        run_to_block(10);
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
        assert_eq!(MarketDpmCollateral::<Test>::get(0), 0);
        assert!(MarketPositions::<Test>::get(0, BOB).is_none());
        assert_eq!(MarketCreatorFees::<Test>::get(0), 0);
        assert_eq!(crate::MarketVolume::<Test>::get(0), 0);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before);
        assert_eq!(System::<Test>::events().len(), events_before);
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
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(6));
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
fn v2_migration_exact_cap_clears_all_legacy_opengov_entries() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(1).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"OpengovConditions");
        for i in 0..crate::migrations::MAX_LEGACY_OPENGOV_CONDITIONS {
            let mut key = prefix.to_vec();
            key.extend_from_slice(&i.to_le_bytes());
            unhashed::put_raw(&key, b"legacy-condition");
        }

        let _ = crate::migrations::v2::Migrate::<Test>::on_runtime_upgrade();

        assert!(!unhashed::contains_prefixed_key(&prefix));
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(2));
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration OpengovConditions exceeds limit 1024")]
fn v2_migration_panics_when_legacy_opengov_prefix_exceeds_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(1).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"OpengovConditions");
        for i in 0..=crate::migrations::MAX_LEGACY_OPENGOV_CONDITIONS {
            let mut key = prefix.to_vec();
            key.extend_from_slice(&i.to_le_bytes());
            unhashed::put_raw(&key, b"legacy-condition");
        }

        let _ = crate::migrations::v2::Migrate::<Test>::on_runtime_upgrade();
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
fn v3_migration_refunds_legacy_governance_bond_claims() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let alice_bond = 1_000;
        let bob_bond = 2_000;
        set_balance(LEGACY_BOND_ESCROW, CANONICAL_ASSET, alice_bond + bob_bond);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        crate::migrations::v3::GovernanceBonds::<Test>::insert(ALICE, alice_bond);
        crate::migrations::v3::GovernanceBonds::<Test>::insert(BOB, bob_bond);
        crate::migrations::v3::CreatorLockedBond::<Test>::insert(ALICE, alice_bond);
        crate::migrations::v3::MarketBondLock::<Test>::insert(0, alice_bond);
        crate::migrations::v3::GovernanceBondMinimumOverride::<Test>::put(alice_bond);

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();

        assert_eq!(balance_of(LEGACY_BOND_ESCROW, CANONICAL_ASSET), 0);
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            alice_before + alice_bond
        );
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before + bob_bond);
        assert!(crate::migrations::v3::GovernanceBonds::<Test>::iter()
            .next()
            .is_none());
        assert!(crate::migrations::v3::CreatorLockedBond::<Test>::iter()
            .next()
            .is_none());
        assert!(crate::migrations::v3::MarketBondLock::<Test>::iter()
            .next()
            .is_none());
        assert!(crate::migrations::v3::GovernanceBondMinimumOverride::<Test>::get().is_none());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(3));
    });
}

#[test]
fn v3_migration_panics_pre_v2_state_without_touching_legacy_storage() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(1).put::<Polkamarkt>();
        crate::migrations::v3::GovernanceBonds::<Test>::insert(ALICE, 777);
        crate::migrations::v3::CreatorLockedBond::<Test>::insert(ALICE, 888);
        crate::migrations::v3::MarketBondLock::<Test>::insert(1, 999);
        crate::migrations::v3::GovernanceBondMinimumOverride::<Test>::put(111);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        set_balance(LEGACY_BOND_ESCROW, CANONICAL_ASSET, 777);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
        }));

        assert!(result.is_err());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(1));
        assert_eq!(
            crate::migrations::v3::GovernanceBonds::<Test>::get(ALICE),
            777
        );
        assert_eq!(
            crate::migrations::v3::CreatorLockedBond::<Test>::get(ALICE),
            888
        );
        assert_eq!(
            crate::migrations::v3::MarketBondLock::<Test>::get(1),
            Some(999)
        );
        assert_eq!(
            crate::migrations::v3::GovernanceBondMinimumOverride::<Test>::get(),
            Some(111)
        );
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(balance_of(LEGACY_BOND_ESCROW, CANONICAL_ASSET), 777);
    });
}

#[test]
fn v3_migration_accepts_exact_governance_bond_cap_and_clears_related_storage() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let mut expected_refund = 0;
        let mut accounts = Vec::new();
        for i in 0..crate::migrations::MAX_LEGACY_GOVERNANCE_BONDS {
            let account = 100 + i as AccountId;
            let amount = (i as Balance) + 1;
            accounts.push((account, amount, balance_of(account, CANONICAL_ASSET)));
            expected_refund += amount;
            crate::migrations::v3::GovernanceBonds::<Test>::insert(account, amount);
        }
        crate::migrations::v3::CreatorLockedBond::<Test>::insert(ALICE, 5);
        crate::migrations::v3::MarketBondLock::<Test>::insert(7, 6);
        crate::migrations::v3::GovernanceBondMinimumOverride::<Test>::put(4);
        set_balance(LEGACY_BOND_ESCROW, CANONICAL_ASSET, expected_refund);

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();

        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(3));
        assert_eq!(balance_of(LEGACY_BOND_ESCROW, CANONICAL_ASSET), 0);
        for (account, amount, before) in accounts {
            assert_eq!(balance_of(account, CANONICAL_ASSET), before + amount);
        }
        assert!(crate::migrations::v3::GovernanceBonds::<Test>::iter()
            .next()
            .is_none());
        assert!(crate::migrations::v3::CreatorLockedBond::<Test>::iter()
            .next()
            .is_none());
        assert!(crate::migrations::v3::MarketBondLock::<Test>::iter()
            .next()
            .is_none());
        assert!(crate::migrations::v3::GovernanceBondMinimumOverride::<Test>::get().is_none());
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration GovernanceBonds exceeds limit 16")]
fn v3_migration_panics_when_governance_bonds_exceed_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let accounts = 10..=10 + crate::migrations::MAX_LEGACY_GOVERNANCE_BONDS;
        set_balance(
            LEGACY_BOND_ESCROW,
            CANONICAL_ASSET,
            accounts.clone().count() as Balance,
        );
        for account in accounts {
            crate::migrations::v3::GovernanceBonds::<Test>::insert(account as AccountId, 1);
        }

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration CreatorLockedBond exceeds limit 1024")]
fn v3_migration_panics_when_creator_locked_bonds_exceed_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        for i in 0..=crate::migrations::MAX_LEGACY_CREATOR_LOCKED_BONDS {
            crate::migrations::v3::CreatorLockedBond::<Test>::insert(200 + i as AccountId, 1);
        }

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration MarketBondLock exceeds limit 1024")]
fn v3_migration_panics_when_market_bond_locks_exceed_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        for market_id in 0..=crate::migrations::MAX_LEGACY_MARKET_BOND_LOCKS {
            crate::migrations::v3::MarketBondLock::<Test>::insert(market_id, 1);
        }

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration GovernanceBondMinimumOverride exceeds limit 16")]
fn v3_migration_panics_when_governance_bond_config_prefix_exceeds_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let prefix = storage_prefix(b"Polkamarkt", b"GovernanceBondMinimumOverride");
        for i in 0..=crate::migrations::MAX_LEGACY_GOVERNANCE_BOND_CONFIGS {
            let mut key = prefix.to_vec();
            key.extend_from_slice(&i.to_le_bytes());
            unhashed::put_raw(&key, b"legacy-config");
        }

        let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
    });
}

#[test]
fn v3_migration_rolls_back_when_legacy_escrow_cannot_refund() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        let alice_bond = 1_000;
        set_balance(LEGACY_BOND_ESCROW, CANONICAL_ASSET, alice_bond - 1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let escrow_before = balance_of(LEGACY_BOND_ESCROW, CANONICAL_ASSET);
        crate::migrations::v3::GovernanceBonds::<Test>::insert(ALICE, alice_bond);
        crate::migrations::v3::CreatorLockedBond::<Test>::insert(ALICE, alice_bond);
        crate::migrations::v3::MarketBondLock::<Test>::insert(0, alice_bond);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
        }));
        assert!(result.is_err());

        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(LEGACY_BOND_ESCROW, CANONICAL_ASSET),
            escrow_before
        );
        assert_eq!(
            crate::migrations::v3::GovernanceBonds::<Test>::get(ALICE),
            alice_bond
        );
        assert_eq!(
            crate::migrations::v3::CreatorLockedBond::<Test>::get(ALICE),
            alice_bond
        );
        assert_eq!(
            crate::migrations::v3::MarketBondLock::<Test>::get(0),
            Some(alice_bond)
        );
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(2));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v4::Migrate::<Test>::on_runtime_upgrade();
        }));
        assert!(result.is_err());

        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(2));
    });
}

#[test]
fn v3_migration_panics_before_v2_completes() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(1).put::<Polkamarkt>();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v3::Migrate::<Test>::on_runtime_upgrade();
        }));

        assert!(result.is_err());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(1));
    });
}

#[test]
fn create_condition_with_details_stores_ui_metadata() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);

        assert_ok!(Polkamarkt::create_condition_with_details(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
            ConditionDetailsInput {
                category: b"Crypto".to_vec(),
                tags: b"SORA,governance".to_vec(),
                metadata_uri: b"ipfs://metadata".to_vec(),
                metadata_hash: Some([7; 32]),
                rules_uri: b"ipfs://rules".to_vec(),
            },
        ));

        let details = ConditionDetails::<Test>::get(0).expect("details stored");
        assert_eq!(details.category.expect("category").to_vec(), b"Crypto");
        assert_eq!(details.tags.expect("tags").to_vec(), b"SORA,governance");
        assert_eq!(
            details.metadata_uri.expect("metadata uri").to_vec(),
            b"ipfs://metadata"
        );
        assert_eq!(details.metadata_hash, Some([7; 32]));
        assert_eq!(
            details.rules_uri.expect("rules uri").to_vec(),
            b"ipfs://rules"
        );
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            alice_before - MinCreationFeeConst::get()
        );
    });
}

#[test]
fn create_condition_with_details_rejects_adversarial_details_without_fee() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let fee_collector_before = balance_of(FEE_COLLECTOR, CANONICAL_ASSET);
        let invalid_utf8 = vec![0xff, 0xfe, 0xfd];
        let too_long = vec![b'x'; MaxMetadataLengthConst::get() as usize + 1];

        assert_noop!(
            Polkamarkt::create_condition_with_details(
                RuntimeOrigin::signed(ALICE),
                default_condition(),
                ConditionDetailsInput {
                    category: invalid_utf8,
                    tags: Vec::new(),
                    metadata_uri: Vec::new(),
                    metadata_hash: Some([1; 32]),
                    rules_uri: Vec::new(),
                },
            ),
            Error::<Test>::InvalidMetadata
        );
        assert_noop!(
            Polkamarkt::create_condition_with_details(
                RuntimeOrigin::signed(ALICE),
                default_condition(),
                ConditionDetailsInput {
                    category: Vec::new(),
                    tags: Vec::new(),
                    metadata_uri: too_long,
                    metadata_hash: Some([2; 32]),
                    rules_uri: Vec::new(),
                },
            ),
            Error::<Test>::MetadataTooLong
        );

        assert_eq!(crate::NextConditionId::<Test>::get(), 0);
        assert!(crate::Conditions::<Test>::get(0).is_none());
        assert!(ConditionDetails::<Test>::get(0).is_none());
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before);
        assert_eq!(
            balance_of(FEE_COLLECTOR, CANONICAL_ASSET),
            fee_collector_before
        );
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 0);
    });
}

#[test]
fn resolve_with_evidence_stores_resolution_evidence() {
    new_test_ext().execute_with(|| {
        setup_market(1_000, 10);
        run_to_block(10);

        assert_ok!(Polkamarkt::resolve_market_with_evidence(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::Yes,
            EvidenceInput {
                uri: b"ipfs://resolution".to_vec(),
                hash: Some([9; 32]),
            },
        ));

        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::Yes));
        let evidence = MarketResolutionEvidence::<Test>::get(0).expect("evidence stored");
        assert_eq!(evidence.uri.to_vec(), b"ipfs://resolution");
        assert_eq!(evidence.hash, Some([9; 32]));
        assert_eq!(evidence.at_block, 10);
    });
}

#[test]
fn resolve_with_evidence_rejects_bad_origin_and_invalid_evidence() {
    new_test_ext().execute_with(|| {
        setup_market(1_000, 10);
        run_to_block(10);

        assert_noop!(
            Polkamarkt::resolve_market_with_evidence(
                RuntimeOrigin::signed(ALICE),
                0,
                BinaryOutcome::Yes,
                EvidenceInput {
                    uri: b"ipfs://resolution".to_vec(),
                    hash: Some([9; 32]),
                },
            ),
            DispatchError::BadOrigin
        );
        assert_eq!(
            Markets::<Test>::get(0).expect("market").status,
            MarketStatus::Open
        );

        assert_noop!(
            Polkamarkt::resolve_market_with_evidence(
                RuntimeOrigin::root(),
                0,
                BinaryOutcome::Yes,
                EvidenceInput {
                    uri: Vec::new(),
                    hash: Some([9; 32]),
                },
            ),
            Error::<Test>::InvalidEvidence
        );
        assert_eq!(
            Markets::<Test>::get(0).expect("market").status,
            MarketStatus::Open
        );
        assert!(MarketResolution::<Test>::get(0).is_none());
        assert!(MarketResolutionEvidence::<Test>::get(0).is_none());

        assert_noop!(
            Polkamarkt::resolve_market_with_evidence(
                RuntimeOrigin::root(),
                0,
                BinaryOutcome::Yes,
                EvidenceInput {
                    uri: vec![0xff, 0xfe],
                    hash: Some([9; 32]),
                },
            ),
            Error::<Test>::InvalidEvidence
        );
        assert!(MarketResolution::<Test>::get(0).is_none());
        assert!(MarketResolutionEvidence::<Test>::get(0).is_none());

        assert_ok!(Polkamarkt::resolve_market_with_evidence(
            RuntimeOrigin::root(),
            0,
            BinaryOutcome::No,
            EvidenceInput {
                uri: b"ipfs://resolution-v2".to_vec(),
                hash: None,
            },
        ));
        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::No));
    });
}

#[test]
fn emergency_cancel_market_requires_governance_valid_evidence_and_nonfinalized_market() {
    new_test_ext().execute_with(|| {
        setup_market(1_000, 10);

        assert_noop!(
            Polkamarkt::emergency_cancel_market(
                RuntimeOrigin::signed(ALICE),
                0,
                EvidenceInput {
                    uri: b"ipfs://cancel".to_vec(),
                    hash: Some([4; 32]),
                },
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Polkamarkt::emergency_cancel_market(
                RuntimeOrigin::root(),
                0,
                EvidenceInput {
                    uri: Vec::new(),
                    hash: Some([4; 32]),
                },
            ),
            Error::<Test>::InvalidEvidence
        );
        assert_eq!(
            Markets::<Test>::get(0).expect("market").status,
            MarketStatus::Open
        );
        assert!(MarketCancellationEvidence::<Test>::get(0).is_none());

        assert_ok!(Polkamarkt::emergency_cancel_market(
            RuntimeOrigin::root(),
            0,
            EvidenceInput {
                uri: b"ipfs://cancel".to_vec(),
                hash: Some([4; 32]),
            },
        ));
        assert_eq!(
            Markets::<Test>::get(0).expect("market").status,
            MarketStatus::Cancelled
        );
        let evidence = MarketCancellationEvidence::<Test>::get(0).expect("evidence");
        assert_eq!(evidence.uri.to_vec(), b"ipfs://cancel");
        assert_eq!(evidence.hash, Some([4; 32]));

        assert_noop!(
            Polkamarkt::emergency_cancel_market(
                RuntimeOrigin::root(),
                0,
                EvidenceInput {
                    uri: b"ipfs://cancel-again".to_vec(),
                    hash: Some([5; 32]),
                },
            ),
            Error::<Test>::MarketAlreadyFinalized
        );
        assert_eq!(
            MarketCancellationEvidence::<Test>::get(0)
                .expect("existing evidence")
                .hash,
            Some([4; 32])
        );
    });
}

#[test]
fn v4_migration_initializes_creator_lp_for_legacy_markets() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(3).put::<Polkamarkt>();
        insert_v4_legacy_market(0, 0, 1_000);

        let _ = crate::migrations::v4::Migrate::<Test>::on_runtime_upgrade();

        assert_eq!(
            LiquidityPositions::<Test>::get(0, ALICE)
                .expect("creator lp")
                .shares,
            1_000
        );
        assert_eq!(LiquidityPositionTotals::<Test>::get(0).total_shares, 1_000);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(4));
    });
}

#[test]
fn v4_migration_does_not_run_before_v3_completes() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(2).put::<Polkamarkt>();
        insert_v4_legacy_market(0, 0, 1_000);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v4::Migrate::<Test>::on_runtime_upgrade();
        }));

        assert!(LiquidityPositions::<Test>::get(0, ALICE).is_none());
        assert_eq!(LiquidityPositionTotals::<Test>::get(0).total_shares, 0);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(2));
        assert!(result.is_err());
    });
}

#[test]
fn v4_migration_does_not_overwrite_zero_seed_or_existing_lp_state() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(3).put::<Polkamarkt>();
        insert_v4_legacy_market(0, 0, 0);
        insert_v4_legacy_market(1, 1, 1_000);
        LiquidityPositions::<Test>::insert(
            1,
            BOB,
            LiquidityPosition {
                shares: 333,
                collateral_contributed: 333,
            },
        );
        LiquidityPositionTotals::<Test>::insert(
            1,
            LiquidityTotals {
                total_shares: 333,
                total_collateral_contributed: 333,
            },
        );

        let _ = crate::migrations::v4::Migrate::<Test>::on_runtime_upgrade();

        assert!(LiquidityPositions::<Test>::get(0, ALICE).is_none());
        assert_eq!(LiquidityPositionTotals::<Test>::get(0).total_shares, 0);
        assert!(LiquidityPositions::<Test>::get(1, ALICE).is_none());
        assert_eq!(
            LiquidityPositions::<Test>::get(1, BOB)
                .expect("existing LP remains")
                .shares,
            333
        );
        assert_eq!(LiquidityPositionTotals::<Test>::get(1).total_shares, 333);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(4));
    });
}

#[test]
fn v4_migration_accepts_exact_market_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(3).put::<Polkamarkt>();
        for market_id in 0..crate::migrations::MAX_LEGACY_MARKETS {
            insert_v4_legacy_market(market_id, market_id, 0);
        }

        let _ = crate::migrations::v4::Migrate::<Test>::on_runtime_upgrade();

        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(4));
        assert!(LiquidityPositions::<Test>::get(0, ALICE).is_none());
        assert_eq!(LiquidityPositionTotals::<Test>::get(0).total_shares, 0);
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration Markets exceeds limit 1024")]
fn v4_migration_panics_when_markets_exceed_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(3).put::<Polkamarkt>();
        for market_id in 0..=crate::migrations::MAX_LEGACY_MARKETS {
            insert_v4_legacy_market(market_id, 0, 0);
        }

        let _ = crate::migrations::v4::Migrate::<Test>::on_runtime_upgrade();
    });
}

#[test]
fn v5_migration_marks_legacy_markets_and_preserves_fields() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(4).put::<Polkamarkt>();
        insert_v5_legacy_market(7, 42, BOB, 99, USDC_ASSET, 123_456, MarketStatus::Locked);

        let _ = crate::migrations::v5::Migrate::<Test>::on_runtime_upgrade();

        let market = Markets::<Test>::get(7).expect("migrated market");
        assert_eq!(market.creator, BOB);
        assert_eq!(market.condition_id, 42);
        assert_eq!(market.close_block, 99);
        assert_eq!(market.collateral_asset, USDC_ASSET);
        assert_eq!(market.seed_liquidity, 123_456);
        assert_eq!(market.mechanism, MarketMechanism::LegacyAmm);
        assert_eq!(market.status, MarketStatus::Locked);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(5));
    });
}

#[test]
fn v5_migration_noops_at_v5() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(5).put::<Polkamarkt>();
        Markets::<Test>::insert(
            1,
            Market {
                creator: ALICE,
                condition_id: 1,
                close_block: 20,
                collateral_asset: CANONICAL_ASSET,
                seed_liquidity: 0,
                mechanism: MarketMechanism::OrderBook,
                status: MarketStatus::Open,
            },
        );

        let _ = crate::migrations::v5::Migrate::<Test>::on_runtime_upgrade();

        let market = Markets::<Test>::get(1).expect("existing market");
        assert_eq!(market.mechanism, MarketMechanism::OrderBook);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(5));
    });
}

#[test]
fn v5_migration_does_not_run_before_v4_completes() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(3).put::<Polkamarkt>();
        insert_v5_legacy_market(0, 0, ALICE, 10, CANONICAL_ASSET, 1_000, MarketStatus::Open);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v5::Migrate::<Test>::on_runtime_upgrade();
        }));

        assert!(result.is_err());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(3));
        assert!(crate::migrations::v5::Markets::<Test>::get(0).is_some());
    });
}

#[test]
fn v5_migration_accepts_exact_market_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(4).put::<Polkamarkt>();
        for market_id in 0..crate::migrations::MAX_LEGACY_MARKETS {
            insert_v5_legacy_market(
                market_id,
                market_id,
                ALICE,
                10,
                CANONICAL_ASSET,
                0,
                MarketStatus::Open,
            );
        }

        let _ = crate::migrations::v5::Migrate::<Test>::on_runtime_upgrade();

        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(5));
        assert_eq!(
            Markets::<Test>::iter().count(),
            crate::migrations::MAX_LEGACY_MARKETS as usize
        );
        assert!(Markets::<Test>::iter()
            .all(|(_, market)| { market.mechanism == MarketMechanism::LegacyAmm }));
    });
}

#[test]
#[should_panic(expected = "Polkamarkt migration Markets exceeds limit 1024")]
fn v5_migration_panics_when_markets_exceed_cap() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(4).put::<Polkamarkt>();
        for market_id in 0..=crate::migrations::MAX_LEGACY_MARKETS {
            insert_v5_legacy_market(
                market_id,
                0,
                ALICE,
                10,
                CANONICAL_ASSET,
                0,
                MarketStatus::Open,
            );
        }

        let _ = crate::migrations::v5::Migrate::<Test>::on_runtime_upgrade();
    });
}

#[test]
fn v6_migration_preserves_dpm_markets_and_sets_version() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(5).put::<Polkamarkt>();
        setup_dpm_market(10);
        assert_ok!(Polkamarkt::buy(
            RuntimeOrigin::signed(BOB),
            0,
            BinaryOutcome::Yes,
            10_000,
            0,
        ));
        let market_before = Markets::<Test>::get(0).expect("market");
        let position_before = MarketPositions::<Test>::get(0, BOB).expect("position");
        let dpm_collateral_before = MarketDpmCollateral::<Test>::get(0);

        let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();

        assert_eq!(Markets::<Test>::get(0), Some(market_before));
        assert_eq!(MarketPositions::<Test>::get(0, BOB), Some(position_before));
        assert_eq!(MarketDpmCollateral::<Test>::get(0), dpm_collateral_before);
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(6));
    });
}

#[test]
fn v6_migration_cancels_orderbook_and_creates_lazy_refunds() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(5).put::<Polkamarkt>();
        run_to_block(1);
        make_orderbook_market(0, 0, ALICE, 10);
        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 1_500);
        MarketOrderBookCollateral::<Test>::insert(0, 1_000);
        MarketPositions::<Test>::insert(
            0,
            BOB,
            crate::MarketPosition {
                yes_shares: 100,
                no_shares: 300,
                net_collateral_paid: 0,
            },
        );
        MarketPositionTotals::<Test>::insert(
            0,
            crate::MarketTotals {
                total_yes_shares: 100,
                total_no_shares: 300,
                total_net_collateral_paid: 0,
            },
        );
        Orders::<Test>::insert(
            1,
            Order {
                owner: BOB,
                market_id: 0,
                outcome: BinaryOutcome::No,
                side: OrderSide::Buy,
                price_cents: 60,
                remaining_shares: 500,
                reserved_collateral: 300,
            },
        );
        Orders::<Test>::insert(
            2,
            Order {
                owner: ALICE,
                market_id: 0,
                outcome: BinaryOutcome::Yes,
                side: OrderSide::Sell,
                price_cents: 40,
                remaining_shares: 200,
                reserved_collateral: 0,
            },
        );
        PendingXorBuybackCollateral::<Test>::put(5);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();

        let market = Markets::<Test>::get(0).expect("market");
        assert_eq!(market.mechanism, MarketMechanism::MigratedLegacy);
        assert_eq!(market.status, MarketStatus::Cancelled);
        assert_eq!(MarketResolution::<Test>::get(0), None);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, BOB), 500);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, ALICE), 100);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 705);
        assert!(Orders::<Test>::iter().next().is_none());
        assert!(MarketPools::<Test>::get(0).is_none());
        assert!(LiquidityPositions::<Test>::iter().next().is_none());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(6));

        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before + 500);
        assert_noop!(
            Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0),
            Error::<Test>::NothingToClaim
        );
    });
}

#[test]
fn v6_migration_groups_orders_across_multiple_orderbook_markets() {
    new_test_ext().execute_with(|| {
        let charlie = 3;
        StorageVersion::new(5).put::<Polkamarkt>();
        run_to_block(1);
        make_orderbook_market(0, 0, ALICE, 10);
        make_orderbook_market(1, 1, BOB, 10);
        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 3_000);
        MarketOrderBookCollateral::<Test>::insert(0, 1_000);
        MarketOrderBookCollateral::<Test>::insert(1, 800);
        MarketPositions::<Test>::insert(
            0,
            BOB,
            crate::MarketPosition {
                yes_shares: 100,
                no_shares: 300,
                net_collateral_paid: 0,
            },
        );
        MarketPositionTotals::<Test>::insert(
            0,
            crate::MarketTotals {
                total_yes_shares: 100,
                total_no_shares: 300,
                total_net_collateral_paid: 0,
            },
        );
        MarketPositions::<Test>::insert(
            1,
            charlie,
            crate::MarketPosition {
                yes_shares: 50,
                no_shares: 50,
                net_collateral_paid: 0,
            },
        );
        MarketPositionTotals::<Test>::insert(
            1,
            crate::MarketTotals {
                total_yes_shares: 50,
                total_no_shares: 50,
                total_net_collateral_paid: 0,
            },
        );
        Orders::<Test>::insert(
            1,
            Order {
                owner: BOB,
                market_id: 0,
                outcome: BinaryOutcome::No,
                side: OrderSide::Buy,
                price_cents: 60,
                remaining_shares: 500,
                reserved_collateral: 300,
            },
        );
        Orders::<Test>::insert(
            2,
            Order {
                owner: ALICE,
                market_id: 0,
                outcome: BinaryOutcome::Yes,
                side: OrderSide::Sell,
                price_cents: 40,
                remaining_shares: 200,
                reserved_collateral: 0,
            },
        );
        Orders::<Test>::insert(
            3,
            Order {
                owner: ALICE,
                market_id: 1,
                outcome: BinaryOutcome::Yes,
                side: OrderSide::Buy,
                price_cents: 55,
                remaining_shares: 250,
                reserved_collateral: 120,
            },
        );
        Orders::<Test>::insert(
            4,
            Order {
                owner: BOB,
                market_id: 1,
                outcome: BinaryOutcome::No,
                side: OrderSide::Sell,
                price_cents: 45,
                remaining_shares: 300,
                reserved_collateral: 0,
            },
        );
        PendingXorBuybackCollateral::<Test>::put(5);

        let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();

        for market_id in 0..=1 {
            let market = Markets::<Test>::get(market_id).expect("market");
            assert_eq!(market.mechanism, MarketMechanism::MigratedLegacy);
            assert_eq!(market.status, MarketStatus::Cancelled);
            assert_eq!(MarketResolution::<Test>::get(market_id), None);
            assert!(MarketPositions::<Test>::iter_prefix(market_id)
                .next()
                .is_none());
        }
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, BOB), 500);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, ALICE), 100);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(1, ALICE), 120);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(1, BOB), 150);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(1, charlie), 50);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), 1_305);
        assert!(Orders::<Test>::iter().next().is_none());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(6));
    });
}

#[test]
fn v6_migration_rejects_order_cap_before_draining_orders() {
    new_test_ext().execute_with(|| {
        const MAX_LEGACY_ORDERS: u64 = 16_384;
        StorageVersion::new(5).put::<Polkamarkt>();
        for order_id in 0..=MAX_LEGACY_ORDERS {
            Orders::<Test>::insert(order_id, legacy_buy_order(BOB, 0, 1));
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();
        }));

        assert!(result.is_err());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(5));
        assert_eq!(
            Orders::<Test>::iter().count(),
            (MAX_LEGACY_ORDERS + 1) as usize
        );
    });
}

#[test]
fn v6_migration_rolls_back_drained_orders_on_late_failure() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(5).put::<Polkamarkt>();
        run_to_block(1);
        make_orderbook_market(0, 0, ALICE, 10);
        let market_before = Markets::<Test>::get(0).expect("market");
        MarketOrderBookCollateral::<Test>::insert(0, 1);
        Orders::<Test>::insert(1, legacy_buy_order(BOB, 0, 1));
        PendingXorBuybackCollateral::<Test>::put(Balance::MAX);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();
        }));

        assert!(result.is_err());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(5));
        assert_eq!(Markets::<Test>::get(0), Some(market_before));
        assert!(Orders::<Test>::get(1).is_some());
        assert_eq!(MarketOrderBookCollateral::<Test>::get(0), 1);
        assert_eq!(PendingXorBuybackCollateral::<Test>::get(), Balance::MAX);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, BOB), 0);
    });
}

#[test]
fn v6_migration_resolves_legacy_amm_as_no_and_pays_lp_residual() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(5).put::<Polkamarkt>();
        run_to_block(1);
        make_legacy_market(0, 0, ALICE, 1_000, 10);
        set_balance(Polkamarkt::account_id(), CANONICAL_ASSET, 1_400);
        MarketPools::<Test>::insert(
            0,
            crate::MarketPool {
                collateral: 1_400,
                yes: 1_000,
                no: 1_000,
            },
        );
        MarketPositions::<Test>::insert(
            0,
            BOB,
            crate::MarketPosition {
                yes_shares: 900,
                no_shares: 250,
                net_collateral_paid: 900,
            },
        );
        MarketPositionTotals::<Test>::insert(
            0,
            crate::MarketTotals {
                total_yes_shares: 900,
                total_no_shares: 250,
                total_net_collateral_paid: 900,
            },
        );
        let alice_before = balance_of(ALICE, CANONICAL_ASSET);
        let bob_before = balance_of(BOB, CANONICAL_ASSET);

        let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();

        let market = Markets::<Test>::get(0).expect("market");
        assert_eq!(market.mechanism, MarketMechanism::MigratedLegacy);
        assert_eq!(market.status, MarketStatus::Resolved);
        assert_eq!(MarketResolution::<Test>::get(0), Some(BinaryOutcome::No));
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, BOB), 250);
        assert_eq!(MigratedLegacyPayouts::<Test>::get(0, ALICE), 1_150);
        assert!(MarketPools::<Test>::get(0).is_none());
        assert!(LiquidityPositions::<Test>::get(0, ALICE).is_none());
        assert_eq!(LiquidityPositionTotals::<Test>::get(0).total_shares, 0);

        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(ALICE), 0));
        assert_ok!(Polkamarkt::claim_market(RuntimeOrigin::signed(BOB), 0));
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), alice_before + 1_150);
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), bob_before + 250);
    });
}

#[test]
fn v6_migration_rejects_pre_v5_and_noops_at_v6() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(4).put::<Polkamarkt>();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();
        }));
        assert!(result.is_err());
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(4));

        StorageVersion::new(6).put::<Polkamarkt>();
        let _ = crate::migrations::v6::Migrate::<Test>::on_runtime_upgrade();
        assert_eq!(StorageVersion::get::<Polkamarkt>(), StorageVersion::new(6));
    });
}

#[cfg(feature = "try-runtime")]
#[test]
fn v5_migration_try_runtime_hooks_validate_v4_upgrade() {
    new_test_ext().execute_with(|| {
        StorageVersion::new(4).put::<Polkamarkt>();
        insert_v5_legacy_market(0, 0, ALICE, 10, CANONICAL_ASSET, 1_000, MarketStatus::Open);
        insert_v5_legacy_market(1, 1, BOB, 20, USDC_ASSET, 2_000, MarketStatus::Locked);

        let state = crate::migrations::v5::Migrate::<Test>::pre_upgrade().unwrap();
        crate::migrations::v5::Migrate::<Test>::on_runtime_upgrade();
        crate::migrations::v5::Migrate::<Test>::post_upgrade(state).unwrap();
    });
}
