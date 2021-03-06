use common::{balance, prelude::FixedWrapper};
use common::{fixed_wrapper, Fixed};
use core::convert::TryInto;
use pallet_balances::Call as BalancesCall;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

use crate::mock::*;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let call: &<Test as frame_system::Trait>::Call = &Call::Balances(BalancesCall::transfer(
            TO_ACCOUNT,
            TRANSFER_AMOUNT as u128 * balance!(1),
        ));

        let len = 10;
        let pre = ChargeTransactionPayment::<Test>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &info_from_weight(MOCK_WEIGHT), len)
            .unwrap();
        let mock_weight = FixedWrapper::from(MOCK_WEIGHT as u128);
        let balance_without_mock = (FixedWrapper::from(initial_balance()) - mock_weight.clone())
            .get()
            .unwrap();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            balance_without_mock.into_bits().try_into().unwrap()
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
            balance_without_mock.into_bits().try_into().unwrap()
        );
        let weights_sum: FixedWrapper =
            FixedWrapper::from(ReferrerWeight::get() as u128 * balance!(1))
                + FixedWrapper::from(XorBurnedWeight::get() as u128 * balance!(1))
                + FixedWrapper::from(XorIntoValBurnedWeight::get() as u128 * balance!(1));
        let referrer_weight = FixedWrapper::from(ReferrerWeight::get() as u128 * balance!(1));
        let initial_balance = FixedWrapper::from(initial_balance());
        let expected_referrer_balance =
            mock_weight * referrer_weight / weights_sum + initial_balance;
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
            pallet_staking::Module::<Test>::era_val_burned(),
            0_u128.into()
        );
        let call: &<Test as frame_system::Trait>::Call = &Call::Balances(BalancesCall::transfer(
            TO_ACCOUNT,
            TRANSFER_AMOUNT as u128 * balance!(1),
        ));

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
        let weights_sum =
            (ReferrerWeight::get() + XorBurnedWeight::get() + XorIntoValBurnedWeight::get()) as u64;
        let x = (MOCK_WEIGHT * XorIntoValBurnedWeight::get() as u64 / weights_sum) as u128;
        let x = FixedWrapper::from(x);
        let y: FixedWrapper = Fixed::from_bits(initial_reserves() as i128).into();
        let expected_val_burned = x.clone() * y.clone() / (x + y);
        assert_eq!(
            pallet_staking::Module::<Test>::era_val_burned(),
            expected_val_burned.into_balance()
        );
    });
}
