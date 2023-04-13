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

//! Benchmarking setup for order-book

#![cfg(feature = "runtime-benchmarks")]
// order-book
#![cfg(feature = "wip")]
// now it works only as benchmarks, not as unit tests
// TODO fix when new approach be developed
#![cfg(not(test))]

#[cfg(not(test))]
use crate::{Config, Pallet};
#[cfg(test)]
use framenode_runtime::order_book::{Config, Pallet};

use frame_benchmarking::benchmarks;

benchmarks! {

    create_orderbook {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    delete_orderbook {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    update_orderbook {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    change_orderbook_status {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    place_limit_order {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    cancel_limit_order {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    quote {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    exchange {
    }: {
        // todo (m.tagirov)
    }
    verify {
    }

    impl_benchmark_test_suite!(Pallet, framenode_chain_spec::ext(), framenode_runtime::Runtime);
}
