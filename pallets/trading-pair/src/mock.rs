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

use crate::{self as trading_pair, Config};
use common::mock::ExistentialDeposits;
use common::prelude::{Balance, DEXInfo};
use common::{
    hash, AssetId32, AssetName, AssetSymbol, BalancePrecision, ContentSource, Description,
    DEFAULT_BALANCE_PRECISION, DOT, KSM, PSWAP, VAL, XOR, XST, XSTUSD,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Zero};
use sp_runtime::{BuildStorage, Perbill};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime! {
    pub enum Runtime {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
    }
}

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i128;

pub const ALICE: AccountId = 1;
pub const BUY_BACK_ACCOUNT: AccountId = 23;
pub const DEX_ID: DEXId = 0;
type AssetId = AssetId32<common::PredefinedAssetId>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Nonce = u64;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
}

impl tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
}

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

type DEXId = u32;

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = XST;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = BUY_BACK_ACCOUNT;
    pub const GetBuyBackDexId: DEXId = 0;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = AccountId;
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, AccountId>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type GetBuyBackAssetId = GetBuyBackAssetId;
    type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
    type GetBuyBackPercentage = GetBuyBackPercentage;
    type GetBuyBackAccountId = GetBuyBackAccountId;
    type GetBuyBackDexId = GetBuyBackDexId;
    type BuyBackLiquidityProxy = ();
    type Currency = currencies::Pallet<Runtime>;
    type GetTotalBalance = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type RuntimeHoldReason = ();
    type FreezeIdentifier = ();
    type MaxHolds = ();
    type MaxFreezes = ();
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl dex_manager::Config for Runtime {}

pub struct ExtBuilder {
    endowed_assets: Vec<(
        AssetId,
        AccountId,
        AssetSymbol,
        AssetName,
        BalancePrecision,
        Balance,
        bool,
        Option<ContentSource>,
        Option<Description>,
    )>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl ExtBuilder {
    pub fn without_initialized_dex() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    DOT,
                    ALICE,
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"Polkadot".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    KSM,
                    ALICE,
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![],
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
    }
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_assets: vec![
                (
                    XOR,
                    ALICE,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    DOT,
                    ALICE,
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"Polkadot".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    KSM,
                    ALICE,
                    AssetSymbol(b"KSM".to_vec()),
                    AssetName(b"Kusama".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
                (
                    XSTUSD,
                    ALICE,
                    AssetSymbol(b"XSTUSD".to_vec()),
                    AssetName(b"XSTUSD".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::zero(),
                    true,
                    None,
                    None,
                ),
            ],
            endowed_accounts: vec![],
            dex_list: vec![
                (
                    DEX_ID,
                    DEXInfo {
                        base_asset_id: XOR,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
                (
                    1,
                    DEXInfo {
                        base_asset_id: XSTUSD,
                        synthetic_base_asset_id: XST,
                        is_public: true,
                    },
                ),
            ],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![ALICE]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_ID)), vec![ALICE]),
            ],
            initial_permissions: vec![
                (ALICE, Scope::Unlimited, vec![INIT_DEX]),
                (ALICE, Scope::Limited(hash(&DEX_ID)), vec![MANAGE_DEX]),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Runtime>::default()
            .build_storage()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(ALICE, 1)],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self.endowed_assets,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
