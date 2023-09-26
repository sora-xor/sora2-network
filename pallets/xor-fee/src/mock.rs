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

use common::mock::ExistentialDeposits;
use common::prelude::{Balance, BlockLength, FixedWrapper, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    self, balance, Amount, AssetId32, AssetName, AssetSymbol, LiquidityProxyTrait,
    LiquiditySourceFilter, LiquiditySourceType, OnValBurned, ReferrerAccountProvider, PSWAP, VAL,
    XOR, XST,
};

use currencies::BasicCurrencyAdapter;
use frame_support::dispatch::{DispatchInfo, Pays, PostDispatchInfo};
use frame_support::pallet_prelude::ValueQuery;
use frame_support::traits::{
    ConstU128, Currency, Everything, ExistenceRequirement, GenesisBuild, WithdrawReasons,
};
use frame_support::weights::{ConstantMultiplier, IdentityFee, Weight};
use frame_support::{construct_runtime, parameter_types, storage_alias};

use permissions::{Scope, BURN, MINT};
use sp_core::H256;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{AccountId32, DispatchError, Percent};
use sp_staking::SessionIndex;
use traits::MultiCurrency;

pub use crate::{self as xor_fee, Config, Pallet};

// Configure a mock runtime to test the pallet.
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;
type AssetId = AssetId32<common::PredefinedAssetId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

pub fn account_from_str(s: &str) -> AccountId {
    sp_core::blake2_256(s.as_bytes()).into()
}

parameter_types! {
    pub GetPostponeAccountId: AccountId = account_from_str("postpone");
    pub GetPaysNoAccountId: AccountId = account_from_str("pays-no");
    pub GetFeeSourceAccountId: AccountId = account_from_str("fee-source");
    pub GetReferalAccountId: AccountId = account_from_str("referal");
    pub GetReferrerAccountId: AccountId = account_from_str("referrer");
    pub const BlockHashCount: u64 = 250;
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWeight: u32 = 50;
    pub const BuyBackXSTPercent: Percent = Percent::from_percent(10);
    pub const ExistentialDeposit: u32 = 0;
    pub const XorId: AssetId = XOR;
    pub const ValId: AssetId = VAL;
    pub const DEXIdValue: DEXId = common::DEXId::Polkaswap;
    pub GetXorFeeAccountId: AccountId = account_from_str("xor-fee");
    pub GetParliamentAccountId: AccountId = account_from_str("sora-parliament");
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},
        XorFee: xor_fee::{Pallet, Call, Event<T>},
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = BlockLength;
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
}

parameter_types! {
    pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = XorFee;
    type WeightToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
    type LengthToFee = ConstantMultiplier<Balance, ConstU128<0>>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = XST;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub GetBuyBackAccountId: AccountId = account_from_str("buy-back");
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = XorId;
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

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
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

pub struct CustomFees;

impl xor_fee::ApplyCustomFees<RuntimeCall, AccountId> for CustomFees {
    type FeeDetails = Balance;
    fn compute_fee(call: &RuntimeCall) -> Option<(Balance, Self::FeeDetails)> {
        let fee = match call {
            RuntimeCall::Assets(assets::Call::register { .. }) => Some(balance!(0.007)),
            RuntimeCall::Assets(..) => Some(balance!(0.0007)),
            _ => None,
        };
        fee.map(|fee| (fee, fee))
    }

    fn compute_actual_fee(
        _post_info: &sp_runtime::traits::PostDispatchInfoOf<RuntimeCall>,
        _info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
        _result: &sp_runtime::DispatchResult,
        fee_details: Option<Self::FeeDetails>,
    ) -> Option<Balance> {
        fee_details
    }

    fn get_fee_source(who: &AccountId, call: &RuntimeCall, _fee: Balance) -> AccountId {
        if matches!(call, RuntimeCall::System(..)) {
            return GetFeeSourceAccountId::get();
        }
        who.clone()
    }

    fn should_be_paid(who: &AccountId, _call: &RuntimeCall) -> bool {
        if *who == GetPaysNoAccountId::get() {
            return false;
        }
        true
    }

    fn should_be_postponed(
        who: &AccountId,
        _fee_source: &AccountId,
        _call: &RuntimeCall,
        _fee: Balance,
    ) -> bool {
        if *who == GetPostponeAccountId::get() {
            return true;
        }
        false
    }
}

#[storage_alias]
pub type ValBurned<T: Config> = StorageValue<crate::Pallet<T>, Balance, ValueQuery>;

pub struct ValBurnedAggregator;

impl OnValBurned for ValBurnedAggregator {
    fn on_val_burned(amount: Balance) {
        ValBurned::<Runtime>::mutate(|x| *x += amount);
    }
}

pub struct WithdrawFee;

impl xor_fee::WithdrawFee<Runtime> for WithdrawFee {
    fn withdraw_fee(
        _who: &AccountId,
        fee_source: &AccountId,
        _call: &RuntimeCall,
        fee: Balance,
    ) -> Result<(AccountId, Option<crate::NegativeImbalanceOf<Runtime>>), DispatchError> {
        Ok((
            fee_source.clone(),
            Some(Balances::withdraw(
                fee_source,
                fee,
                WithdrawReasons::TRANSACTION_PAYMENT,
                ExistenceRequirement::KeepAlive,
            )?),
        ))
    }
}

impl Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type BuyBackXSTPercent = BuyBackXSTPercent;
    type XorId = XorId;
    type ValId = ValId;
    type XstId = GetBuyBackAssetId;
    type DEXIdValue = DEXIdValue;
    type LiquidityProxy = MockLiquidityProxy;
    type OnValBurned = ValBurnedAggregator;
    type CustomFees = CustomFees;
    type GetTechnicalAccountId = GetXorFeeAccountId;
    type SessionManager = MockSessionManager;
    type WithdrawFee = WithdrawFee;
    type FullIdentification = ();
    type BuyBackHandler = ();
    type ReferrerAccountProvider = MockReferrerAccountProvider;
    type WeightInfo = ();
}

pub struct MockReferrerAccountProvider;

impl ReferrerAccountProvider<AccountId> for MockReferrerAccountProvider {
    fn get_referrer_account(who: &AccountId) -> Option<AccountId> {
        if *who == GetReferalAccountId::get() {
            Some(GetReferrerAccountId::get())
        } else {
            None
        }
    }
}

pub struct MockSessionManager;

impl pallet_session::SessionManager<AccountId> for MockSessionManager {
    fn new_session(_new_index: SessionIndex) -> Option<Vec<AccountId>> {
        None
    }

    fn end_session(_end_index: SessionIndex) {}

    fn start_session(_start_index: SessionIndex) {}

    fn new_session_genesis(_new_index: SessionIndex) -> Option<Vec<AccountId>> {
        None
    }
}

impl pallet_session::historical::SessionManager<AccountId, ()> for MockSessionManager {
    fn new_session(_new_index: SessionIndex) -> Option<Vec<(AccountId, ())>> {
        None
    }

    fn start_session(_start_index: SessionIndex) {}

    fn end_session(_end_index: SessionIndex) {}
}

pub struct MockLiquidityProxy;

impl MockLiquidityProxy {
    fn mock_price(asset_id: &AssetId) -> Balance {
        match asset_id {
            &XOR => balance!(1.0),
            &VAL => balance!(3.1),
            &PSWAP => balance!(13),
            _ => balance!(2.5),
        }
    }

    fn exchange_inner(
        sender: Option<&AccountId>,
        receiver: Option<&AccountId>,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let input_price = Self::mock_price(input_asset_id);
        let output_price = Self::mock_price(output_asset_id);
        let price = FixedWrapper::from(output_price) / FixedWrapper::from(input_price);
        let (input_amount, output_amount, is_reversed) = match amount {
            QuoteAmount::WithDesiredInput {
                desired_amount_in, ..
            } => (
                desired_amount_in,
                (FixedWrapper::from(desired_amount_in) * price)
                    .try_into_balance()
                    .unwrap(),
                false,
            ),
            QuoteAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => (
                (FixedWrapper::from(desired_amount_out) / price)
                    .try_into_balance()
                    .unwrap(),
                desired_amount_out,
                true,
            ),
        };
        if let Some((sender, receiver)) = sender.zip(receiver) {
            Currencies::withdraw(*input_asset_id, sender, input_amount)?;
            Currencies::deposit(*output_asset_id, receiver, output_amount)?;
        }
        if is_reversed {
            Ok(SwapOutcome::new(input_amount, 0))
        } else {
            Ok(SwapOutcome::new(output_amount, 0))
        }
    }
}

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Self::exchange_inner(
            Some(sender),
            Some(receiver),
            input_asset_id,
            output_asset_id,
            amount.into(),
        )
    }

    fn quote(
        _dex_id: DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        _filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        _deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Self::exchange_inner(None, None, input_asset_id, output_asset_id, amount)
    }
}

pub fn initial_balance() -> Balance {
    balance!(1000)
}

pub fn initial_reserves() -> Balance {
    balance!(10000)
}

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        common::test_utils::init_logger();
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![(GetXorFeeAccountId::get(), initial_balance())],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![GetXorFeeAccountId::get()]),
                (BURN, Scope::Unlimited, vec![GetXorFeeAccountId::get()]),
            ],
            initial_permissions: vec![(
                GetXorFeeAccountId::get(),
                Scope::Unlimited,
                vec![MINT, BURN],
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: vec![
                (
                    XOR,
                    GetXorFeeAccountId::get(),
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    VAL,
                    GetXorFeeAccountId::get(),
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                ),
                (
                    XST,
                    GetXorFeeAccountId::get(),
                    AssetSymbol(b"XST".to_vec()),
                    AssetName(b"XST".to_vec()),
                    18,
                    balance!(100),
                    true,
                    None,
                    None,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            balances: vec![(GetXorFeeAccountId::get().clone(), VAL, balance!(1000))],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}

/// create a transaction info struct from weight. Handy to avoid building the whole struct.
pub fn info_from_weight(w: Weight) -> DispatchInfo {
    // pays_fee: Pays::Yes -- class: DispatchClass::Normal
    DispatchInfo {
        weight: w,
        ..Default::default()
    }
}

pub fn info_pays_no(w: Weight) -> DispatchInfo {
    DispatchInfo {
        pays_fee: Pays::No,
        weight: w,
        ..Default::default()
    }
}

pub fn default_post_info() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Default::default(),
    }
}

pub fn post_info_from_weight(w: Weight) -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: Some(w),
        pays_fee: Default::default(),
    }
}

pub fn post_info_pays_no() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Pays::No,
    }
}
