pub use crate::{self as xor_fee, Module, Trait};
use codec::{Decode, Encode};
use common::{fixed_from_basis_points, prelude::Balance, Amount, Fixed};
use currencies::BasicCurrencyAdapter;
use frame_support::{
    impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types,
    weights::{DispatchInfo, IdentityFee, PostDispatchInfo, Weight},
};
use frame_system as system;
use pallet_balances::WeightInfo;
use permissions::{Scope, BURN, EXCHANGE, MINT, TRANSFER};
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};

// Configure a mock runtime to test the pallet.

impl_outer_origin! {
    pub enum Origin for Test {}
}

impl_outer_dispatch! {
    pub enum Call for Test where origin: Origin {
        pallet_balances::Balances,
        frame_system::System,
    }
}

impl_outer_event! {
    pub enum Event for Test {
        frame_system<T>,
        pallet_balances<T>,
        referral_system,
        xor_fee,
    }
}

pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;
pub type XorFee = Module<Test>;
type TechAccountIdPrimitive = common::TechAccountId<AccountId, AssetId, DEXId>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<AssetId, DEXId>;
type DEXId = common::DEXId;
pub type AccountId = u64;
pub type AssetId = common::AssetId;
pub type BlockNumber = u64;
pub type Technical = technical::Module<Test>;
pub type MockLiquiditySource =
    mock_liquidity_source::Module<Test, mock_liquidity_source::Instance1>;
pub type Assets = assets::Module<Test>;
pub type Tokens = tokens::Module<Test>;
pub type Currencies = currencies::Module<Test>;

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWieght: u32 = 50;
    pub const ExistentialDeposit: u32 = 1;
    pub const TransactionByteFee: u32 = 0;
    pub const ExtrinsicBaseWeight: u32 = 0;
    pub const XorId: AssetId = AssetId::XOR;
    pub const ValId: AssetId = AssetId::VAL;
    pub const DEXIdValue: DEXId = common::DEXId::Polkaswap;
}

impl system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = BlockNumber;
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
    type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
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
    pub GetFee: Fixed = fixed_from_basis_points(0u16);
    pub const GetDefaultFee: u16 = 0;
    pub const GetDefaultProtocolFee: u16 = 0;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance1> for Test {
    type Event = ();
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Test>;
    type EnsureTradingPairExists = trading_pair::Module<Test>;
}

impl dex_manager::Trait for Test {
    type Event = ();
    type GetDefaultFee = GetDefaultFee;
    type GetDefaultProtocolFee = GetDefaultProtocolFee;
}

impl trading_pair::Trait for Test {
    type Event = ();
    type EnsureDEXOwner = dex_manager::Module<Test>;
}

impl referral_system::Trait for Test {
    type Event = ();
}

impl pallet_balances::Trait for Test {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = MockWeightInfo;
}

impl pallet_transaction_payment::Trait for Test {
    type Currency = Balances;
    type OnTransactionPayment = XorFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
}

impl common::Trait for Test {
    type DEXId = DEXId;
}

impl technical::Trait for Test {
    type Event = ();
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

impl currencies::Trait for Test {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Balances, Balance, Balance, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Test as assets::Trait>::GetBaseAssetId;
}

impl assets::Trait for Test {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = XorId;
    type Currency = currencies::Module<Test>;
}

impl permissions::Trait for Test {
    type Event = ();
}

impl tokens::Trait for Test {
    type Event = ();
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Trait>::AssetId;
    type OnReceived = ();
}

impl Trait for Test {
    type Event = ();
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWieght;
    type XorId = XorId;
    type ValId = ValId;
    type DEXIdValue = DEXIdValue;
    type LiquiditySource = MockLiquiditySource;
}

pub const MOCK_WEIGHT: u64 = 100;

pub struct MockWeightInfo;

impl WeightInfo for MockWeightInfo {
    fn transfer() -> Weight {
        MOCK_WEIGHT
    }
    fn transfer_keep_alive() -> Weight {
        MOCK_WEIGHT
    }
    fn set_balance_creating() -> Weight {
        MOCK_WEIGHT
    }
    fn set_balance_killing() -> Weight {
        MOCK_WEIGHT
    }
    fn force_transfer() -> Weight {
        MOCK_WEIGHT
    }
}

pub struct ExtBuilder;

pub const REFERRER_ACCOUNT: u64 = 3;
pub const FROM_ACCOUNT: u64 = 1;
pub const TO_ACCOUNT: u64 = 2;
pub const INITIAL_BALANCE: u64 = 1_000;
pub const TRANSFER_AMOUNT: u64 = 69;

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        referral_system::GenesisConfig::<Test> {
            accounts_to_referrers: vec![(FROM_ACCOUNT, REFERRER_ACCOUNT)],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let initial_balance: Balance = INITIAL_BALANCE.into();
        pallet_balances::GenesisConfig::<Test> {
            balances: vec![
                (FROM_ACCOUNT, initial_balance),
                (TO_ACCOUNT, initial_balance),
                (REFERRER_ACCOUNT, initial_balance),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let tech_account_id = TechAccountId::Generic(
            xor_fee::TECH_ACCOUNT_PREFIX.to_vec(),
            xor_fee::TECH_ACCOUNT_MAIN.to_vec(),
        );
        let repr = technical::tech_account_id_encoded_to_account_id_32(&tech_account_id.encode());
        let xor_fee_account_id: AccountId =
            AccountId::decode(&mut &repr[..]).expect("Failed to decode account Id");

        permissions::GenesisConfig::<Test> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![xor_fee_account_id]),
                (BURN, Scope::Unlimited, vec![xor_fee_account_id]),
                (TRANSFER, Scope::Unlimited, vec![xor_fee_account_id]),
                (EXCHANGE, Scope::Unlimited, vec![xor_fee_account_id]),
            ],
            initial_permissions: vec![(
                xor_fee_account_id,
                Scope::Unlimited,
                vec![MINT, BURN, EXCHANGE],
            )],
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

pub fn default_post_info() -> PostDispatchInfo {
    PostDispatchInfo {
        actual_weight: None,
        pays_fee: Default::default(),
    }
}
