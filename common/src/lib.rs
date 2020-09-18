#![cfg_attr(not(feature = "std"), no_std)]

use sp_arithmetic::FixedU128;

mod fixed_wrapper;
mod primitives;
mod traits;

pub mod mock;

use blake2_rfc;
use codec::Encode;
use sp_core::hash::H512;
//use twox_hash;

pub use traits::Trait;
pub mod prelude {
    pub use super::fixed_wrapper::*;
    pub use super::primitives::*;
    pub use super::traits::*;
    pub use super::Fixed;
}
use sp_core::crypto::AccountId32;

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

/// This data is used as prefix in AccountId32, if it is representative for TechAccId encode twox
/// hash (128 + 128 = 256 bit of AccountId32 for example).
pub const TECH_ACCOUNT_MAGIC_PREFIX: [u8; 16] = [
    84, 115, 79, 144, 249, 113, 160, 44, 96, 155, 45, 104, 78, 97, 181, 87,
];

impl IsRepresentation for AccountId32 {
    fn is_repr(&self) -> bool {
        let b: [u8; 32] = self.clone().into();
        b[0..16] == TECH_ACCOUNT_MAGIC_PREFIX
    }
}
