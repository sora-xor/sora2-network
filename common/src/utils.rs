use core::convert::TryFrom;

use codec::{Decode, Encode};
use fixnum::ops::{RoundMode::*, RoundingDiv};
use frame_support::RuntimeDebug;
use sp_std::{iter::once, vec::Vec};

use crate::prelude::{FilterMode, Fixed, FixedInner, FixedWrapper, LiquiditySourceId};

/// Basis points range (0..10000) corresponds to 0.01%..100.00%.
const BASIS_POINTS_RANGE: u16 = 10000;

/// Check if value belongs valid range of basis points.
/// Returns true if range is valid, false otherwise.
pub fn in_basis_points_range<BP: Into<u16>>(value: BP) -> bool {
    match value.into() {
        0..=BASIS_POINTS_RANGE => true,
        _ => false,
    }
}

/// Create fraction as Fixed from BasisPoints value.
pub fn fixed_from_basis_points<BP: Into<u16>>(value: BP) -> Fixed {
    let value: u16 = value.into();
    Fixed::try_from(i128::from(value))
        .unwrap()
        .rdiv(i128::from(BASIS_POINTS_RANGE), Floor)
        .unwrap() // TODO(quasiyoke): should be checked
}

/// An auxiliary type to denote an interval variants: (a, b), [a, b), (a, b] and [a, b].
pub enum IntervalEndpoints {
    None,
    Left,
    Right,
    Both,
}

/// Evenly distribute N points inside an interval one of the following ways:
/// - none endpoint included:   o - - - - - x - - - - - x - - - - - x - - - - - o
/// - left endpoint included:   x - - - - - - - x - - - - - - - x - - - - - - - o
/// - right endpoint included:  o - - - - - - - x - - - - - - - x - - - - - - - x
/// - both endpoints included:  x - - - - - - - - - - - x - - - - - - - - - - - x
pub fn linspace(a: Fixed, b: Fixed, n: usize, endpoints: IntervalEndpoints) -> Vec<Fixed> {
    if n == 0 {
        return Vec::<Fixed>::new();
    };

    if a == b {
        return vec![a; n];
    }

    match endpoints {
        IntervalEndpoints::None => linspace_inner(a, b, n),
        IntervalEndpoints::Left => once(a)
            .chain(linspace_inner(a, b, n - 1).into_iter())
            .collect(),
        IntervalEndpoints::Right => linspace_inner(a, b, n - 1)
            .into_iter()
            .chain(once(b))
            .collect(),
        IntervalEndpoints::Both => {
            if n == 1 {
                once(b).collect()
            } else {
                once(a)
                    .chain(linspace_inner(a, b, n - 2).into_iter())
                    .chain(once(b))
                    .collect()
            }
        }
    }
}

/// Helper function that evenly spreads points inside an interval with endpoints excluded
/// Can only be called from public function `linspace` hence no additional bound checks
fn linspace_inner(a: Fixed, b: Fixed, n: usize) -> Vec<Fixed> {
    let a: FixedWrapper = a.into();
    let b: FixedWrapper = b.into();
    let width: FixedWrapper = (n as u128 + 1).into();
    (1..=n)
        .map(|x| -> Fixed {
            let x: FixedWrapper =
                a.clone() + (b.clone() - a.clone()) / width.clone() / FixedWrapper::from(x as u128);
            x.get().unwrap()
        })
        .collect()
}

pub mod string_serialization {
    #[cfg(feature = "std")]
    use serde::{Deserialize, Deserializer, Serializer};

    #[cfg(feature = "std")]
    pub fn serialize<S: Serializer, T: std::fmt::Display>(
        t: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&t.to_string())
    }

    #[cfg(feature = "std")]
    pub fn deserialize<'de, D: Deserializer<'de>, T: std::str::FromStr>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<T>()
            .map_err(|_| serde::de::Error::custom("Parse from string failed"))
    }
}

/// Generalized filtration mechanism for listing liquidity sources.
#[derive(Encode, Decode, Clone, RuntimeDebug)]
pub struct LiquiditySourceFilter<DEXId: PartialEq + Copy, LiquiditySourceIndex: PartialEq + Copy> {
    /// DEX Id to which listing is limited.
    pub dex_id: DEXId,
    /// Selected Liquidity Source Indices, e.g. Types comprising filter.
    pub selected: Vec<LiquiditySourceIndex>,
    /// Switch to either include only sources selected if `false`,
    /// or include only sources not selected if `true`.
    pub forbid_selected: bool,
}

impl<DEXId: PartialEq + Copy, LiquiditySourceIndex: PartialEq + Copy>
    LiquiditySourceFilter<DEXId, LiquiditySourceIndex>
{
    /// Create filter with no effect.
    pub fn empty(dex_id: DEXId) -> Self {
        Self {
            dex_id,
            selected: Vec::new(),
            forbid_selected: true,
        }
    }

    pub fn new(
        dex_id: DEXId,
        selected_indices: Vec<LiquiditySourceIndex>,
        forbid_selected: bool,
    ) -> Self {
        Self {
            dex_id,
            selected: selected_indices,
            forbid_selected,
        }
    }

    /// Create filter with fully identified liquidity sources which are forbidden, all other sources are allowed.
    pub fn with_forbidden(dex_id: DEXId, forbidden_indices: Vec<LiquiditySourceIndex>) -> Self {
        Self {
            dex_id,
            selected: forbidden_indices,
            forbid_selected: true,
        }
    }

    /// Create filter with fully identified liquidity sources which are allowed, all other sources are forbidden.
    pub fn with_allowed(dex_id: DEXId, allowed_indices: Vec<LiquiditySourceIndex>) -> Self {
        Self {
            dex_id,
            selected: allowed_indices,
            forbid_selected: false,
        }
    }

    pub fn with_mode(
        dex_id: DEXId,
        mode: FilterMode,
        selected_indices: Vec<LiquiditySourceIndex>,
    ) -> Self {
        match mode {
            FilterMode::Disabled => LiquiditySourceFilter::empty(dex_id),
            FilterMode::AllowSelected => {
                LiquiditySourceFilter::with_allowed(dex_id, selected_indices)
            }
            FilterMode::ForbidSelected => {
                LiquiditySourceFilter::with_forbidden(dex_id, selected_indices)
            }
        }
    }

    pub fn matches_dex_id(&self, dex_id: DEXId) -> bool {
        self.dex_id == dex_id
    }

    pub fn matches_index(&self, index: LiquiditySourceIndex) -> bool {
        for idx in self.selected.iter() {
            if *idx == index {
                return !self.forbid_selected;
            }
        }
        self.forbid_selected
    }

    /// Check if given liquidity source is allowed by filter. Return True if allowed.
    pub fn matches(
        &self,
        liquidity_source_id: &LiquiditySourceId<DEXId, LiquiditySourceIndex>,
    ) -> bool {
        self.matches_dex_id(liquidity_source_id.dex_id)
            && self.matches_index(liquidity_source_id.liquidity_source_index)
    }
}

/// Rises `base` to the power of `exp`.
/// Differs from std's `pow` with `const`
pub const fn pow(base: u32, mut exp: u32) -> FixedInner {
    let int = base as FixedInner;
    let mut n = 1;
    while exp > 0 {
        exp -= 1;
        n *= int;
    }
    n
}

#[cfg(test)]
mod tests {
    use fixnum::ops::{CheckedMul, Numeric};

    use crate::*;

    fn fp(s: &str) -> Fixed {
        s.parse().unwrap()
    }

    #[test]
    fn test_in_basis_points_range_should_pass() {
        for num in u16::MIN..u16::MAX {
            assert_eq!(in_basis_points_range(num), num <= 10_000);
        }
    }

    #[test]
    fn test_fixed_from_basis_points_should_pass() {
        assert_eq!(fixed_from_basis_points(1u16).cmul(10_000).unwrap(), fp("1"));
        assert_eq!(fixed_from_basis_points(30u16), fp("0.003"));
        assert_eq!(fixed_from_basis_points(1u16), fp("0.0001"));
        assert_eq!(fixed_from_basis_points(9_999u16), fp("0.9999"));
        assert_eq!(fixed_from_basis_points(10_000u16), fp("1"));
    }

    #[test]
    fn test_filter_indices_empty_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::empty(0);
        assert!(filter.matches_index(0));
    }

    #[test]
    fn test_filter_matches_dex_id_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::empty(0);
        assert!(filter.matches_dex_id(0));
        assert!(!filter.matches_dex_id(1));
    }

    #[test]
    fn test_filter_forbid_liquidity_source_id_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_forbidden(0, [0, 1].into());
        assert!(!filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 0)));
        assert!(!filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 1)));
        assert!(filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 2)));
    }

    #[test]
    fn test_filter_allow_liquidity_source_id_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, [0, 1].into());
        assert!(filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 0)));
        assert!(filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 1)));
        assert!(!filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 2)));
    }

    #[test]
    fn test_filter_forbid_none_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_forbidden(0, [].into());
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_forbid_some_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_forbidden(0, [0, 1].into());
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_forbid_all_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_forbidden(0, [0, 1, 2].into());
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(!filter.matches_index(2));
    }

    #[test]
    fn test_filter_allow_none_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, [].into());
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(!filter.matches_index(2));
    }

    #[test]
    fn test_filter_allow_some_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, [1, 2].into());
        assert!(!filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_allow_all_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, [0, 1, 2].into());
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_mode_disabled_none_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_mode(0, FilterMode::Disabled, [].into());
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_mode_disabled_some_should_pass() {
        let filter =
            LiquiditySourceFilter::<u8, u8>::with_mode(0, FilterMode::Disabled, [0, 1, 2].into());
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_mode_allowed_none_should_pass() {
        let filter =
            LiquiditySourceFilter::<u8, u8>::with_mode(0, FilterMode::AllowSelected, [].into());
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(!filter.matches_index(2));
    }

    #[test]
    fn test_filter_mode_allowed_some_should_pass() {
        let filter =
            LiquiditySourceFilter::<u8, u8>::with_mode(0, FilterMode::AllowSelected, [1, 2].into());
        assert!(!filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_mode_forbidden_none_should_pass() {
        let filter =
            LiquiditySourceFilter::<u8, u8>::with_mode(0, FilterMode::ForbidSelected, [].into());
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_mode_forbidden_some_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_mode(
            0,
            FilterMode::ForbidSelected,
            [0, 1].into(),
        );
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_pow() {
        assert_eq!(pow(0, 2), 0);
        assert_eq!(pow(2, 0), 1);
        assert_eq!(pow(2, 3), 8);
        assert_eq!(pow(3, 2), 9);
    }

    #[test]
    fn test_linspace_should_pass() {
        // (0, 2], 6 points
        assert_eq!(
            &linspace(fixed!(0), fixed!(2), 6, IntervalEndpoints::Right),
            &[
                fixed!(0.333333333333333333),
                fixed!(0.666666666666666666),
                fixed!(1),
                fixed!(1.333333333333333333),
                fixed!(1.666666666666666666),
                fixed!(2),
            ]
        );

        // [1, 11), 6 points
        assert_eq!(
            &linspace(fixed!(1), fixed!(11), 6, IntervalEndpoints::Left),
            &[
                fixed!(1),
                fixed!(2.666666666666666666),
                fixed!(4.333333333333333333),
                fixed!(6),
                fixed!(7.666666666666666666),
                fixed!(9.333333333333333333),
            ]
        );

        // (0, 1), 6 points
        assert_eq!(
            &linspace(fixed!(0), fixed!(1), 6, IntervalEndpoints::None),
            &[
                fixed!(0.142857142857142857),
                fixed!(0.285714285714285714),
                fixed!(0.428571428571428571),
                fixed!(0.571428571428571428),
                fixed!(0.714285714285714285),
                fixed!(0.857142857142857143),
            ]
        );

        // (0, 1), 8 points
        assert_eq!(
            &linspace(fixed!(0), fixed!(1), 8, IntervalEndpoints::Both),
            &[
                fixed!(0),
                fixed!(0.142857142857142857),
                fixed!(0.285714285714285714),
                fixed!(0.428571428571428571),
                fixed!(0.571428571428571428),
                fixed!(0.714285714285714285),
                fixed!(0.857142857142857143),
                fixed!(1),
            ]
        );
    }

    #[test]
    fn test_linspace_corner_cases_should_pass() {
        // 0 points requested => []
        assert_eq!(
            &linspace(fixed!(0), fixed!(2), 0, IntervalEndpoints::Right),
            &[]
        );

        // [100, 100), 5 points => [100, 100, 100, 100, 100]
        assert_eq!(
            linspace(fixed!(100), fixed!(100), 5, IntervalEndpoints::Left),
            vec![fixed!(100); 5]
        );

        // [100, 100], 6 points => [100, 100, 100, 100, 100, 100]
        assert_eq!(
            linspace(fixed!(100), fixed!(100), 6, IntervalEndpoints::Both),
            vec![fixed!(100); 6]
        );

        // [0, Fixed::max_value()], 3 points
        assert_eq!(
            &linspace(
                fixed!(0),
                <Fixed as Numeric>::MAX,
                3,
                IntervalEndpoints::Both
            ),
            &[
                fixed!(0),
                fixed!(85070591730234615865.843651857942052864),
                fixed!(170141183460469231731.687303715884105727),
            ]
        );
    }
}
