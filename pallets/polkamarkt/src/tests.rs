use crate::{
    BlockedJurisdictions, BridgeEntitlements, CommitmentHash, ConditionInput,
    CreatorRewardActivated, CreatorRewards, Credentials, CredentialsEnforced, Error, Event,
    ForkTaxOwed, MaintenancePoolBalance, MaintenancePoolTotal, MarketCollateral, MarketId,
    OpengovProposalInput, Pallet, RelayNetwork,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok};
use frame_system::Pallet as System;
use proptest::prelude::*;
use sp_io::hashing::blake2_256;
use sp_runtime::Perbill;

use super::mock::*;
use super::mock::{
    last_plaza_condition, reset_plaza_notifications, BlockNumber, CredentialTtlConst,
    LiquiditySafetyBpsConst, RuntimeEvent, WalletCooldownConst, MAINTENANCE_ACCOUNT, USDC_ASSET,
};

type Polkamarkt = Pallet<Test>;

fn default_condition() -> ConditionInput<BlockNumber> {
    ConditionInput {
        question: b"Will SORA win?".to_vec(),
        oracle: b"Chainlink".to_vec(),
        resolution_source: b"https://example.com".to_vec(),
        submission_deadline: 100,
    }
}

fn compute_commitment(
    who: AccountId,
    market_id: MarketId,
    payload: &[u8],
    salt: &[u8],
) -> CommitmentHash {
    let mut data = who.encode();
    data.extend_from_slice(&market_id.encode());
    data.extend_from_slice(payload);
    data.extend_from_slice(salt);
    blake2_256(&data)
}

proptest! {
    #[test]
    fn compute_commitment_hash_fuzz(
        account in any::<AccountId>(),
        market_id in any::<MarketId>(),
        payload in proptest::collection::vec(any::<u8>(), 0..96),
        salt in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let hash = compute_commitment(account, market_id, &payload, &salt);
        let mut data = account.encode();
        data.extend_from_slice(&market_id.encode());
        data.extend_from_slice(&payload);
        data.extend_from_slice(&salt);
        let manual = blake2_256(&data);
        prop_assert_eq!(hash, manual);
    }

    #[test]
    fn credential_guard_fuzz(
        enforce in proptest::bool::ANY,
        has_credential in proptest::bool::ANY,
        expiry_offset in 0u32..2_000,
        block in 0u32..2_000,
        expire_in_past in proptest::bool::ANY,
        block_jurisdiction in proptest::bool::ANY,
        force_zero_code in proptest::bool::ANY,
        raw_jurisdiction in any::<[u8; 3]>(),
    ) {
        let (actual_ok, expected_ok) = new_test_ext().execute_with(|| {
            CredentialsEnforced::<Test>::put(enforce);
            let now: BlockNumber = block.into();
            run_to_block(now);

            let mut jurisdiction = raw_jurisdiction;
            if force_zero_code {
                jurisdiction = [0; 3];
            }
            if block_jurisdiction {
                BlockedJurisdictions::<Test>::insert(jurisdiction, ());
            }

            Credentials::<Test>::remove(ALICE);
            if has_credential {
                let expiry_offset_bn: BlockNumber = expiry_offset.into();
                let expires_at = if expire_in_past {
                    now.saturating_sub(expiry_offset_bn)
                } else {
                    now.saturating_add(expiry_offset_bn)
                };
                Credentials::<Test>::insert(ALICE, (expires_at, [1u8; 32], jurisdiction, false));
            }

            let blocked = block_jurisdiction || jurisdiction == [0; 3];
            let stored = Credentials::<Test>::get(ALICE);
            let expected_ok = if !enforce {
                true
            } else if let Some((expiry, _, _, _)) = stored {
                now <= expiry && !blocked
            } else {
                false
            };

            let result = Pallet::<Test>::ensure_has_credential(&ALICE);
            (result.is_ok(), expected_ok)
        });
        prop_assert_eq!(actual_ok, expected_ok);
    }

    #[test]
    fn maintenance_pool_debit_fuzz(
        total in 0u128..10_000_000u128,
        amount in 0u128..10_000_000u128,
    ) {
        let (actual_ok, balance_after, total_after) = new_test_ext().execute_with(|| {
            MaintenancePoolBalance::<Test>::put(total);
            MaintenancePoolTotal::<Test>::put(total);

            let result = Pallet::<Test>::debit_pool(amount);
            (
                result.is_ok(),
                MaintenancePoolBalance::<Test>::get(),
                MaintenancePoolTotal::<Test>::get(),
            )
        });
        let floor_bps = LiquiditySafetyBpsConst::get().min(10_000);
        let expected_floor = Perbill::from_rational(floor_bps, 10_000u32) * total;
        let remaining = total.saturating_sub(amount);
        let enough_balance = total >= amount;
        let respects_floor = remaining >= expected_floor;
        let should_succeed = amount == 0 || (enough_balance && respects_floor);

        prop_assert_eq!(actual_ok, should_succeed);

        if should_succeed {
            if amount == 0 {
                prop_assert_eq!(balance_after, total);
                prop_assert_eq!(total_after, total);
            } else {
                prop_assert_eq!(balance_after, remaining);
                prop_assert_eq!(total_after, total.saturating_sub(amount));
            }
        } else {
            prop_assert_eq!(balance_after, total);
            prop_assert_eq!(total_after, total);
        }
    }
}

const TEST_BOND: Balance = 2_000;
const DEFAULT_JURISDICTION: [u8; 3] = *b"USA";

fn provide_credential(account: AccountId) {
    let expiry = System::<Test>::block_number() + CredentialTtlConst::get();
    assert_ok!(Polkamarkt::submit_credential(
        RuntimeOrigin::signed(account),
        [account as u8; 32],
        expiry,
        DEFAULT_JURISDICTION,
        false,
    ));
}

fn bond_alice() {
    provide_credential(ALICE);
    assert_ok!(Polkamarkt::bond_governance(
        RuntimeOrigin::signed(ALICE),
        TEST_BOND
    ));
}

fn default_opengov_input() -> OpengovProposalInput {
    OpengovProposalInput {
        network: RelayNetwork::Polkadot,
        parachain_id: 2_000,
        track_id: 33,
        referendum_index: 101,
        plaza_tag: b"polkadot-plaza".to_vec(),
    }
}

#[test]
fn create_condition_works() {
    new_test_ext().execute_with(|| {
        bond_alice();
        let meta = default_condition();
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            meta.clone()
        ));
        let stored = Polkamarkt::conditions(0).expect("condition stored");
        assert_eq!(stored.question, meta.question);
    });
}

#[test]
fn create_opengov_condition_records_metadata() {
    new_test_ext().execute_with(|| {
        bond_alice();
        reset_plaza_notifications();
        let meta = default_condition();
        let proposal = default_opengov_input();
        assert_ok!(Polkamarkt::create_opengov_condition(
            RuntimeOrigin::signed(ALICE),
            meta,
            proposal.clone()
        ));
        let stored = Polkamarkt::opengov_proposals(0).expect("metadata stored");
        assert_eq!(stored.parachain_id, proposal.parachain_id);
        assert_eq!(stored.track_id, proposal.track_id);
        assert_eq!(stored.referendum_index, proposal.referendum_index);
        assert_eq!(stored.plaza_tag.to_vec(), proposal.plaza_tag);
        assert_eq!(last_plaza_condition(), Some(0));
    });
}

#[test]
fn create_opengov_condition_rejects_invalid_inputs() {
    new_test_ext().execute_with(|| {
        bond_alice();
        let mut proposal = default_opengov_input();
        proposal.referendum_index = 0;
        assert_noop!(
            Polkamarkt::create_opengov_condition(
                RuntimeOrigin::signed(ALICE),
                default_condition(),
                proposal
            ),
            Error::<Test>::InvalidOpengovProposal
        );
    });
}

#[test]
fn governance_can_clear_opengov_metadata() {
    new_test_ext().execute_with(|| {
        bond_alice();
        assert_ok!(Polkamarkt::create_opengov_condition(
            RuntimeOrigin::signed(ALICE),
            default_condition(),
            default_opengov_input()
        ));
        assert!(Polkamarkt::opengov_proposals(0).is_some());
        assert_ok!(Polkamarkt::clear_opengov_condition(
            RuntimeOrigin::root(),
            0
        ));
        assert!(Polkamarkt::opengov_proposals(0).is_none());
    });
}

#[test]
fn create_condition_rejects_short_questions() {
    new_test_ext().execute_with(|| {
        bond_alice();
        let mut meta = default_condition();
        meta.question = b"no".to_vec();
        assert_noop!(
            Polkamarkt::create_condition(RuntimeOrigin::signed(ALICE), meta),
            Error::<Test>::QuestionTooShort
        );
    });
}

#[test]
fn create_market_reserves_collateral_and_fees() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        let meta = default_condition();
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            meta
        ));

        let seed = 1_000u128;
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            200,
            seed,
            None
        ));

        // Market storage updated
        let market = Polkamarkt::markets(0).expect("market stored");
        assert_eq!(market.seed_liquidity, seed);
        assert_eq!(MarketCollateral::<Test>::get(0), seed);

        // Collateral transferred to pallet account
        let pallet_account = Pallet::<Test>::account_id();
        assert_eq!(balance_of(pallet_account, CANONICAL_ASSET), seed);

        // Fee collector receives min fee minus maintenance allocation (8)
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 8);
        // Maintenance pool collects an extra 2 on top of the bonded amount
        assert_eq!(
            balance_of(MAINTENANCE_ACCOUNT, CANONICAL_ASSET),
            TEST_BOND + 2
        );
        // Fork tax pool records 0.1% of usage (1)
        assert_eq!(ForkTaxOwed::<Test>::get(), 1);

        // Alice balance reduced by seed + fee
        let expected_remaining = 1_000_000_000_000u128 - seed - 10 - TEST_BOND;
        assert_eq!(balance_of(ALICE, CANONICAL_ASSET), expected_remaining);

        // Event emitted
        let events = frame_system::Pallet::<Test>::events();
        assert!(events.iter().any(|record| record.event
            == Event::CollateralSeeded {
                market_id: 0,
                amount: seed,
            }
            .into()));
    });
}

#[test]
fn create_market_fails_without_condition() {
    new_test_ext().execute_with(|| {
        bond_alice();
        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 99, 10, 0, None),
            Error::<Test>::ConditionNotFound
        );
    });
}

#[test]
fn create_market_rejects_short_duration() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        let meta = default_condition();
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            meta
        ));
        assert_noop!(
            Polkamarkt::create_market(RuntimeOrigin::signed(ALICE), 0, 1, 0, None),
            Error::<Test>::MarketDurationTooShort
        );
    });
}

#[test]
fn commit_and_reveal_flow_enforces_delays() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        let meta = default_condition();
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            meta
        ));
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            20,
            100,
            None
        ));

        run_to_block(2);
        let payload = b"BUY:100@55".to_vec();
        let salt = b"secret".to_vec();
        let hash = compute_commitment(ALICE, 0, &payload, &salt);

        assert_ok!(Polkamarkt::commit_order(
            RuntimeOrigin::signed(ALICE),
            0,
            hash
        ));

        // Too soon to reveal
        run_to_block(3);
        let payload_fail = payload.clone();
        let salt_fail = salt.clone();
        assert_noop!(
            Polkamarkt::reveal_order(RuntimeOrigin::signed(ALICE), 0, payload_fail, salt_fail, 50),
            Error::<Test>::RevealTooSoon
        );

        // Reveal after delay
        run_to_block(5);
        let payload_success = payload;
        let salt_success = salt;
        assert_ok!(Polkamarkt::reveal_order(
            RuntimeOrigin::signed(ALICE),
            0,
            payload_success,
            salt_success,
            50
        ));

        // Commitment removed
        assert!(crate::Commitments::<Test>::get(0, hash).is_none());

        // Orderbook event emitted
        let order_event = frame_system::Pallet::<Test>::events()
            .iter()
            .find_map(|record| match &record.event {
                RuntimeEvent::Polkamarkt(Event::OrderbookOrderPlaced {
                    market_id,
                    trader,
                    order_value,
                }) => Some((*market_id, *trader, *order_value)),
                _ => None,
            })
            .expect("order placement event");
        assert_eq!(order_event.0, 0);
        assert_eq!(order_event.1, ALICE);
        assert_eq!(order_event.2, 50);
    });
}

#[test]
fn commit_expires_if_not_revealed() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        let meta = default_condition();
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            meta
        ));
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            30,
            0,
            None
        ));

        run_to_block(2);
        let payload = b"SELL:50@70".to_vec();
        let salt = b"salt".to_vec();
        let hash = compute_commitment(ALICE, 0, &payload, &salt);
        assert_ok!(Polkamarkt::commit_order(
            RuntimeOrigin::signed(ALICE),
            0,
            hash
        ));

        // Jump beyond expiry window
        run_to_block(20);
        let payload_expired = payload;
        let salt_expired = salt;
        assert_noop!(
            Polkamarkt::reveal_order(
                RuntimeOrigin::signed(ALICE),
                0,
                payload_expired,
                salt_expired,
                50
            ),
            Error::<Test>::CommitmentExpired
        );
    });
}

#[test]
fn creator_reward_activates_when_threshold_reached() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        bond_alice();
        let meta = default_condition();
        assert_ok!(Polkamarkt::create_condition(
            RuntimeOrigin::signed(ALICE),
            meta
        ));
        assert_ok!(Polkamarkt::create_market(
            RuntimeOrigin::signed(ALICE),
            0,
            50,
            1_000,
            None
        ));

        run_to_block(2);
        let payload = b"BUY:10000@1".to_vec();
        let salt = b"reward".to_vec();
        let hash = compute_commitment(ALICE, 0, &payload, &salt);
        assert_ok!(Polkamarkt::commit_order(
            RuntimeOrigin::signed(ALICE),
            0,
            hash
        ));
        run_to_block(10);
        let payload_reveal = payload;
        let salt_reveal = salt;
        assert_ok!(Polkamarkt::reveal_order(
            RuntimeOrigin::signed(ALICE),
            0,
            payload_reveal,
            salt_reveal,
            20_000
        ));

        assert!(CreatorRewardActivated::<Test>::get(0));
        assert!(CreatorRewards::<Test>::get(0) > 0);
    });
}
#[test]
fn bridge_deposit_respects_daily_cap() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::bridge_deposit(
            RuntimeOrigin::signed(ALICE),
            USDC_ASSET,
            4_000
        ));
        assert_noop!(
            Polkamarkt::bridge_deposit(RuntimeOrigin::signed(ALICE), USDC_ASSET, 2_000),
            Error::<Test>::BridgeDailyLimitReached
        );
    });
}

#[test]
fn bridge_wallet_cooldown_enforced() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_noop!(
            Polkamarkt::set_bridge_wallet(RuntimeOrigin::signed(ALICE), FEE_COLLECTOR),
            Error::<Test>::BridgeWalletLocked
        );
        let unlock = WalletCooldownConst::get() + 5;
        run_to_block(unlock);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            FEE_COLLECTOR
        ));
    });
}

#[test]
fn bridge_withdraw_applies_tax() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        provide_credential(ALICE);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_ok!(Polkamarkt::bridge_deposit(
            RuntimeOrigin::signed(ALICE),
            USDC_ASSET,
            1_000
        ));
        assert_eq!(BridgeEntitlements::<Test>::get(ALICE), 1_000);
        assert_ok!(Polkamarkt::bridge_withdraw(
            RuntimeOrigin::signed(ALICE),
            1_000
        ));
        assert_eq!(BridgeEntitlements::<Test>::get(ALICE), 0);
        assert_eq!(ForkTaxOwed::<Test>::get(), 1);
    });
}

#[test]
fn bridge_withdraw_requires_credential() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_noop!(
            Polkamarkt::bridge_withdraw(RuntimeOrigin::signed(ALICE), 1_000),
            Error::<Test>::CredentialMissing
        );
    });
}

#[test]
fn bridge_withdraw_requires_entitlement() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        provide_credential(ALICE);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_noop!(
            Polkamarkt::bridge_withdraw(RuntimeOrigin::signed(ALICE), 1_000),
            Error::<Test>::BridgeInsufficientEntitlement
        );
    });
}

#[test]
fn bridge_withdraw_debits_entitlement() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        provide_credential(ALICE);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_ok!(Polkamarkt::bridge_deposit(
            RuntimeOrigin::signed(ALICE),
            USDC_ASSET,
            1_500
        ));
        assert_ok!(Polkamarkt::bridge_withdraw(
            RuntimeOrigin::signed(ALICE),
            1_000
        ));
        assert_eq!(BridgeEntitlements::<Test>::get(ALICE), 500);
        assert_noop!(
            Polkamarkt::bridge_withdraw(RuntimeOrigin::signed(ALICE), 600),
            Error::<Test>::BridgeInsufficientEntitlement
        );
    });
}

#[test]
fn governance_bond_and_unbond_flow() {
    new_test_ext().execute_with(|| {
        bond_alice();
        assert_eq!(Polkamarkt::governance_bonds(ALICE), 2_000);
        let pool_before = Polkamarkt::maintenance_pool_balance();
        let unbond_amount: Balance = 300;

        assert_ok!(Polkamarkt::unbond_governance(
            RuntimeOrigin::signed(ALICE),
            unbond_amount
        ));
        assert_eq!(
            Polkamarkt::governance_bonds(ALICE),
            TEST_BOND - unbond_amount
        );
        let pool_after = Polkamarkt::maintenance_pool_balance();
        assert!(pool_after < pool_before);
        assert_eq!(
            balance_of(ALICE, CANONICAL_ASSET),
            1_000_000_000_000 - (TEST_BOND - unbond_amount)
        );
    });
}

#[test]
fn governance_unbond_respects_pool_floor() {
    new_test_ext().execute_with(|| {
        bond_alice();
        assert_noop!(
            Polkamarkt::unbond_governance(RuntimeOrigin::signed(ALICE), 1_000),
            Error::<Test>::PoolBelowSafetyThreshold
        );
    });
}

#[test]
fn flagged_accounts_block_usage() {
    new_test_ext().execute_with(|| {
        assert_ok!(Polkamarkt::flag_account(RuntimeOrigin::root(), ALICE));
        let meta = default_condition();
        assert_noop!(
            Polkamarkt::create_condition(RuntimeOrigin::signed(ALICE), meta),
            Error::<Test>::AccountFlagged
        );
    });
}

#[test]
fn draining_flagged_wallet_moves_funds() {
    new_test_ext().execute_with(|| {
        set_balance(BOB, CANONICAL_ASSET, 5_000);
        assert_ok!(Polkamarkt::flag_account(RuntimeOrigin::root(), BOB));
        assert_ok!(Polkamarkt::drain_flagged_account(
            RuntimeOrigin::root(),
            BOB,
            2_000
        ));
        assert_eq!(balance_of(BOB, CANONICAL_ASSET), 3_000);
        assert_eq!(balance_of(MAINTENANCE_ACCOUNT, CANONICAL_ASSET), 2_000);
    });
}

#[test]
fn maintenance_pool_withdraw_enforces_safety_floor() {
    new_test_ext().execute_with(|| {
        bond_alice();
        // Remaining balance must stay above 85% of the total (floor = 1_700 when total = 2_000).
        assert_noop!(
            Polkamarkt::withdraw_maintenance_pool(RuntimeOrigin::root(), FEE_COLLECTOR, 500),
            Error::<Test>::PoolBelowSafetyThreshold
        );

        assert_ok!(Polkamarkt::withdraw_maintenance_pool(
            RuntimeOrigin::root(),
            FEE_COLLECTOR,
            200
        ));
        assert_eq!(balance_of(FEE_COLLECTOR, CANONICAL_ASSET), 200);
        assert_eq!(Polkamarkt::maintenance_pool_balance(), TEST_BOND - 200);
        assert_eq!(Polkamarkt::maintenance_pool_total(), TEST_BOND - 200);
    });
}

#[test]
fn blocked_jurisdiction_prevents_credential_submission() {
    new_test_ext().execute_with(|| {
        assert_ok!(Polkamarkt::set_jurisdiction_block(
            RuntimeOrigin::root(),
            DEFAULT_JURISDICTION,
            true
        ));
        let expiry = System::<Test>::block_number() + CredentialTtlConst::get();
        assert_noop!(
            Polkamarkt::submit_credential(
                RuntimeOrigin::signed(ALICE),
                [0u8; 32],
                expiry,
                DEFAULT_JURISDICTION,
                false
            ),
            Error::<Test>::JurisdictionBlocked
        );
    });
}

#[test]
fn credential_submission_flags_ai_agents() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        let expiry = System::<Test>::block_number() + CredentialTtlConst::get();
        assert_ok!(Polkamarkt::submit_credential(
            RuntimeOrigin::signed(ALICE),
            [9u8; 32],
            expiry,
            DEFAULT_JURISDICTION,
            true
        ));
        let stored = Polkamarkt::credentials(ALICE).expect("stored credential");
        assert!(stored.3, "expected AI agent flag");
        System::<Test>::assert_last_event(RuntimeEvent::Polkamarkt(Event::CredentialSubmitted {
            who: ALICE,
            expires_at: stored.0,
            jurisdiction: DEFAULT_JURISDICTION,
            ai_agent: true,
        }));
    });
}

#[test]
fn blocked_jurisdiction_halts_withdrawals() {
    new_test_ext().execute_with(|| {
        run_to_block(1);
        provide_credential(ALICE);
        assert_ok!(Polkamarkt::set_bridge_wallet(
            RuntimeOrigin::signed(ALICE),
            BOB
        ));
        assert_ok!(Polkamarkt::set_jurisdiction_block(
            RuntimeOrigin::root(),
            DEFAULT_JURISDICTION,
            true
        ));
        assert_noop!(
            Polkamarkt::bridge_withdraw(RuntimeOrigin::signed(ALICE), 1_000),
            Error::<Test>::JurisdictionBlocked
        );
    });
}
