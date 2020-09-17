#![cfg_attr(not(feature = "std"), no_std)]

use sp_arithmetic::FixedU128;

mod primitives;
mod traits;

use blake2_rfc;
use codec::Encode;
use sp_core::hash::H512;
//use twox_hash;

pub use primitives::*;
pub use traits::*;

/// Basic type representing asset.
pub type Asset<T, GetAssetId> = currencies::Currency<T, GetAssetId>;

/// Basic type representing assets quantity.
pub type Fixed = FixedU128;

/// Type definition representing financial basis points (1bp is 0.01%)
pub type BasisPoints = u16;

/// Check if value belongs valid range of basis points, 0..10000 corresponds to 0.01%..100.00%.
/// Returns true if range is valid, false otherwise.
pub fn in_basis_points_range<T: Into<u16>>(value: T) -> bool {
    match value.into() {
        0..=10000 => true,
        _ => false,
    }
}

pub fn hash<T: Encode>(val: &T) -> H512 {
    H512::from_slice(blake2_rfc::blake2b::blake2b(64, &[], &val.encode()).as_bytes())
}
