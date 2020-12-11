use common::Fixed;
use frame_support::sp_runtime::traits::Saturating;
use sp_std::vec::Vec;

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
    if sample_data.is_empty() {
        return (Vec::new(), Fixed::from(0));
    }
    let n = sample_data.len();
    let s = sample_data[0].len();

    if s == 0 {
        return (Vec::new(), Fixed::from(0));
    }

    let mut accumulator: Vec<Vec<Fixed>> = vec![vec![Fixed::from(0); s + 1]; n];
    accumulator[0][1..].copy_from_slice(&sample_data[0][..]);
    let mut foreign: Vec<Vec<usize>> = vec![vec![0; s + 1]; n];

    for i in 1..n {
        for j in 1..=s {
            accumulator[i][j] = accumulator[i - 1][j];
            foreign[i][j] = j;

            for k in 0..j {
                let tmp: Fixed = accumulator[i - 1][j - k - 1].saturating_add(sample_data[i][k]);
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

    let total_parts = Fixed::from(s as u128);
    let mut parts_left = s;
    let mut cur_exchange = n;
    let mut distribution = vec![Fixed::from(0); n];

    while parts_left > 0 {
        if cur_exchange == 0 {
            break;
        }
        cur_exchange -= 1;
        distribution[cur_exchange] = Fixed::from(parts_left as u128)
            .saturating_sub(Fixed::from(foreign[cur_exchange][parts_left] as u128))
            / total_parts;
        parts_left = foreign[cur_exchange][parts_left] as usize;
    }

    let best_amount = accumulator[n - 1][s];
    (distribution, best_amount)
}
