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

use frame_support::weights::Weight;
use sp_std::marker::PhantomData;

pub trait WeightInfo {
    fn create_orderbook() -> Weight {
        Weight::zero()
    }
    fn delete_orderbook() -> Weight {
        Weight::zero()
    }
    fn update_orderbook() -> Weight {
        Weight::zero()
    }
    fn change_orderbook_status() -> Weight {
        Weight::zero()
    }
    fn place_limit_order() -> Weight {
        Weight::zero()
    }
    fn cancel_limit_order() -> Weight {
        Weight::zero()
    }
    fn execute_market_order() -> Weight {
        Weight::zero()
    }
    fn quote() -> Weight {
        Weight::zero()
    }
    fn exchange_single_order() -> Weight {
        Weight::zero()
    }
    fn service_base() -> Weight {
        Weight::zero()
    }
    fn service_block_base() -> Weight {
        Weight::zero()
    }
    fn service_single_expiration() -> Weight {
        Weight::zero()
    }
}

impl WeightInfo for () {}

pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn create_orderbook() -> Weight {
        Weight::zero()
    }
    fn delete_orderbook() -> Weight {
        Weight::zero()
    }
    fn update_orderbook() -> Weight {
        Weight::zero()
    }
    fn change_orderbook_status() -> Weight {
        Weight::zero()
    }
    fn place_limit_order() -> Weight {
        Weight::zero()
    }
    fn cancel_limit_order() -> Weight {
        Weight::zero()
    }
    fn execute_market_order() -> Weight {
        Weight::zero()
    }
    fn quote() -> Weight {
        Weight::zero()
    }
    fn exchange_single_order() -> Weight {
        Weight::zero()
    }
    fn service_base() -> Weight {
        Weight::zero()
    }
    fn service_block_base() -> Weight {
        Weight::zero()
    }
    fn service_single_expiration() -> Weight {
        // todo: benchmark
        // not zero for now to test weight limits in `on_initialize`
        Weight::from_parts(93_304_000, 21168)
    }
}
