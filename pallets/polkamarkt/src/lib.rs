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
use sp_runtime::{DispatchError, Perbill, RuntimeDebug, TransactionOutcome};
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
    frame_support::traits::StorageVersion::new(5);
const CREATION_FEE_BUYBACK_BPS: u32 = 2_000;
const PRICE_CENTS_DENOMINATOR: u128 = 100;
const MIN_PRICE_CENTS: PriceCents = 1;
const MAX_PRICE_CENTS: PriceCents = 99;

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
    fn flip_position() -> Weight;
    fn add_liquidity() -> Weight;
    fn sync_market_status() -> Weight;
    fn resolve_market() -> Weight;
    fn resolve_market_with_evidence() -> Weight;
    fn cancel_market() -> Weight;
    fn emergency_cancel_market() -> Weight;
    fn claim_market() -> Weight;
    fn claim_markets(n: u32) -> Weight;
    fn claim_creator_fees() -> Weight;
    fn claim_creator_liquidity() -> Weight;
    fn claim_liquidity() -> Weight;
    fn sweep_xor_buyback_and_burn() -> Weight;
    fn place_order(f: u32) -> Weight;
    fn cancel_order() -> Weight;
    fn split_position() -> Weight;
    fn merge_positions() -> Weight;
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    MaxEncodedLen,
)]
pub struct ConditionMetadata<BoundedString> {
    pub question: BoundedString,
    pub oracle: BoundedString,
    pub resolution_source: BoundedString,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, Default,
)]
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
    RuntimeDebug,
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

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, Default,
)]
pub struct ConditionDetailsInput {
    pub category: Vec<u8>,
    pub tags: Vec<u8>,
    pub metadata_uri: Vec<u8>,
    pub metadata_hash: Option<[u8; 32]>,
    pub rules_uri: Vec<u8>,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, Default,
)]
pub struct EvidenceInput {
    pub uri: Vec<u8>,
    pub hash: Option<[u8; 32]>,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    MaxEncodedLen,
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
    RuntimeDebug,
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
    RuntimeDebug,
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
    RuntimeDebug,
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
    RuntimeDebug,
    MaxEncodedLen,
)]
pub enum TimeInForce {
    Gtc,
    Ioc,
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
    RuntimeDebug,
    MaxEncodedLen,
)]
pub enum MarketMechanism {
    LegacyAmm,
    OrderBook,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    MaxEncodedLen,
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
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    sp_runtime::RuntimeDebug,
    MaxEncodedLen,
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
    sp_runtime::RuntimeDebug,
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
    sp_runtime::RuntimeDebug,
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
    sp_runtime::RuntimeDebug,
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
    sp_runtime::RuntimeDebug,
    MaxEncodedLen,
    Default,
)]
pub struct LiquidityTotals<Balance> {
    pub total_shares: Balance,
    pub total_collateral_contributed: Balance,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    sp_runtime::RuntimeDebug,
    MaxEncodedLen,
)]
pub struct MarketEvidence<BlockNumber, BoundedString> {
    pub uri: BoundedString,
    pub hash: Option<[u8; 32]>,
    pub at_block: BlockNumber,
}

#[derive(
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    Clone,
    PartialEq,
    Eq,
    sp_runtime::RuntimeDebug,
    MaxEncodedLen,
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

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug, Default)]
pub struct OrderBookLevel<Balance> {
    pub price_cents: PriceCents,
    pub shares: Balance,
}

#[derive(Clone, PartialEq, Eq, RuntimeDebug, Default)]
pub struct OrderBookDepth<Balance> {
    pub bids: Vec<OrderBookLevel<Balance>>,
    pub asks: Vec<OrderBookLevel<Balance>>,
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct OrderQuote<Balance> {
    pub market_id: MarketId,
    pub outcome: BinaryOutcome,
    pub side: OrderSide,
    pub price_cents: PriceCents,
    pub shares: Balance,
    pub filled_shares: Balance,
    pub posted_shares: Balance,
    pub collateral_in: Balance,
    pub collateral_out: Balance,
    pub fee_amount: Balance,
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct BuyQuote<Balance> {
    pub market_id: MarketId,
    pub outcome: BinaryOutcome,
    pub collateral_in: Balance,
    pub fee_amount: Balance,
    pub pricing_collateral: Balance,
    pub shares_out: Balance,
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct SellQuote<Balance> {
    pub market_id: MarketId,
    pub outcome: BinaryOutcome,
    pub shares_in: Balance,
    pub gross_collateral_out: Balance,
    pub fee_amount: Balance,
    pub collateral_out: Balance,
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct LiquidityQuote<Balance> {
    pub market_id: MarketId,
    pub collateral_in: Balance,
    pub lp_shares_out: Balance,
    pub pool_collateral: Balance,
    pub total_lp_shares: Balance,
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
pub struct FlipQuote<Balance> {
    pub market_id: MarketId,
    pub from_outcome: BinaryOutcome,
    pub to_outcome: BinaryOutcome,
    pub shares_in: Balance,
    pub gross_collateral_out: Balance,
    pub sell_fee_amount: Balance,
    pub collateral_reinvested: Balance,
    pub buy_fee_amount: Balance,
    pub pricing_collateral: Balance,
    pub shares_out: Balance,
}

#[derive(Clone, PartialEq, Eq, RuntimeDebug)]
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
    pub open_yes_shares: Balance,
    pub open_no_shares: Balance,
    pub open_collateral: Balance,
    pub creator_fees: Balance,
    pub creator_liquidity: Balance,
    pub is_creator: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
struct TradeFeeSplit<Balance> {
    pool: Balance,
    creator: Balance,
    buyback: Balance,
}

#[derive(Clone, Copy, Default)]
struct OrderExecutionTotals<Balance> {
    filled_shares: Balance,
    collateral_in: Balance,
    collateral_out: Balance,
    fee_amount: Balance,
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
pub type LiquidityPositionOf<T> = LiquidityPosition<<T as Config>::Balance>;
pub type LiquidityTotalsOf<T> = LiquidityTotals<<T as Config>::Balance>;
pub type BuyQuoteOf<T> = BuyQuote<<T as Config>::Balance>;
pub type SellQuoteOf<T> = SellQuote<<T as Config>::Balance>;
pub type LiquidityQuoteOf<T> = LiquidityQuote<<T as Config>::Balance>;
pub type FlipQuoteOf<T> = FlipQuote<<T as Config>::Balance>;
pub type ClaimableInfoOf<T> =
    ClaimableInfo<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;
pub type OrderOf<T> = Order<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;
pub type OrderBookLevelOf<T> = OrderBookLevel<<T as Config>::Balance>;
pub type OrderBookDepthOf<T> = OrderBookDepth<<T as Config>::Balance>;
pub type OrderQuoteOf<T> = OrderQuote<<T as Config>::Balance>;

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
        CollateralSeeded {
            market_id: MarketId,
            amount: T::Balance,
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
        PositionFlipped {
            market_id: MarketId,
            trader: T::AccountId,
            from_outcome: BinaryOutcome,
            to_outcome: BinaryOutcome,
            shares_in: T::Balance,
            collateral_reinvested: T::Balance,
            shares_out: T::Balance,
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
        LiquidityAdded {
            market_id: MarketId,
            provider: T::AccountId,
            collateral_amount: T::Balance,
            lp_shares: T::Balance,
        },
        LiquidityClaimed {
            market_id: MarketId,
            provider: T::AccountId,
            lp_shares: T::Balance,
            amount: T::Balance,
        },
        CreatorFeesClaimed {
            market_id: MarketId,
            creator: T::AccountId,
            amount: T::Balance,
        },
        CreatorLiquidityClaimed {
            market_id: MarketId,
            creator: T::AccountId,
            amount: T::Balance,
        },
        XorBuybackSwept {
            collateral_amount: T::Balance,
            xor_burned: T::Balance,
        },
        HollarRouted {
            user: T::AccountId,
            amount: T::Balance,
        },
        OrderPlaced {
            order_id: OrderId,
            market_id: MarketId,
            owner: T::AccountId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
            time_in_force: TimeInForce,
        },
        OrderFilled {
            maker_order_id: OrderId,
            taker_order_id: OrderId,
            market_id: MarketId,
            maker: T::AccountId,
            taker: T::AccountId,
            outcome: BinaryOutcome,
            price_cents: PriceCents,
            shares: T::Balance,
            collateral_amount: T::Balance,
            fee_amount: T::Balance,
        },
        OrderCancelled {
            order_id: OrderId,
            market_id: MarketId,
            owner: T::AccountId,
        },
        PositionSplit {
            market_id: MarketId,
            account: T::AccountId,
            collateral_amount: T::Balance,
        },
        PositionMerged {
            market_id: MarketId,
            account: T::AccountId,
            shares: T::Balance,
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
        ZeroSeedLiquidity,
        InsufficientShares,
        InsufficientLiquidityShares,
        NotMarketCreator,
        NothingToClaim,
        NothingToSweep,
        UnsupportedMarketMechanism,
        InvalidPrice,
        TooManyOrdersAtPrice,
        TooManyOpenOrders,
        OrderUnknown,
        NotOrderOwner,
        InsufficientReservedCollateral,
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

        /// Create a market for a registered condition. New markets use the complete-set order book.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::create_market())]
        #[transactional]
        pub fn create_market(
            origin: OriginFor<T>,
            condition_id: ConditionId,
            close_block: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
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
            let now = <frame_system::Pallet<T>>::block_number();
            let min_close = now
                .checked_add(&T::MinMarketDuration::get())
                .ok_or(Error::<T>::Overflow)?;
            ensure!(close_block >= min_close, Error::<T>::MarketDurationTooShort);

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
                mechanism: MarketMechanism::OrderBook,
                status: MarketStatus::Open,
            };
            Markets::<T>::insert(market_id, data);
            ConditionMarket::<T>::insert(condition_id, market_id);
            Self::deposit_event(Event::MarketCreated {
                market_id,
                seed_liquidity: T::Balance::zero(),
            });
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
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_legacy_amm_market(&market)?;
            with_storage_transaction(|| -> DispatchResult {
                let total_fee = Self::trade_fee(collateral_in);
                let pricing_input = collateral_in
                    .checked_sub(&total_fee)
                    .ok_or(Error::<T>::TradeAmountTooSmall)?;
                ensure!(!pricing_input.is_zero(), Error::<T>::TradeAmountTooSmall);

                let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
                let share_amount = Self::quote_buy(&pool, outcome, pricing_input)?;
                ensure!(!share_amount.is_zero(), Error::<T>::TradeAmountTooSmall);
                ensure!(
                    share_amount >= min_shares_out,
                    Error::<T>::SlippageToleranceExceeded
                );
                Self::ensure_position_can_credit(
                    market_id,
                    &who,
                    outcome,
                    share_amount,
                    pricing_input,
                )?;
                let fee_split = Self::split_trade_fee(total_fee);
                let updated_pool =
                    Self::pool_after_buy(pool, outcome, pricing_input, fee_split.pool)?;

                T::Assets::transfer(
                    T::CanonicalStableAssetId::get(),
                    &who,
                    &Self::account_id(),
                    collateral_in,
                )?;
                Self::record_trade_fees(market_id, fee_split);
                MarketPools::<T>::insert(market_id, updated_pool);
                Self::record_market_volume(market_id, pricing_input);
                Self::credit_position_on_buy(
                    market_id,
                    &who,
                    outcome,
                    share_amount,
                    pricing_input,
                )?;

                Self::deposit_event(Event::TradeExecuted {
                    market_id,
                    trader: who.clone(),
                    side: TradeSide::Buy,
                    outcome,
                    collateral_amount: collateral_in,
                    share_amount,
                    fee_amount: total_fee,
                });
                Ok(())
            })
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
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_legacy_amm_market(&market)?;
            Self::ensure_position_has_shares(market_id, &who, outcome, shares_in)?;
            with_storage_transaction(|| -> DispatchResult {
                let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
                let gross_collateral_out = Self::quote_sell(&pool, outcome, shares_in)?;
                ensure!(
                    !gross_collateral_out.is_zero(),
                    Error::<T>::TradeAmountTooSmall
                );
                let total_fee = Self::trade_fee(gross_collateral_out);
                let collateral_out = gross_collateral_out
                    .checked_sub(&total_fee)
                    .ok_or(Error::<T>::TradeAmountTooSmall)?;
                ensure!(!collateral_out.is_zero(), Error::<T>::TradeAmountTooSmall);
                ensure!(
                    collateral_out >= min_collateral_out,
                    Error::<T>::SlippageToleranceExceeded
                );

                let fee_split = Self::split_trade_fee(total_fee);
                let updated_pool = Self::pool_after_sell(
                    pool,
                    outcome,
                    shares_in,
                    gross_collateral_out,
                    fee_split.pool,
                )?;
                Self::record_trade_fees(market_id, fee_split);
                MarketPools::<T>::insert(market_id, updated_pool);
                Self::record_market_volume(market_id, gross_collateral_out);
                Self::debit_position_on_sell(
                    market_id,
                    &who,
                    outcome,
                    shares_in,
                    gross_collateral_out,
                )?;
                T::Assets::transfer(
                    market.collateral_asset,
                    &Self::account_id(),
                    &who,
                    collateral_out,
                )?;

                Self::deposit_event(Event::TradeExecuted {
                    market_id,
                    trader: who.clone(),
                    side: TradeSide::Sell,
                    outcome,
                    collateral_amount: collateral_out,
                    share_amount: shares_in,
                    fee_amount: total_fee,
                });
                Ok(())
            })
        }

        /// Atomically sell one side and reinvest the net collateral into the opposite side.
        #[pallet::call_index(33)]
        #[pallet::weight(T::WeightInfo::flip_position())]
        pub fn flip_position(
            origin: OriginFor<T>,
            market_id: MarketId,
            from_outcome: BinaryOutcome,
            shares_in: T::Balance,
            min_collateral_out: T::Balance,
            min_shares_out: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!shares_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_legacy_amm_market(&market)?;
            Self::ensure_position_has_shares(market_id, &who, from_outcome, shares_in)?;

            with_storage_transaction(|| -> DispatchResult {
                let quote = Self::quote_flip_position_market(market_id, from_outcome, shares_in)?;
                ensure!(
                    quote.collateral_reinvested >= min_collateral_out,
                    Error::<T>::SlippageToleranceExceeded
                );
                ensure!(
                    quote.shares_out >= min_shares_out,
                    Error::<T>::SlippageToleranceExceeded
                );

                let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
                let sell_fee_split = Self::split_trade_fee(quote.sell_fee_amount);
                let buy_fee_split = Self::split_trade_fee(quote.buy_fee_amount);
                let pool_after_sell = Self::pool_after_sell(
                    pool,
                    from_outcome,
                    shares_in,
                    quote.gross_collateral_out,
                    sell_fee_split.pool,
                )?;
                let updated_pool = Self::pool_after_buy(
                    pool_after_sell,
                    quote.to_outcome,
                    quote.pricing_collateral,
                    buy_fee_split.pool,
                )?;

                Self::record_trade_fees(market_id, sell_fee_split);
                Self::record_trade_fees(market_id, buy_fee_split);
                MarketPools::<T>::insert(market_id, updated_pool);
                Self::record_market_volume(market_id, quote.gross_collateral_out);
                Self::record_market_volume(market_id, quote.pricing_collateral);
                Self::debit_position_on_sell(
                    market_id,
                    &who,
                    from_outcome,
                    shares_in,
                    quote.gross_collateral_out,
                )?;
                Self::credit_position_on_buy(
                    market_id,
                    &who,
                    quote.to_outcome,
                    quote.shares_out,
                    quote.pricing_collateral,
                )?;

                Self::deposit_event(Event::TradeExecuted {
                    market_id,
                    trader: who.clone(),
                    side: TradeSide::Sell,
                    outcome: from_outcome,
                    collateral_amount: quote.collateral_reinvested,
                    share_amount: shares_in,
                    fee_amount: quote.sell_fee_amount,
                });
                Self::deposit_event(Event::TradeExecuted {
                    market_id,
                    trader: who.clone(),
                    side: TradeSide::Buy,
                    outcome: quote.to_outcome,
                    collateral_amount: quote.collateral_reinvested,
                    share_amount: quote.shares_out,
                    fee_amount: quote.buy_fee_amount,
                });
                Self::deposit_event(Event::PositionFlipped {
                    market_id,
                    trader: who,
                    from_outcome,
                    to_outcome: quote.to_outcome,
                    shares_in,
                    collateral_reinvested: quote.collateral_reinvested,
                    shares_out: quote.shares_out,
                });
                Ok(())
            })
        }

        /// Add KUSD liquidity to an open binary AMM and mint locked LP shares.
        #[pallet::call_index(30)]
        #[pallet::weight(T::WeightInfo::add_liquidity())]
        pub fn add_liquidity(
            origin: OriginFor<T>,
            market_id: MarketId,
            collateral_amount: T::Balance,
            min_lp_shares: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!collateral_amount.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_legacy_amm_market(&market)?;

            with_storage_transaction(|| -> DispatchResult {
                let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
                let totals = LiquidityPositionTotals::<T>::get(market_id);
                let lp_shares = Self::quote_lp_shares(&pool, &totals, collateral_amount)?;
                ensure!(!lp_shares.is_zero(), Error::<T>::TradeAmountTooSmall);
                ensure!(
                    lp_shares >= min_lp_shares,
                    Error::<T>::SlippageToleranceExceeded
                );

                T::Assets::transfer(
                    market.collateral_asset,
                    &who,
                    &Self::account_id(),
                    collateral_amount,
                )?;
                MarketPools::<T>::try_mutate(market_id, |maybe_pool| -> DispatchResult {
                    let pool = maybe_pool.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    pool.collateral = pool
                        .collateral
                        .checked_add(&collateral_amount)
                        .ok_or(Error::<T>::Overflow)?;
                    pool.yes = pool
                        .yes
                        .checked_add(&collateral_amount)
                        .ok_or(Error::<T>::Overflow)?;
                    pool.no = pool
                        .no
                        .checked_add(&collateral_amount)
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(())
                })?;
                LiquidityPositions::<T>::try_mutate(
                    market_id,
                    &who,
                    |position| -> DispatchResult {
                        let entry = position.get_or_insert_with(Default::default);
                        entry.shares = entry
                            .shares
                            .checked_add(&lp_shares)
                            .ok_or(Error::<T>::Overflow)?;
                        entry.collateral_contributed = entry
                            .collateral_contributed
                            .checked_add(&collateral_amount)
                            .ok_or(Error::<T>::Overflow)?;
                        Ok(())
                    },
                )?;
                LiquidityPositionTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                    totals.total_shares = totals
                        .total_shares
                        .checked_add(&lp_shares)
                        .ok_or(Error::<T>::Overflow)?;
                    totals.total_collateral_contributed = totals
                        .total_collateral_contributed
                        .checked_add(&collateral_amount)
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(())
                })?;

                Self::deposit_event(Event::LiquidityAdded {
                    market_id,
                    provider: who.clone(),
                    collateral_amount,
                    lp_shares,
                });
                Ok(())
            })
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

        /// Claim residual creator liquidity after a market is resolved or cancelled.
        #[pallet::call_index(24)]
        #[pallet::weight(T::WeightInfo::claim_creator_liquidity())]
        pub fn claim_creator_liquidity(
            origin: OriginFor<T>,
            market_id: MarketId,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            Self::ensure_legacy_amm_market(&market)?;
            ensure!(market.creator == who, Error::<T>::NotMarketCreator);
            let amount = Self::claim_liquidity_for(&who, market_id, &market, T::Balance::zero())?;
            Self::deposit_event(Event::CreatorLiquidityClaimed {
                market_id,
                creator: who,
                amount,
            });
            Ok(())
        }

        /// Claim locked AMM LP residual after a market is resolved or cancelled.
        #[pallet::call_index(31)]
        #[pallet::weight(T::WeightInfo::claim_liquidity())]
        pub fn claim_liquidity(
            origin: OriginFor<T>,
            market_id: MarketId,
            lp_shares: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            Self::ensure_legacy_amm_market(&market)?;
            Self::claim_liquidity_for(&who, market_id, &market, lp_shares).map(|_| ())
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

        /// Place a bounded on-chain limit order. GTC remainders post; IOC remainders refund.
        #[pallet::call_index(34)]
        #[pallet::weight(T::WeightInfo::place_order(T::MaxFillsPerOrder::get()))]
        pub fn place_order(
            origin: OriginFor<T>,
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
            time_in_force: TimeInForce,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::place_order_for(
                &who,
                market_id,
                outcome,
                side,
                price_cents,
                shares,
                time_in_force,
            )
        }

        /// Cancel an open order and refund its reserved collateral or shares.
        #[pallet::call_index(35)]
        #[pallet::weight(T::WeightInfo::cancel_order())]
        pub fn cancel_order(origin: OriginFor<T>, order_id: OrderId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let order = Orders::<T>::get(order_id).ok_or(Error::<T>::OrderUnknown)?;
            ensure!(order.owner == who, Error::<T>::NotOrderOwner);
            let market = Markets::<T>::get(order.market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_order_book_market(&market)?;
            with_storage_transaction(|| -> DispatchResult {
                Self::cancel_order_unchecked(order_id, order)?;
                Ok(())
            })
        }

        /// Lock collateral and mint a complete YES/NO set.
        #[pallet::call_index(36)]
        #[pallet::weight(T::WeightInfo::split_position())]
        pub fn split_position(
            origin: OriginFor<T>,
            market_id: MarketId,
            collateral_amount: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!collateral_amount.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_order_book_market(&market)?;
            with_storage_transaction(|| -> DispatchResult {
                T::Assets::transfer(
                    market.collateral_asset,
                    &who,
                    &Self::account_id(),
                    collateral_amount,
                )?;
                Self::credit_complete_set(market_id, &who, collateral_amount)?;
                Self::increase_order_book_collateral(market_id, collateral_amount)?;
                Self::deposit_event(Event::PositionSplit {
                    market_id,
                    account: who.clone(),
                    collateral_amount,
                });
                Ok(())
            })
        }

        /// Burn a complete YES/NO set and unlock its backing collateral.
        #[pallet::call_index(37)]
        #[pallet::weight(T::WeightInfo::merge_positions())]
        pub fn merge_positions(
            origin: OriginFor<T>,
            market_id: MarketId,
            shares: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!shares.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_order_book_market(&market)?;
            with_storage_transaction(|| -> DispatchResult {
                Self::debit_outcome_shares(market_id, &who, BinaryOutcome::Yes, shares)?;
                Self::debit_outcome_shares(market_id, &who, BinaryOutcome::No, shares)?;
                Self::decrease_order_book_collateral(market_id, shares)?;
                T::Assets::transfer(market.collateral_asset, &Self::account_id(), &who, shares)?;
                Self::deposit_event(Event::PositionMerged {
                    market_id,
                    account: who.clone(),
                    shares,
                });
                Ok(())
            })
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

        fn ensure_legacy_amm_market(market: &MarketOf<T>) -> DispatchResult {
            ensure!(
                matches!(market.mechanism, MarketMechanism::LegacyAmm),
                Error::<T>::UnsupportedMarketMechanism
            );
            Ok(())
        }

        fn ensure_order_book_market(market: &MarketOf<T>) -> DispatchResult {
            ensure!(
                matches!(market.mechanism, MarketMechanism::OrderBook),
                Error::<T>::UnsupportedMarketMechanism
            );
            Ok(())
        }

        fn place_order_for(
            who: &T::AccountId,
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
            time_in_force: TimeInForce,
        ) -> DispatchResult {
            ensure!(!shares.is_zero(), Error::<T>::InvalidTradeAmount);
            Self::ensure_order_lot_size(shares)?;
            Self::ensure_valid_price(price_cents)?;
            let market = Self::ensure_market_tradable(market_id)?;
            Self::ensure_order_book_market(&market)?;
            if matches!(time_in_force, TimeInForce::Gtc)
                && Self::posts_order_remainder_readonly(
                    market_id,
                    outcome,
                    side,
                    price_cents,
                    shares,
                )?
            {
                Self::ensure_open_order_slot(who, market_id)?;
                Self::ensure_price_level_slot(market_id, outcome, side, price_cents)?;
            }

            let mut remaining_shares = shares;
            let mut available_collateral = T::Balance::zero();
            let mut execution = OrderExecutionTotals::<T::Balance>::default();

            with_storage_transaction(|| -> DispatchResult {
                let order_id = Self::next_order_id_checked()?;
                match side {
                    OrderSide::Buy => {
                        let max_notional = Self::price_amount(price_cents, shares)?;
                        ensure!(!max_notional.is_zero(), Error::<T>::TradeAmountTooSmall);
                        let fee_budget = Self::orderbook_trade_fee(max_notional);
                        available_collateral = max_notional
                            .checked_add(&fee_budget)
                            .ok_or(Error::<T>::Overflow)?;
                        T::Assets::transfer(
                            market.collateral_asset,
                            who,
                            &Self::account_id(),
                            available_collateral,
                        )?;
                    }
                    OrderSide::Sell => {
                        Self::debit_outcome_shares(market_id, who, outcome, shares)?;
                    }
                }

                Self::deposit_event(Event::OrderPlaced {
                    order_id,
                    market_id,
                    owner: who.clone(),
                    outcome,
                    side,
                    price_cents,
                    shares,
                    time_in_force,
                });

                Self::match_order(
                    order_id,
                    who,
                    market_id,
                    &market,
                    outcome,
                    side,
                    price_cents,
                    &mut remaining_shares,
                    &mut available_collateral,
                    &mut execution,
                )?;

                if remaining_shares.is_zero() {
                    if !available_collateral.is_zero() {
                        T::Assets::transfer(
                            market.collateral_asset,
                            &Self::account_id(),
                            who,
                            available_collateral,
                        )?;
                    }
                    return Ok(());
                }

                match time_in_force {
                    TimeInForce::Ioc => match side {
                        OrderSide::Buy => {
                            if !available_collateral.is_zero() {
                                T::Assets::transfer(
                                    market.collateral_asset,
                                    &Self::account_id(),
                                    who,
                                    available_collateral,
                                )?;
                            }
                        }
                        OrderSide::Sell => {
                            Self::credit_outcome_shares(market_id, who, outcome, remaining_shares)?;
                        }
                    },
                    TimeInForce::Gtc => {
                        Self::ensure_open_order_slot(who, market_id)?;
                        Self::ensure_price_level_slot(market_id, outcome, side, price_cents)?;
                        let reserved_collateral = match side {
                            OrderSide::Buy => {
                                let reserved = Self::price_amount(price_cents, remaining_shares)?;
                                ensure!(
                                    available_collateral >= reserved,
                                    Error::<T>::InsufficientReservedCollateral
                                );
                                let refund = available_collateral.saturating_sub(reserved);
                                if !refund.is_zero() {
                                    T::Assets::transfer(
                                        market.collateral_asset,
                                        &Self::account_id(),
                                        who,
                                        refund,
                                    )?;
                                }
                                reserved
                            }
                            OrderSide::Sell => T::Balance::zero(),
                        };
                        Self::insert_open_order(
                            order_id,
                            Order {
                                owner: who.clone(),
                                market_id,
                                outcome,
                                side,
                                price_cents,
                                remaining_shares,
                                reserved_collateral,
                            },
                        )?;
                    }
                }

                Ok(())
            })
        }

        fn match_order(
            taker_order_id: OrderId,
            taker: &T::AccountId,
            market_id: MarketId,
            market: &MarketOf<T>,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            remaining_shares: &mut T::Balance,
            available_collateral: &mut T::Balance,
            totals: &mut OrderExecutionTotals<T::Balance>,
        ) -> DispatchResult {
            let max_fills = T::MaxFillsPerOrder::get();
            let mut fills = 0u32;
            while !remaining_shares.is_zero() && fills < max_fills {
                let candidate = match side {
                    OrderSide::Buy => Self::best_buy_match(market_id, outcome, price_cents),
                    OrderSide::Sell => Self::best_same_outcome_buy(market_id, outcome, price_cents)
                        .map(|id| (id, false)),
                };

                let Some((maker_order_id, complementary)) = candidate else {
                    break;
                };
                let maker_order =
                    Orders::<T>::get(maker_order_id).ok_or(Error::<T>::OrderUnknown)?;
                let fill_shares = core::cmp::min(*remaining_shares, maker_order.remaining_shares);
                ensure!(!fill_shares.is_zero(), Error::<T>::TradeAmountTooSmall);

                if complementary {
                    Self::fill_complementary_buy(
                        taker_order_id,
                        taker,
                        market,
                        outcome,
                        maker_order_id,
                        maker_order,
                        fill_shares,
                        remaining_shares,
                        available_collateral,
                        totals,
                    )?;
                } else {
                    Self::fill_same_outcome(
                        taker_order_id,
                        taker,
                        market,
                        outcome,
                        side,
                        maker_order_id,
                        maker_order,
                        fill_shares,
                        remaining_shares,
                        available_collateral,
                        totals,
                    )?;
                }
                fills = fills.saturating_add(1);
            }
            Ok(())
        }

        fn posts_order_remainder_readonly(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
        ) -> Result<bool, DispatchError> {
            let mut remaining = shares;
            let mut fills = 0u32;
            let mut simulated_fills: Vec<(OrderId, T::Balance)> = Vec::new();

            while !remaining.is_zero() && fills < T::MaxFillsPerOrder::get() {
                let candidate = match side {
                    OrderSide::Buy => Self::best_buy_match_readonly(
                        market_id,
                        outcome,
                        price_cents,
                        &simulated_fills,
                    ),
                    OrderSide::Sell => Self::best_same_outcome_buy_readonly(
                        market_id,
                        outcome,
                        price_cents,
                        &simulated_fills,
                    )
                    .map(|id| (id, false)),
                };
                let Some((order_id, _)) = candidate else {
                    break;
                };
                let Some(order) = Orders::<T>::get(order_id) else {
                    break;
                };
                let Some(simulated_remaining) =
                    Self::simulated_remaining_shares(order_id, &order, &simulated_fills)
                else {
                    break;
                };
                let fill = core::cmp::min(remaining, simulated_remaining);
                if fill.is_zero() {
                    break;
                }
                remaining = remaining.saturating_sub(fill);
                Self::record_simulated_fill(&mut simulated_fills, order_id, fill)?;
                fills = fills.saturating_add(1);
            }

            Ok(!remaining.is_zero())
        }

        fn best_buy_match(
            market_id: MarketId,
            outcome: BinaryOutcome,
            price_cents: PriceCents,
        ) -> Option<(OrderId, bool)> {
            let same = Self::best_same_outcome_sell(market_id, outcome, price_cents);
            let complementary =
                Self::best_complementary_buy(market_id, outcome.opposite(), price_cents);

            match (same, complementary) {
                (Some((same_id, same_price)), Some((comp_id, comp_price))) => {
                    let comp_taker_price = 100u8.saturating_sub(comp_price);
                    if comp_taker_price < same_price {
                        Some((comp_id, true))
                    } else {
                        Some((same_id, false))
                    }
                }
                (Some((same_id, _)), None) => Some((same_id, false)),
                (None, Some((comp_id, _))) => Some((comp_id, true)),
                (None, None) => None,
            }
        }

        fn best_same_outcome_sell(
            market_id: MarketId,
            outcome: BinaryOutcome,
            limit_price: PriceCents,
        ) -> Option<(OrderId, PriceCents)> {
            let levels = OrderBookPriceLevels::<T>::get(market_id, (outcome, OrderSide::Sell));
            for (price, _) in levels.iter() {
                if *price > limit_price {
                    break;
                }
                if let Some(order_id) =
                    Self::front_order_id(market_id, outcome, OrderSide::Sell, *price)
                {
                    return Some((order_id, *price));
                }
            }
            None
        }

        fn best_same_outcome_buy(
            market_id: MarketId,
            outcome: BinaryOutcome,
            limit_price: PriceCents,
        ) -> Option<OrderId> {
            let levels = OrderBookPriceLevels::<T>::get(market_id, (outcome, OrderSide::Buy));
            for (price, _) in levels.iter().rev() {
                if *price < limit_price {
                    break;
                }
                if let Some(order_id) =
                    Self::front_order_id(market_id, outcome, OrderSide::Buy, *price)
                {
                    return Some(order_id);
                }
            }
            None
        }

        fn best_complementary_buy(
            market_id: MarketId,
            maker_outcome: BinaryOutcome,
            taker_price: PriceCents,
        ) -> Option<(OrderId, PriceCents)> {
            let min_maker_price = 100u8.saturating_sub(taker_price);
            let levels = OrderBookPriceLevels::<T>::get(market_id, (maker_outcome, OrderSide::Buy));
            for (price, _) in levels.iter().rev() {
                if *price < min_maker_price || *price < MIN_PRICE_CENTS {
                    break;
                }
                if let Some(order_id) =
                    Self::front_order_id(market_id, maker_outcome, OrderSide::Buy, *price)
                {
                    return Some((order_id, *price));
                }
            }
            None
        }

        fn best_buy_match_readonly(
            market_id: MarketId,
            outcome: BinaryOutcome,
            price_cents: PriceCents,
            simulated_fills: &[(OrderId, T::Balance)],
        ) -> Option<(OrderId, bool)> {
            let same = Self::best_same_outcome_sell_readonly(
                market_id,
                outcome,
                price_cents,
                simulated_fills,
            );
            let complementary = Self::best_complementary_buy_readonly(
                market_id,
                outcome.opposite(),
                price_cents,
                simulated_fills,
            );

            match (same, complementary) {
                (Some((same_id, same_price)), Some((comp_id, comp_price))) => {
                    let comp_taker_price = 100u8.saturating_sub(comp_price);
                    if comp_taker_price < same_price {
                        Some((comp_id, true))
                    } else {
                        Some((same_id, false))
                    }
                }
                (Some((same_id, _)), None) => Some((same_id, false)),
                (None, Some((comp_id, _))) => Some((comp_id, true)),
                (None, None) => None,
            }
        }

        fn best_same_outcome_sell_readonly(
            market_id: MarketId,
            outcome: BinaryOutcome,
            limit_price: PriceCents,
            simulated_fills: &[(OrderId, T::Balance)],
        ) -> Option<(OrderId, PriceCents)> {
            let levels = OrderBookPriceLevels::<T>::get(market_id, (outcome, OrderSide::Sell));
            for (price, _) in levels.iter() {
                if *price > limit_price {
                    break;
                }
                if let Some(order_id) = Self::front_order_id_readonly(
                    market_id,
                    outcome,
                    OrderSide::Sell,
                    *price,
                    simulated_fills,
                ) {
                    return Some((order_id, *price));
                }
            }
            None
        }

        fn best_same_outcome_buy_readonly(
            market_id: MarketId,
            outcome: BinaryOutcome,
            limit_price: PriceCents,
            simulated_fills: &[(OrderId, T::Balance)],
        ) -> Option<OrderId> {
            let levels = OrderBookPriceLevels::<T>::get(market_id, (outcome, OrderSide::Buy));
            for (price, _) in levels.iter().rev() {
                if *price < limit_price {
                    break;
                }
                if let Some(order_id) = Self::front_order_id_readonly(
                    market_id,
                    outcome,
                    OrderSide::Buy,
                    *price,
                    simulated_fills,
                ) {
                    return Some(order_id);
                }
            }
            None
        }

        fn best_complementary_buy_readonly(
            market_id: MarketId,
            maker_outcome: BinaryOutcome,
            taker_price: PriceCents,
            simulated_fills: &[(OrderId, T::Balance)],
        ) -> Option<(OrderId, PriceCents)> {
            let min_maker_price = 100u8.saturating_sub(taker_price);
            let levels = OrderBookPriceLevels::<T>::get(market_id, (maker_outcome, OrderSide::Buy));
            for (price, _) in levels.iter().rev() {
                if *price < min_maker_price || *price < MIN_PRICE_CENTS {
                    break;
                }
                if let Some(order_id) = Self::front_order_id_readonly(
                    market_id,
                    maker_outcome,
                    OrderSide::Buy,
                    *price,
                    simulated_fills,
                ) {
                    return Some((order_id, *price));
                }
            }
            None
        }

        fn front_order_id_readonly(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            simulated_fills: &[(OrderId, T::Balance)],
        ) -> Option<OrderId> {
            let key = (outcome, side, price_cents);
            OrderBookQueues::<T>::get(market_id, key)
                .into_iter()
                .find(|order_id| {
                    Orders::<T>::get(order_id)
                        .map(|order| {
                            order.market_id == market_id
                                && order.outcome == outcome
                                && order.side == side
                                && order.price_cents == price_cents
                                && Self::simulated_remaining_shares(
                                    *order_id,
                                    &order,
                                    simulated_fills,
                                )
                                .map(|remaining| !remaining.is_zero())
                                .unwrap_or(false)
                        })
                        .unwrap_or(false)
                })
        }

        fn simulated_remaining_shares(
            order_id: OrderId,
            order: &OrderOf<T>,
            simulated_fills: &[(OrderId, T::Balance)],
        ) -> Option<T::Balance> {
            let consumed = simulated_fills
                .iter()
                .find_map(|(id, shares)| (*id == order_id).then_some(*shares))
                .unwrap_or_default();
            if consumed >= order.remaining_shares {
                None
            } else {
                Some(order.remaining_shares.saturating_sub(consumed))
            }
        }

        fn record_simulated_fill(
            simulated_fills: &mut Vec<(OrderId, T::Balance)>,
            order_id: OrderId,
            fill_shares: T::Balance,
        ) -> DispatchResult {
            if let Some((_, shares)) = simulated_fills
                .iter_mut()
                .find(|(existing_id, _)| *existing_id == order_id)
            {
                *shares = shares
                    .checked_add(&fill_shares)
                    .ok_or(Error::<T>::Overflow)?;
                return Ok(());
            }
            simulated_fills.push((order_id, fill_shares));
            Ok(())
        }

        fn front_order_id(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
        ) -> Option<OrderId> {
            let key = (outcome, side, price_cents);
            loop {
                let order_id = OrderBookQueues::<T>::get(market_id, key).first().copied()?;
                let stale = Orders::<T>::get(order_id)
                    .map(|order| {
                        order.market_id != market_id
                            || order.outcome != outcome
                            || order.side != side
                            || order.price_cents != price_cents
                            || order.remaining_shares.is_zero()
                    })
                    .unwrap_or(true);
                if !stale {
                    return Some(order_id);
                }
                OrderBookQueues::<T>::mutate(market_id, key, |queue| {
                    if !queue.is_empty() {
                        queue.remove(0);
                    }
                });
                if OrderBookQueues::<T>::get(market_id, key).is_empty() {
                    OrderBookPriceLevels::<T>::mutate(market_id, (outcome, side), |levels| {
                        levels.remove(&price_cents);
                    });
                }
            }
        }

        fn fill_same_outcome(
            taker_order_id: OrderId,
            taker: &T::AccountId,
            market: &MarketOf<T>,
            outcome: BinaryOutcome,
            taker_side: OrderSide,
            maker_order_id: OrderId,
            mut maker_order: OrderOf<T>,
            fill_shares: T::Balance,
            remaining_shares: &mut T::Balance,
            available_collateral: &mut T::Balance,
            totals: &mut OrderExecutionTotals<T::Balance>,
        ) -> DispatchResult {
            let notional = Self::price_amount(maker_order.price_cents, fill_shares)?;
            ensure!(!notional.is_zero(), Error::<T>::TradeAmountTooSmall);
            let fee = Self::orderbook_trade_fee(notional);

            match taker_side {
                OrderSide::Buy => {
                    let debit = notional.checked_add(&fee).ok_or(Error::<T>::Overflow)?;
                    ensure!(
                        *available_collateral >= debit,
                        Error::<T>::InsufficientReservedCollateral
                    );
                    *available_collateral = available_collateral.saturating_sub(debit);
                    Self::credit_outcome_shares(
                        maker_order.market_id,
                        taker,
                        outcome,
                        fill_shares,
                    )?;
                    T::Assets::transfer(
                        market.collateral_asset,
                        &Self::account_id(),
                        &maker_order.owner,
                        notional,
                    )?;
                    totals.collateral_in = totals.collateral_in.saturating_add(debit);
                }
                OrderSide::Sell => {
                    ensure!(
                        maker_order.reserved_collateral >= notional,
                        Error::<T>::InsufficientReservedCollateral
                    );
                    maker_order.reserved_collateral =
                        maker_order.reserved_collateral.saturating_sub(notional);
                    Self::credit_outcome_shares(
                        maker_order.market_id,
                        &maker_order.owner,
                        outcome,
                        fill_shares,
                    )?;
                    let payout = notional.checked_sub(&fee).ok_or(Error::<T>::Overflow)?;
                    if !payout.is_zero() {
                        T::Assets::transfer(
                            market.collateral_asset,
                            &Self::account_id(),
                            taker,
                            payout,
                        )?;
                    }
                    totals.collateral_out = totals.collateral_out.saturating_add(payout);
                }
            }

            *remaining_shares = remaining_shares.saturating_sub(fill_shares);
            maker_order.remaining_shares = maker_order.remaining_shares.saturating_sub(fill_shares);
            totals.filled_shares = totals.filled_shares.saturating_add(fill_shares);
            totals.fee_amount = totals.fee_amount.saturating_add(fee);
            Self::record_trade_fees(maker_order.market_id, Self::split_orderbook_trade_fee(fee));
            Self::record_market_volume(maker_order.market_id, notional);
            Self::deposit_event(Event::OrderFilled {
                maker_order_id,
                taker_order_id,
                market_id: maker_order.market_id,
                maker: maker_order.owner.clone(),
                taker: taker.clone(),
                outcome,
                price_cents: maker_order.price_cents,
                shares: fill_shares,
                collateral_amount: notional,
                fee_amount: fee,
            });
            Self::decrease_price_level_shares(
                maker_order.market_id,
                maker_order.outcome,
                maker_order.side,
                maker_order.price_cents,
                fill_shares,
            )?;
            Self::update_or_remove_maker_order(maker_order_id, maker_order, market.collateral_asset)
        }

        fn fill_complementary_buy(
            taker_order_id: OrderId,
            taker: &T::AccountId,
            market: &MarketOf<T>,
            taker_outcome: BinaryOutcome,
            maker_order_id: OrderId,
            mut maker_order: OrderOf<T>,
            fill_shares: T::Balance,
            remaining_shares: &mut T::Balance,
            available_collateral: &mut T::Balance,
            totals: &mut OrderExecutionTotals<T::Balance>,
        ) -> DispatchResult {
            ensure!(
                matches!(maker_order.side, OrderSide::Buy)
                    && maker_order.outcome == taker_outcome.opposite(),
                Error::<T>::OrderUnknown
            );
            let maker_notional = Self::price_amount(maker_order.price_cents, fill_shares)?;
            ensure!(
                fill_shares >= maker_notional,
                Error::<T>::InsufficientReservedCollateral
            );
            let taker_notional = fill_shares.saturating_sub(maker_notional);
            let fee = Self::orderbook_trade_fee(taker_notional);
            let taker_debit = taker_notional
                .checked_add(&fee)
                .ok_or(Error::<T>::Overflow)?;
            ensure!(
                maker_order.reserved_collateral >= maker_notional,
                Error::<T>::InsufficientReservedCollateral
            );
            ensure!(
                *available_collateral >= taker_debit,
                Error::<T>::InsufficientReservedCollateral
            );

            maker_order.reserved_collateral = maker_order
                .reserved_collateral
                .saturating_sub(maker_notional);
            *available_collateral = available_collateral.saturating_sub(taker_debit);
            Self::credit_outcome_shares(
                maker_order.market_id,
                &maker_order.owner,
                maker_order.outcome,
                fill_shares,
            )?;
            Self::credit_outcome_shares(maker_order.market_id, taker, taker_outcome, fill_shares)?;
            Self::increase_order_book_collateral(maker_order.market_id, fill_shares)?;

            *remaining_shares = remaining_shares.saturating_sub(fill_shares);
            maker_order.remaining_shares = maker_order.remaining_shares.saturating_sub(fill_shares);
            totals.filled_shares = totals.filled_shares.saturating_add(fill_shares);
            totals.collateral_in = totals.collateral_in.saturating_add(taker_debit);
            totals.fee_amount = totals.fee_amount.saturating_add(fee);
            Self::record_trade_fees(maker_order.market_id, Self::split_orderbook_trade_fee(fee));
            Self::record_market_volume(maker_order.market_id, fill_shares);
            Self::deposit_event(Event::OrderFilled {
                maker_order_id,
                taker_order_id,
                market_id: maker_order.market_id,
                maker: maker_order.owner.clone(),
                taker: taker.clone(),
                outcome: taker_outcome,
                price_cents: 100u8.saturating_sub(maker_order.price_cents),
                shares: fill_shares,
                collateral_amount: taker_notional,
                fee_amount: fee,
            });
            Self::decrease_price_level_shares(
                maker_order.market_id,
                maker_order.outcome,
                maker_order.side,
                maker_order.price_cents,
                fill_shares,
            )?;
            Self::update_or_remove_maker_order(maker_order_id, maker_order, market.collateral_asset)
        }

        fn update_or_remove_maker_order(
            order_id: OrderId,
            order: OrderOf<T>,
            collateral_asset: T::AssetId,
        ) -> DispatchResult {
            if order.remaining_shares.is_zero() {
                Self::remove_order_from_indexes(order_id, &order)?;
                Orders::<T>::remove(order_id);
                if matches!(order.side, OrderSide::Buy) && !order.reserved_collateral.is_zero() {
                    T::Assets::transfer(
                        collateral_asset,
                        &Self::account_id(),
                        &order.owner,
                        order.reserved_collateral,
                    )?;
                }
            } else {
                Orders::<T>::insert(order_id, order);
            }
            Ok(())
        }

        fn next_order_id_checked() -> Result<OrderId, DispatchError> {
            NextOrderId::<T>::try_mutate(|next_id| -> Result<OrderId, DispatchError> {
                let id = *next_id;
                *next_id = next_id.checked_add(1).ok_or(Error::<T>::Overflow)?;
                Ok(id)
            })
        }

        fn ensure_valid_price(price_cents: PriceCents) -> DispatchResult {
            ensure!(
                (MIN_PRICE_CENTS..=MAX_PRICE_CENTS).contains(&price_cents),
                Error::<T>::InvalidPrice
            );
            Ok(())
        }

        fn ensure_order_lot_size(shares: T::Balance) -> DispatchResult {
            let units = shares.saturated_into::<u128>();
            ensure!(
                units >= PRICE_CENTS_DENOMINATOR && units % PRICE_CENTS_DENOMINATOR == 0,
                Error::<T>::TradeAmountTooSmall
            );
            Ok(())
        }

        fn price_amount(
            price_cents: PriceCents,
            shares: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let value = U256::from(shares.saturated_into::<u128>())
                .checked_mul(U256::from(price_cents as u128))
                .ok_or(Error::<T>::Overflow)?
                / U256::from(PRICE_CENTS_DENOMINATOR);
            Self::u256_to_balance(value)
        }

        fn orderbook_trade_fee(amount: T::Balance) -> T::Balance {
            Self::trade_fee(amount)
        }

        fn split_orderbook_trade_fee(amount: T::Balance) -> TradeFeeSplit<T::Balance> {
            let fee = amount.saturated_into::<u128>();
            let creator = fee.saturating_mul(80) / 100;
            let buyback = fee.saturating_sub(creator);
            TradeFeeSplit {
                pool: T::Balance::zero(),
                creator: creator.saturated_into::<T::Balance>(),
                buyback: buyback.saturated_into::<T::Balance>(),
            }
        }

        fn ensure_open_order_slot(who: &T::AccountId, market_id: MarketId) -> DispatchResult {
            let open_orders = OpenOrdersByAccountMarket::<T>::get(who, market_id);
            ensure!(
                (open_orders.len() as u32) < T::MaxOpenOrdersPerAccountMarket::get(),
                Error::<T>::TooManyOpenOrders
            );
            Ok(())
        }

        fn ensure_price_level_slot(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
        ) -> DispatchResult {
            let orders = OrderBookQueues::<T>::get(market_id, (outcome, side, price_cents));
            ensure!(
                (orders.len() as u32) < T::MaxOrdersPerPrice::get(),
                Error::<T>::TooManyOrdersAtPrice
            );
            Ok(())
        }

        fn insert_open_order(order_id: OrderId, order: OrderOf<T>) -> DispatchResult {
            OrderBookQueues::<T>::try_mutate(
                order.market_id,
                (order.outcome, order.side, order.price_cents),
                |queue| -> DispatchResult {
                    queue
                        .try_push(order_id)
                        .map_err(|_| Error::<T>::TooManyOrdersAtPrice)?;
                    Ok(())
                },
            )?;
            Self::increase_price_level_shares(
                order.market_id,
                order.outcome,
                order.side,
                order.price_cents,
                order.remaining_shares,
            )?;
            OpenOrdersByAccountMarket::<T>::try_mutate(
                &order.owner,
                order.market_id,
                |orders| -> DispatchResult {
                    orders
                        .try_push(order_id)
                        .map_err(|_| Error::<T>::TooManyOpenOrders)?;
                    Ok(())
                },
            )?;
            Orders::<T>::insert(order_id, order);
            Ok(())
        }

        fn remove_order_from_indexes(order_id: OrderId, order: &OrderOf<T>) -> DispatchResult {
            OrderBookQueues::<T>::mutate(
                order.market_id,
                (order.outcome, order.side, order.price_cents),
                |queue| {
                    if let Some(index) = queue.iter().position(|id| *id == order_id) {
                        queue.remove(index);
                    }
                },
            );
            OpenOrdersByAccountMarket::<T>::mutate(&order.owner, order.market_id, |orders| {
                if let Some(index) = orders.iter().position(|id| *id == order_id) {
                    orders.remove(index);
                }
            });
            Self::decrease_price_level_shares(
                order.market_id,
                order.outcome,
                order.side,
                order.price_cents,
                order.remaining_shares,
            )?;
            Ok(())
        }

        fn increase_price_level_shares(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
        ) -> DispatchResult {
            if shares.is_zero() {
                return Ok(());
            }
            OrderBookPriceLevels::<T>::try_mutate(
                market_id,
                (outcome, side),
                |levels| -> DispatchResult {
                    let updated = levels
                        .get(&price_cents)
                        .copied()
                        .unwrap_or_default()
                        .checked_add(&shares)
                        .ok_or(Error::<T>::Overflow)?;
                    levels
                        .try_insert(price_cents, updated)
                        .map_err(|_| Error::<T>::TooManyOrdersAtPrice)?;
                    Ok(())
                },
            )
        }

        fn decrease_price_level_shares(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
        ) -> DispatchResult {
            if shares.is_zero() {
                return Ok(());
            }
            OrderBookPriceLevels::<T>::try_mutate(
                market_id,
                (outcome, side),
                |levels| -> DispatchResult {
                    let current = levels
                        .get(&price_cents)
                        .copied()
                        .ok_or(Error::<T>::Overflow)?;
                    ensure!(current >= shares, Error::<T>::Overflow);
                    let updated = current.saturating_sub(shares);
                    if updated.is_zero() {
                        levels.remove(&price_cents);
                    } else {
                        levels
                            .try_insert(price_cents, updated)
                            .map_err(|_| Error::<T>::TooManyOrdersAtPrice)?;
                    }
                    Ok(())
                },
            )
        }

        fn cancel_order_unchecked(order_id: OrderId, order: OrderOf<T>) -> DispatchResult {
            Self::remove_order_from_indexes(order_id, &order)?;
            Orders::<T>::remove(order_id);
            match order.side {
                OrderSide::Buy => {
                    if !order.reserved_collateral.is_zero() {
                        let market =
                            Markets::<T>::get(order.market_id).ok_or(Error::<T>::MarketUnknown)?;
                        T::Assets::transfer(
                            market.collateral_asset,
                            &Self::account_id(),
                            &order.owner,
                            order.reserved_collateral,
                        )?;
                    }
                }
                OrderSide::Sell => {
                    Self::credit_outcome_shares(
                        order.market_id,
                        &order.owner,
                        order.outcome,
                        order.remaining_shares,
                    )?;
                }
            }
            Self::deposit_event(Event::OrderCancelled {
                order_id,
                market_id: order.market_id,
                owner: order.owner,
            });
            Ok(())
        }

        fn credit_complete_set(
            market_id: MarketId,
            who: &T::AccountId,
            shares: T::Balance,
        ) -> DispatchResult {
            Self::credit_outcome_shares(market_id, who, BinaryOutcome::Yes, shares)?;
            Self::credit_outcome_shares(market_id, who, BinaryOutcome::No, shares)
        }

        fn credit_outcome_shares(
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

        fn debit_outcome_shares(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares: T::Balance,
        ) -> DispatchResult {
            if shares.is_zero() {
                return Ok(());
            }
            MarketPositions::<T>::try_mutate_exists(
                market_id,
                who,
                |position| -> DispatchResult {
                    let entry = position.as_mut().ok_or(Error::<T>::InsufficientShares)?;
                    match outcome {
                        BinaryOutcome::Yes => {
                            ensure!(entry.yes_shares >= shares, Error::<T>::InsufficientShares);
                            entry.yes_shares = entry.yes_shares.saturating_sub(shares);
                        }
                        BinaryOutcome::No => {
                            ensure!(entry.no_shares >= shares, Error::<T>::InsufficientShares);
                            entry.no_shares = entry.no_shares.saturating_sub(shares);
                        }
                    }
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
                            .checked_sub(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                    BinaryOutcome::No => {
                        totals.total_no_shares = totals
                            .total_no_shares
                            .checked_sub(&shares)
                            .ok_or(Error::<T>::Overflow)?;
                    }
                }
                Ok(())
            })
        }

        fn increase_order_book_collateral(
            market_id: MarketId,
            amount: T::Balance,
        ) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            MarketOrderBookCollateral::<T>::try_mutate(market_id, |total| -> DispatchResult {
                *total = total.checked_add(&amount).ok_or(Error::<T>::Overflow)?;
                Ok(())
            })
        }

        fn decrease_order_book_collateral(
            market_id: MarketId,
            amount: T::Balance,
        ) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            MarketOrderBookCollateral::<T>::try_mutate(market_id, |total| -> DispatchResult {
                ensure!(*total >= amount, Error::<T>::Overflow);
                *total = total.saturating_sub(amount);
                Ok(())
            })
        }

        fn claim_market_for(
            who: &T::AccountId,
            market_id: MarketId,
        ) -> Result<T::Balance, DispatchError> {
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            if matches!(market.mechanism, MarketMechanism::OrderBook) {
                return Self::claim_order_book_market_for(who, market_id, &market);
            }
            let position =
                MarketPositions::<T>::get(market_id, who).ok_or(Error::<T>::NothingToClaim)?;
            ensure!(
                !position.yes_shares.is_zero()
                    || !position.no_shares.is_zero()
                    || !position.net_collateral_paid.is_zero(),
                Error::<T>::NothingToClaim
            );

            let payout = match market.status {
                MarketStatus::Resolved => {
                    let outcome = MarketResolution::<T>::get(market_id)
                        .ok_or(Error::<T>::MarketNotResolved)?;
                    Self::winning_shares(&position, outcome)
                }
                MarketStatus::Cancelled => position.net_collateral_paid,
                _ => return Err(Error::<T>::MarketNotFinalized.into()),
            };

            with_storage_transaction(|| -> Result<T::Balance, DispatchError> {
                MarketPositions::<T>::remove(market_id, who);
                Self::debit_market_totals(market_id, &position)?;
                Self::debit_market_collateral(market_id, payout)?;
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

        fn claim_order_book_market_for(
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
            ensure!(
                !position.yes_shares.is_zero() || !position.no_shares.is_zero(),
                Error::<T>::NothingToClaim
            );

            let payout = match market.status {
                MarketStatus::Resolved => {
                    let outcome = MarketResolution::<T>::get(market_id)
                        .ok_or(Error::<T>::MarketNotResolved)?;
                    Self::winning_shares(&position, outcome)
                }
                MarketStatus::Cancelled => Self::cancelled_order_book_payout(&position)?,
                _ => return Err(Error::<T>::MarketNotFinalized.into()),
            };

            with_storage_transaction(|| -> Result<T::Balance, DispatchError> {
                MarketPositions::<T>::remove(market_id, who);
                Self::debit_market_totals(market_id, &position)?;
                Self::decrease_order_book_collateral(market_id, payout)?;
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

        fn claim_liquidity_for(
            who: &T::AccountId,
            market_id: MarketId,
            market: &MarketOf<T>,
            lp_shares: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            ensure!(
                matches!(
                    market.status,
                    MarketStatus::Resolved | MarketStatus::Cancelled
                ),
                Error::<T>::MarketNotFinalized
            );
            let position =
                LiquidityPositions::<T>::get(market_id, who).ok_or(Error::<T>::NothingToClaim)?;
            let shares_to_claim = if lp_shares.is_zero() {
                position.shares
            } else {
                lp_shares
            };
            ensure!(!shares_to_claim.is_zero(), Error::<T>::NothingToClaim);
            ensure!(
                position.shares >= shares_to_claim,
                Error::<T>::InsufficientLiquidityShares
            );
            let totals = LiquidityPositionTotals::<T>::get(market_id);
            ensure!(
                !totals.total_shares.is_zero() && totals.total_shares >= shares_to_claim,
                Error::<T>::InsufficientLiquidityShares
            );
            let total_claimable = Self::creator_liquidity_claimable(market_id, market)?;
            let amount = Self::pro_rata(total_claimable, shares_to_claim, totals.total_shares)?;
            ensure!(!amount.is_zero(), Error::<T>::NothingToClaim);
            let collateral_reduction = Self::pro_rata(
                position.collateral_contributed,
                shares_to_claim,
                position.shares,
            )?;

            with_storage_transaction(|| -> Result<T::Balance, DispatchError> {
                LiquidityPositions::<T>::try_mutate_exists(
                    market_id,
                    who,
                    |position| -> DispatchResult {
                        let entry = position.as_mut().ok_or(Error::<T>::NothingToClaim)?;
                        ensure!(
                            entry.shares >= shares_to_claim,
                            Error::<T>::InsufficientLiquidityShares
                        );
                        entry.shares = entry.shares.saturating_sub(shares_to_claim);
                        entry.collateral_contributed = entry.collateral_contributed.saturating_sub(
                            core::cmp::min(entry.collateral_contributed, collateral_reduction),
                        );
                        if entry.shares.is_zero() {
                            *position = None;
                        }
                        Ok(())
                    },
                )?;
                LiquidityPositionTotals::<T>::try_mutate(market_id, |totals| -> DispatchResult {
                    totals.total_shares = totals
                        .total_shares
                        .checked_sub(&shares_to_claim)
                        .ok_or(Error::<T>::Overflow)?;
                    totals.total_collateral_contributed = totals
                        .total_collateral_contributed
                        .saturating_sub(core::cmp::min(
                            totals.total_collateral_contributed,
                            collateral_reduction,
                        ));
                    Ok(())
                })?;
                Self::debit_market_collateral(market_id, amount)?;
                T::Assets::transfer(market.collateral_asset, &Self::account_id(), who, amount)?;
                Self::deposit_event(Event::LiquidityClaimed {
                    market_id,
                    provider: who.clone(),
                    lp_shares: shares_to_claim,
                    amount,
                });
                Ok(amount)
            })
        }

        pub fn quote_buy_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
        ) -> Result<BuyQuoteOf<T>, DispatchError> {
            ensure!(!collateral_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_legacy_amm_market(&market)?;
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
            let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            let shares_out = Self::quote_buy(&pool, outcome, pricing_collateral)?;
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

        pub fn quote_sell_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<SellQuoteOf<T>, DispatchError> {
            ensure!(!shares_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_legacy_amm_market(&market)?;
            ensure!(
                matches!(Self::effective_market_status(&market), MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );

            let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            let gross_collateral_out = Self::quote_sell(&pool, outcome, shares_in)?;
            ensure!(
                !gross_collateral_out.is_zero(),
                Error::<T>::TradeAmountTooSmall
            );
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

        pub fn quote_add_liquidity_market(
            market_id: MarketId,
            collateral_in: T::Balance,
        ) -> Result<LiquidityQuoteOf<T>, DispatchError> {
            ensure!(!collateral_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_legacy_amm_market(&market)?;
            ensure!(
                matches!(Self::effective_market_status(&market), MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );
            let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            let totals = LiquidityPositionTotals::<T>::get(market_id);
            let lp_shares_out = Self::quote_lp_shares(&pool, &totals, collateral_in)?;
            ensure!(!lp_shares_out.is_zero(), Error::<T>::TradeAmountTooSmall);

            Ok(LiquidityQuote {
                market_id,
                collateral_in,
                lp_shares_out,
                pool_collateral: pool.collateral,
                total_lp_shares: totals.total_shares,
            })
        }

        pub fn quote_flip_position_market(
            market_id: MarketId,
            from_outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<FlipQuoteOf<T>, DispatchError> {
            ensure!(!shares_in.is_zero(), Error::<T>::InvalidTradeAmount);
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_legacy_amm_market(&market)?;
            ensure!(
                matches!(Self::effective_market_status(&market), MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );

            let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            let gross_collateral_out = Self::quote_sell(&pool, from_outcome, shares_in)?;
            ensure!(
                !gross_collateral_out.is_zero(),
                Error::<T>::TradeAmountTooSmall
            );
            let sell_fee_amount = Self::trade_fee(gross_collateral_out);
            let collateral_reinvested = gross_collateral_out
                .checked_sub(&sell_fee_amount)
                .ok_or(Error::<T>::TradeAmountTooSmall)?;
            ensure!(
                !collateral_reinvested.is_zero(),
                Error::<T>::TradeAmountTooSmall
            );

            let sell_fee_split = Self::split_trade_fee(sell_fee_amount);
            let pool_after_sell = Self::pool_after_sell(
                pool,
                from_outcome,
                shares_in,
                gross_collateral_out,
                sell_fee_split.pool,
            )?;
            let buy_fee_amount = Self::trade_fee(collateral_reinvested);
            let pricing_collateral = collateral_reinvested
                .checked_sub(&buy_fee_amount)
                .ok_or(Error::<T>::TradeAmountTooSmall)?;
            ensure!(
                !pricing_collateral.is_zero(),
                Error::<T>::TradeAmountTooSmall
            );
            let to_outcome = from_outcome.opposite();
            let shares_out = Self::quote_buy(&pool_after_sell, to_outcome, pricing_collateral)?;
            ensure!(!shares_out.is_zero(), Error::<T>::TradeAmountTooSmall);

            Ok(FlipQuote {
                market_id,
                from_outcome,
                to_outcome,
                shares_in,
                gross_collateral_out,
                sell_fee_amount,
                collateral_reinvested,
                buy_fee_amount,
                pricing_collateral,
                shares_out,
            })
        }

        pub fn quote_order_market(
            market_id: MarketId,
            outcome: BinaryOutcome,
            side: OrderSide,
            price_cents: PriceCents,
            shares: T::Balance,
        ) -> Result<OrderQuoteOf<T>, DispatchError> {
            ensure!(!shares.is_zero(), Error::<T>::InvalidTradeAmount);
            Self::ensure_order_lot_size(shares)?;
            Self::ensure_valid_price(price_cents)?;
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_order_book_market(&market)?;
            ensure!(
                matches!(Self::effective_market_status(&market), MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );

            let mut remaining = shares;
            let mut filled = T::Balance::zero();
            let mut collateral_in = T::Balance::zero();
            let mut collateral_out = T::Balance::zero();
            let mut fee_amount = T::Balance::zero();
            let mut fills = 0u32;
            let mut simulated_fills: Vec<(OrderId, T::Balance)> = Vec::new();

            while !remaining.is_zero() && fills < T::MaxFillsPerOrder::get() {
                let candidate = match side {
                    OrderSide::Buy => Self::best_buy_match_readonly(
                        market_id,
                        outcome,
                        price_cents,
                        &simulated_fills,
                    ),
                    OrderSide::Sell => Self::best_same_outcome_buy_readonly(
                        market_id,
                        outcome,
                        price_cents,
                        &simulated_fills,
                    )
                    .map(|id| (id, false)),
                };
                let Some((order_id, complementary)) = candidate else {
                    break;
                };
                let Some(order) = Orders::<T>::get(order_id) else {
                    break;
                };
                let Some(simulated_remaining) =
                    Self::simulated_remaining_shares(order_id, &order, &simulated_fills)
                else {
                    break;
                };
                let fill = core::cmp::min(remaining, simulated_remaining);
                if fill.is_zero() {
                    break;
                }
                if complementary {
                    let maker_notional = Self::price_amount(order.price_cents, fill)?;
                    let taker_notional = fill.saturating_sub(maker_notional);
                    let fee = Self::orderbook_trade_fee(taker_notional);
                    collateral_in = collateral_in
                        .checked_add(
                            &taker_notional
                                .checked_add(&fee)
                                .ok_or(Error::<T>::Overflow)?,
                        )
                        .ok_or(Error::<T>::Overflow)?;
                    fee_amount = fee_amount.saturating_add(fee);
                } else {
                    let notional = Self::price_amount(order.price_cents, fill)?;
                    let fee = Self::orderbook_trade_fee(notional);
                    match side {
                        OrderSide::Buy => {
                            collateral_in = collateral_in
                                .checked_add(
                                    &notional.checked_add(&fee).ok_or(Error::<T>::Overflow)?,
                                )
                                .ok_or(Error::<T>::Overflow)?;
                        }
                        OrderSide::Sell => {
                            collateral_out = collateral_out
                                .checked_add(
                                    &notional.checked_sub(&fee).ok_or(Error::<T>::Overflow)?,
                                )
                                .ok_or(Error::<T>::Overflow)?;
                        }
                    }
                    fee_amount = fee_amount.saturating_add(fee);
                }
                filled = filled.saturating_add(fill);
                remaining = remaining.saturating_sub(fill);
                Self::record_simulated_fill(&mut simulated_fills, order_id, fill)?;
                fills = fills.saturating_add(1);
            }

            Ok(OrderQuote {
                market_id,
                outcome,
                side,
                price_cents,
                shares,
                filled_shares: filled,
                posted_shares: remaining,
                collateral_in,
                collateral_out,
                fee_amount,
            })
        }

        pub fn order_book_depth(
            market_id: MarketId,
            outcome: BinaryOutcome,
            depth: u32,
        ) -> Result<OrderBookDepthOf<T>, DispatchError> {
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            Self::ensure_order_book_market(&market)?;
            let mut bids = Vec::new();
            let mut asks = Vec::new();
            let bid_levels = OrderBookPriceLevels::<T>::get(market_id, (outcome, OrderSide::Buy));
            for (price, shares) in bid_levels.iter().rev() {
                if bids.len() as u32 >= depth {
                    break;
                }
                if !shares.is_zero() {
                    bids.push(OrderBookLevel {
                        price_cents: *price,
                        shares: *shares,
                    });
                }
            }
            let ask_levels = OrderBookPriceLevels::<T>::get(market_id, (outcome, OrderSide::Sell));
            for (price, shares) in ask_levels.iter() {
                if asks.len() as u32 >= depth {
                    break;
                }
                if !shares.is_zero() {
                    asks.push(OrderBookLevel {
                        price_cents: *price,
                        shares: *shares,
                    });
                }
            }
            Ok(OrderBookDepth { bids, asks })
        }

        pub fn claimable_info(
            who: T::AccountId,
            market_id: MarketId,
        ) -> Result<ClaimableInfoOf<T>, DispatchError> {
            let mut market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            market.status = Self::effective_market_status(&market);
            let resolution_outcome = MarketResolution::<T>::get(market_id);
            let position = MarketPositions::<T>::get(market_id, &who).unwrap_or_default();
            let (open_yes_shares, open_no_shares, open_collateral) =
                Self::open_order_exposure(&who, market_id);
            let trader_payout = match market.status {
                MarketStatus::Resolved => resolution_outcome
                    .map(|outcome| Self::winning_shares(&position, outcome))
                    .unwrap_or_default(),
                MarketStatus::Cancelled => {
                    if matches!(market.mechanism, MarketMechanism::OrderBook) {
                        Self::cancelled_order_book_payout(&position)?
                    } else {
                        position.net_collateral_paid
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
            let creator_liquidity = if is_creator
                && matches!(
                    market.status,
                    MarketStatus::Resolved | MarketStatus::Cancelled
                ) {
                if matches!(market.mechanism, MarketMechanism::LegacyAmm) {
                    let total_claimable = Self::creator_liquidity_claimable(market_id, &market)?;
                    let totals = LiquidityPositionTotals::<T>::get(market_id);
                    if totals.total_shares.is_zero() {
                        total_claimable
                    } else {
                        match LiquidityPositions::<T>::get(market_id, &who) {
                            Some(position) => Self::pro_rata(
                                total_claimable,
                                position.shares,
                                totals.total_shares,
                            )?,
                            None => T::Balance::zero(),
                        }
                    }
                } else {
                    T::Balance::zero()
                }
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
                open_yes_shares,
                open_no_shares,
                open_collateral,
                creator_fees,
                creator_liquidity,
                is_creator,
            })
        }

        fn ensure_market_can_finalize(
            market_id: MarketId,
        ) -> Result<(MarketOf<T>, bool), DispatchError> {
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

        fn split_trade_fee(amount: T::Balance) -> TradeFeeSplit<T::Balance> {
            let fee = amount.saturated_into::<u128>();
            let creator = fee.saturating_mul(10) / 100;
            let buyback = fee.saturating_mul(20) / 100;
            let pool = fee.saturating_sub(creator).saturating_sub(buyback);
            TradeFeeSplit {
                pool: pool.saturated_into::<T::Balance>(),
                creator: creator.saturated_into::<T::Balance>(),
                buyback: buyback.saturated_into::<T::Balance>(),
            }
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

        pub(crate) fn quote_buy(
            pool: &MarketPoolOf<T>,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let (selected, opposite) = Self::pool_reserves(pool, outcome);
            ensure!(
                !selected.is_zero() && !opposite.is_zero(),
                Error::<T>::Overflow
            );
            let selected_u = U256::from(selected.saturated_into::<u128>());
            let opposite_u = U256::from(opposite.saturated_into::<u128>());
            let input_u = U256::from(collateral_in.saturated_into::<u128>());
            let denominator = opposite_u + input_u;
            let numerator = selected_u * opposite_u;
            let selected_after = Self::div_ceil_u256(numerator, denominator);
            let shares_out = selected_u
                .checked_add(input_u)
                .ok_or(Error::<T>::Overflow)?
                .checked_sub(selected_after)
                .ok_or(Error::<T>::Overflow)?;
            Self::u256_to_balance(shares_out)
        }

        fn pool_after_buy(
            mut pool: MarketPoolOf<T>,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
            pool_fee: T::Balance,
        ) -> Result<MarketPoolOf<T>, DispatchError> {
            let shares_out = Self::quote_buy(&pool, outcome, collateral_in)?;
            let total_added = collateral_in
                .checked_add(&pool_fee)
                .ok_or(Error::<T>::Overflow)?;
            pool.collateral = pool
                .collateral
                .checked_add(&total_added)
                .ok_or(Error::<T>::Overflow)?;

            match outcome {
                BinaryOutcome::Yes => {
                    let yes_after = pool
                        .yes
                        .checked_add(&collateral_in)
                        .ok_or(Error::<T>::Overflow)?
                        .checked_sub(&shares_out)
                        .ok_or(Error::<T>::Overflow)?;
                    let no_after = pool
                        .no
                        .checked_add(&collateral_in)
                        .ok_or(Error::<T>::Overflow)?;
                    pool.yes = yes_after;
                    pool.no = no_after;
                }
                BinaryOutcome::No => {
                    let no_after = pool
                        .no
                        .checked_add(&collateral_in)
                        .ok_or(Error::<T>::Overflow)?
                        .checked_sub(&shares_out)
                        .ok_or(Error::<T>::Overflow)?;
                    let yes_after = pool
                        .yes
                        .checked_add(&collateral_in)
                        .ok_or(Error::<T>::Overflow)?;
                    pool.no = no_after;
                    pool.yes = yes_after;
                }
            }
            Ok(pool)
        }

        pub(crate) fn quote_sell(
            pool: &MarketPoolOf<T>,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let (selected, opposite) = Self::pool_reserves(pool, outcome);
            ensure!(
                !selected.is_zero() && !opposite.is_zero(),
                Error::<T>::Overflow
            );
            let selected_u = U256::from(selected.saturated_into::<u128>());
            let opposite_u = U256::from(opposite.saturated_into::<u128>());
            let shares_u = U256::from(shares_in.saturated_into::<u128>());
            let invariant = selected_u * opposite_u;
            let selected_after_base = selected_u
                .checked_add(shares_u)
                .ok_or(Error::<T>::Overflow)?;

            let mut low = U256::zero();
            let mut high = opposite_u;
            while low < high {
                let mid = low + ((high - low + U256::one()) / U256::from(2u8));
                let lhs = match selected_after_base.checked_sub(mid) {
                    Some(selected_after) => selected_after
                        .checked_mul(opposite_u - mid)
                        .unwrap_or(U256::MAX),
                    None => U256::zero(),
                };
                if lhs >= invariant {
                    low = mid;
                } else {
                    high = mid.saturating_sub(U256::one());
                }
            }
            Self::u256_to_balance(low)
        }

        fn pool_after_sell(
            mut pool: MarketPoolOf<T>,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
            gross_collateral_out: T::Balance,
            pool_fee: T::Balance,
        ) -> Result<MarketPoolOf<T>, DispatchError> {
            let collateral_delta = gross_collateral_out
                .checked_sub(&pool_fee)
                .ok_or(Error::<T>::Overflow)?;
            pool.collateral = pool
                .collateral
                .checked_sub(&collateral_delta)
                .ok_or(Error::<T>::Overflow)?;

            match outcome {
                BinaryOutcome::Yes => {
                    pool.yes = pool
                        .yes
                        .checked_add(&shares_in)
                        .ok_or(Error::<T>::Overflow)?
                        .checked_sub(&gross_collateral_out)
                        .ok_or(Error::<T>::Overflow)?;
                    pool.no = pool
                        .no
                        .checked_sub(&gross_collateral_out)
                        .ok_or(Error::<T>::Overflow)?;
                }
                BinaryOutcome::No => {
                    pool.no = pool
                        .no
                        .checked_add(&shares_in)
                        .ok_or(Error::<T>::Overflow)?
                        .checked_sub(&gross_collateral_out)
                        .ok_or(Error::<T>::Overflow)?;
                    pool.yes = pool
                        .yes
                        .checked_sub(&gross_collateral_out)
                        .ok_or(Error::<T>::Overflow)?;
                }
            }
            Ok(pool)
        }

        fn pool_reserves(
            pool: &MarketPoolOf<T>,
            outcome: BinaryOutcome,
        ) -> (T::Balance, T::Balance) {
            match outcome {
                BinaryOutcome::Yes => (pool.yes, pool.no),
                BinaryOutcome::No => (pool.no, pool.yes),
            }
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

        fn debit_position_on_sell(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
            gross_collateral_out: T::Balance,
        ) -> DispatchResult {
            let mut net_paid_reduction = T::Balance::zero();
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
                    net_paid_reduction =
                        core::cmp::min(entry.net_collateral_paid, gross_collateral_out);
                    entry.net_collateral_paid =
                        entry.net_collateral_paid.saturating_sub(net_paid_reduction);
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
                    .checked_sub(&net_paid_reduction)
                    .ok_or(Error::<T>::Overflow)?;
                Ok(())
            })
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

        fn debit_market_collateral(market_id: MarketId, amount: T::Balance) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            MarketPools::<T>::try_mutate(market_id, |pool| -> DispatchResult {
                let pool = pool.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                ensure!(pool.collateral >= amount, Error::<T>::Overflow);
                pool.collateral = pool.collateral.saturating_sub(amount);
                Ok(())
            })
        }

        fn winning_shares(position: &MarketPositionOf<T>, outcome: BinaryOutcome) -> T::Balance {
            match outcome {
                BinaryOutcome::Yes => position.yes_shares,
                BinaryOutcome::No => position.no_shares,
            }
        }

        fn cancelled_order_book_payout(
            position: &MarketPositionOf<T>,
        ) -> Result<T::Balance, DispatchError> {
            let total_shares = position
                .yes_shares
                .checked_add(&position.no_shares)
                .ok_or(Error::<T>::Overflow)?;
            Self::pro_rata(total_shares, One::one(), 2u32.into())
        }

        fn open_order_exposure(
            who: &T::AccountId,
            market_id: MarketId,
        ) -> (T::Balance, T::Balance, T::Balance) {
            let mut open_yes = T::Balance::zero();
            let mut open_no = T::Balance::zero();
            let mut open_collateral = T::Balance::zero();
            for order_id in OpenOrdersByAccountMarket::<T>::get(who, market_id).into_iter() {
                let Some(order) = Orders::<T>::get(order_id) else {
                    continue;
                };
                match order.side {
                    OrderSide::Buy => {
                        open_collateral = open_collateral.saturating_add(order.reserved_collateral);
                    }
                    OrderSide::Sell => match order.outcome {
                        BinaryOutcome::Yes => {
                            open_yes = open_yes.saturating_add(order.remaining_shares);
                        }
                        BinaryOutcome::No => {
                            open_no = open_no.saturating_add(order.remaining_shares);
                        }
                    },
                }
            }
            (open_yes, open_no, open_collateral)
        }

        fn creator_liquidity_claimable(
            market_id: MarketId,
            market: &MarketOf<T>,
        ) -> Result<T::Balance, DispatchError> {
            let pool = MarketPools::<T>::get(market_id).ok_or(Error::<T>::MarketUnknown)?;
            let totals = MarketPositionTotals::<T>::get(market_id);
            let locked = match market.status {
                MarketStatus::Resolved => {
                    let outcome = MarketResolution::<T>::get(market_id)
                        .ok_or(Error::<T>::MarketNotResolved)?;
                    match outcome {
                        BinaryOutcome::Yes => totals.total_yes_shares,
                        BinaryOutcome::No => totals.total_no_shares,
                    }
                }
                MarketStatus::Cancelled => totals.total_net_collateral_paid,
                _ => return Err(Error::<T>::MarketNotFinalized.into()),
            };
            Ok(pool.collateral.saturating_sub(locked))
        }

        fn div_ceil_u256(numerator: U256, denominator: U256) -> U256 {
            if numerator.is_zero() {
                return U256::zero();
            }
            ((numerator - U256::one()) / denominator) + U256::one()
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

        fn quote_lp_shares(
            pool: &MarketPoolOf<T>,
            totals: &LiquidityTotalsOf<T>,
            collateral_amount: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            if totals.total_shares.is_zero() || pool.collateral.is_zero() {
                Ok(collateral_amount)
            } else {
                Self::pro_rata(collateral_amount, totals.total_shares, pool.collateral)
            }
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

        fn ensure_next_condition_id_available() -> DispatchResult {
            NextConditionId::<T>::get()
                .checked_add(One::one())
                .ok_or(Error::<T>::Overflow)?;
            Ok(())
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
        Encode,
        Decode,
        DecodeWithMemTracking,
        TypeInfo,
        Clone,
        PartialEq,
        Eq,
        RuntimeDebug,
        MaxEncodedLen,
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
}

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
