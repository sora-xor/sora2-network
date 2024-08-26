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

#[cfg(feature = "wip")] // Contracts pallet
mod contracts;
mod liquidity_proxy;
mod referrals;
#[cfg(feature = "try-runtime")]
mod remote;
mod xor_fee;

mod tests {
    use crate::{Currencies, Referrals, RuntimeOrigin};
    use assets::GetTotalBalance;
    use common::mock::{alice, bob};
    use common::prelude::constants::SMALL_FEE;
    use common::XOR;
    use frame_support::assert_ok;
    use framenode_chain_spec::ext;

    #[test]
    fn get_total_balance() {
        ext().execute_with(|| {
            assert_ok!(Currencies::update_balance(
                RuntimeOrigin::root(),
                alice(),
                XOR.into(),
                SMALL_FEE as i128
            ));
            Referrals::reserve(RuntimeOrigin::signed(alice()), SMALL_FEE).unwrap();
            assert_eq!(
                crate::GetTotalBalance::total_balance(&XOR, &alice()),
                Ok(SMALL_FEE)
            );

            assert_eq!(crate::GetTotalBalance::total_balance(&XOR, &bob()), Ok(0));
        });
    }
}
