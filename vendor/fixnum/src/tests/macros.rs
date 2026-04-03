/// Testing helper macro. Allows to use generalized `FixedPoint` and `Layout` types in the test cases.
#[macro_export]
macro_rules! test_fixed_point {
    (
        case ($( $case_pattern:pat | $case_type:ty ),* $( , )?) => $case:block,
        $(
            $section_name:ident {$( ($( $section_args:expr ),* $( , )?) );+ $( ; )?},
        )+
    ) => {{
        macro_rules! impl_test_case {
            () => {
                fn test_case($( $case_pattern: $case_type ),*) -> anyhow::Result<()> {
                    $case
                    Ok(())
                }
            }
        }

        #[allow(unused)]
        macro_rules! fp {
            ($val:literal) => {{
                let value: FixedPoint = stringify!($val).parse()?;
                value
            }};
        }

        $(
            test_fixed_point!(@section $section_name {$( ($( $section_args )*) )*});
        )*
    }};
    (case () => $case:block,) => {
        test_fixed_point! {
            case () => $case,
            all {
                ();
            },
        };
    };
    (@section all {$( ($( $args:expr )*) )*}) => {
        #[cfg(feature = "i64")]
        {
            test_fixed_point!(@suite_impl fp64);
            test_fixed_point!(@suite_passes {$( ($( $args )*) )*});
        }
        #[cfg(feature = "i128")]
        {
            test_fixed_point!(@suite_impl fp128);
            test_fixed_point!(@suite_passes {$( ($( $args )*) )*});
        }
    };
    (@section fp64 {$( ($( $args:expr )*) )*}) => {
        #[cfg(feature = "i64")]
        {
            test_fixed_point!(@suite_impl fp64);
            test_fixed_point!(@suite_passes {$( ($( $args )*) )*});
        }
        #[cfg(feature = "i128")]
        {
            test_fixed_point!(@suite_impl fp128);
            test_fixed_point!(@suite_fails {$( ($( $args )*) )*});
        }
    };
    (@section fp128 {$( ($( $args:expr )*) )*}) => {
        #[cfg(feature = "i128")]
        {
            test_fixed_point!(@suite_impl fp128);
            test_fixed_point!(@suite_passes {$( ($( $args )*) )*});
        }
        #[cfg(feature = "i64")]
        {
            test_fixed_point!(@suite_impl fp64);
            test_fixed_point!(@suite_fails {$( ($( $args )*) )*});
        }
    };
    (@suite_impl fp64) => {
        type Layout = i64;
        #[allow(unused)]
        type FixedPoint = crate::FixedPoint<Layout, typenum::U9>;
        impl_test_case!();
    };
    (@suite_impl fp128) => {
        type Layout = i128;
        #[allow(unused)]
        type FixedPoint = crate::FixedPoint<Layout, typenum::U18>;
        impl_test_case!();
    };
    (@suite_passes {$( ($( $args:expr )*) )*}) => {
        $(
            $crate::tests::macros::r#impl::catch_and_augment(stringify!($( $args ),*), || {
                test_case($( $args ),*)
            })?;
        )*
    };
    (@suite_fails {$( ($( $args:expr )*) )*}) => {
        $(
            $crate::tests::macros::r#impl::catch_and_augment(stringify!($( $args ),*), || {
                $crate::tests::macros::r#impl::assert_fails(|| test_case($( $args ),*));
                Ok(())
            })?;
        )*
    };
}

#[cfg(not(feature = "std"))]
pub(crate) mod r#impl {
    use anyhow::Result;

    pub(crate) fn assert_fails(_case: impl FnOnce() -> Result<()>) {}

    pub(crate) fn catch_and_augment(
        _name: &'static str,
        case: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        case()
    }
}

#[cfg(feature = "std")]
pub(crate) mod r#impl {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use anyhow::{anyhow, Context, Result};
    use colored::Colorize;

    pub(crate) fn assert_fails(_case: impl FnOnce() -> Result<()>) {
        assert!(!matches!(catch_unwind(AssertUnwindSafe(_case)), Ok(Ok(()))));
    }

    pub(crate) fn catch_and_augment(
        name: &'static str,
        case: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        // TODO: the implementation isn't ideal and prints the panic twice.
        // A better solution requires a custom panic hook and manual backtrace handling.
        let result = match catch_unwind(AssertUnwindSafe(case)) {
            Ok(res) => res,
            Err(panic) => Err(anyhow!(stringify_panic(panic))),
        };

        result.context(format!("\n\n    case {} failed", name.blue()))
    }

    fn stringify_panic(payload: Box<dyn std::any::Any>) -> String {
        if let Some(message) = payload.downcast_ref::<&str>() {
            format!("panic: {}", message)
        } else if let Some(message) = payload.downcast_ref::<String>() {
            format!("panic: {}", message)
        } else {
            "panic: <unsupported payload>".into()
        }
    }
}
