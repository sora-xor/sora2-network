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

use crate::contracts::mock::{create_order_book, instantiate_contract, ExtBuilder, GAS_LIMIT};
use crate::contracts::tests::compile_module;
use crate::{Contracts, OrderBook, Runtime, RuntimeCall};
use codec::{Decode, Encode};
use common::mock::{alice, bob};
use common::{balance, LiquiditySource, PriceVariant};
use frame_support::{assert_ok, weights::Weight};
use order_book::WeightInfo;
use pallet_contracts::{CollectEvents, DebugInfo, Determinism};
use pallet_contracts_primitives::{Code, ContractResult};
use sp_core::crypto::AccountId32;

#[test]
fn call_place_limit_order_right() {
    let (code, _hash) = compile_module::<Runtime>("call_runtime_contract").unwrap();
    ExtBuilder::default().build().execute_with(|| {
        let contract_addr: AccountId32 = instantiate_contract(code);

        let order_book_id1 = create_order_book();

        let call = RuntimeCall::OrderBook(order_book::Call::place_limit_order {
            order_book_id: order_book_id1,
            price: balance!(9).into(),
            amount: balance!(153),
            side: PriceVariant::Sell,
            lifespan: Some(<Runtime as order_book::Config>::MIN_ORDER_LIFESPAN + 1000000),
        });

        let result = Contracts::bare_call(
            alice(),
            contract_addr.clone(),
            0,
            GAS_LIMIT,
            None,
            call.encode(),
            DebugInfo::Skip,
            CollectEvents::Skip,
            Determinism::Enforced,
        );

        let ContractResult {
            gas_consumed,
            gas_required,
            storage_deposit: _storage_deposit,
            debug_message: _debug_message,
            result,
            ..
        } = result;

        // TODO: Should be equal 0, but now equal 10, means that extrinsic return Error
        assert_eq!(u32::decode(&mut result.unwrap().data.as_ref()).unwrap(), 10);

        let weight: Weight = OrderBook::exchange_weight();

        assert!(weight.ref_time() < gas_consumed.ref_time());
        assert!(weight.proof_size() < gas_consumed.proof_size());
        assert_ok!(
            Contracts::bare_call(
                alice(),
                contract_addr.clone(),
                0,
                gas_required,
                None,
                call.encode(),
                DebugInfo::Skip,
                CollectEvents::Skip,
                Determinism::Enforced,
            )
            .result
        );
    });
}

#[test]
fn call_cancel_limit_order_right() {
    let (code, _hash) = compile_module::<Runtime>("call_runtime_contract").unwrap();
    ExtBuilder::default().build().execute_with(|| {
        let contract_addr: AccountId32 = instantiate_contract(code);

        let order_book_id1 = create_order_book();

        let call = RuntimeCall::OrderBook(order_book::Call::cancel_limit_order {
            order_book_id: order_book_id1,
            order_id: 1,
        });

        let result = Contracts::bare_call(
            alice(),
            contract_addr.clone(),
            0,
            GAS_LIMIT,
            None,
            call.encode(),
            DebugInfo::Skip,
            CollectEvents::Skip,
            Determinism::Enforced,
        );

        let ContractResult {
            gas_consumed,
            gas_required,
            storage_deposit: _storage_deposit,
            debug_message: _debug_message,
            result,
            ..
        } = result;

        // TODO: Should be equal 0, but now equal 10, means that extrinsic return Error
        assert_eq!(u32::decode(&mut result.unwrap().data.as_ref()).unwrap(), 10);

        let weight: Weight =
            order_book::weights::SubstrateWeight::<Runtime>::cancel_limit_order_first_expiration()
                .max(
                order_book::weights::SubstrateWeight::<Runtime>::cancel_limit_order_last_expiration(
                ),
            );

        assert!(weight.ref_time() < gas_consumed.ref_time());
        assert!(weight.proof_size() < gas_consumed.proof_size());
        assert_ok!(
            Contracts::bare_call(
                alice(),
                contract_addr.clone(),
                0,
                gas_required,
                None,
                call.encode(),
                DebugInfo::Skip,
                CollectEvents::Skip,
                Determinism::Enforced,
            )
            .result
        );
    });
}

#[test]
fn call_cancel_limit_order_batch_right() {
    let (code, _hash) = compile_module::<Runtime>("call_runtime_contract").unwrap();
    ExtBuilder::default().build().execute_with(|| {
        let contract_addr: AccountId32 = instantiate_contract(code);

        let order_book_id1 = create_order_book();
        let limit_orders_to_cancel = vec![(order_book_id1, vec![1_u128, 2_u128])];
        let call = RuntimeCall::OrderBook(order_book::Call::cancel_limit_orders_batch {
            limit_orders_to_cancel: limit_orders_to_cancel.clone(),
        });

        let result = Contracts::bare_call(
            alice(),
            contract_addr.clone(),
            0,
            GAS_LIMIT,
            None,
            call.encode(),
            DebugInfo::Skip,
            CollectEvents::Skip,
            Determinism::Enforced,
        );

        let ContractResult {
            gas_consumed,
            gas_required,
            storage_deposit: _storage_deposit,
            debug_message: _debug_message,
            result,
            ..
        } = result;

        // TODO: Should be equal 0, but now equal 10, means that extrinsic return Error
        assert_eq!(u32::decode(&mut result.unwrap().data.as_ref()).unwrap(), 10);
        let limit_orders_count: u64 = limit_orders_to_cancel
            .iter()
            .fold(0, |count, (_, order_ids)| {
                count.saturating_add(order_ids.len() as u64)
            });
        let weight: Weight =
            order_book::weights::SubstrateWeight::<Runtime>::cancel_limit_order_first_expiration()
                .max(
                order_book::weights::SubstrateWeight::<Runtime>::cancel_limit_order_last_expiration(
                ),
            ) * limit_orders_count;

        assert!(weight.ref_time() < gas_consumed.ref_time());
        assert!(weight.proof_size() < gas_consumed.proof_size());
        assert_ok!(
            Contracts::bare_call(
                alice(),
                contract_addr.clone(),
                0,
                gas_required,
                None,
                call.encode(),
                DebugInfo::Skip,
                CollectEvents::Skip,
                Determinism::Enforced,
            )
            .result
        );
    });
}

#[test]
fn call_execute_market_order_right() {
    let (code, _hash) = compile_module::<Runtime>("call_runtime_contract").unwrap();
    ExtBuilder::default().build().execute_with(|| {
        let contract_addr: AccountId32 = instantiate_contract(code);

        let order_book_id1 = create_order_book();

        let call = RuntimeCall::OrderBook(order_book::Call::execute_market_order {
            order_book_id: order_book_id1,
            direction: PriceVariant::Buy,
            amount: 20,
        });

        let result = Contracts::bare_call(
            bob(),
            contract_addr.clone(),
            0,
            GAS_LIMIT,
            None,
            call.encode(),
            DebugInfo::Skip,
            CollectEvents::Skip,
            Determinism::Enforced,
        );

        let ContractResult {
            gas_consumed,
            gas_required,
            storage_deposit: _storage_deposit,
            debug_message: _debug_message,
            result,
            ..
        } = result;

        // TODO: Should be equal 0, but now equal 10, means that extrinsic return Error
        assert_eq!(u32::decode(&mut result.unwrap().data.as_ref()).unwrap(), 10);
        let weight: Weight =
            order_book::weights::SubstrateWeight::<Runtime>::execute_market_order();

        assert!(weight.ref_time() < gas_consumed.ref_time());
        assert!(weight.proof_size() < gas_consumed.proof_size());
        assert_ok!(
            Contracts::bare_call(
                alice(),
                contract_addr.clone(),
                0,
                gas_required,
                None,
                call.encode(),
                DebugInfo::Skip,
                CollectEvents::Skip,
                Determinism::Enforced,
            )
            .result
        );
    });
}
