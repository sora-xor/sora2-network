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

use crate as kensetsu;

use common::mock::ExistentialDeposits;
use common::{
    balance, Amount, AssetId32, AssetName, AssetSymbol, DEXId, FromGenericPair, PredefinedAssetId,
    DAI, DEFAULT_BALANCE_PRECISION, KUSD, XOR, XST,
};
use currencies::BasicCurrencyAdapter;
use frame_support::parameter_types;
use frame_support::traits::{ConstU16, ConstU64};
use frame_support::traits::{Everything, GenesisBuild};
use hex_literal::hex;
use permissions::Scope;
use sp_core::H256;
use sp_runtime::MultiSignature;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
};

type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type AssetId = AssetId32<PredefinedAssetId>;
type Balance = u128;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type Moment = u64;
type Signature = MultiSignature;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<PredefinedAssetId>;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

pub struct ReferencePriceProviderMock;

impl common::ReferencePriceProvider<AssetId, Balance> for ReferencePriceProviderMock {
    /// Returns `asset_id` price is $1
    fn get_reference_price(
        _asset_id: &AssetId,
    ) -> Result<Balance, frame_support::dispatch::DispatchError> {
        Ok(balance!(1))
    }
}

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Kensetsu: kensetsu::{Pallet, Call, Storage, Event<T>, ValidateUnsigned},
    }
);

parameter_types! {
    // Assets
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![];
    pub const GetBuyBackPercentage: u8 = 0;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;

    // Balances
    pub const MaxLocks: u32 = 50;
    pub const ExistentialDeposit: u128 = 1;
    pub const MaxReserves: u32 = 50;

    // Timestamp
    pub const MinimumPeriod: u64 = 5;

    // Kensetsu
    pub KensetsuTreasuryTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            kensetsu::TECH_ACCOUNT_PREFIX.to_vec(),
            kensetsu::TECH_ACCOUNT_TREASURY_MAIN.to_vec(),
        )
    };
    pub KensetsuTreasuryAccountId: AccountId = {
        let tech_account_id = KensetsuTreasuryTechAccountId::get();
        technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };
    pub const KusdAssetId: AssetId = KUSD;

    // 1 day
    pub const AccrueInterestPeriod: Moment = 86_400_000;

}

impl assets::Config for TestRuntime {
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
    type Currency = currencies::Pallet<TestRuntime>;
    type WeightInfo = ();
    type GetTotalBalance = ();
}

impl pallet_balances::Config for TestRuntime {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type MaxLocks = MaxLocks;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = ();
}

impl common::Config for TestRuntime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl currencies::Config for TestRuntime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<TestRuntime, Balances, Amount, u64>;
    type GetNativeCurrencyId = <TestRuntime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl frame_system::Config for TestRuntime {
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
    type BlockHashCount = ConstU64<250>;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ConstU16<42>;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl permissions::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
}

impl technical::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

impl tokens::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <TestRuntime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

impl pallet_timestamp::Config for TestRuntime {
    type Moment = Moment;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl kensetsu::Config for TestRuntime {
    type RuntimeEvent = RuntimeEvent;
    type AssetInfoProvider = Assets;
    type TreasuryTechAccount = KensetsuTreasuryTechAccountId;
    type KusdAssetId = KusdAssetId;
    type ReferencePriceProvider = ReferencePriceProviderMock;
    type AccrueInterestPeriod = AccrueInterestPeriod;
    type UnsignedPriority = ConstU64<100>;
    type UnsignedLongevity = ConstU64<100>;
}

// Builds testing externalities
pub fn new_test_ext() -> sp_io::TestExternalities {
    let assets_and_permissions_tech_account_id =
        TechAccountId::Generic(b"SYSTEM_ACCOUNT".to_vec(), b"ASSETS_PERMISSIONS".to_vec());
    let assets_and_permissions_account_id =
        technical::Pallet::<TestRuntime>::tech_account_id_to_account_id(
            &assets_and_permissions_tech_account_id,
        )
        .unwrap();

    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();
    technical::GenesisConfig::<TestRuntime> {
        register_tech_accounts: vec![
            (
                KensetsuTreasuryAccountId::get(),
                KensetsuTreasuryTechAccountId::get(),
            ),
            (
                assets_and_permissions_account_id.clone(),
                assets_and_permissions_tech_account_id.clone(),
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
        initial_permissions: vec![
            (
                assets_and_permissions_account_id.clone(),
                Scope::Unlimited,
                vec![permissions::MINT, permissions::BURN],
            ),
            (
                KensetsuTreasuryAccountId::get(),
                Scope::Unlimited,
                vec![permissions::MINT, permissions::BURN],
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    AssetsConfig {
        endowed_assets: vec![
            (
                XOR.into(),
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                DAI.into(),
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"DAI".to_vec()),
                AssetName(b"DAI".to_vec()),
                DEFAULT_BALANCE_PRECISION,
                0,
                true,
                None,
                None,
            ),
            (
                KUSD.into(),
                assets_and_permissions_account_id.clone(),
                AssetSymbol(b"KUSD".to_vec()),
                AssetName(b"Kensetsu Stable Dollar".to_vec()),
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

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}
