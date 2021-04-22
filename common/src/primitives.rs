// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::traits::{IsRepresentation, PureOrWrapped};
use codec::{Decode, Encode};
use core::fmt::Debug;
use frame_support::dispatch::DispatchError;
use frame_support::{ensure, RuntimeDebug};
use rustc_hex::{FromHex, ToHex};
#[cfg(feature = "std")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_core::H256;
use sp_std::convert::{TryFrom, TryInto};
use sp_std::fmt::Display;
use sp_std::marker::PhantomData;
#[cfg(feature = "std")]
use sp_std::str::FromStr;
use sp_std::vec::Vec;
use static_assertions::_core::fmt::Formatter;
#[cfg(feature = "std")]
#[allow(unused)]
use std::fmt;

pub type Balance = u128;

/// Wrapper type which extends Balance serialization, used for json in RPC's.
#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq)]
pub struct BalanceWrapper(pub Balance);

impl From<Balance> for BalanceWrapper {
    fn from(balance: Balance) -> Self {
        BalanceWrapper(balance)
    }
}

impl From<BalanceWrapper> for Balance {
    fn from(wrapper: BalanceWrapper) -> Self {
        wrapper.0
    }
}

#[cfg(feature = "std")]
impl Serialize for BalanceWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self.0))
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for BalanceWrapper {
    fn deserialize<D>(deserializer: D) -> Result<BalanceWrapper, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let inner = Balance::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))?;
        Ok(BalanceWrapper(inner))
    }
}

/// Information about state of particular DEX.
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DEXInfo<AssetId> {
    /// AssetId of Base Asset in DEX.
    pub base_asset_id: AssetId,
    /// Determines if DEX can be managed by regular users.
    pub is_public: bool,
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
pub enum PredefinedAssetId {
    XOR = 0,
    DOT = 1,
    KSM = 2,
    USDT = 3,
    VAL = 4,
    PSWAP = 5,
    DAI = 6,
    ETH = 7,
}

pub const XOR: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::XOR);
pub const DOT: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::DOT);
pub const KSM: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::KSM);
pub const USDT: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::USDT);
pub const VAL: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::VAL);
pub const PSWAP: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::PSWAP);
pub const DAI: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::DAI);
pub const ETH: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::ETH);

impl IsRepresentation for PredefinedAssetId {
    fn is_representation(&self) -> bool {
        false
    }
}

impl Default for PredefinedAssetId {
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

// LstId is Liquidity Source Type Id.
impl<AssetId> From<TechAssetId<AssetId>> for Option<AssetId> {
    fn from(a: TechAssetId<AssetId>) -> Option<AssetId> {
        match a {
            TechAssetId::Wrapped(a) => Some(a),
            _ => None,
        }
    }
}

// LstId is Liquidity Source Type Id.
impl<AssetId> From<TechAssetId<AssetId>> for Result<AssetId32<AssetId>, ()>
where
    TechAssetId<AssetId>: Encode,
    AssetId: IsRepresentation,
    AssetId32<AssetId>: From<TechAssetId<AssetId>>,
{
    fn from(tech_asset: TechAssetId<AssetId>) -> Self {
        Ok(tech_asset.into())
    }
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

    pub const fn from_asset_id(asset_id: PredefinedAssetId) -> Self {
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

impl<AssetId> From<AssetId32<AssetId>> for AssetId32Code {
    fn from(compat: AssetId32<AssetId>) -> Self {
        compat.code
    }
}

impl<AssetId: Default> Default for AssetId32<AssetId>
where
    AssetId32<AssetId>: From<TechAssetId<AssetId>>,
{
    fn default() -> Self {
        AssetId32::<AssetId>::from(TechAssetId::Wrapped(AssetId::default()))
    }
}

// LstId is Liquidity Source Type Id.
impl<AssetId> TryFrom<AssetId32<AssetId>> for TechAssetId<AssetId>
where
    TechAssetId<AssetId>: Decode,
{
    type Error = DispatchError;
    fn try_from(compat: AssetId32<AssetId>) -> Result<Self, Self::Error> {
        let can_fail = || {
            let code = compat.code;
            let end = (code[0] as usize) + 1;
            ensure!(end < 32, "Invalid format");
            let mut frag: &[u8] = &code[1..end];
            TechAssetId::<AssetId>::decode(&mut frag)
        };
        match can_fail() {
            Ok(v) => Ok(v),
            Err(_) => Ok(TechAssetId::<AssetId>::Escaped(compat.code)),
        }
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
pub const DEFAULT_BALANCE_PRECISION: BalancePrecision = crate::FIXED_PRECISION as u8;

#[derive(Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetSymbol(pub Vec<u8>);

#[cfg(feature = "std")]
impl Serialize for AssetSymbol {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for AssetSymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

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

#[derive(Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetName(pub Vec<u8>);

#[cfg(feature = "std")]
impl Serialize for AssetName {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for AssetName {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(|str_err| serde::de::Error::custom(str_err))
    }
}

#[cfg(feature = "std")]
impl FromStr for AssetName {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<u8> = s.chars().map(|un| un as u8).collect();
        Ok(AssetName(chars))
    }
}

#[cfg(feature = "std")]
impl Display for AssetName {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s: String = self.0.iter().map(|un| *un as char).collect();
        write!(f, "{}", s)
    }
}

impl Default for AssetName {
    fn default() -> Self {
        Self(Vec::new())
    }
}

#[derive(Encode, Decode, Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone, Hash)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechAssetId<AssetId> {
    Wrapped(AssetId),
    Escaped(AssetId32Code),
}

#[derive(Encode, Decode, Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone, Hash)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum AssetIdExtraAssetRecordArg<DEXId, LstId, AccountId> {
    DEXId(DEXId),
    LstId(LstId),
    AccountId(AccountId),
}

impl<AssetId: Default> Default for TechAssetId<AssetId> {
    fn default() -> Self {
        TechAssetId::Wrapped(AssetId::default())
    }
}

impl<AssetId> From<AssetId> for TechAssetId<AssetId> {
    fn from(a: AssetId) -> Self {
        TechAssetId::Wrapped(a)
    }
}

/// Enumaration of all available liquidity sources.
#[derive(Encode, Decode, RuntimeDebug, PartialEq, Eq, Copy, Clone, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum LiquiditySourceType {
    XYKPool,
    BondingCurvePool,
    MulticollateralBondingCurvePool,
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum ManagementMode {
    /// All functions can be managed with this mode.
    Private,
    /// Functions checked as public can be managed with this mode.
    Public,
}

impl Default for ManagementMode {
    fn default() -> Self {
        Self::Private
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

// LstId is Liquidity Source Type Id.
impl<AssetId> PureOrWrapped<AssetId> for TechAssetId<AssetId> {
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
    FeeCollectorForPair(TradingPair<AssetId>),
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
impl<AccountId, AssetId, DEXId> IsRepresentation for TechAccountId<AccountId, AssetId, DEXId> {
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

impl<AccountId, AssetId: Clone, DEXId: Clone> crate::traits::ToFeeAccount
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn to_fee_account(&self) -> Option<Self> {
        match self {
            TechAccountId::Pure(dex, purpose) => match purpose {
                TechPurpose::LiquidityKeeper(tpair) => Some(TechAccountId::Pure(
                    dex.clone(),
                    TechPurpose::FeeCollectorForPair(tpair.clone()),
                )),
                _ => None,
            },
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
    AccountId: IsRepresentation,
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
    > PureOrWrapped<AccountId> for TechAccountId<AccountId, AssetId, DEXId>
where
    AccountId: IsRepresentation,
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
    AssetId32<AssetId>: From<TechAssetId<AssetId>>,
    AssetId: IsRepresentation,
{
    fn from(asset_id: AssetId) -> Self {
        // Must be not representation, only direct asset must be here.
        // Assert must exist here because it must never happen in runtime and must be covered by tests.
        assert!(!asset_id.is_representation());
        AssetId32::<AssetId>::from(TechAssetId::Wrapped(asset_id))
    }
}

// LstId is Liquidity Source Type Id.
impl<AssetId> From<TechAssetId<AssetId>> for AssetId32<AssetId>
where
    TechAssetId<AssetId>: Encode,
    AssetId: IsRepresentation,
{
    fn from(tech_asset: TechAssetId<AssetId>) -> Self {
        match tech_asset {
            TechAssetId::Escaped(code) => AssetId32::new(code, PhantomData),
            _ => {
                let mut slice = [0_u8; 32];
                let asset_encoded: Vec<u8> = tech_asset.encode();
                let asset_length = asset_encoded.len();
                // Encode size of TechAssetId must be always less or equal to 31.
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
                slice[1..asset_length + 1].copy_from_slice(&asset_encoded);
                AssetId32::new(slice, PhantomData)
            }
        }
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

/// Reason for particular reward during swap.
#[derive(Encode, Decode, Eq, PartialEq, Clone, Copy, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RewardReason {
    /// Reason is unknown.
    Unspecified,
    /// Buying XOR with collateral tokens (except PSWAP and VAL) is rewarded.
    BuyOnBondingCurve,
    /// Providing liquidyty on secondary market is rewarded.
    LiquidityProvisionFarming,
    /// High volume trading is rewarded.
    MarketMakerVolume,
}

impl Default for RewardReason {
    fn default() -> Self {
        Self::Unspecified
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Default)]
pub struct PswapRemintInfo {
    pub liquidity_providers: Balance,
    pub parliament: Balance,
    pub vesting: Balance,
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
            serde_json::from_str::<AssetId32<PredefinedAssetId>>(json_str).unwrap(),
            asset_id
        );

        // should not panic
        serde_json::to_value(&asset_id).unwrap();
    }

    #[test]
    fn should_serialize_and_deserialize_balance_properly_with_string() {
        let balance: Balance = 123_456u128;
        let wrapper: BalanceWrapper = balance.into();

        let json_str = r#""123456""#;

        assert_eq!(serde_json::to_string(&wrapper).unwrap(), json_str);
        let unwrapped: Balance = serde_json::from_str::<BalanceWrapper>(json_str)
            .unwrap()
            .into();
        assert_eq!(unwrapped, balance);

        // should not panic
        serde_json::to_value(&BalanceWrapper(balance)).unwrap();
    }
}
