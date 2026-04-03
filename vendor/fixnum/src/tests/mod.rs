#[cfg(feature = "std")]
use core::f64;
use core::i64;

use anyhow::Result;

use crate::{RoundMode::*, *};

mod macros;

#[test]
fn from_decimal() -> Result<()> {
    test_fixed_point! {
        case (numerator | Layout, denominator | i32, expected | FixedPoint) => {
            assert_eq!(FixedPoint::from_decimal(numerator, denominator)?, expected);
        },
        all {
            (5_000_000_000, -9, fp!(5));
            (1, 0, fp!(1));
            (1, 1, fp!(10));
        },
        fp128 {
            (5_000_000_000_000_000_000, -18, fp!(5));
        },
    };
    Ok(())
}

#[test]
#[cfg(feature = "std")]
fn display() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, expected | &str) => {
            assert_eq!(format!("{}", x), String::from(expected));

            #[cfg(feature = "serde")]
            assert_eq!(serde_json::to_string(&x).unwrap(), format!("\"{}\"", expected));
        },
        all {
            (fp!(0), "0.0");
            (fp!(10.042), "10.042");
            (fp!(-10.042), "-10.042");
            (fp!(0.000000001), "0.000000001");
            (fp!(-0.000000001), "-0.000000001");
            (fp!(9223372036.854775807), "9223372036.854775807");
            (fp!(-9223372036.854775808), "-9223372036.854775808");
        },
        fp128 {
            (fp!(0.000000000000000001), "0.000000000000000001");
            (fp!(-0.000000000000000001), "-0.000000000000000001");
            (fp!(170141183460469231731.687303715884105727), "170141183460469231731.687303715884105727");
            (fp!(-170141183460469231731.687303715884105728), "-170141183460469231731.687303715884105728");
        },
    };
    Ok(())
}

#[test]
#[allow(overflowing_literals)]
fn from_good_str() -> Result<()> {
    test_fixed_point! {
        case (input | &str, expected | Layout) => {
            let expected = FixedPoint::from_bits(expected);
            let input: FixedPoint = input.parse()?;
            assert_eq!(input, expected);

            #[cfg(all(feature = "std", feature = "serde"))]
            assert_eq!(
                serde_json::from_str::<FixedPoint>(&format!("\"{}\"", input)).unwrap(),
                expected
            );
        },
        fp64 {
            ("1", 1000000000);
            ("1.1", 1100000000);
            ("1.02", 1020000000);
            ("-1.02", -1020000000);
            ("+1.02", 1020000000);
            ("0.1234", 123400000);
            ("-0.1234", -123400000);
            ("123456789.123456789", 123456789123456789);
            ("9223372036.854775807", 9223372036854775807);
            ("-9223372036.854775808", -9223372036854775808);
        },
        fp128 {
            ("1", 1000000000000000000);
            ("1.1", 1100000000000000000);
            ("1.02", 1020000000000000000);
            ("-1.02", -1020000000000000000);
            ("+1.02", 1020000000000000000);
            ("0.1234", 123400000000000000);
            ("-0.1234", -123400000000000000);
            ("123456789.123456789", 123456789123456789000000000);
            ("9223372036.854775807", 9223372036854775807000000000);
            ("-9223372036.854775808", -9223372036854775808000000000);
            ("170141183460469231731.687303715884105727",
             170141183460469231731687303715884105727);
            ("-170141183460469231731.687303715884105728",
             -170141183460469231731687303715884105728);
        },
    };
    Ok(())
}

#[test]
fn from_bad_str() -> Result<()> {
    test_fixed_point! {
        case (bad_str | &str) => {
            let result: Result<FixedPoint, ConvertError> = bad_str.parse();
            assert!(result.is_err(), "must not parse '{}'", bad_str);

            #[cfg(all(feature = "std", feature = "serde"))]
            assert!(serde_json::from_str::<FixedPoint>(&format!("\"{}\"", bad_str)).is_err());
        },
        all {
            ("");
            ("7.02e5");
            ("a.12");
            ("12.a");
            ("13.9999999999999999999999999999999999999999999999999999999999999");
            ("100000000000000000000000");
            ("170141183460469231731687303715.884105728");
            ("13.0000000000000000001");
            ("13.1000000000000000001");
            ("9223372036.8547758204856183567");
        },
        fp64 {
            ("13.0000000001");
            ("13.1000000001");
            ("9223372036.854775808");
        },
    };
    Ok(())
}

#[test]
#[cfg(all(feature = "std", feature = "serde"))]
fn serde_with() -> Result<()> {
    test_fixed_point! {
        case (input | f64, expected | FixedPoint) => {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            struct Struct {
                #[serde(with = "crate::serde::as_f64")]
                number: FixedPoint,
            }

            let actual = serde_json::from_str::<Struct>(&format!(r#"{{"number":{}}}"#, input)).unwrap().number;
            assert_eq!(expected, actual);
        },
        all {
            (1., fp!(1.0));
            (1.1, fp!(1.1));
            (1.02, fp!(1.02));
            (-1.02, fp!(-1.02));
            (0.1234, fp!(0.1234));
            (-0.1234, fp!(-0.1234));
        },
    };
    Ok(())
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn exp_and_coef_should_agree() -> Result<()> {
    test_fixed_point! {
        case () => {
            assert!(FixedPoint::PRECISION > 0);
            const TEN: Layout = 10;
            assert_eq!(FixedPoint::COEF, TEN.pow(FixedPoint::PRECISION as u32));
        },
    };
    Ok(())
}

#[test]
fn cmul_overflow() -> Result<()> {
    test_fixed_point! {
        case () => {
            let result = FixedPoint::MAX.cmul(Layout::MAX);
            assert_eq!(result, Err(ArithmeticError::Overflow));

            let result = FixedPoint::MAX.cmul(Layout::MIN);
            assert_eq!(result, Err(ArithmeticError::Overflow));
        },
    };
    Ok(())
}

#[test]
fn rmul_exact() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            // Check the result
            assert_eq!(a.rmul(b, Floor)?, expected);
            // Check the commutative property
            assert_eq!(b.rmul(a, Floor)?, expected);
            // Check that round mode doesn't matter
            assert_eq!(a.rmul(b, Ceil)?, expected);
            assert_eq!(b.rmul(a, Ceil)?, expected);
        },
        all {
            (fp!(525), fp!(10), fp!(5250));
            (fp!(-525), fp!(10), fp!(-5250));
            (fp!(-525), fp!(-10), fp!(5250));
            (fp!(525), fp!(-10), fp!(-5250));
            (fp!(525), fp!(0.0001), fp!(0.0525));
            (fp!(-525), fp!(0.0001), fp!(-0.0525));
            (fp!(-525), fp!(-0.0001), fp!(0.0525));
            (FixedPoint::MAX, FixedPoint::ONE, FixedPoint::MAX);
            (FixedPoint::MIN, FixedPoint::ONE, FixedPoint::MIN);
            (FixedPoint::ONE, fp!(0.000000001), fp!(0.000000001));
            (fp!(-1), fp!(-0.000000001), fp!(0.000000001));
            (
                FixedPoint::from_bits(Layout::MAX / 10 * 10),
                fp!(0.1),
                FixedPoint::from_bits(Layout::MAX / 10),
            );
            (
                FixedPoint::from_bits(Layout::MIN / 10 * 10),
                fp!(0.1),
                FixedPoint::from_bits(Layout::MIN / 10),
            );
        },
        fp128 {
            (fp!(13043817825.332782), fp!(13043817825.332782), fp!(170141183460469226191.989043859524));
        },
    };
    Ok(())
}

#[test]
fn rmul_round() -> Result<()> {
    test_fixed_point! {
        case (
            a | FixedPoint,
            b | FixedPoint,
            expected_floor | FixedPoint,
            expected_ceil | FixedPoint,
        ) => {
            // Check the result
            assert_eq!(a.rmul(b, Floor)?, expected_floor);
            assert_eq!(a.rmul(b, Ceil)?, expected_ceil);
            // Check the commutative property
            assert_eq!(b.rmul(a, Floor)?, expected_floor);
            assert_eq!(b.rmul(a, Ceil)?, expected_ceil);
            // Arguments' negation doesn't change the result
            assert_eq!(b.cneg()?.rmul(a.cneg()?, Floor)?, expected_floor);
            assert_eq!(b.cneg()?.rmul(a.cneg()?, Ceil)?, expected_ceil);
        },
        fp64 {
            (fp!(0.1), fp!(0.000000001), fp!(0), fp!(0.000000001));
            (fp!(-0.1), fp!(0.000000001), fp!(-0.000000001), fp!(0));
            (fp!(0.000000001), fp!(0.000000001), fp!(0), fp!(0.000000001));
            (fp!(-0.000000001), fp!(0.000000001), fp!(-0.000000001), fp!(0));
        },
        fp128 {
            (fp!(0.1), fp!(0.000000000000000001), FixedPoint::ZERO, fp!(0.000000000000000001));
            (fp!(-0.1), fp!(0.000000000000000001), fp!(-0.000000000000000001), FixedPoint::ZERO);
            (fp!(0.000000000000000001), fp!(0.000000000000000001), FixedPoint::ZERO, fp!(0.000000000000000001));
            (fp!(-0.000000000000000001), fp!(0.000000000000000001), fp!(-0.000000000000000001), FixedPoint::ZERO);
        },
    };
    Ok(())
}

#[test]
fn rmul_overflow() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint) => {
            assert_eq!(a.rmul(b, Ceil), Err(ArithmeticError::Overflow));
        },
        all {
            (FixedPoint::MAX, fp!(1.000000001));
        },
        fp64 {
            (fp!(96038.388349945), fp!(96038.388349945));
            (fp!(-97000), fp!(96100))
        },
        fp128 {
            (FixedPoint::MAX, fp!(1.000000000000000001));
            (fp!(13043817825.332783), fp!(13043817825.332783));
            (fp!(-13043817826), fp!(13043817826))
        },
    };
    Ok(())
}

#[test]
fn lossless_mul_round() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
    macro_rules! fp {
        ($val:literal) => {{
            let value: FixedPoint = stringify!($val).parse()?;
            value
        }};
    }
    let cases = [
        (fp!(0.1), fp!(0.000000001), None),
        (fp!(-0.1), fp!(0.000000001), None),
        (fp!(0.000000001), fp!(0.000000001), None),
        (fp!(-0.000000001), fp!(0.000000001), None),
        (fp!(5.1), fp!(0.000000001), None),
        (fp!(-5.1), fp!(0.000000001), None),
        (fp!(5), fp!(2), Some(fp!(10))),
        (fp!(0.5), fp!(0.000000002), Some(fp!(0.000000001))),
        (fp!(-0.5), fp!(0.000000002), Some(fp!(-0.000000001))),
    ];
    for (a, b, c) in cases {
        assert_eq!(a.lossless_mul(b)?, c);
        assert_eq!(b.lossless_mul(a)?, c);
        assert_eq!(b.cneg()?.lossless_mul(a.cneg()?)?, c);
    }
    Ok(())
}

#[test]
fn lossless_mul_overflow() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint) => {
            assert_eq!(a.lossless_mul(b), Err(ArithmeticError::Overflow));
        },
        all {
            (FixedPoint::MAX, fp!(1.000000001));
        },
        fp64 {
            (fp!(96038.388349945), fp!(96038.388349945));
            (fp!(-97000), fp!(96100))
        },
        fp128 {
            (FixedPoint::MAX, fp!(1.000000000000000001));
            (fp!(13043817825.332783), fp!(13043817825.332783));
            (fp!(-13043817826), fp!(13043817826))
        },
    };
    Ok(())
}

#[test]
fn rdiv_exact() -> Result<()> {
    test_fixed_point! {
        case (numerator | FixedPoint, denominator | FixedPoint, expected | FixedPoint) => {
            assert_eq!(numerator.rdiv(denominator, Ceil)?, expected);
            assert_eq!(numerator.rdiv(denominator, Floor)?, expected);
        },
        all {
            (FixedPoint::MAX, FixedPoint::MAX, FixedPoint::ONE);
            (fp!(5), fp!(2), fp!(2.5));
            (fp!(-5), fp!(2), fp!(-2.5));
            (fp!(5), fp!(-2), fp!(-2.5));
            (fp!(-5), fp!(-2), fp!(2.5));
            (fp!(5), fp!(0.2), fp!(25));
            (fp!(0.00000001), fp!(10), fp!(0.000000001));
            (fp!(0.000000001), fp!(0.1), fp!(0.00000001));
        },
        fp128 {
            (fp!(0.00000000000000001), fp!(10), fp!(0.000000000000000001));
            (fp!(0.000000000000000001), fp!(0.1), fp!(0.00000000000000001));
        },
    };
    Ok(())
}

#[test]
fn lossless_div_exact() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
    macro_rules! fp {
        ($val:literal) => {{
            let value: FixedPoint = stringify!($val).parse()?;
            value
        }};
    }
    let cases = [
        (FixedPoint::MAX, FixedPoint::MAX, FixedPoint::ONE),
        (fp!(5), fp!(2), fp!(2.5)),
        (fp!(-5), fp!(2), fp!(-2.5)),
        (fp!(5), fp!(-2), fp!(-2.5)),
        (fp!(-5), fp!(-2), fp!(2.5)),
        (fp!(5), fp!(0.2), fp!(25)),
        (fp!(0.00000001), fp!(10), fp!(0.000000001)),
        (fp!(0.000000001), fp!(0.1), fp!(0.00000001)),
    ];
    for (a, b, c) in cases {
        assert_eq!(a.lossless_div(b)?, Some(c));
    }
    Ok(())
}

#[test]
fn rdiv_by_layout() -> Result<()> {
    test_fixed_point! {
        case (
            a | FixedPoint,
            b | Layout,
            expected_floor | FixedPoint,
            expected_ceil | FixedPoint,
        ) => {
            assert_eq!(a.rdiv(b, Floor)?, expected_floor);
            assert_eq!(a.rdiv(b, Ceil)?, expected_ceil);
        },
        all {
            (fp!(2.4), 2, fp!(1.2), fp!(1.2));
            (fp!(0), 5, FixedPoint::ZERO, FixedPoint::ZERO);
        },
        fp64 {
            (fp!(7), 3, fp!(2.333333333), fp!(2.333333334));
            (fp!(-7), 3, fp!(-2.333333334), fp!(-2.333333333));
            (fp!(-7), -3, fp!(2.333333333), fp!(2.333333334));
            (fp!(7), -3, fp!(-2.333333334), fp!(-2.333333333));
            (fp!(0.000000003), 2, fp!(0.000000001), fp!(0.000000002));
            (fp!(0.000000003), 7, fp!(0), fp!(0.000000001));
            (fp!(0.000000001), 7, fp!(0), fp!(0.000000001));
        },
        fp128 {
            (fp!(7), 3, fp!(2.333333333333333333), fp!(2.333333333333333334));
            (fp!(-7), 3, fp!(-2.333333333333333334), fp!(-2.333333333333333333));
            (fp!(-7), -3, fp!(2.333333333333333333), fp!(2.333333333333333334));
            (fp!(7), -3, fp!(-2.333333333333333334), fp!(-2.333333333333333333));
            (fp!(0.000000000000000003), 2, fp!(0.000000000000000001), fp!(0.000000000000000002));
            (fp!(0.000000000000000003), 7, fp!(0), fp!(0.000000000000000001));
            (fp!(0.000000000000000001), 7, fp!(0), fp!(0.000000000000000001));
        },
    };
    Ok(())
}

#[test]
fn lossless_div_by_layout() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
    macro_rules! fp {
        ($val:literal) => {{
            let value: FixedPoint = stringify!($val).parse()?;
            value
        }};
    }
    let cases = [
        (fp!(2.4), 2, Some(fp!(1.2))),
        (fp!(0), 5, Some(FixedPoint::ZERO)),
        (fp!(7), 3, None),
        (fp!(-7), 3, None),
        (fp!(-7), -3, None),
        (fp!(7), -3, None),
        (fp!(0.000000003), 2, None),
        (fp!(0.000000003), 7, None),
        (fp!(0.000000001), 7, None),
    ];
    for (a, b, c) in cases {
        assert_eq!(a.lossless_div(b)?, c);
    }
    Ok(())
}

#[test]
fn rdiv_round() -> Result<()> {
    test_fixed_point! {
        case (
            numerator | FixedPoint,
            denominator | FixedPoint,
            expected_ceil | FixedPoint,
            expected_floor | FixedPoint,
        ) => {
            assert_eq!(numerator.rdiv(denominator, Ceil)?, expected_ceil);
            assert_eq!(numerator.rdiv(denominator, Floor)?, expected_floor);
        },
        fp64 {
            (fp!(100), fp!(3), fp!(33.333333334), fp!(33.333333333));
            (fp!(-100), fp!(-3), fp!(33.333333334), fp!(33.333333333));
            (fp!(-100), fp!(3), fp!(-33.333333333), fp!(-33.333333334));
            (fp!(100), fp!(-3), fp!(-33.333333333), fp!(-33.333333334));
        },
        fp128 {
            (fp!(100), fp!(3), fp!(33.333333333333333334), fp!(33.333333333333333333));
            (fp!(-100), fp!(-3), fp!(33.333333333333333334), fp!(33.333333333333333333));
            (fp!(-100), fp!(3), fp!(-33.333333333333333333), fp!(-33.333333333333333334));
            (fp!(100), fp!(-3), fp!(-33.333333333333333333), fp!(-33.333333333333333334));
        },
    };
    Ok(())
}

#[test]
fn lossless_div_round() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
    macro_rules! fp {
        ($val:literal) => {{
            let value: FixedPoint = stringify!($val).parse()?;
            value
        }};
    }
    let cases = [
        (fp!(100), fp!(3), None),
        (fp!(-100), fp!(-3), None),
        (fp!(-100), fp!(3), None),
        (fp!(100), fp!(-3), None),
    ];
    for (a, b, c) in cases {
        assert_eq!(a.lossless_div(b)?, c);
    }
    Ok(())
}

#[test]
fn rdiv_layout() -> Result<()> {
    test_fixed_point! {
        case (
            a | Layout,
            b | Layout,
            expected_floor | Layout,
            expected_ceil | Layout,
        ) => {
            assert_eq!(a.rdiv(b, Floor)?, expected_floor);
            assert_eq!(a.rdiv(b, Ceil)?, expected_ceil);
            assert_eq!(a.rdiv(-b, Floor)?, -expected_ceil);
            assert_eq!((-a).rdiv(b, Floor)?, -expected_ceil);
            assert_eq!(a.rdiv(-b, Ceil)?, -expected_floor);
            assert_eq!((-a).rdiv(b, Ceil)?, -expected_floor);
            assert_eq!((-a).rdiv(-b, Floor)?, expected_floor);
            assert_eq!((-a).rdiv(-b, Ceil)?, expected_ceil);
        },
        all {
            (5, 2, 2, 3);
            (0, 5, 0, 0);
        },
    };
    Ok(())
}

#[test]
fn lossless_div_layout() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    let cases: [(Layout, Layout, Option<Layout>); 2] = [(5, 2, None), (0, 5, Some(0))];
    for (a, b, c) in cases {
        assert_eq!(a.lossless_div(b)?, c);
    }
    Ok(())
}

#[test]
fn rdiv_division_by_zero() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint) => {
            let expected = Err(ArithmeticError::DivisionByZero);
            assert_eq!(x.rdiv(FixedPoint::ZERO, Floor), expected);
            assert_eq!(x.rdiv(FixedPoint::ZERO, Ceil), expected);
        },
        all {
            (fp!(0));
            (fp!(1));
            (fp!(-1));
            (FixedPoint::MAX);
            (FixedPoint::MIN);
        },
    };
    Ok(())
}

#[test]
fn lossless_div_by_zero() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
    macro_rules! fp {
        ($val:literal) => {{
            let value: FixedPoint = stringify!($val).parse()?;
            value
        }};
    }
    let cases = [
        (fp!(0)),
        (fp!(1)),
        (fp!(-1)),
        (FixedPoint::MAX),
        (FixedPoint::MIN),
    ];
    for x in cases {
        assert_eq!(
            x.lossless_div(FixedPoint::ZERO),
            Err(ArithmeticError::DivisionByZero)
        );
    }
    Ok(())
}

#[test]
fn rdiv_overflow() -> Result<()> {
    test_fixed_point! {
        case (denominator | FixedPoint) => {
            assert_eq!(
                FixedPoint::MAX.rdiv(denominator, Ceil),
                Err(ArithmeticError::Overflow)
            );
        },
        all {
            (fp!(0.999999999));
        },
        fp128 {
            (fp!(0.999999999999999999));
        },
    };
    Ok(())
}

#[test]
fn lossless_div_overflow() -> Result<()> {
    // apparently the `test_fixed_point` macros does not work
    // for non-fp expected value (some weird errors were reported)
    type Layout = i64;

    type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
    macro_rules! fp {
        ($val:literal) => {{
            let value: FixedPoint = stringify!($val).parse()?;
            value
        }};
    }
    assert_eq!(
        FixedPoint::MAX.lossless_div(fp!(0.999999999)),
        Err(ArithmeticError::Overflow)
    );
    Ok(())
}

#[test]
fn float_mul() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(a.rmul(b, Ceil)?, expected);
        },
        all {
            (fp!(525), fp!(10), fp!(5250));
            (fp!(525), fp!(0.0001), fp!(0.0525));
            (FixedPoint::MAX, FixedPoint::ONE, FixedPoint::MAX);
            (
                FixedPoint::from_bits(Layout::MAX / 10 * 10),
                fp!(0.1),
                FixedPoint::from_bits(Layout::MAX / 10),
            );
        },
    };
    Ok(())
}

#[test]
fn float_mul_overflow() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint) => {
            assert!(a.rmul(b, Ceil).is_err());
        },
        fp64 {
            (fp!(140000), fp!(140000));
            (fp!(-140000), fp!(140000));
        },
        fp128 {
            (fp!(13043817826), fp!(13043817825));
            (fp!(-13043817826), fp!(13043817825));
        },
    };
    Ok(())
}

#[test]
fn half_sum_exact() -> Result<()> {
    test_fixed_point! {
        case (expected | FixedPoint) => {
            assert_eq!(FixedPoint::half_sum(expected, expected, Floor), expected);
            assert_eq!(FixedPoint::half_sum(expected, expected, Ceil), expected);
        },
        all {
            (fp!(0));
            (fp!(1));
            (fp!(-1));
            (FixedPoint::MAX);
            (FixedPoint::MIN);
        },
    };
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(FixedPoint::half_sum(a, b, Floor), expected);
            assert_eq!(FixedPoint::half_sum(b, a, Floor), expected);
            assert_eq!(FixedPoint::half_sum(a, b, Ceil), expected);
            assert_eq!(FixedPoint::half_sum(b, a, Ceil), expected);
        },
        all {
            (fp!(1), fp!(3), fp!(2));
            (fp!(1), fp!(2), fp!(1.5));
            (fp!(7.123456789), fp!(7.123456783), fp!(7.123456786));
            (fp!(9000), fp!(9050), fp!(9025));
            (fp!(9000), fp!(-9000), fp!(0));
            (fp!(9000000000), fp!(9000000002), fp!(9000000001));
            (
                fp!(9000000000.000000001),
                fp!(-9000000000.000000005),
                fp!(-0.000000002),
            );
            (FixedPoint::MAX, FixedPoint::MIN.cadd(FixedPoint::EPSILON)?, fp!(0));
        },
        fp128 {
            (fp!(7.123456789123456789), fp!(7.123456789123456783), fp!(7.123456789123456786));
        },
    };
    Ok(())
}

#[test]
fn half_sum_rounded() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected_floor | FixedPoint, expected_ceil | FixedPoint) => {
            assert_eq!(FixedPoint::half_sum(a, b, Floor), expected_floor);
            assert_eq!(FixedPoint::half_sum(b, a, Floor), expected_floor);
            assert_eq!(FixedPoint::half_sum(a, b, Ceil), expected_ceil);
            assert_eq!(FixedPoint::half_sum(b, a, Ceil), expected_ceil);
        },
        all {
            (FixedPoint::MIN, FixedPoint::MAX, FixedPoint::EPSILON.cneg()?, fp!(0));
        },
        fp64 {
            (fp!(9000000000.000000394), fp!(9000000001.000000397), fp!(9000000000.500000395), fp!(9000000000.500000396));
            (
                fp!(9000000000.000000001),
                fp!(-9000000000.000000006),
                fp!(-0.000000003),
                fp!(-0.000000002),
            );
            (fp!(7.123456789), fp!(7.123456788), fp!(7.123456788), fp!(7.123456789));
        },
        fp128 {
            (fp!(7.123456789123456789), fp!(7.123456789123456788), fp!(7.123456789123456788), fp!(7.123456789123456789));
        },
    };
    Ok(())
}

#[test]
fn integral() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, expected_floor | Layout, expected_ceil | Layout) => {
            assert_eq!(a.integral(Floor), expected_floor);
            assert_eq!(a.integral(Ceil), expected_ceil);
        },
        all {
            (FixedPoint::ZERO, 0, 0);
            (fp!(0.0001), 0, 1);
            (fp!(-0.0001), -1, 0);
            (fp!(2.0001), 2, 3);
            (fp!(-2.0001), -3, -2);
        },
    };
    Ok(())
}

#[test]
fn round_towards_zero_by() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, rounder | FixedPoint, expected | FixedPoint) => {
            assert_eq!(x.round_towards_zero_by(rounder), expected);
            assert_eq!(x.cneg()?.round_towards_zero_by(rounder), expected.cneg()?);
        },
        all {
            (fp!(1234.56789), fp!(100), fp!(1200));
            (fp!(1234.56789), fp!(10), fp!(1230));
            (fp!(1234.56789), fp!(1), fp!(1234));
            (fp!(1234.56789), fp!(0.1), fp!(1234.5));
            (fp!(1234.56789), fp!(0.01), fp!(1234.56));
            (fp!(1234.56789), fp!(0.001), fp!(1234.567));
            (fp!(1234.56789), fp!(0.0001), fp!(1234.5678));
            (fp!(1234.56789), fp!(0.00001), fp!(1234.56789));
        },
        fp128 {
            (fp!(1234.56789123456789), fp!(0.0000000000001), fp!(1234.5678912345678));
            (fp!(1234.56789123456789), fp!(0.00000000000001), fp!(1234.56789123456789));
        },
    };
    Ok(())
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn next_power_of_ten() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, expected | FixedPoint) => {
            assert_eq!(x.next_power_of_ten()?, expected);
            assert_eq!(x.cneg()?.next_power_of_ten()?, expected.cneg()?);
        },
        all {
            (fp!(0.000000001), fp!(0.000000001));
            (fp!(0.000000002), fp!(0.00000001));
            (fp!(0.000000009), fp!(0.00000001));
            (fp!(0.0000001), fp!(0.0000001));
            (fp!(0.0000002), fp!(0.000001));
            (fp!(0.1), fp!(0.1));
            (fp!(0.100000001), fp!(1));
            (fp!(1), fp!(1));
            (fp!(2), fp!(10));
            (fp!(1234567), fp!(10000000));
            (fp!(923372036.654775807), fp!(1000000000));
            (fp!(-0.000000001), fp!(-0.000000001));
            (fp!(-0.000000002), fp!(-0.00000001));
            (fp!(-0.000000009), fp!(-0.00000001));
            (fp!(-0.00000001), fp!(-0.00000001));
            (fp!(-0.00000002), fp!(-0.0000001));
            (fp!(-0.100000001), fp!(-1));
            (fp!(-923372021.854775808), fp!(-1000000000));
        },
        fp128 {
            (fp!(0.000000000000000001), fp!(0.000000000000000001));
            (fp!(0.000000000000000002), fp!(0.00000000000000001));
            (fp!(0.000000000000000009), fp!(0.00000000000000001));
            (fp!(0.00000000000000001), fp!(0.00000000000000001));
            (fp!(0.00000000000000002), fp!(0.0000000000000001));
            (fp!(0.100000000000000001), fp!(1));
            (fp!(1234567891234567), fp!(10000000000000000));
            (fp!(923372036987654321.854775807), fp!(1000000000000000000));
            (fp!(-0.000000000000000001), fp!(-0.000000000000000001));
            (fp!(-0.000000000000000002), fp!(-0.00000000000000001));
            (fp!(-0.000000000000000009), fp!(-0.00000000000000001));
            (fp!(-0.00000000000000001), fp!(-0.00000000000000001));
            (fp!(-0.00000000000000002), fp!(-0.0000000000000001));
            (fp!(-0.100000000000000001), fp!(-1));
            (fp!(-923372036987654321.854775808), fp!(-1000000000000000000));
        },
    };
    test_fixed_point! {
        case (x | FixedPoint, expected | FixedPoint) => {
            assert_eq!(x.next_power_of_ten()?, expected);
        },
        fp64 {
            (fp!(0), fp!(0.000000001));
        },
        fp128 {
            (fp!(0), fp!(0.000000000000000001));
        },
    };
    test_fixed_point! {
        case (x | FixedPoint) => {
            assert_eq!(x.next_power_of_ten(), Err(ArithmeticError::Overflow));
        },
        all {
            (FixedPoint::MAX);
            (FixedPoint::MIN);
        },
        fp64 {
            (fp!(9223372036.654775807));
            (fp!(-9223372036.654775807));
        },
        fp128 {
            (fp!(150000000000000000000.0));
            (fp!(-150000000000000000000.854775807));
        },
    };
    Ok(())
}

#[test]
fn rounding_to_i64() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, expected | i64) => {
            assert_eq!(x.rounding_to_i64(), expected);
        },
        all {
            (fp!(0), 0);
            (fp!(42), 42);
            (fp!(1.4), 1);
            (fp!(1.6), 2);
            (fp!(-1.4), -1);
            (fp!(-1.6), -2);
            (fp!(0.4999), 0);
            (fp!(0.5), 1);
            (fp!(0.5001), 1);
        },
    };
    Ok(())
}

#[test]
#[cfg(feature = "std")]
#[allow(clippy::float_cmp)]
fn to_f64() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, expected | f64) => {
            assert_eq!(f64::from(x), expected);
        },
        all {
            (fp!(0), 0.0);
            (fp!(0.1), 0.1);
            (fp!(1), 1.0);
            (fp!(1.5), 1.5);
            (fp!(-5), -5.);
            (fp!(-14.14), -14.14);
            (fp!(42.123456789), 42.123456789);
            (fp!(-42.123456789), -42.123456789);
            (fp!(8003332421.536753168), 8003332421.536753);
            (fp!(-8003332421.536753168), -8003332421.536753);
            (fp!(9223372036.854775807), 9223372036.854776);
            (fp!(-9223372036.854775807), -9223372036.854776);
            (fp!(922337203.685477581), 922337203.6854776);
            (fp!(-922337203.685477581), -922337203.6854776);
            (fp!(92233720.36854775), 92233720.36854775);
            (fp!(-92233720.36854775), -92233720.36854775);
        },
        fp128 {
            (fp!(0.0000000000025), 25e-13);
            (fp!(1000000.0000000000025), 1e6);
            (fp!(-1000000.0000000000025), -1e6);
            (fp!(0.000000000000000025), 25e-18);
            (fp!(-0.000000000000000025), -25e-18);
            (fp!(2.1234567890123457), 2.1234567890123457);
            (fp!(-2.1234567890123457), -2.1234567890123457);
        },
    };
    Ok(())
}

#[test]
#[cfg(feature = "std")]
#[allow(clippy::float_cmp)]
fn from_f64() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, expected | f64) => {
            assert_eq!(Ok(x), expected.try_into());
            assert_eq!(Ok(x.cneg().unwrap()), (-expected).try_into());
        },
        all {
            (fp!(0), 0.0);
            (fp!(0.5), 0.5);
            (fp!(1), 1.0);
            (fp!(1.5), 1.5);
            (fp!(42.123456789), 42.123456789);

            (fp!(803332.421536753), 803332.421536753);
            (fp!(8033324.21536753), 8033324.21536753);
            (fp!(80333242.1536753), 80333242.1536753);
            (fp!(803332421.536753), 803332421.536753);
            (fp!(8033324215.36753), 8033324215.36753);

            (fp!(9223372036.85477), 9223372036.85477);
        },
        fp128 {
            (fp!(0.803332421536753), 0.803332421536753);
            (fp!(8.03332421536753), 8.03332421536753);
            (fp!(8.03332421536753), 8.03332421536753);
            (fp!(80.3332421536753), 80.3332421536753);
            (fp!(803.332421536753), 803.332421536753);
            (fp!(8033.32421536753), 8033.32421536753);
            (fp!(80333.2421536753), 80333.2421536753);
            // <see part of cases in `all` sections>
            (fp!(80333242153.6753), 80333242153.6753);
            (fp!(803332421536.753), 803332421536.753);
            (fp!(8033324215367.53), 8033324215367.53);
            (fp!(80333242153675.3), 80333242153675.3);
            (fp!(803332421536753), 803332421536753.);
            (fp!(8033324215367530), 8033324215367530.);
            (fp!(8033324215367533), 8033324215367533.);
        },
    };
    Ok(())
}

#[test]
fn saturating_add() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(a.saturating_add(b), expected);
            assert_eq!(b.saturating_add(a), expected);
            assert_eq!(a.cneg()?.saturating_add(b.cneg()?), expected.cneg()?);
        },
        all {
            (fp!(0), fp!(0), fp!(0));
            (fp!(0), fp!(3000.0000006), fp!(3000.0000006));
            (fp!(-1000.0000002), fp!(0), fp!(-1000.0000002));
            (fp!(-1000.0000002), fp!(3000.0000006), fp!(2000.0000004));
            (fp!(-1000.0000002), fp!(-3000.0000006), fp!(-4000.0000008));
            (fp!(4611686018.427387903), fp!(4611686018.427387903), fp!(9223372036.854775806));
        },
        fp128 {
            (fp!(0), fp!(3000000000000.0000000000000006), fp!(3000000000000.0000000000000006));
            (fp!(-1000000000000.0000000000000002), fp!(0), fp!(-1000000000000.0000000000000002));
            (fp!(-1000000000000.0000000000000002), fp!(3000000000000.0000000000000006), fp!(2000000000000.0000000000000004));
            (fp!(-1000000000000.0000000000000002), fp!(-3000000000000.0000000000000006), fp!(-4000000000000.0000000000000008));
            (fp!(4611686018000000000.000000000427387903), fp!(4611686018000000000.000000000427387903), fp!(9223372036000000000.000000000854775806));
        },
    };
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(a.saturating_add(b), expected);
        },
        fp64 {
            (fp!(9222222222), fp!(9222222222), FixedPoint::MAX);
            (fp!(4611686019), fp!(4611686018.427387903), FixedPoint::MAX);
            (fp!(-9222222222), fp!(-9222222222), FixedPoint::MIN);
            (fp!(-4611686019), fp!(-4611686018.427387903), FixedPoint::MIN);
        },
        fp128 {
            (fp!(85550005550005550005), fp!(85550005550005550005), FixedPoint::MAX);
            (fp!(85550005550005550005), fp!(85550005550005550005.000000000427387), FixedPoint::MAX);
            (fp!(-85550005550005550005), fp!(-85550005550005550005), FixedPoint::MIN);
            (fp!(-85550005550005550005), fp!(-85550005550005550005.000000000427387), FixedPoint::MIN);
        },
    };
    Ok(())
}

#[test]
fn saturating_mul() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | Layout, expected | FixedPoint) => {
            assert_eq!(a.saturating_mul(b), expected);
            assert_eq!(CheckedMul::saturating_mul(b, a), expected);
            assert_eq!(a.cneg()?.saturating_mul(b), expected.cneg()?);
            assert_eq!(a.saturating_mul(-b), expected.cneg()?);
            assert_eq!(a.cneg()?.saturating_mul(-b), expected);
        },
        all {
            (fp!(0), 0, fp!(0));
            (fp!(3000.0000006), 0, fp!(0));
            (fp!(3000.0000006), 1, fp!(3000.0000006));
            (fp!(-1000.0000002), 0, fp!(0));
            (fp!(-1000.0000002), 3, fp!(-3000.0000006));
            (fp!(-1000.0000002), -4, fp!(4000.0000008));
            (fp!(68601.48179), -468, fp!(-32105493.47772));
        },
        fp128 {
            (fp!(3000000000000.0000000000000006), 0, FixedPoint::ZERO);
            (fp!(3000000000000.0000000000000006), 1, fp!(3000000000000.0000000000000006));
            (fp!(-1000000000000.0000000000000002), 0, FixedPoint::ZERO);
            (fp!(-1000000000000.0000000000000002), 3, fp!(-3000000000000.0000000000000006));
            (fp!(-1000000000000.0000000000000002), -4, fp!(4000000000000.0000000000000008));
            (fp!(68603957391461.48475635294179), -85204, fp!(-5845331585582084347.18029605227516));
        },
    };
    test_fixed_point! {
        case (a | FixedPoint, b | i128, expected | FixedPoint) => {
            let b = b as Layout;
            assert_eq!(a.saturating_mul(b), expected);
        },
        fp64 {
            (fp!(9222222222), 9222222222, FixedPoint::MAX);
            (fp!(4611686019.427387903), 4611686019, FixedPoint::MAX);
            (fp!(-9222222222), 9222222222, FixedPoint::MIN);
            (fp!(4611686019.427387903), -4611686019, FixedPoint::MIN);
        },
        fp128 {
            (fp!(85550005550005550005), 85550005550005550005, FixedPoint::MAX);
            (fp!(14000444000.427387), 14000444000, FixedPoint::MAX);
            (fp!(-85550005550005550005), 85550005550005550005, FixedPoint::MIN);
            (fp!(14000444000.427387), -14000444000, FixedPoint::MIN);
        },
    };
    Ok(())
}

#[test]
fn saturating_rmul() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(a.saturating_rmul(b, Floor), expected);
            assert_eq!(b.saturating_rmul(a, Floor), expected);
            assert_eq!(a.cneg()?.saturating_rmul(b, Floor), expected.cneg()?);
            assert_eq!(a.saturating_rmul(b.cneg()?, Floor), expected.cneg()?);
            assert_eq!(a.cneg()?.saturating_rmul(b.cneg()?, Floor), expected);
        },
        all {
            (fp!(0), fp!(0), fp!(0));
            (fp!(0), fp!(3000.0000006), fp!(0));
            (fp!(1), fp!(3000.0000006), fp!(3000.0000006));
            (fp!(-1000.0000002), fp!(0), fp!(0));
            (fp!(-1000.0000002), fp!(3), fp!(-3000.0000006));
            (fp!(-1000.0000002), fp!(-4), fp!(4000.0000008));
            (fp!(68601.48179), fp!(-468.28), fp!(-32124701.8926212));
        },
        fp128 {
            (fp!(0), fp!(3000000000000.0000000000000006), fp!(0));
            (fp!(1), fp!(3000000000000.0000000000000006), fp!(3000000000000.0000000000000006));
            (fp!(-1000000000000.0000000000000002), fp!(0), fp!(0));
            (fp!(-1000000000000.0000000000000002), fp!(3), fp!(-3000000000000.0000000000000006));
            (fp!(-1000000000000.0000000000000002), fp!(-4), fp!(4000000000000.0000000000000008));
        },
    };
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, mode | RoundMode, expected | FixedPoint) => {
            assert_eq!(a.saturating_rmul(b, mode), expected);
        },
        fp64 {
            (fp!(0.000000001), fp!(-0.1), Floor, fp!(-0.000000001));
            (fp!(0.000000001), fp!(0.1), Ceil, fp!(0.000000001));
            (fp!(0.000000001), fp!(0.1), Floor, fp!(0));
            (fp!(-0.000000001), fp!(0.1), Ceil, fp!(0));
            (fp!(9222222222), fp!(9222222222), Floor, FixedPoint::MAX);
            (fp!(4611686019), fp!(4611686018.427387903), Floor, FixedPoint::MAX);
            (fp!(-9222222222), fp!(9222222222), Floor, FixedPoint::MIN);
            (fp!(4611686019), fp!(-4611686018.427387903), Floor, FixedPoint::MIN);
        },
        fp128 {
            (fp!(0.000000000000000001), fp!(0.1), Floor, fp!(0));
            (fp!(0.000000000000000001), fp!(-0.1), Floor, fp!(-0.000000000000000001));
            (fp!(0.000000000000000001), fp!(0.1), Ceil, fp!(0.000000000000000001));
            (fp!(-0.000000000000000001), fp!(0.1), Ceil, fp!(0));
            (fp!(85550005550005550005), fp!(85550005550005550005), Floor, FixedPoint::MAX);
            (fp!(4611686019), fp!(4611686018000000000.000000000427387903), Floor, FixedPoint::MAX);
            (fp!(-85550005550005550005), fp!(85550005550005550005), Floor, FixedPoint::MIN);
            (fp!(4611686019), fp!(-4611686018000000000.000000000427387903), Floor, FixedPoint::MIN);
        },
    };
    Ok(())
}

#[test]
fn saturating_sub() -> Result<()> {
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(a.saturating_sub(b), expected);
            assert_eq!(b.saturating_sub(a), expected.cneg()?);
            assert_eq!(a.cneg()?.saturating_sub(b.cneg()?), expected.cneg()?);
        },
        all {
            (fp!(0), fp!(0), fp!(0));
            (fp!(0), fp!(3000.0000006), fp!(-3000.0000006));
            (fp!(-1000.0000002), fp!(0), fp!(-1000.0000002));
            (fp!(-1000.0000002), fp!(3000.0000006), fp!(-4000.0000008));
            (fp!(-1000.0000002), fp!(-3000.0000006), fp!(2000.0000004));
            (fp!(4611686018.427387903), fp!(-4611686018.427387903), fp!(9223372036.854775806));
        },
        fp128 {
            (fp!(0), fp!(3000000000000.0000000000000006), fp!(-3000000000000.0000000000000006));
            (fp!(-1000000000000.0000000000000002), fp!(0), fp!(-1000000000000.0000000000000002));
            (fp!(-1000000000000.0000000000000002), fp!(3000000000000.0000000000000006), fp!(-4000000000000.0000000000000008));
            (fp!(-1000000000000.0000000000000002), fp!(-3000000000000.0000000000000006), fp!(2000000000000.0000000000000004));
            (fp!(4611686018000000000.000000000427387903), fp!(-4611686018000000000.000000000427387903), fp!(9223372036000000000.000000000854775806));
        },
    };
    test_fixed_point! {
        case (a | FixedPoint, b | FixedPoint, expected | FixedPoint) => {
            assert_eq!(a.saturating_sub(b), expected);
        },
        fp64 {
            (fp!(9222222222), fp!(-9222222222), FixedPoint::MAX);
            (fp!(4611686019), fp!(-4611686018.27387903), FixedPoint::MAX);
            (fp!(-9222222222), fp!(9222222222), FixedPoint::MIN);
            (fp!(-4611686019), fp!(4611686018.47387903), FixedPoint::MIN);
        },
        fp128 {
            (fp!(85550005550005550005), fp!(-85550005550005550005), FixedPoint::MAX);
            (fp!(85550005550005550005), fp!(-85550005550005550005.000000000427387903), FixedPoint::MAX);
            (fp!(-85550005550005550005), fp!(85550005550005550005), FixedPoint::MIN);
            (fp!(-85550005550005550005), fp!(85550005550005550005.000000000427387903), FixedPoint::MIN);
        },
    };
    Ok(())
}

#[test]
fn sqrt_exact() -> Result<()> {
    test_fixed_point! {
        case (expected | FixedPoint) => {
            let square = expected.rmul(expected, Floor)?;
            assert_eq!(expected.rmul(expected, Ceil)?, square);
            assert_eq!(square.rsqrt(Floor)?, expected);
            assert_eq!(square.rsqrt(Ceil)?, expected);
        },
        all {
            (fp!(0));
            (fp!(1));
            (fp!(2));
            (fp!(3));
            (fp!(1000));
            (fp!(96038));
            (FixedPoint::MAX.rsqrt(Floor)?.integral(Floor).try_into()?);
        },
        fp128 {
            (fp!(10431725));
            (fp!(13043817825));
        },
    };
    Ok(())
}

#[test]
fn sqrt_approx() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint, expected_floor | FixedPoint) => {
            assert_eq!(x.rsqrt(Floor)?, expected_floor);
            assert_eq!(x.rsqrt(Ceil)?.inner, expected_floor.inner + 1);
        },
        fp64 {
            (fp!(2), fp!(1.414213562));
            (FixedPoint::MAX, fp!(96038.388349944));
        },
        fp128 {
            (fp!(2), fp!(1.414213562373095048));
            (fp!(3.14159265358979323), fp!(1.772453850905516024));
            (fp!(5), fp!(2.236067977499789696));
            (FixedPoint::MAX, fp!(13043817825.332782212349571806));
        },
    };
    Ok(())
}

#[test]
fn sqrt_negative() -> Result<()> {
    test_fixed_point! {
        case (x | FixedPoint) => {
            let expected = Err(ArithmeticError::DomainViolation);
            assert_eq!(x.rsqrt(Floor), expected);
            assert_eq!(x.rsqrt(Ceil), expected);
        },
        all {
            (fp!(-1));
            (FixedPoint::EPSILON.cneg()?);
            (FixedPoint::MIN);
        },
    };
    Ok(())
}

#[test]
#[ignore]
fn const_fn() {
    let test_cases = trybuild::TestCases::new();
    test_cases.compile_fail(
        "src/tests/const_fn/01_fixnum_const_bad_str_with_too_long_fractional_part.rs",
    );
}
