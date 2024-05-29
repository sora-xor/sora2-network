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

use currencies::BasicCurrencyAdapter;
use frame_support::traits::{Everything, GenesisBuild, OnFinalize, OnInitialize};
use frame_support::weights::{RuntimeDbWeight, Weight};
use frame_support::{construct_runtime, parameter_types};
use hex_literal::hex;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify};
use sp_runtime::{MultiSignature, Perbill, Percent};

use common::mock::ExistentialDeposits;
use common::prelude::{Balance, OnValBurned};
use common::{
    self, balance, mock_pallet_balances_config, mock_technical_config, Amount, AssetId32,
    AssetName, AssetSymbol, TechPurpose, DEFAULT_BALANCE_PRECISION, PSWAP, VAL, XOR, XST,
};
use permissions::{Scope, BURN, MINT};

use crate::{self as rewards, Config};

pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

type DEXId = common::DEXId;
type Signature = MultiSignature;
type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type AssetId = AssetId32<common::PredefinedAssetId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn alice() -> AccountId32 {
    AccountId32::from([1u8; 32])
}

pub fn tech_account_id() -> TechAccountId {
    TechAccountId::Pure(
        DEXId::Polkaswap,
        TechPurpose::Identifier(b"rewards_tech_account_id".to_vec()),
    )
}

pub fn account_id() -> AccountId {
    Technical::tech_account_id_to_account_id(&tech_account_id()).unwrap()
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = XOR;
    pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight {
        read: 100,
        write: 1000,
    };
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Rewards: rewards::{Pallet, Call, Config<T>, Storage, Event<T>},
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
    }
}

impl Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumber = 20;
    const UPDATE_FREQUENCY: BlockNumber = 5;
    const MAX_CHUNK_SIZE: usize = 1;
    const MAX_VESTING_RATIO: Percent = Percent::from_percent(55);
    const TIME_TO_SATURATION: BlockNumber = 100;
    const VAL_BURN_PERCENT: Percent = Percent::from_percent(3);
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
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
    type BlockHashCount = BlockHashCount;
    type DbWeight = DbWeight;
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

mock_technical_config!(Runtime);

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = XST;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, [u8; 32]>;
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
    type AssetRegulator = permissions::Pallet<Runtime>;
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
}

// Required by assets::Config
impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

// Required by assets::Config
impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

// Required by currencies::Config
mock_pallet_balances_config!(Runtime);

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

pub struct ExtBuilder {
    with_rewards: bool,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self { with_rewards: true }
    }
}

impl ExtBuilder {
    pub fn with_rewards(with_rewards: bool) -> Self {
        Self { with_rewards }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        let tech_account_id = tech_account_id();
        let account_id: AccountId = account_id();

        BalancesConfig {
            balances: vec![(account_id.clone(), balance!(150)), (alice(), balance!(0))],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![(BURN, Scope::Unlimited, vec![account_id.clone()])],
            initial_permissions: vec![(account_id.clone(), Scope::Unlimited, vec![MINT, BURN])],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        AssetsConfig {
            endowed_assets: vec![
                (
                    PSWAP,
                    alice(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    VAL.into(),
                    alice(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            balances: vec![
                (account_id.clone(), VAL.into(), balance!(30000)),
                (account_id.clone(), PSWAP.into(), balance!(1000)),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TechnicalConfig {
            register_tech_accounts: vec![(account_id, tech_account_id.clone())],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let (val_owners, pswap_farm_owners, pswap_waifu_owners) = if self.with_rewards {
            (
                vec![
                    (
                        hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                        (balance!(111), balance!(1000)).into(),
                    ),
                    (
                        hex!("d170a274320333243b9f860e8891c6792de1ec19").into(),
                        (balance!(2888.99), balance!(20000)).into(),
                    ),
                    (
                        hex!("886021f300dc809269cfc758a2364a2baf63af0c").into(),
                        (balance!(0.01), balance!(0.1)).into(),
                    ),
                ],
                vec![(
                    hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                    balance!(222),
                )],
                vec![
                    (
                        hex!("21Bc9f4a3d9Dc86f142F802668dB7D908cF0A636").into(),
                        balance!(333),
                    ),
                    (
                        hex!("d170a274320333243b9f860e8891c6792de1ec19").into(),
                        balance!(100),
                    ),
                    (
                        hex!("886021f300dc809269cfc758a2364a2baf63af0c").into(),
                        balance!(10000),
                    ),
                ],
            )
        } else {
            (vec![], vec![], vec![])
        };
        RewardsConfig {
            reserves_account_id: tech_account_id,
            val_owners,
            pswap_farm_owners,
            pswap_waifu_owners,
            umi_nfts: vec![PSWAP.into()],
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
        Rewards::on_initialize(System::block_number());
        Rewards::on_val_burned(balance!(10));
    }
}
