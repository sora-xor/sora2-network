#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

#[macro_use]
extern crate alloc;
use alloc::string::String;

/// Constant values used within the runtime.
pub mod constants;
use constants::time::*;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use core::time::Duration;
use currencies::BasicCurrencyAdapter;
pub use farming::{
    domain::{FarmInfo, FarmerInfo},
    FarmId,
};
use frame_system::offchain::{Account, SigningTypes};
use hex_literal::hex;
use pallet_grandpa::fg_primitives;
use pallet_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use pallet_session::historical as pallet_session_historical;
use sp_api::impl_runtime_apis;
use sp_core::Encode;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::{
        BlakeTwo256, Block as BlockT, Bounded, Convert, IdentifyAccount, IdentityLookup, NumberFor,
        OpaqueKeys, SaturatedConversion, Saturating, Verify, Zero,
    },
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, FixedPointNumber, MultiSignature, Perbill, Percent, Perquintill,
};
use sp_std::marker::PhantomData;
use sp_std::prelude::*;
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::assert_eq_size;

// A few exports that help ease life for downstream crates.
pub use common::{
    fixed, fixed_from_basis_points,
    prelude::{Balance, SwapAmount, SwapOutcome, SwapVariant, WeightToFixedFee},
    AssetSymbol, BalancePrecision, BasisPoints, FilterMode, Fixed, FromGenericPair,
    LiquiditySource, LiquiditySourceFilter, LiquiditySourceId, LiquiditySourceType,
};
pub use frame_support::{
    construct_runtime, debug, parameter_types,
    traits::KeyOwnerProofSystem,
    traits::Randomness,
    weights::constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    weights::{constants::WEIGHT_PER_SECOND, Weight},
    StorageValue,
};
pub use pallet_balances::Call as BalancesCall;
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub use bonding_curve_pool;
pub use eth_bridge;

/// An index to a block.
pub type BlockNumber = u32;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

// This assert is needed for `technical` pallet in order to create
// `AccountId` from the hash type.
assert_eq_size!(AccountId, sp_core::H256);

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Index of a transaction in the chain.
pub type Index = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem<Hash>;

/// Identification of DEX.
pub type DEXId = u32;

pub type Moment = u64;

pub type PeriodicSessions = pallet_session::PeriodicSessions<SessionPeriod, SessionOffset>;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core datastructures.
pub mod opaque {
    use super::*;

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;

    impl_opaque_keys! {
        pub struct SessionKeys {
            pub babe: Babe,
            pub grandpa: Grandpa,
        }
    }
}

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("sora-substrate"),
    impl_name: create_runtime_str!("sora-substrate"),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
};

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
    /// Assume 10% of weight for average on_initialize calls.
    pub MaximumExtrinsicWeight: Weight = AvailableBlockRatio::get()
        .saturating_sub(Perbill::from_percent(10)) * MaximumBlockWeight::get();
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
    pub const Version: RuntimeVersion = VERSION;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub const UncleGenerations: BlockNumber = 5;
    pub const GetValAssetId: AssetId = common::AssetId32 { code: [2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
    pub const SessionsPerEra: sp_staking::SessionIndex = 3; // 3 hours
    pub const BondingDuration: pallet_staking::EraIndex = 4; // 12 hours
    pub const SlashDeferDuration: pallet_staking::EraIndex = 2; // 6 hours
    pub const MaxNominatorRewardedPerValidator: u32 = 256;
    pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
    pub const MaxIterations: u32 = 10;
    // 0.05%. The higher the value, the more strict solution acceptance becomes.
    pub MinSolutionScoreBump: Perbill = Perbill::from_rational_approximation(5u32, 10_000);
    pub const ValRewardCurve: pallet_staking::ValRewardCurve = pallet_staking::ValRewardCurve {
        duration_to_reward_flatline: Duration::from_secs(5 * 365 * 24 * 60 * 60),
        min_val_burned_percentage_reward: Percent::from_percent(35),
        max_val_burned_percentage_reward: Percent::from_percent(90),
    };
    pub const SessionPeriod: BlockNumber = 150;
    pub const SessionOffset: BlockNumber = 0;
}

impl frame_system::Trait for Runtime {
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The aggregated dispatch type that is available for extrinsics.
    type Call = Call;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = IdentityLookup<AccountId>;
    /// The index type for storing how many extrinsics an account has signed.
    type Index = Index;
    /// The index type for blocks.
    type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The header type.
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// The ubiquitous event type.
    type Event = Event;
    /// The ubiquitous origin type.
    type Origin = Origin;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// Maximum weight of each block. With a default weight system of 1byte == 1weight, 4mb is ok.
    type MaximumBlockWeight = MaximumBlockWeight;
    /// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
    type MaximumBlockLength = MaximumBlockLength;
    /// Portion of the block weight that is available to all normal transactions.
    type AvailableBlockRatio = AvailableBlockRatio;
    /// Runtime version.
    type Version = Version;
    /// Converts a module to an index of this module in the runtime.
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
    type BlockExecutionWeight = ();
    type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
    type BaseCallFilter = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
}

impl pallet_babe::Trait for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type KeyOwnerProofSystem = Historical;
    type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        pallet_babe::AuthorityId,
    )>>::Proof;
    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        pallet_babe::AuthorityId,
    )>>::IdentificationTuple;
    type HandleEquivocation = pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, ()>;
    type WeightInfo = ();
}

impl pallet_grandpa::Trait for Runtime {
    type Event = Event;
    type Call = Call;

    type KeyOwnerProofSystem = Historical;

    type KeyOwnerProof =
        <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        GrandpaId,
    )>>::IdentificationTuple;

    type HandleEquivocation = pallet_grandpa::EquivocationHandler<Self::KeyOwnerIdentification, ()>;
    type WeightInfo = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Trait for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_session::Trait for Runtime {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
    type Keys = opaque::SessionKeys;
    type ShouldEndSession = Babe;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Event = Event;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type DisabledValidatorsThreshold = ();
    type NextSessionRotation = Babe;
    type WeightInfo = ();
}

impl pallet_session::historical::Trait for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

impl pallet_authorship::Trait for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = (Staking, ()); // ImOnline
}

pub struct CurrencyToVoteHandler;
impl Convert<Balance, u64> for CurrencyToVoteHandler {
    fn convert(x: Balance) -> u64 {
        x.saturated_into()
    }
}
impl Convert<u128, Balance> for CurrencyToVoteHandler {
    fn convert(x: u128) -> Balance {
        x.into()
    }
}

impl pallet_staking::Trait for Runtime {
    type Currency = Balances;
    type MultiCurrency = Tokens;
    type ValTokenId = GetValAssetId;
    type ValRewardCurve = ValRewardCurve;
    type UnixTime = Timestamp;
    type CurrencyToVote = CurrencyToVoteHandler;
    type Event = Event;
    type Slash = ();
    type SessionsPerEra = SessionsPerEra;
    type SlashDeferDuration = SlashDeferDuration;
    type SlashCancelOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type BondingDuration = BondingDuration;
    type SessionInterface = Self;
    type NextNewSession = Session;
    type ElectionLookahead = ElectionLookahead;
    type Call = Call;
    type MaxIterations = MaxIterations;
    type MinSolutionScoreBump = MinSolutionScoreBump;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type UnsignedPriority = UnsignedPriority;
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
}

impl pallet_balances::Trait for Runtime {
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

pub type Amount = i128;

impl tokens::Trait for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

parameter_types! {
    // This is common::AssetId with 0 index, 2 is size, 0 and 0 is code.
    pub const XorId: AssetId = common::AssetId32::from_bytes(hex!("0200000000000000000000000000000000000000000000000000000000000000"));
    pub const DotId: AssetId = common::AssetId32::from_bytes(hex!("0200010000000000000000000000000000000000000000000000000000000000"));
    pub const KsmId: AssetId = common::AssetId32::from_bytes(hex!("0200020000000000000000000000000000000000000000000000000000000000"));
    pub const UsdId: AssetId = common::AssetId32::from_bytes(hex!("0200030000000000000000000000000000000000000000000000000000000000"));
    pub const ValId: AssetId = common::AssetId32::from_bytes(hex!("0200040000000000000000000000000000000000000000000000000000000000"));
    pub const PswapId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000"));

    pub const GetBaseAssetId: AssetId = XorId::get();
}

impl currencies::Trait for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

impl assets::Trait for Runtime {
    type Event = Event;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl trading_pair::Trait for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

impl dex_manager::Trait for Runtime {
    type Event = Event;
    type WeightInfo = ();
}

impl bonding_curve_pool::Trait for Runtime {
    type DEXApi = ();
}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::AssetId, DEXId, common::LiquiditySourceType>;
pub type AssetId = common::AssetId32<common::AssetId>;

impl technical::Trait for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction =
        pool_xyk::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WeightInfo = ();
}

impl pool_xyk::Trait for Runtime {
    type Event = Event;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, Balance, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type PolySwapAction =
        pool_xyk::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type WeightInfo = ();
}

parameter_types! {
    pub GetLiquidityProxyTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            pswap_distribution::TECH_ACCOUNT_PREFIX.to_vec(),
            pswap_distribution::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetLiquidityProxyAccountId: AccountId = {
        let tech_account_id = GetLiquidityProxyTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetNumSamples: usize = 40;
}

impl liquidity_proxy::Trait for Runtime {
    type Event = Event;
    type LiquidityRegistry = dex_api::Module<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
    type WeightInfo = ();
}

parameter_types! {
    pub GetFee: Fixed = fixed_from_basis_points(30u16);
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance1> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance2> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance3> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance4> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl dex_api::Trait for Runtime {
    type Event = Event;
    type MockLiquiditySource =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
    type MockLiquiditySource2 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance2>;
    type MockLiquiditySource3 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance3>;
    type MockLiquiditySource4 =
        mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance4>;
    type BondingCurvePool = bonding_curve_pool::Module<Runtime>;
    type XYKPool = pool_xyk::Module<Runtime>;
    type WeightInfo = ();
}

impl farming::Trait for Runtime {
    type Event = Event;
    type WeightInfo = ();
}

impl pallet_multisig::Trait for Runtime {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl iroha_migration::Trait for Runtime {
    type Event = Event;
}

impl<T: SigningTypes> frame_system::offchain::SignMessage<T> for Runtime {
    type SignatureData = ();

    fn sign_message(&self, _message: &[u8]) -> Self::SignatureData {
        unimplemented!()
    }

    fn sign<TPayload, F>(&self, _f: F) -> Self::SignatureData
    where
        F: Fn(&Account<T>) -> TPayload,
        TPayload: frame_system::offchain::SignedPayload<T>,
    {
        unimplemented!()
    }
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        public: <Signature as sp_runtime::traits::Verify>::Signer,
        account: AccountId,
        index: Index,
    ) -> Option<(
        Call,
        <UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
    )> {
        let period = BlockHashCount::get() as u64;
        let current_block = System::block_number()
            .saturated_into::<u64>()
            .saturating_sub(1);
        let tip = 0u32;
        let extra: SignedExtra = (
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
            frame_system::CheckNonce::<Runtime>::from(index),
            frame_system::CheckWeight::<Runtime>::new(),
            pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip.into()),
        );
        #[cfg_attr(not(feature = "std"), allow(unused_variables))]
        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|e| {
                debug::native::warn!("SignedPayload error: {:?}", e);
            })
            .ok()?;

        let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;

        let address = account;
        let (call, extra, _) = raw_payload.deconstruct();
        Some((call, (address, signature, extra)))
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as sp_runtime::traits::Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    Call: From<C>,
{
    type OverarchingCall = Call;
    type Extrinsic = UncheckedExtrinsic;
}

impl referral_system::Trait for Runtime {
    type Event = Event;
}

parameter_types! {
    pub const DEXIdValue: DEXId = 0;
}

impl xor_fee::Trait for Runtime {
    // Pass native currency.
    type Event = Event;
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type XorId = XorId;
    type ValId = ValId;
    type DEXIdValue = DEXIdValue;
    type LiquiditySource = mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
    type ValBurnedNotifier = Staking;
}

parameter_types! {
    pub const TransactionByteFee: Balance = Balance(Fixed::from_bits(1_000_000_000_000_i128)); // 10^-6 XOR ~ 10 * MILLICENTS
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
    pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000_u128);
}

impl pallet_transaction_payment::Trait for Runtime {
    // Pass native currency.
    type Currency = Balances;
    type OnTransactionPayment = XorFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = WeightToFixedFee;
    type FeeMultiplierUpdate =
        TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
}

impl pallet_sudo::Trait for Runtime {
    type Call = Call;
    type Event = Event;
}

impl permissions::Trait for Runtime {
    type Event = Event;
}

impl pallet_utility::Trait for Runtime {
    type Event = Event;
    type Call = Call;
    type WeightInfo = ();
}

parameter_types! {
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
}

impl bridge_multisig::Trait for Runtime {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl eth_bridge::Trait for Runtime {
    type Event = Event;
    type Call = Call;
    type PeerId = eth_bridge::crypto::TestAuthId;
}

impl faucet::Trait for Runtime {
    type Event = Event;
}

parameter_types! {
    pub GetPswapDistributionTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            pswap_distribution::TECH_ACCOUNT_PREFIX.to_vec(),
            pswap_distribution::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetPswapDistributionAccountId: AccountId = {
        let tech_account_id = GetPswapDistributionTechAccountId::get();
        let account_id =
            technical::Module::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 14400;
}

impl pswap_distribution::Trait for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = PswapId;
    type Exchange = LiquidityProxy;
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = DEXManager;
}

/// Payload data to be signed when making signed transaction from off-chain workers,
///   inside `create_transaction` function.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;

parameter_types! {
    pub const UnsignedPriority: u64 = 100;
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWeight: u32 = 50;
}

construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Module, Call, Storage, Config, Event<T>},
        Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
        // Balances in native currency - XOR.
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        Sudo: pallet_sudo::{Module, Call, Storage, Config<T>, Event<T>},
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
        TransactionPayment: pallet_transaction_payment::{Module, Storage},
        Permissions: permissions::{Module, Call, Storage, Config<T>, Event<T>},
        ReferralSystem: referral_system::{Module, Call, Storage, Event},
        XorFee: xor_fee::{Module, Call, Storage, Event},
        BridgeMultisig: bridge_multisig::{Module, Call, Storage, Config<T>, Event<T>},
        Utility: pallet_utility::{Module, Call, Event},

        // Consensus and staking.
        Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
        Historical: pallet_session_historical::{Module},
        Babe: pallet_babe::{Module, Call, Storage, Config, Inherent, ValidateUnsigned},
        Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event},
        Authorship: pallet_authorship::{Module, Call, Storage, Inherent},
        Staking: pallet_staking::{Module, Call, Config<T>, Storage, Event<T>},

        // Non-native tokens - everything apart of XOR.
        Tokens: tokens::{Module, Storage, Config<T>, Event<T>},
        // Unified interface for XOR and non-native tokens.
        Currencies: currencies::{Module, Call, Event<T>},
        TradingPair: trading_pair::{Module, Call, Event<T>},
        Assets: assets::{Module, Call, Storage, Config<T>, Event<T>},
        DEXManager: dex_manager::{Module, Call, Storage, Config<T>, Event<T>},
        BondingCurvePool: bonding_curve_pool::{Module, Call, Storage, Config<T>},
        Technical: technical::{Module, Call, Config<T>, Event<T>},
        PoolXYK: pool_xyk::{Module, Call, Storage, Event<T>},
        LiquidityProxy: liquidity_proxy::{Module, Call, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Module, Call, Storage, Config<T>, Event<T>},
        MockLiquiditySource2: mock_liquidity_source::<Instance2>::{Module, Call, Storage, Config<T>, Event<T>},
        MockLiquiditySource3: mock_liquidity_source::<Instance3>::{Module, Call, Storage, Config<T>, Event<T>},
        MockLiquiditySource4: mock_liquidity_source::<Instance4>::{Module, Call, Storage, Config<T>, Event<T>},
        DEXAPI: dex_api::{Module, Call, Storage, Config, Event<T>},
        Faucet: faucet::{Module, Call, Config<T>, Event<T>, ValidateUnsigned},
        EthBridge: eth_bridge::{Module, Call, Config<T>, Event<T>},
        Farming: farming::{Module, Call, Storage, Config<T>, Event<T>},
        PswapDistribution: pswap_distribution::{Module, Call, Storage, Config<T>, Event<T>},
        Multisig: pallet_multisig::{Module, Call, Storage, Event<T>},
        IrohaMigration: iroha_migration::{Module, Call, Storage, Config<T>, Event<T>},
    }
}

/// The address format for describing accounts.
pub type Address = AccountId;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllModules,
>;

impl_runtime_apis! {
    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            Runtime::metadata().into()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(
            extrinsic: <Block as BlockT>::Extrinsic,
        ) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(block: Block, data: sp_inherents::InherentData) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }

        fn random_seed() -> <Block as BlockT>::Hash {
            RandomnessCollectiveFlip::random_seed()
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
            opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
        }

        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            opaque::SessionKeys::generate(seed)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
        Block,
        Balance,
    > for Runtime {
        fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
    }

    impl dex_manager_runtime_api::DEXManagerAPI<Block, DEXId> for Runtime {
        fn list_dex_ids() -> Vec<DEXId> {
            DEXManager::list_dex_ids()
        }
    }

    impl dex_runtime_api::DEXAPI<
        Block,
        AssetId,
        DEXId,
        Balance,
        LiquiditySourceType,
        SwapVariant,
    > for Runtime {
        fn quote(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            desired_input_amount: Balance,
            swap_variant: SwapVariant,
        ) -> Option<dex_runtime_api::SwapOutcomeInfo<Balance>> {
            // TODO: remove with proper QuoteAmount refactor
            let limit = if swap_variant == SwapVariant::WithDesiredInput {
                Balance::zero()
            } else {
                Balance::max_value()
            };
            DEXAPI::quote(
                &LiquiditySourceId::new(dex_id, liquidity_source_type),
                &input_asset_id,
                &output_asset_id,
                SwapAmount::with_variant(swap_variant, desired_input_amount.0, limit.0),
            ).ok().map(|sa| dex_runtime_api::SwapOutcomeInfo::<Balance> { amount: Balance(sa.amount), fee: Balance(sa.fee)})
        }

        fn can_exchange(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
        ) -> bool {
            DEXAPI::can_exchange(
                &LiquiditySourceId::new(dex_id, liquidity_source_type),
                &input_asset_id,
                &output_asset_id,
            )
        }

        fn list_supported_sources() -> Vec<LiquiditySourceType> {
            DEXAPI::get_supported_types()
        }
    }

    impl trading_pair_runtime_api::TradingPairAPI<Block, DEXId, common::TradingPair<AssetId>, AssetId, LiquiditySourceType> for Runtime {
        fn list_enabled_pairs(dex_id: DEXId) -> Vec<common::TradingPair<AssetId>> {
            // TODO: error passing PR fixes this crunch return
            TradingPair::list_trading_pairs(&dex_id).unwrap_or(Vec::new())
        }

        fn is_pair_enabled(dex_id: DEXId, asset_id_a: AssetId, asset_id_b: AssetId) -> bool {
            // TODO: error passing PR fixes this crunch return
            TradingPair::is_trading_pair_enabled(&dex_id, &asset_id_a, &asset_id_b).unwrap_or(false)
                || TradingPair::is_trading_pair_enabled(&dex_id, &asset_id_b, &asset_id_a).unwrap_or(false)
        }

        fn list_enabled_sources_for_pair(
            dex_id: DEXId,
            base_asset_id: AssetId,
            target_asset_id: AssetId,
        ) -> Vec<LiquiditySourceType> {
            // TODO: error passing PR fixes this crunch return
            TradingPair::list_enabled_sources_for_trading_pair(&dex_id, &base_asset_id, &target_asset_id).map(|bts| bts.into_iter().collect::<Vec<_>>()).unwrap_or(Vec::new())
        }

        fn is_source_enabled_for_pair(
            dex_id: DEXId,
            base_asset_id: AssetId,
            target_asset_id: AssetId,
            source_type: LiquiditySourceType,
        ) -> bool {
            // TODO: error passing PR fixes this crunch return
            TradingPair::is_source_enabled_for_trading_pair(&dex_id, &base_asset_id, &target_asset_id, source_type).unwrap_or(false)
        }
    }

    impl assets_runtime_api::AssetsAPI<Block, AccountId, AssetId, Balance, AssetSymbol, BalancePrecision> for Runtime {
        fn free_balance(account_id: AccountId, asset_id: AssetId) -> Option<assets_runtime_api::BalanceInfo<Balance>> {
            Assets::free_balance(&asset_id, &account_id).ok().map(|balance|
                assets_runtime_api::BalanceInfo::<Balance> {
                    balance: balance.clone(),
                }
            )
        }

        fn total_balance(account_id: AccountId, asset_id: AssetId) -> Option<assets_runtime_api::BalanceInfo<Balance>> {
            Assets::total_balance(&asset_id, &account_id).ok().map(|balance|
                assets_runtime_api::BalanceInfo::<Balance> {
                    balance: balance.clone(),
                }
            )
        }

        fn total_supply(asset_id: AssetId) -> Option<assets_runtime_api::BalanceInfo<Balance>> {
            Assets::total_issuance(&asset_id).ok().map(|balance|
                assets_runtime_api::BalanceInfo::<Balance> {
                    balance: balance.clone(),
                }
            )
        }

        fn list_asset_ids() -> Vec<AssetId> {
            Assets::list_registered_asset_ids()
        }

        fn list_asset_infos() -> Vec<assets_runtime_api::AssetInfo<AssetId, AssetSymbol, u8>> {
            Assets::list_registered_asset_infos().into_iter().map(|(asset_id, symbol, precision)|
                assets_runtime_api::AssetInfo::<AssetId, AssetSymbol, BalancePrecision> {
                    asset_id, symbol, precision
                }
            ).collect()
        }

        fn get_asset_info(asset_id: AssetId) -> Option<assets_runtime_api::AssetInfo<AssetId, AssetSymbol, BalancePrecision>> {
            let (symbol, precision) = Assets::get_asset_info(&asset_id);
            Some(assets_runtime_api::AssetInfo::<AssetId, AssetSymbol, BalancePrecision> {
                asset_id, symbol, precision
            })
        }
    }

    impl farming_runtime_api::FarmingRuntimeApi<Block, AccountId, FarmId, FarmInfo<AccountId, AssetId, BlockNumber>, FarmerInfo<AccountId, TechAccountId, BlockNumber>> for Runtime {
        fn get_farm_info(who: AccountId, name: FarmId) -> Option<FarmInfo<AccountId, AssetId, BlockNumber>> {
            Farming::get_farm_info(who, name).ok()?
        }

        fn get_farmer_info(who: AccountId, name: FarmId) -> Option<FarmerInfo<AccountId, TechAccountId, BlockNumber>> {
            Farming::get_farmer_info(who, name).ok()?
        }
    }

    impl iroha_migration_runtime_api::IrohaMigrationAPI<Block> for Runtime {
        fn needs_migration(iroha_address: String) -> bool {
            IrohaMigration::needs_migration(&iroha_address)
        }
    }

    impl liquidity_proxy_runtime_api::LiquidityProxyAPI<
        Block,
        DEXId,
        AssetId,
        Balance,
        SwapVariant,
        LiquiditySourceType,
        FilterMode,
    > for Runtime {
        fn quote(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            amount: Balance,
            swap_variant: SwapVariant,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> Option<liquidity_proxy_runtime_api::SwapOutcomeInfo<Balance>> {
            // TODO: remove with proper QuoteAmount refactor
            let limit = if swap_variant == SwapVariant::WithDesiredInput {
                Balance::zero()
            } else {
                Balance::max_value()
            };
            LiquidityProxy::quote_with_filter(
                &input_asset_id,
                &output_asset_id,
                SwapAmount::with_variant(swap_variant, amount.0, limit.0),
                LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
            ).ok().map(|asa| liquidity_proxy_runtime_api::SwapOutcomeInfo::<Balance> { amount: Balance(asa.amount), fee: Balance(asa.fee)})
        }
    }

       impl sp_consensus_babe::BabeApi<Block> for Runtime {
               fn configuration() -> sp_consensus_babe::BabeGenesisConfiguration {
                       // The choice of `c` parameter (where `1 - c` represents the
                       // probability of a slot being empty), is done in accordance to the
                       // slot duration and expected target block time, for safely
                       // resisting network delays of maximum two seconds.
                       // <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
                       sp_consensus_babe::BabeGenesisConfiguration {
                               slot_duration: Babe::slot_duration(),
                               epoch_length: EpochDuration::get(),
                               c: PRIMARY_PROBABILITY,
                               genesis_authorities: Babe::authorities(),
                               randomness: Babe::randomness(),
                               allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
                       }
               }
               fn current_epoch_start() -> sp_consensus_babe::SlotNumber {
                       Babe::current_epoch_start()
               }
               fn generate_key_ownership_proof(
                       _slot_number: sp_consensus_babe::SlotNumber,
                       authority_id: sp_consensus_babe::AuthorityId,
               ) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
                       use codec::Encode;
                       Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
                               .map(|p| p.encode())
                               .map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
               }
               fn submit_report_equivocation_unsigned_extrinsic(
                       equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
                       key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
               ) -> Option<()> {
                       let key_owner_proof = key_owner_proof.decode()?;
                       Babe::submit_unsigned_equivocation_report(
                               equivocation_proof,
                               key_owner_proof,
                       )
               }
       }



    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
        fn account_nonce(account: AccountId) -> Index {
            System::account_nonce(account)
        }
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: fg_primitives::EquivocationProof<
                <Block as BlockT>::Hash,
                NumberFor<Block>,
            >,
            key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;
            Grandpa::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: fg_primitives::SetId,
            authority_id: GrandpaId,
        ) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
            use codec::Encode;
            Historical::prove((fg_primitives::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(fg_primitives::OpaqueKeyOwnershipProof::new)
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};

            use dex_api_benchmarking::Module as DEXAPIBench;
            use liquidity_proxy_benchmarking::Module as LiquidityProxyBench;
            use pool_xyk_benchmarking::Module as XYKPoolBench;

            impl dex_api_benchmarking::Trait for Runtime {}
            impl liquidity_proxy_benchmarking::Trait for Runtime {}
            impl pool_xyk_benchmarking::Trait for Runtime {}

            let whitelist: Vec<TrackedStorageKey> = vec![
                // Block Number
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
                // Total Issuance
                hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
                // Execution Phase
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
                // Event Count
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
                // System Events
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec().into(),
                // Treasury Account
                hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da95ecffd7b6c0f78751baa9d281e0bfa3a6d6f646c70792f74727372790000000000000000000000000000000000000000").to_vec().into(),
            ];

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);

            add_benchmark!(params, batches, assets, Assets);
            add_benchmark!(params, batches, dex_api, DEXAPIBench::<Runtime>);
            add_benchmark!(params, batches, dex_manager, DEXManager);
            add_benchmark!(params, batches, faucet, Faucet);
            add_benchmark!(params, batches, liquidity_proxy, LiquidityProxyBench::<Runtime>);
            add_benchmark!(params, batches, trading_pair, TradingPair);
            add_benchmark!(params, batches, pool_xyk, XYKPoolBench::<Runtime>);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }
}
