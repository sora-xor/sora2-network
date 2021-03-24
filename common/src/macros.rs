#[macro_export]
macro_rules! fixed {
    ($val:literal) => {
        $crate::fixnum::fixnum!($val, 18)
    };
}

#[macro_export]
macro_rules! balance {
    ($val:literal) => {{
        use core::convert::TryInto;
        let fixed: $crate::Fixed = $crate::fixed!($val);
        let value: u128 = fixed.into_bits().try_into().unwrap();
        value
    }};
}

#[macro_export]
macro_rules! fixed_wrapper {
    ($val:literal) => {{
        let val: $crate::prelude::FixedWrapper = $crate::fixed!($val);
        val
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

#[macro_export]
macro_rules! location_stamp {
    ($name:tt) => {
        &format!("{} at {}:{}", $name, core::file!(), core::line!())
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn should_calculate_formula() {
        use crate::Fixed;

        fn fp(s: &str) -> Fixed {
            s.parse().unwrap()
        }

        let f: Fixed = fixed!(1);
        assert_eq!(f, fp("1"));
        let f: Fixed = fixed!(1.2);
        assert_eq!(f, fp("1.2"));
        let f: Fixed = fixed!(10.09);
        assert_eq!(f, fp("10.09"));
    }
}
