use crate::BasisPoints;
use codec::{Decode, Encode};
use frame_support::RuntimeDebug;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Information about state of particular DEX.
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DEXInfo<AssetId> {
    /// AssetId of Base Asset in DEX.
    pub base_asset_id: AssetId,
    /// Default value for fee in basis points.
    pub default_fee: BasisPoints,
    /// Default value for protocol fee in basis points.
    pub default_protocol_fee: BasisPoints,
}

//TODO: consider replacing base_asset_id with dex_id, and getting base asset from dex
/// Trading pair data.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TradingPair<AssetId> {
    /// Base token of exchange.
    pub base_asset_id: AssetId,
    /// Target token of exchange.
    pub target_asset_id: AssetId,
}

/// Asset identifier.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum AssetId {
    XOR = 0,
    DOT = 1,
    KSM = 2,
    USD = 3,
}

impl Default for AssetId {
    fn default() -> Self {
        Self::XOR
    }
}

/// Technical asset ID.
/// A special type of asset, DEX marker, is used to obtain legal units for providing liquidity, as
/// well as the ability to implement these legal units. These are conditionally exchange markers on
/// liquidity.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechAssetId<AssetId, DexId> {
    Wrapped(AssetId),
    DexMarker(DexId, TradingPair<AssetId>),
}

impl<AssetId, DEXId> From<AssetId> for TechAssetId<AssetId, DEXId> {
    fn from(a: AssetId) -> Self {
        TechAssetId::Wrapped(a)
    }
}

impl<AssetId, DEXId> From<TechAssetId<AssetId, DEXId>> for Option<AssetId> {
    fn from(a: TechAssetId<AssetId, DEXId>) -> Option<AssetId> {
        match a {
            TechAssetId::Wrapped(a) => Some(a),
            _ => None,
        }
    }
}

/// Enumaration of all available liquidity sources.
#[derive(Encode, Decode, RuntimeDebug, PartialEq, Eq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum LiquiditySourceType {
    XYKPool,
    MockPool,
}

/// Identification of liquidity source.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LiquiditySourceId<DEXId, LiquiditySourceIndex> {
    /// Identification of target DEX.
    pub dex_id: DEXId,
    /// Index value to distinguish particular liquidity source, e.g. index in array or enum-type.
    pub liquidity_source_index: LiquiditySourceIndex,
}

impl<DEXId, LiquiditySourceIndex> LiquiditySourceId<DEXId, LiquiditySourceIndex> {
    pub fn new(dex_id: DEXId, liquidity_source_index: LiquiditySourceIndex) -> Self {
        Self {
            dex_id,
            liquidity_source_index,
        }
    }
}

impl<AssetId, DEXId> crate::traits::PureOrWrapped<AssetId> for TechAssetId<AssetId, DEXId> {
    fn is_pure(&self) -> bool {
        match self {
            TechAssetId::Wrapped(_) => false,
            _ => true,
        }
    }

    fn is_wrapped(&self) -> bool {
        match self {
            TechAssetId::Wrapped(_) => true,
            _ => false,
        }
    }

    fn is_wrapped_regular(&self) -> bool {
        match self {
            TechAssetId::Wrapped(_) => true,
            _ => false,
        }
    }
}

/// Code of purpose for technical account.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechPurpose<AssetId> {
    FeeCollector,
    LiquidityKeeper(TradingPair<AssetId>),
}

/// Enum encoding of technical account id, pure and wrapped records.
/// Enum record `WrappedRepr` is wrapped represention of `Pure` variant of enum, this is useful then
/// representation is known but backward mapping is not known.
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechAccountId<AccountId, AssetId, DEXId> {
    Pure(DEXId, TechPurpose<AssetId>),
    /// First field is used as name or tag of binary format, second field is used as binary data.
    Generic(sp_std::vec::Vec<u8>, sp_std::vec::Vec<u8>),
    Wrapped(AccountId),
    WrappedRepr(AccountId),
}

impl<AccountId: Default, AssetId, DEXId> Default for TechAccountId<AccountId, AssetId, DEXId> {
    fn default() -> Self {
        TechAccountId::Wrapped(AccountId::default())
    }
}

impl<AccountId, AssetId, DEXId> crate::traits::WrappedRepr<AccountId>
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn wrapped_repr(repr: AccountId) -> Self {
        TechAccountId::WrappedRepr(repr)
    }
}

impl<AccountId, AssetId, DEXId> From<AccountId> for TechAccountId<AccountId, AssetId, DEXId>
where
    AccountId: crate::traits::IsRepresentation,
{
    fn from(a: AccountId) -> Self {
        if a.is_repr() {
            TechAccountId::Wrapped(a)
        } else {
            TechAccountId::WrappedRepr(a)
        }
    }
}

impl<AccountId, AssetId, DEXId> From<TechAccountId<AccountId, AssetId, DEXId>>
    for Option<AccountId>
{
    fn from(a: TechAccountId<AccountId, AssetId, DEXId>) -> Option<AccountId> {
        match a {
            TechAccountId::Wrapped(a) => Some(a),
            _ => None,
        }
    }
}

impl<
        AccountId: Clone + Encode + From<[u8; 32]> + Into<[u8; 32]>,
        AssetId: Encode,
        DEXId: Encode,
    > crate::traits::PureOrWrapped<AccountId> for TechAccountId<AccountId, AssetId, DEXId>
where
    AccountId: crate::traits::IsRepresentation,
{
    fn is_pure(&self) -> bool {
        match self {
            TechAccountId::Pure(_, _) => true,
            TechAccountId::Generic(_, _) => true,
            _ => false,
        }
    }
    fn is_wrapped_regular(&self) -> bool {
        match self {
            TechAccountId::Wrapped(_) => true,
            _ => false,
        }
    }
    fn is_wrapped(&self) -> bool {
        match self {
            TechAccountId::Pure(_, _) => false,
            TechAccountId::Generic(_, _) => false,
            _ => true,
        }
    }
}
