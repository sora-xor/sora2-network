#[macro_export]
macro_rules! fixed {
    ($int:literal,$dec:literal %) => {{
        use fixnum::ops::{RoundingDiv, RoundMode::*};

        fixed!($int, $dec).rdiv(fixed!(100), Floor).unwrap() // TODO(quasiyoke): should be checked compile-time operation
    }};
    ($int:literal,$dec:literal) => {{
        use fixnum::ops::{RoundingDiv, CheckedAdd, RoundMode::*};

        let integer = fixed!($int);
        let fractional = fixed!($dec);
        integer
            .cadd(fractional.rdiv(
                fractional.next_power_of_ten().unwrap(),
                Floor,
                ).unwrap())
            .unwrap() // TODO(quasiyoke): should be checked compile-time operation
    }};
    ($percent:literal %) => {{
        use fixnum::ops::CheckedMul;
        use crate::{Fixed, FixedInner, FIXED_PRECISION, utils::pow};

        let percent = Fixed::from_bits($percent);
        const PERCENT_DECIMAL_PLACES: u32 = 2;
        const COEF: FixedInner = pow(10, FIXED_PRECISION as u32 - PERCENT_DECIMAL_PLACES);
        percent.cmul(COEF).unwrap() // TODO(quasiyoke): should be checked compile-time operation
    }};
    ($n:literal / $d:literal) => {{
        use fixnum::ops::{RoundingDiv, RoundMode::*};

        let nominator = fixed!($n);
        let denominator = fixed!($d);
        nominator.rdiv(denominator, Floor).unwrap() // TODO(quasiyoke): should be checked compile-time operation
    }};
    ($n:literal e+ $e:literal) => {{
        use core::convert::TryFrom;
        use crate::{Fixed, FixedInner, utils::pow};

        const VALUE: FixedInner = $n * pow(10, $e);
        Fixed::try_from(VALUE).unwrap()
    }};
    ($n:literal e- $e:literal) => {{
        use fixnum::ops::{RoundingDiv, RoundMode::*};
        use crate::{Fixed, utils::pow};

        let n = fixed!($n);
        const DENOMINATOR: Fixed = Fixed::from_bits(pow(10, $e));
        n.rdiv(DENOMINATOR, Floor).unwrap() // TODO(quasiyoke): should be checked compile-time operation
    }};
    ($n:literal) => {{
        use core::convert::TryFrom;
        use crate::{Fixed, FixedInner};

        Fixed::try_from(FixedInner::from($n)).unwrap() // TODO(quasiyoke): should be checked compile-time operation
    }};
}

#[allow(unused)]
#[macro_export]
macro_rules! dbg {
    () => {
        debug::info!("[{}]", core::line!());
    };
    ($val:expr) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                debug::info!("[{}] {} = {:#?}",
                    core::line!(), core::stringify!($val), &tmp);
                tmp
            }
        }
    };
    // Trailing comma with single argument is ignored
    ($val:expr,) => { debug::info!($val) };
    ($($val:expr),+ $(,)?) => {
        ($(debug::info!($val)),+,)
    };
}

#[cfg(test)]
mod tests {
    use crate::Fixed;

    fn fp(s: &str) -> Fixed {
        s.parse().unwrap()
    }

    #[test]
    #[rustfmt::skip]
    fn should_calculate_formula() {
        assert_eq!(fixed!(1), fp("1"));
        assert_eq!(fixed!(1/2), fp("0.5"));
        assert_eq!(fixed!(10%), fp("0.1"));
        assert_eq!(fixed!(1,2), fp("1.2"));
        assert_eq!(fixed!(010,0_90), fp("10.09"));
        assert_eq!(fixed!(2,5%), fp("0.025"));
        assert_eq!(fixed!(020,0_50%), fp("0.2005"));
        assert_eq!(fixed!(1 e+2), fp("100"));
        assert_eq!(fixed!(100 e-2), fp("1"));
    }
}
