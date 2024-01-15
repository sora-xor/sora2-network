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

use crate::prelude::FixedWrapper;
use crate::Fixed;
use fixnum::_priv::RoundMode;
use fixnum::ops::{Bounded, CheckedAdd, RoundingMul};
use fixnum::typenum::Unsigned;

/// Can be useful to check that an extrinsic is failed due to an error in another pallet
#[macro_export]
macro_rules! assert_noop_msg {
    ( $x:expr, $msg:expr ) => {
        let h = frame_support::storage_root(frame_support::StateVersion::V1);
        if let Err(e) = $crate::with_transaction(|| $x) {
            if let frame_support::dispatch::DispatchError::Module(sp_runtime::ModuleError {
                message,
                ..
            }) = e.error
            {
                assert_eq!(message, Some($msg));
            } else {
                panic!("expected DispatchError::Module, got {:?}", e.error);
            }
        } else {
            panic!("expected Err(_), got Ok(_)");
        }
        assert_eq!(
            h,
            frame_support::storage_root(frame_support::StateVersion::V1)
        );
    };
}

pub fn init_logger() {
    let _ = env_logger::builder().is_test(true).try_init();
}
// Calculate if two values are approximately equal
// (up to some absolute tolerance (constant value))
pub fn are_approx_eq_abs(left: FixedWrapper, right: FixedWrapper, tolerance: FixedWrapper) -> bool {
    left.clone() < right.clone() + tolerance.clone() && right < left + tolerance
}
