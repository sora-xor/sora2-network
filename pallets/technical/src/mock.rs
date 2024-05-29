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

use crate::{self as technical, Config};
use codec::{Decode, Encode};
use common::prelude::Balance;
use common::{mock_pallet_balances_config, mock_tokens_config, PSWAP, VAL, XST};
use currencies::BasicCurrencyAdapter;
use dispatch::DispatchResult;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, dispatch, parameter_types};
use frame_system;
use hex_literal::hex;
use orml_traits::parameter_type_with_key;
use sp_core::crypto::AccountId32;
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::Perbill;
use sp_std::marker::PhantomData;
use PolySwapActionExample::*;

pub use common::mock::*;
pub use common::TechAssetId::*;
pub use common::TechPurpose::*;
pub use common::TradingPair;

pub type BlockNumber = u64;
pub type AccountId = AccountId32;
pub type Amount = i128;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type DEXId = u32;
type AssetId = common::AssetId32<common::mock::ComicAssetId>;
type TechAssetId = common::TechAssetId<common::mock::ComicAssetId>;
type TechAmount = Amount;
type TechBalance = Balance;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId = common::AssetId32 { code: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
    }
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

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
}

mock_pallet_balances_config!(Runtime);

mock_tokens_config!(Runtime);

impl currencies::Config for Runtime {
    type MultiCurrency = tokens::Pallet<Runtime>;
    type NativeCurrency =
        BasicCurrencyAdapter<Runtime, pallet_balances::Pallet<Runtime>, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

parameter_types! {
    pub GetBuyBackAssetId: AssetId = XST.into();
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL.into(), PSWAP.into()];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = 0;
}

impl assets::Config for Runtime {
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
    type Currency = currencies::Pallet<Runtime>;
    type GetTotalBalance = ();
    type WeightInfo = ();
    type AssetRegulator = permissions::Pallet<Runtime>;
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = PolySwapActionExample;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

pub fn get_alice() -> AccountId {
    AccountId32::from([1; 32])
}
pub fn get_bob() -> AccountId {
    AccountId32::from([2; 32])
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

#[allow(non_snake_case)]
pub fn RedPepper() -> AssetId {
    common::mock::ComicAssetId::RedPepper.into()
}

#[allow(non_snake_case)]
pub fn BlackPepper() -> AssetId {
    common::mock::ComicAssetId::BlackPepper.into()
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (get_alice(), RedPepper(), 99_000_u128.into()),
                (get_alice(), BlackPepper(), 2000_000_u128.into()),
                (get_bob(), RedPepper(), 2000_000_u128.into()),
            ],
        }
    }
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, scale_info::TypeInfo)]
pub struct GenericPairSwapActionExample {
    pub give_minted: bool,
    pub give_asset: AssetId,
    pub give_amount: TechBalance,
    pub take_burn: bool,
    pub take_asset: AssetId,
    pub take_amount: TechBalance,
    pub take_account: TechAccountId,
}

impl common::SwapAction<AccountId, TechAccountId, AssetId, Runtime>
    for GenericPairSwapActionExample
{
    fn reserve(&self, source: &AccountId, _base_asset_id: &AssetId) -> dispatch::DispatchResult {
        //FIXME now in this place exist two operations, and it is not lock.
        crate::Pallet::<Runtime>::transfer_in(
            &self.give_asset.into(),
            source,
            &self.take_account,
            self.give_amount,
        )?;
        crate::Pallet::<Runtime>::transfer_out(
            &self.take_asset.into(),
            &self.take_account,
            source,
            self.take_amount,
        )?;
        Ok(())
    }
    fn claim(&self, _source: &AccountId) -> bool {
        //FIXME implement lock for swap and apply swap from lock, these operation must come
        //together and appears in one block as one container for operations.
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountId) {
        unimplemented!()
    }
}

impl common::SwapRulesValidation<AccountId, TechAccountId, AssetId, Runtime>
    for GenericPairSwapActionExample
{
    fn is_abstract_checking(&self) -> bool {
        false
    }
    fn prepare_and_validate(
        &mut self,
        _source: Option<&AccountId>,
        _base_asset_id: &AssetId,
    ) -> DispatchResult {
        Ok(())
    }
    fn instant_auto_claim_used(&self) -> bool {
        true
    }
    fn triggered_auto_claim_used(&self) -> bool {
        false
    }
    fn is_able_to_claim(&self) -> bool {
        true
    }
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, scale_info::TypeInfo)]
pub struct MultiSwapActionExample {
    give_amount_a: TechAmount,
    give_amount_b: TechAmount,
    take_amount_c: TechAmount,
    take_amount_d: TechAmount,
    take_amount_e: TechAmount,
}

impl common::SwapAction<AccountId, TechAccountId, AssetId, Runtime> for MultiSwapActionExample {
    fn reserve(&self, _source: &AccountId, _base_asset_id: &AssetId) -> dispatch::DispatchResult {
        Ok(())
    }
    fn claim(&self, _source: &AccountId) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountId) {
        unimplemented!()
    }
}

impl common::SwapRulesValidation<AccountId, TechAccountId, AssetId, Runtime>
    for MultiSwapActionExample
{
    fn is_abstract_checking(&self) -> bool {
        false
    }
    fn prepare_and_validate(
        &mut self,
        _source: Option<&AccountId>,
        _base_asset_id: &AssetId,
    ) -> DispatchResult {
        Ok(())
    }
    fn instant_auto_claim_used(&self) -> bool {
        true
    }
    fn triggered_auto_claim_used(&self) -> bool {
        true
    }
    fn is_able_to_claim(&self) -> bool {
        true
    }
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, scale_info::TypeInfo)]
pub struct CrowdSwapActionExample {
    crowd_id: u32,
    give_amount: TechAmount,
    take_amount: TechAmount,
}

impl common::SwapAction<AccountId, TechAccountId, AssetId, Runtime> for CrowdSwapActionExample {
    fn reserve(&self, _source: &AccountId, _base_asset_id: &AssetId) -> dispatch::DispatchResult {
        unimplemented!()
    }
    fn claim(&self, _source: &AccountId) -> bool {
        true
    }
    fn weight(&self) -> Weight {
        unimplemented!()
    }
    fn cancel(&self, _source: &AccountId) {
        unimplemented!()
    }
}

impl common::SwapRulesValidation<AccountId, TechAccountId, AssetId, Runtime>
    for CrowdSwapActionExample
{
    fn is_abstract_checking(&self) -> bool {
        false
    }
    fn prepare_and_validate(
        &mut self,
        _source: Option<&AccountId>,
        _base_asset_id: &AssetId,
    ) -> DispatchResult {
        Ok(())
    }
    fn instant_auto_claim_used(&self) -> bool {
        false
    }
    fn triggered_auto_claim_used(&self) -> bool {
        true
    }
    fn is_able_to_claim(&self) -> bool {
        true
    }
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, Debug, scale_info::TypeInfo)]
pub enum PolySwapActionExample {
    GenericPair(GenericPairSwapActionExample),
    Multi(MultiSwapActionExample),
    Crowd(CrowdSwapActionExample),
}

impl common::SwapAction<AccountId, TechAccountId, AssetId, Runtime> for PolySwapActionExample {
    fn reserve(&self, source: &AccountId, base_asset_id: &AssetId) -> dispatch::DispatchResult {
        match self {
            GenericPair(a) => a.reserve(source, base_asset_id),
            Multi(a) => a.reserve(source, base_asset_id),
            Crowd(a) => a.reserve(source, base_asset_id),
        }
    }
    fn claim(&self, source: &AccountId) -> bool {
        match self {
            GenericPair(a) => a.claim(source),
            Multi(a) => a.claim(source),
            Crowd(a) => a.claim(source),
        }
    }
    fn weight(&self) -> Weight {
        match self {
            GenericPair(a) => a.weight(),
            Multi(a) => a.weight(),
            Crowd(a) => a.weight(),
        }
    }
    fn cancel(&self, source: &AccountId) {
        match self {
            GenericPair(a) => a.cancel(source),
            Multi(a) => a.cancel(source),
            Crowd(a) => a.cancel(source),
        }
    }
}

impl common::SwapRulesValidation<AccountId, TechAccountId, AssetId, Runtime>
    for PolySwapActionExample
{
    fn is_abstract_checking(&self) -> bool {
        match self {
            GenericPair(a) => a.is_abstract_checking(),
            Multi(a) => a.is_abstract_checking(),
            Crowd(a) => a.is_abstract_checking(),
        }
    }

    fn prepare_and_validate(
        &mut self,
        source: Option<&AccountId>,
        base_asset_id: &AssetId,
    ) -> DispatchResult {
        match self {
            GenericPair(a) => a.prepare_and_validate(source, base_asset_id),
            Multi(a) => a.prepare_and_validate(source, base_asset_id),
            Crowd(a) => a.prepare_and_validate(source, base_asset_id),
        }
    }

    fn instant_auto_claim_used(&self) -> bool {
        match self {
            GenericPair(a) => a.instant_auto_claim_used(),
            Multi(a) => a.instant_auto_claim_used(),
            Crowd(a) => a.instant_auto_claim_used(),
        }
    }
    fn triggered_auto_claim_used(&self) -> bool {
        match self {
            GenericPair(a) => a.triggered_auto_claim_used(),
            Multi(a) => a.triggered_auto_claim_used(),
            Crowd(a) => a.triggered_auto_claim_used(),
        }
    }
    fn is_able_to_claim(&self) -> bool {
        match self {
            GenericPair(a) => a.is_able_to_claim(),
            Multi(a) => a.is_able_to_claim(),
            Crowd(a) => a.is_able_to_claim(),
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = SystemConfig::default().build_storage::<Runtime>().unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(get_alice(), 0), (get_bob(), 0)],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![],
            initial_permissions: vec![],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        TokensConfig {
            balances: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
