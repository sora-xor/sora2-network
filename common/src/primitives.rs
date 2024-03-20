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
use codec::{Decode, Encode, MaxEncodedLen};
use core::{fmt::Debug, str::FromStr};
use frame_support::dispatch::TypeInfo;
use frame_support::traits::ConstU32;
use frame_support::{ensure, BoundedVec, RuntimeDebug};
use hex_literal::hex;
use sp_core::H256;
use sp_runtime::traits::Get;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;
use static_assertions::_core::cmp::Ordering;

use crate::{Fixed, IsValid};
#[cfg(feature = "std")]
use {
    rustc_hex::ToHex,
    serde::{Deserialize, Serialize},
    sp_std::convert::TryInto,
    sp_std::fmt::Display,
    static_assertions::_core::fmt::Formatter,
};

pub type Balance = u128;

/// Max length of asset content source. The same value as IE URL length. It should enough for any URI / IPFS address (CID)
pub const ASSET_CONTENT_SOURCE_MAX_LENGTH: usize = 2048;

/// Max length of asset description, it should be enough to describe everything the user wants
pub const ASSET_DESCRIPTION_MAX_LENGTH: usize = 512;

/// Wrapper type which extends Balance serialization, used for json in RPC's.
#[derive(Encode, Decode, Debug, Clone, PartialEq, Eq, scale_info::TypeInfo)]
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

/// Information about state of particular DEX.
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, Default, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct DEXInfo<AssetId> {
    /// AssetId of Base Asset in DEX.
    pub base_asset_id: AssetId,
    /// AssetId of synthetic base Asset in DEX.
    pub synthetic_base_asset_id: AssetId,
    /// Determines if DEX can be managed by regular users.
    pub is_public: bool,
}

//TODO: consider replacing base_asset_id with dex_id, and getting base asset from dex
/// Trading pair data.
#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    RuntimeDebug,
    Hash,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct TradingPair<AssetId> {
    /// Base token of exchange.
    pub base_asset_id: AssetId,
    /// Target token of exchange.
    pub target_asset_id: AssetId,
}

impl<AssetId: Eq> TradingPair<AssetId> {
    pub fn consists_of(&self, asset_id: &AssetId) -> bool {
        &self.base_asset_id == asset_id || &self.target_asset_id == asset_id
    }
}

impl<T> TradingPair<T> {
    pub fn map<U, F: Fn(T) -> U>(self, f: F) -> TradingPair<U> {
        TradingPair {
            base_asset_id: f(self.base_asset_id),
            target_asset_id: f(self.target_asset_id),
        }
    }
}

/// Asset identifier.
#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
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
    XSTUSD = 8,
    XST = 9,
    TBCD = 10,
}

pub const XOR: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::XOR);
pub const DOT: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::DOT);
pub const KSM: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::KSM);
pub const USDT: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::USDT);
pub const VAL: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::VAL);
pub const PSWAP: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::PSWAP);
pub const DAI: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::DAI);
pub const ETH: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::ETH);
pub const XSTUSD: AssetId32<PredefinedAssetId> =
    AssetId32::from_asset_id(PredefinedAssetId::XSTUSD);
pub const XST: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::XST);
pub const TBCD: AssetId32<PredefinedAssetId> = AssetId32::from_asset_id(PredefinedAssetId::TBCD);
pub const CERES_ASSET_ID: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
    "008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"
));
pub const DEMETER_ASSET_ID: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
    "00f2f4fda40a4bf1fc3769d156fa695532eec31e265d75068524462c0b80f674"
));
pub const HERMES_ASSET_ID: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
    "002d4e9e03f192cc33b128319a049f353db98fbf4d98f717fd0b7f66a0462142"
));
pub const APOLLO_ASSET_ID: AssetId32<PredefinedAssetId> = AssetId32::from_bytes(hex!(
    "00c9594f342106df38447209a6bfa8bf99742f652f20b9cb508219c2ac567982"
));

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
#[derive(
    Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, scale_info::TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetId32<AssetId> {
    /// Internal data representing given AssetId.
    pub code: AssetId32Code,
    /// Additional typing information.
    pub phantom: PhantomData<AssetId>,
}

// More readable representation of AssetId
impl<AssetId> core::fmt::Debug for AssetId32<AssetId>
where
    AssetId: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_tuple("AssetId")
            .field(&H256::from(self.code))
            .finish()
    }
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
impl<AssetId> FromStr for AssetId32<AssetId> {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vec: Vec<u8> = crate::utils::parse_hex_string(s).ok_or("error parsing hex string")?;
        let code: [u8; 32] = vec
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

    /// Construct asset id for synthetic asset using its `reference_symbol`
    pub fn from_synthetic_reference_symbol<Symbol>(reference_symbol: &Symbol) -> Self
    where
        Symbol: From<SymbolName> + PartialEq + Encode,
    {
        if *reference_symbol == SymbolName::usd().into() {
            return Self::from_asset_id(PredefinedAssetId::XSTUSD);
        }

        let mut bytes = [0u8; 32];
        let symbol_bytes = reference_symbol.encode();
        let symbol_hash = sp_io::hashing::blake2_128(&symbol_bytes);
        bytes[0] = 3;
        bytes[2..18].copy_from_slice(&symbol_hash);

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
impl<AssetId> From<AssetId32<AssetId>> for TechAssetId<AssetId>
where
    TechAssetId<AssetId>: Decode,
{
    fn from(compat: AssetId32<AssetId>) -> Self {
        let can_fail = || {
            let code = compat.code;
            let end = (code[0] as usize) + 1;
            ensure!(end < 32, "Invalid format");
            let mut frag: &[u8] = &code[1..end];
            TechAssetId::<AssetId>::decode(&mut frag)
        };
        match can_fail() {
            Ok(v) => v,
            Err(_) => TechAssetId::<AssetId>::Escaped(compat.code),
        }
    }
}

/// DEX identifier.
#[derive(
    Encode,
    Decode,
    Eq,
    PartialEq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    RuntimeDebug,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum DEXId {
    Polkaswap = 0,
    PolkaswapXSTUSD = 1,
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

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Hash))]
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
        Self(b"TEST".to_vec())
    }
}

const ASSET_SYMBOL_MAX_LENGTH: usize = 7;

impl IsValid for AssetSymbol {
    /// According to UTF-8 encoding, graphemes that start with byte 0b0XXXXXXX belong
    /// to ASCII range and are of single byte, therefore passing check in range 'A' to 'Z'
    /// and '0' to '9' guarantees that all graphemes are of length 1, therefore length check is valid.
    fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= ASSET_SYMBOL_MAX_LENGTH
            && self
                .0
                .iter()
                .all(|byte| (b'A'..=b'Z').contains(&byte) || (b'0'..=b'9').contains(&byte))
    }
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct AssetName(pub Vec<u8>);

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
        Self(b"Test".to_vec())
    }
}

const ASSET_NAME_MAX_LENGTH: usize = 33;

impl IsValid for AssetName {
    /// According to UTF-8 encoding, graphemes that start with byte 0b0XXXXXXX belong
    /// to ASCII range and are of single byte, therefore passing check in range 'A' to 'z'
    /// guarantees that all graphemes are of length 1, therefore length check is valid.
    fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= ASSET_NAME_MAX_LENGTH
            && self.0.iter().all(|byte| {
                (b'A'..=b'Z').contains(&byte)
                    || (b'a'..=b'z').contains(&byte)
                    || (b'0'..=b'9').contains(&byte)
                    || byte == &b' '
            })
    }
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct ContentSource(pub Vec<u8>);

#[cfg(feature = "std")]
impl FromStr for ContentSource {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<u8> = s.chars().map(|un| un as u8).collect();
        Ok(ContentSource(chars))
    }
}

#[cfg(feature = "std")]
impl Display for ContentSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s: String = self.0.iter().map(|un| *un as char).collect();
        write!(f, "{}", s)
    }
}

impl Default for ContentSource {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl IsValid for ContentSource {
    fn is_valid(&self) -> bool {
        self.0.is_ascii() && self.0.len() <= ASSET_CONTENT_SOURCE_MAX_LENGTH
    }
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct Description(pub Vec<u8>);

#[cfg(feature = "std")]
impl FromStr for Description {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<u8> = s.chars().map(|un| un as u8).collect();
        Ok(Description(chars))
    }
}

#[cfg(feature = "std")]
impl Display for Description {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s: String = self.0.iter().map(|un| *un as char).collect();
        write!(f, "{}", s)
    }
}

impl Default for Description {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl IsValid for Description {
    fn is_valid(&self) -> bool {
        self.0.len() <= ASSET_DESCRIPTION_MAX_LENGTH
    }
}

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Hash))]
pub struct SymbolName(pub Vec<u8>);

impl SymbolName {
    pub fn usd() -> Self {
        Self::from_str("USD").expect("`USD` is a valid symbol name")
    }
}

impl FromStr for SymbolName {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars: Vec<u8> = s.chars().map(|un| un as u8).collect();
        Ok(SymbolName(chars))
    }
}

#[cfg(feature = "std")]
impl Display for SymbolName {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s: String = self.0.iter().map(|un| *un as char).collect();
        write!(f, "{}", s)
    }
}

impl Default for SymbolName {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl IsValid for SymbolName {
    /// Same as for AssetSymbol
    fn is_valid(&self) -> bool {
        !self.0.is_empty()
            && self.0.len() <= ASSET_SYMBOL_MAX_LENGTH
            && self
                .0
                .iter()
                .all(|byte| (b'A'..=b'Z').contains(&byte) || (b'0'..=b'9').contains(&byte))
    }
}

const CROWDLOAN_TAG_MAX_LENGTH: u32 = 128;

#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Ord, PartialOrd, RuntimeDebug, scale_info::TypeInfo,
)]
pub struct CrowdloanTag(pub BoundedVec<u8, ConstU32<CROWDLOAN_TAG_MAX_LENGTH>>);

#[cfg(feature = "std")]
impl FromStr for CrowdloanTag {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chars = s
            .chars()
            .map(|un| un as u8)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "CrowdloanTag length out of bounds")?;
        Ok(CrowdloanTag(chars))
    }
}

#[cfg(feature = "std")]
impl Display for CrowdloanTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> sp_std::fmt::Result {
        let s: String = self.0.iter().map(|un| *un as char).collect();
        write!(f, "{}", s)
    }
}

impl Default for CrowdloanTag {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl IsValid for CrowdloanTag {
    /// Same as for AssetSymbol
    fn is_valid(&self) -> bool {
        !self.0.is_empty() && self.0.is_ascii()
    }
}

#[derive(
    Encode, Decode, Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone, Hash, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechAssetId<AssetId> {
    Wrapped(AssetId),
    Escaped(AssetId32Code),
}

#[derive(
    Encode, Decode, Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone, Hash, scale_info::TypeInfo,
)]
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
#[derive(
    Encode,
    Decode,
    RuntimeDebug,
    PartialEq,
    Eq,
    Copy,
    Clone,
    PartialOrd,
    Ord,
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
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
    XSTPool,

    #[cfg(feature = "ready-to-test")] // order-book
    OrderBook,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
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
#[derive(
    Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, PartialOrd, Ord, scale_info::TypeInfo,
)]
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
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum TechPurpose<AssetId> {
    FeeCollector = 0,
    FeeCollectorForPair(TradingPair<AssetId>) = 1,
    XykLiquidityKeeper(TradingPair<AssetId>) = 2,
    Identifier(Vec<u8>) = 3,
    OrderBookLiquidityKeeper(TradingPair<AssetId>) = 4,
}

/// Enum encoding of technical account id, pure and wrapped records.
/// Enum record `WrappedRepr` is wrapped represention of `Pure` variant of enum, this is useful then
/// representation is known but backward mapping is not known.
#[derive(Encode, Decode, Eq, PartialEq, Clone, PartialOrd, Ord, Debug, scale_info::TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum TechAccountId<AccountId, AssetId, DEXId> {
    Pure(DEXId, TechPurpose<AssetId>),
    /// First field is used as name or tag of binary format, second field is used as binary data.
    Generic(Vec<u8>, Vec<u8>),
    Wrapped(AccountId),
    WrappedRepr(AccountId),
    None,
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

impl<AccountId, AssetId, DEXId> Default for TechAccountId<AccountId, AssetId, DEXId> {
    fn default() -> Self {
        TechAccountId::None
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
                TechPurpose::XykLiquidityKeeper(tpair) => Some(TechAccountId::Pure(
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
    crate::traits::ToXykTechUnitFromDEXAndTradingPair<DEXId, TradingPair<AssetId>>
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn to_xyk_tech_unit_from_dex_and_trading_pair(
        dex_id: DEXId,
        trading_pair: TradingPair<AssetId>,
    ) -> Self {
        TechAccountId::Pure(dex_id, TechPurpose::XykLiquidityKeeper(trading_pair))
    }
}

impl<AccountId, AssetId, DEXId: Clone>
    crate::traits::ToOrderTechUnitFromDEXAndTradingPair<DEXId, TradingPair<AssetId>>
    for TechAccountId<AccountId, AssetId, DEXId>
{
    fn to_order_tech_unit_from_dex_and_trading_pair(
        dex_id: DEXId,
        trading_pair: TradingPair<AssetId>,
    ) -> Self {
        TechAccountId::Pure(dex_id, TechPurpose::OrderBookLiquidityKeeper(trading_pair))
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

impl From<InvokeRPCError> for i32 {
    fn from(item: InvokeRPCError) -> i32 {
        match item {
            InvokeRPCError::RuntimeError => 1,
        }
    }
}

/// Reason for particular reward during swap.
#[derive(
    Encode, Decode, Eq, PartialEq, Clone, Copy, PartialOrd, Ord, Debug, scale_info::TypeInfo,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum RewardReason {
    /// Reason is unknown.
    Unspecified,
    /// Buying XOR with collateral tokens (except PSWAP and VAL) is rewarded.
    BuyOnBondingCurve,
    /// Providing liquidity on secondary market is rewarded.
    LiquidityProvisionFarming,
    /// DEPRECATED: High volume trading is rewarded.
    DeprecatedMarketMakerVolume,
    /// Crowdloan reward.
    Crowdloan,
}

impl Default for RewardReason {
    fn default() -> Self {
        Self::Unspecified
    }
}

#[derive(Encode, Decode, Clone, RuntimeDebug, Default, scale_info::TypeInfo)]
pub struct PswapRemintInfo {
    pub liquidity_providers: Balance,
    pub buy_back_tbcd: Balance,
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

/// List of available oracles
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Oracle {
    BandChainFeed,
}

/// Information about received oracle symbol (price and last update time)
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Copy, Clone, PartialEq, Eq)]
pub struct Rate {
    pub value: Balance,
    pub last_updated: u64,
    pub dynamic_fee: Fixed,
}

#[derive(Encode, MaxEncodedLen, Default, TypeInfo)]
#[scale_info(skip_type_params(N))]
pub struct BoundedString<N: Get<u32>>(BoundedVec<u8, N>);

impl<N: Get<u32>> BoundedString<N> {
    pub fn truncate_from(data: &str) -> Self {
        Self(BoundedVec::truncate_from(data.as_bytes().to_vec()))
    }
}

impl<N: Get<u32>> codec::Decode for BoundedString<N> {
    fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
        let inner = BoundedVec::<u8, N>::decode(input)?;
        core::str::from_utf8(&inner).map_err(|_| "Invalid UTF-8 string")?;
        Ok(Self(inner))
    }
}

impl<N: Get<u32>> Clone for BoundedString<N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<N: Get<u32>> PartialEq for BoundedString<N> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<N: Get<u32>> Eq for BoundedString<N> {}

impl<N: Get<u32>> Debug for BoundedString<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Ok(s) = core::str::from_utf8(&self.0) {
            write!(f, "{:?}", s)
        } else {
            write!(f, "<invalid utf8 string>")
        }
    }
}

impl<N: Get<u32>> TryFrom<&str> for BoundedString<N> {
    type Error = ();
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(value.as_bytes().to_vec().try_into().map_err(|_| ())?))
    }
}

impl<N: Get<u32>> PartialOrd for BoundedString<N> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<N: Get<u32>> Ord for BoundedString<N> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}
