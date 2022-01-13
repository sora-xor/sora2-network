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

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

extern crate alloc;
use alloc::string::String;

/// Constant values used within the runtime.
pub mod constants;
mod extensions;
mod impls;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
pub mod tests;

use common::prelude::constants::{BIG_FEE, SMALL_FEE};
use common::prelude::QuoteAmount;
use common::{AssetId32, PredefinedAssetId, ETH};
use constants::time::*;
use dispatch::EnsureEthereumAccount;
use snowbridge_core::ChannelId;

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub use beefy_primitives::crypto::AuthorityId as BeefyId;
use beefy_primitives::mmr::MmrLeafVersion;
use core::marker::PhantomData;
use core::time::Duration;
use currencies::BasicCurrencyAdapter;
use extensions::ChargeTransactionPayment;
use frame_support::traits::{Currency, OnRuntimeUpgrade};
use frame_system::offchain::{Account, SigningTypes};
use frame_system::{EnsureOneOf, EnsureRoot};
use hex_literal::hex;
use pallet_grandpa::{
    fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
use pallet_mmr_primitives as mmr;
use pallet_session::historical as pallet_session_historical;
#[cfg(feature = "std")]
use serde::{Serialize, Serializer};
use sp_api::impl_runtime_apis;
use sp_core::crypto::KeyTypeId;
use sp_core::u32_trait::{_1, _2, _3};
use sp_core::{Encode, OpaqueMetadata, H160, U256};
use sp_runtime::traits::{
    BlakeTwo256, Block as BlockT, Convert, IdentifyAccount, IdentityLookup, NumberFor, OpaqueKeys,
    SaturatedConversion, Verify,
};
use sp_runtime::transaction_validity::{
    TransactionPriority, TransactionSource, TransactionValidity,
};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, DispatchError,
    DispatchResult, MultiSignature, Perbill, Percent, Perquintill,
};
use sp_std::cmp::Ordering;
use sp_std::prelude::*;
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::assert_eq_size;
use traits::parameter_type_with_key;

// A few exports that help ease life for downstream crates.
pub use common::prelude::{
    Balance, BalanceWrapper, PresetWeightInfo, SwapAmount, SwapOutcome, SwapVariant,
    WeightToFixedFee,
};
pub use common::weights::{BlockLength, BlockWeights, TransactionByteFee};
pub use common::{
    balance, fixed, fixed_from_basis_points, AssetName, AssetSymbol, BalancePrecision, BasisPoints,
    ContentSource, FilterMode, Fixed, FromGenericPair, LiquiditySource, LiquiditySourceFilter,
    LiquiditySourceId, LiquiditySourceType, OnPswapBurned, OnValBurned,
};
pub use ethereum_light_client::{EthereumDifficultyConfig, EthereumHeader};
pub use frame_support::traits::schedule::Named as ScheduleNamed;
pub use frame_support::traits::{
    KeyOwnerProofSystem, LockIdentifier, OnUnbalanced, Randomness, U128CurrencyToVote,
};
pub use frame_support::weights::constants::{
    BlockExecutionWeight, RocksDbWeight, WEIGHT_PER_SECOND,
};
pub use frame_support::weights::{DispatchClass, Weight};
pub use frame_support::{construct_runtime, debug, parameter_types, StorageValue};
pub use pallet_balances::Call as BalancesCall;
pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_transaction_payment::{Multiplier, MultiplierUpdate};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

use eth_bridge::offchain::SignatureParams;
use eth_bridge::requests::{AssetKind, OffchainRequest, OutgoingRequestEncoded, RequestStatus};
use impls::{
    CollectiveWeightInfo, DemocracyWeightInfo, NegativeImbalanceOf, OnUnbalancedDemocracySlash,
};

use frame_support::traits::{
    Contains, Everything, ExistenceRequirement, Get, PrivilegeCmp, WithdrawReasons,
};
use sp_runtime::traits::Keccak256;
pub use {assets, eth_bridge, frame_system, multicollateral_bonding_curve_pool, xst};

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
pub type DigestItem = generic::DigestItem;

/// Identification of DEX.
pub type DEXId = u32;

pub type Moment = u64;

pub type PeriodicSessions = pallet_session::PeriodicSessions<SessionPeriod, SessionOffset>;

type CouncilCollective = pallet_collective::Instance1;
type TechnicalCollective = pallet_collective::Instance2;

type MoreThanHalfCouncil = EnsureOneOf<
    AccountId,
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, CouncilCollective>,
>;
type AtLeastHalfCouncil = EnsureOneOf<
    AccountId,
    pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>,
    EnsureRoot<AccountId>,
>;
type AtLeastTwoThirdsCouncil = EnsureOneOf<
    AccountId,
    pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, CouncilCollective>,
    EnsureRoot<AccountId>,
>;

type SlashCancelOrigin = EnsureOneOf<
    AccountId,
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<_1, _2, AccountId, CouncilCollective>,
>;

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
            pub im_online: ImOnline,
            pub beefy: Beefy,
        }
    }
}

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("sora-substrate"),
    impl_name: create_runtime_str!("sora-substrate"),
    authoring_version: 1,
    spec_version: 21,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 21,
};

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

pub const FARMING_PSWAP_PER_DAY: Balance = balance!(2500000);
pub const FARMING_REFRESH_FREQUENCY: BlockNumber = 2 * HOURS;
// Defined in the article
pub const FARMING_VESTING_COEFF: u32 = 3;
pub const FARMING_VESTING_FREQUENCY: BlockNumber = 6 * HOURS;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const Version: RuntimeVersion = VERSION;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_BLOCKS as u64;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub const UncleGenerations: BlockNumber = 0;
    pub const SessionsPerEra: sp_staking::SessionIndex = 6; // 6 hours
    pub const BondingDuration: pallet_staking::EraIndex = 28; // 28 eras for unbonding (7 days).
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
    pub const SlashDeferDuration: pallet_staking::EraIndex = 27; // 27 eras in which slashes can be cancelled (slightly less than 7 days).
    pub const MaxNominatorRewardedPerValidator: u32 = 256;
    pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
    pub const MaxIterations: u32 = 10;
    // 0.05%. The higher the value, the more strict solution acceptance becomes.
    pub MinSolutionScoreBump: Perbill = Perbill::from_rational(5u32, 10_000);
    pub const ValRewardCurve: pallet_staking::ValRewardCurve = pallet_staking::ValRewardCurve {
        duration_to_reward_flatline: Duration::from_secs(5 * 365 * 24 * 60 * 60),
        min_val_burned_percentage_reward: Percent::from_percent(35),
        max_val_burned_percentage_reward: Percent::from_percent(90),
    };
    pub const SessionPeriod: BlockNumber = 150;
    pub const SessionOffset: BlockNumber = 0;
    pub const SS58Prefix: u8 = 69;
    /// A limit for off-chain phragmen unsigned solution submission.
    ///
    /// We want to keep it as high as possible, but can't risk having it reject,
    /// so we always subtract the base block execution weight.
    pub OffchainSolutionWeightLimit: Weight = BlockWeights::get()
    .get(DispatchClass::Normal)
    .max_extrinsic
    .expect("Normal extrinsics have weight limit configured by default; qed")
    .saturating_sub(BlockExecutionWeight::get());
    pub const DemocracyEnactmentPeriod: BlockNumber = 30 * DAYS;
    pub const DemocracyLaunchPeriod: BlockNumber = 28 * DAYS;
    pub const DemocracyVotingPeriod: BlockNumber = 14 * DAYS;
    pub const DemocracyMinimumDeposit: Balance = balance!(1);
    pub const DemocracyFastTrackVotingPeriod: BlockNumber = 3 * HOURS;
    pub const DemocracyInstantAllowed: bool = true;
    pub const DemocracyCooloffPeriod: BlockNumber = 28 * DAYS;
    pub const DemocracyPreimageByteDeposit: Balance = balance!(0.000002); // 2 * 10^-6, 5 MiB -> 10.48576 XOR
    pub const DemocracyMaxVotes: u32 = 100;
    pub const DemocracyMaxProposals: u32 = 100;
    pub const CouncilCollectiveMotionDuration: BlockNumber = 5 * DAYS;
    pub const CouncilCollectiveMaxProposals: u32 = 100;
    pub const CouncilCollectiveMaxMembers: u32 = 100;
    pub const TechnicalCollectiveMotionDuration: BlockNumber = 5 * DAYS;
    pub const TechnicalCollectiveMaxProposals: u32 = 100;
    pub const TechnicalCollectiveMaxMembers: u32 = 100;
    pub const SchedulerMaxWeight: Weight = 1024;
    pub OffencesWeightSoftLimit: Weight = Perbill::from_percent(60) * BlockWeights::get().max_block;
    pub const ImOnlineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
    pub const SessionDuration: BlockNumber = EPOCH_DURATION_IN_BLOCKS;
    pub const ElectionsCandidacyBond: Balance = balance!(1);
    // 1 storage item created, key size is 32 bytes, value size is 16+16.
    pub const ElectionsVotingBondBase: Balance = balance!(0.000001);
    // additional data per vote is 32 bytes (account id).
    pub const ElectionsVotingBondFactor: Balance = balance!(0.000001);
    pub const ElectionsTermDuration: BlockNumber = 7 * DAYS;
    /// 13 members initially, to be increased to 23 eventually.
    pub const ElectionsDesiredMembers: u32 = 13;
    pub const ElectionsDesiredRunnersUp: u32 = 20;
    pub const ElectionsModuleId: LockIdentifier = *b"phrelect";
    pub FarmingRewardDoublingAssets: Vec<AssetId> = vec![GetPswapAssetId::get(), GetValAssetId::get(), GetDaiAssetId::get(), GetEthAssetId::get()];
    pub const MaxAuthorities: u32 = 100_000;
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = BlockWeights;
    /// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
    type BlockLength = BlockLength;
    /// The ubiquitous origin type.
    type Origin = Origin;
    /// The aggregated dispatch type that is available for extrinsics.
    type Call = Call;
    /// The index type for storing how many extrinsics an account has signed.
    type Index = Index;
    /// The index type for blocks.
    type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = IdentityLookup<AccountId>;
    /// The header type.
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// The ubiquitous event type.
    type Event = Event;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Runtime version.
    type Version = Version;
    type PalletInfo = PalletInfo;
    /// Converts a module to an index of this module in the runtime.
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = ();
    type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        pallet_babe::AuthorityId,
    )>>::Proof;
    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        pallet_babe::AuthorityId,
    )>>::IdentificationTuple;
    type KeyOwnerProofSystem = Historical;
    type HandleEquivocation =
        pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
}

impl pallet_collective::Config<CouncilCollective> for Runtime {
    type Origin = Origin;
    type Proposal = Call;
    type Event = Event;
    type MotionDuration = CouncilCollectiveMotionDuration;
    type MaxProposals = CouncilCollectiveMaxProposals;
    type MaxMembers = CouncilCollectiveMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = CollectiveWeightInfo<Self>;
}

impl pallet_collective::Config<TechnicalCollective> for Runtime {
    type Origin = Origin;
    type Proposal = Call;
    type Event = Event;
    type MotionDuration = TechnicalCollectiveMotionDuration;
    type MaxProposals = TechnicalCollectiveMaxProposals;
    type MaxMembers = TechnicalCollectiveMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = CollectiveWeightInfo<Self>;
}

impl pallet_democracy::Config for Runtime {
    type Proposal = Call;
    type Event = Event;
    type Currency = Balances;
    type EnactmentPeriod = DemocracyEnactmentPeriod;
    type LaunchPeriod = DemocracyLaunchPeriod;
    type VotingPeriod = DemocracyVotingPeriod;
    type MinimumDeposit = DemocracyMinimumDeposit;
    /// `external_propose` call condition
    type ExternalOrigin = AtLeastHalfCouncil;
    /// A super-majority can have the next scheduled referendum be a straight majority-carries vote.
    /// `external_propose_majority` call condition
    type ExternalMajorityOrigin = AtLeastHalfCouncil;
    /// `external_propose_default` call condition
    type ExternalDefaultOrigin = AtLeastHalfCouncil;
    /// Two thirds of the technical committee can have an ExternalMajority/ExternalDefault vote
    /// be tabled immediately and with a shorter voting/enactment period.
    type FastTrackOrigin = EnsureOneOf<
        AccountId,
        pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, TechnicalCollective>,
        EnsureRoot<AccountId>,
    >;
    type InstantOrigin = EnsureOneOf<
        AccountId,
        pallet_collective::EnsureProportionAtLeast<_2, _3, AccountId, TechnicalCollective>,
        EnsureRoot<AccountId>,
    >;
    type InstantAllowed = DemocracyInstantAllowed;
    type FastTrackVotingPeriod = DemocracyFastTrackVotingPeriod;
    /// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
    /// `emergency_cancel` call condition.
    type CancellationOrigin = AtLeastTwoThirdsCouncil;
    type CancelProposalOrigin = AtLeastTwoThirdsCouncil;
    type BlacklistOrigin = EnsureRoot<AccountId>;
    /// `veto_external` - vetoes and blacklists the external proposal hash
    type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCollective>;
    type CooloffPeriod = DemocracyCooloffPeriod;
    type PreimageByteDeposit = DemocracyPreimageByteDeposit;
    type OperationalPreimageOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
    type Slash = OnUnbalancedDemocracySlash<Self>;
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type MaxVotes = DemocracyMaxVotes;
    type WeightInfo = DemocracyWeightInfo;
    type MaxProposals = DemocracyMaxProposals;
    type VoteLockingPeriod = DemocracyEnactmentPeriod;
}

impl pallet_elections_phragmen::Config for Runtime {
    type Event = Event;
    type PalletId = ElectionsModuleId;
    type Currency = Balances;
    type ChangeMembers = Council;
    type InitializeMembers = Council;
    type CurrencyToVote = frame_support::traits::U128CurrencyToVote;
    type CandidacyBond = ElectionsCandidacyBond;
    type VotingBondBase = ElectionsVotingBondBase;
    type VotingBondFactor = ElectionsVotingBondFactor;
    type LoserCandidate = OnUnbalancedDemocracySlash<Self>;
    type KickedMember = OnUnbalancedDemocracySlash<Self>;
    type DesiredMembers = ElectionsDesiredMembers;
    type DesiredRunnersUp = ElectionsDesiredRunnersUp;
    type TermDuration = ElectionsTermDuration;
    type WeightInfo = ();
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
    type Event = Event;
    type AddOrigin = MoreThanHalfCouncil;
    type RemoveOrigin = MoreThanHalfCouncil;
    type SwapOrigin = MoreThanHalfCouncil;
    type ResetOrigin = MoreThanHalfCouncil;
    type PrimeOrigin = MoreThanHalfCouncil;
    type MembershipInitialized = TechnicalCommittee;
    type MembershipChanged = TechnicalCommittee;
    type MaxMembers = ();
    type WeightInfo = ();
}

impl pallet_grandpa::Config for Runtime {
    type Event = Event;
    type Call = Call;

    type KeyOwnerProofSystem = Historical;

    type KeyOwnerProof =
        <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

    type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
        KeyTypeId,
        GrandpaId,
    )>>::IdentificationTuple;

    type HandleEquivocation = pallet_grandpa::EquivocationHandler<
        Self::KeyOwnerIdentification,
        Offences,
        ReportLongevity,
    >;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = Moment;
    type OnTimestampSet = Babe;
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

impl pallet_session::Config for Runtime {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, XorFee>;
    type Keys = opaque::SessionKeys;
    type ShouldEndSession = Babe;
    type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
    type Event = Event;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Self>;
    type NextSessionRotation = Babe;
    type WeightInfo = ();
}

impl pallet_session::historical::Config for Runtime {
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = (Staking, ImOnline);
}

impl pallet_staking::Config for Runtime {
    type Currency = Balances;
    type MultiCurrency = Tokens;
    type ValTokenId = GetValAssetId;
    type ValRewardCurve = ValRewardCurve;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type Event = Event;
    type Slash = ();
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type SlashCancelOrigin = SlashCancelOrigin;
    type SessionInterface = Self;
    type NextNewSession = Session;
    type ElectionLookahead = ElectionLookahead;
    type Call = Call;
    type MaxIterations = MaxIterations;
    type MinSolutionScoreBump = MinSolutionScoreBump;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type UnsignedPriority = UnsignedPriority;
    type OffchainSolutionWeightLimit = OffchainSolutionWeightLimit;
    type WeightInfo = ();
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
    fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
        if left == right {
            return Some(Ordering::Equal);
        }

        match (left, right) {
            // Root is greater than anything.
            (OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
            // Check which one has more yes votes.
            (
                OriginCaller::Council(pallet_collective::RawOrigin::Members(l_yes_votes, l_count)),
                OriginCaller::Council(pallet_collective::RawOrigin::Members(r_yes_votes, r_count)),
            ) => Some((l_yes_votes * r_count).cmp(&(r_yes_votes * l_count))),
            // For every other origin we don't care, as they are not used for `ScheduleOrigin`.
            _ => None,
        }
    }
}

impl pallet_scheduler::Config for Runtime {
    type Event = Event;
    type Origin = Origin;
    type PalletsOrigin = OriginCaller;
    type Call = Call;
    type MaximumWeight = SchedulerMaxWeight;
    type ScheduleOrigin = frame_system::EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ();
    type WeightInfo = ();
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Runtime {
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    /// The ubiquitous event type.
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = ();
    type ReserveIdentifier = ();
}

pub type Amount = i128;

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
    type MaxLocks = ();
    type DustRemovalWhitelist = Everything;
}

parameter_types! {
    // This is common::PredefinedAssetId with 0 index, 2 is size, 0 and 0 is code.
    pub const GetXorAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200000000000000000000000000000000000000000000000000000000000000"));
    pub const GetDotAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200010000000000000000000000000000000000000000000000000000000000"));
    pub const GetKsmAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200020000000000000000000000000000000000000000000000000000000000"));
    pub const GetUsdAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200030000000000000000000000000000000000000000000000000000000000"));
    pub const GetValAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200040000000000000000000000000000000000000000000000000000000000"));
    pub const GetPswapAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200050000000000000000000000000000000000000000000000000000000000"));
    pub const GetDaiAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200060000000000000000000000000000000000000000000000000000000000"));
    pub const GetEthAssetId: AssetId = common::AssetId32::from_bytes(hex!("0200070000000000000000000000000000000000000000000000000000000000"));

    pub const GetBaseAssetId: AssetId = GetXorAssetId::get();
    pub const GetTeamReservesAccountId: AccountId = AccountId::new(hex!("feb92c0acb61f75309730290db5cbe8ac9b46db7ad6f3bbb26a550a73586ea71"));
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
}

pub struct GetTotalBalance;

impl assets::GetTotalBalance<Runtime> for GetTotalBalance {
    fn total_balance(asset_id: &AssetId, who: &AccountId) -> Result<Balance, DispatchError> {
        if asset_id == &GetXorAssetId::get() {
            Ok(Referrals::referrer_balance(who).unwrap_or(0))
        } else {
            Ok(0)
        }
    }
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Pallet<Runtime>;
    type GetTeamReservesAccountId = GetTeamReservesAccountId;
    type GetTotalBalance = GetTotalBalance;
    type WeightInfo = assets::weights::WeightInfo<Runtime>;
}

impl trading_pair::Config for Runtime {
    type Event = Event;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
}

impl dex_manager::Config for Runtime {}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
pub type AssetId = common::AssetId32<common::PredefinedAssetId>;

impl technical::Config for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
}

parameter_types! {
    pub GetFee: Fixed = fixed!(0.003);
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type Event = Event;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type GetFee = GetFee;
    type OnPoolCreated = (PswapDistribution, Farming);
    type OnPoolReservesChanged = PriceTools;
    type WeightInfo = pool_xyk::weights::WeightInfo<Runtime>;
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
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub const GetNumSamples: usize = 5;
    pub const BasicDeposit: Balance = balance!(0.01);
    pub const FieldDeposit: Balance = balance!(0.01);
    pub const SubAccountDeposit: Balance = balance!(0.01);
    pub const MaxSubAccounts: u32 = 100;
    pub const MaxAdditionalFields: u32 = 100;
    pub const MaxRegistrars: u32 = 20;
    pub ReferralsReservesAcc: AccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            b"referrals".to_vec(),
            b"main".to_vec(),
        );
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}

impl liquidity_proxy::Config for Runtime {
    type Event = Event;
    type LiquidityRegistry = dex_api::Pallet<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
    type PrimaryMarketTBC = multicollateral_bonding_curve_pool::Pallet<Runtime>;
    type PrimaryMarketXST = xst::Pallet<Runtime>;
    type SecondaryMarket = pool_xyk::Pallet<Runtime>;
    type WeightInfo = liquidity_proxy::weights::WeightInfo<Runtime>;
    type VestedRewardsPallet = VestedRewards;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance2> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance3> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance4> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
}

impl dex_api::Config for Runtime {
    type Event = Event;
    type MockLiquiditySource =
        mock_liquidity_source::Pallet<Runtime, mock_liquidity_source::Instance1>;
    type MockLiquiditySource2 =
        mock_liquidity_source::Pallet<Runtime, mock_liquidity_source::Instance2>;
    type MockLiquiditySource3 =
        mock_liquidity_source::Pallet<Runtime, mock_liquidity_source::Instance3>;
    type MockLiquiditySource4 =
        mock_liquidity_source::Pallet<Runtime, mock_liquidity_source::Instance4>;
    type MulticollateralBondingCurvePool = multicollateral_bonding_curve_pool::Pallet<Runtime>;
    type XYKPool = pool_xyk::Pallet<Runtime>;
    type XSTPool = xst::Pallet<Runtime>;
    type WeightInfo = dex_api::weights::WeightInfo<Runtime>;
}

impl pallet_multisig::Config for Runtime {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl iroha_migration::Config for Runtime {
    type Event = Event;
    type WeightInfo = iroha_migration::weights::WeightInfo<Runtime>;
}

impl pallet_identity::Config for Runtime {
    type Event = Event;
    type Currency = Balances;
    type BasicDeposit = BasicDeposit;
    type FieldDeposit = FieldDeposit;
    type SubAccountDeposit = SubAccountDeposit;
    type MaxSubAccounts = MaxSubAccounts;
    type MaxAdditionalFields = MaxAdditionalFields;
    type MaxRegistrars = MaxRegistrars;
    type Slashed = ();
    type ForceOrigin = MoreThanHalfCouncil;
    type RegistrarOrigin = MoreThanHalfCouncil;
    type WeightInfo = ();
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
        let extra: SignedExtra = (
            frame_system::CheckSpecVersion::<Runtime>::new(),
            frame_system::CheckTxVersion::<Runtime>::new(),
            frame_system::CheckGenesis::<Runtime>::new(),
            frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
            frame_system::CheckNonce::<Runtime>::from(index),
            frame_system::CheckWeight::<Runtime>::new(),
            ChargeTransactionPayment::<Runtime>::new(),
        );
        #[cfg_attr(not(feature = "std"), allow(unused_variables))]
        let raw_payload = SignedPayload::new(call, extra)
            .map_err(|e| {
                frame_support::log::warn!("SignedPayload error: {:?}", e);
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

impl referrals::Config for Runtime {
    type ReservesAcc = ReferralsReservesAcc;
    type WeightInfo = referrals::weights::WeightInfo<Runtime>;
}

impl rewards::Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumber = 1 * DAYS;
    const UPDATE_FREQUENCY: BlockNumber = 10 * MINUTES;
    const MAX_CHUNK_SIZE: usize = 100;
    const MAX_VESTING_RATIO: Percent = Percent::from_percent(55);
    const TIME_TO_SATURATION: BlockNumber = 5 * 365 * DAYS; // 5 years
    type Event = Event;
    type WeightInfo = rewards::weights::WeightInfo<Runtime>;
}

pub struct ExtrinsicsFlatFees;

// Flat fees implementation for the selected extrinsics.
// Returns a value if the extirnsic is subject to manual fee adjustment
// and `None` otherwise
impl xor_fee::ApplyCustomFees<Call> for ExtrinsicsFlatFees {
    fn compute_fee(call: &Call) -> Option<Balance> {
        match call {
            Call::Assets(assets::Call::register { .. })
            | Call::EthBridge(eth_bridge::Call::transfer_to_sidechain { .. })
            | Call::PoolXYK(pool_xyk::Call::withdraw_liquidity { .. })
            | Call::Rewards(rewards::Call::claim { .. }) => Some(balance!(0.007)),
            Call::VestedRewards(vested_rewards::Call::claim_rewards { .. }) => Some(BIG_FEE),
            Call::Assets(..)
            | Call::EthBridge(..)
            | Call::LiquidityProxy(..)
            | Call::MulticollateralBondingCurvePool(..)
            | Call::PoolXYK(..)
            | Call::Rewards(..)
            | Call::Staking(pallet_staking::Call::payout_stakers { .. })
            | Call::TradingPair(..)
            | Call::Referrals(..) => Some(SMALL_FEE),
            _ => None,
        }
    }
}

impl xor_fee::ExtractProxySwap for Call {
    type DexId = DEXId;
    type AssetId = AssetId;
    type Amount = SwapAmount<u128>;
    fn extract(&self) -> Option<xor_fee::SwapInfo<Self::DexId, Self::AssetId, Self::Amount>> {
        if let Call::LiquidityProxy(liquidity_proxy::Call::swap {
            dex_id,
            input_asset_id,
            output_asset_id,
            swap_amount,
            selected_source_types,
            filter_mode,
        }) = self
        {
            Some(xor_fee::SwapInfo {
                dex_id: *dex_id,
                input_asset_id: *input_asset_id,
                output_asset_id: *output_asset_id,
                amount: *swap_amount,
                selected_source_types: selected_source_types.to_vec(),
                filter_mode: filter_mode.clone(),
            })
        } else {
            None
        }
    }
}

impl xor_fee::IsCalledByBridgePeer<AccountId> for Call {
    fn is_called_by_bridge_peer(&self, who: &AccountId) -> bool {
        match self {
            Call::BridgeMultisig(call) => match call {
                bridge_multisig::Call::as_multi {
                    id: multisig_id, ..
                }
                | bridge_multisig::Call::as_multi_threshold_1 {
                    id: multisig_id, ..
                } => bridge_multisig::Accounts::<Runtime>::get(multisig_id)
                    .map(|acc| acc.is_signatory(&who)),
                _ => None,
            },
            Call::EthBridge(call) => match call {
                eth_bridge::Call::approve_request { network_id, .. } => {
                    Some(eth_bridge::Pallet::<Runtime>::is_peer(who, *network_id))
                }
                eth_bridge::Call::register_incoming_request { incoming_request } => {
                    let net_id = incoming_request.network_id();
                    eth_bridge::BridgeAccount::<Runtime>::get(net_id).map(|acc| acc == *who)
                }
                eth_bridge::Call::import_incoming_request {
                    load_incoming_request,
                    ..
                } => {
                    let net_id = load_incoming_request.network_id();
                    eth_bridge::BridgeAccount::<Runtime>::get(net_id).map(|acc| acc == *who)
                }
                eth_bridge::Call::finalize_incoming_request { network_id, .. }
                | eth_bridge::Call::abort_request { network_id, .. } => {
                    eth_bridge::BridgeAccount::<Runtime>::get(network_id).map(|acc| acc == *who)
                }
                _ => None,
            },
            _ => None,
        }
        .unwrap_or(false)
    }
}

pub struct ValBurnedAggregator<T>(sp_std::marker::PhantomData<T>);

impl<T> OnValBurned for ValBurnedAggregator<T>
where
    T: pallet_staking::ValBurnedNotifier<Balance>,
{
    fn on_val_burned(amount: Balance) {
        Rewards::on_val_burned(amount);
        T::notify_val_burned(amount);
    }
}

pub struct WithdrawFee;

impl xor_fee::WithdrawFee<Runtime> for WithdrawFee {
    fn withdraw_fee(
        who: &AccountId,
        call: &Call,
        fee: Balance,
    ) -> Result<(AccountId, Option<NegativeImbalanceOf<Runtime>>), DispatchError> {
        match call {
            Call::Referrals(referrals::Call::set_referrer { referrer })
                if Referrals::can_set_referrer(who) =>
            {
                Referrals::withdraw_fee(referrer, fee)?;
                Ok((
                    referrer.clone(),
                    Some(Balances::withdraw(
                        &ReferralsReservesAcc::get(),
                        fee,
                        WithdrawReasons::TRANSACTION_PAYMENT,
                        ExistenceRequirement::KeepAlive,
                    )?),
                ))
            }
            _ => Ok((
                who.clone(),
                Some(Balances::withdraw(
                    who,
                    fee,
                    WithdrawReasons::TRANSACTION_PAYMENT,
                    ExistenceRequirement::KeepAlive,
                )?),
            )),
        }
    }
}

parameter_types! {
    pub const DEXIdValue: DEXId = 0;
}

impl xor_fee::Config for Runtime {
    type Event = Event;
    // Pass native currency.
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type SoraParliamentShare = SoraParliamentShare;
    type XorId = GetXorAssetId;
    type ValId = GetValAssetId;
    type DEXIdValue = DEXIdValue;
    type LiquidityProxy = LiquidityProxy;
    type OnValBurned = ValBurnedAggregator<Staking>;
    type CustomFees = ExtrinsicsFlatFees;
    type GetTechnicalAccountId = GetXorFeeAccountId;
    type GetParliamentAccountId = GetParliamentAccountId;
    type SessionManager = Staking;
    type WithdrawFee = WithdrawFee;
}

pub struct ConstantFeeMultiplier;

impl MultiplierUpdate for ConstantFeeMultiplier {
    fn min() -> Multiplier {
        Default::default()
    }
    fn target() -> Perquintill {
        Default::default()
    }
    fn variability() -> Multiplier {
        Default::default()
    }
}
impl Convert<Multiplier, Multiplier> for ConstantFeeMultiplier {
    fn convert(previous: Multiplier) -> Multiplier {
        previous
    }
}

parameter_types! {
    pub const OperationalFeeMultiplier: u8 = 5;
}

impl pallet_transaction_payment::Config for Runtime {
    type OnChargeTransaction = XorFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = WeightToFixedFee;
    type FeeMultiplierUpdate = ConstantFeeMultiplier;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

#[cfg(feature = "private-net")]
impl pallet_sudo::Config for Runtime {
    type Call = Call;
    type Event = Event;
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl pallet_utility::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type WeightInfo = ();
    type PalletsOrigin = OriginCaller;
}

parameter_types! {
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 100;
}

impl bridge_multisig::Config for Runtime {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

parameter_types! {
    pub const EthNetworkId: u32 = 0;
}

pub struct RemoveTemporaryPeerAccountIds;

#[cfg(feature = "private-net")]
impl Get<Vec<(AccountId, H160)>> for RemoveTemporaryPeerAccountIds {
    fn get() -> Vec<(AccountId, H160)> {
        vec![
            // Dev
            (
                AccountId::new(hex!(
                    "aa79aa80b94b1cfba69c4a7d60eeb7b469e6411d1f686cc61de8adc8b1b76a69"
                )),
                H160(hex!("f858c8366f3a2553516a47f3e0503a85ef93bbba")),
            ),
            (
                AccountId::new(hex!(
                    "60dc5adadc262770cbe904e3f65a26a89d46b70447640cd7968b49ddf5a459bc"
                )),
                H160(hex!("ccd7fe44d58640dc79c55b98f8c3474646e5ea2b")),
            ),
            (
                AccountId::new(hex!(
                    "70d61e980602e09ac8b5fb50658ebd345774e73b8248d3b61862ba1a9a035082"
                )),
                H160(hex!("13d26a91f791e884fe6faa7391c4ef401638baa4")),
            ),
            (
                AccountId::new(hex!(
                    "05918034f4a7f7c5d99cd0382aa6574ec2aba148aa3d769e50e0ac7663e36d58"
                )),
                H160(hex!("aa19829ae887212206be8e97ea47d8fed2120d4e")),
            ),
            // Test
            (
                AccountId::new(hex!(
                    "07f5670d08b8f3bd493ff829482a489d94494fd50dd506957e44e9fdc2e98684"
                )),
                H160(hex!("457d710255184dbf63c019ab50f65743c6cb072f")),
            ),
            (
                AccountId::new(hex!(
                    "211bb96e9f746183c05a1d583bccf513f9d8f679d6f36ecbd06609615a55b1cc"
                )),
                H160(hex!("6d04423c97e8ce36d04c9b614926ce0d029d04df")),
            ),
            (
                AccountId::new(hex!(
                    "ef3139b81d14977d5bf6b4a3994872337dfc1d2af2069a058bc26123a3ed1a5c"
                )),
                H160(hex!("e34022904b1ab539729cc7b5bfa5c8a74b165e80")),
            ),
            (
                AccountId::new(hex!(
                    "71124b336fbf3777d743d4390acce6be1cf5e0781e40c51d4cf2e5b5fd8e41e1"
                )),
                H160(hex!("ee74a5b5346915012d103cf1ccee288f25bcbc81")),
            ),
            // Stage
            (
                AccountId::new(hex!(
                    "07f5670d08b8f3bd493ff829482a489d94494fd50dd506957e44e9fdc2e98684"
                )),
                H160(hex!("457d710255184dbf63c019ab50f65743c6cb072f")),
            ),
            (
                AccountId::new(hex!(
                    "211bb96e9f746183c05a1d583bccf513f9d8f679d6f36ecbd06609615a55b1cc"
                )),
                H160(hex!("6d04423c97e8ce36d04c9b614926ce0d029d04df")),
            ),
        ]
    }
}

#[cfg(not(feature = "private-net"))]
impl Get<Vec<(AccountId, H160)>> for RemoveTemporaryPeerAccountIds {
    fn get() -> Vec<(AccountId, H160)> {
        vec![] // the peer is already removed on main-net.
    }
}

#[cfg(not(feature = "private-net"))]
parameter_types! {
    pub const RemovePendingOutgoingRequestsAfter: BlockNumber = 1 * DAYS;
    pub const TrackPendingIncomingRequestsAfter: (BlockNumber, u64) = (1 * DAYS, 12697214);
}

#[cfg(feature = "private-net")]
parameter_types! {
    pub const RemovePendingOutgoingRequestsAfter: BlockNumber = 30 * MINUTES;
    pub const TrackPendingIncomingRequestsAfter: (BlockNumber, u64) = (30 * MINUTES, 0);
}

pub type NetworkId = u32;

impl eth_bridge::Config for Runtime {
    type Event = Event;
    type Call = Call;
    type PeerId = eth_bridge::offchain::crypto::TestAuthId;
    type NetworkId = NetworkId;
    type GetEthNetworkId = EthNetworkId;
    type WeightInfo = eth_bridge::weights::WeightInfo<Runtime>;
    type RemovePendingOutgoingRequestsAfter = RemovePendingOutgoingRequestsAfter;
    type TrackPendingIncomingRequestsAfter = TrackPendingIncomingRequestsAfter;
    type RemovePeerAccountIds = RemoveTemporaryPeerAccountIds;
    type SchedulerOriginCaller = OriginCaller;
    type Scheduler = Scheduler;
}

#[cfg(feature = "private-net")]
impl faucet::Config for Runtime {
    type Event = Event;
    type WeightInfo = faucet::weights::WeightInfo<Runtime>;
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
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetParliamentAccountId: AccountId = hex!("881b87c9f83664b95bd13e2bb40675bfa186287da93becc0b22683334d411e4e").into();
    pub GetXorFeeTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            xor_fee::TECH_ACCOUNT_PREFIX.to_vec(),
            xor_fee::TECH_ACCOUNT_MAIN.to_vec(),
        )
    };
    pub GetXorFeeAccountId: AccountId = {
        let tech_account_id = GetXorFeeTechAccountId::get();
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
            .expect("Failed to get ordinary account id for technical account id.")
    };
    pub GetXSTPoolPermissionedTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            xst::TECH_ACCOUNT_PREFIX.to_vec(),
            xst::TECH_ACCOUNT_PERMISSIONED.to_vec(),
        );
        tech_account_id
    };
    pub GetXSTPoolPermissionedAccountId: AccountId = {
        let tech_account_id = GetXSTPoolPermissionedTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}

#[cfg(feature = "reduced-pswap-reward-periods")]
parameter_types! {
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 150;
    pub const GetBurnUpdateFrequency: BlockNumber = 150;
}

#[cfg(not(feature = "reduced-pswap-reward-periods"))]
parameter_types! {
    pub const GetDefaultSubscriptionFrequency: BlockNumber = 14400;
    pub const GetBurnUpdateFrequency: BlockNumber = 14400;
}

pub struct RuntimeOnPswapBurnedAggregator;

impl OnPswapBurned for RuntimeOnPswapBurnedAggregator {
    fn on_pswap_burned(distribution: common::PswapRemintInfo) {
        VestedRewards::on_pswap_burned(distribution);
    }
}

impl farming::Config for Runtime {
    const PSWAP_PER_DAY: Balance = FARMING_PSWAP_PER_DAY;
    const REFRESH_FREQUENCY: BlockNumber = FARMING_REFRESH_FREQUENCY;
    const VESTING_COEFF: u32 = FARMING_VESTING_COEFF;
    const VESTING_FREQUENCY: BlockNumber = FARMING_VESTING_FREQUENCY;
    const BLOCKS_PER_DAY: BlockNumber = 1 * DAYS;
    type Call = Call;
    type SchedulerOriginCaller = OriginCaller;
    type Scheduler = Scheduler;
    type RewardDoublingAssets = FarmingRewardDoublingAssets;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type Event = Event;
    type GetIncentiveAssetId = GetPswapAssetId;
    type LiquidityProxy = LiquidityProxy;
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = DEXManager;
    type OnPswapBurnedAggregator = RuntimeOnPswapBurnedAggregator;
    type WeightInfo = pswap_distribution::weights::WeightInfo<Runtime>;
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXYK;
}

parameter_types! {
    pub GetMbcReservesTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_RESERVES.to_vec(),
        );
        tech_account_id
    };
    pub GetMbcReservesAccountId: AccountId = {
        let tech_account_id = GetMbcReservesTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetMbcPoolRewardsTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_REWARDS.to_vec(),
        );
        tech_account_id
    };
    pub GetMbcPoolRewardsAccountId: AccountId = {
        let tech_account_id = GetMbcPoolRewardsTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetMbcPoolFreeReservesTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_PREFIX.to_vec(),
            multicollateral_bonding_curve_pool::TECH_ACCOUNT_FREE_RESERVES.to_vec(),
        );
        tech_account_id
    };
    pub GetMbcPoolFreeReservesAccountId: AccountId = {
        let tech_account_id = GetMbcPoolFreeReservesTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetMarketMakerRewardsTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            vested_rewards::TECH_ACCOUNT_PREFIX.to_vec(),
            vested_rewards::TECH_ACCOUNT_MARKET_MAKERS.to_vec(),
        );
        tech_account_id
    };
    pub GetMarketMakerRewardsAccountId: AccountId = {
        let tech_account_id = GetMarketMakerRewardsTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetFarmingRewardsTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            vested_rewards::TECH_ACCOUNT_PREFIX.to_vec(),
            vested_rewards::TECH_ACCOUNT_FARMING.to_vec(),
        );
        tech_account_id
    };
    pub GetFarmingRewardsAccountId: AccountId = {
        let tech_account_id = GetFarmingRewardsTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = LiquidityProxy;
    type EnsureDEXManager = DEXManager;
    type EnsureTradingPairExists = TradingPair;
    type PriceToolsPallet = PriceTools;
    type VestedRewardsPallet = VestedRewards;
    type WeightInfo = multicollateral_bonding_curve_pool::weights::WeightInfo<Runtime>;
}

impl xst::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = LiquidityProxy;
    type EnsureDEXManager = DEXManager;
    type EnsureTradingPairExists = TradingPair;
    type PriceToolsPallet = PriceTools;
    type WeightInfo = xst::weights::WeightInfo<Runtime>;
}

parameter_types! {
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type Event = Event;
    type ValidatorSet = Historical;
    type NextSessionRotation = (); //SessionDuration;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = ();
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
    type MaxPeerDataEncodingSize = MaxPeerDataEncodingSize;
}

impl pallet_offences::Config for Runtime {
    type Event = Event;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = Staking;
}

impl vested_rewards::Config for Runtime {
    type Event = Event;
    type GetBondingCurveRewardsAccountId = GetMbcPoolRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type WeightInfo = vested_rewards::weights::WeightInfo<Runtime>;
}

impl price_tools::Config for Runtime {
    type Event = Event;
    type LiquidityProxy = LiquidityProxy;
    type WeightInfo = price_tools::weights::WeightInfo<Runtime>;
}

impl pallet_randomness_collective_flip::Config for Runtime {}

impl pallet_beefy::Config for Runtime {
    type BeefyId = BeefyId;
}

impl pallet_mmr::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = b"mmr";
    type Hashing = Keccak256;
    type Hash = <Keccak256 as sp_runtime::traits::Hash>::Output;
    type OnNewRoot = pallet_beefy_mmr::DepositBeefyDigest<Runtime>;
    type WeightInfo = ();
    type LeafData = pallet_beefy_mmr::Pallet<Runtime>;
}

pub struct ParasProvider;
impl pallet_beefy_mmr::ParachainHeadsProvider for ParasProvider {
    fn parachain_heads() -> Vec<(u32, Vec<u8>)> {
        // FIXME:
        // Paras::parachains()
        //     .into_iter()
        //     .filter_map(|id| Paras::para_head(&id).map(|head| (id.into(), head.0)))
        //     .collect()
        Vec::new()
    }
}

parameter_types! {
    /// Version of the produced MMR leaf.
    ///
    /// The version consists of two parts;
    /// - `major` (3 bits)
    /// - `minor` (5 bits)
    ///
    /// `major` should be updated only if decoding the previous MMR Leaf format from the payload
    /// is not possible (i.e. backward incompatible change).
    /// `minor` should be updated if fields are added to the previous MMR Leaf, which given SCALE
    /// encoding does not prevent old leafs from being decoded.
    ///
    /// Hence we expect `major` to be changed really rarely (think never).
    /// See [`MmrLeafVersion`] type documentation for more details.
    pub LeafVersion: MmrLeafVersion = MmrLeafVersion::new(0, 0);
}

impl pallet_beefy_mmr::Config for Runtime {
    type LeafVersion = LeafVersion;
    type BeefyAuthorityToMerkleLeaf = pallet_beefy_mmr::BeefyEcdsaToEthereum;
    type ParachainHeads = ParasProvider;
}

parameter_types! {
    pub const CeresPerDay: Balance = balance!(6.66666666667);
    pub const CeresAssetId: AssetId = common::AssetId32::from_bytes
        (hex!("008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"));
    pub const MaximumCeresInStakingPool: Balance = balance!(7200);
}

impl ceres_staking::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumber = 1 * DAYS;
    type Event = Event;
    type CeresPerDay = CeresPerDay;
    type CeresAssetId = CeresAssetId;
    type MaximumCeresInStakingPool = MaximumCeresInStakingPool;
    type WeightInfo = ceres_staking::weights::WeightInfo<Runtime>;
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumber = 1 * DAYS;
    type Event = Event;
    type XYKPool = PoolXYK;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ceres_liquidity_locker::weights::WeightInfo<Runtime>;
}

/// Payload data to be signed when making signed transaction from off-chain workers,
///   inside `create_transaction` function.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;

parameter_types! {
    pub const UnsignedPriority: u64 = 100;
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWeight: u32 = 50;
    pub const SoraParliamentShare: Percent = Percent::from_percent(10);
}

// Ethereum bridge pallets

pub struct CallFilter;
impl Contains<Call> for CallFilter {
    fn contains(_: &Call) -> bool {
        true
    }
}

impl dispatch::Config for Runtime {
    type Origin = Origin;
    type Event = Event;
    type MessageId = snowbridge_core::MessageId;
    type Call = Call;
    type CallFilter = CallFilter;
}

use basic_channel::{inbound as basic_channel_inbound, outbound as basic_channel_outbound};
use incentivized_channel::{
    inbound as incentivized_channel_inbound, outbound as incentivized_channel_outbound,
};
const INDEXING_PREFIX: &'static [u8] = b"commitment";

pub struct OutboundRouter<T>(PhantomData<T>);

impl<T> snowbridge_core::OutboundRouter<T::AccountId> for OutboundRouter<T>
where
    T: basic_channel::outbound::Config + incentivized_channel::outbound::Config,
{
    fn submit(
        channel_id: ChannelId,
        who: &T::AccountId,
        target: H160,
        payload: &[u8],
    ) -> DispatchResult {
        match channel_id {
            ChannelId::Basic => basic_channel::outbound::Pallet::<T>::submit(who, target, payload),
            ChannelId::Incentivized => {
                incentivized_channel::outbound::Pallet::<T>::submit(who, target, payload)
            }
        }
    }
}

parameter_types! {
    pub const MaxMessagePayloadSize: u64 = 256;
    pub const MaxMessagesPerCommit: u64 = 20;
    pub const Decimals: u32 = 12;
}

impl basic_channel_inbound::Config for Runtime {
    type Event = Event;
    type Verifier = ethereum_light_client::Pallet<Runtime>;
    type MessageDispatch = dispatch::Pallet<Runtime>;
    type WeightInfo = ();
}

impl basic_channel_outbound::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = INDEXING_PREFIX;
    type Event = Event;
    type Hashing = Keccak256;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type SetPrincipalOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
}

pub struct FeeConverter;
impl Convert<U256, Balance> for FeeConverter {
    fn convert(amount: U256) -> Balance {
        common::eth::unwrap_balance(amount, Decimals::get())
            .expect("Should not panic unless runtime is misconfigured")
    }
}

parameter_types! {
    pub const Ether: AssetId32<PredefinedAssetId> = ETH;
}

impl incentivized_channel_inbound::Config for Runtime {
    type Event = Event;
    type Verifier = ethereum_light_client::Pallet<Runtime>;
    type MessageDispatch = dispatch::Pallet<Runtime>;
    type FeeConverter = FeeConverter;
    type UpdateOrigin = MoreThanHalfCouncil;
    type WeightInfo = ();
    type FeeAssetId = Ether;
}

impl incentivized_channel_outbound::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = INDEXING_PREFIX;
    type Event = Event;
    type Hashing = Keccak256;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type FeeCurrency = Ether;
    type SetFeeOrigin = MoreThanHalfCouncil;
    type WeightInfo = ();
}

parameter_types! {
    pub const DescendantsUntilFinalized: u8 = 3;
    pub const DifficultyConfig: EthereumDifficultyConfig = EthereumDifficultyConfig::mainnet();
    pub const VerifyPoW: bool = true;
}

impl ethereum_light_client::Config for Runtime {
    type Event = Event;
    type DescendantsUntilFinalized = DescendantsUntilFinalized;
    type DifficultyConfig = DifficultyConfig;
    type VerifyPoW = VerifyPoW;
    type WeightInfo = ();
}

impl eth_app::Config for Runtime {
    type Event = Event;
    type OutboundRouter = OutboundRouter<Runtime>;
    type CallOrigin = EnsureEthereumAccount;
    type WeightInfo = ();
    type FeeCurrency = Ether;
}

#[cfg(feature = "private-net")]
construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Pallet, Call, Storage, Config, Event<T>} = 0,
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 1,
        // Balances in native currency - XOR.
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 2,
        Sudo: pallet_sudo::{Pallet, Call, Storage, Config<T>, Event<T>} = 3,
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage} = 4,
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage} = 5,
        Permissions: permissions::{Pallet, Call, Storage, Config<T>, Event<T>} = 6,
        Referrals: referrals::{Pallet, Call, Storage} = 7,
        Rewards: rewards::{Pallet, Call, Config<T>, Storage, Event<T>} = 8,
        XorFee: xor_fee::{Pallet, Call, Storage, Event<T>} = 9,
        BridgeMultisig: bridge_multisig::{Pallet, Call, Storage, Config<T>, Event<T>} = 10,
        Utility: pallet_utility::{Pallet, Call, Event} = 11,

        // Consensus and staking.
        Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 12,
        Historical: pallet_session_historical::{Pallet} = 13,
        Babe: pallet_babe::{Pallet, Call, Storage, Config, ValidateUnsigned} = 14,
        Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event} = 15,
        Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent} = 16,
        Staking: pallet_staking::{Pallet, Call, Config<T>, Storage, Event<T>} = 17,

        // Non-native tokens - everything apart of XOR.
        Tokens: tokens::{Pallet, Storage, Config<T>, Event<T>} = 18,
        // Unified interface for XOR and non-native tokens.
        Currencies: currencies::{Pallet, Call, Event<T>} = 19,
        TradingPair: trading_pair::{Pallet, Call, Storage, Config<T>, Event<T>} = 20,
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>} = 21,
        DEXManager: dex_manager::{Pallet, Storage, Config<T>} = 22,
        MulticollateralBondingCurvePool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Config<T>, Event<T>} = 23,
        Technical: technical::{Pallet, Call, Config<T>, Event<T>} = 24,
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>} = 25,
        LiquidityProxy: liquidity_proxy::{Pallet, Call, Event<T>} = 26,
        Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 27,
        TechnicalCommittee: pallet_collective::<Instance2>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 28,
        Democracy: pallet_democracy::{Pallet, Call, Storage, Config<T>, Event<T>} = 29,
        DEXAPI: dex_api::{Pallet, Call, Storage, Config, Event<T>} = 30,
        EthBridge: eth_bridge::{Pallet, Call, Storage, Config<T>, Event<T>} = 31,
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Config<T>, Event<T>} = 32,
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 33,
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 34,
        IrohaMigration: iroha_migration::{Pallet, Call, Storage, Config<T>, Event<T>} = 35,
        ImOnline: pallet_im_online::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 36,
        Offences: pallet_offences::{Pallet, Storage, Event} = 37,
        TechnicalMembership: pallet_membership::<Instance1>::{Pallet, Call, Storage, Event<T>, Config<T>} = 38,
        ElectionsPhragmen: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>} = 39,
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>} = 40,
        Identity: pallet_identity::{Pallet, Call, Storage, Event<T>} = 41,
        Farming: farming::{Pallet, Storage} = 42,
        XSTPool: xst::{Pallet, Call, Storage, Config<T>, Event<T>} = 43,
        PriceTools: price_tools::{Pallet, Storage, Event<T>} = 44,
        CeresStaking: ceres_staking::{Pallet, Call, Storage, Event<T>} = 45,
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>} = 46,

        // Available only for test net
        Faucet: faucet::{Pallet, Call, Config<T>, Event<T>} = 80,

        // Trustless ethereum bridge
        Mmr: pallet_mmr::{Pallet, Storage} = 90,
        Beefy: pallet_beefy::{Pallet, Config<T>, Storage} = 91,
        MmrLeaf: pallet_beefy_mmr::{Pallet, Storage} = 92,
        EthereumLightClient: ethereum_light_client::{Pallet, Call, Storage, Event<T>, Config} = 93,
        BasicInboundChannel: basic_channel_inbound::{Pallet, Call, Storage, Event<T>, Config} = 94,
        BasicOutboundChannel: basic_channel_outbound::{Pallet, Storage, Event<T>, Config<T>} = 95,
        IncentivizedInboundChannel: incentivized_channel_inbound::{Pallet, Call, Config<T>, Storage, Event<T>} = 96,
        IncentivizedOutboundChannel: incentivized_channel_outbound::{Pallet, Config<T>, Storage, Event<T>} = 97,
        Dispatch: dispatch::{Pallet, Storage, Event<T>, Origin} = 98,
        EthApp: eth_app::{Pallet, Call, Storage, Event<T>, Config<T>} = 99,
    }
}

#[cfg(not(feature = "private-net"))]
construct_runtime! {
    pub enum Runtime where
        Block = Block,
        NodeBlock = opaque::Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Pallet, Call, Storage, Config, Event<T>} = 0,
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 1,
        // Balances in native currency - XOR.
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>} = 2,
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage} = 4,
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage} = 5,
        Permissions: permissions::{Pallet, Call, Storage, Config<T>, Event<T>} = 6,
        Referrals: referrals::{Pallet, Call, Storage} = 7,
        Rewards: rewards::{Pallet, Call, Config<T>, Storage, Event<T>} = 8,
        XorFee: xor_fee::{Pallet, Call, Storage, Event<T>} = 9,
        BridgeMultisig: bridge_multisig::{Pallet, Call, Storage, Config<T>, Event<T>} = 10,
        Utility: pallet_utility::{Pallet, Call, Event} = 11,

        // Consensus and staking.
        Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 12,
        Historical: pallet_session_historical::{Pallet} = 13,
        Babe: pallet_babe::{Pallet, Call, Storage, Config, ValidateUnsigned} = 14,
        Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event} = 15,
        Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent} = 16,
        Staking: pallet_staking::{Pallet, Call, Config<T>, Storage, Event<T>} = 17,

        // Non-native tokens - everything apart of XOR.
        Tokens: tokens::{Pallet, Storage, Config<T>, Event<T>} = 18,
        // Unified interface for XOR and non-native tokens.
        Currencies: currencies::{Pallet, Call, Event<T>} = 19,
        TradingPair: trading_pair::{Pallet, Call, Storage, Config<T>, Event<T>} = 20,
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>} = 21,
        DEXManager: dex_manager::{Pallet, Storage, Config<T>} = 22,
        MulticollateralBondingCurvePool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Config<T>, Event<T>} = 23,
        Technical: technical::{Pallet, Call, Config<T>, Event<T>} = 24,
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>} = 25,
        LiquidityProxy: liquidity_proxy::{Pallet, Call, Event<T>} = 26,
        Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 27,
        TechnicalCommittee: pallet_collective::<Instance2>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 28,
        Democracy: pallet_democracy::{Pallet, Call, Storage, Config<T>, Event<T>} = 29,
        DEXAPI: dex_api::{Pallet, Call, Storage, Config, Event<T>} = 30,
        EthBridge: eth_bridge::{Pallet, Call, Storage, Config<T>, Event<T>} = 31,
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Config<T>, Event<T>} = 32,
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 33,
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 34,
        IrohaMigration: iroha_migration::{Pallet, Call, Storage, Config<T>, Event<T>} = 35,
        ImOnline: pallet_im_online::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 36,
        Offences: pallet_offences::{Pallet, Storage, Event} = 37,
        TechnicalMembership: pallet_membership::<Instance1>::{Pallet, Call, Storage, Event<T>, Config<T>} = 38,
        ElectionsPhragmen: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>} = 39,
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>} = 40,
        Identity: pallet_identity::{Pallet, Call, Storage, Event<T>} = 41,
        Farming: farming::{Pallet, Storage} = 42,
        XSTPool: xst::{Pallet, Call, Storage, Config<T>, Event<T>} = 43,
        PriceTools: price_tools::{Pallet, Storage, Event<T>} = 44,
        CeresStaking: ceres_staking::{Pallet, Call, Storage, Event<T>} = 45,
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>} = 46,


        // Trustless ethereum bridge
        Mmr: pallet_mmr::{Pallet, Storage} = 90,
        Beefy: pallet_beefy::{Pallet, Config<T>, Storage} = 91,
        MmrLeaf: pallet_beefy_mmr::{Pallet, Storage} = 92,
        EthereumLightClient: ethereum_light_client::{Pallet, Call, Storage, Event<T>, Config} = 93,
        BasicInboundChannel: basic_channel_inbound::{Pallet, Call, Storage, Event<T>, Config} = 94,
        BasicOutboundChannel: basic_channel_outbound::{Pallet, Storage, Event<T>, Config<T>} = 95,
        IncentivizedInboundChannel: incentivized_channel_inbound::{Pallet, Call, Config<T>, Storage, Event<T>} = 96,
        IncentivizedOutboundChannel: incentivized_channel_outbound::{Pallet, Config<T>, Storage, Event<T>} = 97,
        Dispatch: dispatch::{Pallet, Storage, Event<T>, Origin} = 98,
        EthApp: eth_app::{Pallet, Call, Storage, Event<T>, Config<T>} = 99,
    }
}

// This is needed, because the compiler automatically places `Serialize` bound
// when `derive` is used, but the method is never actually used
#[cfg(feature = "std")]
impl Serialize for Runtime {
    fn serialize<S>(
        &self,
        _serializer: S,
    ) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        unreachable!("we never serialize runtime; qed")
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
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    ChargeTransactionPayment<Runtime>,
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
    AllPalletsWithSystem,
    MigratePalletVersionToStorageVersion,
>;

/// Migrate from `PalletVersion` to the new `StorageVersion`
pub struct MigratePalletVersionToStorageVersion;

impl OnRuntimeUpgrade for MigratePalletVersionToStorageVersion {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        frame_support::migrations::migrate_from_pallet_version_to_storage_version::<
            AllPalletsWithSystem,
        >(&RocksDbWeight::get())
    }
}

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
            OpaqueMetadata::new(Runtime::metadata().into())
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

        // fn random_seed() -> <Block as BlockT>::Hash {
        //     RandomnessCollectiveFlip::random_seed()
        // }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
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
            let maybe_dispatch_info = XorFee::query_info(&uxt, len);
            let output = match maybe_dispatch_info {
                Some(dispatch_info) => dispatch_info,
                _ => TransactionPayment::query_info(uxt, len),
            };
            output
        }

        fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
            let maybe_fee_details = XorFee::query_fee_details(&uxt, len);
            let output = match maybe_fee_details {
                Some(fee_details) => fee_details,
                _ => TransactionPayment::query_fee_details(uxt, len),
            };
            output
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
        #[cfg_attr(not(feature = "private-net"), allow(unused))]
        fn quote(
            dex_id: DEXId,
            liquidity_source_type: LiquiditySourceType,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
            desired_input_amount: BalanceWrapper,
            swap_variant: SwapVariant,
        ) -> Option<dex_runtime_api::SwapOutcomeInfo<Balance>> {
            #[cfg(feature = "private-net")]
            {
                DEXAPI::quote(
                    &LiquiditySourceId::new(dex_id, liquidity_source_type),
                    &input_asset_id,
                    &output_asset_id,
                    QuoteAmount::with_variant(swap_variant, desired_input_amount.into()),
                    true,
                ).ok().map(|sa| dex_runtime_api::SwapOutcomeInfo::<Balance> { amount: sa.amount, fee: sa.fee})
            }
            #[cfg(not(feature = "private-net"))]
            {
                // Mainnet should not be able to access liquidity source quote directly, to avoid arbitrage exploits.
                None
            }
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

    impl assets_runtime_api::AssetsAPI<Block, AccountId, AssetId, Balance, AssetSymbol, AssetName, BalancePrecision> for Runtime {
        fn free_balance(account_id: AccountId, asset_id: AssetId) -> Option<assets_runtime_api::BalanceInfo<Balance>> {
            Assets::free_balance(&asset_id, &account_id).ok().map(|balance|
                assets_runtime_api::BalanceInfo::<Balance> {
                    balance: balance.clone(),
                }
            )
        }

        fn usable_balance(account_id: AccountId, asset_id: AssetId) -> Option<assets_runtime_api::BalanceInfo<Balance>> {
            let usable_balance = if asset_id == <Runtime as currencies::Config>::GetNativeCurrencyId::get() {
                Balances::usable_balance(account_id)
            } else {
                let account_data = Tokens::accounts(account_id, asset_id);
                account_data.free.saturating_sub(account_data.frozen)
            };
            Some(assets_runtime_api::BalanceInfo { balance: usable_balance })
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

        fn list_asset_infos() -> Vec<assets_runtime_api::AssetInfo<AssetId, AssetSymbol, AssetName, u8>> {
            Assets::list_registered_asset_infos().into_iter().map(|(asset_id, symbol, name, precision, is_mintable)|
                assets_runtime_api::AssetInfo::<AssetId, AssetSymbol, AssetName, BalancePrecision> {
                    asset_id, symbol, name, precision, is_mintable
                }
            ).collect()
        }

        fn get_asset_info(asset_id: AssetId) -> Option<assets_runtime_api::AssetInfo<AssetId, AssetSymbol, AssetName, BalancePrecision>> {
            let (symbol, name, precision, is_mintable) = Assets::get_asset_info(&asset_id);
            Some(assets_runtime_api::AssetInfo::<AssetId, AssetSymbol, AssetName, BalancePrecision> {
                asset_id, symbol, name, precision, is_mintable,
            })
        }

        fn get_asset_content_src(asset_id: AssetId) -> Option<ContentSource> {
            Assets::get_asset_content_src(&asset_id)
        }
    }

    impl
        eth_bridge_runtime_api::EthBridgeRuntimeApi<
            Block,
            sp_core::H256,
            SignatureParams,
            AccountId,
            AssetKind,
            AssetId,
            sp_core::H160,
            OffchainRequest<Runtime>,
            RequestStatus,
            OutgoingRequestEncoded,
            NetworkId,
            BalancePrecision,
        > for Runtime
    {
        fn get_requests(
            hashes: Vec<sp_core::H256>,
            network_id: Option<NetworkId>,
            redirect_finished_load_requests: bool,
        ) -> Result<
            Vec<(
                OffchainRequest<Runtime>,
                RequestStatus,
            )>,
            DispatchError,
        > {
            EthBridge::get_requests(&hashes, network_id, redirect_finished_load_requests)
        }

        fn get_approved_requests(
            hashes: Vec<sp_core::H256>,
            network_id: Option<NetworkId>
        ) -> Result<
            Vec<(
                OutgoingRequestEncoded,
                Vec<SignatureParams>,
            )>,
            DispatchError,
        > {
            EthBridge::get_approved_requests(&hashes, network_id)
        }

        fn get_approvals(
            hashes: Vec<sp_core::H256>,
            network_id: Option<NetworkId>
        ) -> Result<Vec<Vec<SignatureParams>>, DispatchError> {
            EthBridge::get_approvals(&hashes, network_id)
        }

        fn get_account_requests(account_id: AccountId, status_filter: Option<RequestStatus>) -> Result<Vec<(NetworkId, sp_core::H256)>, DispatchError> {
            EthBridge::get_account_requests(&account_id, status_filter)
        }

        fn get_registered_assets(
            network_id: Option<NetworkId>
        ) -> Result<Vec<(
                AssetKind,
                (AssetId, BalancePrecision),
                Option<(sp_core::H160, BalancePrecision)
        >)>, DispatchError> {
            EthBridge::get_registered_assets(network_id)
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
            amount: BalanceWrapper,
            swap_variant: SwapVariant,
            selected_source_types: Vec<LiquiditySourceType>,
            filter_mode: FilterMode,
        ) -> Option<liquidity_proxy_runtime_api::SwapOutcomeInfo<Balance, AssetId>> {
            if LiquidityProxy::is_forbidden_filter(&input_asset_id, &output_asset_id, &selected_source_types, &filter_mode) {
                return None;
            }

            LiquidityProxy::inner_quote(
                &input_asset_id,
                &output_asset_id,
                QuoteAmount::with_variant(swap_variant, amount.into()),
                LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
                false,
                true,
            ).ok().map(|(asa, rewards, amount_without_impact)| liquidity_proxy_runtime_api::SwapOutcomeInfo::<Balance, AssetId> {
                amount: asa.amount,
                fee: asa.fee,
                rewards: rewards.into_iter()
                                .map(|(amount, currency, reason)| liquidity_proxy_runtime_api::RewardsInfo::<Balance, AssetId> {
                                    amount,
                                    currency,
                                    reason
                                })
                                .collect(),
                amount_without_impact: amount_without_impact.unwrap_or(0)})
        }

        fn is_path_available(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId
        ) -> bool {
            LiquidityProxy::is_path_available(
                dex_id, input_asset_id, output_asset_id
            ).unwrap_or(false)
        }

        fn list_enabled_sources_for_path(
            dex_id: DEXId,
            input_asset_id: AssetId,
            output_asset_id: AssetId,
        ) -> Vec<LiquiditySourceType> {
            LiquidityProxy::list_enabled_sources_for_path_with_xyk_forbidden(
                dex_id, input_asset_id, output_asset_id
            ).unwrap_or(Vec::new())
        }
    }

    impl pswap_distribution_runtime_api::PswapDistributionAPI<
        Block,
        AccountId,
        Balance,
    > for Runtime {
        fn claimable_amount(
            account_id: AccountId,
        ) -> pswap_distribution_runtime_api::BalanceInfo<Balance> {
            let claimable = PswapDistribution::claimable_amount(&account_id).unwrap_or(0);
            pswap_distribution_runtime_api::BalanceInfo::<Balance> {
                balance: claimable
            }
        }
    }

    impl rewards_runtime_api::RewardsAPI<Block, sp_core::H160, Balance> for Runtime {
        fn claimables(eth_address: sp_core::H160) -> Vec<rewards_runtime_api::BalanceInfo<Balance>> {
            Rewards::claimables(&eth_address).into_iter().map(|balance| rewards_runtime_api::BalanceInfo::<Balance> { balance }).collect()
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
                            genesis_authorities: Babe::authorities().to_vec(),
                            randomness: Babe::randomness(),
                            allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
                    }
            }

            fn current_epoch() -> sp_consensus_babe::Epoch {
                Babe::current_epoch()
            }

            fn current_epoch_start() -> sp_consensus_babe::Slot {
                Babe::current_epoch_start()
            }

            fn next_epoch() -> sp_consensus_babe::Epoch {
                Babe::next_epoch()
            }

            fn generate_key_ownership_proof(
                    _slot_number: sp_consensus_babe::Slot,
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

    impl beefy_primitives::BeefyApi<Block> for Runtime {
        fn validator_set() -> beefy_primitives::ValidatorSet<BeefyId> {
            Beefy::validator_set()
        }
    }

    impl pallet_mmr_primitives::MmrApi<Block, Hash> for Runtime {
        fn generate_proof(leaf_index: u64)
            -> Result<(mmr::EncodableOpaqueLeaf, mmr::Proof<Hash>), mmr::Error>
        {
            Mmr::generate_proof(leaf_index)
                .map(|(leaf, proof)| (mmr::EncodableOpaqueLeaf::from_leaf(&leaf), proof))
        }

        fn verify_proof(leaf: mmr::EncodableOpaqueLeaf, proof: mmr::Proof<Hash>)
            -> Result<(), mmr::Error>
        {
            pub type Leaf = <
                <Runtime as pallet_mmr::Config>::LeafData as mmr::LeafDataProvider
            >::LeafData;

            let leaf: Leaf = leaf
                .into_opaque_leaf()
                .try_decode()
                .ok_or(mmr::Error::Verify)?;
            Mmr::verify_leaf(leaf, proof)
        }

        fn verify_proof_stateless(
            root: Hash,
            leaf: mmr::EncodableOpaqueLeaf,
            proof: mmr::Proof<Hash>
        ) -> Result<(), mmr::Error> {
            type MmrHashing = <Runtime as pallet_mmr::Config>::Hashing;
            let node = mmr::DataOrHash::Data(leaf.into_opaque_leaf());
            pallet_mmr::verify_leaf_proof::<MmrHashing, _>(root, node, proof)
        }
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
        fn grandpa_authorities() -> GrandpaAuthorityList {
            Grandpa::grandpa_authorities()
        }

        fn current_set_id() -> fg_primitives::SetId {
            Grandpa::current_set_id()
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
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{list_benchmark, Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;

            use dex_api_benchmarking::Pallet as DEXAPIBench;
            use liquidity_proxy_benchmarking::Pallet as LiquidityProxyBench;
            use pool_xyk_benchmarking::Pallet as XYKPoolBench;
            use pswap_distribution_benchmarking::Pallet as PswapDistributionBench;
            use xor_fee_benchmarking::Pallet as XorFeeBench;
            use ceres_liquidity_locker_benchmarking::Pallet as CeresLiquidityLockerBench;

            let mut list = Vec::<BenchmarkList>::new();

            list_benchmark!(list, extra, assets, Assets);
            list_benchmark!(list, extra, dex_api, DEXAPIBench::<Runtime>);
            #[cfg(feature = "private-net")]
            list_benchmark!(list, extra, faucet, Faucet);
            list_benchmark!(list, extra, farming, Farming);
            list_benchmark!(list, extra, iroha_migration, IrohaMigration);
            list_benchmark!(list, extra, liquidity_proxy, LiquidityProxyBench::<Runtime>);
            list_benchmark!(list, extra, multicollateral_bonding_curve_pool, MulticollateralBondingCurvePool);
            list_benchmark!(list, extra, pswap_distribution, PswapDistributionBench::<Runtime>);
            list_benchmark!(list, extra, rewards, Rewards);
            list_benchmark!(list, extra, trading_pair, TradingPair);
            list_benchmark!(list, extra, pool_xyk, XYKPoolBench::<Runtime>);
            list_benchmark!(list, extra, eth_bridge, EthBridge);
            list_benchmark!(list, extra, vested_rewards, VestedRewards);
            list_benchmark!(list, extra, price_tools, PriceTools);
            list_benchmark!(list, extra, xor_fee, XorFeeBench::<Runtime>);
            list_benchmark!(list, extra, ethereum_light_client, EthereumLightClient);
            list_benchmark!(list, extra, referrals, Referrals);
            list_benchmark!(list, extra, ceres_staking, CeresStaking);
            list_benchmark!(list, extra, ceres_liquidity_locker, CeresLiquidityLockerBench::<Runtime>);

            let storage_info = AllPalletsWithSystem::storage_info();

            return (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark, TrackedStorageKey};

            use dex_api_benchmarking::Pallet as DEXAPIBench;
            use liquidity_proxy_benchmarking::Pallet as LiquidityProxyBench;
            use pool_xyk_benchmarking::Pallet as XYKPoolBench;
            use pswap_distribution_benchmarking::Pallet as PswapDistributionBench;
            use xor_fee_benchmarking::Pallet as XorFeeBench;
            use ceres_liquidity_locker_benchmarking::Pallet as CeresLiquidityLockerBench;

            impl dex_api_benchmarking::Config for Runtime {}
            impl liquidity_proxy_benchmarking::Config for Runtime {}
            impl pool_xyk_benchmarking::Config for Runtime {}
            impl pswap_distribution_benchmarking::Config for Runtime {}
            impl xor_fee_benchmarking::Config for Runtime {}
            impl ceres_liquidity_locker_benchmarking::Config for Runtime {}


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

            add_benchmark!(params, batches, assets, Assets);add_benchmark!(params, batches, dex_api, DEXAPIBench::<Runtime>);
            #[cfg(feature = "private-net")]
            add_benchmark!(params, batches, faucet, Faucet);
            add_benchmark!(params, batches, farming, Farming);
            add_benchmark!(params, batches, iroha_migration, IrohaMigration);
            add_benchmark!(params, batches, liquidity_proxy, LiquidityProxyBench::<Runtime>);
            add_benchmark!(params, batches, multicollateral_bonding_curve_pool, MulticollateralBondingCurvePool);
            add_benchmark!(params, batches, pswap_distribution, PswapDistributionBench::<Runtime>);
            add_benchmark!(params, batches, rewards, Rewards);
            add_benchmark!(params, batches, trading_pair, TradingPair);
            add_benchmark!(params, batches, pool_xyk, XYKPoolBench::<Runtime>);
            add_benchmark!(params, batches, eth_bridge, EthBridge);
            add_benchmark!(params, batches, vested_rewards, VestedRewards);
            add_benchmark!(params, batches, price_tools, PriceTools);
            add_benchmark!(params, batches, xor_fee, XorFeeBench::<Runtime>);
            add_benchmark!(params, batches, ethereum_light_client, EthereumLightClient);
            add_benchmark!(params, batches, referrals, Referrals);
            add_benchmark!(params, batches, ceres_staking, CeresStaking);
            add_benchmark!(params, batches, ceres_liquidity_locker, CeresLiquidityLockerBench::<Runtime>);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }
}
