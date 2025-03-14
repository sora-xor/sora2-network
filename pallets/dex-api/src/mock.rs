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

use crate as dex_api;
use common::alt::DiscreteQuotation;
use common::mock::ExistentialDeposits;
use common::prelude::{Balance, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    balance, fixed, fixed_from_basis_points, hash, mock_assets_config,
    mock_ceres_liquidity_locker_config, mock_common_config, mock_currencies_config,
    mock_demeter_farming_platform_config, mock_dex_api_config, mock_dex_manager_config,
    mock_frame_system_config, mock_liquidity_source_config, mock_pallet_balances_config,
    mock_pallet_timestamp_config, mock_permissions_config, mock_pool_xyk_config,
    mock_pswap_distribution_config, mock_technical_config, mock_tokens_config, Amount, AssetId32,
    DEXId, DEXInfo, Fixed, LiquiditySource, LiquiditySourceType, RewardReason, DOT, KSM, VXOR, XOR,
    XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::GenesisBuild;
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::pallet_prelude::BlockNumberFor;
use hex_literal::hex;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
use sp_core::crypto::AccountId32;
use sp_runtime::Perbill;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type AssetId = AssetId32<common::PredefinedAssetId>;
type ReservesInit = Vec<(DEXId, AssetId, (Fixed, Fixed))>;

pub fn alice() -> AccountId {
    AccountId32::from(hex!(
        "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"
    ))
}

pub fn bob() -> AccountId {
    AccountId32::from([2u8; 32])
}

pub const DEX_A_ID: DEXId = DEXId::Polkaswap;
pub const DEX_B_ID: DEXId = DEXId::PolkaswapXSTUSD;

mock_assets_config!(Runtime);
mock_ceres_liquidity_locker_config!(Runtime, PoolXYK);
mock_common_config!(Runtime);
mock_currencies_config!(Runtime);
mock_demeter_farming_platform_config!(Runtime);
mock_dex_api_config!(
    Runtime,
    WeightedEmptyLiquiditySource,
    pool_xyk::Pallet<Runtime>,
    WeightedEmptyLiquiditySource,
    mock_liquidity_source::Instance1,
    mock_liquidity_source::Instance2,
    mock_liquidity_source::Instance3,
    mock_liquidity_source::Instance4,
    WeightedEmptyLiquiditySource
);
mock_dex_manager_config!(Runtime);
mock_frame_system_config!(Runtime);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance1,
    dex_manager::Pallet<Runtime>,
    GetFee
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance2,
    dex_manager::Pallet<Runtime>,
    GetFee
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance3,
    dex_manager::Pallet<Runtime>,
    GetFee
);
mock_liquidity_source_config!(
    Runtime,
    mock_liquidity_source::Instance4,
    dex_manager::Pallet<Runtime>,
    GetFee
);
mock_pallet_balances_config!(Runtime);
mock_pallet_timestamp_config!(Runtime);
mock_permissions_config!(Runtime);
mock_pool_xyk_config!(Runtime, (), ());
mock_pswap_distribution_config!(Runtime, PoolXYK);
mock_technical_config!(Runtime, pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>);
mock_tokens_config!(Runtime);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetSyntheticBaseAssetId: AssetId = XST;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
    pub GetFee: Fixed = fixed_from_basis_points(30u16);
    pub GetPswapDistributionAccountId: AccountId = AccountId32::from([3; 32]);
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 10;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
    pub GetParliamentAccountId: AccountId = AccountId32::from([8; 32]);
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        DexApi: dex_api::{Pallet, Call, Config, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Pallet, Call, Config<T>, Storage},
        MockLiquiditySource2: mock_liquidity_source::<Instance2>::{Pallet, Call, Config<T>, Storage},
        MockLiquiditySource3: mock_liquidity_source::<Instance3>::{Pallet, Call, Config<T>, Storage},
        MockLiquiditySource4: mock_liquidity_source::<Instance4>::{Pallet, Call, Config<T>, Storage},
        Technical: technical::{Pallet, Call, Storage, Event<T>},
        DexManager: dex_manager::{Pallet, Call, Storage},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>}
    }
}

// We need non-zero weight for testing weight calculation
pub struct WeightedEmptyLiquiditySource;

impl<DEXId, AccountId, AssetId: Ord + Clone>
    LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>
    for WeightedEmptyLiquiditySource
{
    fn can_exchange(
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> bool {
        <() as LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>>::can_exchange(
            target_id,
            input_asset_id,
            output_asset_id,
        )
    }

    fn quote(
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        <() as LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>>::quote(
            target_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
        )
    }

    fn step_quote(
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        recommended_samples_count: usize,
        deduce_fee: bool,
    ) -> Result<(DiscreteQuotation<AssetId, Balance>, Weight), DispatchError> {
        <() as LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>>::step_quote(
            target_id,
            input_asset_id,
            output_asset_id,
            amount,
            recommended_samples_count,
            deduce_fee,
        )
    }

    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<(SwapOutcome<Balance, AssetId>, Weight), DispatchError> {
        <()>::exchange(
            sender,
            receiver,
            target_id,
            input_asset_id,
            output_asset_id,
            swap_amount,
        )
    }

    fn check_rewards(
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        input_amount: Balance,
        output_amount: Balance,
    ) -> Result<(Vec<(Balance, AssetId, RewardReason)>, Weight), DispatchError> {
        <() as LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>>::check_rewards(
            target_id,
            input_asset_id,
            output_asset_id,
            input_amount,
            output_amount,
        )
    }

    fn quote_without_impact(
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance, AssetId>, DispatchError> {
        <() as LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError>>::quote_without_impact(
            target_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
        )
    }

    fn quote_weight() -> Weight {
        Weight::from_all(1)
    }

    fn step_quote_weight(_samples_count: usize) -> Weight {
        Weight::from_all(1)
    }

    fn exchange_weight() -> Weight {
        Weight::from_all(10)
    }

    fn check_rewards_weight() -> Weight {
        Weight::from_all(100)
    }
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = VXOR;
}
pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    reserves: ReservesInit,
    reserves_2: ReservesInit,
    reserves_3: ReservesInit,
    reserves_4: ReservesInit,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    source_types: Vec<LiquiditySourceType>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (alice(), XOR, balance!(1000000000000000000)),
                (bob(), DOT, balance!(1000000000000000000)),
            ],
            reserves: vec![
                (DEX_A_ID, DOT, (fixed!(5000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(5500), fixed!(4000))),
                (DEX_B_ID, DOT, (fixed!(100), fixed!(45))),
            ],
            reserves_2: vec![
                (DEX_A_ID, DOT, (fixed!(6000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(6500), fixed!(3000))),
                (DEX_B_ID, DOT, (fixed!(200), fixed!(45))),
            ],
            reserves_3: vec![
                (DEX_A_ID, DOT, (fixed!(7000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(7500), fixed!(2000))),
                (DEX_B_ID, DOT, (fixed!(300), fixed!(45))),
            ],
            reserves_4: vec![
                (DEX_A_ID, DOT, (fixed!(8000), fixed!(7000))),
                (DEX_A_ID, KSM, (fixed!(8500), fixed!(1000))),
                (DEX_B_ID, DOT, (fixed!(400), fixed!(45))),
            ],
            dex_list: vec![
                (
                    DEX_A_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                        is_public: true,
                    },
                ),
                (
                    DEX_B_ID,
                    DEXInfo {
                        base_asset_id: GetBaseAssetId::get(),
                        synthetic_base_asset_id: GetSyntheticBaseAssetId::get(),
                        is_public: true,
                    },
                ),
            ],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_B_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (alice(), Scope::Limited(hash(&DEX_B_ID)), vec![MANAGE_DEX]),
            ],
            source_types: vec![
                LiquiditySourceType::MockPool,
                LiquiditySourceType::MockPool2,
                LiquiditySourceType::MockPool3,
                LiquiditySourceType::MockPool4,
            ],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
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

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: self.initial_permission_owners,
            initial_permissions: self.initial_permissions,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance1> {
            reserves: self.reserves,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance2> {
            reserves: self.reserves_2,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance3> {
            reserves: self.reserves_3,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance4> {
            reserves: self.reserves_4,
            phantom: Default::default(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        <crate::GenesisConfig as GenesisBuild<Runtime>>::assimilate_storage(
            &crate::GenesisConfig {
                source_types: self.source_types,
            },
            &mut t,
        )
        .unwrap();

        t.into()
    }
}
