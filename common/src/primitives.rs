use crate::traits::Trait;
use crate::BasisPoints;
use codec::{Decode, Encode};
use frame_support::dispatch::DispatchError;
use frame_support::RuntimeDebug;
use frame_support::{decl_error, decl_module};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::convert::TryFrom;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Liquidity source can't exchange assets with the given IDs on the given DEXId.
        CantExchange,
        /// Assets can't be swapped or exchanged with the given method.
        UnsupportedSwapMethod,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
    }
}

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
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, RuntimeDebug, Hash)]
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
    VAL = 4,
}

impl crate::traits::IsRepresentation for AssetId {
    fn is_representation(&self) -> bool {
        false
    }
}

impl Default for AssetId {
    fn default() -> Self {
        Self::XOR
    }
}

/// This code is H256 like.
type JsonCompatCode = [u8; 32];

/// This is wrapped structure, this is like H256 or Р512, and for this reason tuple is used here.
/// H256 uses [u8; 32] wrapped in single tuple, and this structure is like this, but extra
/// PhantomData is added for typing reasons.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
pub struct JsonCompatAssetId<AssetId>(pub JsonCompatCode, pub PhantomData<AssetId>);

#[allow(dead_code)]
impl<AssetId: Clone> JsonCompatAssetId<AssetId>
where
    Result<TechAssetId<AssetId, DEXId>, codec::Error>: From<JsonCompatAssetId<AssetId>>,
{
    fn try_from_code(code: JsonCompatCode) -> Result<Self, codec::Error> {
        let json_compat = JsonCompatAssetId(code, PhantomData);
        Result::<TechAssetId<AssetId, DEXId>, codec::Error>::from(json_compat.clone())
            .map(|_| json_compat)
    }
}

impl<AssetId> From<JsonCompatAssetId<AssetId>> for JsonCompatCode {
    fn from(json_compat: JsonCompatAssetId<AssetId>) -> Self {
        json_compat.0
    }
}

impl<AssetId: Default> Default for JsonCompatAssetId<AssetId>
where
    JsonCompatAssetId<AssetId>: From<TechAssetId<AssetId, DEXId>>,
{
    fn default() -> Self {
        JsonCompatAssetId::<AssetId>::from(TechAssetId::Wrapped(AssetId::default()))
    }
}

impl<AssetId, DEXId> TryFrom<JsonCompatAssetId<AssetId>> for TechAssetId<AssetId, DEXId>
where
    TechAssetId<AssetId, DEXId>: Decode,
{
    type Error = DispatchError;
    fn try_from(json_compat: JsonCompatAssetId<AssetId>) -> Result<Self, Self::Error> {
        let code = json_compat.0;
        let end = (code[0] as usize) + 1;
        if end >= 32 {
            return Err("Invalid format".into());
        }
        let mut frag: &[u8] = &code[1..end];
        TechAssetId::<AssetId, DEXId>::decode(&mut frag).map_err(|e| e.what().into())
    }
}

/// DEX identifier.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum DEXId {
    Polkaswap = 0,
}

impl From<DEXId> for u32 {
    fn from(dex_id: DEXId) -> Self {
        dex_id as u32
    }
}

impl Default for DEXId {
    fn default() -> Self {
        DEXId::Polkaswap
    }
}

/// Technical asset ID.
/// A special type of asset, DEX marker, is used to obtain legal units for providing liquidity, as
/// well as the ability to implement these legal units. These are conditionally exchange markers on
/// liquidity.
#[derive(Encode, Decode, Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone, Hash)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum MakeTechAssetId<AssetId, DEXId, ShallowerAssetId> {
    Wrapped(AssetId),
    DexMarker(DEXId, TradingPair<ShallowerAssetId>),
}

pub type TechAssetId<A, D> =
    MakeTechAssetId<A, D, MakeTechAssetId<A, D, MakeTechAssetId<A, D, ()>>>;

impl<AssetId: Clone, DEXId: Clone>
    crate::traits::ToTechUnitFromDEXAndTradingPair<DEXId, TradingPair<TechAssetId<AssetId, DEXId>>>
    for TechAssetId<AssetId, DEXId>
{
    fn to_tech_unit_from_dex_and_trading_pair(
        dex_id: DEXId,
        trading_pair: TradingPair<TechAssetId<AssetId, DEXId>>,
    ) -> Self {
        use MakeTechAssetId::*;
        match (trading_pair.base_asset_id, trading_pair.target_asset_id) {
            (Wrapped(a), Wrapped(b)) => {
                let tp = TradingPair {
                    base_asset_id: Wrapped(a),
                    target_asset_id: Wrapped(b),
                };
                TechAssetId::DexMarker(dex_id.clone(), tp)
            }
            _ => unimplemented!(),
        }
    }
}

impl<AssetId: Clone, DEXId: Clone>
    crate::traits::ToTechUnitFromDEXAndTradingPair<DEXId, TradingPair<AssetId>>
    for TechAssetId<AssetId, DEXId>
{
    fn to_tech_unit_from_dex_and_trading_pair(
        dex_id: DEXId,
        trading_pair: TradingPair<AssetId>,
    ) -> Self {
        use MakeTechAssetId::*;
        match (
            trading_pair.clone().base_asset_id,
            trading_pair.clone().target_asset_id,
        ) {
            (a, b) => {
                let tp = TradingPair {
                    base_asset_id: Wrapped(a),
                    target_asset_id: Wrapped(b),
                };
                TechAssetId::DexMarker(dex_id.clone(), tp)
            }
        }
    }
}

impl<AssetId: Default, DEXId> Default for TechAssetId<AssetId, DEXId> {
    fn default() -> Self {
        TechAssetId::Wrapped(AssetId::default())
    }
}

impl<AssetId, DEXId> From<AssetId> for TechAssetId<AssetId, DEXId> {
    fn from(a: AssetId) -> Self {
        TechAssetId::Wrapped(a)
    }
}

impl<DEXId> From<AssetId> for TechAssetId<crate::mock::ComicAssetId, DEXId> {
    fn from(asset_id: AssetId) -> Self {
        TechAssetId::Wrapped(crate::mock::ComicAssetId::from(asset_id))
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
    BondingCurvePool,
    MockPool,
    MockPool2,
    MockPool3,
    MockPool4,
}

/// Identification of liquidity source.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LiquiditySourceId<DEXId: Copy, LiquiditySourceIndex: Copy> {
    /// Identification of target DEX.
    pub dex_id: DEXId,
    /// Index value to distinguish particular liquidity source, e.g. index in array or enum-type.
    pub liquidity_source_index: LiquiditySourceIndex,
}

impl<DEXId: Copy, LiquiditySourceIndex: Copy> LiquiditySourceId<DEXId, LiquiditySourceIndex> {
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
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechPurpose<AssetId> {
    FeeCollector,
    LiquidityKeeper(TradingPair<AssetId>),
    Identifier(Vec<u8>),
}

/// Enum encoding of technical account id, pure and wrapped records.
/// Enum record `WrappedRepr` is wrapped represention of `Pure` variant of enum, this is useful then
/// representation is known but backward mapping is not known.
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechAccountId<AccountId, AssetId, DEXId> {
    Pure(DEXId, TechPurpose<AssetId>),
    /// First field is used as name or tag of binary format, second field is used as binary data.
    Generic(Vec<u8>, Vec<u8>),
    Wrapped(AccountId),
    WrappedRepr(AccountId),
}

/// Implementation of `IsRepresentation` for `TechAccountId`, because is has `WrappedRepr`.
impl<AccountId, AssetId, DEXId> crate::traits::IsRepresentation
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn is_representation(&self) -> bool {
        match self {
            TechAccountId::WrappedRepr(_) => true,
            _ => false,
        }
    }
}

/// Implementation of `FromGenericPair` for cases when trait method is better than data type
/// constructor.
impl<AccountId, AssetId, DEXId> crate::traits::FromGenericPair
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn from_generic_pair(tag: Vec<u8>, data: Vec<u8>) -> Self {
        TechAccountId::Generic(tag, data)
    }
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

impl<AccountId, AssetId, DEXId: Clone> crate::traits::ToFeeAccount
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn to_fee_account(&self) -> Option<Self> {
        match self {
            TechAccountId::Pure(dex, _) => {
                Some(TechAccountId::Pure(dex.clone(), TechPurpose::FeeCollector))
            }
            _ => None,
        }
    }
}

impl<AccountId, AssetId: Clone, DEXId: Clone>
    crate::traits::ToMarkerAsset<TechAssetId<AssetId, DEXId>>
    for TechAccountId<AccountId, TechAssetId<AssetId, DEXId>, DEXId>
{
    fn to_marker_asset(&self) -> Option<TechAssetId<AssetId, DEXId>> {
        use MakeTechAssetId::*;
        match self {
            TechAccountId::Pure(dex, TechPurpose::LiquidityKeeper(tpair)) => {
                match (tpair.clone().base_asset_id, tpair.clone().target_asset_id) {
                    (Wrapped(base_asset_id), Wrapped(target_asset_id)) => {
                        let trading_pair = TradingPair {
                            base_asset_id: Wrapped(base_asset_id),
                            target_asset_id: Wrapped(target_asset_id),
                        };
                        Some(TechAssetId::DexMarker(dex.clone(), trading_pair))
                    }
                    //TODO: will be implemented for cases like pool token of pool token.
                    _ => unimplemented!(),
                }
            }
            _ => None,
        }
    }
}

impl<AccountId, AssetId, DEXId: Clone>
    crate::traits::ToTechUnitFromDEXAndTradingPair<DEXId, TradingPair<AssetId>>
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn to_tech_unit_from_dex_and_trading_pair(
        dex_id: DEXId,
        trading_pair: TradingPair<AssetId>,
    ) -> Self {
        TechAccountId::Pure(dex_id.clone(), TechPurpose::LiquidityKeeper(trading_pair))
    }
}

impl<AccountId, AssetId, DEXId> From<AccountId> for TechAccountId<AccountId, AssetId, DEXId>
where
    AccountId: crate::traits::IsRepresentation,
{
    fn from(a: AccountId) -> Self {
        if a.is_representation() {
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
            TechAccountId::WrappedRepr(a) => Some(a),
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
