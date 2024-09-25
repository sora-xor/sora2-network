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

use crate::{AssetId32, Balance, PredefinedAssetId, TechAssetId};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::dispatch::DispatchError;
use frame_support::weights::{
    WeightToFeeCoefficient, WeightToFeeCoefficients, WeightToFeePolynomial,
};
use orml_traits::parameter_type_with_key;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use smallvec::smallvec;
use sp_arithmetic::Perbill;
use sp_runtime::AccountId32;
use sp_std::convert::TryFrom;

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
    scale_info::TypeInfo,
    MaxEncodedLen,
)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
#[repr(u8)]
pub enum ComicAssetId {
    GoldenTicket,
    AppleTree,
    Apple,
    Teapot,
    Flower,
    RedPepper,
    BlackPepper,
    AcmeSpyKit,
    BatteryForMusicPlayer,
    MusicPlayer,
    Headphones,
    GreenPromise,
    BluePromise,
    Mango,
    MichaelJacksonCD,
    JesterMarotte,
    CrackedBrassBell,
    Tomato,
    Potato,
    Table,
    Future,
}

impl crate::traits::IsRepresentation for ComicAssetId {
    fn is_representation(&self) -> bool {
        false
    }
}

impl From<PredefinedAssetId> for AssetId32<ComicAssetId> {
    fn from(asset: PredefinedAssetId) -> Self {
        let comic = ComicAssetId::from(asset);
        AssetId32::<ComicAssetId>::from(comic)
    }
}

impl From<PredefinedAssetId> for ComicAssetId {
    fn from(asset_id: PredefinedAssetId) -> Self {
        use ComicAssetId::*;
        // only conversion; the `asset_id`'s place of construction must receive the warnings
        #[allow(deprecated)]
        match asset_id {
            PredefinedAssetId::XOR => GoldenTicket,
            PredefinedAssetId::DOT => AppleTree,
            PredefinedAssetId::KSM => Apple,
            PredefinedAssetId::USDT => Teapot,
            PredefinedAssetId::VAL => Flower,
            PredefinedAssetId::PSWAP => RedPepper,
            PredefinedAssetId::DAI => BlackPepper,
            PredefinedAssetId::ETH => AcmeSpyKit,
            PredefinedAssetId::XSTUSD => Mango,
            PredefinedAssetId::XST => BatteryForMusicPlayer,
            PredefinedAssetId::KEN => JesterMarotte,
            PredefinedAssetId::TBCD => MichaelJacksonCD,
            PredefinedAssetId::KUSD => CrackedBrassBell,
            PredefinedAssetId::KGOLD => Tomato,
            PredefinedAssetId::KXOR => Potato,
            PredefinedAssetId::KARMA => Table,
        }
    }
}

impl Default for ComicAssetId {
    fn default() -> Self {
        Self::GoldenTicket
    }
}

// This is never used, and just makes some tests compatible.
impl From<AssetId32<PredefinedAssetId>> for AssetId32<ComicAssetId> {
    fn from(_asset: AssetId32<PredefinedAssetId>) -> Self {
        unreachable!()
    }
}

// This is never used, and just makes some tests compatible.
impl From<TechAssetId<PredefinedAssetId>> for PredefinedAssetId {
    fn from(_tech: TechAssetId<PredefinedAssetId>) -> Self {
        unimplemented!()
    }
}

// This is never used, and just makes some tests compatible.
impl TryFrom<PredefinedAssetId> for TechAssetId<TechAssetId<PredefinedAssetId>>
where
    TechAssetId<PredefinedAssetId>: Decode,
{
    type Error = DispatchError;
    fn try_from(_asset: PredefinedAssetId) -> Result<Self, Self::Error> {
        unimplemented!()
    }
}

impl From<PredefinedAssetId> for TechAssetId<ComicAssetId> {
    fn from(asset_id: PredefinedAssetId) -> Self {
        TechAssetId::Wrapped(ComicAssetId::from(asset_id))
    }
}

pub struct WeightToFixedFee;

impl WeightToFeePolynomial for WeightToFixedFee {
    type Balance = Balance;

    fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
        smallvec!(WeightToFeeCoefficient {
            coeff_integer: 7_000_000,
            coeff_frac: Perbill::zero(),
            negative: false,
            degree: 1,
        })
    }
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId32<PredefinedAssetId>| -> Balance {
        0
    };
}
pub struct GetTradingPairRestrictedFlag;

impl<T> orml_traits::get_by_key::GetByKey<T, bool> for GetTradingPairRestrictedFlag {
    fn get(_key: &T) -> bool {
        false
    }
}

parameter_type_with_key! {
    pub GetChameleonPools: |base: AssetId32<PredefinedAssetId>| -> Option<(AssetId32<PredefinedAssetId>, sp_std::collections::btree_set::BTreeSet<AssetId32<PredefinedAssetId>>)> {
        if *base == crate::XOR {
            Some((crate::KXOR, [crate::ETH].into_iter().collect()))
        } else {
            None
        }
    };
}

pub fn alice() -> AccountId32 {
    AccountId32::from([1; 32])
}

pub fn bob() -> AccountId32 {
    AccountId32::from([2; 32])
}

pub fn charlie() -> AccountId32 {
    AccountId32::from([3; 32])
}

/// Mock of pallet `assets::Config`.
#[macro_export]
macro_rules! mock_assets_config {
    ($runtime:ty) => {
        parameter_types! {
            pub GetBuyBackAccountId: AccountId = AccountId32::from([23; 32]);
            pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![];
            pub const GetBuyBackPercentage: u8 = 0;
            pub GetBuyBackDexId: DEXId = DEXId::from(common::DEXId::PolkaswapXSTUSD);
        }
        impl assets::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
            type ExtraAccountId = [u8; 32];
            type ExtraAssetRecordArg =
                common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
            type AssetId = AssetId;
            type GetBaseAssetId = GetBaseAssetId;
            type GetBuyBackAssetId = GetBuyBackAssetId;
            type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
            type GetBuyBackPercentage = GetBuyBackPercentage;
            type GetBuyBackAccountId = GetBuyBackAccountId;
            type GetBuyBackDexId = GetBuyBackDexId;
            type BuyBackLiquidityProxy = ();
            type Currency = currencies::Pallet<$runtime>;
            type GetTotalBalance = ();
            type WeightInfo = ();
            type AssetRegulator = permissions::Pallet<$runtime>;
        }
    };
}

/// Mock of pallet `pallet_balances::Config`.
#[macro_export]
macro_rules! mock_pallet_balances_config {
    ($runtime:ty) => {
        parameter_types! {
            pub const ExistentialDeposit: u128 = 0;
        }
        impl pallet_balances::Config for $runtime {
            type Balance = Balance;
            type DustRemoval = ();
            type RuntimeEvent = RuntimeEvent;
            type ExistentialDeposit = ExistentialDeposit;
            type AccountStore = System;
            type WeightInfo = ();
            type MaxLocks = ();
            type MaxReserves = ();
            type ReserveIdentifier = ();
        }
    };
}

/// Mock of pallet `common::Config`.
#[macro_export]
macro_rules! mock_common_config {
    ($runtime:ty) => {
        impl common::Config for $runtime {
            type DEXId = DEXId;
            type LstId = common::LiquiditySourceType;
            type MultiCurrency = currencies::Pallet<$runtime>;
            type AssetManager = assets::Pallet<$runtime>;
        }
    };
}

/// Mock of pallet `currencies::Config`.
#[macro_export]
macro_rules! mock_currencies_config {
    ($runtime:ty) => {
        impl currencies::Config for $runtime {
            type MultiCurrency = Tokens;
            type NativeCurrency = BasicCurrencyAdapter<$runtime, Balances, Amount, BlockNumber>;
            type GetNativeCurrencyId = <$runtime as assets::Config>::GetBaseAssetId;
            type WeightInfo = ();
        }
    };
}

/// Mock of pallet `frame_system::Config`.
#[macro_export]
macro_rules! mock_frame_system_config {
    ($runtime:ty) => {
        impl frame_system::Config for $runtime {
            type BaseCallFilter = frame_support::traits::Everything;
            type BlockWeights = ();
            type BlockLength = ();
            type RuntimeOrigin = RuntimeOrigin;
            type RuntimeCall = RuntimeCall;
            type Index = u64;
            type BlockNumber = u64;
            type Hash = H256;
            type Hashing = BlakeTwo256;
            type AccountId = AccountId;
            type Lookup = IdentityLookup<Self::AccountId>;
            type Header = Header;
            type RuntimeEvent = RuntimeEvent;
            type BlockHashCount = frame_support::traits::ConstU64<250>;
            type DbWeight = ();
            type Version = ();
            type PalletInfo = PalletInfo;
            type AccountData = pallet_balances::AccountData<Balance>;
            type OnNewAccount = ();
            type OnKilledAccount = ();
            type SystemWeightInfo = ();
            type SS58Prefix = ();
            type OnSetCode = ();
            type MaxConsumers = frame_support::traits::ConstU32<65536>;
        }
    };
}

/// Mock of pallet `permissions::Config`.
#[macro_export]
macro_rules! mock_permissions_config {
    ($runtime:ty) => {
        impl permissions::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
        }
    };
}

/// Mock of pallet `technical::Config`.
#[macro_export]
macro_rules! mock_technical_config {
    ($runtime:ty) => {
        impl technical::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
            type TechAssetId = TechAssetId;
            type TechAccountId = TechAccountId;
            type Trigger = ();
            type Condition = ();
            type SwapAction = ();
            type AssetInfoProvider = assets::Pallet<$runtime>;
        }
    };
    ($runtime:ty, $swap_action:ty) => {
        impl technical::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
            type TechAssetId = TechAssetId;
            type TechAccountId = TechAccountId;
            type Trigger = ();
            type Condition = ();
            type SwapAction = $swap_action;
            type AssetInfoProvider = assets::Pallet<$runtime>;
        }
    };
}

/// Mock of pallet `tokens::Config`.
#[macro_export]
macro_rules! mock_tokens_config {
    ($runtime:ty) => {
        impl tokens::Config for $runtime {
            type RuntimeEvent = RuntimeEvent;
            type Balance = Balance;
            type Amount = Amount;
            type CurrencyId = <$runtime as assets::Config>::AssetId;
            type WeightInfo = ();
            type ExistentialDeposits = ExistentialDeposits;
            type CurrencyHooks = ();
            type MaxLocks = ();
            type MaxReserves = ();
            type ReserveIdentifier = ();
            type DustRemovalWhitelist = Everything;
        }
    };
}

/// Mock of pallet `pallet_timestamp::Config`.
#[macro_export]
macro_rules! mock_pallet_timestamp_config {
    ($runtime:ty) => {
        parameter_types! {
            pub const MinimumPeriod: u64 = 5;
        }
        impl pallet_timestamp::Config for $runtime {
            type Moment = Moment;
            type OnTimestampSet = ();
            type MinimumPeriod = MinimumPeriod;
            type WeightInfo = ();
        }
    };
}
