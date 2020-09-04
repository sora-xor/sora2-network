#![cfg_attr(not(feature = "std"), no_std)]

use sp_arithmetic::FixedU128;

mod primitives;
mod traits;

pub use primitives::*;
pub use traits::*;

pub type Fixed = FixedU128;
