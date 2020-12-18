#[macro_export]
macro_rules! fixed {
    ($val:literal) => {
        $crate::fixnum::fixnum!($val, 18)
    };
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
