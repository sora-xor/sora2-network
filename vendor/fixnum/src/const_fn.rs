#[cfg(not(feature = "i128"))]
pub type Int = i64;
#[cfg(feature = "i128")]
pub type Int = i128;

macro_rules! const_assert {
    ($expr:expr) => {
        if !$expr {
            loop {}
        }
    };
}

pub const fn pow10(power: i32) -> Int {
    const POW_10: [Int; 19] = [
        1,
        10,
        100,
        1_000,
        10_000,
        100_000,
        1_000_000,
        10_000_000,
        100_000_000,
        1_000_000_000,
        10_000_000_000,
        100_000_000_000,
        1_000_000_000_000,
        10_000_000_000_000,
        100_000_000_000_000,
        1_000_000_000_000_000,
        10_000_000_000_000_000,
        100_000_000_000_000_000,
        1_000_000_000_000_000_000,
    ];

    if power < POW_10.len() as i32 {
        return POW_10[power as usize];
    }

    let mut result = POW_10[POW_10.len() - 1];
    let mut i = power - POW_10.len() as i32 + 1;

    while i > 0 {
        result *= 10;
        i -= 1;
    }

    result
}

const fn find(bytes: &[u8], pattern: u8) -> Option<usize> {
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == pattern {
            return Some(i);
        }
        i += 1;
    }

    None
}

const fn parse_digit(byte: u8) -> Int {
    let digit = byte.wrapping_sub(48);
    const_assert!(digit < 10);
    digit as _
}

const fn parse_int(bytes: &[u8], start: usize, end: usize) -> Int {
    let mut result: Int = 0;
    let mut i = start;

    while i < end {
        let digit = parse_digit(bytes[i]);
        i += 1;
        result += digit * pow10((end - i) as i32);
    }

    result
}

// TODO: check overflow explicitly.
pub const fn parse_fixed(str: &str, coef: Int) -> Int {
    let bytes = str.as_bytes();
    let signum = if bytes[0] == b'-' { -1 } else { 1 };

    let start = if bytes[0] == b'-' || bytes[0] == b'+' {
        1
    } else {
        0
    };

    let point = match find(bytes, b'.') {
        Some(point) => point,
        None => {
            let integral = parse_int(bytes, start, bytes.len());
            return signum * integral * coef;
        }
    };

    let integral = parse_int(bytes, start, point);
    let exp = pow10((bytes.len() - point - 1) as i32);
    const_assert!(exp <= coef);

    let fractional = parse_int(bytes, point + 1, bytes.len());
    let final_integral = integral * coef;
    let final_fractional = coef / exp * fractional;

    signum * (final_integral + final_fractional)
}

#[test]
fn from_good_str() {
    let c = 1_000_000_000;
    assert_eq!(parse_fixed("1", c), 1000000000);
    assert_eq!(parse_fixed("1.1", c), 1100000000);
    assert_eq!(parse_fixed("1.02", c), 1020000000);
    assert_eq!(parse_fixed("-1.02", c), -1020000000);
    assert_eq!(parse_fixed("+1.02", c), 1020000000);
    assert_eq!(parse_fixed("123456789.123456789", c), 123456789123456789);
    assert_eq!(parse_fixed("9223372036.854775807", c), 9223372036854775807);
    assert_eq!(parse_fixed("0.1234", c), 123400000);
    assert_eq!(parse_fixed("-0.1234", c), -123400000);
}
