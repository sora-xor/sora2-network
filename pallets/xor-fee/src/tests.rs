use crate::mock::*;
use common::prelude::FixedWrapper;
use pallet_balances::Call as BalancesCall;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let call: &<Test as frame_system::Trait>::Call =
            &Call::Balances(BalancesCall::transfer(TO_ACCOUNT, TRANSFER_AMOUNT.into()));

        let len = 10;
        let pre = ChargeTransactionPayment::<Test>::from(0u128.into())
            .pre_dispatch(&FROM_ACCOUNT, call, &info_from_weight(MOCK_WEIGHT), len)
            .unwrap();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            (INITIAL_BALANCE - MOCK_WEIGHT).into()
        );
        assert!(ChargeTransactionPayment::<Test>::post_dispatch(
            pre,
            &info_from_weight(100),
            &default_post_info(),
            len,
            &Ok(())
        )
        .is_ok());
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            (INITIAL_BALANCE - MOCK_WEIGHT).into()
        );
        let weights_sum =
            ReferrerWeight::get() + XorBurnedWeight::get() + XorIntoValBurnedWeight::get();
        let expected_referrer_balance =
            INITIAL_BALANCE + MOCK_WEIGHT * ReferrerWeight::get() as u64 / weights_sum as u64;
        assert!(
            Balances::free_balance(REFERRER_ACCOUNT) >= (expected_referrer_balance - 1).into()
                && Balances::free_balance(REFERRER_ACCOUNT)
                    <= (expected_referrer_balance + 1).into()
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
        let weights_sum =
            ReferrerWeight::get() + XorBurnedWeight::get() + XorIntoValBurnedWeight::get();
        let x: FixedWrapper = (MOCK_WEIGHT as u128 * XorIntoValBurnedWeight::get() as u128
            / weights_sum as u128)
            .into();
        let y: FixedWrapper = INITIAL_RESERVES.into();
        let expected_val_burned = (x * y / (x + y)).get().unwrap();
        assert_eq!(
            pallet_staking::Module::<Test>::era_val_burned(),
            expected_val_burned.into()
        );
    });
}
