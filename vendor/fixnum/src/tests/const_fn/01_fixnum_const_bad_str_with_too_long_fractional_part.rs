// Ensure that `fixnum_const` function fails compilation with first argument of incorrect format:
// with a too long fractional part of a number.

use fixnum::{fixnum_const, typenum::U9, FixedPoint};

const VALUE: FixedPoint<i64, U9> = fixnum_const!(0.1234567891, 9);

fn main() {}
