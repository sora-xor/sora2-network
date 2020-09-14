use crate::BasisPoints;
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

/// Information about state of particular DEX.
#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DEXInfo<AccountId, AssetId> {
    /// AccountId of owner which can manage DEX.
    pub owner_account_id: AccountId,
    /// AssetId of Base Asset in DEX.
    pub base_asset_id: AssetId,
    /// Default value for fee in basis points.
    pub default_fee: BasisPoints,
    /// Default value for protocol fee in basis points.
    pub default_protocol_fee: BasisPoints,
}

/// Trading pair data.
#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TradingPair<AssetId> {
    /// Base token of exchange.
    pub base_asset_id: AssetId,
    /// Target token of exchange.
    pub target_asset_id: AssetId,
}

/// Asset identifier.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AssetId {
    XOR = 0,
    DOT = 1,
    KSM = 2,
}

impl Default for AssetId {
    fn default() -> Self {
        Self::XOR
    }
}
