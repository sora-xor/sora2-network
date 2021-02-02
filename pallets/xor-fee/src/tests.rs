use common::prelude::FixedWrapper;
use common::{fixed_wrapper, Fixed};
use pallet_balances::Call as BalancesCall;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

use crate::mock::*;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let call: &<Test as frame_system::Trait>::Call =
            &Call::Balances(BalancesCall::transfer(TO_ACCOUNT, TRANSFER_AMOUNT.into()));

        let len = 10;
        let pre = ChargeTransactionPayment::<Test>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &info_from_weight(MOCK_WEIGHT), len)
            .unwrap();
        let mock_weight: FixedWrapper = MOCK_WEIGHT.into();
        let balance_without_mock: Fixed = (FixedWrapper::from(INITIAL_BALANCE)
            - mock_weight.clone())
        .get()
        .unwrap();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_without_mock.into()
        );
        assert!(ChargeTransactionPayment::<Test>::post_dispatch(
            pre,
            &info_from_weight(MOCK_WEIGHT),
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_without_mock.into()
        );
        let weights_sum: FixedWrapper = FixedWrapper::from(ReferrerWeight::get())
            + FixedWrapper::from(XorBurnedWeight::get())
            + FixedWrapper::from(XorIntoValBurnedWeight::get());
        let referrer_weight: FixedWrapper = ReferrerWeight::get().into();
        let initial_balance: FixedWrapper = INITIAL_BALANCE.into();
        let expected_referrer_balance: FixedWrapper =
            mock_weight * referrer_weight / weights_sum + initial_balance;
        assert!(
            Balances::free_balance(REFERRER_ACCOUNT)
                >= (expected_referrer_balance.clone() - fixed_wrapper!(1))
                    .get()
                    .unwrap()
                    .into()
                && Balances::free_balance(REFERRER_ACCOUNT)
                    <= (expected_referrer_balance + fixed_wrapper!(1))
                        .get()
                        .unwrap()
                        .into()
        );
    });
}

#[test]
fn notify_val_burned_works() {
    ExtBuilder::build().execute_with(|| {
        assert_eq!(
            pallet_staking::Module::<Test>::era_val_burned(),
            0_u128.into()
        );
        let call: &<Test as frame_system::Trait>::Call =
            &Call::Balances(BalancesCall::transfer(TO_ACCOUNT, TRANSFER_AMOUNT.into()));

        let len = 10;
        let pre = ChargeTransactionPayment::<Test>::from(0_u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &info_from_weight(MOCK_WEIGHT), len)
            .unwrap();
        assert!(ChargeTransactionPayment::<Test>::post_dispatch(
            pre,
            &info_from_weight(MOCK_WEIGHT),
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        let weights_sum: FixedWrapper =
            (ReferrerWeight::get() + XorBurnedWeight::get() + XorIntoValBurnedWeight::get()).into();
        let x: FixedWrapper =
            (MOCK_WEIGHT as u128 * XorIntoValBurnedWeight::get() as u128 / weights_sum).into();
        let y: FixedWrapper = INITIAL_RESERVES.into();
        let expected_val_burned = (x.clone() * y.clone() / (x + y)).get().unwrap();
        assert_eq!(
            pallet_staking::Module::<Test>::era_val_burned(),
            expected_val_burned.into()
        );
    });
}
