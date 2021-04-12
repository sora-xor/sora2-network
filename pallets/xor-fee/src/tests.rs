use common::prelude::{AssetName, AssetSymbol, FixedWrapper};
use common::{balance, fixed_wrapper, XOR};
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;
use traits::MultiCurrency;

use crate::mock::*;

type BlockWeights = <Runtime as frame_system::Config>::BlockWeights;
type TransactionByteFee = <Runtime as pallet_transaction_payment::Config>::TransactionByteFee;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            pallet_balances::Call::transfer(TO_ACCOUNT, TRANSFER_AMOUNT as u128 * balance!(1)),
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
fn notify_val_burned_works() {
    ExtBuilder::build().execute_with(|| {
        assert_eq!(
            pallet_staking::Module::<Runtime>::era_val_burned(),
            0_u128.into()
        );
        let call: &<Runtime as frame_system::Config>::Call = &Call::Balances(
            pallet_balances::Call::transfer(TO_ACCOUNT, TRANSFER_AMOUNT as u128 * balance!(1)),
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
            pallet_balances::Call::transfer(TO_ACCOUNT, TRANSFER_AMOUNT as u128 * balance!(1)),
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
            pallet_balances::Call::transfer(TO_ACCOUNT, TRANSFER_AMOUNT as u128 * balance!(1)),
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
        assert!(
            Tokens::free_balance(ValId::get(), &SORA_PARLIAMENT_ACCOUNT)
                >= (expected_balance.clone() - FixedWrapper::from(1)).into_balance()
                && Balances::free_balance(SORA_PARLIAMENT_ACCOUNT)
                    <= (expected_balance + FixedWrapper::from(1)).into_balance()
        );
    });
}
