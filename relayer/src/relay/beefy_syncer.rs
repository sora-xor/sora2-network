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

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[derive(Clone)]
pub struct BeefySyncer {
    latest_requested: Arc<AtomicU64>,
    latest_sent: Arc<AtomicU64>,
}

impl BeefySyncer {
    pub fn new() -> Self {
        Self {
            latest_requested: Default::default(),
            latest_sent: Default::default(),
        }
    }

    pub fn latest_requested(&self) -> u64 {
        self.latest_requested.load(Ordering::Relaxed)
    }

    pub fn latest_sent(&self) -> u64 {
        self.latest_sent.load(Ordering::Relaxed)
    }

    pub fn update_latest_requested(&self, block: u64) {
        self.latest_requested
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v < block {
                    debug!("Requesting new BEEFY block {}", block);
                    Some(block)
                } else {
                    None
                }
            })
            .ok();
    }

    pub fn update_latest_sent(&self, block: u64) {
        self.latest_sent
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                if v < block {
                    debug!("Updating latest sent BEEFY block to {}", block);
                    Some(block)
                } else {
                    None
                }
            })
            .ok();
    }
}
