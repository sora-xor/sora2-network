use crate::{Fixed, LiquiditySourceId};
use sp_std::vec::Vec;

/// Check if value belongs valid range of basis points, 0..10000 corresponds to 0.01%..100.00%.
/// Returns true if range is valid, false otherwise.
pub fn in_basis_points_range<BP: Into<u16>>(value: BP) -> bool {
    match value.into() {
        0..=10000 => true,
        _ => false,
    }
}

/// Create fraction as Fixed from BasisPoints value.
pub fn fixed_from_basis_points<BP: Into<u16>>(value: BP) -> Fixed {
    let value_inner: u16 = value.into();
    Fixed::from_inner(value_inner as u128 * 100_000_000_000_000)
}
/// Generalized filtration mechanism for listing liquidity sources.
pub struct LiquiditySourceFilter<DEXId: PartialEq, LiquiditySourceIndex: PartialEq> {
    pub list: Vec<(Option<DEXId>, Option<LiquiditySourceIndex>)>,
    pub ignore_selected: bool,
}

impl<DEXId: PartialEq + Clone, LiquiditySourceIndex: PartialEq + Clone>
    LiquiditySourceFilter<DEXId, LiquiditySourceIndex>
{
    fn make_list(
        liquidity_sources: &[LiquiditySourceId<DEXId, LiquiditySourceIndex>],
    ) -> Vec<(Option<DEXId>, Option<LiquiditySourceIndex>)> {
        liquidity_sources
            .iter()
            .map(|elem| {
                (
                    Some(elem.dex_id.clone()),
                    Some(elem.liquidity_source_index.clone()),
                )
            })
            .collect()
    }

    /// Create filter with no effect.
    pub fn empty() -> Self {
        Self {
            list: Vec::new(),
            ignore_selected: true,
        }
    }

    /// Create filter with fully identified liquidity sources which are ignored.
    pub fn with_concrete_ignored(
        liquidity_sources: &[LiquiditySourceId<DEXId, LiquiditySourceIndex>],
    ) -> Self {
        Self {
            list: Self::make_list(liquidity_sources),
            ignore_selected: true,
        }
    }

    /// Create filter with fully identified liquidity sources which are allowed.
    pub fn with_concrete_allowed(
        liquidity_sources: &[LiquiditySourceId<DEXId, LiquiditySourceIndex>],
    ) -> Self {
        Self {
            list: Self::make_list(liquidity_sources),
            ignore_selected: false,
        }
    }

    /// Create filter with partially identified liquidity sources - by their DEXId.
    pub fn with_ignored_dex_ids(dex_ids: &[DEXId]) -> Self {
        Self {
            list: dex_ids
                .iter()
                .map(|elem| (Some(elem.clone()), None))
                .collect(),
            ignore_selected: true,
        }
    }

    /// Create filter with partially identified liquidity sources - by their DEXId.
    pub fn with_allowed_dex_ids(dex_ids: &[DEXId]) -> Self {
        Self {
            list: dex_ids
                .iter()
                .map(|elem| (Some(elem.clone()), None))
                .collect(),
            ignore_selected: false,
        }
    }

    /// Check if given liquidity source is allowed by filter. Return True if allowed.
    pub fn matches(
        &self,
        liquidity_source_id: &LiquiditySourceId<DEXId, LiquiditySourceIndex>,
    ) -> bool {
        for filter in self.list.iter() {
            match filter {
                (Some(dex_id), Some(index)) => {
                    if dex_id == &liquidity_source_id.dex_id
                        && index == &liquidity_source_id.liquidity_source_index
                    {
                        return !self.ignore_selected;
                    }
                }
                (Some(dex_id), None) => {
                    if dex_id == &liquidity_source_id.dex_id {
                        return !self.ignore_selected;
                    }
                }
                (None, Some(index)) => {
                    if index == &liquidity_source_id.liquidity_source_index {
                        return !self.ignore_selected;
                    }
                }
                (None, None) => return !self.ignore_selected,
            }
        }
        self.ignore_selected
    }
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
}
