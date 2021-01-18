use core::convert::TryFrom;

use sp_std::vec::Vec;

use common::{
    fixed,
    prelude::{fixnum::ops::CheckedAdd, FixedWrapper},
    Fixed,
};

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
        let distribution_part = (FixedWrapper::from(parts_left)
            - FixedWrapper::from(foreign[cur_exchange][parts_left]))
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
