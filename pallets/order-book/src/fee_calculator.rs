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

use crate::{Config, MomentOf};
use common::prelude::FixedWrapper;
use common::Balance;
use sp_std::marker::PhantomData;

use common::prelude::constants::SMALL_FEE as BASE_FEE;

/// Calculator for order book custom fees
pub struct FeeCalculator<T: Config>(PhantomData<T>);

impl<T: Config> FeeCalculator<T> {
    /// Calculates the fee for `place_limit_order` extrinsic.
    ///
    /// The idea is to provide the reduced fee for market maker.
    /// If extrinsic is successfull, user pays maximum half network fee.
    /// If extrinsic failed, user pays full network fee.
    ///
    /// The actual fee contains two parts: constant and dynamic.
    /// Dynamic part depends on limit order lifetime. The longer the lifetime, the higher the dynamic fee.
    ///
    /// const part = (4 / 7) * (base fee / 2)
    /// dynamic part = (3 / 7) * (base fee / 2) * (lifetime / max_lifetime)
    pub fn place_limit_order_fee(lifetime: Option<MomentOf<T>>, is_err: bool) -> Option<Balance> {
        if is_err {
            return Some(BASE_FEE);
        }

        let market_maker_max_fee = BASE_FEE.checked_div(2)?;

        let Some(lifetime) = lifetime else {
            return Some(market_maker_max_fee);
        };

        let lifetime: u64 = lifetime.try_into().ok()?;
        let max_lifetime: u64 = T::MAX_ORDER_LIFESPAN.try_into().ok()?;

        let life_ratio = FixedWrapper::from(lifetime) / FixedWrapper::from(max_lifetime);

        let const_part = (FixedWrapper::from(market_maker_max_fee) / FixedWrapper::from(7))
            * FixedWrapper::from(4);
        let dynamic_part = (FixedWrapper::from(market_maker_max_fee) / FixedWrapper::from(7))
            * FixedWrapper::from(3)
            * life_ratio;

        (const_part + dynamic_part).try_into_balance().ok()
    }
}
