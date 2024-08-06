use crate::utils;
use frame_support::__private::RuntimeDebug;
use frame_support::pallet_prelude::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::testing::H256;
use frame_support::{Deserialize, Serialize};
use rustc_hex::ToHex;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub const ASSET_ID_PREFIX_PREDEFINED: u8 = 2;

pub type OrderId = u128;
pub type DEXId = u32;
pub type Balance = u128;

/// This code is H256 like.
pub type AssetId32Code = [u8; 32];

/// This is wrapped structure, this is like H256 or Р512, extra
/// PhantomData is added for typing reasons.
#[derive(
    Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, scale_info::TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetId32 {
    /// Internal data representing given AssetId.
    pub code: AssetId32Code,
}

// More readable representation of AssetId
impl core::fmt::Debug for AssetId32 {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_tuple("AssetId")
            .field(&H256::from(self.code))
            .finish()
    }
}

impl FromStr for AssetId32 {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vec: Vec<u8> = utils::parse_hex_string(s).ok_or("error parsing hex string")?;
        let code: [u8; 32] = vec
            .try_into()
            .map_err(|_| "expected hex string representing 32-byte object")?;
        Ok(AssetId32 { code })
    }
}

impl Display for AssetId32 {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        write!(f, "0x{}", self.code.to_hex::<String>())
    }
}

impl AssetId32 {
    pub const fn new(code: AssetId32Code) -> Self {
        Self { code }
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { code: bytes }
    }

    // pub const fn from_asset_id(asset_id: PredefinedAssetId) -> Self {
    //     let mut bytes = [0u8; 32];
    //     bytes[0] = ASSET_ID_PREFIX_PREDEFINED;
    //     bytes[2] = asset_id as u8;
    //     Self::from_bytes(bytes)
    // }
}

#[derive(
    Encode, Decode, PartialEq, Eq, Copy, Clone, RuntimeDebug, scale_info::TypeInfo, MaxEncodedLen,
)]
pub enum PriceVariant {
    Buy,
    Sell,
}

impl PriceVariant {
    pub fn switched(&self) -> Self {
        match self {
            PriceVariant::Buy => PriceVariant::Sell,
            PriceVariant::Sell => PriceVariant::Buy,
        }
    }
}

#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    Debug,
    Hash,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OrderBookId<AssetId, DEXId> {
    /// DEX id
    pub dex_id: DEXId,
    /// Base asset.
    pub base: AssetId,
    /// Quote asset. It should be a base asset of DEX.
    pub quote: AssetId,
}
