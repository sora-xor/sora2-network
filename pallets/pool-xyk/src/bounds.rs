#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

/// Bounds enum, used for cases than min max limits is used. Also used for cases than values is
/// Desired by used or Calculated by forumula. Dummy is used to abstract checking.
#[derive(Clone, Copy, RuntimeDebug, Eq, PartialEq, Encode, Decode)]
pub enum Bounds<Balance> {
    /// This is consequence of computations, and not sed by used.
    Calculated(Balance),
    /// This values set by used as fixed and determed value.
    Desired(Balance),
    /// This is undetermined value, bounded by some logic or ranges.
    Min(Balance),
    Max(Balance),
    /// This is determined value than pool is emply, then pool is not empty this works like range.
    RangeFromDesiredToMin(Balance, Balance),
    /// This is just unknown value that must be calulated and filled.
    Decide,
    /// This is used in some checks tests and predicates, than value is not needed.
    Dummy,
}

impl<Balance: Ord + Eq + Clone> Bounds<Balance> {
    /// Unwrap only known values, min and max is not known for final value.
    pub fn unwrap(self) -> Balance {
        match self {
            Bounds::Calculated(a) => a,
            Bounds::Desired(a) => a,
            Bounds::RangeFromDesiredToMin(a, _) => a,
            _ => unreachable!("Must not happen, every uncalculated bound must be set in prepare_and_validate function"),
        }
    }

    pub fn meets_the_boundaries(&self, rhs: &Self) -> bool {
        use Bounds::*;
        match (
            self,
            Option::<&Balance>::from(self),
            Option::<&Balance>::from(rhs),
        ) {
            (Min(a), _, Some(b)) => a <= b,
            (Max(a), _, Some(b)) => a >= b,
            (RangeFromDesiredToMin(a, b), _, Some(c)) => a >= c && c <= b,
            (_, Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    #[allow(dead_code)]
    fn meets_the_boundaries_mutally(&self, rhs: &Self) -> bool {
        self.meets_the_boundaries(rhs) || rhs.meets_the_boundaries(self)
    }
}

impl<Balance> From<Bounds<Balance>> for Option<Balance> {
    fn from(bounds: Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
            Bounds::RangeFromDesiredToMin(a, _) => Some(a),
            _ => None,
        }
    }
}

impl<'a, Balance> From<&'a Bounds<Balance>> for Option<&'a Balance> {
    fn from(bounds: &'a Bounds<Balance>) -> Self {
        match bounds {
            Bounds::Calculated(a) => Some(a),
            Bounds::Desired(a) => Some(a),
            Bounds::RangeFromDesiredToMin(a, _) => Some(a),
            _ => None,
        }
    }
}
