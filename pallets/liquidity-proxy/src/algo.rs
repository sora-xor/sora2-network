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

use core::convert::TryFrom;

use sp_std::vec::Vec;

use common::prelude::fixnum::ops::CheckedAdd;
use common::prelude::FixedWrapper;
use common::{balance, fixed, Fixed};

/// Given a set of monotoneous sequences A_i(n), i = 0..M-1, n = 0..N-1 returns a pair of:
/// - a vector of "weights" [W_i / N], i = 0..M-1, where W_i are lengths of respective
/// subsequences A_i(k), k = 0..W_i-1 such that the sum S = Sum(A_i(W_i)) for i = 0..M-1
/// is the largest (smallest) across all possible combinations while the sum of weights
/// Sum(W_i) = N,
/// - the optimal sum value S.
///
/// - `sample_data`: a 2D matrix of N vectors each composed of M elements,
/// - `inversed`: boolean flag: if true, the overall sum is minimized (otherwise maximized).
pub fn find_distribution(sample_data: Vec<Vec<Fixed>>, inversed: bool) -> (Vec<Fixed>, Fixed) {
    fn default() -> (Vec<Fixed>, Fixed) {
        (Default::default(), fixed!(0))
    }
    if sample_data.is_empty() {
        return default();
    }
    let n = sample_data.len();
    let s = sample_data[0].len();
    let total_parts = match Fixed::try_from(s) {
        Err(_) => return default(),
        Ok(value) if value == fixed!(0) => return default(),
        Ok(value) => value,
    };

    let mut accumulator: Vec<Vec<Fixed>> = vec![vec![fixed!(0); s + 1]; n];
    accumulator[0][1..].copy_from_slice(&sample_data[0][..]);
    let mut foreign: Vec<Vec<usize>> = vec![vec![0; s + 1]; n];

    for i in 1..n {
        for j in 1..=s {
            accumulator[i][j] = accumulator[i - 1][j];
            foreign[i][j] = j;

            for k in 0..j {
                let tmp: Fixed = match accumulator[i - 1][j - k - 1].cadd(sample_data[i][k]) {
                    Err(_) => continue,
                    Ok(value) => value,
                };
                let is_better = match inversed {
                    true => tmp < accumulator[i][j],
                    _ => tmp > accumulator[i][j],
                };
                if is_better {
                    accumulator[i][j] = tmp;
                    foreign[i][j] = j - k - 1;
                }
            }
        }
    }

    let mut parts_left = s;
    let mut cur_exchange = n;
    let mut distribution = vec![fixed!(0); n];

    while parts_left > 0 && cur_exchange != 0 {
        cur_exchange -= 1;
        let distribution_part = (FixedWrapper::from(parts_left as u128 * balance!(1))
            - foreign[cur_exchange][parts_left] as u128 * balance!(1))
            / total_parts;
        distribution[cur_exchange] = match distribution_part.get() {
            Err(_) => return default(),
            Ok(value) => value,
        };
        parts_left = foreign[cur_exchange][parts_left];
    }

    let best_amount = accumulator[n - 1][s];
    (distribution, best_amount)
}
