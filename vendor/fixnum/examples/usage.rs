use derive_more::From;
use fixnum::{impl_op, typenum::U9, ArithmeticError, FixedPoint};

type Fp = FixedPoint<i64, U9>;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, From)]
struct Size(i32);
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, From)]
struct Price(Fp);
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, From)]
struct PriceDelta(Fp);
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, From)]
struct Amount(Fp);
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, From)]
struct Ratio(Fp);

impl_op!(Size [cadd] Size = Size);
impl_op!(Size [csub] Size = Size);
impl_op!(Size [rdiv] Size = Ratio);
impl_op!(Size [cmul] Price = Amount);
impl_op!(Price [csub] Price = PriceDelta);
impl_op!(Price [cadd] PriceDelta = Price);
impl_op!(Price [rdiv] Price = Ratio);
impl_op!(Price [rmul] Ratio = Price);
impl_op!(PriceDelta [cadd] PriceDelta = PriceDelta);
impl_op!(Amount [cadd] Amount = Amount);
impl_op!(Amount [csub] Amount = Amount);

macro_rules! fp {
    ($val:literal) => {
        fixnum::fixnum!($val, 9)
    };
}

fn main() -> Result<(), ArithmeticError> {
    use fixnum::ops::*;

    let size = Size(4);
    let price = fp!(4.25);
    let amount = size.cmul(price)?;
    assert_eq!(amount, fp!(17));

    let half = price.rmul(fp!(0.5), RoundMode::Ceil)?;
    assert_eq!(half, fp!(2.125));

    Ok(())
}
