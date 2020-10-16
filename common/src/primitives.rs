use crate::traits::Trait;
use crate::BasisPoints;
use codec::{Decode, Encode};
use core::fmt::Debug;
use frame_support::dispatch::DispatchError;
use frame_support::RuntimeDebug;
use frame_support::{decl_error, decl_module};
use rustc_hex::{FromHex, ToHex};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_core::H256;
use sp_std::convert::TryFrom;
use sp_std::convert::TryInto;
use sp_std::fmt::Display;
use sp_std::marker::PhantomData;
#[cfg(feature = "std")]
use sp_std::str::FromStr;
use sp_std::vec::Vec;
use static_assertions::_core::fmt::Formatter;
#[cfg(feature = "std")]
#[allow(unused)]
use std::fmt;

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
    PSWAP = 5,
}

pub const XOR: AssetId32<AssetId> = AssetId32::from_asset_id(AssetId::XOR);
pub const DOT: AssetId32<AssetId> = AssetId32::from_asset_id(AssetId::DOT);
pub const KSM: AssetId32<AssetId> = AssetId32::from_asset_id(AssetId::KSM);
pub const USD: AssetId32<AssetId> = AssetId32::from_asset_id(AssetId::USD);
pub const VAL: AssetId32<AssetId> = AssetId32::from_asset_id(AssetId::VAL);
pub const PSWAP: AssetId32<AssetId> = AssetId32::from_asset_id(AssetId::PSWAP);

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
pub type AssetId32Code = [u8; 32];

/// This is wrapped structure, this is like H256 or Р512, extra
/// PhantomData is added for typing reasons.
#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetId32<AssetId> {
    /// Internal data representing given AssetId.
    pub code: AssetId32Code,
    /// Additional typing information.
    pub phantom: PhantomData<AssetId>,
}

#[cfg(feature = "std")]
impl<AssetId> Serialize for AssetId32<AssetId> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

#[cfg(feature = "std")]
impl<'de, AssetId> Deserialize<'de> for AssetId32<AssetId> {
    fn deserialize<D>(deserializer: D) -> Result<AssetId32<AssetId>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        AssetId32::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

#[cfg(feature = "std")]
impl<AssetId> FromStr for AssetId32<AssetId> {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.to_owned();
        if s.starts_with("0x") {
            s = (&s[2..]).to_owned();
        } else {
            return Err("expected hex string, e.g. 0x00..00");
        }
        let code: Vec<u8> = s.from_hex().map_err(|_| "error parsing hex string")?;
        let code: [u8; 32] = code
            .try_into()
            .map_err(|_| "expected hex string representing 32-byte object")?;
        Ok(AssetId32 {
            code,
            phantom: PhantomData,
        })
    }
}

#[cfg(feature = "std")]
impl<AssetId> Display for AssetId32<AssetId> {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        write!(f, "0x{}", self.code.to_hex::<String>())
    }
}

impl<AssetId> AssetId32<AssetId> {
    pub const fn new(code: AssetId32Code, phantom: PhantomData<AssetId>) -> Self {
        Self { code, phantom }
    }

    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            code: bytes,
            phantom: PhantomData,
        }
    }

    pub const fn from_asset_id(asset_id: super::AssetId) -> Self {
        let mut bytes = [0u8; 32];
        bytes[0] = 2;
        bytes[2] = asset_id as u8;
        Self::from_bytes(bytes)
    }
}

impl<AssetId> From<H256> for AssetId32<AssetId> {
    fn from(value: H256) -> Self {
        AssetId32::<AssetId>::new(value.0, Default::default())
    }
}

impl<AssetId> From<AssetId32<AssetId>> for H256 {
    fn from(value: AssetId32<AssetId>) -> H256 {
        H256(value.code)
    }
}

#[allow(dead_code)]
impl<AssetId: Clone> AssetId32<AssetId>
where
    Result<TechAssetId<AssetId, DEXId>, codec::Error>: From<AssetId32<AssetId>>,
{
    fn try_from_code(code: AssetId32Code) -> Result<Self, codec::Error> {
        let compat = AssetId32::new(code, PhantomData);
        Result::<TechAssetId<AssetId, DEXId>, codec::Error>::from(compat.clone()).map(|_| compat)
    }
}

impl<AssetId> From<AssetId32<AssetId>> for AssetId32Code {
    fn from(compat: AssetId32<AssetId>) -> Self {
        compat.code
    }
}

impl<AssetId: Default> Default for AssetId32<AssetId>
where
    AssetId32<AssetId>: From<TechAssetId<AssetId, DEXId>>,
{
    fn default() -> Self {
        AssetId32::<AssetId>::from(TechAssetId::Wrapped(AssetId::default()))
    }
}

impl<AssetId, DEXId> TryFrom<AssetId32<AssetId>> for TechAssetId<AssetId, DEXId>
where
    TechAssetId<AssetId, DEXId>: Decode,
{
    type Error = DispatchError;
    fn try_from(compat: AssetId32<AssetId>) -> Result<Self, Self::Error> {
        let code = compat.code;
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

pub type BalancePrecision = u8;

#[derive(Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
pub struct AssetSymbol(pub Vec<u8>);

#[cfg(feature = "std")]
impl FromStr for AssetSymbol {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<u8> = s.chars().map(|un| un as u8).collect();
        Ok(AssetSymbol(chars))
    }
}

#[cfg(feature = "std")]
impl Display for AssetSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s: String = self.0.iter().map(|un| *un as char).collect();
        write!(f, "{}", s)
    }
}

impl Default for AssetSymbol {
    fn default() -> Self {
        Self(Vec::new())
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
#[repr(u8)]
pub enum LiquiditySourceType {
    XYKPool,
    BondingCurvePool,
    MockPool,
    MockPool2,
    MockPool3,
    MockPool4,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum FilterMode {
    /// Filter is disabled, all items regardless of filter are included.
    Disabled,
    /// Only selected items are filtered out, rest will be included.
    ForbidSelected,
    /// Only selected items will be included, rest are filtered out.
    AllowSelected,
}

impl Default for FilterMode {
    fn default() -> Self {
        Self::Disabled
    }
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

impl<AssetId> From<AssetId> for AssetId32<AssetId>
where
    AssetId32<AssetId>: From<TechAssetId<AssetId, DEXId>>,
    AssetId: crate::traits::IsRepresentation,
{
    fn from(asset_id: AssetId) -> Self {
        // Must be not representation, only direct asset must be here.
        // Assert must exist here because it must never happen in runtime and must be covered by tests.
        assert!(!asset_id.is_representation());
        AssetId32::<AssetId>::from(TechAssetId::Wrapped(asset_id))
    }
}

impl<AssetId, DEXId> From<TechAssetId<AssetId, DEXId>> for AssetId32<AssetId>
where
    TechAssetId<AssetId, DEXId>: Encode,
    AssetId: crate::traits::IsRepresentation,
{
    fn from(tech_asset: TechAssetId<AssetId, DEXId>) -> Self {
        let mut slice = [0_u8; 32];
        let asset_encoded: Vec<u8> = tech_asset.encode();
        let asset_length = asset_encoded.len();
        // Encode size of TechAssetId must be always less or equal to 31.
        // Recursion of MakeTechAssetId is limited for this to specific number of iterations.
        // Assert must exist here because it must never happen in runtime and must be covered by tests.
        assert!(asset_length <= 31);
        // Must be not representation, only direct asset must be here.
        // Assert must exist here because it must never happen in runtime and must be covered by tests.
        assert!({
            match tech_asset {
                TechAssetId::Wrapped(a) => !a.is_representation(),
                _ => true,
            }
        });
        slice[0] = asset_length as u8;
        for i in 0..asset_length {
            slice[i + 1] = asset_encoded[i];
        }
        AssetId32::new(slice, PhantomData)
    }
}

/// Common error which can arise while invoking particular RPC call in runtime.
pub enum InvokeRPCError {
    RuntimeError,
}

impl From<InvokeRPCError> for i64 {
    fn from(item: InvokeRPCError) -> i64 {
        match item {
            InvokeRPCError::RuntimeError => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_serialize_and_deserialize_assetid32_properly_with_string() {
        let asset_id = AssetId32 {
            code: [
                2, 0, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0, 10, 0, 11, 0, 12, 0, 13, 0, 14, 0,
                15, 0, 1, 0, 2, 0,
            ],
            phantom: PhantomData,
        };

        let json_str = r#""0x020003000400050006000700080009000a000b000c000d000e000f0001000200""#;

        assert_eq!(serde_json::to_string(&asset_id).unwrap(), json_str);
        assert_eq!(
            serde_json::from_str::<AssetId32<AssetId>>(json_str).unwrap(),
            asset_id
        );

        // should not panic
        serde_json::to_value(&asset_id).unwrap();
    }
}
