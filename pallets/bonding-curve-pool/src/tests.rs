mod tests {
    use crate::{mock::*, Error};
    use common::prelude::Fixed;

    #[test]
    fn should_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(
                BondingCurvePool::buy_price(XOR).expect("failed to calculate buy price"),
                Fixed::from(100)
            );
            assert_eq!(
                BondingCurvePool::buy_tokens_price(XOR, 100_000)
                    .expect("failed to calculate buy tokens price"),
                Fixed::from(100_10_000)
            );
            assert_eq!(
                BondingCurvePool::sell_price(XOR).expect("failed to calculate sell price"),
                Fixed::from(80)
            );
            assert_eq!(
                BondingCurvePool::sell_tokens_price(XOR, 100_000)
                    .expect("failed to calculate sell tokens price"),
                Fixed::from(80_08_000)
            );
            assert_eq!(
                BondingCurvePool::sell_tokens_price(XOR, 0)
                    .expect("failed to calculate sell tokens price"),
                Fixed::from(0)
            );
        });
    }

    #[test]
    fn should_not_calculate_price() {
        let mut ext = ExtBuilder::default().build();
        ext.execute_with(|| {
            assert_eq!(
                BondingCurvePool::sell_tokens_price(XOR, u128::max_value())
                    .unwrap_err()
                    .as_u8(),
                Error::<Runtime>::CalculatePriceFailed.as_u8()
            );
        });
    }
}
