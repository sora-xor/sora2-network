use crate::{Fixed, LiquiditySourceId};
use sp_arithmetic::FixedPointNumber;
use sp_std::vec::Vec;

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
    let value_inner: u16 = value.into();
    Fixed::from_inner(
        value_inner as u128 * (<Fixed as FixedPointNumber>::DIV / BASIS_POINTS_RANGE as u128),
    )
}

/// Generalized filtration mechanism for listing liquidity sources.
pub struct LiquiditySourceFilter<DEXId: PartialEq + Copy, LiquiditySourceIndex: PartialEq + Copy> {
    /// DEX Id to which listing is limited.
    pub dex_id: DEXId,
    /// Selected Liquidity Source Indices, e.g. Types comprising filter.
    pub selected: Vec<LiquiditySourceIndex>,
    /// Switch to either include only sources selected if `false`,
    /// or include only sources not selected if `true`.
    pub ignore_selected: bool,
}

impl<DEXId: PartialEq + Copy, LiquiditySourceIndex: PartialEq + Copy>
    LiquiditySourceFilter<DEXId, LiquiditySourceIndex>
{
    /// Create filter with no effect.
    pub fn empty(dex_id: DEXId) -> Self {
        Self {
            dex_id,
            selected: Vec::new(),
            ignore_selected: true,
        }
    }

    pub fn new(
        dex_id: DEXId,
        selected_indices: &[LiquiditySourceIndex],
        ignore_selected: bool,
    ) -> Self {
        Self {
            dex_id,
            selected: selected_indices.iter().cloned().collect(),
            ignore_selected,
        }
    }

    /// Create filter with fully identified liquidity sources which are ignored.
    pub fn with_ignored(dex_id: DEXId, ignored_indices: &[LiquiditySourceIndex]) -> Self {
        Self {
            dex_id,
            selected: ignored_indices.iter().cloned().collect(),
            ignore_selected: true,
        }
    }

    /// Create filter with fully identified liquidity sources which are allowed.
    pub fn with_allowed(dex_id: DEXId, allowed_indices: &[LiquiditySourceIndex]) -> Self {
        Self {
            dex_id,
            selected: allowed_indices.iter().cloned().collect(),
            ignore_selected: false,
        }
    }

    pub fn matches_dex_id(&self, dex_id: DEXId) -> bool {
        self.dex_id == dex_id
    }

    pub fn matches_index(&self, index: LiquiditySourceIndex) -> bool {
        for idx in self.selected.iter() {
            if *idx == index {
                return !self.ignore_selected;
            }
        }
        self.ignore_selected
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
pub const fn pow(base: u32, mut exp: u32) -> u128 {
    let int = base as u128;
    let mut n = 1;
    while exp > 0 {
        exp -= 1;
        n *= int;
    }
    n
}

/// Counts all non-underscore characters in the string (which is expected to be
/// a string-formatted number).
/// This function is used by `fixed!` macro to calculate order of the number.
pub const fn number_str_order(num_s: &str) -> u32 {
    let bytes = num_s.as_bytes();
    let mut ord = 0;
    let mut n = 0;
    while n < bytes.len() {
        let a = bytes[n];
        if a != b'_' {
            ord += 1;
        }
        n += 1;
    }
    ord
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_in_basis_points_range_should_pass() {
        for num in u16::MIN..u16::MAX {
            assert_eq!(in_basis_points_range(num), num <= 10_000);
        }
    }

    #[test]
    fn test_fixed_from_basis_points_should_pass() {
        assert_eq!(
            fixed_from_basis_points(1u16) * Fixed::from(10_000),
            Fixed::from(1)
        );
        assert_eq!(Fixed::from_fraction(0.003), fixed_from_basis_points(30u16));
        assert_eq!(Fixed::from_fraction(0.0001), fixed_from_basis_points(1u16));
        assert_eq!(
            Fixed::from_fraction(0.9999),
            fixed_from_basis_points(9_999u16)
        );
        assert_eq!(Fixed::from(1), fixed_from_basis_points(10_000u16));
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
    fn test_filter_ignore_liquidity_source_id_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_ignored(0, &[0, 1]);
        assert!(!filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 0)));
        assert!(!filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 1)));
        assert!(filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 2)));
    }

    #[test]
    fn test_filter_allow_liquidity_source_id_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, &[0, 1]);
        assert!(filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 0)));
        assert!(filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 1)));
        assert!(!filter.matches(&LiquiditySourceId::<u8, u8>::new(0, 2)));
    }

    #[test]
    fn test_filter_ignore_none_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_ignored(0, &[]);
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_ignore_some_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_ignored(0, &[0, 1]);
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_ignore_all_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_ignored(0, &[0, 1, 2]);
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(!filter.matches_index(2));
    }

    #[test]
    fn test_filter_allow_none_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, &[]);
        assert!(!filter.matches_index(0));
        assert!(!filter.matches_index(1));
        assert!(!filter.matches_index(2));
    }

    #[test]
    fn test_filter_allow_some_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, &[1, 2]);
        assert!(!filter.matches_index(0));
        assert!(filter.matches_index(1));
        assert!(filter.matches_index(2));
    }

    #[test]
    fn test_filter_allow_all_should_pass() {
        let filter = LiquiditySourceFilter::<u8, u8>::with_allowed(0, &[0, 1, 2]);
        assert!(filter.matches_index(0));
        assert!(filter.matches_index(1));
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
    fn test_number_str_len() {
        assert_eq!(number_str_order("1234"), 4);
        assert_eq!(number_str_order("12_34"), 4);
    }
}
