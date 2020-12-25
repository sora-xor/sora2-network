pub use crate::{self as xor_fee, Module, Trait};
use codec::{Decode, Encode};
use common::{
    self, fixed_from_basis_points, prelude::Balance, Amount, AssetId32, AssetSymbol, Fixed, VAL,
    XOR,
};
use core::time::Duration;
use currencies::BasicCurrencyAdapter;
use frame_support::{
    impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types,
    traits::Get,
    weights::{DispatchInfo, IdentityFee, PostDispatchInfo, Weight},
};
use frame_system as system;
use pallet_balances::WeightInfo;
use permissions::{Scope, BURN, MINT, TRANSFER};
use sp_core::H256;
use sp_runtime::{
    testing::{Header, TestXt, UintAuthorityId},
    traits::{BlakeTwo256, Convert, IdentityLookup, SaturatedConversion},
    Perbill, Percent,
};

// Configure a mock runtime to test the pallet.
type AssetId = AssetId32<common::AssetId>;

/// Simple structure that exposes how u64 currency can be represented as... u64.
pub struct CurrencyToVoteHandler;
impl Convert<Balance, u64> for CurrencyToVoteHandler {
    fn convert(x: Balance) -> u64 {
        x.saturated_into()
    }
}
impl Convert<u128, Balance> for CurrencyToVoteHandler {
    fn convert(x: u128) -> Balance {
        x.saturated_into()
    }
}

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;
impl pallet_session::OneSessionHandler<AccountId> for OtherSessionHandler {
    type Key = UintAuthorityId;

    fn on_genesis_session<'a, I: 'a>(_: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_new_session<'a, I: 'a>(_: bool, _validators: I, _: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
        AccountId: 'a,
    {
    }

    fn on_disabled(_validator_index: usize) {}
}

impl sp_runtime::BoundToRuntimeAppPublic for OtherSessionHandler {
    type Public = UintAuthorityId;
}

pub struct Period;
impl Get<BlockNumber> for Period {
    fn get() -> BlockNumber {
        1u64
    }
}

impl_outer_origin! {
    pub enum Origin for Test {}
}

impl_outer_dispatch! {
    pub enum Call for Test where origin: Origin {
        pallet_balances::Balances,
        frame_system::System,
        pallet_staking::Staking,
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
pub type Timestamp = pallet_timestamp::Module<Test>;
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::AssetId, DEXId>;
type DEXId = common::DEXId;
pub type AccountId = u64;
pub type BlockNumber = u64;
pub type MockLiquiditySource =
    mock_liquidity_source::Module<Test, mock_liquidity_source::Instance1>;
pub type Tokens = tokens::Module<Test>;
pub type Staking = pallet_staking::Module<Test>;
pub type Session = pallet_session::Module<Test>;

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWeight: u32 = 50;
    pub const ExistentialDeposit: u32 = 1;
    pub const TransactionByteFee: u32 = 0;
    pub const ExtrinsicBaseWeight: u32 = 0;
    pub const XorId: AssetId = XOR;
    pub const ValId: AssetId = VAL;
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
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
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
    type WeightInfo = ();
}

impl trading_pair::Trait for Test {
    type Event = ();
    type EnsureDEXOwner = dex_manager::Module<Test>;
    type WeightInfo = ();
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
    type MaxLocks = ();
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
    type WeightInfo = ();
}

impl currencies::Trait for Test {
    type Event = ();
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Test as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Trait for Test {
    type Event = ();
    type AssetId = AssetId;
    type GetBaseAssetId = XorId;
    type Currency = currencies::Module<Test>;
    type WeightInfo = ();
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
    type WeightInfo = ();
}

parameter_types! {
    pub const Offset: BlockNumber = 0;
    pub const UncleGenerations: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
}
sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub other: OtherSessionHandler,
    }
}
impl pallet_session::Trait for Test {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Test, Staking>;
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type Event = ();
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Test>;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = ();
    type WeightInfo = ();
}

impl pallet_session::historical::Trait for Test {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Test>;
}

parameter_types! {
    pub const MinimumPeriod: u64 = 5;
}
impl pallet_timestamp::Trait for Test {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

parameter_types! {
    pub const BondingDuration: pallet_staking::EraIndex = 3;
    pub const MaxNominatorRewardedPerValidator: u32 = 64;
    pub const UnsignedPriority: u64 = 1 << 20;
    pub const MinSolutionScoreBump: Perbill = Perbill::zero();
    pub const TestValRewardCurve: pallet_staking::ValRewardCurve = pallet_staking::ValRewardCurve {
        duration_to_reward_flatline: Duration::from_millis(100_000),
        min_val_burned_percentage_reward: Percent::from_percent(35),
        max_val_burned_percentage_reward: Percent::from_percent(90),
    };
}

impl pallet_staking::Trait for Test {
    type Currency = Balances;
    type MultiCurrency = Tokens;
    type ValTokenId = ValId;
    type ValRewardCurve = TestValRewardCurve;
    type UnixTime = Timestamp;
    type CurrencyToVote = CurrencyToVoteHandler;
    type Event = ();
    type Slash = ();
    type SessionsPerEra = ();
    type SlashDeferDuration = ();
    type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type BondingDuration = BondingDuration;
    type SessionInterface = Self;
    type NextNewSession = Session;
    type ElectionLookahead = ();
    type Call = Call;
    type MaxIterations = ();
    type MinSolutionScoreBump = MinSolutionScoreBump;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type UnsignedPriority = UnsignedPriority;
    type WeightInfo = ();
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Test
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = Extrinsic;
}

pub type Extrinsic = TestXt<Call, ()>;

impl Trait for Test {
    type Event = ();
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type XorId = XorId;
    type ValId = ValId;
    type DEXIdValue = DEXIdValue;
    type LiquiditySource = MockLiquiditySource;
    type ValBurnedNotifier = Staking;
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

pub const REFERRER_ACCOUNT: u64 = 3;
pub const FROM_ACCOUNT: u64 = 1;
pub const TO_ACCOUNT: u64 = 2;
pub const STASH_ACCOUNT: u64 = 11;
pub const STASH_ACCOUNT2: u64 = 21;
pub const CONTROLLER_ACCOUNT: u64 = 10;
pub const CONTROLLER_ACCOUNT2: u64 = 20;
pub const INITIAL_BALANCE: u64 = 1_000;
pub const TRANSFER_AMOUNT: u64 = 69;
pub const INITIAL_RESERVES: u128 = 10_000;

pub struct ExtBuilder;

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
                (STASH_ACCOUNT, initial_balance),
                (STASH_ACCOUNT2, initial_balance),
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

        technical::GenesisConfig::<Test> {
            account_ids_to_tech_account_ids: vec![(
                xor_fee_account_id.clone(),
                tech_account_id.clone(),
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Test> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![xor_fee_account_id]),
                (BURN, Scope::Unlimited, vec![xor_fee_account_id]),
                (TRANSFER, Scope::Unlimited, vec![xor_fee_account_id]),
            ],
            initial_permissions: vec![(xor_fee_account_id, Scope::Unlimited, vec![MINT, BURN])],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Test> {
            endowed_assets: vec![
                (XOR, xor_fee_account_id, AssetSymbol(b"XOR".to_vec()), 18),
                (VAL, xor_fee_account_id, AssetSymbol(b"VAL".to_vec()), 18),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Test> {
            endowed_accounts: vec![(xor_fee_account_id.clone(), VAL, 1_000_u128.into())],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let stakers = vec![
            // (stash, controller, staked_amount, status)
            (
                STASH_ACCOUNT,
                CONTROLLER_ACCOUNT,
                1_000_u32.into(),
                pallet_staking::StakerStatus::<AccountId>::Validator,
            ),
            (
                STASH_ACCOUNT2,
                CONTROLLER_ACCOUNT2,
                1_000_u32.into(),
                pallet_staking::StakerStatus::<AccountId>::Validator,
            ),
        ];

        pallet_staking::GenesisConfig::<Test> {
            stakers: stakers,
            validator_count: 2_u32,
            minimum_validator_count: 0_u32,
            invulnerables: vec![],
            slash_reward_fraction: Perbill::from_percent(10),
            ..Default::default()
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let validators = vec![STASH_ACCOUNT as AccountId, STASH_ACCOUNT2 as AccountId];
        pallet_session::GenesisConfig::<Test> {
            keys: validators
                .iter()
                .map(|x| {
                    (
                        *x,
                        *x,
                        SessionKeys {
                            other: UintAuthorityId(*x as u64),
                        },
                    )
                })
                .collect(),
        }
        .assimilate_storage(&mut t)
        .unwrap();

        mock_liquidity_source::GenesisConfig::<Test, mock_liquidity_source::Instance1> {
            reserves: vec![(
                common::DEXId::Polkaswap,
                VAL,
                (INITIAL_RESERVES.into(), INITIAL_RESERVES.into()),
            )],
            phantom: Default::default(),
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
