use crate::{Module, Trait};
use common::{
    self, fixed, hash,
    prelude::{Balance, SwapAmount, SwapOutcome},
    Amount, AssetId32, AssetSymbol, DEXInfo, LiquiditySourceFilter, LiquiditySourceType,
    TechPurpose, USDT, VAL, XOR,
};
use currencies::BasicCurrencyAdapter;
use frame_support::{impl_outer_origin, parameter_types, weights::Weight, StorageValue};
use frame_system as system;
use orml_traits::MultiCurrency;
use permissions::{Scope, INIT_DEX, MANAGE_DEX};
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
type TechAssetId = common::TechAssetId<common::AssetId, DEXId, common::LiquiditySourceType>;
pub type ReservesAccount =
    mock_liquidity_source::ReservesAcc<Runtime, mock_liquidity_source::Instance1>;
pub type AssetId = AssetId32<common::AssetId>;

pub fn alice() -> AccountId {
    AccountId32::from([1u8; 32])
}

pub fn assets_owner() -> AccountId {
    AccountId32::from([2u8; 32])
}

impl_outer_origin! {
    pub enum Origin for Runtime {}
}

pub const DEX_A_ID: DEXId = DEXId::Polkaswap;

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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
}

parameter_types! {
    pub const GetDefaultFee: u16 = 30;
    pub const GetDefaultProtocolFee: u16 = 0;
}

impl dex_manager::Trait for Runtime {
    type Event = ();
    type WeightInfo = ();
}

impl trading_pair::Trait for Runtime {
    type Event = ();
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance1> for Runtime {
    type Event = ();
    type GetFee = ();
    type EnsureDEXManager = ();
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
        Currencies::deposit(XOR, &account_id, 100_000_u128.into())?;
        Currencies::deposit(VAL, &account_id, 100_000_u128.into())?;
        Currencies::deposit(USDT, &account_id, 1_000_000_u128.into())?;
        Ok(())
    }

    fn _can_exchange(
        _target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
    ) -> bool {
        get_mock_prices().contains_key(&(*input_asset_id, *output_asset_id))
    }

    fn inner_quote(
        _target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let mut amount_out =
                    desired_amount_in * get_mock_prices()[&(*input_asset_id, *output_asset_id)];
                let fee = amount_out * Balance(fixed!(0.003));
                amount_out = amount_out - fee;
                Ok(SwapOutcome::new(amount_out, fee))
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                let amount_in =
                    desired_amount_out / get_mock_prices()[&(*input_asset_id, *output_asset_id)];
                let with_fee = amount_in / Balance(fixed!(0.997));
                let fee = with_fee - amount_in;
                Ok(SwapOutcome::new(with_fee, fee))
            }
        }
    }

    fn inner_exchange(
        sender: &AccountId,
        receiver: &AccountId,
        target_id: &DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        swap_amount: SwapAmount<Balance>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        match swap_amount {
            SwapAmount::WithDesiredInput {
                desired_amount_in, ..
            } => {
                let outcome =
                    Self::inner_quote(target_id, input_asset_id, output_asset_id, swap_amount)?;
                let reserves_account_id =
                    &Technical::tech_account_id_to_account_id(&ReservesAccount::get())?;
                assert_ne!(desired_amount_in, 0u128.into());
                let old = Assets::total_balance(input_asset_id, sender)?;
                Assets::transfer_from(
                    input_asset_id,
                    sender,
                    reserves_account_id,
                    desired_amount_in,
                )?;
                let new = Assets::total_balance(input_asset_id, sender)?;
                assert_ne!(old, new);
                Assets::transfer_from(
                    output_asset_id,
                    reserves_account_id,
                    receiver,
                    outcome.amount,
                )?;
                Ok(SwapOutcome::new(outcome.amount, outcome.fee))
            }
            SwapAmount::WithDesiredOutput {
                desired_amount_out, ..
            } => {
                let outcome =
                    Self::inner_quote(target_id, input_asset_id, output_asset_id, swap_amount)?;
                let reserves_account_id =
                    &Technical::tech_account_id_to_account_id(&ReservesAccount::get())?;
                assert_ne!(outcome.amount, 0u128.into());
                let old = Assets::total_balance(input_asset_id, sender)?;
                Assets::transfer_from(input_asset_id, sender, reserves_account_id, outcome.amount)?;
                let new = Assets::total_balance(input_asset_id, sender)?;
                assert_ne!(old, new);
                Assets::transfer_from(
                    output_asset_id,
                    reserves_account_id,
                    receiver,
                    desired_amount_out,
                )?;
                Ok(SwapOutcome::new(outcome.amount, outcome.fee))
            }
        }
    }
}

fn get_mock_prices() -> HashMap<(AssetId, AssetId), Balance> {
    vec![
        ((USDT, XOR), Balance(fixed!(0.01))),
        ((XOR, USDT), Balance(fixed!(100.0))),
        ((VAL, XOR), Balance(fixed!(0.5))),
        ((XOR, VAL), Balance(fixed!(2.0))),
        ((USDT, VAL), Balance(fixed!(0.02))),
        ((VAL, USDT), Balance(fixed!(50.0))),
    ]
    .into_iter()
    .collect()
}

impl liquidity_proxy::LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockDEXApi {
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Self::inner_exchange(
            sender,
            receiver,
            &filter.dex_id,
            input_asset_id,
            output_asset_id,
            amount,
        )
    }

    fn quote(
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        Self::inner_quote(&filter.dex_id, input_asset_id, output_asset_id, amount)
    }
}

impl Trait for Runtime {
    type Event = ();
    type LiquidityProxy = MockDEXApi;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
}

impl tokens::Trait for Runtime {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
}

impl currencies::Trait for Runtime {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
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
    type WeightInfo = ();
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
    type WeightInfo = ();
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
    type MaxLocks = ();
}

parameter_types! {
    pub const GetNumSamples: usize = 40;
}

pub type System = frame_system::Module<Runtime>;
pub type Balances = pallet_balances::Module<Runtime>;
pub type Tokens = tokens::Module<Runtime>;
pub type Currencies = currencies::Module<Runtime>;
pub type MBCPool = Module<Runtime>;
pub type Technical = technical::Module<Runtime>;
pub type MockLiquiditySource =
    mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
pub type Assets = assets::Module<Runtime>;
pub type TradingPair = trading_pair::Module<Runtime>;

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, u8)>,
    dex_list: Vec<(DEXId, DEXInfo<AssetId>)>,
    initial_permission_owners: Vec<(u32, Scope, Vec<AccountId>)>,
    initial_permissions: Vec<(AccountId, Scope, Vec<u32>)>,
    reference_asset_id: AssetId,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            endowed_accounts: vec![
                (
                    alice(),
                    USDT,
                    0u128.into(),
                    AssetSymbol(b"USDT".to_vec()),
                    18,
                ),
                (
                    alice(),
                    XOR,
                    350_000u128.into(),
                    AssetSymbol(b"XOR".to_vec()),
                    18,
                ),
                (
                    alice(),
                    VAL,
                    500_000u128.into(),
                    AssetSymbol(b"VAL".to_vec()),
                    18,
                ),
            ],
            dex_list: vec![(
                DEX_A_ID,
                DEXInfo {
                    base_asset_id: GetBaseAssetId::get(),
                    is_public: true,
                },
            )],
            initial_permission_owners: vec![
                (INIT_DEX, Scope::Unlimited, vec![alice()]),
                (MANAGE_DEX, Scope::Limited(hash(&DEX_A_ID)), vec![alice()]),
            ],
            initial_permissions: vec![
                (alice(), Scope::Unlimited, vec![INIT_DEX]),
                (alice(), Scope::Limited(hash(&DEX_A_ID)), vec![MANAGE_DEX]),
                (
                    assets_owner(),
                    Scope::Unlimited,
                    vec![permissions::MINT, permissions::BURN],
                ),
            ],
            reference_asset_id: USDT,
        }
    }
}

impl ExtBuilder {
    pub fn new(endowed_accounts: Vec<(AccountId, AssetId, Balance, AssetSymbol, u8)>) -> Self {
        Self {
            endowed_accounts,
            ..Default::default()
        }
    }

    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        crate::GenesisConfig::<Runtime> {
            distribution_accounts: Default::default(),
            reserves_account_id: Default::default(),
            reference_asset_id: self.reference_asset_id,
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

        assets::GenesisConfig::<Runtime> {
            endowed_assets: self
                .endowed_accounts
                .iter()
                .cloned()
                .map(|(account_id, asset_id, _, symbol, precision)| {
                    (asset_id, account_id, symbol, precision)
                })
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        pallet_balances::GenesisConfig::<Runtime> {
            balances: self
                .endowed_accounts
                .iter()
                .cloned()
                .filter_map(|(account_id, asset_id, balance, ..)| {
                    if asset_id == GetBaseAssetId::get() {
                        Some((account_id, balance))
                    } else {
                        None
                    }
                })
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: self
                .endowed_accounts
                .into_iter()
                .map(|(account_id, asset_id, balance, ..)| (account_id, asset_id, balance))
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        t.into()
    }
}
