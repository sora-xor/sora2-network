#[macro_export]
macro_rules! fixed {
    ($int:literal,$dec:literal %) => {{
        type Inner = <$crate::Fixed as sp_arithmetic::FixedPointNumber>::Inner;
        // Here we are converting the number to a string to save potential underscores ('_').
        const DEC_STR: &str = stringify!($dec);
        // denominator == 10^(digits_count(dec) + 2)
        const DENOMINATOR: Inner = $crate::utils::pow(10, $crate::utils::number_str_order(DEC_STR) + 2);
        let dec: Inner = $dec;
        fixed!($int %) + $crate::Fixed::from((dec, DENOMINATOR))
    }};
    ($int:literal,$dec:literal) => {{
        type Inner = <$crate::Fixed as sp_arithmetic::FixedPointNumber>::Inner;
        // Here we are converting the number to a string to save potential underscores ('_').
        const DEC_STR: &str = stringify!($dec);
        // denominator == 10^(digits_count(dec))
        const DENOMINATOR: Inner = $crate::utils::pow(10, $crate::utils::number_str_order(DEC_STR));
        let dec: Inner = $dec;
        let int: Inner = $int;
        $crate::Fixed::from(int) + $crate::Fixed::from((dec, DENOMINATOR))
    }};
    ($percent:literal %) => {
        $crate::Fixed::from(sp_arithmetic::Percent::from_parts($percent))
    };
    ($n:literal / $d:literal) => {{
        type Inner = <$crate::Fixed as sp_arithmetic::FixedPointNumber>::Inner;
        let n: Inner = $n;
        let d: Inner = $d;
        $crate::Fixed::from((n, d))
    }};
    ($n:literal e+ $e:literal) => {{
        type Inner = <$crate::Fixed as sp_arithmetic::FixedPointNumber>::Inner;
        const VAL: Inner = $n * $crate::utils::pow(10, $e);
        $crate::Fixed::from(VAL)
    }};
    ($n:literal e- $e:literal) => {{
        type Inner = <$crate::Fixed as sp_arithmetic::FixedPointNumber>::Inner;
        const DENOMINATOR: Inner = $crate::utils::pow(10, $e);
        $crate::Fixed::from(($n, DENOMINATOR))
    }};
    ($n:literal) => {{
        type Inner = <$crate::Fixed as sp_arithmetic::FixedPointNumber>::Inner;
        let n: Inner = $n;
        $crate::Fixed::from(n)
    }};
}

#[cfg(test)]
mod tests {
    use crate::Fixed;
    use sp_arithmetic::Percent;
    use sp_runtime::FixedPointNumber;

    #[test]
    #[rustfmt::skip]
    fn should_calculate_formula() {
        assert_eq!(fixed!(1), Fixed::from(1));
        assert_eq!(fixed!(1/2), Fixed::saturating_from_rational(1, 2));
        assert_eq!(fixed!(10%), Fixed::from(Percent::from_parts(10)));
        assert_eq!(fixed!(1,2), Fixed::saturating_from_rational(1_2, 1_0));
        assert_eq!(fixed!(010,0_90), Fixed::saturating_from_rational(10_09, 1_00));
        assert_eq!(fixed!(2,5%), Fixed::saturating_from_rational(2_5, 1_0_00));
        assert_eq!(fixed!(020,0_50%), Fixed::saturating_from_rational(20_05, 1_00_00));
        assert_eq!(fixed!(1 e+2), Fixed::from(100));
        assert_eq!(fixed!(100 e-2), Fixed::from(1));
    }
}
