#![cfg_attr(not(feature = "std"), no_std)]

/// Quick alias for better looking code.
#[macro_export]
macro_rules! to_balance(
    ($a: expr) => ({
        $a.try_into_balance()
          .map_err(|_| Error::<T>::FixedWrapperCalculationFailed)?
    })
);

/// Quick macro for better looking code, rust compiler is clever to optimize this.
#[macro_export]
macro_rules! to_fixed_wrapper(
    ($a: expr) => ({
        FixedWrapper::from($a.clone()).clone()
    })
);
