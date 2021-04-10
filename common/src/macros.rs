// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification, 
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list 
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this 
// list of conditions and the following disclaimer in the documentation and/or other 
// materials provided with the distribution.
// 
// All advertising materials mentioning features or use of this software must display 
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
// 
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used 
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES, 
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR 
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY 
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, 
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; 
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, 
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE 
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

#[macro_export]
macro_rules! fixed {
    ($val:literal) => {
        $crate::fixnum::fixnum!($val, 18)
    };
}

#[macro_export]
macro_rules! fixed_const {
    ($val:literal) => {
        $crate::fixnum::fixnum_const!($val, 18)
    };
}

#[macro_export]
macro_rules! balance {
    ($value:literal) => {{
        use $crate::fixnum::_priv::parse_fixed;
        const VALUE_SIGNED: i128 = parse_fixed(stringify!($value), 1_000_000_000_000_000_000);
        const VALUE: $crate::Balance = VALUE_SIGNED.abs() as u128;
        VALUE
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
