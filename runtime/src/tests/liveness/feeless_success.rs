// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{
    AccountId, Assets, Balances, Executive, OrderBook, Preimage, Runtime, RuntimeCall,
    RuntimeEvent, RuntimeOrigin, Signature, SignedExtra, System, UncheckedExtrinsic, XorFee,
};
use codec::Encode;
use common::{balance, AssetInfoProvider, OrderBookId, PriceVariant, PSWAP, VAL, XOR};
use frame_support::{
    assert_ok,
    dispatch::{GetDispatchInfo, Pays},
    traits::Currency,
};
use framenode_chain_spec::ext;
use sp_core::{sr25519, Pair};
use sp_runtime::{generic::Era, DispatchResult, FixedPointNumber, FixedU128};

type BookId = OrderBookId<crate::AssetId, crate::DEXId>;

fn payer() -> sr25519::Pair {
    sr25519::Pair::from_seed(&[79; 32])
}

fn account(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public())
}

fn setup(pair: &sr25519::Pair) {
    System::set_block_number(1);
    let _ = Balances::make_free_balance_be(&account(pair), balance!(1000000));
    assert_ok!(XorFee::update_multiplier(
        RuntimeOrigin::root(),
        FixedU128::saturating_from_integer(1),
    ));
}

fn signed(call: RuntimeCall, pair: &sr25519::Pair) -> UncheckedExtrinsic {
    let who = account(pair);
    let extra: SignedExtra = (
        frame_system::CheckSpecVersion::<Runtime>::new(),
        frame_system::CheckTxVersion::<Runtime>::new(),
        frame_system::CheckGenesis::<Runtime>::new(),
        frame_system::CheckEra::<Runtime>::from(Era::Immortal),
        frame_system::CheckNonce::<Runtime>::from(System::account_nonce(&who)),
        frame_system::CheckWeight::<Runtime>::new(),
        crate::charge_tx_payment_extension(),
    );
    let payload = crate::SignedPayload::new(call.clone(), extra.clone()).unwrap();
    let signature = payload.using_encoded(|payload| pair.sign(payload));
    UncheckedExtrinsic::new_signed(call, who, Signature::Sr25519(signature), extra)
}

/// Assert the real signed payment extension agrees with fee events and balances,
/// including success refunds and ordinary dispatch failures.
fn apply(call: RuntimeCall, pair: &sr25519::Pair, free: bool) -> DispatchResult {
    frame_system::BlockSize::<Runtime>::kill();
    frame_system::BlockWeight::<Runtime>::kill();
    assert_eq!(call.get_dispatch_info().pays_fee, Pays::Yes);
    let extrinsic = signed(call, pair);
    let quote = XorFee::query_info(
        &extrinsic,
        &extrinsic.function,
        extrinsic.encoded_size() as u32,
    )
    .partial_fee;
    assert!(quote > 0);
    let who = account(pair);
    let before = Balances::free_balance(&who);
    let nonce = System::account_nonce(&who);
    System::reset_events();
    let result = Executive::apply_extrinsic(extrinsic).expect("funded signed call is admitted");
    let charged = before - Balances::free_balance(&who);
    assert_eq!(System::account_nonce(&who), nonce + 1);
    if free {
        assert_eq!(charged, 0);
    } else {
        assert!(charged > 0);
    }
    let paid = System::events()
        .into_iter()
        .find_map(|event| match event.event {
            RuntimeEvent::TransactionPayment(
                pallet_transaction_payment::Event::TransactionFeePaid {
                    who: fee_payer,
                    actual_fee,
                    tip,
                },
            ) if fee_payer == who => {
                assert_eq!(tip, 0);
                Some(actual_fee)
            }
            _ => None,
        })
        .expect("actual fee is emitted even when refunded");
    assert_eq!(paid, charged);
    assert!(charged <= quote);
    result
}

fn book(base: crate::AssetId) -> BookId {
    BookId {
        dex_id: common::DEXId::Polkaswap.into(),
        base,
        quote: XOR,
    }
}

fn place(book: BookId, who: &AccountId) -> <Runtime as order_book::Config>::OrderId {
    if OrderBook::order_books(book).is_none() {
        assert_ok!(OrderBook::create_orderbook(
            RuntimeOrigin::root(),
            book,
            balance!(0.00001),
            balance!(0.00001),
            balance!(1),
            balance!(1000)
        ));
    }
    assert_ok!(crate::Assets::update_balance(
        RuntimeOrigin::root(),
        who.clone(),
        book.base,
        balance!(1000) as i128
    ));
    assert_ok!(OrderBook::place_limit_order(
        RuntimeOrigin::signed(who.clone()),
        book,
        balance!(10),
        balance!(10),
        PriceVariant::Sell,
        Some(100000)
    ));
    OrderBook::order_books(book).unwrap().last_order_id
}

fn order_book_storage() -> Vec<(Vec<u8>, Vec<u8>)> {
    let prefix = sp_io::hashing::twox_128(b"OrderBook").to_vec();
    let mut key = prefix.clone();
    let mut state = Vec::new();
    while let Some(next) = sp_io::storage::next_key(&key) {
        if !next.starts_with(&prefix) {
            break;
        }
        state.push((next.clone(), sp_io::storage::get(&next).unwrap().to_vec()));
        key = next;
    }
    state
}

#[test]
fn empty_and_noop_cancellation_batches_pay_fees() {
    ext().execute_with(|| {
        let pair = payer();
        setup(&pair);
        let id = book(VAL);
        for batch in [vec![], vec![(id, vec![])], vec![(id, vec![]); 16]] {
            let result = apply(
                RuntimeCall::OrderBook(order_book::Call::cancel_limit_orders_batch {
                    limit_orders_to_cancel: batch,
                }),
                &pair,
                false,
            );
            assert_eq!(
                result.unwrap_err(),
                order_book::Error::<Runtime>::EmptyCancellationBatch.into()
            );
        }
    });
}

#[test]
fn real_single_and_multibook_cancellations_stay_free_and_replay_pays() {
    ext().execute_with(|| {
        let pair = payer();
        setup(&pair);
        let who = account(&pair);
        let first = book(VAL);
        let second = book(PSWAP);
        let order = place(first, &who);
        let call = RuntimeCall::OrderBook(order_book::Call::cancel_limit_order {
            order_book_id: first,
            order_id: order,
        });
        assert_ok!(apply(call.clone(), &pair, true));
        assert_eq!(
            apply(call, &pair, false).unwrap_err(),
            order_book::Error::<Runtime>::UnknownLimitOrder.into()
        );
        let a = place(first, &who);
        let b = place(second, &who);
        assert_ok!(apply(
            RuntimeCall::OrderBook(order_book::Call::cancel_limit_orders_batch {
                limit_orders_to_cancel: vec![(first, vec![a]), (second, vec![b])],
            }),
            &pair,
            true
        ));
        assert!(!order_book::LimitOrders::<Runtime>::contains_key(first, a));
        assert!(!order_book::LimitOrders::<Runtime>::contains_key(second, b));
    });
}

#[test]
fn duplicate_and_mixed_cancellation_failures_pay_and_restore_orders_funds_and_expiry() {
    ext().execute_with(|| {
        let pair = payer();
        setup(&pair);
        let who = account(&pair);
        let id = book(VAL);
        let own = place(id, &who);
        let foreign = place(id, &AccountId::from([81; 32]));
        for (batch, error) in [
            (
                vec![(id, vec![own, own])],
                order_book::Error::<Runtime>::UnknownLimitOrder,
            ),
            (
                vec![(id, vec![own]), (id, vec![own])],
                order_book::Error::<Runtime>::UnknownLimitOrder,
            ),
            (
                vec![(id, vec![own, 999999])],
                order_book::Error::<Runtime>::UnknownLimitOrder,
            ),
            (
                vec![(id, vec![own, foreign])],
                order_book::Error::<Runtime>::Unauthorized,
            ),
            (
                vec![(id, vec![own]), (id, vec![])],
                order_book::Error::<Runtime>::EmptyCancellationBatch,
            ),
        ] {
            let storage = order_book_storage();
            let balance = Assets::free_balance(&id.base, &who).unwrap();
            assert_eq!(
                apply(
                    RuntimeCall::OrderBook(order_book::Call::cancel_limit_orders_batch {
                        limit_orders_to_cancel: batch,
                    }),
                    &pair,
                    false
                )
                .unwrap_err(),
                error.into()
            );
            assert_eq!(order_book_storage(), storage);
            assert_eq!(Assets::free_balance(&id.base, &who).unwrap(), balance);
            assert!(!System::events().iter().any(|event| matches!(
                event.event,
                RuntimeEvent::OrderBook(order_book::Event::LimitOrderCanceled { .. })
            )));
        }
        assert_ok!(OrderBook::change_orderbook_status(
            RuntimeOrigin::root(),
            id,
            order_book::OrderBookStatus::Stop,
        ));
        let stopped = order_book_storage();
        assert_eq!(
            apply(
                RuntimeCall::OrderBook(order_book::Call::cancel_limit_orders_batch {
                    limit_orders_to_cancel: vec![(id, vec![own])],
                }),
                &pair,
                false
            )
            .unwrap_err(),
            order_book::Error::<Runtime>::CancellationOfLimitOrdersIsForbidden.into()
        );
        assert_eq!(order_book_storage(), stopped);
        assert_ok!(OrderBook::change_orderbook_status(
            RuntimeOrigin::root(),
            id,
            order_book::OrderBookStatus::OnlyCancel,
        ));
        assert_ok!(apply(
            RuntimeCall::OrderBook(order_book::Call::cancel_limit_orders_batch {
                limit_orders_to_cancel: vec![(id, vec![own])],
            }),
            &pair,
            true
        ));
    });
}

#[test]
fn unauthorized_rewards_updates_keep_nonzero_fees() {
    ext().execute_with(|| {
        let pair = payer();
        setup(&pair);
        for receivers in [vec![], vec![sp_core::H160::repeat_byte(3)]] {
            assert_eq!(
                apply(
                    RuntimeCall::Rewards(rewards::Call::add_umi_nft_receivers { receivers }),
                    &pair,
                    false
                )
                .unwrap_err(),
                frame_support::error::BadOrigin.into()
            );
        }
    });
}

#[test]
fn requested_preimage_first_provision_refunds_and_all_signed_replays_pay() {
    ext().execute_with(|| {
        let pair = payer();
        setup(&pair);
        let bytes = vec![17; 65536];
        let hash = sp_core::H256::from(sp_io::hashing::blake2_256(&bytes));
        assert_ok!(Preimage::request_preimage(RuntimeOrigin::root(), hash));
        assert_ok!(Preimage::request_preimage(RuntimeOrigin::root(), hash));
        let call = RuntimeCall::Preimage(pallet_preimage::Call::note_preimage {
            bytes: bytes.clone(),
        });
        assert_ok!(apply(call.clone(), &pair, true));
        let state = pallet_preimage::RequestStatusFor::<Runtime>::get(hash);
        for _ in 0..2 {
            assert_eq!(
                apply(call.clone(), &pair, false).unwrap_err(),
                pallet_preimage::Error::<Runtime>::AlreadyNoted.into()
            );
            assert_eq!(
                pallet_preimage::RequestStatusFor::<Runtime>::get(hash),
                state
            );
            assert_eq!(
                pallet_preimage::PreimageFor::<Runtime>::get((hash, bytes.len() as u32))
                    .unwrap()
                    .to_vec(),
                bytes
            );
        }
        let other = sr25519::Pair::from_seed(&[80; 32]);
        setup(&other);
        assert_eq!(
            apply(call, &other, false).unwrap_err(),
            pallet_preimage::Error::<Runtime>::AlreadyNoted.into()
        );
        assert_eq!(
            pallet_preimage::RequestStatusFor::<Runtime>::get(hash),
            state
        );
    });
}
