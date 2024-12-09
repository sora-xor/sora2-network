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

#![cfg(feature = "wip")] // presto

use crate as presto;

use common::mock::ExistentialDeposits;
use common::{
    mock_assets_config, mock_common_config, mock_currencies_config, mock_dex_manager_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_permissions_config, mock_technical_config, mock_tokens_config, mock_trading_pair_config,
    Amount, AssetId32, AssetName, AssetSymbol, BoundedString, DEXId, DEXInfo, FromGenericPair,
    PredefinedAssetId, DEFAULT_BALANCE_PRECISION, KUSD, PRUSD, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{ConstU32, EitherOfDiverse, GenesisBuild};
use frame_support::{construct_runtime, parameter_types};
use frame_system::{EnsureRoot, EnsureSigned};
use permissions::Scope;
use sp_runtime::AccountId32;

pub type AccountId = AccountId32;
pub type AssetId = AssetId32<PredefinedAssetId>;
type Balance = u128;
type BlockNumber = u64;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<PredefinedAssetId>;
type Block = frame_system::mocking::MockBlock<Runtime>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Moment = u64;

pub const MILLISECS_PER_BLOCK: Moment = 6000;
pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;
pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        OrderBook: order_book::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        TradingPair: trading_pair::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Presto: presto::{Pallet, Call, Storage, Event<T>},
    }
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = KUSD;
}

mock_common_config!(Runtime);
mock_assets_config!(Runtime);
mock_currencies_config!(Runtime);
mock_dex_manager_config!(Runtime);
mock_tokens_config!(Runtime);
mock_pallet_balances_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_technical_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_trading_pair_config!(Runtime);

impl order_book::Config for Runtime {
    const MAX_ORDER_LIFESPAN: Moment = 30 * (DAYS as Moment) * MILLISECS_PER_BLOCK; // 30 days = 2_592_000_000
    const MIN_ORDER_LIFESPAN: Moment = (MINUTES as Moment) * MILLISECS_PER_BLOCK; // 1 minute = 60_000
    const MILLISECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK;
    const SOFT_MIN_MAX_RATIO: usize = 1000;
    const HARD_MIN_MAX_RATIO: usize = 4000;
    const REGULAR_NUBMER_OF_EXECUTED_ORDERS: usize = 100;
    type RuntimeEvent = RuntimeEvent;
    type OrderId = u128;
    type Locker = order_book::Pallet<Runtime>;
    type Unlocker = order_book::Pallet<Runtime>;
    type Scheduler = order_book::Pallet<Runtime>;
    type Delegate = order_book::Pallet<Runtime>;
    type MaxOpenedLimitOrdersPerUser = ConstU32<1024>;
    type MaxLimitOrdersForPrice = ConstU32<1024>;
    type MaxSidePriceCount = ConstU32<1024>;
    type MaxExpiringOrdersPerBlock = ConstU32<1024>;
    type MaxExpirationWeightPerBlock = ();
    type MaxAlignmentWeightPerBlock = ();
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type SyntheticInfoProvider = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type Time = Timestamp;
    type PermittedCreateOrigin = EitherOfDiverse<EnsureSigned<AccountId>, EnsureRoot<AccountId>>;
    type PermittedEditOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
}

parameter_types! {
    pub const PrestoUsdAssetId: AssetId = PRUSD;
    pub PrestoTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            presto::TECH_ACCOUNT_PREFIX.to_vec(),
            presto::TECH_ACCOUNT_MAIN.to_vec(),
        )
    };
    pub PrestoAccountId: AccountId = {
        let tech_account_id = PrestoTechAccountId::get();
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap()
    };
    pub PrestoBufferTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            presto::TECH_ACCOUNT_PREFIX.to_vec(),
            presto::TECH_ACCOUNT_BUFFER.to_vec(),
        )
    };
    pub PrestoBufferAccountId: AccountId = {
        let tech_account_id = PrestoBufferTechAccountId::get();
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id).unwrap()
    };
}

impl presto::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type OrderBookManager = order_book::Pallet<Runtime>;
    type PrestoUsdAssetId = PrestoUsdAssetId;
    type PrestoTechAccount = PrestoTechAccountId;
    type PrestoBufferTechAccount = PrestoBufferTechAccountId;
    type RequestId = u64;
    type CropReceiptId = u64;
    type CouponId = u64;
    type MaxPrestoManagersCount = ConstU32<100>;
    type MaxPrestoAuditorsCount = ConstU32<100>;
    type MaxUserRequestCount = ConstU32<65536>;
    type MaxUserCropReceiptCount = ConstU32<65536>;
    type MaxRequestPaymentReferenceSize = ConstU32<100>;
    type MaxRequestDetailsSize = ConstU32<200>;
    type MaxPlaceOfIssueSize = ConstU32<100>;
    type MaxDebtorSize = ConstU32<80>;
    type MaxCreditorSize = ConstU32<80>;
    type MaxCropReceiptContentSize = ConstU32<30720>;
    type Time = Timestamp;
    type WeightInfo = ();
}

pub fn ext() -> sp_io::TestExternalities {
    let assets_and_permissions_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
    let assets_and_permissions_account_id =
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(
            &assets_and_permissions_tech_account_id,
        )
        .unwrap();

    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<Runtime>()
        .unwrap();

    TechnicalConfig {
        register_tech_accounts: vec![
            (PrestoAccountId::get(), PrestoTechAccountId::get()),
            (
                PrestoBufferAccountId::get(),
                PrestoBufferTechAccountId::get(),
            ),
            (
                assets_and_permissions_account_id.clone(),
                assets_and_permissions_tech_account_id,
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    PermissionsConfig {
        initial_permission_owners: vec![
            (
                permissions::MINT,
                Scope::Unlimited,
                vec![assets_and_permissions_account_id.clone()],
            ),
            (
                permissions::BURN,
                Scope::Unlimited,
                vec![assets_and_permissions_account_id.clone()],
            ),
        ],
        initial_permissions: vec![(
            assets_and_permissions_account_id.clone(),
            Scope::Unlimited,
            vec![permissions::MINT, permissions::BURN],
        )],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    AssetsConfig {
        endowed_assets: vec![
            (
                XOR,
                assets_and_permissions_account_id,
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                PRUSD,
                PrestoAccountId::get(),
                AssetSymbol(b"PRUSD".to_vec()),
                AssetName(b"Presto USD".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    DexManagerConfig {
        dex_list: vec![
            (
                DEXId::Polkaswap,
                DEXInfo {
                    base_asset_id: XOR,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
            (
                DEXId::PolkaswapPresto,
                DEXInfo {
                    base_asset_id: PRUSD,
                    synthetic_base_asset_id: XST,
                    is_public: true,
                },
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        System::set_block_number(1);
        Timestamp::set_timestamp(0);
    });
    ext
}

pub fn crop_receipt_content_template(
) -> BoundedString<<Runtime as presto::Config>::MaxCropReceiptContentSize> {
    let content = include_str!("../crop_receipt_template.json");
    BoundedString::truncate_from(content)
}
