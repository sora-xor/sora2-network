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

use common::prelude::{AssetName, AssetSymbol, FixedWrapper, SwapAmount};
use common::{balance, fixed_wrapper, FilterMode, VAL, XOR};
use pallet_transaction_payment::{ChargeTransactionPayment, OnChargeTransaction};
use sp_runtime::traits::SignedExtension;
use traits::MultiCurrency;
use xor_fee::LiquidityInfo;

use crate::mock::*;
use crate::XorToVal;

type BlockWeights = <Runtime as frame_system::Config>::BlockWeights;
type TransactionByteFee = <Runtime as pallet_transaction_payment::Config>::TransactionByteFee;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            pallet_balances::Call::transfer(TO_ACCOUNT, balance!(TRANSFER_AMOUNT)),
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
        let weights_sum: FixedWrapper = FixedWrapper::from(balance!(ReferrerWeight::get()))
            + FixedWrapper::from(balance!(XorBurnedWeight::get()))
            + FixedWrapper::from(balance!(XorIntoValBurnedWeight::get()));
        let referrer_weight = FixedWrapper::from(balance!(ReferrerWeight::get()));
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
fn notify_val_burned_works() {
    ExtBuilder::build().execute_with(|| {
        assert_eq!(
            pallet_staking::Module::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        let mut total_xor_val = 0;
        for _ in 0..3 {
            let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
                pallet_balances::Call::transfer(TO_ACCOUNT, balance!(TRANSFER_AMOUNT)),
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
            println!("x = {:?}, y = {}", x, y);
            let expected_val_burned = x.clone() * y / (x + y);
            total_xor_val += expected_val_burned.into_balance();
            println!("total: {}", total_xor_val);
        }

        // The correct answer is 3E-13 away
        assert_eq!(XorToVal::<Runtime>::get(), total_xor_val + 3);
        assert_eq!(
            pallet_staking::Module::<Runtime>::era_val_burned(),
            0_u128.into()
        );

        <Module<Runtime> as pallet_session::historical::SessionManager<_, _>>::end_session(0);

        // The correct answer is 2E-13 away
        assert_eq!(
            pallet_staking::Module::<Runtime>::era_val_burned(),
            total_xor_val + 2
        );
    });
}

#[test]
fn custom_fees_work() {
    ExtBuilder::build().execute_with(|| {
        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let base_fee = BlockWeights::get().get(dispatch_info.class).base_extrinsic as u128;
        let len_fee = len as u128 * TransactionByteFee::get();
        let weight_fee = MOCK_WEIGHT as u128;

        // A ten-fold extrinsic; fee is 0.007 XOR
        let call: &<Runtime as frame_system::Config>::Call = &Call::Assets(assets::Call::register(
            AssetSymbol(b"ALIC".to_vec()),
            AssetName(b"ALICE".to_vec()),
            balance!(0),
            true,
        ));

        let pre = ChargeTransactionPayment::<Runtime>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(initial_balance()) - fixed_wrapper!(0.007);
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_fee_withdrawal
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
            balance_after_fee_withdrawal
        );

        // A normal extrinsic; fee is 0.0007 XOR
        let call: &<Runtime as frame_system::Config>::Call =
            &Call::Assets(assets::Call::mint(XOR, TO_ACCOUNT, balance!(1)));

        let pre = ChargeTransactionPayment::<Runtime>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(balance_after_fee_withdrawal) - fixed_wrapper!(0.0007);
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_fee_withdrawal
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
            balance_after_fee_withdrawal
        );

        // An extrinsic without manual fee adjustment
        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            pallet_balances::Call::transfer(TO_ACCOUNT, balance!(TRANSFER_AMOUNT)),
        );

        let pre = ChargeTransactionPayment::<Runtime>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(balance_after_fee_withdrawal) - base_fee - len_fee - weight_fee;
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_fee_withdrawal
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
            balance_after_fee_withdrawal
        );
    });
}

#[test]
fn refund_if_pays_no_works() {
    ExtBuilder::build().execute_with(|| {
        let tech_account_id = GetXorFeeAccountId::get();
        assert_eq!(Balances::free_balance(tech_account_id), 0_u128.into());

        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);

        let call: &<Runtime as frame_system::Config>::Call = &Call::Assets(assets::Call::register(
            AssetSymbol(b"ALIC".to_vec()),
            AssetName(b"ALICE".to_vec()),
            balance!(0),
            true,
        ));

        let pre = ChargeTransactionPayment::<Runtime>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(initial_balance()) - fixed_wrapper!(0.007);
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            pre,
            &dispatch_info,
            &post_info_pays_no(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(Balances::free_balance(FROM_ACCOUNT), initial_balance(),);
        assert_eq!(Balances::free_balance(tech_account_id), 0_u128.into());
    });
}

#[test]
fn actual_weight_is_ignored_works() {
    ExtBuilder::build().execute_with(|| {
        let len = 10;
        let dispatch_info = info_from_weight(MOCK_WEIGHT);
        let base_fee = BlockWeights::get().get(dispatch_info.class).base_extrinsic as u128;
        let len_fee = len as u128 * TransactionByteFee::get();
        let weight_fee = MOCK_WEIGHT as u128;

        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            pallet_balances::Call::transfer(TO_ACCOUNT, balance!(TRANSFER_AMOUNT)),
        );

        let pre = ChargeTransactionPayment::<Runtime>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &dispatch_info, len)
            .unwrap();
        let balance_after_fee_withdrawal =
            FixedWrapper::from(initial_balance()) - base_fee - len_fee - weight_fee;
        let balance_after_fee_withdrawal = balance_after_fee_withdrawal.into_balance();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_fee_withdrawal
        );
        assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(
            pre,
            &dispatch_info,
            &post_info_from_weight(MOCK_WEIGHT / 2),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_after_fee_withdrawal,
        );
    });
}

#[test]
fn reminting_for_sora_parliament_works() {
    ExtBuilder::build().execute_with(|| {
        assert_eq!(
            Balances::free_balance(SORA_PARLIAMENT_ACCOUNT),
            0_u128.into()
        );
        let call: &<Runtime as frame_system::Config>::Call = &Call::Assets(assets::Call::register(
            AssetSymbol(b"ALIC".to_vec()),
            AssetName(b"ALICE".to_vec()),
            balance!(0),
            true,
        ));

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
        let fee = balance!(0.007);
        let xor_into_val_burned_weight = XorIntoValBurnedWeight::get() as u128;
        let weights_sum = ReferrerWeight::get() as u128
            + XorBurnedWeight::get() as u128
            + xor_into_val_burned_weight;
        let x = FixedWrapper::from(fee / (weights_sum / xor_into_val_burned_weight));
        let y = initial_reserves();
        let val_burned = (x.clone() * y / (x + y)).into_balance();

        let sora_parliament_share = SoraParliamentShare::get();
        let expected_balance = FixedWrapper::from(sora_parliament_share * val_burned);

        <Module<Runtime> as pallet_session::historical::SessionManager<_, _>>::end_session(0);

        assert!(
            Tokens::free_balance(ValId::get(), &SORA_PARLIAMENT_ACCOUNT)
                >= (expected_balance.clone() - FixedWrapper::from(1)).into_balance()
                && Balances::free_balance(SORA_PARLIAMENT_ACCOUNT)
                    <= (expected_balance + FixedWrapper::from(1)).into_balance()
        );
    });
}

/// No special fee handling should be performed
#[test]
fn fee_payment_regular_swap() {
    ExtBuilder::build().execute_with(|| {
        let dex_id = common::DEXId::Polkaswap;
        let dispatch_info = info_from_weight(100_000_000);

        let call = Call::LiquidityProxy(mock_liquidity_proxy::Call::swap(
            dex_id,
            VAL,
            XOR,
            SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(100),
            },
            vec![],
            FilterMode::Disabled,
        ));

        let regular_fee =
            xor_fee::Pallet::<Runtime>::withdraw_fee(&FROM_ACCOUNT, &call, &dispatch_info, 1337, 0);

        assert!(matches!(regular_fee, Ok(LiquidityInfo::Paid(_))));
    });
}

/// Fee should be postponed until after the transaction
#[test]
fn fee_payment_postponed() {
    ExtBuilder::build().execute_with(|| {
        let dex_id = common::DEXId::Polkaswap;
        let dispatch_info = info_from_weight(100_000_000);

        let call = Call::LiquidityProxy(mock_liquidity_proxy::Call::swap(
            dex_id,
            VAL,
            XOR,
            SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(50),
            },
            vec![],
            FilterMode::Disabled,
        ));

        let quoted_fee = xor_fee::Pallet::<Runtime>::withdraw_fee(
            &EMPTY_ACCOUNT,
            &call,
            &dispatch_info,
            1337,
            0,
        );

        assert!(matches!(quoted_fee, Err(_)));
    });
}

/// Payment should not be postponed if we are not producing XOR
#[test]
fn fee_payment_should_not_postpone() {
    ExtBuilder::build().execute_with(|| {
        let dex_id = common::DEXId::Polkaswap;
        let dispatch_info = info_from_weight(100_000_000);

        let call = Call::LiquidityProxy(mock_liquidity_proxy::Call::swap(
            dex_id,
            XOR,
            VAL,
            SwapAmount::WithDesiredInput {
                desired_amount_in: balance!(100),
                min_amount_out: balance!(100),
            },
            vec![],
            FilterMode::Disabled,
        ));

        let quoted_fee = xor_fee::Pallet::<Runtime>::withdraw_fee(
            &EMPTY_ACCOUNT,
            &call,
            &dispatch_info,
            1337,
            0,
        );

        assert!(matches!(quoted_fee, Err(_)));
    });
}
