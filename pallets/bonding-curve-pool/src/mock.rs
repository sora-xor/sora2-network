use crate::{Module, Trait};
use common::{
    fixed,
    prelude::{AssetId, Balance, SwapAmount, SwapOutcome},
    Amount, LiquiditySource, TechPurpose,
};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight, StorageValue};
use frame_system as system;
use orml_traits::MultiCurrency;
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError, Perbill,
};
use std::collections::HashMap;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<AssetId, DEXId>;
pub type ReservesAccount =
    mock_liquidity_source::ReservesAcc<Runtime, mock_liquidity_source::Instance1>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub const USD: AssetId = AssetId::USD;
pub const XOR: AssetId = AssetId::XOR;
pub const VAL: AssetId = AssetId::VAL;

impl_outer_origin! {
    pub enum Origin for Runtime {}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Runtime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
}

impl system::Trait for Runtime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type ModuleToIndex = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
}

parameter_types! {
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
}

impl dex_manager::Trait for Runtime {
    type Event = ();
    type GetDefaultFee = ();
    type GetDefaultProtocolFee = ();
}

impl trading_pair::Trait for Runtime {
    type Event = ();
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance1> for Runtime {
    type Event = ();
    type GetFee = ();
    type EnsureDEXOwner = ();
    type EnsureTradingPairExists = ();
}

pub struct MockDEXApi;

impl MockDEXApi {
    pub fn init() -> Result<(), DispatchError> {
        let mock_liquidity_source_tech_account_id =
            TechAccountId::Pure(DEXId::Polkaswap.into(), TechPurpose::FeeCollector);
        let account_id =
            Technical::tech_account_id_to_account_id(&mock_liquidity_source_tech_account_id)?;
        Technical::register_tech_account_id(mock_liquidity_source_tech_account_id.clone())?;
        MockLiquiditySource::set_reserves_account_id(mock_liquidity_source_tech_account_id)?;
        Currencies::deposit(XOR, &account_id, 1_000_u128.into())?;
        Currencies::deposit(VAL, &account_id, 1_000_u128.into())?;
        Currencies::deposit(USD, &account_id, 1_000_000_u128.into())?;
        Ok(())
    }
}

impl<DEXId> LiquiditySource<DEXId, AccountId, AssetId, Balance, DispatchError> for MockDEXApi {
    fn can_exchange(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
    ) -> bool {
        unimplemented!()
    }

    fn quote(
        _target_id: &DEXId,
        _input_asset_id: &AssetId,
        _output_asset_id: &AssetId,
        _swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        unimplemented!()
    }

    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        _target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let prices: HashMap<_, _> = vec![
            ((USD, XOR), Balance(fixed!(0, 01))),
            ((XOR, VAL), Balance(fixed!(2, 0))),
        ]
        .into_iter()
        .collect();
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let mut amount_out =
                    desired_amount_in * prices[&(*input_asset_id, *output_asset_id)];
                let fee = amount_out * Balance(fixed!(0,3%));
                amount_out = amount_out - fee;
                let reserves_account_id =
                    &Technical::tech_account_id_to_account_id(&ReservesAccount::get())?;
                assert_ne!(desired_amount_in, 0u128.into());
                let old = Assets::total_balance(input_asset_id, sender)?;
                Assets::transfer(
                    input_asset_id,
                    sender,
                    reserves_account_id,
                    desired_amount_in,
                )?;
                let new = Assets::total_balance(input_asset_id, sender)?;
                assert_ne!(old, new);
                Assets::transfer(output_asset_id, reserves_account_id, receiver, amount_out)?;
                Ok(SwapOutcome::new(amount_out, fee))
            }
            _ => Err(DispatchError::Other("Bad swap amount.")),
        }
    }
}

impl Trait for Runtime {
    type DEXApi = MockDEXApi;
}

impl tokens::Trait for Runtime {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Trait>::AssetId;
    type OnReceived = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
}

impl currencies::Trait for Runtime {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Balances, Balance, Balance, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Trait>::GetBaseAssetId;
}

type DEXId = common::DEXId;

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

impl assets::Trait for Runtime {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
}

impl permissions::Trait for Runtime {
    type Event = ();
}

impl technical::Trait for Runtime {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
}

impl pallet_balances::Trait for Runtime {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
}

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type Currencies = currencies::Module<Runtime>;
pub type BondingCurvePool = Module<Runtime>;
pub type Technical = technical::Module<Runtime>;
pub type MockLiquiditySource =
    mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
pub type Assets = assets::Module<Runtime>;

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (alice(), USD, 0u128.into()),
                (alice(), XOR, 350_000u128.into()),
                (alice(), VAL, 0u128.into()),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn new(endowed_accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
        Self { endowed_accounts }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self
                .endowed_accounts
                .iter()
                .map(|(account_id, asset_id, _)| (asset_id.clone(), account_id.clone()))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .filter_map(|(account_id, asset_id, balance)| {
                    if asset_id == &GetBaseAssetId::get() {
                        Some((account_id.clone(), balance.clone()))
                    } else {
                        None
                    }
                })
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self.endowed_accounts,
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
