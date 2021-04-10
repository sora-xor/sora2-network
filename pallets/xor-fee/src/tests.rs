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

use common::prelude::FixedWrapper;
use common::{balance, fixed_wrapper};
use pallet_balances::Call as BalancesCall;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

use crate::mock::*;

type BlockWeights = <Runtime as frame_system::Config>::BlockWeights;
type TransactionByteFee = <Runtime as pallet_transaction_payment::Config>::TransactionByteFee;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            BalancesCall::transfer(TO_ACCOUNT, TRANSFER_AMOUNT as u128 * balance!(1)),
        );

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let pre = ChargeTransactionPayment::<Runtime>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        let base_fee = BlockWeights::get().get(dispatch_info.class).base_extrinsic as u128;
        let len_fee = len as u128 * TransactionByteFee::get();
        let weight_fee = MOCK_WEIGHT as u128;
        let balance_after_reserving_fee =
            FixedWrapper::from(initial_balance()) - base_fee - len_fee - weight_fee;
        let balance_after_reserving_fee = balance_after_reserving_fee.into_balance();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_reserving_fee
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            pre,
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_reserving_fee
        );
        let weights_sum: FixedWrapper =
            FixedWrapper::from(ReferrerWeight::get() as u128 * balance!(1))
                + FixedWrapper::from(XorBurnedWeight::get() as u128 * balance!(1))
                + FixedWrapper::from(XorIntoValBurnedWeight::get() as u128 * balance!(1));
        let referrer_weight = FixedWrapper::from(ReferrerWeight::get() as u128 * balance!(1));
        let initial_balance = FixedWrapper::from(initial_balance());
        let expected_referrer_balance =
            FixedWrapper::from(weight_fee) * referrer_weight / weights_sum + initial_balance;
        assert!(
            Balances::free_balance(REFERRER_ACCOUNT)
                >= (expected_referrer_balance.clone() - fixed_wrapper!(1)).into_balance()
                && Balances::free_balance(REFERRER_ACCOUNT)
                    <= (expected_referrer_balance + fixed_wrapper!(1)).into_balance()
        );
    });
}

#[test]
#[ignore] // FIXME: should be investigated, fails for non-zero extrinsic base weight
fn notify_val_burned_works() {
    ExtBuilder::build().execute_with(|| {
        assert_eq!(
            pallet_staking::Module::<Runtime>::era_val_burned(),
            0_u128.into()
        );
        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            BalancesCall::transfer(TO_ACCOUNT, TRANSFER_AMOUNT as u128 * balance!(1)),
        );

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let pre = ChargeTransactionPayment::<Runtime>::from(0_u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            pre,
            &dispatch_info,
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        let base_fee = BlockWeights::get().get(dispatch_info.class).base_extrinsic as u128;
        let len_fee = len as u128 * TransactionByteFee::get();
        let weight_fee = MOCK_WEIGHT as u128;
        let fee = base_fee + len_fee + weight_fee;
        let xor_into_val_burned_weight = XorIntoValBurnedWeight::get() as u128;
        let weights_sum = ReferrerWeight::get() as u128
            + XorBurnedWeight::get() as u128
            + xor_into_val_burned_weight;
        let x = FixedWrapper::from(fee * xor_into_val_burned_weight as u128 / weights_sum);
        let y = initial_reserves();
        let expected_val_burned = x.clone() * y / (x + y);
        assert_eq!(
            pallet_staking::Module::<Runtime>::era_val_burned(),
            expected_val_burned.into_balance()
        );
    });
}
