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
#![recursion_limit = "512"]
// TODO #167: fix clippy warnings
#![allow(clippy::all)]

extern crate alloc;
use alloc::string::String;
use bridge_types::traits::Verifier;
use bridge_types::{SubNetworkId, H256};
use sp_runtime::traits::Keccak256;

mod bags_thresholds;
/// Constant values used within the runtime.
pub mod constants;
mod impls;
pub mod migrations;
mod xor_fee_impls;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
pub mod tests;

pub mod contracts;
pub mod weights;

use crate::impls::PreimageWeightInfo;
use crate::impls::{DispatchableSubstrateBridgeCall, SubstrateBridgeCallFilter};
#[cfg(feature = "wip")] // Trustless bridges
use bridge_types::types::LeafExtraData;
#[cfg(feature = "wip")] // EVM bridge
use bridge_types::{evm::AdditionalEVMInboundData, U256};
use common::prelude::constants::{BIG_FEE, SMALL_FEE};
use common::prelude::QuoteAmount;
use common::{AssetId32, Description, PredefinedAssetId};
use common::{DOT, XOR, XSTUSD};
use constants::currency::deposit;
use constants::time::*;
use frame_support::traits::EitherOf;
use frame_support::weights::ConstantMultiplier;

// // Make the WASM binary available.
#[cfg(all(feature = "std", feature = "build-wasm-binary"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use core::time::Duration;
use currencies::BasicCurrencyAdapter;
use frame_election_provider_support::{
    bounds::ElectionBoundsBuilder, generate_solution_type, onchain, SequentialPhragmen,
};
use frame_support::traits::{ConstU128, ConstU32, Currency, EitherOfDiverse};
use frame_system::offchain::{Account, SigningTypes};
use frame_system::EnsureRoot;
use frame_system::EnsureSigned;
use hex_literal::hex;
use pallet_grandpa::{
    fg_primitives, AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList,
};
use pallet_session::historical as pallet_session_historical;
use pallet_staking::sora::ValBurnedNotifier;
#[cfg(feature = "std")]
use serde::{Serialize, Serializer};
use sp_api::impl_runtime_apis;
pub use sp_beefy::ecdsa_crypto::AuthorityId as BeefyId;
use sp_beefy::ecdsa_crypto::Signature as BeefySignature;
#[cfg(feature = "wip")] // Trustless bridges
use sp_beefy::mmr::MmrLeafVersion;
use sp_core::crypto::KeyTypeId;
#[cfg(feature = "wip")] // Contracts pallet
use sp_core::ConstBool;
use sp_core::{Encode, OpaqueMetadata, H160};
use sp_mmr_primitives as mmr;
use sp_runtime::traits::{
    BlakeTwo256, Block as BlockT, Convert, IdentifyAccount, IdentityLookup, NumberFor, OpaqueKeys,
    SaturatedConversion, Verify,
};
use sp_runtime::transaction_validity::TransactionLongevity;
use sp_runtime::transaction_validity::{
    TransactionPriority, TransactionSource, TransactionValidity,
};
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys, ApplyExtrinsicResult, DispatchError,
    MultiSignature, Perbill, Percent, Permill, Perquintill,
};
use sp_std::cmp::Ordering;
use sp_std::prelude::*;
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::assert_eq_size;
use traits::{parameter_type_with_key, MultiCurrency};
use xor_fee::extension::ChargeTransactionPayment;

// A few exports that help ease life for downstream crates.
pub use common::prelude::{
    Balance, BalanceWrapper, PresetWeightInfo, SwapAmount, SwapOutcome, SwapVariant,
};
pub use common::weights::{BlockLength, BlockWeights, TransactionByteFee};
pub use common::{
    balance, fixed, fixed_from_basis_points, AssetInfoProvider, AssetName, AssetSymbol,
    BalancePrecision, BasisPoints, ContentSource, CrowdloanTag, DexInfoProvider, FilterMode, Fixed,
    FromGenericPair, LiquiditySource, LiquiditySourceFilter, LiquiditySourceId,
    LiquiditySourceType, OnPswapBurned, OnValBurned, SyntheticInfoProvider,
    TradingPairSourceManager,
};
use constants::rewards::{PSWAP_BURN_PERCENT, VAL_BURN_PERCENT};
pub use frame_support::dispatch::DispatchClass;
pub use frame_support::traits::schedule::Named as ScheduleNamed;
pub use frame_support::traits::{
    Contains, KeyOwnerProofSystem, LockIdentifier, OnUnbalanced, Randomness,
};
pub use frame_support::weights::constants::{BlockExecutionWeight, RocksDbWeight};
pub use frame_support::weights::Weight;
pub use frame_support::{construct_runtime, debug, parameter_types, StorageValue};
pub use pallet_balances::Call as BalancesCall;
pub use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
pub use pallet_transaction_payment::{Multiplier, MultiplierUpdate};
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
use sp_staking::currency_to_vote::U128CurrencyToVote;

use eth_bridge::offchain::SignatureParams;
use eth_bridge::requests::{AssetKind, OffchainRequest, OutgoingRequestEncoded, RequestStatus};
use impls::{
    CollectiveWeightInfo, DemocracyWeightInfo, NegativeImbalanceOf, OnUnbalancedDemocracySlash,
};

use frame_support::traits::{Everything, ExistenceRequirement, Get, PrivilegeCmp, WithdrawReasons};
#[cfg(feature = "runtime-benchmarks")]
pub use order_book_benchmarking;
#[cfg(feature = "private-net")]
pub use qa_tools;
pub use {
    assets, dex_api, eth_bridge, frame_system, kensetsu, liquidity_proxy,
    multicollateral_bonding_curve_pool, order_book, trading_pair, xst,
};

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
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// Digest item type.
pub type DigestItem = generic::DigestItem;

/// Identification of DEX.
pub type DEXId = u32;

pub type Moment = u64;

pub type PeriodicSessions = pallet_session::PeriodicSessions<SessionPeriod, SessionOffset>;

pub type CouncilCollective = pallet_collective::Instance1;
pub type TechnicalCollective = pallet_collective::Instance2;

type MoreThanHalfCouncil = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 1, 2>,
>;
type AtLeastHalfCouncil = EitherOfDiverse<
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
    EnsureRoot<AccountId>,
>;
type AtLeastTwoThirdsCouncil = EitherOfDiverse<
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
    EnsureRoot<AccountId>,
>;

type EventRecord = frame_system::EventRecord<
    <Runtime as frame_system::Config>::RuntimeEvent,
    <Runtime as frame_system::Config>::Hash,
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

/// Types used by oracle related pallets
pub mod oracle_types {
    use common::SymbolName;

    pub type Symbol = SymbolName;

    pub type ResolveTime = u64;
}
pub use oracle_types::*;

/// This runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("sora-substrate"),
    impl_name: create_runtime_str!("sora-substrate"),
    authoring_version: 1,
    spec_version: 91,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 91,
    state_version: 0,
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

#[cfg(feature = "private-net")]
parameter_types! {
    pub const BondingDuration: sp_staking::EraIndex = 1; // 1 era for unbonding (6 hours).
    pub const SlashDeferDuration: sp_staking::EraIndex = 0; // no slash cancellation on testnets expected.
}

#[cfg(not(feature = "private-net"))]
parameter_types! {
    pub const BondingDuration: sp_staking::EraIndex = 28; // 28 eras for unbonding (7 days).
    pub const SlashDeferDuration: sp_staking::EraIndex = 27; // 27 eras in which slashes can be cancelled (slightly less than 7 days).
}

parameter_types! {
    pub const BlockHashCount: BlockNumber = 250;
    pub const Version: RuntimeVersion = VERSION;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const EpochDuration: u64 = EPOCH_DURATION_IN_BLOCKS as u64;
    pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
    pub const SessionsPerEra: sp_staking::SessionIndex = 6; // 6 hours
    pub const ReportLongevity: u64 =
        BondingDuration::get() as u64 * SessionsPerEra::get() as u64 * EpochDuration::get();
    pub const MaxNominatorRewardedPerValidator: u32 = 256;
    pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
    pub const MaxIterations: u32 = 10;
    // 0.05%. The higher the value, the more strict solution acceptance becomes.
    pub MinSolutionScoreBump: Perbill = Perbill::from_rational(5u32, 10_000);
    pub const ValRewardCurve: pallet_staking::sora::ValRewardCurve = pallet_staking::sora::ValRewardCurve {
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
    /// A limit for off-chain phragmen unsigned solution length.
    ///
    /// We allow up to 90% of the block's size to be consumed by the solution.
    pub OffchainSolutionLengthLimit: u32 = Perbill::from_rational(90_u32, 100) *
        *BlockLength::get()
        .max
        .get(DispatchClass::Normal);
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
    pub const DemocracyMaxDeposits: u32 = 100;
    pub const DemocracyMaxBlacklisted: u32 = 100;
    pub const CouncilCollectiveMotionDuration: BlockNumber = 5 * DAYS;
    pub const CouncilCollectiveMaxProposals: u32 = 100;
    pub const CouncilCollectiveMaxMembers: u32 = 100;
    pub const TechnicalCollectiveMotionDuration: BlockNumber = 5 * DAYS;
    pub const TechnicalCollectiveMaxProposals: u32 = 100;
    pub const TechnicalCollectiveMaxMembers: u32 = 100;
    pub SchedulerMaxWeight: Weight = Perbill::from_percent(50) * BlockWeights::get().max_block;
    pub const MaxScheduledPerBlock: u32 = 50;
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
    pub const ElectionsMaxVoters: u32 = 10000;
    pub const ElectionsMaxCandidates: u32 = 1000;
    pub const ElectionsModuleId: LockIdentifier = *b"phrelect";
    pub const MaxVotesPerVoter: u32 = 16;
    pub FarmingRewardDoublingAssets: Vec<AssetId> = vec![
        GetPswapAssetId::get(), GetValAssetId::get(), GetDaiAssetId::get(), GetEthAssetId::get(),
        GetXstAssetId::get(), GetTbcdAssetId::get(), DOT
    ];
    pub const MaxAuthorities: u32 = 100_000;
    pub const NoPreimagePostponement: Option<u32> = Some(10);
    // TODO! Change this parameter
    pub MaxProposalWeight: Weight = Weight::from_parts(u64::MAX, u64::MAX);
}

pub struct BaseCallFilter;

impl Contains<RuntimeCall> for BaseCallFilter {
    fn contains(call: &RuntimeCall) -> bool {
        if call.swap_count() > 1 {
            return false;
        }
        if matches!(
            call,
            RuntimeCall::BridgeMultisig(bridge_multisig::Call::register_multisig { .. })
        ) {
            return false;
        }
        true
    }
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = BaseCallFilter;
    type BlockWeights = BlockWeights;
    /// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
    type BlockLength = BlockLength;
    /// The ubiquitous origin type.
    type RuntimeOrigin = RuntimeOrigin;
    /// The aggregated dispatch type that is available for extrinsics.
    type RuntimeCall = RuntimeCall;
    /// The index type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    // /// The index type for blocks.
    // type BlockNumber = BlockNumber;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The hashing algorithm used.
    type Hashing = BlakeTwo256;
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The lookup mechanism to get account ID from whatever is passed in dispatchers.
    type Lookup = IdentityLookup<AccountId>;
    // /// The header type.
    // type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
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
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
    /// The block type.
    type Block = Block;
}

impl pallet_babe::Config for Runtime {
    type EpochDuration = EpochDuration;
    type ExpectedBlockTime = ExpectedBlockTime;
    type EpochChangeTrigger = pallet_babe::ExternalTrigger;
    type DisabledValidators = Session;
    type KeyOwnerProof =
        <Historical as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;
    // type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
    //     KeyTypeId,
    //     pallet_babe::AuthorityId,
    // )>>::IdentificationTuple;
    // type KeyOwnerProofSystem = Historical;
    // type HandleEquivocation =
    //     pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, Offences, ReportLongevity>;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;

    type MaxNominators = MaxNominatorRewardedPerValidator;
    type EquivocationReportSystem =
        pallet_babe::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

impl pallet_collective::Config<CouncilCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = CouncilCollectiveMotionDuration;
    type MaxProposals = CouncilCollectiveMaxProposals;
    type MaxMembers = CouncilCollectiveMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = CollectiveWeightInfo<Self>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_collective::Config<TechnicalCollective> for Runtime {
    type RuntimeOrigin = RuntimeOrigin;
    type Proposal = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type MotionDuration = TechnicalCollectiveMotionDuration;
    type MaxProposals = TechnicalCollectiveMaxProposals;
    type MaxMembers = TechnicalCollectiveMaxMembers;
    type DefaultVote = pallet_collective::PrimeDefaultVote;
    type WeightInfo = CollectiveWeightInfo<Self>;
    type SetMembersOrigin = EnsureRoot<AccountId>;
    type MaxProposalWeight = MaxProposalWeight;
}

impl pallet_democracy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
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
    type FastTrackOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 1, 2>,
        EnsureRoot<AccountId>,
    >;
    type InstantOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
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
    type Slash = OnUnbalancedDemocracySlash<Self>;
    type Scheduler = Scheduler;
    type PalletsOrigin = OriginCaller;
    type MaxVotes = DemocracyMaxVotes;
    type WeightInfo = DemocracyWeightInfo;
    type MaxProposals = DemocracyMaxProposals;
    type VoteLockingPeriod = DemocracyEnactmentPeriod;
    type Preimages = Preimage;
    type MaxDeposits = DemocracyMaxDeposits;
    type MaxBlacklisted = DemocracyMaxBlacklisted;
    type SubmitOrigin = frame_system::EnsureSigned<AccountId>;
}

impl pallet_elections_phragmen::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PalletId = ElectionsModuleId;
    type Currency = Balances;
    type ChangeMembers = Council;
    type InitializeMembers = Council;
    type CurrencyToVote = U128CurrencyToVote;
    type CandidacyBond = ElectionsCandidacyBond;
    type VotingBondBase = ElectionsVotingBondBase;
    type VotingBondFactor = ElectionsVotingBondFactor;
    type LoserCandidate = OnUnbalancedDemocracySlash<Self>;
    type KickedMember = OnUnbalancedDemocracySlash<Self>;
    type DesiredMembers = ElectionsDesiredMembers;
    type DesiredRunnersUp = ElectionsDesiredRunnersUp;
    type TermDuration = ElectionsTermDuration;
    type MaxVoters = ElectionsMaxVoters;
    type MaxCandidates = ElectionsMaxCandidates;
    type WeightInfo = ();
    type MaxVotesPerVoter = MaxVotesPerVoter;
}

impl pallet_membership::Config<pallet_membership::Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AddOrigin = MoreThanHalfCouncil;
    type RemoveOrigin = MoreThanHalfCouncil;
    type SwapOrigin = MoreThanHalfCouncil;
    type ResetOrigin = MoreThanHalfCouncil;
    type PrimeOrigin = MoreThanHalfCouncil;
    type MembershipInitialized = TechnicalCommittee;
    type MembershipChanged = TechnicalCommittee;
    type MaxMembers = TechnicalCollectiveMaxMembers;
    type WeightInfo = ();
}

parameter_types! {
    pub const MaxSetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

impl pallet_grandpa::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    // type KeyOwnerProofSystem = Historical;

    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

    // type KeyOwnerIdentification = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(
    //     KeyTypeId,
    //     GrandpaId,
    // )>>::IdentificationTuple;

    // type HandleEquivocation = pallet_grandpa::EquivocationHandler<
    //     Self::KeyOwnerIdentification,
    //     Offences,
    //     ReportLongevity,
    // >;
    type WeightInfo = ();
    type MaxAuthorities = MaxAuthorities;
    type MaxSetIdSessionEntries = MaxSetIdSessionEntries;

    type MaxNominators = MaxNominatorRewardedPerValidator;
    type EquivocationReportSystem =
        pallet_grandpa::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
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
    type RuntimeEvent = RuntimeEvent;
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
    type EventHandler = (Staking, ImOnline);
}

/// A reasonable benchmarking config for staking pallet.
pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxValidators = ConstU32<1000>;
    type MaxNominators = ConstU32<1000>;
}

parameter_types! {
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const MaxNominations: u32 = <NposCompactSolution24 as frame_election_provider_support::NposSolution>::LIMIT as u32;
}

type StakingAdminOrigin = EitherOfDiverse<
    EnsureRoot<AccountId>,
    pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
>;

impl pallet_staking::Config for Runtime {
    type Currency = Balances;
    type MultiCurrency = Tokens;
    type CurrencyBalance = Balance;
    type ValTokenId = GetValAssetId;
    type ValRewardCurve = ValRewardCurve;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type RuntimeEvent = RuntimeEvent;
    type Slash = ();
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    type SlashDeferDuration = SlashDeferDuration;
    type AdminOrigin = StakingAdminOrigin;
    type SessionInterface = Self;
    type NextNewSession = Session;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type VoterList = BagsList;
    type ElectionProvider = ElectionProviderMultiPhase;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type MaxUnlockingChunks = ConstU32<32>;
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    // type MaxNominations = MaxNominations;
    // type NominationsQuota = MaxNominations;
    type NominationsQuota = pallet_staking::FixedNominationsQuota<{ MaxNominations::get() }>;
    type GenesisElectionProvider = onchain::OnChainExecution<OnChainSeqPhragmen>;
    // type OnStakerSlash = ();
    type HistoryDepth = frame_support::traits::ConstU32<84>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
    type EventListeners = ();
    type WeightInfo = ();
}

/// The numbers configured here could always be more than the the maximum limits of staking pallet
/// to ensure election snapshot will not run out of memory. For now, we set them to smaller values
/// since the staking is bounded and the weight pipeline takes hours for this single pallet.
pub struct ElectionBenchmarkConfig;
impl pallet_election_provider_multi_phase::BenchmarkingConfig for ElectionBenchmarkConfig {
    const VOTERS: [u32; 2] = [1000, 2000];
    const TARGETS: [u32; 2] = [500, 1000];
    const ACTIVE_VOTERS: [u32; 2] = [500, 800];
    const DESIRED_TARGETS: [u32; 2] = [200, 400];
    const SNAPSHOT_MAXIMUM_VOTERS: u32 = 1000;
    const MINER_MAXIMUM_VOTERS: u32 = 1000;
    const MAXIMUM_TARGETS: u32 = 300;
}

parameter_types! {
    // phase durations. 1/4 of the last session for each.
    // in testing: 1min or half of the session for each
    pub SignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;
    pub UnsignedPhase: u32 = EPOCH_DURATION_IN_BLOCKS / 4;

    // signed config
    pub const SignedMaxSubmissions: u32 = 16;
    pub const SignedMaxRefunds: u32 = 16 / 4;
    pub const SignedDepositBase: Balance = deposit(2, 0);
    pub const SignedDepositByte: Balance = deposit(0, 10) / 1024;
    pub SignedRewardBase: Balance =  constants::currency::UNITS / 10;
    pub SolutionImprovementThreshold: Perbill = Perbill::from_rational(5u32, 10_000);
    pub BetterUnsignedThreshold: Perbill = Perbill::from_rational(5u32, 10_000);

    // 1 hour session, 15 minutes unsigned phase, 8 offchain executions.
    pub OffchainRepeat: BlockNumber = UnsignedPhase::get() / 8;

    /// We take the top 12500 nominators as electing voters..
    pub const MaxElectingVoters: u32 = 12_500;
    /// ... and all of the validators as electable targets. Whilst this is the case, we cannot and
    /// shall not increase the size of the validator intentions.
    pub const MaxElectableTargets: u16 = u16::MAX;
    /// Setup election pallet to support maximum winners upto 1200. This will mean Staking Pallet
    /// cannot have active validators higher than this count.
    pub const MaxActiveValidators: u32 = 1200;
    pub NposSolutionPriority: TransactionPriority =
        Perbill::from_percent(90) * TransactionPriority::max_value();

    /// We take the top 12500 nominators as electing voters and all of the validators as electable
    /// targets. Whilst this is the case, we cannot and shall not increase the size of the
    /// validator intentions.
    pub ElectionBounds: frame_election_provider_support::bounds::ElectionBounds =
        ElectionBoundsBuilder::default().voters_count(MaxElectingVoters::get().into()).build();
}

generate_solution_type!(
    #[compact]
    pub struct NposCompactSolution24::<
        VoterIndex = u32,
        TargetIndex = u16,
        Accuracy = sp_runtime::PerU16,
        MaxVoters = MaxElectingVoters,
    >(24)
);

/// The accuracy type used for genesis election provider;
pub type OnChainAccuracy = sp_runtime::Perbill;

pub struct OnChainSeqPhragmen;
impl onchain::Config for OnChainSeqPhragmen {
    type System = Runtime;
    type Solver = SequentialPhragmen<AccountId, OnChainAccuracy>;
    type DataProvider = Staking;
    type WeightInfo = ();
    type MaxWinners = MaxActiveValidators;
    // type VotersBound = MaxElectingVoters;
    // type TargetsBound = MaxElectableTargets;
    type Bounds = ElectionBounds;
}

impl pallet_election_provider_multi_phase::MinerConfig for Runtime {
    type AccountId = AccountId;
    type MaxLength = OffchainSolutionLengthLimit;
    type MaxWeight = OffchainSolutionWeightLimit;
    type Solution = NposCompactSolution24;
    type MaxVotesPerVoter = <
		<Self as pallet_election_provider_multi_phase::Config>::DataProvider
		as
		frame_election_provider_support::ElectionDataProvider
	>::MaxVotesPerVoter;
    type MaxWinners = MaxActiveValidators;

    // The unsigned submissions have to respect the weight of the submit_unsigned call, thus their
    // weight estimate function is wired to this call's weight.
    fn solution_weight(v: u32, t: u32, a: u32, d: u32) -> Weight {
        <
			<Self as pallet_election_provider_multi_phase::Config>::WeightInfo
			as
			pallet_election_provider_multi_phase::WeightInfo
		>::submit_unsigned(v, t, a, d)
    }
}

impl pallet_election_provider_multi_phase::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type EstimateCallFee = TransactionPayment;
    type UnsignedPhase = UnsignedPhase;
    type SignedMaxSubmissions = SignedMaxSubmissions;
    type SignedMaxRefunds = SignedMaxRefunds;
    type SignedRewardBase = SignedRewardBase;
    type SignedDepositBase = SignedDepositBase;
    type SignedDepositByte = SignedDepositByte;
    type SignedDepositWeight = ();
    type SignedMaxWeight =
        <Self::MinerConfig as pallet_election_provider_multi_phase::MinerConfig>::MaxWeight;
    type MinerConfig = Self;
    type SlashHandler = (); // burn slashes
    type RewardHandler = (); // nothing to do upon rewards
    type SignedPhase = SignedPhase;
    type BetterUnsignedThreshold = BetterUnsignedThreshold;
    type BetterSignedThreshold = ();
    type OffchainRepeat = OffchainRepeat;
    type MinerTxPriority = NposSolutionPriority;
    type DataProvider = Staking;
    type Fallback = frame_election_provider_support::NoElection<(
        AccountId,
        BlockNumber,
        Staking,
        MaxActiveValidators,
    )>;
    type GovernanceFallback = onchain::OnChainExecution<OnChainSeqPhragmen>;
    type Solver = SequentialPhragmen<
        AccountId,
        pallet_election_provider_multi_phase::SolutionAccuracyOf<Self>,
        (),
    >;
    type BenchmarkingConfig = ElectionBenchmarkConfig;
    type ForceOrigin = EitherOfDiverse<
        EnsureRoot<AccountId>,
        EitherOfDiverse<
            pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
            pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 2, 3>,
        >,
    >;
    type WeightInfo = ();
    // type MaxElectingVoters = MaxElectingVoters;
    // type MaxElectableTargets = MaxElectableTargets;
    type MaxWinners = MaxActiveValidators;
    type ElectionBounds = ElectionBounds;
}

parameter_types! {
    pub const BagThresholds: &'static [u64] = &bags_thresholds::THRESHOLDS;
}

impl pallet_bags_list::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ScoreProvider = Staking;
    type WeightInfo = ();
    type BagThresholds = BagThresholds;
    type Score = sp_npos_elections::VoteWeight;
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
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = SchedulerMaxWeight;
    type ScheduleOrigin = frame_system::EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = MaxScheduledPerBlock;
    type WeightInfo = ();
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = Preimage;
}

parameter_types! {
    pub PreimageBaseDeposit: Balance = deposit(2, 64);
    pub PreimageByteDeposit: Balance = deposit(0, 1);
}

impl pallet_preimage::Config for Runtime {
    type WeightInfo = PreimageWeightInfo;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type BaseDeposit = PreimageBaseDeposit;
    type ByteDeposit = PreimageByteDeposit;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const MaxLocks: u32 = 50;
    pub const MaxHolds: u32 = 2;
}

impl pallet_balances::Config for Runtime {
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = MaxLocks;
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type RuntimeHoldReason = RuntimeHoldReason;
    type FreezeIdentifier = ();
    type MaxHolds = MaxHolds;
    type MaxFreezes = ();
}

pub type Amount = i128;

parameter_type_with_key! {
    pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
        0
    };
}

impl tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

parameter_types! {
    // This is common::PredefinedAssetId with 0 index, 2 is size, 0 and 0 is code.
    pub const GetXorAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::XOR);
    pub const GetValAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::VAL);
    pub const GetPswapAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::PSWAP);
    pub const GetDaiAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::DAI);
    pub const GetEthAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::ETH);
    pub const GetXstAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::XST);
    pub const GetTbcdAssetId: AssetId = AssetId32::from_asset_id(PredefinedAssetId::TBCD);

    pub const GetBaseAssetId: AssetId = GetXorAssetId::get();
    pub const GetBuyBackAssetId: AssetId = GetXstAssetId::get();
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![GetValAssetId::get(), GetPswapAssetId::get()];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!("feb92c0acb61f75309730290db5cbe8ac9b46db7ad6f3bbb26a550a73586ea71"));
    pub const GetBuyBackDexId: DEXId = 0;
    pub const GetSyntheticBaseAssetId: AssetId = GetXstAssetId::get();
    pub const GetADARAccountId: AccountId = AccountId::new(hex!("dc5201cda01113be2ca9093c49a92763c95c708dd61df70c945df749c365da5d"));
}

impl currencies::Config for Runtime {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = common::LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
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
    type BuyBackLiquidityProxy = liquidity_proxy::Pallet<Runtime>;
    type Currency = currencies::Pallet<Runtime>;
    type GetTotalBalance = GetTotalBalance;
    type WeightInfo = assets::weights::SubstrateWeight<Runtime>;
    #[cfg(feature = "ready-to-test")] // DeFi-R
    type AssetRegulator = (
        permissions::Pallet<Runtime>,
        extended_assets::Pallet<Runtime>,
    );
    #[cfg(not(feature = "ready-to-test"))] // DeFi-R
    type AssetRegulator = permissions::Pallet<Runtime>;
}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl dex_manager::Config for Runtime {}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<PredefinedAssetId>;
pub type AssetId = AssetId32<PredefinedAssetId>;

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub GetFee: Fixed = fixed!(0.003);
    pub GetXykIrreducibleReservePercent: Percent = Percent::from_percent(1);
}

parameter_type_with_key! {
    pub GetTradingPairRestrictedFlag: |trading_pair: common::TradingPair<AssetId>| -> bool {
        let common::TradingPair {
            base_asset_id,
            target_asset_id
        } = trading_pair;
        (base_asset_id, target_asset_id) == (&XSTUSD.into(), &XOR.into())
    };
}

parameter_type_with_key! {
    pub GetChameleonPoolBaseAssetId: |base_asset_id: AssetId| -> Option<AssetId> {
        if base_asset_id == &common::XOR {
            Some(common::KXOR)
        } else {
            None
        }
    };
}

parameter_type_with_key! {
    pub GetChameleonPool: |tpair: common::TradingPair<AssetId>| -> bool {
        tpair.base_asset_id == common::XOR && tpair.target_asset_id == common::ETH
    };
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = pool_xyk::PairSwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<DEXId, AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type EnabledSourcesManager = trading_pair::Pallet<Runtime>;
    type GetFee = GetFee;
    type OnPoolCreated = (PswapDistribution, Farming);
    type OnPoolReservesChanged = PriceTools;
    type XSTMarketInfo = XSTPool;
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
    type GetChameleonPool = GetChameleonPool;
    type GetChameleonPoolBaseAssetId = GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    #[cfg(feature = "ready-to-test")] // DeFi-R
    type AssetRegulator = extended_assets::Pallet<Runtime>;
    #[cfg(not(feature = "ready-to-test"))] // DeFi-R
    type AssetRegulator = ();
    type IrreducibleReserve = GetXykIrreducibleReservePercent;
    type WeightInfo = pool_xyk::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub GetLiquidityProxyTechAccountId: TechAccountId = {
        // TODO(Harrm): why pswap_distribution?
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
    pub const GetNumSamples: usize = 10;
    pub const BasicDeposit: Balance = balance!(0.01);
    pub const FieldDeposit: Balance = balance!(0.01);
    pub const SubAccountDeposit: Balance = balance!(0.01);
    pub const MaxSubAccounts: u32 = 100;
    pub const MaxAdditionalFields: u32 = 100;
    pub const MaxRegistrars: u32 = 20;
    pub const MaxAdditionalDataLengthXorlessTransfer: u32 = 128;
    pub const MaxAdditionalDataLengthSwapTransferBatch: u32 = 2000;
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
    pub GetInternalSlippageTolerancePercent: Permill = Permill::from_rational(1u32, 1000); // 0.1%
}

impl liquidity_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityRegistry = dex_api::Pallet<Runtime>;
    type GetNumSamples = GetNumSamples;
    type GetTechnicalAccountId = GetLiquidityProxyAccountId;
    type PrimaryMarketTBC = multicollateral_bonding_curve_pool::Pallet<Runtime>;
    type PrimaryMarketXST = xst::Pallet<Runtime>;
    type SecondaryMarket = pool_xyk::Pallet<Runtime>;
    type VestedRewardsPallet = VestedRewards;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type LockedLiquiditySourcesManager = trading_pair::Pallet<Runtime>;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type GetADARAccountId = GetADARAccountId;
    type ADARCommissionRatioUpdateOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 1, 2>,
        EnsureRoot<AccountId>,
    >;
    type MaxAdditionalDataLengthXorlessTransfer = MaxAdditionalDataLengthXorlessTransfer;
    type MaxAdditionalDataLengthSwapTransferBatch = MaxAdditionalDataLengthSwapTransferBatch;
    type GetChameleonPool = GetChameleonPool;
    type GetChameleonPoolBaseAssetId = GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type InternalSlippageTolerance = GetInternalSlippageTolerancePercent;
    type WeightInfo = liquidity_proxy::weights::SubstrateWeight<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance2> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance3> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl mock_liquidity_source::Config<mock_liquidity_source::Instance4> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl dex_api::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
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
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type OrderBook = order_book::Pallet<Runtime>;

    type WeightInfo = dex_api::weights::SubstrateWeight<Runtime>;
}

impl pallet_multisig::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl iroha_migration::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = iroha_migration::weights::SubstrateWeight<Runtime>;
}

impl pallet_identity::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
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
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        public: <Signature as sp_runtime::traits::Verify>::Signer,
        account: AccountId,
        index: Nonce,
    ) -> Option<(
        RuntimeCall,
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
                log::warn!("SignedPayload error: {:?}", e);
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
    RuntimeCall: From<C>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = UncheckedExtrinsic;
}

impl referrals::Config for Runtime {
    type ReservesAcc = ReferralsReservesAcc;
    type WeightInfo = referrals::weights::SubstrateWeight<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl rewards::Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumber = 1 * DAYS;
    const UPDATE_FREQUENCY: BlockNumber = 10 * MINUTES;
    const MAX_CHUNK_SIZE: usize = 100;
    const MAX_VESTING_RATIO: Percent = Percent::from_percent(55);
    const TIME_TO_SATURATION: BlockNumber = 5 * 365 * DAYS; // 5 years
    const VAL_BURN_PERCENT: Percent = VAL_BURN_PERCENT;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = rewards::weights::SubstrateWeight<Runtime>;
}

pub struct ValBurnedAggregator<T>(sp_std::marker::PhantomData<T>);

impl<T> OnValBurned for ValBurnedAggregator<T>
where
    T: ValBurnedNotifier<Balance>,
{
    fn on_val_burned(amount: Balance) {
        Rewards::on_val_burned(amount);
        T::notify_val_burned(amount);
    }
}

parameter_types! {
    pub const DEXIdValue: DEXId = 0;
}

impl xor_fee::Config for Runtime {
    type PermittedSetPeriod = EitherOfDiverse<
        pallet_collective::EnsureProportionAtLeast<AccountId, TechnicalCollective, 3, 4>,
        EnsureRoot<AccountId>,
    >;
    #[cfg(not(feature = "wip"))] // Dynamic fee
    type DynamicMultiplier = ();
    #[cfg(feature = "wip")] // Dynamic fee
    type DynamicMultiplier = xor_fee_impls::DynamicMultiplier;
    type RuntimeEvent = RuntimeEvent;
    // Pass native currency.
    type XorCurrency = Balances;
    type XorId = GetXorAssetId;
    type ValId = GetValAssetId;
    type TbcdId = GetTbcdAssetId;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type BuyBackTBCDPercent = BuyBackTBCDPercent;
    type DEXIdValue = DEXIdValue;
    type LiquidityProxy = LiquidityProxy;
    type OnValBurned = ValBurnedAggregator<Staking>;
    type CustomFees = xor_fee_impls::CustomFees;
    type GetTechnicalAccountId = GetXorFeeAccountId;
    type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
    type SessionManager = Staking;
    type ReferrerAccountProvider = Referrals;
    type BuyBackHandler = liquidity_proxy::LiquidityProxyBuyBackHandler<Runtime, GetBuyBackDexId>;
    type WeightInfo = xor_fee::weights::SubstrateWeight<Runtime>;
    type WithdrawFee = xor_fee_impls::WithdrawFee;
}

pub struct ConstantFeeMultiplier;

impl MultiplierUpdate for ConstantFeeMultiplier {
    fn min() -> Multiplier {
        Default::default()
    }
    fn max() -> Multiplier {
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
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = XorFee;
    type WeightToFee = XorFee;
    type FeeMultiplierUpdate = ConstantFeeMultiplier;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type LengthToFee = ConstantMultiplier<Balance, ConstU128<0>>;
}

#[cfg(feature = "private-net")]
impl pallet_sudo::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = ();
    type PalletsOrigin = OriginCaller;
}

parameter_types! {
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 100;
}

impl bridge_multisig::Config for Runtime {
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetEthNetworkId: u32 = 0;
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
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PeerId = eth_bridge::offchain::crypto::TestAuthId;
    type NetworkId = NetworkId;
    type GetEthNetworkId = GetEthNetworkId;
    type WeightInfo = eth_bridge::weights::SubstrateWeight<Runtime>;
    type WeightToFee = XorFee;
    type MessageStatusNotifier = BridgeProxy;
    type BridgeAssetLockChecker = BridgeProxy;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

#[cfg(feature = "private-net")]
impl faucet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = faucet::weights::SubstrateWeight<Runtime>;
}

#[cfg(feature = "private-net")]
impl qa_tools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetInfoProvider = Assets;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type SyntheticInfoProvider = XSTPool;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = qa_tools::weights::SubstrateWeight<Runtime>;
    type Symbol = <Runtime as band::Config>::Symbol;
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
    pub GetTrustlessBridgeTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetTrustlessBridgeAccountId: AccountId = {
        let tech_account_id = GetTrustlessBridgeTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetTrustlessBridgeFeesTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_FEES.to_vec(),
        );
        tech_account_id
    };
    pub GetTrustlessBridgeFeesAccountId: AccountId = {
        let tech_account_id = GetTrustlessBridgeFeesTechAccountId::get();
        let account_id =
            technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetTreasuryTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_TREASURY_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetTreasuryAccountId: AccountId = {
        let tech_account_id = GetTreasuryTechAccountId::get();
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
    type RuntimeCall = RuntimeCall;
    type SchedulerOriginCaller = OriginCaller;
    type Scheduler = Scheduler;
    type RewardDoublingAssets = FarmingRewardDoublingAssets;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = farming::weights::SubstrateWeight<Runtime>;
    type RuntimeEvent = RuntimeEvent;
}

impl pswap_distribution::Config for Runtime {
    const PSWAP_BURN_PERCENT: Percent = PSWAP_BURN_PERCENT;
    type RuntimeEvent = RuntimeEvent;
    type GetIncentiveAssetId = GetPswapAssetId;
    type GetTBCDAssetId = GetTbcdAssetId;
    type LiquidityProxy = LiquidityProxy;
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = GetDefaultSubscriptionFrequency;
    type GetBurnUpdateFrequency = GetBurnUpdateFrequency;
    type GetTechnicalAccountId = GetPswapDistributionAccountId;
    type EnsureDEXManager = DEXManager;
    type OnPswapBurnedAggregator = RuntimeOnPswapBurnedAggregator;
    type WeightInfo = pswap_distribution::weights::SubstrateWeight<Runtime>;
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXYK;
    type BuyBackHandler = liquidity_proxy::LiquidityProxyBuyBackHandler<Runtime, GetBuyBackDexId>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type GetChameleonPoolBaseAssetId = GetChameleonPoolBaseAssetId;
    type AssetInfoProvider = assets::Pallet<Runtime>;
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
    pub GetTBCBuyBackTBCDPercent: Fixed = fixed!(0.025);
    pub GetTbcIrreducibleReservePercent: Percent = Percent::from_percent(1);
}

impl multicollateral_bonding_curve_pool::Config for Runtime {
    const RETRY_DISTRIBUTION_FREQUENCY: BlockNumber = 1000;
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = LiquidityProxy;
    type EnsureDEXManager = DEXManager;
    type EnsureTradingPairExists = TradingPair;
    type PriceToolsPallet = PriceTools;
    type VestedRewardsPallet = VestedRewards;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type BuyBackHandler = liquidity_proxy::LiquidityProxyBuyBackHandler<Runtime, GetBuyBackDexId>;
    type BuyBackTBCDPercent = GetTBCBuyBackTBCDPercent;
    type AssetInfoProvider = assets::Pallet<Runtime>;
    type IrreducibleReserve = GetTbcIrreducibleReservePercent;
    type WeightInfo = multicollateral_bonding_curve_pool::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const GetXstPoolConversionAssetId: AssetId = GetXstAssetId::get();
    pub const GetSyntheticBaseBuySellLimit: Balance = Balance::MAX;
}

impl xst::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type GetSyntheticBaseAssetId = GetXstPoolConversionAssetId;
    type GetXSTPoolPermissionedTechAccountId = GetXSTPoolPermissionedTechAccountId;
    type EnsureDEXManager = DEXManager;
    type PriceToolsPallet = PriceTools;
    type WeightInfo = xst::weights::SubstrateWeight<Runtime>;
    type Oracle = OracleProxy;
    type Symbol = <Runtime as band::Config>::Symbol;
    type TradingPairSourceManager = TradingPair;
    type GetSyntheticBaseBuySellLimit = GetSyntheticBaseBuySellLimit;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub const MaxKeys: u32 = 10_000;
    pub const MaxPeerInHeartbeats: u32 = 10_000;
    pub const MaxPeerDataEncodingSize: u32 = 1_000;
}

impl pallet_im_online::Config for Runtime {
    type AuthorityId = ImOnlineId;
    type RuntimeEvent = RuntimeEvent;
    type ValidatorSet = Historical;
    type NextSessionRotation = Babe;
    type ReportUnresponsiveness = Offences;
    type UnsignedPriority = ImOnlineUnsignedPriority;
    type WeightInfo = ();
    type MaxKeys = MaxKeys;
    type MaxPeerInHeartbeats = MaxPeerInHeartbeats;
    // type MaxPeerDataEncodingSize = MaxPeerDataEncodingSize;
}

impl pallet_offences::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type IdentificationTuple = pallet_session::historical::IdentificationTuple<Self>;
    type OnOffenceHandler = Staking;
}

impl vested_rewards::Config for Runtime {
    const BLOCKS_PER_DAY: BlockNumber = 1 * DAYS;
    type RuntimeEvent = RuntimeEvent;
    type GetBondingCurveRewardsAccountId = GetMbcPoolRewardsAccountId;
    type GetFarmingRewardsAccountId = GetFarmingRewardsAccountId;
    type GetMarketMakerRewardsAccountId = GetMarketMakerRewardsAccountId;
    type WeightInfo = vested_rewards::weights::SubstrateWeight<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl price_tools::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type LiquidityProxy = LiquidityProxy;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = price_tools::weights::SubstrateWeight<Runtime>;
}

impl pallet_randomness_collective_flip::Config for Runtime {}

parameter_types! {
    pub BeefySetIdSessionEntries: u32 = BondingDuration::get() * SessionsPerEra::get();
}

#[cfg(not(feature = "wip"))] // Basic impl for session keys
impl pallet_beefy::Config for Runtime {
    type BeefyId = BeefyId;
    type MaxAuthorities = MaxAuthorities;
    type OnNewValidatorSet = ();

    type MaxNominators = MaxNominatorRewardedPerValidator;
    type MaxSetIdSessionEntries = BeefySetIdSessionEntries;
    type WeightInfo = ();
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BeefyId)>>::Proof;
    type EquivocationReportSystem =
        pallet_beefy::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

#[cfg(feature = "wip")] // Trustless bridges
impl pallet_beefy::Config for Runtime {
    type BeefyId = BeefyId;
    type MaxAuthorities = MaxAuthorities;
    type OnNewValidatorSet = MmrLeaf;

    type MaxNominators = MaxNominatorRewardedPerValidator;
    type MaxSetIdSessionEntries = BeefySetIdSessionEntries;
    type WeightInfo = ();
    type KeyOwnerProof = <Historical as KeyOwnerProofSystem<(KeyTypeId, BeefyId)>>::Proof;
    type EquivocationReportSystem =
        pallet_beefy::EquivocationReportSystem<Self, Offences, Historical, ReportLongevity>;
}

#[cfg(feature = "wip")] // Trustless bridges
impl pallet_mmr::Config for Runtime {
    const INDEXING_PREFIX: &'static [u8] = b"mmr";
    type Hashing = Keccak256;
    // type Hash = <Keccak256 as sp_runtime::traits::Hash>::Output;
    type OnNewRoot = pallet_beefy_mmr::DepositBeefyDigest<Runtime>;
    type WeightInfo = ();
    type LeafData = pallet_beefy_mmr::Pallet<Runtime>;
}

impl leaf_provider::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Hashing = Keccak256;
    type Hash = <Keccak256 as sp_runtime::traits::Hash>::Output;
    type Randomness = pallet_babe::RandomnessFromTwoEpochsAgo<Self>;
}

#[cfg(feature = "wip")] // Trustless bridges
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

#[cfg(feature = "wip")] // Trustless bridges
impl pallet_beefy_mmr::Config for Runtime {
    type LeafVersion = LeafVersion;
    type BeefyAuthorityToMerkleLeaf = pallet_beefy_mmr::BeefyEcdsaToEthereum;
    type LeafExtra =
        LeafExtraData<<Self as leaf_provider::Config>::Hash, <Self as frame_system::Config>::Hash>;
    type BeefyDataProvider = leaf_provider::Pallet<Runtime>;
}

parameter_types! {
    pub const CeresPerDay: Balance = balance!(6.66666666667);
    pub const CeresAssetId: AssetId = AssetId32::from_bytes
        (hex!("008bcfd2387d3fc453333557eecb0efe59fcba128769b2feefdd306e98e66440"));
    pub const MaximumCeresInStakingPool: Balance = balance!(14400);
}

impl ceres_launchpad::Config for Runtime {
    const MILLISECONDS_PER_DAY: Moment = 86_400_000;
    type RuntimeEvent = RuntimeEvent;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type WeightInfo = ceres_launchpad::weights::SubstrateWeight<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl ceres_staking::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumber = 1 * DAYS;
    type RuntimeEvent = RuntimeEvent;
    type CeresPerDay = CeresPerDay;
    type CeresAssetId = CeresAssetId;
    type MaximumCeresInStakingPool = MaximumCeresInStakingPool;
    type WeightInfo = ceres_staking::weights::SubstrateWeight<Runtime>;
}

impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumber = 1 * DAYS;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXYK;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ceres_liquidity_locker::weights::SubstrateWeight<Runtime>;
}

impl ceres_token_locker::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CeresAssetId = CeresAssetId;
    type WeightInfo = ceres_token_locker::weights::SubstrateWeight<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl ceres_governance_platform::Config for Runtime {
    type StringLimit = StringLimit;
    type OptionsLimit = OptionsLimit;
    type TitleLimit = TitleLimit;
    type DescriptionLimit = DescriptionLimit;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ceres_governance_platform::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub const DemeterAssetId: AssetId = common::DEMETER_ASSET_ID;
}

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = DemeterAssetId;
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumber = 3 * HOURS / 2;
    type WeightInfo = demeter_farming_platform::weights::SubstrateWeight<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

impl oracle_proxy::Config for Runtime {
    type Symbol = Symbol;
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = oracle_proxy::weights::SubstrateWeight<Runtime>;
    type BandChainOracle = band::Pallet<Runtime>;
}

parameter_types! {
    pub const GetBandRateStalePeriod: Moment = 60*5*1000; // 5 minutes
    pub const GetBandRateStaleBlockPeriod: u32 = 600; // 1 hour in blocks
    pub const BandMaxRelaySymbols: u32 = 100;
}

impl band::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Symbol = Symbol;
    type WeightInfo = band::weights::SubstrateWeight<Runtime>;
    type OnNewSymbolsRelayedHook = oracle_proxy::Pallet<Runtime>;
    type Time = Timestamp;
    type GetBandRateStalePeriod = GetBandRateStalePeriod;
    type GetBandRateStaleBlockPeriod = GetBandRateStaleBlockPeriod;
    type OnSymbolDisabledHook = xst::Pallet<Runtime>;
    type MaxRelaySymbols = BandMaxRelaySymbols;
}

parameter_types! {
    pub const HermesAssetId: AssetId = common::HERMES_ASSET_ID;
    pub const StringLimit: u32 = 64;
    pub const OptionsLimit: u32 = 5;
    pub const TitleLimit: u32 = 128;
    pub const DescriptionLimit: u32 = 4096;
}

impl hermes_governance_platform::Config for Runtime {
    const MIN_DURATION_OF_POLL: Moment = 14_400_000;
    const MAX_DURATION_OF_POLL: Moment = 604_800_000;
    type StringLimit = StringLimit;
    type OptionsLimit = OptionsLimit;
    type RuntimeEvent = RuntimeEvent;
    type HermesAssetId = HermesAssetId;
    type TitleLimit = TitleLimit;
    type DescriptionLimit = DescriptionLimit;
    type WeightInfo = hermes_governance_platform::weights::SubstrateWeight<Runtime>;
    type AssetInfoProvider = assets::Pallet<Runtime>;
}

parameter_types! {
    pub KensetsuTreasuryTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            kensetsu::TECH_ACCOUNT_PREFIX.to_vec(),
            kensetsu::TECH_ACCOUNT_TREASURY_MAIN.to_vec(),
        )
    };
    pub KensetsuTreasuryAccountId: AccountId = {
        let tech_account_id = KensetsuTreasuryTechAccountId::get();
        technical::Pallet::<Runtime>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.")
    };

    pub const KenAssetId: AssetId = common::KEN;
    pub const KarmaAssetId: AssetId = common::KARMA;

    pub GetKenIncentiveRemintPercent: Percent = Percent::from_percent(80);
    pub GetKarmaIncentiveRemintPercent: Percent = Percent::from_percent(80);

    // 1 Kensetsu dollar of uncollected stability fee triggers accrue
    pub const MinimalStabilityFeeAccrue: Balance = balance!(1);

    // Not as important as some essential transactions (e.g. im_online or similar ones)
    pub KensetsuOffchainWorkerTxPriority: TransactionPriority =
        Perbill::from_percent(10) * TransactionPriority::max_value();
    // 10 blocks, if tx spoils, worker will resend it
    pub KensetsuOffchainWorkerTxLongevity: TransactionLongevity = 10;
}

impl kensetsu::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Randomness = pallet_babe::ParentBlockRandomness<Self>;
    type AssetInfoProvider = Assets;
    type PriceTools = PriceTools;
    type LiquidityProxy = LiquidityProxy;
    type Oracle = OracleProxy;
    type TradingPairSourceManager = trading_pair::Pallet<Runtime>;
    type TreasuryTechAccount = KensetsuTreasuryTechAccountId;
    type KenAssetId = KenAssetId;
    type KarmaAssetId = KarmaAssetId;
    type TbcdAssetId = GetTbcdAssetId;
    type KenIncentiveRemintPercent = GetKenIncentiveRemintPercent;
    type KarmaIncentiveRemintPercent = GetKarmaIncentiveRemintPercent;
    type MaxCdpsPerOwner = ConstU32<10000>;
    type MinimalStabilityFeeAccrue = MinimalStabilityFeeAccrue;
    type UnsignedPriority = KensetsuOffchainWorkerTxPriority;
    type UnsignedLongevity = KensetsuOffchainWorkerTxLongevity;
    type WeightInfo = kensetsu::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    pub ApolloOffchainWorkerTxPriority: TransactionPriority =
        Perbill::from_percent(10) * TransactionPriority::max_value();
    pub ApolloOffchainWorkerTxLongevity: TransactionLongevity = 5; // set 100 for release
}

impl apollo_platform::Config for Runtime {
    const BLOCKS_PER_FIFTEEN_MINUTES: BlockNumber = 15 * MINUTES;
    type RuntimeEvent = RuntimeEvent;
    type PriceTools = PriceTools;
    type LiquidityProxyPallet = LiquidityProxy;
    type UnsignedPriority = ApolloOffchainWorkerTxPriority;
    type UnsignedLongevity = ApolloOffchainWorkerTxLongevity;
    type WeightInfo = apollo_platform::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
    // small value for test environment in order to check postponing expirations
    pub ExpirationsSchedulerMaxWeight: Weight = Perbill::from_percent(15) * BlockWeights::get().max_block;
    pub AlignmentSchedulerMaxWeight: Weight = Perbill::from_percent(35) * BlockWeights::get().max_block;
}

impl order_book::Config for Runtime {
    const MAX_ORDER_LIFESPAN: Moment = 30 * (DAYS as Moment) * MILLISECS_PER_BLOCK; // 30 days = 2_592_000_000
    const MIN_ORDER_LIFESPAN: Moment = (MINUTES as Moment) * MILLISECS_PER_BLOCK; // 1 minute = 60_000
    const MILLISECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK;
    const SOFT_MIN_MAX_RATIO: usize = 1000;
    const HARD_MIN_MAX_RATIO: usize = 4000;
    const REGULAR_NUBMER_OF_EXECUTED_ORDERS: usize = 100;
    type RuntimeEvent = RuntimeEvent;
    type OrderId = u128;
    type Locker = OrderBook;
    type Unlocker = OrderBook;
    type Scheduler = OrderBook;
    type Delegate = OrderBook;

    // preferably set this and other vec boundaries to an exponent
    // of 2 because amortized (exponential capacity) growth seems
    // to allocate (next_power_of_two) bytes anyway.
    //
    // or initialize it via `with_capacity` instead.
    //
    // this limit is mostly because of requirement to use bounded vectors.
    // a user can create multiple accounts at any time.
    type MaxOpenedLimitOrdersPerUser = ConstU32<1024>;
    type MaxLimitOrdersForPrice = ConstU32<1024>;
    type MaxSidePriceCount = ConstU32<1024>;
    type MaxExpiringOrdersPerBlock = ConstU32<1024>;
    type MaxExpirationWeightPerBlock = ExpirationsSchedulerMaxWeight;
    type MaxAlignmentWeightPerBlock = AlignmentSchedulerMaxWeight;
    type EnsureTradingPairExists = TradingPair;
    type TradingPairSourceManager = TradingPair;
    type AssetInfoProvider = Assets;
    type SyntheticInfoProvider = XSTPool;
    type DexInfoProvider = DEXManager;
    type Time = Timestamp;
    type PermittedCreateOrigin = EitherOfDiverse<
        EnsureSigned<AccountId>,
        EitherOf<
            pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 1, 2>,
            EnsureRoot<AccountId>,
        >,
    >;
    type PermittedEditOrigin = EitherOf<
        pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 1, 2>,
        EnsureRoot<AccountId>,
    >;
    type WeightInfo = order_book::weights::SubstrateWeight<Runtime>;
}

/// Payload data to be signed when making signed transaction from off-chain workers,
///   inside `create_transaction` function.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;

parameter_types! {
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWeight: u32 = 50;
    pub const BuyBackTBCDPercent: Percent = Percent::from_percent(10);
}

// Ethereum bridge pallets

#[cfg(feature = "wip")] // EVM bridge
impl dispatch::Config<dispatch::Instance1> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OriginOutput =
        bridge_types::types::CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>;
    type Origin = RuntimeOrigin;
    type MessageId = bridge_types::types::MessageId;
    type Hashing = Keccak256;
    type Call = DispatchableSubstrateBridgeCall;
    type CallFilter = SubstrateBridgeCallFilter;
    type WeightInfo = dispatch::weights::SubstrateWeight<Runtime>;
}

#[cfg(feature = "wip")] // EVM bridge
use bridge_types::EVMChainId;

parameter_types! {
    pub const BridgeMaxMessagePayloadSize: u32 = 256;
    pub const BridgeMaxMessagesPerCommit: u32 = 20;
    pub const BridgeMaxTotalGasLimit: u64 = 5_000_000;
    pub const BridgeMaxGasPerMessage: u64 = 5_000_000;
    pub const Decimals: u32 = 12;
}

#[cfg(feature = "wip")] // EVM bridge
pub struct FeeConverter;

#[cfg(feature = "wip")] // EVM bridge
impl Convert<U256, Balance> for FeeConverter {
    fn convert(amount: U256) -> Balance {
        common::eth::unwrap_balance(amount, Decimals::get())
            .expect("Should not panic unless runtime is misconfigured")
    }
}

parameter_types! {
    pub const FeeCurrency: AssetId = XOR;
    pub const ThisNetworkId: bridge_types::GenericNetworkId = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
}

#[cfg(feature = "wip")] // EVM bridge
impl bridge_channel::inbound::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Verifier = MultiVerifier;
    type EVMMessageDispatch = Dispatch;
    type SubstrateMessageDispatch = SubstrateDispatch;
    type WeightInfo = ();
    type ThisNetworkId = ThisNetworkId;
    type UnsignedPriority = DataSignerPriority;
    type UnsignedLongevity = DataSignerLongevity;
    type MaxMessagePayloadSize = BridgeMaxMessagePayloadSize;
    type MaxMessagesPerCommit = BridgeMaxMessagesPerCommit;
    type AssetId = AssetId;
    type Balance = Balance;
    type MessageStatusNotifier = BridgeProxy;
    type OutboundChannel = BridgeOutboundChannel;
    type EVMFeeHandler = EVMFungibleApp;
    type EVMPriorityFee = EVMBridgePriorityFee;
}

#[cfg(feature = "wip")] // EVM bridge
impl bridge_channel::outbound::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MaxMessagePayloadSize = BridgeMaxMessagePayloadSize;
    type MaxMessagesPerCommit = BridgeMaxMessagesPerCommit;
    type MessageStatusNotifier = BridgeProxy;
    type AuxiliaryDigestHandler = LeafProvider;
    type ThisNetworkId = ThisNetworkId;
    type AssetId = AssetId;
    type Balance = Balance;
    type MaxGasPerCommit = BridgeMaxTotalGasLimit;
    type MaxGasPerMessage = BridgeMaxGasPerMessage;
    type TimepointProvider = GenericTimepointProvider;
    type WeightInfo = ();
}

#[cfg(feature = "wip")] // EVM bridge
parameter_types! {
    pub const DescendantsUntilFinalized: u8 = 30;
    pub const VerifyPoW: bool = true;
    // Not as important as some essential transactions (e.g. im_online or similar ones)
    pub EthereumLightClientPriority: TransactionPriority = Perbill::from_percent(10) * TransactionPriority::max_value();
    // We don't want to have not relevant imports be stuck in transaction pool
    // for too long
    pub EthereumLightClientLongevity: TransactionLongevity = EPOCH_DURATION_IN_BLOCKS as u64;
    pub EVMBridgePriorityFee: u128 = 5_000_000_000; // 5 Gwei
}

#[cfg(feature = "wip")] // EVM bridge
parameter_types! {
    pub const BaseFeeLifetime: BlockNumber = 10 * MINUTES;
}

#[cfg(feature = "wip")] // EVM bridge
impl evm_fungible_app::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = BridgeOutboundChannel;
    type CallOrigin = dispatch::EnsureAccount<
        bridge_types::types::CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>,
    >;
    type AppRegistry = BridgeInboundChannel;
    type MessageStatusNotifier = BridgeProxy;
    type AssetRegistry = BridgeProxy;
    type BalancePrecisionConverter = impls::BalancePrecisionConverter;
    type AssetIdConverter = sp_runtime::traits::ConvertInto;
    type BridgeAssetLocker = BridgeProxy;
    type BaseFeeLifetime = BaseFeeLifetime;
    type PriorityFee = EVMBridgePriorityFee;
    type WeightInfo = ();
}

parameter_types! {
    pub const GetReferenceAssetId: AssetId = GetDaiAssetId::get();
    pub const GetReferenceDexId: DEXId = 0;
}

impl bridge_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;

    #[cfg(feature = "wip")] // EVM bridge
    type FAApp = EVMFungibleApp;
    #[cfg(not(feature = "wip"))] // EVM bridge
    type FAApp = ();

    type HashiBridge = EthBridge;
    type ParachainApp = ParachainBridgeApp;

    type LiberlandApp = SubstrateBridgeApp;

    type TimepointProvider = GenericTimepointProvider;
    type ReferencePriceProvider =
        liquidity_proxy::ReferencePriceProvider<Runtime, GetReferenceDexId, GetReferenceAssetId>;
    type ManagerOrigin = EitherOfDiverse<
        pallet_collective::EnsureProportionMoreThan<AccountId, TechnicalCollective, 2, 3>,
        EnsureRoot<AccountId>,
    >;
    type WeightInfo = ();
    type AccountIdConverter = sp_runtime::traits::Identity;
}

#[cfg(feature = "wip")] // Trustless substrate bridge
impl beefy_light_client::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Randomness = pallet_babe::RandomnessFromTwoEpochsAgo<Self>;
}

impl dispatch::Config<dispatch::Instance2> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OriginOutput = bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>;
    type Origin = RuntimeOrigin;
    type MessageId = bridge_types::types::MessageId;
    type Hashing = Keccak256;
    type Call = DispatchableSubstrateBridgeCall;
    type CallFilter = SubstrateBridgeCallFilter;
    type WeightInfo = crate::weights::dispatch::WeightInfo<Runtime>;
}

impl substrate_bridge_channel::inbound::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Verifier = MultiVerifier;
    type MessageDispatch = SubstrateDispatch;
    type UnsignedPriority = DataSignerPriority;
    type UnsignedLongevity = DataSignerLongevity;
    type MaxMessagePayloadSize = BridgeMaxMessagePayloadSize;
    type MaxMessagesPerCommit = BridgeMaxMessagesPerCommit;
    type ThisNetworkId = ThisNetworkId;
    type WeightInfo = crate::weights::substrate_inbound_channel::WeightInfo<Runtime>;
}

pub struct MultiVerifier;

#[derive(Clone, Debug, PartialEq, codec::Encode, codec::Decode, scale_info::TypeInfo)]
pub enum MultiProof {
    #[cfg(feature = "wip")] // Trustless substrate bridge
    #[codec(index = 0)]
    Beefy(<BeefyLightClient as Verifier>::Proof),
    #[codec(index = 1)]
    Multisig(<MultisigVerifier as Verifier>::Proof),
    #[cfg(feature = "wip")] // EVM bridge
    #[codec(index = 2)]
    EVMMultisig(<multisig_verifier::MultiEVMVerifier<Runtime> as Verifier>::Proof),
    /// This proof is only used for benchmarking purposes
    #[cfg(feature = "runtime-benchmarks")]
    #[codec(skip)]
    Empty,
}

impl Verifier for MultiVerifier {
    type Proof = MultiProof;

    fn verify(
        network_id: bridge_types::GenericNetworkId,
        message: H256,
        proof: &Self::Proof,
    ) -> frame_support::pallet_prelude::DispatchResult {
        match proof {
            #[cfg(feature = "wip")] // Trustless substrate bridge
            MultiProof::Beefy(proof) => BeefyLightClient::verify(network_id, message, proof),
            MultiProof::Multisig(proof) => MultisigVerifier::verify(network_id, message, proof),
            #[cfg(feature = "wip")] // EVM bridge
            MultiProof::EVMMultisig(proof) => {
                multisig_verifier::MultiEVMVerifier::<Runtime>::verify(network_id, message, proof)
            }
            #[cfg(feature = "runtime-benchmarks")]
            MultiProof::Empty => Ok(()),
        }
    }

    fn verify_weight(proof: &Self::Proof) -> Weight {
        match proof {
            #[cfg(feature = "wip")] // Trustless substrate bridge
            MultiProof::Beefy(proof) => BeefyLightClient::verify_weight(proof),
            MultiProof::Multisig(proof) => MultisigVerifier::verify_weight(proof),
            #[cfg(feature = "wip")] // EVM bridge
            MultiProof::EVMMultisig(proof) => {
                multisig_verifier::MultiEVMVerifier::<Runtime>::verify_weight(proof)
            }
            #[cfg(feature = "runtime-benchmarks")]
            MultiProof::Empty => Default::default(),
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn valid_proof() -> Option<Self::Proof> {
        Some(MultiProof::Empty)
    }
}

pub struct GenericTimepointProvider;

impl bridge_types::traits::TimepointProvider for GenericTimepointProvider {
    fn get_timepoint() -> bridge_types::GenericTimepoint {
        bridge_types::GenericTimepoint::Sora(System::block_number())
    }
}

impl substrate_bridge_channel::outbound::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type MessageStatusNotifier = BridgeProxy;
    type MaxMessagePayloadSize = BridgeMaxMessagePayloadSize;
    type MaxMessagesPerCommit = BridgeMaxMessagesPerCommit;
    type AuxiliaryDigestHandler = LeafProvider;
    type AssetId = AssetId;
    type Balance = Balance;
    type TimepointProvider = GenericTimepointProvider;
    type ThisNetworkId = ThisNetworkId;
    type WeightInfo = crate::weights::substrate_outbound_channel::WeightInfo<Runtime>;
}

impl parachain_bridge_app::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = SubstrateBridgeOutboundChannel;
    type CallOrigin =
        dispatch::EnsureAccount<bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>>;
    type MessageStatusNotifier = BridgeProxy;
    type AssetRegistry = BridgeProxy;
    type AccountIdConverter = sp_runtime::traits::Identity;
    type AssetIdConverter = sp_runtime::traits::ConvertInto;
    type BalancePrecisionConverter = impls::BalancePrecisionConverter;
    type BridgeAssetLocker = BridgeProxy;
    type WeightInfo = crate::weights::parachain_bridge_app::WeightInfo<Runtime>;
}

impl substrate_bridge_app::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = SubstrateBridgeOutboundChannel;
    type CallOrigin =
        dispatch::EnsureAccount<bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>>;
    type MessageStatusNotifier = BridgeProxy;
    type AssetRegistry = BridgeProxy;
    type AccountIdConverter = impls::LiberlandAccountIdConverter;
    type AssetIdConverter = impls::LiberlandAssetIdConverter;
    type BalancePrecisionConverter = impls::GenericBalancePrecisionConverter;
    type BridgeAssetLocker = BridgeProxy;
    type WeightInfo = crate::weights::substrate_bridge_app::WeightInfo<Runtime>;
}

parameter_types! {
    pub const BridgeMaxPeers: u32 = 50;
    // Not as important as some essential transactions (e.g. im_online or similar ones)
    pub DataSignerPriority: TransactionPriority = Perbill::from_percent(10) * TransactionPriority::max_value();
    // We don't want to have not relevant imports be stuck in transaction pool
    // for too long
    pub DataSignerLongevity: TransactionLongevity = EPOCH_DURATION_IN_BLOCKS as u64;
}

impl bridge_data_signer::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = SubstrateBridgeOutboundChannel;
    type CallOrigin =
        dispatch::EnsureAccount<bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>>;
    type MaxPeers = BridgeMaxPeers;
    type UnsignedPriority = DataSignerPriority;
    type UnsignedLongevity = DataSignerLongevity;
    type WeightInfo = crate::weights::bridge_data_signer::WeightInfo<Runtime>;
}

impl multisig_verifier::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type CallOrigin =
        dispatch::EnsureAccount<bridge_types::types::CallOriginOutput<SubNetworkId, H256, ()>>;
    type OutboundChannel = SubstrateBridgeOutboundChannel;
    type MaxPeers = BridgeMaxPeers;
    type WeightInfo = crate::weights::multisig_verifier::WeightInfo<Runtime>;
    type ThisNetworkId = ThisNetworkId;
}

#[cfg(feature = "ready-to-test")] // DeFi-R
impl extended_assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type AssetInfoProvider = Assets;
    type MaxRegulatedAssetsPerSBT = ConstU32<10000>;
    type WeightInfo = extended_assets::weights::SubstrateWeight<Runtime>;
}

#[cfg(feature = "wip")] // Contracts pallet
parameter_types! {
    pub const DepositPerItem: Balance = deposit(1, 0); // differs
    pub const DepositPerByte: Balance = deposit(0, 1);
    pub Schedule: pallet_contracts::Schedule<Runtime> = Default::default();
    pub const DefaultDepositLimit: Balance = deposit(1024, 1024 * 1024); // deposit(16, 16 * 1024), ConstU128<{ u128::MAX }>
    pub CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(30);

    // For 9.38
    // pub const DeletionQueueDepth: u32 = 128;
    // pub DeletionWeightLimit: Weight = BlockWeights::get()
    //     .per_class
    //     .get(DispatchClass::Normal)
    //     .max_total
    //     .unwrap_or(BlockWeights::get().max_block);
}

// TODO: Decide should we use another deposit for contracts? How to calculate?
/// The slight difference to general `deposit` function is because there is fixed bound on how large
/// the DB key can grow so it doesn't make sense to have as high deposit per item as in the general
/// approach.
// const fn contracts_deposit(items: u32, bytes: u32) -> Balance {
//     items as Balance * 40 * MILLICENTS + (bytes as Balance) * MILLICENTS
// }

#[cfg(feature = "wip")] // Contracts pallet
impl pallet_contracts::Config for Runtime {
    // type DeletionQueueDepth = DeletionQueueDepth;
    // type DeletionWeightLimit = DeletionWeightLimit;

    type Time = Timestamp;
    type Randomness = RandomnessCollectiveFlip;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type CallFilter = impls::ContractsCallFilter;
    type WeightPrice = pallet_transaction_payment::Pallet<Self>;
    type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
    type ChainExtension = ();
    type Schedule = Schedule;
    type CallStack = [pallet_contracts::Frame<Self>; 5];
    type DepositPerByte = DepositPerByte;
    // For 1.1.0
    type DefaultDepositLimit = DefaultDepositLimit;
    type DepositPerItem = DepositPerItem;
    type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
    type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
    // Got from `pallet_contracts::integrity_test`
    // For every contract executed in runtime, at least `MaxCodeLen*18*4` memory should be available
    // `(MaxCodeLen * 18 * 4 + MAX_STACK_SIZE + max_heap_size) * max_call_depth <
    // MAX_RUNTIME_MEM/2`
    // `(MaxCodeLen * 72 + 1MB + 1MB) * 6 = 64MB`
    // `MaxCodeLen = (64MB/6 - 2) / 72 = 26/216 = 13/108 MB ≈ 123 * 1024 < 13/108 MB`
    type MaxCodeLen = ConstU32<{ 123 * 1024 }>;
    type MaxStorageKeyLen = ConstU32<128>;
    type MaxDelegateDependencies = ConstU32<32>;
    type UnsafeUnstableInterface = ConstBool<false>;
    type MaxDebugBufferLen = ConstU32<{ 2 * 1024 * 1024 }>;
    type RuntimeHoldReason = RuntimeHoldReason;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type Migrations = ();
    #[cfg(feature = "runtime-benchmarks")]
    type Migrations = pallet_contracts::migration::codegen::BenchMigrations;
    type Debug = ();
    type Environment = ();
}
construct_runtime! {
    pub enum Runtime {
        System: frame_system = 0,

        Babe: pallet_babe = 14,

        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent} = 1,
        // Balances in native currency - XOR.
        Balances: pallet_balances::{Pallet, Storage, Config<T>, Event<T>} = 2,
        RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Pallet, Storage} = 4,
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>} = 5,
        Permissions: permissions::{Pallet, Call, Storage, Config<T>, Event<T>} = 6,
        Referrals: referrals::{Pallet, Call, Storage} = 7,
        Rewards: rewards::{Pallet, Call, Config<T>, Storage, Event<T>} = 8,
        XorFee: xor_fee::{Pallet, Call, Storage, Event<T>} = 9,
        BridgeMultisig: bridge_multisig::{Pallet, Call, Storage, Config<T>, Event<T>} = 10,
        Utility: pallet_utility::{Pallet, Call, Event} = 11,

        // Consensus and staking.
        Authorship: pallet_authorship::{Pallet, Storage} = 16,
        Staking: pallet_staking = 17,
        Offences: pallet_offences::{Pallet, Storage, Event} = 37,
        Historical: pallet_session_historical::{Pallet} = 13,
        Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>} = 12,
        Grandpa: pallet_grandpa = 15,
        ImOnline: pallet_im_online::{Pallet, Call, Storage, Event<T>, ValidateUnsigned, Config<T>} = 36,

        // Non-native tokens - everything apart of XOR.
        Tokens: tokens::{Pallet, Storage, Config<T>, Event<T>} = 18,
        // Unified interface for XOR and non-native tokens.
        Currencies: currencies::{Pallet} = 19,
        TradingPair: trading_pair::{Pallet, Call, Storage, Config<T>, Event<T>} = 20,
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>} = 21,
        DEXManager: dex_manager::{Pallet, Storage, Config<T>} = 22,
        MulticollateralBondingCurvePool: multicollateral_bonding_curve_pool::{Pallet, Call, Storage, Config<T>, Event<T>} = 23,
        Technical: technical::{Pallet, Call, Config<T>, Event<T>, Storage} = 24,
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>} = 25,
        LiquidityProxy: liquidity_proxy::{Pallet, Call, Event<T>} = 26,
        Council: pallet_collective::<Instance1>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 27,
        TechnicalCommittee: pallet_collective::<Instance2>::{Pallet, Call, Storage, Origin<T>, Event<T>, Config<T>} = 28,
        Democracy: pallet_democracy::{Pallet, Call, Storage, Config<T>, Event<T>} = 29,
        DEXAPI: dex_api = 30,
        EthBridge: eth_bridge::{Pallet, Call, Storage, Config<T>, Event<T>} = 31,
        PswapDistribution: pswap_distribution::{Pallet, Call, Storage, Config<T>, Event<T>} = 32,
        Multisig: pallet_multisig::{Pallet, Call, Storage, Event<T>} = 33,
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>} = 34,
        IrohaMigration: iroha_migration::{Pallet, Call, Storage, Config<T>, Event<T>} = 35,
        TechnicalMembership: pallet_membership::<Instance1>::{Pallet, Call, Storage, Event<T>, Config<T>} = 38,
        ElectionsPhragmen: pallet_elections_phragmen::{Pallet, Call, Storage, Event<T>, Config<T>} = 39,
        VestedRewards: vested_rewards::{Pallet, Call, Storage, Event<T>} = 40,
        Identity: pallet_identity::{Pallet, Call, Storage, Event<T>} = 41,
        Farming: farming::{Pallet, Call, Storage, Event<T>} = 42,
        XSTPool: xst::{Pallet, Call, Storage, Config<T>, Event<T>} = 43,
        PriceTools: price_tools::{Pallet, Storage, Event<T>} = 44,
        CeresStaking: ceres_staking::{Pallet, Call, Storage, Event<T>} = 45,
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>} = 46,
        CeresTokenLocker: ceres_token_locker::{Pallet, Call, Storage, Event<T>} = 47,
        CeresGovernancePlatform: ceres_governance_platform::{Pallet, Call, Storage, Event<T>} = 48,
        CeresLaunchpad: ceres_launchpad::{Pallet, Call, Storage, Event<T>} = 49,
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>} = 50,
        // Provides a semi-sorted list of nominators for staking.
        BagsList: pallet_bags_list::{Pallet, Call, Storage, Event<T>} = 51,
        ElectionProviderMultiPhase: pallet_election_provider_multi_phase::{Pallet, Call, Storage, Event<T>, ValidateUnsigned} = 52,
        Band: band::{Pallet, Call, Storage, Event<T>} = 53,
        OracleProxy: oracle_proxy::{Pallet, Call, Storage, Event<T>} = 54,
        HermesGovernancePlatform: hermes_governance_platform::{Pallet, Call, Storage, Event<T>} = 55,
        Preimage: pallet_preimage::{Pallet, Call, Storage, Event<T>} = 56,
        OrderBook: order_book::{Pallet, Call, Storage, Event<T>} = 57,
        Kensetsu: kensetsu::{Pallet, Call, Storage, Config<T>, Event<T>, ValidateUnsigned} = 58,

        // Leaf provider should be placed before any pallet which is uses it
        LeafProvider: leaf_provider::{Pallet, Storage, Event<T>} = 99,

        // Generic bridges pallets
        BridgeProxy: bridge_proxy::{Pallet, Call, Storage, Event} = 103,

        // Trustless EVM bridge
        #[cfg(feature = "wip")] // EVM bridge
        BridgeInboundChannel: bridge_channel::inbound::{Pallet, Call, Storage, Event<T>, ValidateUnsigned} = 96,
        #[cfg(feature = "wip")] // EVM bridge
        BridgeOutboundChannel: bridge_channel::outbound::{Pallet, Config<T>, Storage, Event<T>} = 97,
        #[cfg(feature = "wip")] // EVM bridge
        Dispatch: dispatch::<Instance1>::{Pallet, Storage, Event<T>, Origin<T>} = 98,
        #[cfg(feature = "wip")] // EVM bridge
        EVMFungibleApp: evm_fungible_app::{Pallet, Call, Storage, Event<T>, Config<T>} = 100,

        // Trustless substrate bridge
        #[cfg(feature = "wip")] // Trustless substrate bridge
        BeefyLightClient: beefy_light_client = 104,

        // Federated substrate bridge
        SubstrateBridgeInboundChannel: substrate_bridge_channel::inbound::{Pallet, Call, Storage, Event<T>, ValidateUnsigned} = 106,
        SubstrateBridgeOutboundChannel: substrate_bridge_channel::outbound::{Pallet, Call, Config<T>, Storage, Event<T>} = 107,
        SubstrateDispatch: dispatch::<Instance2>::{Pallet, Storage, Event<T>, Origin<T>} = 108,
        ParachainBridgeApp: parachain_bridge_app::{Pallet, Config<T>, Storage, Event<T>, Call} = 109,
        BridgeDataSigner: bridge_data_signer::{Pallet, Storage, Event<T>, Call, ValidateUnsigned} = 110,
        MultisigVerifier: multisig_verifier::{Pallet, Storage, Event<T>, Call} = 111,

        SubstrateBridgeApp: substrate_bridge_app::{Pallet, Storage, Event<T>, Call} = 113,

        // Trustless bridges
        // Beefy pallets should be placed after channels
        #[cfg(feature = "wip")] // Trustless bridges
        Mmr: pallet_mmr::{Pallet, Storage} = 90,
        // In production needed for session keys
        Beefy: pallet_beefy = 91,
        #[cfg(feature = "wip")] // Trustless bridges
        MmrLeaf: pallet_beefy_mmr::{Pallet, Storage} = 92,

        // Dev
        #[cfg(feature = "private-net")]
        Sudo: pallet_sudo::{Pallet, Call, Storage, Config<T>, Event<T>} = 3,

        // Available only for test net
        #[cfg(feature = "private-net")]
        Faucet: faucet::{Pallet, Call, Config<T>, Event<T>} = 80,
        #[cfg(feature = "private-net")]
        QaTools: qa_tools::{Pallet, Call, Event<T>} = 112,

        ApolloPlatform: apollo_platform::{Pallet, Call, Storage, Event<T>, ValidateUnsigned} = 114,
        #[cfg(feature = "ready-to-test")] // DeFi-R
        ExtendedAssets: extended_assets::{Pallet, Call, Storage, Event<T>} = 115,
        // Ink! contracts
        #[cfg(feature = "wip")] // Contracts pallet
        Contracts: pallet_contracts::{Pallet, Call, Storage, Event<T>, HoldReason} = 116
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
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    migrations::Migrations,
>;

#[cfg(feature = "wip")] // Trustless bridges
pub type MmrHashing = <Runtime as pallet_mmr::Config>::Hashing;

impl_runtime_apis! {

    // impl pallet_contracts::ContractsApi<Block, AccountId, Balance, BlockNumber, Hash> for Runtime {
    //     fn call(
    //         origin: AccountId,
    //         dest: AccountId,
    //         value: Balance,
    //         gas_limit: Option<Weight>,
    //         storage_deposit_limit: Option<Balance>,
    //         input_data: Vec<u8>,
    //     ) -> pallet_contracts_primitives::ContractExecResult<Balance> {
    //         let gas_limit = gas_limit.unwrap_or(BlockWeights::get().max_block);
    //         Contracts::bare_call(
    //             origin,
    //             dest,
    //             value,
    //             gas_limit,
    //             storage_deposit_limit,
    //             input_data,
    //             false,
    //             pallet_contracts::Determinism::Deterministic
    //         )
    //     }
    //
    //     fn instantiate(
    //         origin: AccountId,
    //         value: Balance,
    //         gas_limit: Option<Weight>,
    //         storage_deposit_limit: Option<Balance>,
    //         code: pallet_contracts_primitives::Code<Hash>,
    //         data: Vec<u8>,
    //         salt: Vec<u8>,
    //     ) -> pallet_contracts_primitives::ContractInstantiateResult<AccountId, Balance> {
    //         let gas_limit = gas_limit.unwrap_or(BlockWeights::get().max_block);
    //         Contracts::bare_instantiate(
    //             origin,
    //             value,
    //             gas_limit,
    //             storage_deposit_limit,
    //             code,
    //             data,
    //             salt,
    //             false
    //         )
    //     }
    //     fn upload_code(
    //         origin: AccountId,
    //         code: Vec<u8>,
    //         storage_deposit_limit: Option<Balance>,
    //         determinism: pallet_contracts::Determinism,
    //     ) -> pallet_contracts_primitives::CodeUploadResult<Hash, Balance> {
    //         Contracts::bare_upload_code(
    //             origin,
    //             code,
    //             storage_deposit_limit,
    //             determinism,
    //         )
    //     }
    //
    //     fn get_storage(
    //         address: AccountId,
    //         key: Vec<u8>,
    //     ) -> pallet_contracts_primitives::GetStorageResult {
    //         Contracts::get_storage(
    //             address,
    //             key
    //         )
    //     }
    // }

    // For 1.1.0
    #[cfg(feature = "wip")] // Contracts pallet
    impl pallet_contracts::ContractsApi<Block, AccountId, Balance, BlockNumber, Hash, EventRecord> for Runtime
    {
        fn call(
            origin: AccountId,
            dest: AccountId,
            value: Balance,
            gas_limit: Option<Weight>,
            storage_deposit_limit: Option<Balance>,
            input_data: Vec<u8>,
        ) -> pallet_contracts_primitives::ContractExecResult<Balance, EventRecord> {
            let gas_limit = gas_limit.unwrap_or(BlockWeights::get().max_block);
            Contracts::bare_call(
                origin,
                dest,
                value,
                gas_limit,
                storage_deposit_limit,
                input_data,
                pallet_contracts::DebugInfo::UnsafeDebug,
                pallet_contracts::CollectEvents::UnsafeCollect,
                pallet_contracts::Determinism::Enforced,
            )
        }

        fn instantiate(
            origin: AccountId,
            value: Balance,
            gas_limit: Option<Weight>,
            storage_deposit_limit: Option<Balance>,
            code: pallet_contracts_primitives::Code<Hash>,
            data: Vec<u8>,
            salt: Vec<u8>,
        ) -> pallet_contracts_primitives::ContractInstantiateResult<AccountId, Balance, EventRecord>
        {
            let gas_limit = gas_limit.unwrap_or(BlockWeights::get().max_block);
            Contracts::bare_instantiate(
                origin,
                value,
                gas_limit,
                storage_deposit_limit,
                code,
                data,
                salt,
                pallet_contracts::DebugInfo::UnsafeDebug,
                pallet_contracts::CollectEvents::UnsafeCollect,
            )
        }

        fn upload_code(
            origin: AccountId,
            code: Vec<u8>,
            storage_deposit_limit: Option<Balance>,
            determinism: pallet_contracts::Determinism,
        ) -> pallet_contracts_primitives::CodeUploadResult<Hash, Balance>
        {
            Contracts::bare_upload_code(
                origin,
                code,
                storage_deposit_limit,
                determinism,
            )
        }

        fn get_storage(
            address: AccountId,
            key: Vec<u8>,
        ) -> pallet_contracts_primitives::GetStorageResult {
            Contracts::get_storage(
                address,
                key
            )
        }
    }

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

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> sp_std::vec::Vec<u32> {
            Runtime::metadata_versions()
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
            let call = &uxt.function;
            XorFee::query_info(&uxt, call, len)
        }

        fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
            let call = &uxt.function;
            XorFee::query_fee_details(&uxt, call, len)
        }

        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }

        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
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
        ) -> Option<dex_runtime_api::SwapOutcomeInfo<Balance, AssetId>> {
            #[cfg(feature = "private-net")]
            {
                DEXAPI::quote(
                    &LiquiditySourceId::new(dex_id, liquidity_source_type),
                    &input_asset_id,
                    &output_asset_id,
                    QuoteAmount::with_variant(swap_variant, desired_input_amount.into()),
                    true,
                ).ok().map(|(sa, _)| dex_runtime_api::SwapOutcomeInfo::<Balance, AssetId> { amount: sa.amount, fee: sa.fee})
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

    impl assets_runtime_api::AssetsAPI<Block, AccountId, AssetId, Balance, AssetSymbol, AssetName, BalancePrecision, ContentSource, Description> for Runtime {
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

        fn list_asset_infos() -> Vec<assets_runtime_api::AssetInfo<AssetId, AssetSymbol, AssetName, u8, ContentSource, Description>> {
            Assets::list_registered_asset_infos().into_iter().map(|(asset_id, symbol, name, precision, is_mintable, content_source, description)|
                assets_runtime_api::AssetInfo::<AssetId, AssetSymbol, AssetName, BalancePrecision, ContentSource, Description> {
                    asset_id,
                    symbol,
                    name,
                    precision,
                    is_mintable,
                    content_source,
                    description
                }
            ).collect()
        }

        fn get_asset_info(asset_id: AssetId) -> Option<assets_runtime_api::AssetInfo<AssetId, AssetSymbol, AssetName, BalancePrecision, ContentSource, Description>> {
            let (symbol, name, precision, is_mintable, content_source, description) = Assets::get_asset_info(&asset_id);
            Some(assets_runtime_api::AssetInfo::<AssetId, AssetSymbol, AssetName, BalancePrecision, ContentSource, Description> {
                asset_id,
                symbol,
                name,
                precision,
                is_mintable,
                content_source,
                description
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

    #[cfg(feature = "wip")] // Trustless substrate bridge
    impl beefy_light_client_runtime_api::BeefyLightClientAPI<Block, beefy_light_client::BitField> for Runtime {
        fn get_random_bitfield(network_id: SubNetworkId, prior: beefy_light_client::BitField, num_of_validators: u32) -> beefy_light_client::BitField {
            let len = prior.len() as usize;
            BeefyLightClient::create_random_bit_field(network_id, prior, num_of_validators).unwrap_or(beefy_light_client::BitField::with_capacity(len))
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
                dex_id,
                &input_asset_id,
                &output_asset_id,
                QuoteAmount::with_variant(swap_variant, amount.into()),
                LiquiditySourceFilter::with_mode(dex_id, filter_mode, selected_source_types),
                false,
                true,
            ).ok().map(|(quote_info, _)| liquidity_proxy_runtime_api::SwapOutcomeInfo::<Balance, AssetId> {
                amount: quote_info.outcome.amount,
                amount_without_impact: quote_info.amount_without_impact.unwrap_or(0),
                fee: quote_info.outcome.fee,
                rewards: quote_info.rewards.into_iter()
                                .map(|(amount, currency, reason)| liquidity_proxy_runtime_api::RewardsInfo::<Balance, AssetId> {
                                    amount,
                                    currency,
                                    reason
                                }).collect(),
                route: quote_info.path
                })
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

    impl oracle_proxy_runtime_api::OracleProxyAPI<
        Block,
        Symbol,
        ResolveTime
    > for Runtime {
        fn quote(symbol: Symbol) -> Result<Option<oracle_proxy_runtime_api::RateInfo>, DispatchError>  {
            let rate_wrapped = <
                OracleProxy as common::DataFeed<Symbol, common::Rate, ResolveTime>
            >::quote(&symbol);
            match rate_wrapped {
                Ok(rate) => Ok(rate.map(|rate| oracle_proxy_runtime_api::RateInfo{
                    value: rate.value,
                    last_updated: rate.last_updated
                })),
                Err(e) => Err(e)
            }
        }

        fn list_enabled_symbols() -> Result<Vec<(Symbol, ResolveTime)>, DispatchError> {
            <
                OracleProxy as common::DataFeed<Symbol, common::Rate, ResolveTime>
            >::list_enabled_symbols()
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
            fn configuration() -> sp_consensus_babe::BabeConfiguration {
                    // The choice of `c` parameter (where `1 - c` represents the
                    // probability of a slot being empty), is done in accordance to the
                    // slot duration and expected target block time, for safely
                    // resisting network delays of maximum two seconds.
                    // <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
                    sp_consensus_babe::BabeConfiguration {
                            slot_duration: Babe::slot_duration(),
                            epoch_length: EpochDuration::get(),
                            c: PRIMARY_PROBABILITY,
                            authorities: Babe::authorities().to_vec(),
                            randomness: Babe::randomness(),
                            allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryVRFSlots,
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

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    // For BEEFY gadget
    impl sp_beefy::BeefyApi<Block, BeefyId> for Runtime {
        fn validator_set() -> Option<sp_beefy::ValidatorSet<BeefyId>> {
            #[cfg(not(feature = "wip"))] // Trustless bridges
            return None;

            #[cfg(feature = "wip")] // Trustless bridges
            Beefy::validator_set()
        }

        fn beefy_genesis() -> Option<BlockNumber> {
            Beefy::genesis_block()
        }

        fn submit_report_equivocation_unsigned_extrinsic(
            equivocation_proof: sp_beefy::EquivocationProof<
                BlockNumber,
                BeefyId,
                BeefySignature,
            >,
            key_owner_proof: sp_beefy::OpaqueKeyOwnershipProof,
        ) -> Option<()> {
            let key_owner_proof = key_owner_proof.decode()?;

            Beefy::submit_unsigned_equivocation_report(
                equivocation_proof,
                key_owner_proof,
            )
        }

        fn generate_key_ownership_proof(
            _set_id: sp_beefy::ValidatorSetId,
            authority_id: BeefyId,
        ) -> Option<sp_beefy::OpaqueKeyOwnershipProof> {
            use codec::Encode;

            Historical::prove((sp_beefy::KEY_TYPE, authority_id))
                .map(|p| p.encode())
                .map(sp_beefy::OpaqueKeyOwnershipProof::new)
        }
    }

    impl mmr::MmrApi<Block, Hash, BlockNumber> for Runtime {
        fn mmr_root() -> Result<Hash, mmr::Error> {
            #[cfg(not(feature = "wip"))] // Trustless bridges
            return Err(mmr::Error::PalletNotIncluded);

            #[cfg(feature = "wip")] // Trustless bridges
            Ok(Mmr::mmr_root())
        }

        fn mmr_leaf_count() -> Result<mmr::LeafIndex, mmr::Error> {
            #[cfg(not(feature = "wip"))] // Trustless bridges
            return Err(mmr::Error::PalletNotIncluded);

            #[cfg(feature = "wip")] // Trustless bridges
            Ok(Mmr::mmr_leaves())
        }

        fn generate_proof(
            _block_numbers: Vec<BlockNumber>,
            _best_known_block_number: Option<BlockNumber>,
        ) -> Result<(Vec<mmr::EncodableOpaqueLeaf>, mmr::Proof<Hash>), mmr::Error> {
            #[cfg(not(feature = "wip"))] // Trustless bridges
            return Err(mmr::Error::PalletNotIncluded);

            #[cfg(feature = "wip")] // Trustless bridges
            Mmr::generate_proof(_block_numbers, _best_known_block_number).map(
                |(leaves, proof)| {
                    (
                        leaves
                            .into_iter()
                            .map(|leaf| mmr::EncodableOpaqueLeaf::from_leaf(&leaf))
                            .collect(),
                        proof,
                    )
                },
            )
        }

        fn verify_proof(_leaves: Vec<mmr::EncodableOpaqueLeaf>, _proof: mmr::Proof<Hash>)
            -> Result<(), mmr::Error>
        {
            #[cfg(not(feature = "wip"))] // Trustless bridges
            return Err(mmr::Error::PalletNotIncluded);

            #[cfg(feature = "wip")] // Trustless bridges
            {
                pub type MmrLeaf = <<Runtime as pallet_mmr::Config>::LeafData as mmr::LeafDataProvider>::LeafData;
                let leaves = _leaves.into_iter().map(|leaf|
                    leaf.into_opaque_leaf()
                    .try_decode()
                    .ok_or(mmr::Error::Verify)).collect::<Result<Vec<MmrLeaf>, mmr::Error>>()?;
                Mmr::verify_leaves(leaves, _proof)
            }
        }

        fn verify_proof_stateless(
            _root: Hash,
            _leaves: Vec<mmr::EncodableOpaqueLeaf>,
            _proof: mmr::Proof<Hash>
        ) -> Result<(), mmr::Error> {
            #[cfg(not(feature = "wip"))] // Trustless bridges
            return Err(mmr::Error::PalletNotIncluded);

            #[cfg(feature = "wip")] // Trustless bridges
            {
                let nodes = _leaves.into_iter().map(|leaf|mmr::DataOrHash::Data(leaf.into_opaque_leaf())).collect();
                pallet_mmr::verify_leaves_proof::<MmrHashing, _>(_root, nodes, _proof)
            }
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

    impl leaf_provider_runtime_api::LeafProviderAPI<Block> for Runtime {
        fn latest_digest() -> Option<bridge_types::types::AuxiliaryDigest> {
                LeafProvider::latest_digest().map(|logs| bridge_types::types::AuxiliaryDigest{ logs })
        }

    }

    impl bridge_proxy_runtime_api::BridgeProxyAPI<Block, AssetId> for Runtime {
        fn list_apps() -> Vec<bridge_types::types::BridgeAppInfo> {
            BridgeProxy::list_apps()
        }

        fn list_supported_assets(network_id: bridge_types::GenericNetworkId) -> Vec<bridge_types::types::BridgeAssetInfo> {
            BridgeProxy::list_supported_assets(network_id)
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
            use kensetsu_benchmarking::Pallet as KensetsuBench;
            use liquidity_proxy_benchmarking::Pallet as LiquidityProxyBench;
            use pool_xyk_benchmarking::Pallet as XYKPoolBench;
            use pswap_distribution_benchmarking::Pallet as PswapDistributionBench;
            use ceres_liquidity_locker_benchmarking::Pallet as CeresLiquidityLockerBench;
            use demeter_farming_platform_benchmarking::Pallet as DemeterFarmingPlatformBench;
            use xst_benchmarking::Pallet as XSTPoolBench;
            use order_book_benchmarking::Pallet as OrderBookBench;

            let mut list = Vec::<BenchmarkList>::new();

            list_benchmark!(list, extra, assets, Assets);
            #[cfg(feature = "private-net")]
            list_benchmark!(list, extra, faucet, Faucet);
            list_benchmark!(list, extra, farming, Farming);
            list_benchmark!(list, extra, iroha_migration, IrohaMigration);
            list_benchmark!(list, extra, dex_api, DEXAPI);
            list_benchmark!(list, extra, kensetsu, KensetsuBench::<Runtime>);
            list_benchmark!(list, extra, liquidity_proxy, LiquidityProxyBench::<Runtime>);
            list_benchmark!(list, extra, multicollateral_bonding_curve_pool, MulticollateralBondingCurvePool);
            list_benchmark!(list, extra, pswap_distribution, PswapDistributionBench::<Runtime>);
            list_benchmark!(list, extra, rewards, Rewards);
            list_benchmark!(list, extra, trading_pair, TradingPair);
            list_benchmark!(list, extra, pool_xyk, XYKPoolBench::<Runtime>);
            list_benchmark!(list, extra, eth_bridge, EthBridge);
            list_benchmark!(list, extra, vested_rewards, VestedRewards);
            list_benchmark!(list, extra, price_tools, PriceTools);
            list_benchmark!(list, extra, xor_fee, XorFee);
            list_benchmark!(list, extra, referrals, Referrals);
            list_benchmark!(list, extra, ceres_staking, CeresStaking);
            list_benchmark!(list, extra, hermes_governance_platform, HermesGovernancePlatform);
            list_benchmark!(list, extra, ceres_liquidity_locker, CeresLiquidityLockerBench::<Runtime>);
            list_benchmark!(list, extra, ceres_token_locker, CeresTokenLocker);
            list_benchmark!(list, extra, ceres_governance_platform, CeresGovernancePlatform);
            list_benchmark!(list, extra, ceres_launchpad, CeresLaunchpad);
            list_benchmark!(list, extra, demeter_farming_platform, DemeterFarmingPlatformBench::<Runtime>);
            list_benchmark!(list, extra, band, Band);
            list_benchmark!(list, extra, xst, XSTPoolBench::<Runtime>);
            list_benchmark!(list, extra, oracle_proxy, OracleProxy);
            list_benchmark!(list, extra, apollo_platform, ApolloPlatform);
            list_benchmark!(list, extra, order_book, OrderBookBench::<Runtime>);

            // Trustless bridge
            #[cfg(feature = "wip")] // EVM bridge
            list_benchmark!(list, extra, bridge_inbound_channel, BridgeInboundChannel);
            #[cfg(feature = "wip")] // EVM bridge
            list_benchmark!(list, extra, bridge_outbound_channel, BridgeOutboundChannel);
            #[cfg(feature = "wip")] // EVM bridge
            list_benchmark!(list, extra, evm_fungible_app, EVMFungibleApp);

            list_benchmark!(list, extra, evm_bridge_proxy, BridgeProxy);
            // Dispatch pallet benchmarks is strictly linked to EVM bridge params
            // TODO: fix
            #[cfg(feature = "wip")] // EVM bridge
            list_benchmark!(list, extra, dispatch, Dispatch);
            list_benchmark!(list, extra, substrate_bridge_channel::inbound, SubstrateBridgeInboundChannel);
            list_benchmark!(list, extra, substrate_bridge_channel::outbound, SubstrateBridgeOutboundChannel);
            list_benchmark!(list, extra, parachain_bridge_app, ParachainBridgeApp);
            list_benchmark!(list, extra, substrate_bridge_app, SubstrateBridgeApp);
            list_benchmark!(list, extra, bridge_data_signer, BridgeDataSigner);
            list_benchmark!(list, extra, multisig_verifier, MultisigVerifier);
            #[cfg(feature = "ready-to-test")] // DeFi-R
            list_benchmark!(list, extra, extended_assets, ExtendedAssets);

            let storage_info = AllPalletsWithSystem::storage_info();

            return (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark};
            use frame_support::traits::TrackedStorageKey;
            use kensetsu_benchmarking::Pallet as KensetsuBench;
            use liquidity_proxy_benchmarking::Pallet as LiquidityProxyBench;
            use pool_xyk_benchmarking::Pallet as XYKPoolBench;
            use pswap_distribution_benchmarking::Pallet as PswapDistributionBench;
            use ceres_liquidity_locker_benchmarking::Pallet as CeresLiquidityLockerBench;
            use demeter_farming_platform_benchmarking::Pallet as DemeterFarmingPlatformBench;
            use xst_benchmarking::Pallet as XSTPoolBench;
            use order_book_benchmarking::Pallet as OrderBookBench;

            impl kensetsu_benchmarking::Config for Runtime {}
            impl liquidity_proxy_benchmarking::Config for Runtime {}
            impl pool_xyk_benchmarking::Config for Runtime {}
            impl pswap_distribution_benchmarking::Config for Runtime {}
            impl ceres_liquidity_locker_benchmarking::Config for Runtime {}
            impl xst_benchmarking::Config for Runtime {}
            impl order_book_benchmarking::Config for Runtime {}

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
            #[cfg(feature = "private-net")]
            add_benchmark!(params, batches, faucet, Faucet);
            add_benchmark!(params, batches, farming, Farming);
            add_benchmark!(params, batches, iroha_migration, IrohaMigration);
            add_benchmark!(params, batches, dex_api, DEXAPI);
            add_benchmark!(params, batches, kensetsu, KensetsuBench::<Runtime>);
            add_benchmark!(params, batches, liquidity_proxy, LiquidityProxyBench::<Runtime>);
            add_benchmark!(params, batches, multicollateral_bonding_curve_pool, MulticollateralBondingCurvePool);
            add_benchmark!(params, batches, pswap_distribution, PswapDistributionBench::<Runtime>);
            add_benchmark!(params, batches, rewards, Rewards);
            add_benchmark!(params, batches, trading_pair, TradingPair);
            add_benchmark!(params, batches, pool_xyk, XYKPoolBench::<Runtime>);
            add_benchmark!(params, batches, eth_bridge, EthBridge);
            add_benchmark!(params, batches, vested_rewards, VestedRewards);
            add_benchmark!(params, batches, price_tools, PriceTools);
            add_benchmark!(params, batches, xor_fee, XorFee);
            add_benchmark!(params, batches, referrals, Referrals);
            add_benchmark!(params, batches, ceres_staking, CeresStaking);
            add_benchmark!(params, batches, ceres_liquidity_locker, CeresLiquidityLockerBench::<Runtime>);
            add_benchmark!(params, batches, ceres_token_locker, CeresTokenLocker);
            add_benchmark!(params, batches, ceres_governance_platform, CeresGovernancePlatform);
            add_benchmark!(params, batches, ceres_launchpad, CeresLaunchpad);
            add_benchmark!(params, batches, demeter_farming_platform, DemeterFarmingPlatformBench::<Runtime>);
            add_benchmark!(params, batches, band, Band);
            add_benchmark!(params, batches, xst, XSTPoolBench::<Runtime>);
            add_benchmark!(params, batches, hermes_governance_platform, HermesGovernancePlatform);
            add_benchmark!(params, batches, oracle_proxy, OracleProxy);
            add_benchmark!(params, batches, apollo_platform, ApolloPlatform);
            add_benchmark!(params, batches, order_book, OrderBookBench::<Runtime>);

            // Trustless bridge
            #[cfg(feature = "wip")] // EVM bridge
            add_benchmark!(params, batches, bridge_inbound_channel, BridgeInboundChannel);
            #[cfg(feature = "wip")] // EVM bridge
            add_benchmark!(params, batches, bridge_outbound_channel, BridgeOutboundChannel);
            #[cfg(feature = "wip")] // EVM bridge
            add_benchmark!(params, batches, evm_fungible_app, EVMFungibleApp);

            add_benchmark!(params, batches, evm_bridge_proxy, BridgeProxy);
            // Dispatch pallet benchmarks is strictly linked to EVM bridge params
            // TODO: fix
            #[cfg(feature = "wip")] // EVM bridge
            add_benchmark!(params, batches, dispatch, Dispatch);
            add_benchmark!(params, batches, substrate_bridge_channel::inbound, SubstrateBridgeInboundChannel);
            add_benchmark!(params, batches, substrate_bridge_channel::outbound, SubstrateBridgeOutboundChannel);
            add_benchmark!(params, batches, parachain_bridge_app, ParachainBridgeApp);
            add_benchmark!(params, batches, substrate_bridge_app, SubstrateBridgeApp);
            add_benchmark!(params, batches, bridge_data_signer, BridgeDataSigner);
            add_benchmark!(params, batches, multisig_verifier, MultisigVerifier);
            #[cfg(feature = "ready-to-test")] // DeFi-R
            add_benchmark!(params, batches, extended_assets, ExtendedAssets);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }

    impl vested_rewards_runtime_api::VestedRewardsApi<Block, AccountId, AssetId, Balance, CrowdloanTag> for Runtime {
        fn crowdloan_claimable(tag: CrowdloanTag, account_id: AccountId, asset_id: AssetId) -> Option<vested_rewards_runtime_api::BalanceInfo<Balance>> {
            let balance = VestedRewards::get_claimable_crowdloan_reward(&tag, &account_id, &asset_id)?;
            Some(vested_rewards_runtime_api::BalanceInfo::<Balance> {
                balance
            })
        }

        fn crowdloan_lease(tag: CrowdloanTag) -> Option<vested_rewards_runtime_api::CrowdloanLease> {
            let crowdloan_info = vested_rewards::CrowdloanInfos::<Runtime>::get(&tag)?;

            Some(vested_rewards_runtime_api::CrowdloanLease {
                start_block: crowdloan_info.start_block as u128,
                total_days: crowdloan_info.length as u128 / DAYS as u128,
                blocks_per_day: DAYS as u128,
            })
        }
    }

    impl farming_runtime_api::FarmingApi<Block, AssetId> for Runtime {
        fn reward_doubling_assets() -> Vec<AssetId> {
            Farming::reward_doubling_assets()
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            log::info!("try-runtime::on_runtime_upgrade");
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, BlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }
}
