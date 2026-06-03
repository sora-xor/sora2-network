#![cfg_attr(not(feature = "std"), no_std)]
#![allow(
    clippy::clone_on_copy,
    clippy::duplicated_attributes,
    clippy::manual_div_ceil,
    clippy::needless_borrows_for_generic_args
)]

pub use pallet::*;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use common::BuyBackHandler;
use frame_support::{
    dispatch::DispatchResult, storage::with_transaction, traits::ConstU32, transactional,
    weights::Weight, BoundedBTreeMap, BoundedVec, PalletId,
};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::traits::{
    AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub, MaybeSerializeDeserialize,
    One, SaturatedConversion, Saturating, Zero,
};
use sp_runtime::{DispatchError, Perbill, TransactionOutcome};
use sp_std::{marker::PhantomData, vec::Vec};

mod weights;
pub use weights::SoraWeight;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub type ConditionId = u32;
pub type MarketId = u32;
pub type OrderId = u64;
pub type PriceCents = u8;

const STORAGE_VERSION: frame_support::traits::StorageVersion =
    frame_support::traits::StorageVersion::new(6);
const CREATION_FEE_BUYBACK_BPS: u32 = 2_000;
const BPS_DENOMINATOR: u128 = 10_000;

fn with_storage_transaction<T>(
    f: impl FnOnce() -> Result<T, DispatchError>,
) -> Result<T, DispatchError> {
    with_transaction(|| {
        let result = f();
        if result.is_ok() {
            TransactionOutcome::Commit(result)
        } else {
            TransactionOutcome::Rollback(result)
        }
    })
}

pub trait WeightInfo {
    fn create_condition() -> Weight;
    fn create_condition_with_details() -> Weight;
    fn create_market() -> Weight;
    fn buy() -> Weight;
    fn sell() -> Weight;
    fn sync_market_status() -> Weight;
    fn resolve_market() -> Weight;
    fn resolve_market_with_evidence() -> Weight;
    fn cancel_market() -> Weight;
    fn emergency_cancel_market() -> Weight;
    fn claim_market() -> Weight;
    fn claim_markets(n: u32) -> Weight;
    fn claim_creator_fees() -> Weight;
    fn sweep_xor_buyback_and_burn() -> Weight;
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
)]
pub struct ConditionMetadata<BoundedString> {
    pub question: BoundedString,
    pub oracle: BoundedString,
    pub resolution_source: BoundedString,
}

#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, Default)]
pub struct ConditionInput {
    pub question: Vec<u8>,
    pub oracle: Vec<u8>,
    pub resolution_source: Vec<u8>,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    Default,
)]
pub struct ConditionDetailsRecord<BoundedString> {
    pub category: Option<BoundedString>,
    pub tags: Option<BoundedString>,
    pub metadata_uri: Option<BoundedString>,
    pub metadata_hash: Option<[u8; 32]>,
    pub rules_uri: Option<BoundedString>,
}

#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, Default)]
pub struct ConditionDetailsInput {
    pub category: Vec<u8>,
    pub tags: Vec<u8>,
    pub metadata_uri: Vec<u8>,
    pub metadata_hash: Option<[u8; 32]>,
    pub rules_uri: Vec<u8>,
}

#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, Default)]
pub struct EvidenceInput {
    pub uri: Vec<u8>,
    pub hash: Option<[u8; 32]>,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
)]
pub enum MarketStatus {
    Open,
    Locked,
    Resolved,
    Cancelled,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
)]
pub enum BinaryOutcome {
    Yes,
    No,
}

impl BinaryOutcome {
    pub fn opposite(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
        }
    }
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
)]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
)]
pub enum MarketMechanism {
    LegacyAmm,
    OrderBook,
    DynamicPariMutuel,
    MigratedLegacy,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
)]
pub struct Market<ClassId, AccountId, BlockNumber, Balance> {
    pub creator: AccountId,
    pub condition_id: ConditionId,
    pub close_block: BlockNumber,
    pub collateral_asset: ClassId,
    pub seed_liquidity: Balance,
    pub mechanism: MarketMechanism,
    pub status: MarketStatus,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
)]
pub struct MarketPool<Balance> {
    pub collateral: Balance,
    pub yes: Balance,
    pub no: Balance,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    Default,
)]
pub struct MarketPosition<Balance> {
    pub yes_shares: Balance,
    pub no_shares: Balance,
    pub net_collateral_paid: Balance,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    Default,
)]
pub struct MarketTotals<Balance> {
    pub total_yes_shares: Balance,
    pub total_no_shares: Balance,
    pub total_net_collateral_paid: Balance,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    Default,
)]
pub struct DpmCostBasis<Balance> {
    pub yes: Balance,
    pub no: Balance,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    Default,
)]
pub struct LiquidityPosition<Balance> {
    pub shares: Balance,
    pub collateral_contributed: Balance,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    Debug,
    MaxEncodedLen,
    Default,
)]
pub struct LiquidityTotals<Balance> {
    pub total_shares: Balance,
    pub total_collateral_contributed: Balance,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
)]
pub struct MarketEvidence<BlockNumber, BoundedString> {
    pub uri: BoundedString,
    pub hash: Option<[u8; 32]>,
    pub at_block: BlockNumber,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
)]
pub struct Order<AccountId, Balance> {
    pub owner: AccountId,
    pub market_id: MarketId,
    pub outcome: BinaryOutcome,
    pub side: OrderSide,
    pub price_cents: PriceCents,
    pub remaining_shares: Balance,
    pub reserved_collateral: Balance,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BuyQuote<Balance> {
    pub market_id: MarketId,
    pub outcome: BinaryOutcome,
    pub collateral_in: Balance,
    pub fee_amount: Balance,
    pub pricing_collateral: Balance,
    pub shares_out: Balance,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SellQuote<Balance> {
    pub market_id: MarketId,
    pub outcome: BinaryOutcome,
    pub shares_in: Balance,
    pub gross_collateral_out: Balance,
    pub fee_amount: Balance,
    pub collateral_out: Balance,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClaimableInfo<AccountId, Balance> {
    pub market_id: MarketId,
    pub account: AccountId,
    pub status: MarketStatus,
    pub resolution_outcome: Option<BinaryOutcome>,
    pub yes_shares: Balance,
    pub no_shares: Balance,
    pub net_collateral_paid: Balance,
    pub trader_payout: Balance,
    pub claimable_payout: Balance,
    pub creator_fees: Balance,
    pub is_creator: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MarketRuntimeState<Balance> {
    pub market_id: MarketId,
    pub mechanism: MarketMechanism,
    pub virtual_depth: Balance,
    pub real_yes_shares: Balance,
    pub real_no_shares: Balance,
    pub dpm_collateral: Balance,
    pub marginal_yes_price_bps: u32,
    pub marginal_no_price_bps: u32,
    pub implied_yes_probability_bps: u32,
    pub implied_no_probability_bps: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct TradeFeeSplit<Balance> {
    creator: Balance,
    buyback: Balance,
}

pub type MetadataString<T> = BoundedVec<u8, <T as pallet::Config>::MaxMetadataLength>;
pub type ConditionMetadataOf<T> = ConditionMetadata<MetadataString<T>>;
pub type ConditionDetailsOf<T> = ConditionDetailsRecord<MetadataString<T>>;
pub type MarketEvidenceOf<T> = MarketEvidence<BlockNumberFor<T>, MetadataString<T>>;

pub type MarketOf<T> = Market<
    <T as Config>::AssetId,
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::Balance,
>;

pub type MarketPoolOf<T> = MarketPool<<T as Config>::Balance>;
pub type MarketPositionOf<T> = MarketPosition<<T as Config>::Balance>;
pub type MarketTotalsOf<T> = MarketTotals<<T as Config>::Balance>;
pub type DpmCostBasisOf<T> = DpmCostBasis<<T as Config>::Balance>;
pub type LiquidityPositionOf<T> = LiquidityPosition<<T as Config>::Balance>;
pub type LiquidityTotalsOf<T> = LiquidityTotals<<T as Config>::Balance>;
pub type BuyQuoteOf<T> = BuyQuote<<T as Config>::Balance>;
pub type SellQuoteOf<T> = SellQuote<<T as Config>::Balance>;
pub type ClaimableInfoOf<T> =
    ClaimableInfo<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;
pub type MarketRuntimeStateOf<T> = MarketRuntimeState<<T as Config>::Balance>;
pub type OrderOf<T> = Order<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;

pub trait AssetTransfer<AccountId, AssetId, Balance> {
    fn transfer(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        amount: Balance,
    ) -> DispatchResult;

    #[cfg(feature = "runtime-benchmarks")]
    fn mint_for_bench(_asset: AssetId, _to: &AccountId, _amount: Balance) -> DispatchResult {
        Err(DispatchError::Other("benchmark-minting-not-supported"))
    }
}

impl<AccountId, AssetId, Balance> AssetTransfer<AccountId, AssetId, Balance> for () {
    fn transfer(
        _asset: AssetId,
        _from: &AccountId,
        _to: &AccountId,
        _amount: Balance,
    ) -> DispatchResult {
        Ok(())
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        ensure,
        pallet_prelude::*,
        traits::{BuildGenesisConfig, EnsureOrigin, Get},
    };
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
        /// Canonical censorship-resistant stablecoin used as collateral (KUSD by default).
        type CanonicalStableAssetId: Get<Self::AssetId>;

        /// Asset handler used for collateral transfers.
        type Assets: AssetTransfer<Self::AccountId, Self::AssetId, Self::Balance>;

        type AssetId: Parameter + Copy + Ord + MaxEncodedLen + TypeInfo;

        type Balance: Parameter
            + AtLeast32BitUnsigned
            + Default
            + Copy
            + MaxEncodedLen
            + MaybeSerializeDeserialize
            + TypeInfo;

        /// Account receiving creation fees during MVP.
        #[pallet::constant]
        type FeeCollector: Get<Self::AccountId>;

        /// Minimum question length to avoid spammy markets.
        #[pallet::constant]
        type MinQuestionLength: Get<u32>;

        /// Minimum absolute creation fee in canonical stable units.
        #[pallet::constant]
        type MinCreationFee: Get<Self::Balance>;

        /// Pallet identifier for deriving the escrow account.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

        /// Legacy creator bond escrow account used only by the v3 storage migration.
        type LegacyCreatorBondEscrowAccount: Get<Self::AccountId>;

        /// Handler used to swap canonical collateral into the buyback asset and burn it.
        type BuyBackHandler: BuyBackHandler<Self::AccountId, Self::AssetId>;

        /// Asset purchased and burned when sweeping accrued buyback collateral.
        #[pallet::constant]
        type GetBuyBackAssetId: Get<Self::AssetId>;

        /// Minimum number of blocks between market creation and close block.
        #[pallet::constant]
        type MinMarketDuration: Get<BlockNumberFor<Self>>;

        /// Maximum metadata length (question/oracle/source).
        #[pallet::constant]
        type MaxMetadataLength: Get<u32>;

        /// Maximum markets accepted by the batch claim extrinsic.
        #[pallet::constant]
        type MaxBatchClaims: Get<u32>;

        /// Maximum maker orders matched by one order placement.
        #[pallet::constant]
        type MaxFillsPerOrder: Get<u32>;

        /// Maximum FIFO order ids stored at one market/outcome/side/price level.
        #[pallet::constant]
        type MaxOrdersPerPrice: Get<u32>;

        /// Maximum open order ids tracked for one account in one market.
        #[pallet::constant]
        type MaxOpenOrdersPerAccountMarket: Get<u32>;

        /// Weight information for extrinsics.
        type WeightInfo: crate::WeightInfo;

        /// Trade fee expressed in basis points (e.g., 50 == 0.50%).
        #[pallet::constant]
        type TradeFeeBps: Get<u32>;

        /// Virtual DPM starting depth per outcome.
        #[pallet::constant]
        type DpmVirtualShares: Get<Self::Balance>;

        /// Origin allowed to finalize market outcomes.
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn next_condition_id)]
    pub type NextConditionId<T> = StorageValue<_, ConditionId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_market_id)]
    pub type NextMarketId<T> = StorageValue<_, MarketId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub type NextOrderId<T> = StorageValue<_, OrderId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn conditions)]
    pub type Conditions<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, ConditionMetadataOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn condition_details)]
    pub type ConditionDetails<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, ConditionDetailsOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn condition_creators)]
    pub type ConditionCreators<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, T::AccountId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn condition_market)]
    pub type ConditionMarket<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, MarketId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn markets)]
    pub type Markets<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_pool)]
    pub type MarketPools<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketPoolOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_order_book_collateral)]
    pub type MarketOrderBookCollateral<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_dpm_collateral)]
    pub type MarketDpmCollateral<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn migrated_legacy_payout)]
    pub type MigratedLegacyPayouts<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        T::AccountId,
        T::Balance,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn market_volume)]
    pub type MarketVolume<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_totals)]
    pub type MarketPositionTotals<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketTotalsOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_creator_fees)]
    pub type MarketCreatorFees<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_resolution)]
    pub type MarketResolution<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, BinaryOutcome, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_resolution_evidence)]
    pub type MarketResolutionEvidence<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketEvidenceOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_cancellation_evidence)]
    pub type MarketCancellationEvidence<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketEvidenceOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_xor_buyback_collateral)]
    pub type PendingXorBuybackCollateral<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_positions)]
    pub type MarketPositions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        T::AccountId,
        MarketPositionOf<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn dpm_cost_basis)]
    pub type DpmCostBasisByAccount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        T::AccountId,
        DpmCostBasisOf<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn dpm_cost_basis_totals)]
    pub type DpmCostBasisTotals<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, DpmCostBasisOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn liquidity_positions)]
    pub type LiquidityPositions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        T::AccountId,
        LiquidityPositionOf<T>,
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn liquidity_totals)]
    pub type LiquidityPositionTotals<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, LiquidityTotalsOf<T>, ValueQuery>;

    #[pallet::storage]
    pub type FeeCollectorOverride<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn orders)]
    pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, OrderId, OrderOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn order_book_queues)]
    pub type OrderBookQueues<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        (BinaryOutcome, OrderSide, PriceCents),
        BoundedVec<OrderId, T::MaxOrdersPerPrice>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn order_book_price_levels)]
    pub type OrderBookPriceLevels<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        (BinaryOutcome, OrderSide),
        BoundedBTreeMap<PriceCents, T::Balance, ConstU32<99>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn open_orders_by_account_market)]
    pub type OpenOrdersByAccountMarket<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MarketId,
        BoundedVec<OrderId, T::MaxOpenOrdersPerAccountMarket>,
        ValueQuery,
    >;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fee_collector: Option<T::AccountId>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                fee_collector: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref account) = self.fee_collector {
                FeeCollectorOverride::<T>::put(account.clone());
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ConditionCreated {
            condition_id: ConditionId,
        },
        ConditionDetailsCreated {
            condition_id: ConditionId,
        },
        MarketCreated {
            market_id: MarketId,
            seed_liquidity: T::Balance,
        },
        TradeExecuted {
            market_id: MarketId,
            trader: T::AccountId,
            side: TradeSide,
            outcome: BinaryOutcome,
            collateral_amount: T::Balance,
            share_amount: T::Balance,
            fee_amount: T::Balance,
        },
        MarketLocked {
            market_id: MarketId,
        },
        MarketResolved {
            market_id: MarketId,
            outcome: BinaryOutcome,
        },
        ResolutionEvidenceStored {
            market_id: MarketId,
        },
        MarketCancelled {
            market_id: MarketId,
        },
        MarketEmergencyCancelled {
            market_id: MarketId,
        },
        MarketClaimed {
            market_id: MarketId,
            trader: T::AccountId,
            payout: T::Balance,
        },
        MarketClaimsBatched {
            trader: T::AccountId,
            requested: u32,
            claimed: u32,
        },
        CreatorFeesClaimed {
            market_id: MarketId,
            creator: T::AccountId,
            amount: T::Balance,
        },
        XorBuybackSwept {
            collateral_amount: T::Balance,
            xor_burned: T::Balance,
        },
        DpmResidualBurned {
            market_id: MarketId,
            amount: T::Balance,
        },
        LegacyMarketMigrated {
            market_id: MarketId,
            status: MarketStatus,
        },
        LegacyMigrationResidualRouted {
            amount: T::Balance,
        },
        HollarRouted {
            user: T::AccountId,
            amount: T::Balance,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        QuestionTooShort,
        ConditionNotFound,
        InvalidCollateralAsset,
        InvalidTradeAmount,
        Overflow,
        MarketDurationTooShort,
        MarketNotOpen,
        MarketNotFinalized,
        MarketAlreadyFinalized,
        MarketNotClosed,
        MarketNotResolved,
        MetadataTooLong,
        SlippageToleranceExceeded,
        NotConditionCreator,
        ConditionAlreadyUsed,
        InvalidMetadata,
        InvalidEvidence,
        MarketUnknown,
        TradeAmountTooSmall,
        InsufficientShares,
        NotMarketCreator,
        NothingToClaim,
        NothingToSweep,
        UnsupportedMarketMechanism,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new prediction condition with oracle metadata.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_condition())]
        #[transactional]
        pub fn create_condition(origin: OriginFor<T>, metadata: ConditionInput) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let bounded = Self::validate_condition_metadata(metadata)?;
            Self::ensure_next_condition_id_available()?;
            Self::withdraw_creation_fee(&who)?;
            Self::create_condition_entry(&who, bounded)?;
            Ok(())
        }

        /// Register a condition with structured UI metadata and off-chain metadata integrity.
        #[pallet::call_index(27)]
        #[pallet::weight(T::WeightInfo::create_condition_with_details())]
        #[transactional]
        pub fn create_condition_with_details(
            origin: OriginFor<T>,
            metadata: ConditionInput,
            details: ConditionDetailsInput,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let bounded = Self::validate_condition_metadata(metadata)?;
            let details = Self::validate_condition_details(details)?;
            Self::ensure_next_condition_id_available()?;
            Self::withdraw_creation_fee(&who)?;
            let condition_id = Self::create_condition_entry(&who, bounded)?;
            ConditionDetails::<T>::insert(condition_id, details);
            Self::deposit_event(Event::ConditionDetailsCreated { condition_id });
            Ok(())
        }

        /// Synchronize a market into the locked state once its close block has passed.
        #[pallet::call_index(26)]
        #[pallet::weight(T::WeightInfo::sync_market_status())]
        pub fn sync_market_status(origin: OriginFor<T>, market_id: MarketId) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let (market, changed) = Self::sync_market_status_if_needed(market_id)?;
            if matches!(market.status, MarketStatus::Open) {
                return Err(Error::<T>::MarketNotClosed.into());
            }
            ensure!(
                changed
                    || matches!(
                        market.status,
                        MarketStatus::Locked | MarketStatus::Resolved | MarketStatus::Cancelled
                    ),
                Error::<T>::MarketNotClosed
            );
            Ok(())
        }

        /// Create a market for a registered condition. New markets use DPM curve issuance.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::create_market())]
        #[transactional]
        pub fn create_market(
            origin: OriginFor<T>,
            condition_id: ConditionId,
            close_block: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_market_close_block(close_block)?;
            Self::ensure_next_market_id_available()?;
            ensure!(
                Conditions::<T>::contains_key(condition_id),
                Error::<T>::ConditionNotFound
            );
            let creator =
                ConditionCreators::<T>::get(condition_id).ok_or(Error::<T>::NotConditionCreator)?;
            ensure!(creator == who, Error::<T>::NotConditionCreator);
            ensure!(
                !ConditionMarket::<T>::contains_key(condition_id),
                Error::<T>::ConditionAlreadyUsed
            );
            Self::create_market_entry(&who, condition_id, close_block)?;
            Ok(())
        }

        /// Buy YES or NO shares from the on-chain binary market maker.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::buy())]
        pub fn buy(
            origin: OriginFor<T>,
            market_id: MarketId,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
            min_shares_out: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!collateral_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let stored_market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(
                matches!(stored_market.mechanism, MarketMechanism::DynamicPariMutuel),
                Error::<T>::UnsupportedMarketMechanism
            );
            let market = Self::ensure_market_tradable(market_id)?;
            Self::buy_dpm(
                &who,
                market_id,
                &market,
                outcome,
                collateral_in,
                min_shares_out,
            )
        }

        /// Sell YES or NO shares back into the on-chain binary market maker.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::sell())]
        pub fn sell(
            origin: OriginFor<T>,
            market_id: MarketId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
            min_collateral_out: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!shares_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let stored_market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(
                matches!(stored_market.mechanism, MarketMechanism::DynamicPariMutuel),
                Error::<T>::UnsupportedMarketMechanism
            );
            let market = Self::ensure_market_tradable(market_id)?;
            Self::sell_dpm(
                &who,
                market_id,
                &market,
                outcome,
                shares_in,
                min_collateral_out,
            )
        }

        /// Resolve an expired market to YES or NO.
        #[pallet::call_index(20)]
        #[pallet::weight(T::WeightInfo::resolve_market())]
        pub fn resolve_market(
            origin: OriginFor<T>,
            market_id: MarketId,
            outcome: BinaryOutcome,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let _ = Self::ensure_market_can_finalize(market_id)?;
            with_storage_transaction(|| -> DispatchResult {
                Markets::<T>::try_mutate(market_id, |market| -> DispatchResult {
                    let market = market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    market.status = MarketStatus::Resolved;
                    Ok(())
                })?;
                MarketResolution::<T>::insert(market_id, outcome);
                Self::burn_dpm_residual_if_no_winners(market_id, outcome)?;
                Self::deposit_event(Event::MarketResolved { market_id, outcome });
                Ok(())
            })
        }

        /// Resolve an expired market and attach a verifiable off-chain evidence URI/hash.
        #[pallet::call_index(28)]
        #[pallet::weight(T::WeightInfo::resolve_market_with_evidence())]
        pub fn resolve_market_with_evidence(
            origin: OriginFor<T>,
            market_id: MarketId,
            outcome: BinaryOutcome,
            evidence: EvidenceInput,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let _ = Self::ensure_market_can_finalize(market_id)?;
            let evidence = Self::validate_evidence(evidence)?;
            with_storage_transaction(|| -> DispatchResult {
                Markets::<T>::try_mutate(market_id, |market| -> DispatchResult {
                    let market = market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    market.status = MarketStatus::Resolved;
                    Ok(())
                })?;
                MarketResolution::<T>::insert(market_id, outcome);
                MarketResolutionEvidence::<T>::insert(market_id, evidence);
                Self::burn_dpm_residual_if_no_winners(market_id, outcome)?;
                Self::deposit_event(Event::MarketResolved { market_id, outcome });
                Self::deposit_event(Event::ResolutionEvidenceStored { market_id });
                Ok(())
            })
        }

        /// Cancel an expired market and unlock cancellation refunds.
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::cancel_market())]
        pub fn cancel_market(origin: OriginFor<T>, market_id: MarketId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let _ = Self::ensure_market_can_finalize(market_id)?;
            with_storage_transaction(|| -> DispatchResult {
                Markets::<T>::try_mutate(market_id, |market| -> DispatchResult {
                    let market = market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    market.status = MarketStatus::Cancelled;
                    Ok(())
                })?;
                MarketResolution::<T>::remove(market_id);
                Self::deposit_event(Event::MarketCancelled { market_id });
                Ok(())
            })
        }

        /// Emergency-cancel any non-finalized market with an evidence URI/hash.
        #[pallet::call_index(29)]
        #[pallet::weight(T::WeightInfo::emergency_cancel_market())]
        pub fn emergency_cancel_market(
            origin: OriginFor<T>,
            market_id: MarketId,
            evidence: EvidenceInput,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(
                matches!(market.mechanism, MarketMechanism::DynamicPariMutuel),
                Error::<T>::UnsupportedMarketMechanism
            );
            ensure!(
                !matches!(
                    market.status,
                    MarketStatus::Resolved | MarketStatus::Cancelled
                ),
                Error::<T>::MarketAlreadyFinalized
            );
            let evidence = Self::validate_evidence(evidence)?;
            with_storage_transaction(|| -> DispatchResult {
                Markets::<T>::try_mutate(market_id, |market| -> DispatchResult {
                    let market = market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    market.status = MarketStatus::Cancelled;
                    Ok(())
                })?;
                MarketResolution::<T>::remove(market_id);
                MarketCancellationEvidence::<T>::insert(market_id, evidence);
                Self::deposit_event(Event::MarketCancelled { market_id });
                Self::deposit_event(Event::MarketEmergencyCancelled { market_id });
                Ok(())
            })
        }

        /// Claim a resolved payout or cancellation refund.
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::claim_market())]
        pub fn claim_market(origin: OriginFor<T>, market_id: MarketId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::claim_market_for(&who, market_id).map(|_| ())
        }

        /// Claim multiple finalized markets for the caller, skipping markets with no claim.
        #[pallet::call_index(32)]
        #[pallet::weight(T::WeightInfo::claim_markets(market_ids.len() as u32))]
        pub fn claim_markets(
            origin: OriginFor<T>,
            market_ids: BoundedVec<MarketId, T::MaxBatchClaims>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let requested = market_ids.len() as u32;
            let mut claimed = 0u32;
            for market_id in market_ids.into_iter() {
                if Self::claim_market_for(&who, market_id).is_ok() {
                    claimed = claimed.saturating_add(1);
                }
            }
            ensure!(claimed > 0, Error::<T>::NothingToClaim);
            Self::deposit_event(Event::MarketClaimsBatched {
                trader: who,
                requested,
                claimed,
            });
            Ok(())
        }

        /// Claim accumulated creator trading fees.
        #[pallet::call_index(23)]
        #[pallet::weight(T::WeightInfo::claim_creator_fees())]
        #[transactional]
        pub fn claim_creator_fees(origin: OriginFor<T>, market_id: MarketId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(market.creator == who, Error::<T>::NotMarketCreator);
            let amount = MarketCreatorFees::<T>::take(market_id);
            ensure!(!amount.is_zero(), Error::<T>::NothingToClaim);
            T::Assets::transfer(market.collateral_asset, &Self::account_id(), &who, amount)?;
            Self::deposit_event(Event::CreatorFeesClaimed {
                market_id,
                creator: who,
                amount,
            });
            Ok(())
        }

        /// Permissionlessly swap accrued buyback collateral into XOR and burn it.
        #[pallet::call_index(25)]
        #[pallet::weight(T::WeightInfo::sweep_xor_buyback_and_burn())]
        #[transactional]
        pub fn sweep_xor_buyback_and_burn(origin: OriginFor<T>) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            let amount = PendingXorBuybackCollateral::<T>::get();
            ensure!(!amount.is_zero(), Error::<T>::NothingToSweep);
            let source = Self::account_id();
            let collateral_asset = T::CanonicalStableAssetId::get();
            let buyback_asset = T::GetBuyBackAssetId::get();
            let burned = T::BuyBackHandler::buy_back_and_burn(
                &source,
                &collateral_asset,
                &buyback_asset,
                amount.saturated_into::<common::Balance>(),
            )?;
            PendingXorBuybackCollateral::<T>::put(T::Balance::zero());
            Self::deposit_event(Event::XorBuybackSwept {
                collateral_amount: amount,
                xor_burned: burned.saturated_into(),
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn fee_collector_account() -> T::AccountId {
            FeeCollectorOverride::<T>::get().unwrap_or_else(T::FeeCollector::get)
        }

        fn withdraw_creation_fee(who: &T::AccountId) -> DispatchResult {
            let fee = T::MinCreationFee::get();
            let fee_collector = Self::fee_collector_account();
            let deposited = Self::deposit_canonical(who, &fee_collector, fee)?;

            let buyback_amount =
                Perbill::from_rational(CREATION_FEE_BUYBACK_BPS, 10_000u32) * deposited;
            if !buyback_amount.is_zero() {
                T::Assets::transfer(
                    T::CanonicalStableAssetId::get(),
                    &fee_collector,
                    &Self::account_id(),
                    buyback_amount,
                )?;
                PendingXorBuybackCollateral::<T>::mutate(|total| {
                    *total = total.saturating_add(buyback_amount);
                });
            }
            Ok(())
        }

        pub(crate) fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        fn deposit_canonical(
            who: &T::AccountId,
            dest: &T::AccountId,
            amount: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            if amount.is_zero() {
                return Ok(amount);
            }
            T::Assets::transfer(T::CanonicalStableAssetId::get(), who, dest, amount)?;
            Ok(amount)
        }

        fn bounded_optional_text(
            value: Vec<u8>,
        ) -> Result<Option<MetadataString<T>>, DispatchError> {
            if value.is_empty() {
                return Ok(None);
            }
            ensure!(
                core::str::from_utf8(&value).is_ok(),
                Error::<T>::InvalidMetadata
            );
            Ok(Some(
                MetadataString::<T>::try_from(value).map_err(|_| Error::<T>::MetadataTooLong)?,
            ))
        }

        fn validate_condition_details(
            details: ConditionDetailsInput,
        ) -> Result<ConditionDetailsOf<T>, DispatchError> {
            Ok(ConditionDetailsRecord {
                category: Self::bounded_optional_text(details.category)?,
                tags: Self::bounded_optional_text(details.tags)?,
                metadata_uri: Self::bounded_optional_text(details.metadata_uri)?,
                metadata_hash: details.metadata_hash,
                rules_uri: Self::bounded_optional_text(details.rules_uri)?,
            })
        }

        fn validate_evidence(
            evidence: EvidenceInput,
        ) -> Result<MarketEvidenceOf<T>, DispatchError> {
            ensure!(!evidence.uri.is_empty(), Error::<T>::InvalidEvidence);
            ensure!(
                core::str::from_utf8(&evidence.uri).is_ok(),
                Error::<T>::InvalidEvidence
            );
            Ok(MarketEvidence {
                uri: MetadataString::<T>::try_from(evidence.uri)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
                hash: evidence.hash,
                at_block: <frame_system::Pallet<T>>::block_number(),
            })
        }

        fn ensure_market_tradable(market_id: MarketId) -> Result<MarketOf<T>, DispatchError> {
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            ensure!(
                matches!(market.status, MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );
            Ok(market)
        }

        fn buy_dpm(
            who: &T::AccountId,
            market_id: MarketId,
            market: &MarketOf<T>,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
            min_shares_out: T::Balance,
        ) -> DispatchResult {
            with_storage_transaction(|| -> DispatchResult {
                let quote = Self::quote_dpm_buy_market(market_id, outcome, collateral_in)?;
                ensure!(
                    quote.shares_out >= min_shares_out,
                    Error::<T>::SlippageToleranceExceeded
                );
                Self::ensure_position_can_credit(
                    market_id,
                    who,
                    outcome,
                    quote.shares_out,
                    quote.pricing_collateral,
                )?;

                T::Assets::transfer(
                    market.collateral_asset,
                    who,
                    &Self::account_id(),
                    collateral_in,
                )?;
                Self::record_trade_fees(market_id, Self::split_dpm_trade_fee(quote.fee_amount));
                Self::increase_dpm_collateral(market_id, quote.pricing_collateral)?;
                Self::record_market_volume(market_id, quote.pricing_collateral);
                Self::credit_position_on_buy(
                    market_id,
                    who,
                    outcome,
                    quote.shares_out,
                    quote.pricing_collateral,
                )?;
                Self::credit_dpm_cost_basis(market_id, who, outcome, quote.pricing_collateral)?;

                Self::deposit_event(Event::TradeExecuted {
                    market_id,
                    trader: who.clone(),
                    side: TradeSide::Buy,
                    outcome,
                    collateral_amount: collateral_in,
                    share_amount: quote.shares_out,
                    fee_amount: quote.fee_amount,
                });
                Ok(())
            })
        }

        fn sell_dpm(
            who: &T::AccountId,
            market_id: MarketId,
            market: &MarketOf<T>,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
            min_collateral_out: T::Balance,
        ) -> DispatchResult {
            Self::ensure_position_has_shares(market_id, who, outcome, shares_in)?;
            with_storage_transaction(|| -> DispatchResult {
                let quote = Self::quote_dpm_sell_market(market_id, outcome, shares_in)?;
                ensure!(
                    quote.collateral_out >= min_collateral_out,
                    Error::<T>::SlippageToleranceExceeded
                );
                Self::ensure_dpm_collateral(market_id, quote.gross_collateral_out)?;
                Self::record_trade_fees(market_id, Self::split_dpm_trade_fee(quote.fee_amount));
                Self::decrease_dpm_collateral(market_id, quote.gross_collateral_out)?;
                Self::record_market_volume(market_id, quote.gross_collateral_out);
                Self::debit_dpm_position_on_sell(market_id, who, outcome, shares_in)?;
                T::Assets::transfer(
                    market.collateral_asset,
                    &Self::account_id(),
                    who,
                    quote.collateral_out,
                )?;

                Self::deposit_event(Event::TradeExecuted {
                    market_id,
                    trader: who.clone(),
                    side: TradeSide::Sell,
                    outcome,
                    collateral_amount: quote.collateral_out,
                    share_amount: shares_in,
                    fee_amount: quote.fee_amount,
                });
                Ok(())
            })
        }

        fn split_dpm_trade_fee(amount: T::Balance) -> TradeFeeSplit<T::Balance> {
            let fee = amount.saturated_into::<u128>();
            let creator = fee.saturating_mul(80) / 100;
            let buyback = fee.saturating_sub(creator);
            TradeFeeSplit {
                creator: creator.saturated_into::<T::Balance>(),
                buyback: buyback.saturated_into::<T::Balance>(),
            }
        }

        fn claim_market_for(
            who: &T::AccountId,
            market_id: MarketId,
        ) -> Result<T::Balance, DispatchError> {
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            match market.mechanism {
                MarketMechanism::MigratedLegacy => {
                    Self::claim_migrated_legacy_market_for(who, market_id, &market)
                }
                MarketMechanism::DynamicPariMutuel => {
                    Self::claim_dpm_market_for(who, market_id, &market)
                }
                MarketMechanism::OrderBook | MarketMechanism::LegacyAmm => {
                    Err(Error::<T>::UnsupportedMarketMechanism.into())
                }
            }
        }

        fn claim_migrated_legacy_market_for(
            who: &T::AccountId,
            market_id: MarketId,
            market: &MarketOf<T>,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(
                matches!(
                    market.status,
                    MarketStatus::Resolved | MarketStatus::Cancelled
                ),
                Error::<T>::MarketNotFinalized
            );
            let payout = MigratedLegacyPayouts::<T>::get(market_id, who);
            ensure!(!payout.is_zero(), Error::<T>::NothingToClaim);
            with_storage_transaction(|| -> Result<T::Balance, DispatchError> {
                MigratedLegacyPayouts::<T>::remove(market_id, who);
                T::Assets::transfer(market.collateral_asset, &Self::account_id(), who, payout)?;
                Self::deposit_event(Event::MarketClaimed {
                    market_id,
                    trader: who.clone(),
                    payout,
                });
                Ok(payout)
            })
        }

        fn claim_dpm_market_for(
            who: &T::AccountId,
            market_id: MarketId,
            market: &MarketOf<T>,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(
                matches!(
                    market.status,
                    MarketStatus::Resolved | MarketStatus::Cancelled
                ),
                Error::<T>::MarketNotFinalized
            );
            let position =
                MarketPositions::<T>::get(market_id, who).ok_or(Error::<T>::NothingToClaim)?;
            let cost_basis = DpmCostBasisByAccount::<T>::get(market_id, who).unwrap_or_default();
            ensure!(
                !position.yes_shares.is_zero()
                    || !position.no_shares.is_zero()
                    || !position.net_collateral_paid.is_zero()
                    || !Self::dpm_cost_basis_sum(&cost_basis)?.is_zero(),
                Error::<T>::NothingToClaim
            );

            let payout = match market.status {
                MarketStatus::Resolved => {
                    let outcome = MarketResolution::<T>::get(market_id)
                        .ok_or(Error::<T>::MarketNotResolved)?;
                    let totals = MarketPositionTotals::<T>::get(market_id);
                    let winning_shares = Self::winning_shares(&position, outcome);
                    let total_winning_shares = match outcome {
                        BinaryOutcome::Yes => totals.total_yes_shares,
                        BinaryOutcome::No => totals.total_no_shares,
                    };
                    if winning_shares.is_zero() || total_winning_shares.is_zero() {
                        T::Balance::zero()
                    } else if winning_shares == total_winning_shares {
                        MarketDpmCollateral::<T>::get(market_id)
                    } else {
                        Self::pro_rata(
                            MarketDpmCollateral::<T>::get(market_id),
                            winning_shares,
                            total_winning_shares,
                        )?
                    }
                }
                MarketStatus::Cancelled => {
                    let account_basis = Self::dpm_cost_basis_sum(&cost_basis)?;
                    let total_basis =
                        Self::dpm_cost_basis_sum(&DpmCostBasisTotals::<T>::get(market_id))?;
                    if account_basis.is_zero() || total_basis.is_zero() {
                        T::Balance::zero()
                    } else if account_basis == total_basis {
                        MarketDpmCollateral::<T>::get(market_id)
                    } else {
                        Self::pro_rata(
                            MarketDpmCollateral::<T>::get(market_id),
                            account_basis,
                            total_basis,
                        )?
                    }
                }
                _ => return Err(Error::<T>::MarketNotFinalized.into()),
            };

            with_storage_transaction(|| -> Result<T::Balance, DispatchError> {
                MarketPositions::<T>::remove(market_id, who);
                Self::debit_market_totals(market_id, &position)?;
                Self::remove_dpm_cost_basis(market_id, who)?;
                Self::decrease_dpm_collateral(market_id, payout)?;
                if !payout.is_zero() {
                    T::Assets::transfer(market.collateral_asset, &Self::account_id(), who, payout)?;
                }
                Self::deposit_event(Event::MarketClaimed {
                    market_id,
                    trader: who.clone(),
                    payout,
                });
                Ok(payout)
            })
        }

        pub fn quote_buy_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
        ) -> Result<BuyQuoteOf<T>, DispatchError> {
            ensure!(!collateral_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            if matches!(market.mechanism, MarketMechanism::DynamicPariMutuel) {
                return Self::quote_dpm_buy_market(market_id, outcome, collateral_in);
            }
            Err(Error::<T>::UnsupportedMarketMechanism.into())
        }

        pub fn quote_sell_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<SellQuoteOf<T>, DispatchError> {
            ensure!(!shares_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            if matches!(market.mechanism, MarketMechanism::DynamicPariMutuel) {
                return Self::quote_dpm_sell_market(market_id, outcome, shares_in);
            }
            Err(Error::<T>::UnsupportedMarketMechanism.into())
        }

        pub fn claimable_info(
            who: T::AccountId,
            market_id: MarketId,
        ) -> Result<ClaimableInfoOf<T>, DispatchError> {
            let mut market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            market.status = Self::effective_market_status(&market);
            let resolution_outcome = MarketResolution::<T>::get(market_id);
            let position = MarketPositions::<T>::get(market_id, &who).unwrap_or_default();
            let trader_payout = match market.status {
                MarketStatus::Resolved | MarketStatus::Cancelled
                    if matches!(market.mechanism, MarketMechanism::MigratedLegacy) =>
                {
                    MigratedLegacyPayouts::<T>::get(market_id, &who)
                }
                MarketStatus::Resolved => match (market.mechanism, resolution_outcome) {
                    (MarketMechanism::DynamicPariMutuel, Some(outcome)) => {
                        let totals = MarketPositionTotals::<T>::get(market_id);
                        let winning_shares = Self::winning_shares(&position, outcome);
                        let total_winning_shares = match outcome {
                            BinaryOutcome::Yes => totals.total_yes_shares,
                            BinaryOutcome::No => totals.total_no_shares,
                        };
                        if winning_shares.is_zero() || total_winning_shares.is_zero() {
                            T::Balance::zero()
                        } else if winning_shares == total_winning_shares {
                            MarketDpmCollateral::<T>::get(market_id)
                        } else {
                            Self::pro_rata(
                                MarketDpmCollateral::<T>::get(market_id),
                                winning_shares,
                                total_winning_shares,
                            )?
                        }
                    }
                    (_, None) => T::Balance::zero(),
                    _ => T::Balance::zero(),
                },
                MarketStatus::Cancelled => {
                    if matches!(market.mechanism, MarketMechanism::DynamicPariMutuel) {
                        let cost_basis =
                            DpmCostBasisByAccount::<T>::get(market_id, &who).unwrap_or_default();
                        let account_basis = Self::dpm_cost_basis_sum(&cost_basis)?;
                        let total_basis =
                            Self::dpm_cost_basis_sum(&DpmCostBasisTotals::<T>::get(market_id))?;
                        if account_basis.is_zero() || total_basis.is_zero() {
                            T::Balance::zero()
                        } else if account_basis == total_basis {
                            MarketDpmCollateral::<T>::get(market_id)
                        } else {
                            Self::pro_rata(
                                MarketDpmCollateral::<T>::get(market_id),
                                account_basis,
                                total_basis,
                            )?
                        }
                    } else {
                        T::Balance::zero()
                    }
                }
                _ => T::Balance::zero(),
            };
            let claimable_payout = trader_payout;
            let is_creator = market.creator == who;
            let creator_fees = if is_creator {
                MarketCreatorFees::<T>::get(market_id)
            } else {
                T::Balance::zero()
            };

            Ok(ClaimableInfo {
                market_id,
                account: who,
                status: market.status,
                resolution_outcome,
                yes_shares: position.yes_shares,
                no_shares: position.no_shares,
                net_collateral_paid: position.net_collateral_paid,
                trader_payout,
                claimable_payout,
                creator_fees,
                is_creator,
            })
        }

        pub fn market_state(market_id: MarketId) -> Result<MarketRuntimeStateOf<T>, DispatchError> {
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            let totals = MarketPositionTotals::<T>::get(market_id);
            if !matches!(market.mechanism, MarketMechanism::DynamicPariMutuel) {
                return Ok(MarketRuntimeState {
                    market_id,
                    mechanism: market.mechanism,
                    virtual_depth: T::Balance::zero(),
                    real_yes_shares: totals.total_yes_shares,
                    real_no_shares: totals.total_no_shares,
                    dpm_collateral: T::Balance::zero(),
                    marginal_yes_price_bps: 0,
                    marginal_no_price_bps: 0,
                    implied_yes_probability_bps: 0,
                    implied_no_probability_bps: 0,
                });
            }

            let (q_yes, q_no) = Self::dpm_quantities(market_id)?;
            let cost = Self::dpm_cost(q_yes, q_no)?;
            let total_q = q_yes.checked_add(q_no).ok_or(Error::<T>::Overflow)?;
            Ok(MarketRuntimeState {
                market_id,
                mechanism: market.mechanism,
                virtual_depth: T::DpmVirtualShares::get(),
                real_yes_shares: totals.total_yes_shares,
                real_no_shares: totals.total_no_shares,
                dpm_collateral: MarketDpmCollateral::<T>::get(market_id),
                marginal_yes_price_bps: Self::ratio_bps(q_yes, cost),
                marginal_no_price_bps: Self::ratio_bps(q_no, cost),
                implied_yes_probability_bps: Self::ratio_bps(q_yes, total_q),
                implied_no_probability_bps: Self::ratio_bps(q_no, total_q),
            })
        }

        fn ensure_market_can_finalize(
            market_id: MarketId,
        ) -> Result<(MarketOf<T>, bool), DispatchError> {
            let stored_market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(
                matches!(stored_market.mechanism, MarketMechanism::DynamicPariMutuel),
                Error::<T>::UnsupportedMarketMechanism
            );
            let (market, changed) = Self::sync_market_status_if_needed(market_id)?;
            match market.status {
                MarketStatus::Open => Err(Error::<T>::MarketNotClosed.into()),
                MarketStatus::Locked => Ok((market, changed)),
                MarketStatus::Resolved | MarketStatus::Cancelled => {
                    Err(Error::<T>::MarketAlreadyFinalized.into())
                }
            }
        }

        fn effective_market_status(market: &MarketOf<T>) -> MarketStatus {
            let now = <frame_system::Pallet<T>>::block_number();
            if matches!(market.status, MarketStatus::Open) && now >= market.close_block {
                MarketStatus::Locked
            } else {
                market.status.clone()
            }
        }

        fn sync_market_status_if_needed(
            market_id: MarketId,
        ) -> Result<(MarketOf<T>, bool), DispatchError> {
            let now = <frame_system::Pallet<T>>::block_number();
            let mut changed = false;
            let market = Markets::<T>::try_mutate(
                market_id,
                |maybe_market| -> Result<MarketOf<T>, DispatchError> {
                    let market = maybe_market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    if matches!(market.status, MarketStatus::Open) && now >= market.close_block {
                        market.status = MarketStatus::Locked;
                        changed = true;
                    }
                    Ok(market.clone())
                },
            )?;
            if changed {
                Self::deposit_event(Event::MarketLocked { market_id });
            }
            Ok((market, changed))
        }

        fn record_market_volume(market_id: MarketId, amount: T::Balance) {
            if amount.is_zero() {
                return;
            }
            MarketVolume::<T>::mutate(market_id, |volume| {
                *volume = volume.saturating_add(amount);
            });
        }

        fn trade_fee(amount: T::Balance) -> T::Balance {
            let fee_bps = T::TradeFeeBps::get().min(10_000);
            Perbill::from_rational(fee_bps, 10_000u32) * amount
        }

        fn record_trade_fees(market_id: MarketId, split: TradeFeeSplit<T::Balance>) {
            if !split.creator.is_zero() {
                MarketCreatorFees::<T>::mutate(market_id, |total| {
                    *total = total.saturating_add(split.creator);
                });
            }
            if !split.buyback.is_zero() {
                PendingXorBuybackCollateral::<T>::mutate(|total| {
                    *total = total.saturating_add(split.buyback);
                });
            }
        }

        fn increase_dpm_collateral(market_id: MarketId, amount: T::Balance) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            MarketDpmCollateral::<T>::try_mutate(market_id, |total| -> DispatchResult {
                *total = total.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })
        }

        fn ensure_dpm_collateral(market_id: MarketId, amount: T::Balance) -> DispatchResult {
            ensure!(
                MarketDpmCollateral::<T>::get(market_id) >= amount,
                Error::<T>::Overflow
            );
            Ok(())
        }

        fn decrease_dpm_collateral(market_id: MarketId, amount: T::Balance) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            MarketDpmCollateral::<T>::try_mutate(market_id, |total| -> DispatchResult {
                ensure!(*total >= amount, Error::<T>::Overflow);
                *total = total.saturating_sub(amount);
                Ok(())
            })
        }

        fn dpm_quantities(market_id: MarketId) -> Result<(U256, U256), DispatchError> {
            let virtual_shares = T::DpmVirtualShares::get();
            ensure!(!virtual_shares.is_zero(), Error::<T>::Overflow);
            let totals = MarketPositionTotals::<T>::get(market_id);
            let yes = virtual_shares
                .checked_add(&totals.total_yes_shares)
                .ok_or(Error::<T>::Overflow)?;
            let no = virtual_shares
                .checked_add(&totals.total_no_shares)
                .ok_or(Error::<T>::Overflow)?;
            Ok((
                U256::from(yes.saturated_into::<u128>()),
                U256::from(no.saturated_into::<u128>()),
            ))
        }

        fn dpm_cost(q_yes: U256, q_no: U256) -> Result<U256, DispatchError> {
            let yes_sq = q_yes.checked_mul(q_yes).ok_or(Error::<T>::Overflow)?;
            let no_sq = q_no.checked_mul(q_no).ok_or(Error::<T>::Overflow)?;
            Ok(Self::sqrt_u256_floor(
                yes_sq.checked_add(no_sq).ok_or(Error::<T>::Overflow)?,
            ))
        }

        fn quote_dpm_buy(
            market_id: MarketId,
            outcome: BinaryOutcome,
            pricing_collateral: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let (q_yes, q_no) = Self::dpm_quantities(market_id)?;
            let (selected, opposite) = match outcome {
                BinaryOutcome::Yes => (q_yes, q_no),
                BinaryOutcome::No => (q_no, q_yes),
            };
            let cost = Self::dpm_cost(q_yes, q_no)?;
            let input = U256::from(pricing_collateral.saturated_into::<u128>());
            let target_cost = cost.checked_add(input).ok_or(Error::<T>::Overflow)?;
            let target_sq = target_cost
                .checked_mul(target_cost)
                .ok_or(Error::<T>::Overflow)?;
            let opposite_sq = opposite.checked_mul(opposite).ok_or(Error::<T>::Overflow)?;
            ensure!(target_sq > opposite_sq, Error::<T>::TradeAmountTooSmall);
            let selected_after = Self::sqrt_u256_floor(target_sq - opposite_sq);
            ensure!(selected_after > selected, Error::<T>::TradeAmountTooSmall);
            Self::u256_to_balance(selected_after - selected)
        }

        fn quote_dpm_sell(
            market_id: MarketId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let totals = MarketPositionTotals::<T>::get(market_id);
            let real_selected = match outcome {
                BinaryOutcome::Yes => totals.total_yes_shares,
                BinaryOutcome::No => totals.total_no_shares,
            };
            ensure!(real_selected >= shares_in, Error::<T>::InsufficientShares);

            let (q_yes, q_no) = Self::dpm_quantities(market_id)?;
            let shares = U256::from(shares_in.saturated_into::<u128>());
            let (yes_after, no_after) = match outcome {
                BinaryOutcome::Yes => {
                    ensure!(q_yes >= shares, Error::<T>::InsufficientShares);
                    (q_yes - shares, q_no)
                }
                BinaryOutcome::No => {
                    ensure!(q_no >= shares, Error::<T>::InsufficientShares);
                    (q_yes, q_no - shares)
                }
            };
            let before = Self::dpm_cost(q_yes, q_no)?;
            let after = Self::dpm_cost(yes_after, no_after)?;
            ensure!(before > after, Error::<T>::TradeAmountTooSmall);
            Self::u256_to_balance(before - after)
        }

        fn quote_dpm_buy_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
        ) -> Result<BuyQuoteOf<T>, DispatchError> {
            ensure!(!collateral_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(
                matches!(market.mechanism, MarketMechanism::DynamicPariMutuel),
                Error::<T>::UnsupportedMarketMechanism
            );
            ensure!(
                matches!(Self::effective_market_status(&market), MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );

            let fee_amount = Self::trade_fee(collateral_in);
            let pricing_collateral = collateral_in
                .checked_sub(&fee_amount)
                .ok_or(Error::<T>::TradeAmountTooSmall)?;
            ensure!(
                !pricing_collateral.is_zero(),
                Error::<T>::TradeAmountTooSmall
            );
            let shares_out = Self::quote_dpm_buy(market_id, outcome, pricing_collateral)?;
            ensure!(!shares_out.is_zero(), Error::<T>::TradeAmountTooSmall);

            Ok(BuyQuote {
                market_id,
                outcome,
                collateral_in,
                fee_amount,
                pricing_collateral,
                shares_out,
            })
        }

        fn quote_dpm_sell_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<SellQuoteOf<T>, DispatchError> {
            ensure!(!shares_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            ensure!(
                matches!(market.mechanism, MarketMechanism::DynamicPariMutuel),
                Error::<T>::UnsupportedMarketMechanism
            );
            ensure!(
                matches!(Self::effective_market_status(&market), MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );

            let gross_collateral_out = Self::quote_dpm_sell(market_id, outcome, shares_in)?;
            ensure!(
                !gross_collateral_out.is_zero(),
                Error::<T>::TradeAmountTooSmall
            );
            Self::ensure_dpm_collateral(market_id, gross_collateral_out)?;
            let fee_amount = Self::trade_fee(gross_collateral_out);
            let collateral_out = gross_collateral_out
                .checked_sub(&fee_amount)
                .ok_or(Error::<T>::TradeAmountTooSmall)?;
            ensure!(!collateral_out.is_zero(), Error::<T>::TradeAmountTooSmall);

            Ok(SellQuote {
                market_id,
                outcome,
                shares_in,
                gross_collateral_out,
                fee_amount,
                collateral_out,
            })
        }

        fn credit_dpm_cost_basis(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            amount: T::Balance,
        ) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            DpmCostBasisByAccount::<T>::try_mutate(market_id, who, |basis| -> DispatchResult {
                let entry = basis.get_or_insert_with(Default::default);
                match outcome {
                    BinaryOutcome::Yes => {
                        entry.yes = entry.yes.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        entry.no = entry.no.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
                    }
                }
                Ok(())
            })?;
            DpmCostBasisTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.yes = totals
                            .yes
                            .checked_add(&amount)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        totals.no = totals.no.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
                    }
                }
                Ok(())
            })
        }

        fn debit_dpm_position_on_sell(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> DispatchResult {
            let position =
                MarketPositions::<T>::get(market_id, who).ok_or(Error::<T>::InsufficientShares)?;
            let side_shares = match outcome {
                BinaryOutcome::Yes => position.yes_shares,
                BinaryOutcome::No => position.no_shares,
            };
            ensure!(side_shares >= shares_in, Error::<T>::InsufficientShares);
            let basis = DpmCostBasisByAccount::<T>::get(market_id, who).unwrap_or_default();
            let side_basis = match outcome {
                BinaryOutcome::Yes => basis.yes,
                BinaryOutcome::No => basis.no,
            };
            let basis_reduction = if shares_in == side_shares {
                side_basis
            } else {
                Self::pro_rata(side_basis, shares_in, side_shares)?
            };

            MarketPositions::<T>::try_mutate_exists(
                market_id,
                who,
                |position| -> DispatchResult {
                    let entry = position.as_mut().ok_or(Error::<T>::InsufficientShares)?;
                    match outcome {
                        BinaryOutcome::Yes => {
                            ensure!(
                                entry.yes_shares >= shares_in,
                                Error::<T>::InsufficientShares
                            );
                            entry.yes_shares = entry.yes_shares.saturating_sub(shares_in);
                        }
                        BinaryOutcome::No => {
                            ensure!(entry.no_shares >= shares_in, Error::<T>::InsufficientShares);
                            entry.no_shares = entry.no_shares.saturating_sub(shares_in);
                        }
                    }
                    entry.net_collateral_paid =
                        entry.net_collateral_paid.saturating_sub(basis_reduction);
                    if entry.yes_shares.is_zero()
                        && entry.no_shares.is_zero()
                        && entry.net_collateral_paid.is_zero()
                    {
                        *position = None;
                    }
                    Ok(())
                },
            )?;
            MarketPositionTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.total_yes_shares = totals
                            .total_yes_shares
                            .checked_sub(&shares_in)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        totals.total_no_shares = totals
                            .total_no_shares
                            .checked_sub(&shares_in)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                }
                totals.total_net_collateral_paid = totals
                    .total_net_collateral_paid
                    .checked_sub(&basis_reduction)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;
            DpmCostBasisByAccount::<T>::try_mutate_exists(
                market_id,
                who,
                |basis| -> DispatchResult {
                    let Some(entry) = basis.as_mut() else {
                        return Ok(());
                    };
                    match outcome {
                        BinaryOutcome::Yes => {
                            entry.yes = entry.yes.saturating_sub(basis_reduction);
                        }
                        BinaryOutcome::No => {
                            entry.no = entry.no.saturating_sub(basis_reduction);
                        }
                    }
                    if entry.yes.is_zero() && entry.no.is_zero() {
                        *basis = None;
                    }
                    Ok(())
                },
            )?;
            DpmCostBasisTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.yes = totals.yes.saturating_sub(basis_reduction);
                    }
                    BinaryOutcome::No => {
                        totals.no = totals.no.saturating_sub(basis_reduction);
                    }
                }
                Ok(())
            })
        }

        fn remove_dpm_cost_basis(
            market_id: MarketId,
            who: &T::AccountId,
        ) -> Result<DpmCostBasisOf<T>, DispatchError> {
            let basis = DpmCostBasisByAccount::<T>::take(market_id, who).unwrap_or_default();
            DpmCostBasisTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                totals.yes = totals
                    .yes
                    .checked_sub(&basis.yes)
                    .ok_or(Error::<T>::Overflow)?;
                totals.no = totals
                    .no
                    .checked_sub(&basis.no)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;
            Ok(basis)
        }

        fn dpm_cost_basis_sum(basis: &DpmCostBasisOf<T>) -> Result<T::Balance, DispatchError> {
            basis
                .yes
                .checked_add(&basis.no)
                .ok_or(Error::<T>::Overflow.into())
        }

        fn burn_dpm_residual_if_no_winners(
            market_id: MarketId,
            outcome: BinaryOutcome,
        ) -> DispatchResult {
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            if !matches!(market.mechanism, MarketMechanism::DynamicPariMutuel) {
                return Ok(());
            }
            let totals = MarketPositionTotals::<T>::get(market_id);
            let winning_shares = match outcome {
                BinaryOutcome::Yes => totals.total_yes_shares,
                BinaryOutcome::No => totals.total_no_shares,
            };
            if !winning_shares.is_zero() {
                return Ok(());
            }
            let amount = MarketDpmCollateral::<T>::take(market_id);
            if !amount.is_zero() {
                PendingXorBuybackCollateral::<T>::mutate(|total| {
                    *total = total.saturating_add(amount);
                });
                Self::deposit_event(Event::DpmResidualBurned { market_id, amount });
            }
            Ok(())
        }

        fn sqrt_u256_floor(value: U256) -> U256 {
            if value <= U256::one() {
                return value;
            }
            let mut low = U256::one();
            let mut high = value;
            let mut answer = U256::one();
            while low <= high {
                let mid = low + ((high - low) / U256::from(2u8));
                match mid.checked_mul(mid) {
                    Some(square) if square <= value => {
                        answer = mid;
                        low = mid + U256::one();
                    }
                    _ => {
                        high = mid.saturating_sub(U256::one());
                    }
                }
            }
            answer
        }

        fn u256_to_u32_saturating(value: U256) -> u32 {
            u128::try_from(value)
                .unwrap_or(u128::MAX)
                .min(u32::MAX as u128) as u32
        }

        fn ratio_bps(numerator: U256, denominator: U256) -> u32 {
            if denominator.is_zero() {
                return 0;
            }
            let value = numerator
                .checked_mul(U256::from(BPS_DENOMINATOR))
                .unwrap_or(U256::MAX)
                / denominator;
            Self::u256_to_u32_saturating(value)
        }

        fn ensure_position_can_credit(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares: T::Balance,
            collateral_paid: T::Balance,
        ) -> DispatchResult {
            let position = MarketPositions::<T>::get(market_id, who).unwrap_or_default();
            match outcome {
                BinaryOutcome::Yes => {
                    position
                        .yes_shares
                        .checked_add(&shares)
                        .ok_or(Error::<T>::Overflow)?;
                }
                BinaryOutcome::No => {
                    position
                        .no_shares
                        .checked_add(&shares)
                        .ok_or(Error::<T>::Overflow)?;
                }
            }
            position
                .net_collateral_paid
                .checked_add(&collateral_paid)
                .ok_or(Error::<T>::Overflow)?;

            let totals = MarketPositionTotals::<T>::get(market_id);
            match outcome {
                BinaryOutcome::Yes => {
                    totals
                        .total_yes_shares
                        .checked_add(&shares)
                        .ok_or(Error::<T>::Overflow)?;
                }
                BinaryOutcome::No => {
                    totals
                        .total_no_shares
                        .checked_add(&shares)
                        .ok_or(Error::<T>::Overflow)?;
                }
            }
            totals
                .total_net_collateral_paid
                .checked_add(&collateral_paid)
                .ok_or(Error::<T>::Overflow)?;
            Ok(())
        }

        fn credit_position_on_buy(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares: T::Balance,
            collateral_paid: T::Balance,
        ) -> DispatchResult {
            MarketPositions::<T>::try_mutate(market_id, who, |position| -> DispatchResult {
                let entry = position.get_or_insert_with(Default::default);
                match outcome {
                    BinaryOutcome::Yes => {
                        entry.yes_shares = entry
                            .yes_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        entry.no_shares = entry
                            .no_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                }
                entry.net_collateral_paid = entry
                    .net_collateral_paid
                    .checked_add(&collateral_paid)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(())
            })?;
            MarketPositionTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.total_yes_shares = totals
                            .total_yes_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        totals.total_no_shares = totals
                            .total_no_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                }
                totals.total_net_collateral_paid = totals
                    .total_net_collateral_paid
                    .checked_add(&collateral_paid)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(())
            })
        }

        fn ensure_position_has_shares(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares: T::Balance,
        ) -> DispatchResult {
            let Some(position) = MarketPositions::<T>::get(market_id, who) else {
                return Err(Error::<T>::InsufficientShares.into());
            };
            let balance = match outcome {
                BinaryOutcome::Yes => position.yes_shares,
                BinaryOutcome::No => position.no_shares,
            };
            ensure!(balance >= shares, Error::<T>::InsufficientShares);
            Ok(())
        }

        fn debit_market_totals(
            market_id: MarketId,
            position: &MarketPositionOf<T>,
        ) -> DispatchResult {
            MarketPositionTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                totals.total_yes_shares = totals
                    .total_yes_shares
                    .checked_sub(&position.yes_shares)
                    .ok_or(Error::<T>::Overflow)?;
                totals.total_no_shares = totals
                    .total_no_shares
                    .checked_sub(&position.no_shares)
                    .ok_or(Error::<T>::Overflow)?;
                totals.total_net_collateral_paid = totals
                    .total_net_collateral_paid
                    .checked_sub(&position.net_collateral_paid)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(())
            })
        }

        fn winning_shares(position: &MarketPositionOf<T>, outcome: BinaryOutcome) -> T::Balance {
            match outcome {
                BinaryOutcome::Yes => position.yes_shares,
                BinaryOutcome::No => position.no_shares,
            }
        }

        fn pro_rata(
            amount: T::Balance,
            numerator: T::Balance,
            denominator: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(!denominator.is_zero(), Error::<T>::Overflow);
            let value = U256::from(amount.saturated_into::<u128>())
                .checked_mul(U256::from(numerator.saturated_into::<u128>()))
                .ok_or(Error::<T>::Overflow)?
                / U256::from(denominator.saturated_into::<u128>());
            Self::u256_to_balance(value)
        }

        fn u256_to_balance(value: U256) -> Result<T::Balance, DispatchError> {
            let raw = u128::try_from(value).map_err(|_| Error::<T>::Overflow)?;
            Ok(raw.saturated_into::<T::Balance>())
        }

        fn validate_condition_metadata(
            metadata: ConditionInput,
        ) -> Result<ConditionMetadataOf<T>, DispatchError> {
            ensure!(
                metadata.question.len() as u32 >= T::MinQuestionLength::get(),
                Error::<T>::QuestionTooShort
            );
            ensure!(
                !metadata.oracle.is_empty() && !metadata.resolution_source.is_empty(),
                Error::<T>::InvalidMetadata
            );
            ensure!(
                core::str::from_utf8(&metadata.question).is_ok()
                    && core::str::from_utf8(&metadata.oracle).is_ok()
                    && core::str::from_utf8(&metadata.resolution_source).is_ok(),
                Error::<T>::InvalidMetadata
            );
            Ok(ConditionMetadata {
                question: MetadataString::<T>::try_from(metadata.question)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
                oracle: MetadataString::<T>::try_from(metadata.oracle)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
                resolution_source: MetadataString::<T>::try_from(metadata.resolution_source)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
            })
        }

        fn create_condition_entry(
            who: &T::AccountId,
            metadata: ConditionMetadataOf<T>,
        ) -> Result<ConditionId, DispatchError> {
            let condition_id = NextConditionId::<T>::try_mutate(
                |next_id| -> Result<ConditionId, DispatchError> {
                    let id = *next_id;
                    *next_id = next_id
                        .checked_add(One::one())
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(id)
                },
            )?;

            Conditions::<T>::insert(condition_id, metadata);
            ConditionCreators::<T>::insert(condition_id, who.clone());
            Self::deposit_event(Event::ConditionCreated { condition_id });
            Ok(condition_id)
        }

        fn ensure_market_close_block(close_block: BlockNumberFor<T>) -> DispatchResult {
            let now = <frame_system::Pallet<T>>::block_number();
            let min_close = now
                .checked_add(&T::MinMarketDuration::get())
                .ok_or(Error::<T>::Overflow)?;
            ensure!(close_block >= min_close, Error::<T>::MarketDurationTooShort);
            Ok(())
        }

        fn create_market_entry(
            who: &T::AccountId,
            condition_id: ConditionId,
            close_block: BlockNumberFor<T>,
        ) -> Result<MarketId, DispatchError> {
            ensure!(
                !ConditionMarket::<T>::contains_key(condition_id),
                Error::<T>::ConditionAlreadyUsed
            );
            let market_id =
                NextMarketId::<T>::try_mutate(|next_id| -> Result<MarketId, DispatchError> {
                    let id = *next_id;
                    *next_id = next_id
                        .checked_add(One::one())
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(id)
                })?;

            let data = Market {
                creator: who.clone(),
                condition_id,
                close_block,
                collateral_asset: T::CanonicalStableAssetId::get(),
                seed_liquidity: T::Balance::zero(),
                mechanism: MarketMechanism::DynamicPariMutuel,
                status: MarketStatus::Open,
            };
            Markets::<T>::insert(market_id, data);
            ConditionMarket::<T>::insert(condition_id, market_id);
            Self::deposit_event(Event::MarketCreated {
                market_id,
                seed_liquidity: T::Balance::zero(),
            });
            Ok(market_id)
        }

        fn ensure_next_market_id_available() -> DispatchResult {
            NextMarketId::<T>::get()
                .checked_add(One::one())
                .ok_or(Error::<T>::Overflow)?;
            Ok(())
        }

        fn ensure_next_condition_id_available() -> Result<ConditionId, DispatchError> {
            let condition_id = NextConditionId::<T>::get();
            condition_id
                .checked_add(One::one())
                .ok_or(Error::<T>::Overflow)?;
            Ok(condition_id)
        }
    }
}

pub mod migrations {
    use super::*;

    pub(crate) const MAX_LEGACY_OPENGOV_CONDITIONS: u32 = 1024;
    pub(crate) const MAX_LEGACY_GOVERNANCE_BONDS: u32 = 16;
    pub(crate) const MAX_LEGACY_CREATOR_LOCKED_BONDS: u32 = 1024;
    pub(crate) const MAX_LEGACY_MARKET_BOND_LOCKS: u32 = 1024;
    pub(crate) const MAX_LEGACY_GOVERNANCE_BOND_CONFIGS: u32 = 16;
    pub(crate) const MAX_LEGACY_MARKETS: u32 = 1024;

    #[derive(
        Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, Debug, MaxEncodedLen,
    )]
    pub struct LegacyMarket<ClassId, AccountId, BlockNumber, Balance> {
        pub creator: AccountId,
        pub condition_id: ConditionId,
        pub close_block: BlockNumber,
        pub collateral_asset: ClassId,
        pub seed_liquidity: Balance,
        pub status: MarketStatus,
    }

    fn ensure_items_within_limit<I>(items: I, limit: u32, label: &str) -> u64
    where
        I: IntoIterator,
    {
        let mut count = 0u64;
        for _ in items.into_iter().take(limit as usize + 1) {
            count = count.saturating_add(1);
        }
        if count > limit.into() {
            panic!("Polkamarkt migration {label} exceeds limit {limit}");
        }
        count
    }

    fn count_raw_prefix_keys(prefix: &[u8], limit: u32) -> u64 {
        let mut previous_key = prefix.to_vec();
        let mut count = 0u64;
        while let Some(next_key) = sp_io::storage::next_key(&previous_key) {
            if !next_key.starts_with(prefix) {
                break;
            }
            count = count.saturating_add(1);
            if count > limit.into() {
                break;
            }
            previous_key = next_key;
        }
        count
    }

    fn ensure_raw_prefix_within_limit(prefix: &[u8], limit: u32, label: &str) -> u64 {
        let count = count_raw_prefix_keys(prefix, limit);
        if count > limit.into() {
            panic!("Polkamarkt migration {label} exceeds limit {limit}");
        }
        count
    }

    fn clear_raw_prefix_with_limit(prefix: &[u8], limit: u32, label: &str) -> u64 {
        let result = frame_support::storage::unhashed::clear_prefix(prefix, Some(limit), None);
        if result.maybe_cursor.is_some() {
            panic!("Polkamarkt migration {label} clear exceeded limit {limit}");
        }
        result.unique.into()
    }

    pub mod v2 {
        use super::super::*;
        use frame_support::{
            storage::storage_prefix,
            traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion},
        };
        use sp_core::Get;

        pub struct Migrate<T>(PhantomData<T>);

        impl<T: Config> OnRuntimeUpgrade for Migrate<T> {
            fn on_runtime_upgrade() -> Weight {
                let db_weight = T::DbWeight::get();
                let on_chain = Pallet::<T>::on_chain_storage_version();
                let mut weight = db_weight.reads(1);
                let prefix = storage_prefix(b"Polkamarkt", b"OpengovConditions");
                let scanned = super::ensure_raw_prefix_within_limit(
                    &prefix,
                    super::MAX_LEGACY_OPENGOV_CONDITIONS,
                    "OpengovConditions",
                );
                weight.saturating_accrue(db_weight.reads(scanned));
                let removed = super::clear_raw_prefix_with_limit(
                    &prefix,
                    super::MAX_LEGACY_OPENGOV_CONDITIONS,
                    "OpengovConditions",
                );
                weight.saturating_accrue(db_weight.writes(removed));

                if on_chain < StorageVersion::new(2) {
                    StorageVersion::new(2).put::<Pallet<T>>();
                    weight.saturating_accrue(db_weight.writes(1));
                }

                weight
            }
        }
    }

    pub mod v3 {
        use super::super::*;
        use frame_support::{
            __private::log,
            pallet_prelude::{Blake2_128Concat, OptionQuery, ValueQuery},
            storage::storage_prefix,
            traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion},
        };
        use sp_core::Get;

        #[frame_support::storage_alias]
        pub type GovernanceBonds<T: Config> = StorageMap<
            Pallet<T>,
            Blake2_128Concat,
            <T as frame_system::Config>::AccountId,
            <T as Config>::Balance,
            ValueQuery,
        >;

        #[frame_support::storage_alias]
        pub type CreatorLockedBond<T: Config> = StorageMap<
            Pallet<T>,
            Blake2_128Concat,
            <T as frame_system::Config>::AccountId,
            <T as Config>::Balance,
            ValueQuery,
        >;

        #[frame_support::storage_alias]
        pub type MarketBondLock<T: Config> =
            StorageMap<Pallet<T>, Blake2_128Concat, MarketId, <T as Config>::Balance, OptionQuery>;

        #[frame_support::storage_alias]
        pub type GovernanceBondMinimumOverride<T: Config> =
            StorageValue<Pallet<T>, <T as Config>::Balance, OptionQuery>;

        pub struct Migrate<T>(PhantomData<T>);

        struct MigrationStats {
            refunded_accounts: u64,
            cleared_bonds: u64,
            cleared_locks: u64,
            cleared_market_locks: u64,
            cleared_config: u64,
        }

        fn clear_prefix_for(storage_item: &[u8], limit: u32, label: &str) -> u64 {
            let prefix = storage_prefix(b"Polkamarkt", storage_item);
            super::clear_raw_prefix_with_limit(&prefix, limit, label)
        }

        impl<T: Config> OnRuntimeUpgrade for Migrate<T> {
            fn on_runtime_upgrade() -> Weight {
                let db_weight = T::DbWeight::get();
                let on_chain = Pallet::<T>::on_chain_storage_version();
                if on_chain >= StorageVersion::new(3) {
                    return db_weight.reads(1);
                }
                if on_chain < StorageVersion::new(2) {
                    panic!(
                        "Polkamarkt v3 migration requires storage version at least 2, found {on_chain:?}"
                    );
                }

                super::ensure_items_within_limit(
                    GovernanceBonds::<T>::iter_keys(),
                    super::MAX_LEGACY_GOVERNANCE_BONDS,
                    "GovernanceBonds",
                );
                super::ensure_items_within_limit(
                    CreatorLockedBond::<T>::iter_keys(),
                    super::MAX_LEGACY_CREATOR_LOCKED_BONDS,
                    "CreatorLockedBond",
                );
                super::ensure_items_within_limit(
                    MarketBondLock::<T>::iter_keys(),
                    super::MAX_LEGACY_MARKET_BOND_LOCKS,
                    "MarketBondLock",
                );
                let config_prefix = storage_prefix(b"Polkamarkt", b"GovernanceBondMinimumOverride");
                super::ensure_raw_prefix_within_limit(
                    &config_prefix,
                    super::MAX_LEGACY_GOVERNANCE_BOND_CONFIGS,
                    "GovernanceBondMinimumOverride",
                );

                let migration_result =
                    common::with_transaction(|| -> Result<MigrationStats, DispatchError> {
                        let legacy_escrow = T::LegacyCreatorBondEscrowAccount::get();
                        let canonical_asset = T::CanonicalStableAssetId::get();
                        let mut refunded_accounts = 0u64;
                        let mut cleared_bonds = 0u64;

                        for (account, amount) in GovernanceBonds::<T>::drain() {
                            cleared_bonds = cleared_bonds.saturating_add(1);
                            if amount.is_zero() {
                                continue;
                            }
                            T::Assets::transfer(canonical_asset, &legacy_escrow, &account, amount)?;
                            refunded_accounts = refunded_accounts.saturating_add(1);
                        }

                        let cleared_bond_remainders = clear_prefix_for(
                            b"GovernanceBonds",
                            super::MAX_LEGACY_GOVERNANCE_BONDS,
                            "GovernanceBonds",
                        );
                        let cleared_locks = clear_prefix_for(
                            b"CreatorLockedBond",
                            super::MAX_LEGACY_CREATOR_LOCKED_BONDS,
                            "CreatorLockedBond",
                        );
                        let cleared_market_locks = clear_prefix_for(
                            b"MarketBondLock",
                            super::MAX_LEGACY_MARKET_BOND_LOCKS,
                            "MarketBondLock",
                        );
                        let cleared_config = clear_prefix_for(
                            b"GovernanceBondMinimumOverride",
                            super::MAX_LEGACY_GOVERNANCE_BOND_CONFIGS,
                            "GovernanceBondMinimumOverride",
                        );

                        StorageVersion::new(3).put::<Pallet<T>>();

                        Ok(MigrationStats {
                            refunded_accounts,
                            cleared_bonds: cleared_bonds.saturating_add(cleared_bond_remainders),
                            cleared_locks,
                            cleared_market_locks,
                            cleared_config,
                        })
                    });

                match migration_result {
                    Ok(stats) => {
                        let MigrationStats {
                            refunded_accounts,
                            cleared_bonds,
                            cleared_locks,
                            cleared_market_locks,
                            cleared_config,
                        } = stats;
                        log::info!(
                            "Polkamarkt v3 migration refunded {refunded_accounts} legacy bond accounts and cleared {cleared_bonds} bond, {cleared_locks} creator-lock, {cleared_market_locks} market-lock, {cleared_config} config entries",
                        );
                    }
                    Err(error) => {
                        log::error!(
                            "Polkamarkt v3 migration failed and was rolled back: {error:?}",
                        );
                        panic!("Polkamarkt v3 migration failed and was rolled back: {error:?}");
                    }
                }

                <T as frame_system::Config>::BlockWeights::get().max_block
            }
        }
    }

    pub mod v4 {
        use super::super::*;
        use frame_support::{
            __private::log,
            pallet_prelude::{Blake2_128Concat, OptionQuery},
            traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion},
        };
        use sp_core::Get;

        #[frame_support::storage_alias]
        pub type Markets<T: Config> = StorageMap<
            Pallet<T>,
            Blake2_128Concat,
            MarketId,
            super::LegacyMarket<
                <T as Config>::AssetId,
                <T as frame_system::Config>::AccountId,
                BlockNumberFor<T>,
                <T as Config>::Balance,
            >,
            OptionQuery,
        >;

        pub struct Migrate<T>(PhantomData<T>);

        impl<T: Config> OnRuntimeUpgrade for Migrate<T> {
            fn on_runtime_upgrade() -> Weight {
                let db_weight = T::DbWeight::get();
                let on_chain = Pallet::<T>::on_chain_storage_version();
                if on_chain >= StorageVersion::new(4) {
                    return db_weight.reads(1);
                }
                if on_chain != StorageVersion::new(3) {
                    panic!(
                        "Polkamarkt v4 migration requires storage version 3, found {on_chain:?}"
                    );
                }

                let preflight_reads = super::ensure_items_within_limit(
                    self::Markets::<T>::iter_keys(),
                    super::MAX_LEGACY_MARKETS,
                    "Markets",
                );
                let mut scanned_markets = 0u64;
                let mut totals_reads = 0u64;
                let mut seeded_markets = 0u64;
                for (market_id, market) in self::Markets::<T>::iter() {
                    scanned_markets = scanned_markets.saturating_add(1);
                    if market.seed_liquidity.is_zero() {
                        continue;
                    }

                    totals_reads = totals_reads.saturating_add(1);
                    if !LiquidityPositionTotals::<T>::get(market_id)
                        .total_shares
                        .is_zero()
                    {
                        continue;
                    }

                    LiquidityPositions::<T>::insert(
                        market_id,
                        &market.creator,
                        LiquidityPosition {
                            shares: market.seed_liquidity,
                            collateral_contributed: market.seed_liquidity,
                        },
                    );
                    LiquidityPositionTotals::<T>::insert(
                        market_id,
                        LiquidityTotals {
                            total_shares: market.seed_liquidity,
                            total_collateral_contributed: market.seed_liquidity,
                        },
                    );
                    seeded_markets = seeded_markets.saturating_add(1);
                }

                StorageVersion::new(4).put::<Pallet<T>>();
                log::info!(
                    "Polkamarkt v4 migration initialized locked LP shares for {seeded_markets} of {scanned_markets} markets",
                );
                db_weight.reads_writes(
                    preflight_reads
                        .saturating_add(scanned_markets)
                        .saturating_add(totals_reads)
                        .saturating_add(1),
                    seeded_markets.saturating_mul(2).saturating_add(1),
                )
            }
        }
    }

    pub mod v5 {
        use super::super::*;
        #[cfg(feature = "try-runtime")]
        use codec::{Decode, Encode};
        use frame_support::{
            __private::log,
            pallet_prelude::{Blake2_128Concat, OptionQuery},
            traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion},
        };
        use sp_core::Get;
        #[cfg(feature = "try-runtime")]
        use sp_runtime::TryRuntimeError;

        #[frame_support::storage_alias]
        pub type Markets<T: Config> = StorageMap<
            Pallet<T>,
            Blake2_128Concat,
            MarketId,
            super::LegacyMarket<
                <T as Config>::AssetId,
                <T as frame_system::Config>::AccountId,
                BlockNumberFor<T>,
                <T as Config>::Balance,
            >,
            OptionQuery,
        >;

        pub struct Migrate<T>(PhantomData<T>);

        #[cfg(feature = "try-runtime")]
        #[derive(Decode, Encode)]
        struct V5PreUpgradeState {
            previous_storage_version: StorageVersion,
            market_count: u64,
        }

        #[cfg(feature = "try-runtime")]
        fn v5_try_runtime_error(message: &'static str) -> TryRuntimeError {
            TryRuntimeError::Other(message)
        }

        impl<T: Config> OnRuntimeUpgrade for Migrate<T> {
            fn on_runtime_upgrade() -> Weight {
                let db_weight = T::DbWeight::get();
                let on_chain = Pallet::<T>::on_chain_storage_version();
                if on_chain >= StorageVersion::new(5) {
                    return db_weight.reads(1);
                }
                if on_chain != StorageVersion::new(4) {
                    panic!(
                        "Polkamarkt v5 migration requires storage version 4, found {on_chain:?}"
                    );
                }

                let preflight_reads = super::ensure_items_within_limit(
                    self::Markets::<T>::iter_keys(),
                    super::MAX_LEGACY_MARKETS,
                    "Markets",
                );
                let mut migrated = 0u64;
                let legacy_markets = self::Markets::<T>::iter().collect::<Vec<_>>();
                for (market_id, market) in legacy_markets {
                    super::super::Markets::<T>::insert(
                        market_id,
                        Market {
                            creator: market.creator,
                            condition_id: market.condition_id,
                            close_block: market.close_block,
                            collateral_asset: market.collateral_asset,
                            seed_liquidity: market.seed_liquidity,
                            mechanism: MarketMechanism::LegacyAmm,
                            status: market.status,
                        },
                    );
                    migrated = migrated.saturating_add(1);
                }

                StorageVersion::new(5).put::<Pallet<T>>();
                log::info!("Polkamarkt v5 migration marked {migrated} markets as LegacyAmm");
                db_weight.reads_writes(
                    preflight_reads.saturating_add(migrated).saturating_add(1),
                    migrated.saturating_add(1),
                )
            }

            #[cfg(feature = "try-runtime")]
            fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
                let previous_storage_version = Pallet::<T>::on_chain_storage_version();
                let market_count = if previous_storage_version == StorageVersion::new(4) {
                    super::ensure_items_within_limit(
                        self::Markets::<T>::iter_keys(),
                        super::MAX_LEGACY_MARKETS,
                        "Markets",
                    )
                } else {
                    0
                };

                Ok(V5PreUpgradeState {
                    previous_storage_version,
                    market_count,
                }
                .encode())
            }

            #[cfg(feature = "try-runtime")]
            fn post_upgrade(state: Vec<u8>) -> Result<(), TryRuntimeError> {
                let state = V5PreUpgradeState::decode(&mut &state[..]).map_err(|_| {
                    v5_try_runtime_error("Polkamarkt v5 failed to decode pre-upgrade state")
                })?;
                let actual_storage_version = Pallet::<T>::on_chain_storage_version();

                if state.previous_storage_version == StorageVersion::new(4) {
                    if actual_storage_version != StorageVersion::new(5) {
                        return Err(v5_try_runtime_error(
                            "Polkamarkt v5 did not set storage version to 5",
                        ));
                    }

                    let mut migrated = 0u64;
                    for (_, market) in super::super::Markets::<T>::iter() {
                        if market.mechanism != MarketMechanism::LegacyAmm {
                            return Err(v5_try_runtime_error(
                                "Polkamarkt v5 migrated a market without LegacyAmm mechanism",
                            ));
                        }
                        migrated = migrated.saturating_add(1);
                    }

                    if migrated != state.market_count {
                        return Err(v5_try_runtime_error(
                            "Polkamarkt v5 migrated market count mismatch",
                        ));
                    }
                } else if state.previous_storage_version >= StorageVersion::new(5)
                    && actual_storage_version != state.previous_storage_version
                {
                    return Err(v5_try_runtime_error(
                        "Polkamarkt v5 changed storage version for a no-op migration",
                    ));
                }

                Ok(())
            }
        }
    }

    pub mod v6 {
        use super::super::*;
        use frame_support::{
            __private::log,
            ensure,
            storage::storage_prefix,
            traits::{GetStorageVersion as _, OnRuntimeUpgrade, StorageVersion},
        };
        use sp_core::Get;
        use sp_std::{collections::btree_map::BTreeMap, vec::Vec};

        const MAX_LEGACY_ORDERS: u32 = 16_384;
        const MAX_LEGACY_ORDER_INDEXES: u32 = 65_536;
        const MAX_LEGACY_POSITIONS: u32 = 16_384;
        const MAX_LEGACY_LIQUIDITY_POSITIONS: u32 = 16_384;
        const MAX_LEGACY_POOLS: u32 = 1_024;

        pub struct Migrate<T>(PhantomData<T>);
        type OrdersByMarket<T> = BTreeMap<MarketId, Vec<OrderOf<T>>>;

        fn add_payout<T: Config>(
            market_id: MarketId,
            account: &T::AccountId,
            amount: T::Balance,
        ) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            MigratedLegacyPayouts::<T>::try_mutate(market_id, account, |payout| -> DispatchResult {
                *payout = payout.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })
        }

        fn clear_storage<T: Config>(storage_item: &[u8], limit: u32, label: &str) -> u64 {
            let prefix = storage_prefix(b"Polkamarkt", storage_item);
            super::clear_raw_prefix_with_limit(&prefix, limit, label)
        }

        fn drain_orders_by_market<T: Config>() -> (OrdersByMarket<T>, u64) {
            let mut orders_by_market = OrdersByMarket::<T>::new();
            let mut drained = 0u64;
            for (_, order) in Orders::<T>::drain() {
                drained = drained.saturating_add(1);
                orders_by_market
                    .entry(order.market_id)
                    .or_default()
                    .push(order);
            }
            (orders_by_market, drained)
        }

        fn migrate_orderbook_market<T: Config>(
            market_id: MarketId,
            _market: &MarketOf<T>,
            orders_by_market: &mut OrdersByMarket<T>,
        ) -> Result<T::Balance, DispatchError> {
            let mut position_payouts = T::Balance::zero();
            if let Some(orders) = orders_by_market.remove(&market_id) {
                for order in orders {
                    match order.side {
                        OrderSide::Buy => {
                            add_payout::<T>(market_id, &order.owner, order.reserved_collateral)?;
                        }
                        OrderSide::Sell => {
                            credit_outcome_shares::<T>(
                                market_id,
                                &order.owner,
                                order.outcome,
                                order.remaining_shares,
                            )?;
                        }
                    }
                }
            }

            let positions = MarketPositions::<T>::iter_prefix(market_id).collect::<Vec<_>>();
            for (account, position) in positions {
                let payout = cancelled_order_book_payout::<T>(&position)?;
                position_payouts = position_payouts
                    .checked_add(&payout)
                    .ok_or(Error::<T>::Overflow)?;
                add_payout::<T>(market_id, &account, payout)?;
                MarketPositions::<T>::remove(market_id, &account);
            }
            MarketPositionTotals::<T>::remove(market_id);

            let locked = MarketOrderBookCollateral::<T>::take(market_id);
            if locked > position_payouts {
                Ok(locked.saturating_sub(position_payouts))
            } else {
                Ok(T::Balance::zero())
            }
        }

        fn migrate_amm_market<T: Config>(
            market_id: MarketId,
            market: &MarketOf<T>,
        ) -> DispatchResult {
            let pool = MarketPools::<T>::get(market_id).unwrap_or(MarketPool {
                collateral: T::Balance::zero(),
                yes: T::Balance::zero(),
                no: T::Balance::zero(),
            });
            let totals = MarketPositionTotals::<T>::get(market_id);
            let total_no_payout = core::cmp::min(pool.collateral, totals.total_no_shares);
            let lp_claimable = pool.collateral.saturating_sub(total_no_payout);

            let positions = MarketPositions::<T>::iter_prefix(market_id).collect::<Vec<_>>();
            for (account, position) in positions {
                add_payout::<T>(market_id, &account, position.no_shares)?;
                MarketPositions::<T>::remove(market_id, &account);
            }
            MarketPositionTotals::<T>::remove(market_id);

            let liquidity_totals = LiquidityPositionTotals::<T>::get(market_id);
            let liquidity_positions =
                LiquidityPositions::<T>::iter_prefix(market_id).collect::<Vec<_>>();
            let mut remaining_lp_claimable = lp_claimable;
            let lp_count = liquidity_positions.len();
            for (index, (account, position)) in liquidity_positions.into_iter().enumerate() {
                let payout = if liquidity_totals.total_shares.is_zero() {
                    T::Balance::zero()
                } else if index + 1 == lp_count {
                    remaining_lp_claimable
                } else {
                    pro_rata::<T>(lp_claimable, position.shares, liquidity_totals.total_shares)?
                };
                remaining_lp_claimable = remaining_lp_claimable.saturating_sub(payout);
                add_payout::<T>(market_id, &account, payout)?;
                LiquidityPositions::<T>::remove(market_id, &account);
            }
            LiquidityPositionTotals::<T>::remove(market_id);
            MarketPools::<T>::remove(market_id);
            MarketResolution::<T>::insert(market_id, BinaryOutcome::No);
            let _ = market;
            Ok(())
        }

        fn credit_outcome_shares<T: Config>(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares: T::Balance,
        ) -> DispatchResult {
            if shares.is_zero() {
                return Ok(());
            }
            MarketPositions::<T>::try_mutate(market_id, who, |position| -> DispatchResult {
                let entry = position.get_or_insert_with(Default::default);
                match outcome {
                    BinaryOutcome::Yes => {
                        entry.yes_shares = entry
                            .yes_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        entry.no_shares = entry
                            .no_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                }
                Ok(())
            })?;
            MarketPositionTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.total_yes_shares = totals
                            .total_yes_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        totals.total_no_shares = totals
                            .total_no_shares
                            .checked_add(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                }
                Ok(())
            })
        }

        fn cancelled_order_book_payout<T: Config>(
            position: &MarketPositionOf<T>,
        ) -> Result<T::Balance, DispatchError> {
            let total_shares = position
                .yes_shares
                .checked_add(&position.no_shares)
                .ok_or(Error::<T>::Overflow)?;
            pro_rata::<T>(total_shares, One::one(), 2u32.into())
        }

        fn pro_rata<T: Config>(
            amount: T::Balance,
            numerator: T::Balance,
            denominator: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(!denominator.is_zero(), Error::<T>::Overflow);
            let value = U256::from(amount.saturated_into::<u128>())
                .checked_mul(U256::from(numerator.saturated_into::<u128>()))
                .ok_or(Error::<T>::Overflow)?
                / U256::from(denominator.saturated_into::<u128>());
            let raw = u128::try_from(value).map_err(|_| Error::<T>::Overflow)?;
            Ok(raw.saturated_into::<T::Balance>())
        }

        impl<T: Config> OnRuntimeUpgrade for Migrate<T> {
            fn on_runtime_upgrade() -> Weight {
                let db_weight = T::DbWeight::get();
                let on_chain = Pallet::<T>::on_chain_storage_version();
                if on_chain >= StorageVersion::new(6) {
                    return db_weight.reads(1);
                }
                if on_chain != StorageVersion::new(5) {
                    panic!(
                        "Polkamarkt v6 migration requires storage version 5, found {on_chain:?}"
                    );
                }

                super::ensure_items_within_limit(
                    Markets::<T>::iter_keys(),
                    super::MAX_LEGACY_MARKETS,
                    "Markets",
                );
                super::ensure_items_within_limit(
                    Orders::<T>::iter_keys(),
                    MAX_LEGACY_ORDERS,
                    "Orders",
                );
                super::ensure_raw_prefix_within_limit(
                    &storage_prefix(b"Polkamarkt", b"MarketPositions"),
                    MAX_LEGACY_POSITIONS,
                    "MarketPositions",
                );
                super::ensure_raw_prefix_within_limit(
                    &storage_prefix(b"Polkamarkt", b"LiquidityPositions"),
                    MAX_LEGACY_LIQUIDITY_POSITIONS,
                    "LiquidityPositions",
                );

                let migration = common::with_transaction(
                    || -> Result<(u64, T::Balance), DispatchError> {
                        let mut migrated = 0u64;
                        let mut residual = T::Balance::zero();
                        let markets = Markets::<T>::iter().collect::<Vec<_>>();
                        let (mut orders_by_market, drained_orders) = drain_orders_by_market::<T>();

                        for (market_id, mut market) in markets {
                            match market.mechanism {
                                MarketMechanism::DynamicPariMutuel
                                | MarketMechanism::MigratedLegacy => {
                                    continue;
                                }
                                MarketMechanism::OrderBook => {
                                    residual = residual
                                        .checked_add(&migrate_orderbook_market::<T>(
                                            market_id,
                                            &market,
                                            &mut orders_by_market,
                                        )?)
                                        .ok_or(Error::<T>::Overflow)?;
                                    market.status = MarketStatus::Cancelled;
                                    market.mechanism = MarketMechanism::MigratedLegacy;
                                    MarketResolution::<T>::remove(market_id);
                                }
                                MarketMechanism::LegacyAmm => {
                                    migrate_amm_market::<T>(market_id, &market)?;
                                    market.status = MarketStatus::Resolved;
                                    market.mechanism = MarketMechanism::MigratedLegacy;
                                }
                            }
                            Markets::<T>::insert(market_id, market.clone());
                            Pallet::<T>::deposit_event(Event::LegacyMarketMigrated {
                                market_id,
                                status: market.status,
                            });
                            migrated = migrated.saturating_add(1);
                        }

                        let cleared_orders = drained_orders;
                        let cleared_queues = clear_storage::<T>(
                            b"OrderBookQueues",
                            MAX_LEGACY_ORDER_INDEXES,
                            "OrderBookQueues",
                        );
                        let cleared_levels = clear_storage::<T>(
                            b"OrderBookPriceLevels",
                            MAX_LEGACY_ORDER_INDEXES,
                            "OrderBookPriceLevels",
                        );
                        let cleared_open_orders = clear_storage::<T>(
                            b"OpenOrdersByAccountMarket",
                            MAX_LEGACY_ORDER_INDEXES,
                            "OpenOrdersByAccountMarket",
                        );
                        let cleared_order_collateral = clear_storage::<T>(
                            b"MarketOrderBookCollateral",
                            MAX_LEGACY_POOLS,
                            "MarketOrderBookCollateral",
                        );
                        let cleared_pools =
                            clear_storage::<T>(b"MarketPools", MAX_LEGACY_POOLS, "MarketPools");
                        let cleared_lp_positions = clear_storage::<T>(
                            b"LiquidityPositions",
                            MAX_LEGACY_LIQUIDITY_POSITIONS,
                            "LiquidityPositions",
                        );
                        let cleared_lp_totals = clear_storage::<T>(
                            b"LiquidityPositionTotals",
                            MAX_LEGACY_POOLS,
                            "LiquidityPositionTotals",
                        );
                        let cleared = cleared_orders
                            .saturating_add(cleared_queues)
                            .saturating_add(cleared_levels)
                            .saturating_add(cleared_open_orders)
                            .saturating_add(cleared_order_collateral)
                            .saturating_add(cleared_pools)
                            .saturating_add(cleared_lp_positions)
                            .saturating_add(cleared_lp_totals);

                        if !residual.is_zero() {
                            PendingXorBuybackCollateral::<T>::try_mutate(
                                |total| -> DispatchResult {
                                    *total =
                                        total.checked_add(&residual).ok_or(Error::<T>::Overflow)?;
                                    Ok(())
                                },
                            )?;
                            Pallet::<T>::deposit_event(Event::LegacyMigrationResidualRouted {
                                amount: residual,
                            });
                        }

                        StorageVersion::new(6).put::<Pallet<T>>();
                        log::info!(
                        "Polkamarkt v6 migrated {migrated} legacy markets, cleared {cleared} legacy entries, routed residual collateral",
                    );
                        Ok((migrated, residual))
                    },
                );

                if let Err(error) = migration {
                    panic!("Polkamarkt v6 migration failed and was rolled back: {error:?}");
                }

                <T as frame_system::Config>::BlockWeights::get().max_block
            }
        }
    }
}

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
