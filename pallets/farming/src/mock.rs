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

use crate::{self as farming, Config};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{balance, fixed, hash, AssetName, AssetSymbol, DEXInfo, Fixed, DOT, PSWAP, VAL, XOR};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{GenesisBuild, OnFinalize, OnInitialize};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::EnsureRoot;
use permissions::*;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::Perbill;
use sp_std::marker::PhantomData;

pub use common::mock::*;
pub use common::TechAssetId as Tas;
pub use common::TechPurpose::*;

pub type DEXId = u32;
pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AssetId = common::AssetId32<common::PredefinedAssetId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub const PSWAP_PER_DAY: Balance = balance!(2500000);
pub const REFRESH_FREQUENCY: BlockNumberFor<Runtime> = 1200;
pub const VESTING_COEFF: u32 = 3;
pub const VESTING_FREQUENCY: BlockNumberFor<Runtime> = 3600;
pub const BLOCKS_PER_DAY: BlockNumberFor<Runtime> = 14_440;

#[allow(non_snake_case)]
pub fn ALICE() -> AccountId {
    AccountId32::from([1; 32])
}

#[allow(non_snake_case)]
pub fn BOB() -> AccountId {
    AccountId32::from([2; 32])
}

#[allow(non_snake_case)]
pub fn CHARLIE() -> AccountId {
    AccountId32::from([3; 32])
}

#[allow(non_snake_case)]
pub fn DAVE() -> AccountId {
    AccountId32::from([4; 32])
}

#[allow(non_snake_case)]
pub fn EVE() -> AccountId {
    AccountId32::from([5; 32])
}

#[allow(non_snake_case)]
pub fn FERDIE() -> AccountId {
    AccountId32::from([6; 32])
}

pub const DEX_A_ID: DEXId = 0;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = common::AssetId32 { code: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
    pub const ExistentialDeposit: u128 = 0;
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetIncentiveAssetId: AssetId = common::PSWAP.into();
    pub GetParliamentAccountId: AccountId = AccountId32::from([8; 32]);
    pub RewardDoublingAssets: Vec<AssetId> = vec![VAL.into(), PSWAP.into()];
    pub GetXykFee: Fixed = fixed!(0.003);
    pub GetTeamReservesAccountId: AccountId = AccountId32::from([11; 32]);
    pub const SchedulerMaxWeight: Weight = 1024;
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        DexManager: dex_manager::{Module, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Module, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Module, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Module, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Module, Call, Config<T>, Storage, Event<T>},
        MBCPool: multicollateral_bonding_curve_pool::{Module, Call, Storage, Event<T>},
        VestedRewards: vested_rewards::{Module, Storage, Event<T>},
        Scheduler: pallet_scheduler::{Module, Call, Storage, Event<T>},

        Farming: farming::{Module, Call, Storage},
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = tokens::Module<Runtime>;
    type NativeCurrency =
        BasicCurrencyAdapter<Runtime, pallet_balances::Module<Runtime>, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type GetTeamReservesAccountId = GetTeamReservesAccountId;
    type WeightInfo = ();
}

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.007);
    type Event = Event;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type GetFee = GetXykFee;
    type OnPoolCreated = (PswapDistribution, Farming);
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = GetIncentiveAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
    type WeightInfo = ();
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXYK;
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = ();
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type PriceToolsPallet = ();
    type VestedRewardsPallet = VestedRewards;
    type WeightInfo = ();
}

impl vested_rewards::Config for Runtime {
    type Event = Event;
    type GetMarketMakerRewardsAccountId = ();
    type GetBondingCurveRewardsAccountId = ();
    type WeightInfo = ();
}

impl pallet_scheduler::Config for Runtime {
    type Event = Event;
    type Origin = Origin;
    type PalletsOrigin = OriginCaller;
    type Call = Call;
    type MaximumWeight = SchedulerMaxWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ();
    type WeightInfo = ();
}

impl Config for Runtime {
    const PSWAP_PER_DAY: Balance = PSWAP_PER_DAY;
    const REFRESH_FREQUENCY: BlockNumberFor<Self> = REFRESH_FREQUENCY;
    const VESTING_COEFF: u32 = VESTING_COEFF;
    const VESTING_FREQUENCY: BlockNumberFor<Self> = VESTING_FREQUENCY;
    const BLOCKS_PER_DAY: BlockNumberFor<Self> = BLOCKS_PER_DAY;
    type Call = Call;
    type SchedulerOriginCaller = OriginCaller;
    type Scheduler = Scheduler;
    type RewardDoublingAssets = RewardDoublingAssets;
    type WeightInfo = ();
}

pub struct ExtBuilder {
    initial_dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        let preset01 = vec![
            INIT_DEX,
            CREATE_FARM,
            LOCK_TO_FARM,
            UNLOCK_FROM_FARM,
            CLAIM_FROM_FARM,
        ];
        Self {
            initial_dex_list: vec![(
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: XOR,
                    is_public: true,
                },
            )],
            endowed_accounts: vec![
                (ALICE(), DOT, balance!(2000000)),
                (ALICE(), PSWAP, balance!(2000000)),
                (BOB(), DOT, balance!(2000000)),
                (BOB(), PSWAP, balance!(2000000)),
                (CHARLIE(), DOT, balance!(2000000)),
                (CHARLIE(), PSWAP, balance!(2000000)),
                (DAVE(), DOT, balance!(2000000)),
                (DAVE(), PSWAP, balance!(2000000)),
                (EVE(), DOT, balance!(2000000)),
                (EVE(), PSWAP, balance!(2000000)),
                (FERDIE(), DOT, balance!(2000000)),
                (FERDIE(), PSWAP, balance!(2000000)),
            ],
            initial_permission_owners: vec![
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![BOB()]),
                (CREATE_FARM, Scope::Unlimited, vec![ALICE()]),
                (LOCK_TO_FARM, Scope::Unlimited, vec![ALICE()]),
                (UNLOCK_FROM_FARM, Scope::Unlimited, vec![ALICE()]),
                (CLAIM_FROM_FARM, Scope::Unlimited, vec![ALICE()]),
            ],
            initial_permissions: vec![
                (BOB(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (ALICE(), Scope::Unlimited, preset01.clone()),
                (BOB(), Scope::Unlimited, preset01.clone()),
                (CHARLIE(), Scope::Unlimited, preset01.clone()),
                (DAVE(), Scope::Unlimited, preset01.clone()),
                (EVE(), Scope::Unlimited, preset01.clone()),
                (FERDIE(), Scope::Unlimited, preset01.clone()),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (ALICE(), balance!(99000)),
                (BOB(), balance!(99000)),
                (CHARLIE(), balance!(99000)),
                (DAVE(), balance!(99000)),
                (EVE(), balance!(99000)),
                (FERDIE(), balance!(99000)),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
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
            endowed_assets: vec![
                (
                    XOR.into(),
                    ALICE(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    0,
                    true,
                ),
                (
                    DOT.into(),
                    ALICE(),
                    AssetSymbol(b"DOT".to_vec()),
                    AssetName(b"DOT".to_vec()),
                    18,
                    0,
                    true,
                ),
                (
                    PSWAP.into(),
                    ALICE(),
                    AssetSymbol(b"PSWAP".to_vec()),
                    AssetName(b"PSWAP".to_vec()),
                    18,
                    0,
                    true,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: self.initial_dex_list,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}

pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
        Scheduler::on_initialize(System::block_number());
        Farming::on_initialize(System::block_number());
    }
}
