// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

pub mod burn_xor_in_tech_accounts {
    use crate::storage_data_layer::StorageDataLayer;
    use crate::{CancelReason, Config, LimitOrders, MarketChange, OrderBooks, Pallet};
    use common::{AssetIdOf, AssetInfoProvider, AssetManager, PriceVariant, XOR};
    use core::marker::PhantomData;
    use frame_support::ensure;
    use frame_support::traits::{Get, OnRuntimeUpgrade, StorageVersion};
    use frame_support::weights::Weight;
    use sp_runtime::traits::Zero;
    use sp_runtime::DispatchError;
    use sp_std::collections::btree_map::BTreeMap;

    pub struct Migrate<T>(PhantomData<T>);

    const MAX_ORDER_BOOKS_TO_PROCESS: usize = 100_000;
    const MAX_XOR_BACKED_ORDERS_TO_PROCESS: usize = 1_000_000;

    impl<T: Config + assets::Config> OnRuntimeUpgrade for Migrate<T> {
        fn on_runtime_upgrade() -> Weight {
            let on_chain = StorageVersion::get::<Pallet<T>>();
            if on_chain != StorageVersion::new(0) {
                frame_support::__private::log::info!(
                    "order-book v1 migration skipped, on-chain storage version is {:?}",
                    on_chain
                );
                return Weight::zero();
            }

            let migration_result = common::with_transaction(|| -> Result<(), DispatchError> {
                let xor: AssetIdOf<T> = XOR.into();
                let xor_assets: <T as assets::Config>::AssetId = xor.into();
                let mut data = StorageDataLayer::<T>::new();
                let mut order_book_count = 0usize;
                let mut xor_backed_order_count = 0usize;

                for (order_book_id, order_book) in OrderBooks::<T>::iter() {
                    order_book_count = order_book_count.saturating_add(1);
                    ensure!(
                        order_book_count <= MAX_ORDER_BOOKS_TO_PROCESS,
                        DispatchError::Other(
                            "order-book migration aborted: too many order books to process"
                        )
                    );

                    let to_cancel = LimitOrders::<T>::iter_prefix_values(order_book_id)
                        .filter(|order| {
                            (order.side == PriceVariant::Buy && order_book_id.quote == xor)
                                || (order.side == PriceVariant::Sell && order_book_id.base == xor)
                        })
                        .map(|order| (order.id, (order, CancelReason::Aligned)))
                        .collect::<BTreeMap<_, _>>();

                    if to_cancel.is_empty() {
                        continue;
                    }

                    xor_backed_order_count = xor_backed_order_count.saturating_add(to_cancel.len());
                    ensure!(
                        xor_backed_order_count <= MAX_XOR_BACKED_ORDERS_TO_PROCESS,
                        DispatchError::Other(
                            "order-book migration aborted: too many XOR-backed orders to process"
                        )
                    );
                    let mut market_change = MarketChange::new(order_book_id);
                    market_change.to_cancel = to_cancel;
                    market_change.ignore_unschedule_error = true;
                    order_book.apply_market_change(market_change, &mut data)?;
                }

                for order_book_id in OrderBooks::<T>::iter_keys() {
                    let tech_account = Pallet::<T>::tech_account_for_order_book(&order_book_id);
                    let tech_account_id =
                        technical::Pallet::<T>::tech_account_id_to_account_id(&tech_account)?;

                    let total_xor =
                        <T as Config>::AssetInfoProvider::total_balance(&xor, &tech_account_id)?;
                    if total_xor.is_zero() {
                        continue;
                    }

                    let free_xor =
                        <T as Config>::AssetInfoProvider::free_balance(&xor, &tech_account_id)?;
                    let reserved_xor = total_xor.saturating_sub(free_xor);

                    if !reserved_xor.is_zero() {
                        let remainder = assets::Pallet::<T>::unreserve(
                            &xor_assets,
                            &tech_account_id,
                            reserved_xor,
                        )?;
                        ensure!(
                            remainder.is_zero(),
                            DispatchError::Other(
                                "order-book migration: failed to unreserve all reserved XOR"
                            )
                        );
                    }

                    let burnable_xor =
                        <T as Config>::AssetInfoProvider::free_balance(&xor, &tech_account_id)?;
                    if !burnable_xor.is_zero() {
                        T::AssetManager::burn_from(
                            &xor,
                            &tech_account_id,
                            &tech_account_id,
                            burnable_xor,
                        )?;
                    }

                    let remaining_xor =
                        <T as Config>::AssetInfoProvider::total_balance(&xor, &tech_account_id)?;
                    ensure!(
                        remaining_xor.is_zero(),
                        DispatchError::Other(
                            "order-book migration: non-zero XOR balance remains in tech account"
                        )
                    );
                }

                Ok(())
            });

            if let Err(error) = migration_result {
                frame_support::__private::log::error!(
                    "order-book v1 migration failed and was rolled back: {:?}",
                    error
                );
                return <T as frame_system::Config>::BlockWeights::get().max_block;
            }

            StorageVersion::new(1).put::<Pallet<T>>();
            <T as frame_system::Config>::BlockWeights::get().max_block
        }
    }

    #[cfg(test)]
    mod tests {
        use crate::test_utils::*;
        use common::{
            balance, AssetIdOf, AssetInfoProvider, OrderBookId, PriceVariant, KUSD, VAL, XOR,
        };
        use frame_support::weights::Weight;
        use frame_support::{
            assert_ok,
            traits::{OnRuntimeUpgrade, StorageVersion},
        };
        use frame_system::RawOrigin;
        use framenode_chain_spec::ext;
        use framenode_runtime::{order_book as runtime_order_book, Runtime, TradingPair};

        fn order_book_with_xor_quote() -> OrderBookId<AssetIdOf<Runtime>, DEXId> {
            OrderBookId {
                dex_id: DEX.into(),
                base: VAL,
                quote: XOR,
            }
        }

        fn order_book_without_xor_collateral() -> OrderBookId<AssetIdOf<Runtime>, DEXId> {
            OrderBookId {
                dex_id: common::DEXId::PolkaswapKUSD.into(),
                base: VAL,
                quote: KUSD,
            }
        }

        fn order_book_with_xor_base() -> OrderBookId<AssetIdOf<Runtime>, DEXId> {
            OrderBookId {
                dex_id: common::DEXId::PolkaswapKUSD.into(),
                base: XOR,
                quote: KUSD,
            }
        }

        fn create_order_book(order_book_id: OrderBookId<AssetIdOf<Runtime>, DEXId>) {
            assert_ok!(OrderBookPallet::create_orderbook(
                RawOrigin::Root.into(),
                order_book_id,
                balance!(0.00001),
                balance!(0.00001),
                balance!(1),
                balance!(1000)
            ));
        }

        fn tech_account_for_order_book(
            order_book_id: &OrderBookId<AssetIdOf<Runtime>, DEXId>,
        ) -> <Runtime as frame_system::Config>::AccountId {
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(
                &OrderBookPallet::tech_account_for_order_book(order_book_id),
            )
            .unwrap()
        }

        type RuntimeMigration =
            framenode_runtime::order_book::migrations::burn_xor_in_tech_accounts::Migrate<Runtime>;

        #[test]
        fn migration_should_cancel_xor_backed_orders_and_burn_xor() {
            ext().execute_with(|| {
                let xor_quote_order_book = order_book_with_xor_quote();
                let no_xor_collateral_order_book = order_book_without_xor_collateral();
                let xor_base_order_book = order_book_with_xor_base();

                assert_ok!(TradingPair::register(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    no_xor_collateral_order_book.dex_id,
                    no_xor_collateral_order_book.quote,
                    no_xor_collateral_order_book.base
                ));
                assert_ok!(TradingPair::register(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    xor_base_order_book.dex_id,
                    xor_base_order_book.quote,
                    xor_base_order_book.base
                ));

                fill_balance::<Runtime>(accounts::alice::<Runtime>(), xor_quote_order_book);
                fill_balance::<Runtime>(accounts::alice::<Runtime>(), no_xor_collateral_order_book);
                fill_balance::<Runtime>(accounts::bob::<Runtime>(), xor_base_order_book);

                create_order_book(xor_quote_order_book);
                create_order_book(no_xor_collateral_order_book);
                create_order_book(xor_base_order_book);

                let lifespan = Some(100_000u32.into());

                assert_ok!(OrderBookPallet::place_limit_order(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    xor_quote_order_book,
                    balance!(10),
                    balance!(5),
                    PriceVariant::Buy,
                    lifespan
                ));

                assert_ok!(OrderBookPallet::place_limit_order(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    no_xor_collateral_order_book,
                    balance!(10),
                    balance!(5),
                    PriceVariant::Buy,
                    lifespan
                ));

                assert_ok!(OrderBookPallet::place_limit_order(
                    RawOrigin::Signed(accounts::bob::<Runtime>()).into(),
                    xor_base_order_book,
                    balance!(10),
                    balance!(3),
                    PriceVariant::Sell,
                    lifespan
                ));

                let xor_quote_tech = tech_account_for_order_book(&xor_quote_order_book);
                let no_xor_collateral_tech =
                    tech_account_for_order_book(&no_xor_collateral_order_book);
                let xor_base_tech = tech_account_for_order_book(&xor_base_order_book);

                let alice_xor_after_placement =
                    free_balance::<Runtime>(&XOR, &accounts::alice::<Runtime>());
                let bob_xor_after_placement =
                    free_balance::<Runtime>(&XOR, &accounts::bob::<Runtime>());

                assert!(free_balance::<Runtime>(&XOR, &xor_quote_tech) > balance!(0));
                assert_eq!(
                    free_balance::<Runtime>(&XOR, &no_xor_collateral_tech),
                    balance!(0)
                );
                assert!(free_balance::<Runtime>(&XOR, &xor_base_tech) > balance!(0));

                StorageVersion::new(0).put::<OrderBookPallet>();
                let migration_weight = RuntimeMigration::on_runtime_upgrade();
                assert_ne!(migration_weight, Weight::zero());

                assert_eq!(
                    StorageVersion::get::<OrderBookPallet>(),
                    StorageVersion::new(1)
                );
                assert!(OrderBookPallet::limit_orders(xor_quote_order_book, 1).is_none());
                assert!(OrderBookPallet::limit_orders(xor_base_order_book, 1).is_none());
                assert!(OrderBookPallet::limit_orders(no_xor_collateral_order_book, 1).is_some());

                assert_eq!(free_balance::<Runtime>(&XOR, &xor_quote_tech), balance!(0));
                assert_eq!(free_balance::<Runtime>(&XOR, &xor_base_tech), balance!(0));

                assert_eq!(
                    free_balance::<Runtime>(&XOR, &accounts::alice::<Runtime>()),
                    alice_xor_after_placement
                );
                assert_eq!(
                    free_balance::<Runtime>(&XOR, &accounts::bob::<Runtime>()),
                    bob_xor_after_placement
                );

                let second_weight = RuntimeMigration::on_runtime_upgrade();
                assert_eq!(second_weight, Weight::zero());
                assert!(OrderBookPallet::limit_orders(no_xor_collateral_order_book, 1).is_some());
                assert_eq!(free_balance::<Runtime>(&XOR, &xor_quote_tech), balance!(0));
                assert_eq!(free_balance::<Runtime>(&XOR, &xor_base_tech), balance!(0));
            });
        }

        #[test]
        fn migration_should_burn_reserved_xor_in_tech_accounts() {
            ext().execute_with(|| {
                let xor_quote_order_book = order_book_with_xor_quote();

                fill_balance::<Runtime>(accounts::alice::<Runtime>(), xor_quote_order_book);
                create_order_book(xor_quote_order_book);

                assert_ok!(OrderBookPallet::place_limit_order(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    xor_quote_order_book,
                    balance!(10),
                    balance!(6),
                    PriceVariant::Buy,
                    Some(100_000u32.into())
                ));

                let xor_quote_tech = tech_account_for_order_book(&xor_quote_order_book);
                let total_before = assets::Pallet::<Runtime>::total_balance(&XOR, &xor_quote_tech)
                    .expect("XOR must exist");
                assert!(total_before > balance!(0));

                let reserve_amount = total_before / 2;
                assert_ok!(assets::Pallet::<Runtime>::reserve(
                    &XOR,
                    &xor_quote_tech,
                    reserve_amount
                ));
                assert_eq!(
                    assets::Pallet::<Runtime>::total_balance(&XOR, &xor_quote_tech)
                        .expect("XOR must exist"),
                    total_before
                );

                StorageVersion::new(0).put::<OrderBookPallet>();
                assert_ne!(RuntimeMigration::on_runtime_upgrade(), Weight::zero());

                assert_eq!(
                    assets::Pallet::<Runtime>::total_balance(&XOR, &xor_quote_tech)
                        .expect("XOR must exist"),
                    balance!(0)
                );
                assert_eq!(
                    StorageVersion::get::<OrderBookPallet>(),
                    StorageVersion::new(1)
                );
            });
        }

        #[test]
        fn migration_should_rollback_and_keep_version_on_failure() {
            ext().execute_with(|| {
                let xor_quote_order_book = order_book_with_xor_quote();

                fill_balance::<Runtime>(accounts::alice::<Runtime>(), xor_quote_order_book);
                create_order_book(xor_quote_order_book);

                assert_ok!(OrderBookPallet::place_limit_order(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    xor_quote_order_book,
                    balance!(10),
                    balance!(5),
                    PriceVariant::Buy,
                    Some(100_000u32.into())
                ));

                // Corrupt the user-order index to force cancellation failure in the migration.
                runtime_order_book::UserLimitOrders::<Runtime>::remove(
                    accounts::alice::<Runtime>(),
                    xor_quote_order_book,
                );

                StorageVersion::new(0).put::<OrderBookPallet>();
                assert_ne!(RuntimeMigration::on_runtime_upgrade(), Weight::zero());

                assert_eq!(
                    StorageVersion::get::<OrderBookPallet>(),
                    StorageVersion::new(0)
                );
                assert!(OrderBookPallet::limit_orders(xor_quote_order_book, 1).is_some());
            });
        }

        #[test]
        fn migration_should_work_for_undercollateralized_xor_orders() {
            ext().execute_with(|| {
                let xor_quote_order_book = order_book_with_xor_quote();

                fill_balance::<Runtime>(accounts::alice::<Runtime>(), xor_quote_order_book);
                create_order_book(xor_quote_order_book);

                assert_ok!(OrderBookPallet::place_limit_order(
                    RawOrigin::Signed(accounts::alice::<Runtime>()).into(),
                    xor_quote_order_book,
                    balance!(10),
                    balance!(5),
                    PriceVariant::Buy,
                    Some(100_000u32.into())
                ));

                let xor_quote_tech = tech_account_for_order_book(&xor_quote_order_book);
                let locked_xor = free_balance::<Runtime>(&XOR, &xor_quote_tech);
                assert!(locked_xor > balance!(0));

                assert_ok!(assets::Pallet::<Runtime>::burn_from(
                    &XOR,
                    &xor_quote_tech,
                    &xor_quote_tech,
                    locked_xor
                ));
                assert_eq!(free_balance::<Runtime>(&XOR, &xor_quote_tech), balance!(0));

                let alice_xor_after_placement =
                    free_balance::<Runtime>(&XOR, &accounts::alice::<Runtime>());

                StorageVersion::new(0).put::<OrderBookPallet>();
                let _ = RuntimeMigration::on_runtime_upgrade();

                assert!(OrderBookPallet::limit_orders(xor_quote_order_book, 1).is_none());
                assert_eq!(
                    free_balance::<Runtime>(&XOR, &accounts::alice::<Runtime>()),
                    alice_xor_after_placement
                );
                assert_eq!(free_balance::<Runtime>(&XOR, &xor_quote_tech), balance!(0));
                assert_eq!(
                    StorageVersion::get::<OrderBookPallet>(),
                    StorageVersion::new(1)
                );
            });
        }
    }
}
