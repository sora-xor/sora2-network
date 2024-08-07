pub const ASSET_ID_PREFIX_PREDEFINED: u8 = 2;

pub type OrderId = u128;
pub type DEXId = u32;
pub type Balance = u128;

/// This code is H256 like.
pub type AssetId32Code = [u8; 32];

/// This is wrapped structure, this is like H256 or Р512, extra
/// PhantomData is added for typing reasons.
#[derive(Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetId32 {
    /// Internal data representing given AssetId.
    pub code: AssetId32Code,
}

impl AssetId32 {
    pub const fn new(code: AssetId32Code) -> Self {
        Self { code }
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { code: bytes }
    }

    // TODO: should use?
    // pub const fn from_asset_id(asset_id: PredefinedAssetId) -> Self {
    //     let mut bytes = [0u8; 32];
    //     bytes[0] = ASSET_ID_PREFIX_PREDEFINED;
    //     bytes[2] = asset_id as u8;
    //     Self::from_bytes(bytes)
    // }
}

#[derive(PartialEq, Eq, Copy, Clone)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
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

#[derive(Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug, Hash)]
#[ink::scale_derive(Encode, Decode, TypeInfo)]
pub struct OrderBookId<AssetId, DEXId> {
    /// DEX id
    pub dex_id: DEXId,
    /// Base asset.
    pub base: AssetId,
    /// Quote asset. It should be a base asset of DEX.
    pub quote: AssetId,
}
