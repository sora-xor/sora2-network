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

pub mod source_initializers {
    use crate::{settings, Config};
    use frame_support::dispatch::DispatchResult;
    use frame_system::pallet_prelude::BlockNumberFor;
    use order_book::{MomentOf, OrderBookId};
    use sp_std::vec::Vec;

    /// Create multiple order books with default parameters if do not exist and
    /// fill them according to given parameters.
    ///
    /// Balance for placing the orders is minted automatically, trading pairs are
    /// created if needed.
    ///
    /// Parameters:
    /// - `caller`: account to mint non-divisible assets (for creating an order book)
    /// - `bids_owner`: Creator of the buy orders placed on the order books,
    /// - `asks_owner`: Creator of the sell orders placed on the order books,
    /// - `fill_settings`: Parameters for placing the orders in each order book.
    pub fn order_book<T: Config>(
        caller: T::AccountId,
        bids_owner: T::AccountId,
        asks_owner: T::AccountId,
        fill_settings: Vec<(
            OrderBookId<T::AssetId, T::DEXId>,
            settings::OrderBookFill<MomentOf<T>, BlockNumberFor<T>>,
        )>,
    ) -> DispatchResult {
        let order_book_ids: Vec<_> = fill_settings.iter().map(|(id, _)| id).cloned().collect();
        crate::pallet_tools::order_book::create_multiple_empty_unchecked::<T>(
            &caller,
            order_book_ids,
        )?;
        crate::pallet_tools::order_book::fill_multiple_empty_unchecked::<T>(
            bids_owner,
            asks_owner,
            fill_settings,
        )?;
        Ok(())
    }
}
