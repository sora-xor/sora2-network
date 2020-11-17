// Copyright 2020 Parity Technologies (UK) Ltd.
#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use currencies::BasicCurrencyAdapter;
use frame_system::offchain::{Account, SigningTypes};
use sp_api::impl_runtime_apis;
use sp_core::Encode;
use sp_core::OpaqueMetadata;
use sp_runtime::{
    create_runtime_str, generic, impl_opaque_keys,
    traits::SaturatedConversion,
    traits::{BlakeTwo256, Block as BlockT, IdentifyAccount, IdentityLookup, Saturating, Verify},
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, MultiSignature,
};
use sp_std::marker::PhantomData;
use sp_std::prelude::*;
use sp_std::vec::Vec;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::assert_eq_size;

// Exports from RPC extensions.
use dex_runtime_api::SwapOutcomeInfo;

// A few exports that help ease life for downstream crates.
pub use common::{
    fixed, fixed_from_basis_points,
    prelude::{Balance, SwapAmount, SwapOutcome, SwapVariant},
    BasisPoints, Fixed, LiquiditySource, LiquiditySourceId, LiquiditySourceType,
};
pub use frame_support::{
    construct_runtime, debug, parameter_types,
    traits::KeyOwnerProofSystem,
    traits::Randomness,
    weights::constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight},
    weights::{constants::WEIGHT_PER_SECOND, IdentityFee, Weight},
    StorageValue,
};
pub use pallet_balances::Call as BalancesCall;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Permill};

/// Import the template pallet.
pub use template;

/// Import the message pallet.
pub use cumulus_token_dealer;

/*
/// Importing a iroha-bridge pallet
pub use iroha_bridge;
*/

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

    pub type SessionHandlers = ();

    impl_opaque_keys! {
        pub struct SessionKeys {}
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

pub const MILLISECS_PER_BLOCK: u64 = 6000;

pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

pub const EPOCH_DURATION_IN_BLOCKS: u32 = 10 * MINUTES;

// These time units are defined in number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// 1 in 4 blocks (on average, not counting collisions) will be primary babe blocks.
pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);

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
    type ModuleToIndex = ModuleToIndex;
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
}

parameter_types! {
    pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Trait for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = ();
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
    pub const TransactionByteFee: u128 = 1;
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
}

pub type Amount = i128;

impl tokens::Trait for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Trait>::AssetId;
    type OnReceived = ();
}

parameter_types! {
    // This is common::AssetId with 0 index, 2 is size, 0 and 0 is code.
    pub const GetBaseAssetId: AssetId = common::AssetId32 { code: [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
}

impl currencies::Trait for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Balances, Balance, Balance, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Trait>::GetBaseAssetId;
}

impl common::Trait for Runtime {
    type DEXId = DEXId;
}

impl assets::Trait for Runtime {
    type Event = Event;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
}

impl trading_pair::Trait for Runtime {
    type Event = Event;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
}

parameter_types! {
    pub const GetDefaultFee: BasisPoints = 30;
    pub const GetDefaultProtocolFee: BasisPoints = 0;
}

impl dex_manager::Trait for Runtime {
    type Event = Event;
    type GetDefaultFee = GetDefaultFee;
    type GetDefaultProtocolFee = GetDefaultProtocolFee;
}

impl bonding_curve_pool::Trait for Runtime {
    type DEXApi = ();
}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::AssetId, DEXId>;
pub type AssetId = common::AssetId32<common::AssetId>;

impl technical::Trait for Runtime {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction =
        pool_xyk::PolySwapAction<AssetId, TechAssetId, Balance, AccountId, TechAccountId>;
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
}

parameter_types! {
    pub const GetNumSamples: usize = 100;
}

impl liquidity_proxy::Trait for Runtime {
    type Event = Event;
    type LiquidityRegistry = dex_api::Module<Runtime>;
    type GetNumSamples = GetNumSamples;
}

parameter_types! {
    pub GetFee: Fixed = fixed_from_basis_points(30u16);
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance1> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance2> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance3> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
    type EnsureTradingPairExists = trading_pair::Module<Runtime>;
}

impl mock_liquidity_source::Trait<mock_liquidity_source::Instance4> for Runtime {
    type Event = Event;
    type GetFee = GetFee;
    type EnsureDEXOwner = dex_manager::Module<Runtime>;
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
            // system::CheckSpecVersion::<Runtime>::new(),
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
    pub const ValId: AssetId = common::AssetId32 { code: [2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], phantom: PhantomData };
    pub const DEXIdValue: DEXId = 0;
}

impl xor_fee::Trait for Runtime {
    // Pass native currency.
    type Event = Event;
    type XorCurrency = Balances;
    type ReferrerWeight = ReferrerWeight;
    type XorBurnedWeight = XorBurnedWeight;
    type XorIntoValBurnedWeight = XorIntoValBurnedWeight;
    type XorId = GetBaseAssetId;
    type ValId = ValId;
    type DEXIdValue = DEXIdValue;
    type LiquiditySource = mock_liquidity_source::Module<Runtime, mock_liquidity_source::Instance1>;
}

impl pallet_transaction_payment::Trait for Runtime {
    // Pass native currency.
    type Currency = Balances;
    type OnTransactionPayment = XorFee;
    type TransactionByteFee = TransactionByteFee;
    type WeightToFee = IdentityFee<Balance>;
    type FeeMultiplierUpdate = ();
}

impl pallet_sudo::Trait for Runtime {
    type Call = Call;
    type Event = Event;
}

impl cumulus_parachain_upgrade::Trait for Runtime {
    type Event = Event;
    type OnValidationFunctionParams = ();
}

impl cumulus_message_broker::Trait for Runtime {
    type Event = Event;
    type DownwardMessageHandlers = TokenDealer;
    type UpwardMessage = common::prelude::RococoUpwardMessage;
    type ParachainId = ParachainInfo;
    type XCMPMessage = cumulus_token_dealer::XCMPMessage<AccountId, Balance>;
    type XCMPMessageHandlers = TokenDealer;
}

impl parachain_info::Trait for Runtime {}
impl permissions::Trait for Runtime {
    type Event = Event;
}

impl cumulus_token_dealer::Trait for Runtime {
    type Event = Event;
    type UpwardMessageSender = MessageBroker;
    type UpwardMessage = common::prelude::RococoUpwardMessage;
    type Currency = Balances;
    type XCMPMessageSender = MessageBroker;
}

/// Configure the pallet template in pallets/template.
impl template::Trait for Runtime {
    type Event = Event;
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
        ParachainUpgrade: cumulus_parachain_upgrade::{Module, Call, Storage, Inherent, Event},
        MessageBroker: cumulus_message_broker::{Module, Call, Inherent, Event<T>},
        TransactionPayment: pallet_transaction_payment::{Module, Storage},
        ParachainInfo: parachain_info::{Module, Storage, Config},
        Permissions: permissions::{Module, Call, Storage, Config<T>, Event<T>},
        TokenDealer: cumulus_token_dealer::{Module, Call, Event<T>},
        TemplateModule: template::{Module, Call, Storage, Event<T>},
        ReferralSystem: referral_system::{Module, Call, Storage, Event},
        XorFee: xor_fee::{Module, Call, Storage, Event},

        // Non-native tokens - everything apart of XOR.
        Tokens: tokens::{Module, Storage, Event<T>},
        // Unified interface for XOR and non-native tokens.
        Currencies: currencies::{Module, Call, Event<T>},
        TradingPair: trading_pair::{Module, Call, Event<T>},
        Assets: assets::{Module, Call, Event<T>},
        DEXManager: dex_manager::{Module, Call, Storage, Config<T>, Event<T>},
        BondingCurvePool: bonding_curve_pool::{Module},
        Technical: technical::{Module, Call, Config<T>, Event<T>},
        PoolXYK: pool_xyk::{Module, Call, Event<T>},
        LiquidityProxy: liquidity_proxy::{Module, Call, Event<T>},
        MockLiquiditySource: mock_liquidity_source::<Instance1>::{Module, Call, Storage, Config<T>, Event<T>},
        MockLiquiditySource2: mock_liquidity_source::<Instance2>::{Module, Call, Storage, Config<T>, Event<T>},
        MockLiquiditySource3: mock_liquidity_source::<Instance3>::{Module, Call, Storage, Config<T>, Event<T>},
        MockLiquiditySource4: mock_liquidity_source::<Instance4>::{Module, Call, Storage, Config<T>, Event<T>},
        DEXAPI: dex_api::{Module, Call, Event<T>},
        //IrohaBridge: iroha_bridge::{Module, Call, Storage, Config<T>, Event<T>},
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
        ) -> Option<SwapOutcomeInfo<Balance>> {
            DEXAPI::quote(
                &LiquiditySourceId::new(dex_id, liquidity_source_type),
                &input_asset_id,
                &output_asset_id,
                SwapAmount::with_variant(swap_variant, desired_input_amount.0, Default::default()),
            ).ok().map(|sa| SwapOutcomeInfo::<Balance> { amount: Balance(sa.amount), fee: Balance(sa.fee)})
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

    impl template_runtime_api::TemplateAPI<Block, Balance> for Runtime {
        fn test_multiply_2(amount: Balance) -> Option<template_runtime_api::CustomInfo<Balance>> {
            Some(template_runtime_api::CustomInfo::<Balance> { amount: Balance(Fixed::from(2) * amount.0)})
        }
    }

    impl trading_pair_runtime_api::TradingPairAPI<Block, DEXId, common::TradingPair<AssetId>, AssetId> for Runtime {
        fn list_enabled_pairs(dex_id: DEXId) -> Vec<common::TradingPair<AssetId>> {
            TradingPair::list_trading_pairs(dex_id)
        }

        fn is_pair_enabled(dex_id: DEXId, base_asset_id: AssetId, target_asset_id: AssetId) -> bool {
            TradingPair::is_trading_pair_enabled(dex_id, base_asset_id, target_asset_id)
        }
    }
}

cumulus_runtime::register_validate_block!(Block, Executive);
