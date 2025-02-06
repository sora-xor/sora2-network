use crate::{
    arithmetic::bounds::Bounded,
    arithmetic::checked::{checked_pow, CheckedMul},
    arithmetic::identities::{One, Zero},
    arithmetic::saturating,
};

/// Saturating arithmetic operations, returning maximum or minimum values instead of overflowing.
pub trait Saturating {
    /// Saturating addition. Compute `self + rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    fn saturating_add(self, rhs: Self) -> Self;

    /// Saturating subtraction. Compute `self - rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    fn saturating_sub(self, rhs: Self) -> Self;

    /// Saturating multiply. Compute `self * rhs`, saturating at the numeric bounds instead of
    /// overflowing.
    fn saturating_mul(self, rhs: Self) -> Self;

    /// Saturating exponentiation. Compute `self.pow(exp)`, saturating at the numeric bounds
    /// instead of overflowing.
    fn saturating_pow(self, exp: usize) -> Self;

    /// Increment self by one, saturating.
    fn saturating_inc(&mut self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_add(One::one());
    }

    /// Decrement self by one, saturating at zero.
    fn saturating_dec(&mut self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_sub(One::one());
    }

    /// Increment self by some `amount`, saturating.
    fn saturating_accrue(&mut self, amount: Self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_add(amount);
    }

    /// Decrement self by some `amount`, saturating at zero.
    fn saturating_reduce(&mut self, amount: Self)
    where
        Self: One,
    {
        let mut o = Self::one();
        sp_std::mem::swap(&mut o, self);
        *self = o.saturating_sub(amount);
    }
}

impl<T: Clone + Zero + One + PartialOrd + CheckedMul + Bounded + saturating::Saturating> Saturating
    for T
{
    fn saturating_add(self, o: Self) -> Self {
        <Self as saturating::Saturating>::saturating_add(self, o)
    }

    fn saturating_sub(self, o: Self) -> Self {
        <Self as saturating::Saturating>::saturating_sub(self, o)
    }

    fn saturating_mul(self, o: Self) -> Self {
        self.checked_mul(&o).unwrap_or_else(|| {
            if (self < T::zero()) != (o < T::zero()) {
                Bounded::min_value()
            } else {
                Bounded::max_value()
            }
        })
    }

    fn saturating_pow(self, exp: usize) -> Self {
        let neg = self < T::zero() && exp % 2 != 0;
        checked_pow(self, exp).unwrap_or_else(|| {
            if neg {
                Bounded::min_value()
            } else {
                Bounded::max_value()
            }
        })
    }
}
