use codec::{Decode, Encode};
use common::mock::ExistentialDeposits;
use common::prelude::{
    Balance, BlockLength, BlockWeights, SwapAmount, SwapOutcome, TransactionByteFee,
};
use common::{
    self, balance, fixed_from_basis_points, Amount, AssetId32, AssetName, AssetSymbol, Fixed,
    LiquiditySource, LiquiditySourceFilter, LiquiditySourceType, VAL, XOR,
};
use core::time::Duration;
use currencies::BasicCurrencyAdapter;
use frame_support::traits::{GenesisBuild, Get, OneSessionHandler, U128CurrencyToVote};
use frame_support::weights::{DispatchInfo, IdentityFee, PostDispatchInfo, Weight};
use frame_support::{construct_runtime, parameter_types};
use frame_system;
use pallet_session::historical;
use permissions::{Scope, BURN, MINT, TRANSFER};
use sp_core::H256;
use sp_runtime::testing::{Header, TestXt, UintAuthorityId};
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::{DispatchError, Perbill, Percent};

pub use crate::{self as xor_fee, Config, Module};

// Configure a mock runtime to test the pallet.
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type AccountId = u64;
pub type BlockNumber = u64;
type AssetId = AssetId32<common::PredefinedAssetId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const ReferrerWeight: u32 = 300_000_000;
    pub const XorBurnedWeight: u32 = 400_000_000;
    pub const XorIntoValBurnedWeight: u32 = 500_000_000;
    pub const ExistentialDeposit: u32 = 0;
    pub const XorId: AssetId = XOR;
    pub const ValId: AssetId = VAL;
    pub const DEXIdValue: DEXId = common::DEXId::Polkaswap;
    pub GetFee: Fixed = fixed_from_basis_points(0u16);
    pub const Offset: BlockNumber = 0;
    pub const UncleGenerations: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
    pub const MinimumPeriod: u64 = 5;
    pub const BondingDuration: pallet_staking::EraIndex = 3;
    pub const MaxNominatorRewardedPerValidator: u32 = 64;
    pub const UnsignedPriority: u64 = 1 << 20;
    pub const MinSolutionScoreBump: Perbill = Perbill::zero();
    pub const TestValRewardCurve: pallet_staking::ValRewardCurve = pallet_staking::ValRewardCurve {
        duration_to_reward_flatline: Duration::from_millis(100_000),
        min_val_burned_percentage_reward: Percent::from_percent(35),
        max_val_burned_percentage_reward: Percent::from_percent(90),
    };
    pub OffchainSolutionWeightLimit: Weight = 600_000_000;
}

sp_runtime::impl_opaque_keys! {
    pub struct SessionKeys {
        pub other: OtherSessionHandler,
    }
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Module, Call, Config<T>, Storage},
        DexManager: dex_manager::{Module, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Module, Call, Config<T>, Storage, Event<T>},
        ReferralSystem: referral_system::{Module, Call, Config<T>, Storage},
        Balances: pallet_balances::{Module, Call, Storage, Event<T>},
        TransactionPayment: pallet_transaction_payment::{Module, Storage},
        Technical: technical::{Module, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Module, Call, Storage, Event<T>},
        Assets: assets::{Module, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Module, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Module, Call, Config<T>, Storage, Event<T>},
        Session: pallet_session::{Module, Call, Config<T>, Storage, Event},
        Historical: historical::{Module},
        Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
        Staking: pallet_staking::{Module, Call, Config<T>, Storage, Event<T>},
        XorFee: xor_fee::{Module, Call, Event<T>},
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = ();
    type BlockWeights = BlockWeights;
    type BlockLength = BlockLength;
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl referral_system::Config for Runtime {}

impl pallet_balances::Config for Runtime {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl pallet_transaction_payment::Config for Runtime {
    type OnChargeTransaction = XorFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
    type WeightInfo = ();
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = AccountId;
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, AccountId>;
    type AssetId = AssetId;
    type GetBaseAssetId = XorId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
}

impl pallet_session::Config for Runtime {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Runtime, Staking>;
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type Event = Event;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Runtime>;
    type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
    type NextSessionRotation = ();
    type WeightInfo = ();
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_staking::Config for Runtime {
    type Currency = Balances;
    type MultiCurrency = Tokens;
    type ValTokenId = ValId;
    type ValRewardCurve = TestValRewardCurve;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type Event = Event;
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
    type OffchainSolutionWeightLimit = OffchainSolutionWeightLimit;
    type WeightInfo = ();
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Runtime
where
    Call: From<LocalCall>,
{
    type OverarchingCall = Call;
    type Extrinsic = Extrinsic;
}

pub type Extrinsic = TestXt<Call, ()>;

impl Config for Runtime {
    type Event = Event;
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type XorId = XorId;
    type ValId = ValId;
    type DEXIdValue = DEXIdValue;
    type LiquidityProxy = MockLiquidityProxy;
    type ValBurnedNotifier = Staking;
}

pub struct MockLiquidityProxy;

impl liquidity_proxy::LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    fn exchange(
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        MockLiquiditySource::exchange(
            &sender,
            &receiver,
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
        MockLiquiditySource::quote(&filter.dex_id, input_asset_id, output_asset_id, amount)
    }
}

pub const MOCK_WEIGHT: Weight = 600_000_000;

pub const REFERRER_ACCOUNT: u64 = 3;
pub const FROM_ACCOUNT: u64 = 1;
pub const TO_ACCOUNT: u64 = 2;
pub const STASH_ACCOUNT: u64 = 11;
pub const STASH_ACCOUNT2: u64 = 21;
pub const CONTROLLER_ACCOUNT: u64 = 10;
pub const CONTROLLER_ACCOUNT2: u64 = 20;
pub const TRANSFER_AMOUNT: u64 = 69;

pub fn initial_balance() -> Balance {
    balance!(1000)
}

pub fn initial_reserves() -> Balance {
    balance!(10000)
}

/// Another session handler struct to test on_disabled.
pub struct OtherSessionHandler;
impl OneSessionHandler<AccountId> for OtherSessionHandler {
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

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        let initial_balance = initial_balance();
        pallet_balances::GenesisConfig::<Runtime> {
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

        referral_system::GenesisConfig::<Runtime> {
            referrers: vec![(FROM_ACCOUNT, REFERRER_ACCOUNT)],
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

        technical::GenesisConfig::<Runtime> {
            account_ids_to_tech_account_ids: vec![(
                xor_fee_account_id.clone(),
                tech_account_id.clone(),
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![xor_fee_account_id]),
                (BURN, Scope::Unlimited, vec![xor_fee_account_id]),
                (TRANSFER, Scope::Unlimited, vec![xor_fee_account_id]),
            ],
            initial_permissions: vec![(xor_fee_account_id, Scope::Unlimited, vec![MINT, BURN])],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        assets::GenesisConfig::<Runtime> {
            endowed_assets: vec![
                (
                    XOR,
                    xor_fee_account_id,
                    AssetSymbol(b"XOR".to_vec()),
                    AssetName(b"SORA".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
                (
                    VAL,
                    xor_fee_account_id,
                    AssetSymbol(b"VAL".to_vec()),
                    AssetName(b"SORA Validator Token".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                ),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        tokens::GenesisConfig::<Runtime> {
            endowed_accounts: vec![(xor_fee_account_id.clone(), VAL, balance!(1000))],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        let stakers = vec![
            // (stash, controller, staked_amount, status)
            (
                STASH_ACCOUNT,
                CONTROLLER_ACCOUNT,
                balance!(1000),
                pallet_staking::StakerStatus::<AccountId>::Validator,
            ),
            (
                STASH_ACCOUNT2,
                CONTROLLER_ACCOUNT2,
                balance!(1000),
                pallet_staking::StakerStatus::<AccountId>::Validator,
            ),
        ];

        pallet_staking::GenesisConfig::<Runtime> {
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
        pallet_session::GenesisConfig::<Runtime> {
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

        let initial_reserves: Fixed = Fixed::from_bits(initial_reserves() as i128);
        mock_liquidity_source::GenesisConfig::<Runtime, mock_liquidity_source::Instance1> {
            reserves: vec![(
                common::DEXId::Polkaswap,
                VAL,
                (initial_reserves, initial_reserves),
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
