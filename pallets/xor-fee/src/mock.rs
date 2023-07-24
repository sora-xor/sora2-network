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

use codec::{Decode, Encode};
use common::mock::{ExistentialDeposits, GetTradingPairRestrictedFlag};
use common::prelude::{Balance, BlockLength, QuoteAmount, SwapAmount, SwapOutcome};
use common::{
    self, balance, fixed_from_basis_points, Amount, AssetId32, AssetName, AssetSymbol, DEXInfo,
    Fixed, LiquidityProxyTrait, LiquiditySource, LiquiditySourceFilter, LiquiditySourceType,
    OnValBurned, PSWAP, VAL, XOR, XST,
};
use core::time::Duration;
use currencies::BasicCurrencyAdapter;
use frame_election_provider_support::{generate_solution_type, NoElection};
use frame_support::dispatch::{DispatchInfo, Pays, PostDispatchInfo};
use frame_support::traits::{
    ConstU128, ConstU32, Currency, Everything, ExistenceRequirement, GenesisBuild, Get,
    OneSessionHandler, PrivilegeCmp, U128CurrencyToVote, WithdrawReasons,
};
use frame_support::weights::{ConstantMultiplier, IdentityFee, Weight};
use frame_support::{construct_runtime, parameter_types};
use pallet_session::historical;
use pallet_staking::UseNominatorsAndValidatorsMap;
use permissions::{Scope, BURN, MINT};
use sp_core::H256;
use sp_runtime::testing::{Header, TestXt, UintAuthorityId};
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Verify};
use sp_runtime::{DispatchError, Perbill, Percent};
use std::cmp::Ordering;

pub use crate::{self as xor_fee, Config, Pallet};

// Configure a mock runtime to test the pallet.
pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type AccountId = u64;
pub type BlockNumber = u64;
type AssetId = AssetId32<common::PredefinedAssetId>;
type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;
type DEXId = common::DEXId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;
use frame_support::sp_runtime::testing::TestSignature;
use frame_system::offchain::{Account, AppCrypto, SigningTypes};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::EnsureRoot;
use sp_core::H160;

pub type Signature = TestSignature;

pub struct TestAppCrypto;
impl AppCrypto<UintAuthorityId, TestSignature> for TestAppCrypto {
    type RuntimeAppPublic = UintAuthorityId;
    type GenericPublic = UintAuthorityId;
    type GenericSignature = TestSignature;
}

pub const BUY_BACK_ACCOUNT: AccountId = 23;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const ReferrerWeight: u32 = 10;
    pub const XorBurnedWeight: u32 = 40;
    pub const XorIntoValBurnedWeight: u32 = 50;
    pub const BuyBackXSTPercent: Percent = Percent::from_percent(10);
    pub const ExistentialDeposit: u32 = 0;
    pub const XorId: AssetId = XOR;
    pub const ValId: AssetId = VAL;
    pub const DEXIdValue: DEXId = common::DEXId::Polkaswap;
    pub GetFee: Fixed = fixed_from_basis_points(0u16);
    pub const Offset: BlockNumber = 0;
    pub const UncleGenerations: u64 = 0;
    pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(25);
    pub const MinimumPeriod: u64 = 5;
    pub const BondingDuration: sp_staking::EraIndex = 3;
    pub const MaxNominatorRewardedPerValidator: u32 = 64;
    pub const UnsignedPriority: u64 = 1 << 20;
    pub const MinSolutionScoreBump: Perbill = Perbill::zero();
    pub const TestValRewardCurve: pallet_staking::sora::ValRewardCurve = pallet_staking::sora::ValRewardCurve {
        duration_to_reward_flatline: Duration::from_millis(100_000),
        min_val_burned_percentage_reward: Percent::from_percent(35),
        max_val_burned_percentage_reward: Percent::from_percent(90),
    };
    pub OffchainSolutionWeightLimit: Weight = Weight::from_parts(600_000_000, 0);
    pub GetXorFeeTechAccountId: TechAccountId = {
        TechAccountId::Generic(
            crate::TECH_ACCOUNT_PREFIX.to_vec(),
            crate::TECH_ACCOUNT_MAIN.to_vec(),
        )
    };
    pub GetXorFeeAccountId: AccountId = {
        let tech_account_id = GetXorFeeTechAccountId::get();
        let repr = technical::tech_account_id_encoded_to_account_id_32(&tech_account_id.encode());
        AccountId::decode(&mut &repr[..]).expect("Failed to decode account Id")
    };
    pub GetParliamentAccountId: AccountId = SORA_PARLIAMENT_ACCOUNT;
    pub const EthNetworkId: <Runtime as eth_bridge::Config>::NetworkId = 0;
    pub const RemovePendingOutgoingRequestsAfter: BlockNumber = 100;
    pub const TrackPendingIncomingRequestsAfter: (BlockNumber, u64) = (0, 0);
    pub RemoveTemporaryPeerAccountId: Vec<(AccountId, H160)> = Vec::new();
    pub const SchedulerMaxWeight: Weight = Weight::from_parts(1024, 0);
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
    pub const ReferralsReservesAcc: AccountId = 22;
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
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Pallet, Call, Config<T>, Storage},
        DexManager: dex_manager::{Pallet, Call, Config<T>, Storage},
        TradingPair: trading_pair::{Pallet, Call, Config<T>, Storage, Event<T>},
        ReferralSystem: referrals::{Pallet, Call, Config<T>, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        TransactionPayment: pallet_transaction_payment::{Pallet, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Config<T>, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Session: pallet_session::{Pallet, Call, Config<T>, Storage, Event},
        Historical: historical::{Pallet},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        Staking: pallet_staking::{Pallet, Call, Config<T>, Storage, Event<T>},
        PoolXYK: pool_xyk::{Pallet, Call, Storage, Event<T>},
        PswapDistribution: pswap_distribution::{Pallet, Call, Config<T>, Storage, Event<T>},
        XorFee: xor_fee::{Pallet, Call, Event<T>},
        LiquidityProxy: mock_liquidity_proxy::{Pallet, Call, Event<T>},
        EthBridge: eth_bridge::{Pallet, Call, Storage, Config<T>, Event<T>},
        BridgeMultisig: bridge_multisig::{Pallet, Call, Storage, Config<T>, Event<T>},
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
        DemeterFarmingPlatform: demeter_farming_platform::{Pallet, Call, Storage, Event<T>},
        CeresLiquidityLocker: ceres_liquidity_locker::{Pallet, Call, Storage, Event<T>},
    }
}

impl xor_fee::ExtractProxySwap for RuntimeCall {
    type AccountId = AccountId;
    type DexId = DEXId;
    type AssetId = AssetId;
    type Amount = SwapAmount<u128>;

    fn extract(
        &self,
    ) -> Option<xor_fee::SwapInfo<Self::AccountId, Self::DexId, Self::AssetId, Self::Amount>> {
        if let RuntimeCall::LiquidityProxy(mock_liquidity_proxy::Call::swap {
            dex_id,
            input_asset_id,
            output_asset_id,
            swap_amount: amount,
            selected_source_types,
            filter_mode,
        }) = self
        {
            Some(xor_fee::SwapInfo {
                fee_source: None,
                dex_id: *dex_id,
                input_asset_id: *input_asset_id,
                output_asset_id: *output_asset_id,
                amount: *amount,
                selected_source_types: selected_source_types.to_vec(),
                filter_mode: filter_mode.clone(),
            })
        } else {
            None
        }
    }
}

impl xor_fee::IsCalledByBridgePeer<AccountId> for RuntimeCall {
    fn is_called_by_bridge_peer(&self, who: &AccountId) -> bool {
        match self {
            RuntimeCall::BridgeMultisig(call) => match call {
                bridge_multisig::Call::as_multi { id, .. } => {
                    bridge_multisig::Accounts::<Runtime>::get(id).map(|acc| acc.is_signatory(&who))
                }
                bridge_multisig::Call::as_multi_threshold_1 { id, .. } => {
                    bridge_multisig::Accounts::<Runtime>::get(id).map(|acc| acc.is_signatory(&who))
                }
                _ => None,
            },
            RuntimeCall::EthBridge(call) => match call {
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
        _public: <Signature as Verify>::Signer,
        account: <Runtime as frame_system::Config>::AccountId,
        _index: <Runtime as frame_system::Config>::Index,
    ) -> Option<(
        RuntimeCall,
        <Extrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
    )> {
        Some((call, (account, ())))
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl eth_bridge::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PeerId = TestAppCrypto;
    type RuntimeCall = RuntimeCall;
    type NetworkId = u32;
    type GetEthNetworkId = EthNetworkId;
    type WeightInfo = ();
    type WeightToFee = XorFee;
    type MessageStatusNotifier = ();
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
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ();
    type WeightInfo = ();
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = ();
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

impl mock_liquidity_source::Config<mock_liquidity_source::Instance1> for Runtime {
    type GetFee = GetFee;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type EnsureTradingPairExists = trading_pair::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl dex_manager::Config for Runtime {}

impl trading_pair::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
    type WeightInfo = ();
}

impl referrals::Config for Runtime {
    type ReservesAcc = ReferralsReservesAcc;
    type WeightInfo = ();
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

impl technical::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
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
    pub const GetBuyBackAccountId: AccountId = BUY_BACK_ACCOUNT;
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = AccountId;
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, AccountId>;
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

impl demeter_farming_platform::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type DemeterAssetId = ();
    const BLOCKS_PER_HOUR_AND_A_HALF: BlockNumberFor<Self> = 900;
    type WeightInfo = ();
}

impl pswap_distribution::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    const PSWAP_BURN_PERCENT: Percent = Percent::from_percent(3);
    type GetIncentiveAssetId = ();
    type GetXSTAssetId = GetBuyBackAssetId;
    type LiquidityProxy = ();
    type CompatBalance = Balance;
    type GetDefaultSubscriptionFrequency = ();
    type GetBurnUpdateFrequency = ();
    type GetTechnicalAccountId = ();
    type EnsureDEXManager = ();
    type OnPswapBurnedAggregator = ();
    type WeightInfo = ();
    type GetParliamentAccountId = GetParliamentAccountId;
    type PoolXykPallet = PoolXYK;
    type BuyBackHandler = ();
    type DexInfoProvider = dex_manager::Pallet<Runtime>;
}

impl pool_xyk::Config for Runtime {
    const MIN_XOR: Balance = balance!(0.0007);
    type RuntimeEvent = RuntimeEvent;
    type PairSwapAction = pool_xyk::PairSwapAction<AssetId, AccountId, TechAccountId>;
    type DepositLiquidityAction =
        pool_xyk::DepositLiquidityAction<AssetId, AccountId, TechAccountId>;
    type WithdrawLiquidityAction =
        pool_xyk::WithdrawLiquidityAction<AssetId, AccountId, TechAccountId>;
    type PolySwapAction = pool_xyk::PolySwapAction<AssetId, AccountId, TechAccountId>;
    type EnsureDEXManager = dex_manager::Pallet<Runtime>;
    type GetFee = GetFee;
    type OnPoolCreated = PswapDistribution;
    type OnPoolReservesChanged = ();
    type WeightInfo = ();
    type XSTMarketInfo = ();
    type GetTradingPairRestrictedFlag = GetTradingPairRestrictedFlag;
}
impl ceres_liquidity_locker::Config for Runtime {
    const BLOCKS_PER_ONE_DAY: BlockNumberFor<Self> = 14_440;
    type RuntimeEvent = RuntimeEvent;
    type XYKPool = PoolXYK;
    type DemeterFarmingPlatform = DemeterFarmingPlatform;
    type CeresAssetId = ();
    type WeightInfo = ();
}

impl pallet_session::Config for Runtime {
    type SessionManager = pallet_session::historical::NoteHistoricalRoot<Runtime, XorFee>;
    type Keys = SessionKeys;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionHandler = (OtherSessionHandler,);
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    type ValidatorIdOf = pallet_staking::StashOf<Runtime>;
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

generate_solution_type!(
    #[compact]
    pub struct NposCompactSolution24::<
        VoterIndex = u32,
        TargetIndex = u16,
        Accuracy = sp_runtime::PerU16,
        MaxVoters = MaxElectingVoters,
    >(24)
);

parameter_types! {
    pub const OffendingValidatorsThreshold: Perbill = Perbill::from_percent(17);
    pub const MaxNominations: u32 = <NposCompactSolution24 as frame_election_provider_support::NposSolution>::LIMIT as u32;
    pub const MaxElectingVoters: u32 = 12_500;
}

pub struct StakingBenchmarkingConfig;
impl pallet_staking::BenchmarkingConfig for StakingBenchmarkingConfig {
    type MaxValidators = ConstU32<1000>;
    type MaxNominators = ConstU32<1000>;
}

impl pallet_staking::Config for Runtime {
    type Currency = Balances;
    type CurrencyBalance = Balance;
    type MultiCurrency = Tokens;
    type ValTokenId = ValId;
    type ValRewardCurve = TestValRewardCurve;
    type UnixTime = Timestamp;
    type CurrencyToVote = U128CurrencyToVote;
    type RuntimeEvent = RuntimeEvent;
    type Slash = ();
    type SessionsPerEra = ();
    type SlashDeferDuration = ();
    type AdminOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type BondingDuration = BondingDuration;
    type SessionInterface = Self;
    type NextNewSession = Session;
    type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
    type ElectionProvider = NoElection<(
        AccountId,
        BlockNumber,
        Staking,
        <StakingBenchmarkingConfig as pallet_staking::BenchmarkingConfig>::MaxValidators,
    )>;
    type GenesisElectionProvider = NoElection<(
        AccountId,
        BlockNumber,
        Staking,
        <StakingBenchmarkingConfig as pallet_staking::BenchmarkingConfig>::MaxValidators,
    )>;
    type OnStakerSlash = ();
    type OffendingValidatorsThreshold = OffendingValidatorsThreshold;
    type MaxNominations = MaxNominations;
    type WeightInfo = ();
    type VoterList = UseNominatorsAndValidatorsMap<Runtime>;
    type MaxUnlockingChunks = ConstU32<32>;
    type BenchmarkingConfig = StakingBenchmarkingConfig;
    type HistoryDepth = frame_support::traits::ConstU32<84>;
    type TargetList = pallet_staking::UseValidatorsMap<Self>;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Runtime
where
    RuntimeCall: From<LocalCall>,
{
    type OverarchingCall = RuntimeCall;
    type Extrinsic = Extrinsic;
}

pub type Extrinsic = TestXt<RuntimeCall, ()>;

pub struct CustomFees;

impl xor_fee::ApplyCustomFees<RuntimeCall> for CustomFees {
    fn compute_fee(call: &RuntimeCall) -> Option<Balance> {
        match call {
            RuntimeCall::Assets(assets::Call::register { .. }) => Some(balance!(0.007)),
            RuntimeCall::Assets(..)
            | RuntimeCall::Staking(pallet_staking::Call::payout_stakers { .. })
            | RuntimeCall::TradingPair(..) => Some(balance!(0.0007)),
            _ => None,
        }
    }
}

pub struct ValBurnedAggregator<T>(sp_std::marker::PhantomData<T>);

impl<T> OnValBurned for ValBurnedAggregator<T>
where
    T: pallet_staking::sora::ValBurnedNotifier<Balance>,
{
    fn on_val_burned(amount: Balance) {
        T::notify_val_burned(amount);
    }
}

pub struct WithdrawFee;

impl xor_fee::WithdrawFee<Runtime> for WithdrawFee {
    fn withdraw_fee(
        who: &AccountId,
        _call: &RuntimeCall,
        fee: Balance,
    ) -> Result<(AccountId, Option<crate::NegativeImbalanceOf<Runtime>>), DispatchError> {
        Ok((
            who.clone(),
            Some(Balances::withdraw(
                who,
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
    type OnValBurned = ValBurnedAggregator<Staking>;
    type CustomFees = CustomFees;
    type GetTechnicalAccountId = GetXorFeeAccountId;
    type SessionManager = Staking;
    type WithdrawFee = WithdrawFee;
    type BuyBackHandler = ();
    type WeightInfo = ();
}

// Allow dead_code because we never call swap, just use its Call variant
#[allow(dead_code)]
#[frame_support::pallet]
pub mod mock_liquidity_proxy {
    use super::*;
    use assets::AssetIdOf;
    use common::{DexIdOf, FilterMode};
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + assets::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn swap(
            _origin: OriginFor<T>,
            _dex_id: DexIdOf<T>,
            _input_asset_id: AssetIdOf<T>,
            _output_asset_id: AssetIdOf<T>,
            _swap_amount: SwapAmount<Balance>,
            _selected_source_types: Vec<LiquiditySourceType>,
            _filter_mode: FilterMode,
        ) -> DispatchResultWithPostInfo {
            return Ok(().into());
        }
    }

    #[pallet::event]
    pub enum Event<T: Config> {}

    #[pallet::error]
    pub enum Error<T> {}
}

type MockLiquidityProxy = mock_liquidity_proxy::Pallet<Runtime>;

impl mock_liquidity_proxy::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl LiquidityProxyTrait<DEXId, AccountId, AssetId> for MockLiquidityProxy {
    fn exchange(
        _dex_id: DEXId,
        sender: &AccountId,
        receiver: &AccountId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: SwapAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let (outcome, _) = MockLiquiditySource::exchange(
            &sender,
            &receiver,
            &filter.dex_id,
            input_asset_id,
            output_asset_id,
            amount,
        )?;
        Ok(outcome)
    }

    fn quote(
        _dex_id: DEXId,
        input_asset_id: &AssetId,
        output_asset_id: &AssetId,
        amount: QuoteAmount<Balance>,
        filter: LiquiditySourceFilter<DEXId, LiquiditySourceType>,
        deduce_fee: bool,
    ) -> Result<SwapOutcome<Balance>, DispatchError> {
        let (outcome, _) = MockLiquiditySource::quote(
            &filter.dex_id,
            input_asset_id,
            output_asset_id,
            amount,
            deduce_fee,
        )?;
        Ok(outcome)
    }
}

pub const REFERRER_ACCOUNT: u64 = 3;
pub const FROM_ACCOUNT: u64 = 1;
pub const TO_ACCOUNT: u64 = 2;
pub const STASH_ACCOUNT: u64 = 11;
pub const STASH_ACCOUNT2: u64 = 21;
pub const CONTROLLER_ACCOUNT: u64 = 10;
pub const CONTROLLER_ACCOUNT2: u64 = 20;
pub const SORA_PARLIAMENT_ACCOUNT: u64 = 7;
pub const EMPTY_ACCOUNT: u64 = 420;

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

    fn on_disabled(_validator_index: u32) {}
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
        common::test_utils::init_logger();
        let mut t = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        let initial_balance = initial_balance();
        pallet_balances::GenesisConfig::<Runtime> {
            balances: vec![
                (FROM_ACCOUNT, initial_balance),
                (TO_ACCOUNT, initial_balance),
                (EMPTY_ACCOUNT, 0),
                (REFERRER_ACCOUNT, initial_balance),
                (STASH_ACCOUNT, initial_balance),
                (STASH_ACCOUNT2, initial_balance),
            ],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        dex_manager::GenesisConfig::<Runtime> {
            dex_list: vec![(
                DEXId::Polkaswap,
                DEXInfo {
                    base_asset_id: XOR.into(),
                    synthetic_base_asset_id: XST.into(),
                    is_public: true,
                },
            )],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        referrals::GenesisConfig::<Runtime> {
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
            register_tech_accounts: vec![(xor_fee_account_id.clone(), tech_account_id.clone())],
        }
        .assimilate_storage(&mut t)
        .unwrap();

        permissions::GenesisConfig::<Runtime> {
            initial_permission_owners: vec![
                (MINT, Scope::Unlimited, vec![xor_fee_account_id]),
                (BURN, Scope::Unlimited, vec![xor_fee_account_id]),
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
                    None,
                    None,
                ),
                (
                    VAL,
                    xor_fee_account_id,
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
                    xor_fee_account_id,
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
            balances: vec![(xor_fee_account_id.clone(), VAL, balance!(1000))],
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
            reserves: vec![
                (
                    common::DEXId::Polkaswap,
                    VAL,
                    (initial_reserves, initial_reserves),
                ),
                (
                    common::DEXId::Polkaswap,
                    XST,
                    (initial_reserves, initial_reserves),
                ),
            ],
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
