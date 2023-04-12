mod referrals;
#[cfg(feature = "try-runtime")]
mod remote;
mod xor_fee;

mod tests {
    use crate::{Currencies, Referrals, RuntimeOrigin};
    use assets::GetTotalBalance;
    use common::mock::{alice, bob};
    use common::prelude::constants::SMALL_FEE;
    use common::XOR;
    use frame_support::assert_ok;
    use framenode_chain_spec::ext;

    #[test]
    fn get_total_balance() {
        ext().execute_with(|| {
            assert_ok!(Currencies::update_balance(
                RuntimeOrigin::root(),
                alice(),
                XOR.into(),
                SMALL_FEE as i128
            ));
            Referrals::reserve(RuntimeOrigin::signed(alice()), SMALL_FEE).unwrap();
            assert_eq!(
                crate::GetTotalBalance::total_balance(&XOR, &alice()),
                Ok(SMALL_FEE)
            );

            assert_eq!(crate::GetTotalBalance::total_balance(&XOR, &bob()), Ok(0));
        });
    }
}
