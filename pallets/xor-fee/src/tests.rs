use crate::mock::*;
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

#[test]
fn referrer_gets_bonus_from_tx_fee() {
    ExtBuilder::build().execute_with(|| {
        let len = 10;
        let pre = ChargeTransactionPayment::<Test>::from(0)
            .pre_dispatch(&FROM_ACCOUNT, CALL, &info_from_weight(MOCK_WEIGHT), len)
            .unwrap();
        assert_eq!(
            Balances::free_balance(FROM_ACCOUNT),
            INITIAL_BALANCE - MOCK_WEIGHT
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
            INITIAL_BALANCE - MOCK_WEIGHT
        );
        let weights_sum =
            ReferrerWeight::get() + XorBurnedWeight::get() + XorIntoValBurnedWieght::get();
        let expected_referrer_balance =
            INITIAL_BALANCE + MOCK_WEIGHT * ReferrerWeight::get() as u64 / weights_sum as u64;
        assert!(
            Balances::free_balance(REFERRER_ACCOUNT) >= expected_referrer_balance - 1
                && Balances::free_balance(REFERRER_ACCOUNT) <= expected_referrer_balance + 1
        );
    });
}
