#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use common::BuyBackHandler;
use frame_support::{
    dispatch::DispatchResult, storage::with_transaction, transactional, weights::Weight,
    BoundedVec, PalletId,
};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::traits::{
    AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, MaybeSerializeDeserialize, One,
    SaturatedConversion, Saturating, Zero,
};
use sp_runtime::{DispatchError, Perbill, RuntimeDebug, TransactionOutcome};
use sp_std::{marker::PhantomData, vec::Vec};

mod weights;
pub use weights::SoraWeight;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub type ConditionId = u32;
pub type MarketId = u32;

const STORAGE_VERSION: frame_support::traits::StorageVersion =
    frame_support::traits::StorageVersion::new(1);
const CREATION_FEE_BUYBACK_BPS: u32 = 2_000;

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
    fn create_opengov_condition() -> Weight;
    fn create_market() -> Weight;
    fn buy() -> Weight;
    fn sell() -> Weight;
    fn sync_market_status() -> Weight;
    fn bond_governance() -> Weight;
    fn unbond_governance() -> Weight;
    fn resolve_market() -> Weight;
    fn cancel_market() -> Weight;
    fn claim_market() -> Weight;
    fn claim_creator_fees() -> Weight;
    fn claim_creator_liquidity() -> Weight;
    fn sweep_xor_buyback_and_burn() -> Weight;
}

/// Hook notifying off-chain integrations (e.g., Polkadot Plaza) when a condition
/// tied to an OpenGov referendum is registered.
pub trait PlazaIntegrationHook<Metadata> {
    fn on_opengov_condition(_condition_id: ConditionId, _metadata: &Metadata) {}
}

impl<Metadata> PlazaIntegrationHook<Metadata> for () {}

pub struct PolkadotPlazaBridge<T>(PhantomData<T>);

impl<T: Config> PlazaIntegrationHook<OpengovProposalOf<T>> for PolkadotPlazaBridge<T> {
    fn on_opengov_condition(condition_id: ConditionId, metadata: &OpengovProposalOf<T>) {
        if metadata.plaza_tag.is_empty() {
            return;
        }
        let tag: Vec<u8> = metadata.plaza_tag.clone().into();
        Pallet::<T>::deposit_event(Event::PolkadotPlazaBroadcast { condition_id, tag });
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
    Default,
)]
pub enum RelayNetwork {
    #[default]
    Polkadot,
    Kusama,
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
pub struct OpengovProposalMetadata<Tag> {
    pub network: RelayNetwork,
    pub parachain_id: u32,
    pub track_id: u16,
    pub referendum_index: u32,
    pub plaza_tag: Tag,
}

#[derive(
    Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, Default,
)]
pub struct OpengovProposalInput {
    pub network: RelayNetwork,
    pub parachain_id: u32,
    pub track_id: u16,
    pub referendum_index: u32,
    pub plaza_tag: Vec<u8>,
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

#[derive(Clone, Copy, PartialEq, Eq, RuntimeDebug)]
struct TradeFeeSplit<Balance> {
    pool: Balance,
    creator: Balance,
    buyback: Balance,
}

pub type MetadataString<T> = BoundedVec<u8, <T as pallet::Config>::MaxMetadataLength>;
pub type ConditionMetadataOf<T> = ConditionMetadata<MetadataString<T>>;

pub type PlazaTagOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxPlazaTagLength>;
pub type OpengovProposalOf<T> = OpengovProposalMetadata<PlazaTagOf<T>>;

pub type MarketOf<T> = Market<
    <T as Config>::AssetId,
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::Balance,
>;

pub type MarketPoolOf<T> = MarketPool<<T as Config>::Balance>;
pub type MarketPositionOf<T> = MarketPosition<<T as Config>::Balance>;
pub type MarketTotalsOf<T> = MarketTotals<<T as Config>::Balance>;

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

        /// Creation fee expressed in basis points (e.g., 35 == 0.35%).
        #[pallet::constant]
        type CreationFeeBps: Get<u32>;

        /// Minimum absolute creation fee in canonical stable units.
        #[pallet::constant]
        type MinCreationFee: Get<Self::Balance>;

        /// Pallet identifier for deriving the escrow account.
        #[pallet::constant]
        type PalletId: Get<PalletId>;

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
        /// Maximum length for Polkadot Plaza tags associated with OpenGov proposals.
        #[pallet::constant]
        type MaxPlazaTagLength: Get<u32>;
        /// Weight information for extrinsics.
        type WeightInfo: crate::WeightInfo;

        /// Trade fee expressed in basis points (e.g., 50 == 0.50%).
        #[pallet::constant]
        type TradeFeeBps: Get<u32>;

        /// Minimum bond required to join the governance whitelist (canonical stable units).
        #[pallet::constant]
        type GovernanceBondMinimum: Get<Self::Balance>;

        /// Canonical stable escrow account backing creator bonds.
        #[pallet::constant]
        type CreatorBondEscrowAccount: Get<Self::AccountId>;

        /// Origin allowed to finalize market outcomes.
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Hook for notifying third-party integrations (e.g., Polkadot Plaza).
        type PlazaIntegration: PlazaIntegrationHook<OpengovProposalOf<Self>>;
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
    #[pallet::getter(fn conditions)]
    pub type Conditions<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, ConditionMetadataOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn markets)]
    pub type Markets<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketOf<T>, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_pool)]
    pub type MarketPools<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, MarketPoolOf<T>, OptionQuery>;

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
    #[pallet::getter(fn governance_bonds)]
    pub type GovernanceBonds<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn creator_locked_bond)]
    pub type CreatorLockedBond<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_bond_lock)]
    pub type MarketBondLock<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn opengov_proposals)]
    pub type OpengovConditions<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, OpengovProposalOf<T>, OptionQuery>;

    #[pallet::storage]
    pub type FeeCollectorOverride<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type GovernanceBondMinimumOverride<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fee_collector: Option<T::AccountId>,
        pub governance_bond_minimum: Option<T::Balance>,
    }

    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                fee_collector: None,
                governance_bond_minimum: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref account) = self.fee_collector {
                FeeCollectorOverride::<T>::put(account.clone());
            }
            if let Some(value) = self.governance_bond_minimum {
                GovernanceBondMinimumOverride::<T>::put(value);
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ConditionCreated {
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
        MarketLocked {
            market_id: MarketId,
        },
        MarketResolved {
            market_id: MarketId,
            outcome: BinaryOutcome,
        },
        MarketCancelled {
            market_id: MarketId,
        },
        MarketClaimed {
            market_id: MarketId,
            trader: T::AccountId,
            payout: T::Balance,
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
        GovernanceBonded {
            who: T::AccountId,
            amount: T::Balance,
        },
        GovernanceUnbonded {
            who: T::AccountId,
            amount: T::Balance,
        },
        HollarRouted {
            user: T::AccountId,
            amount: T::Balance,
        },
        OpengovConditionCreated {
            condition_id: ConditionId,
            network: RelayNetwork,
            parachain_id: u32,
            track_id: u16,
            referendum_index: u32,
        },
        PolkadotPlazaBroadcast {
            condition_id: ConditionId,
            tag: Vec<u8>,
        },
        OpengovConditionCleared {
            condition_id: ConditionId,
        },
        MarketBondLockReleased {
            market_id: MarketId,
            creator: T::AccountId,
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
        GovernanceBondTooLow,
        AccountNotBonded,
        GovernanceBondLocked,
        InvalidOpengovProposal,
        MarketUnknown,
        TradeAmountTooSmall,
        ZeroSeedLiquidity,
        InsufficientShares,
        NotMarketCreator,
        NothingToClaim,
        NothingToSweep,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new prediction condition with oracle metadata.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_condition())]
        pub fn create_condition(origin: OriginFor<T>, metadata: ConditionInput) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_authorized_creator(&who)?;
            Self::create_condition_entry(&who, metadata)?;
            Ok(())
        }

        /// Register an OpenGov-linked condition with oracle metadata.
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::create_opengov_condition())]
        #[transactional]
        pub fn create_opengov_condition(
            origin: OriginFor<T>,
            metadata: ConditionInput,
            proposal: OpengovProposalInput,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_authorized_creator(&who)?;
            ensure!(
                proposal.parachain_id > 0,
                Error::<T>::InvalidOpengovProposal
            );
            let condition_id = Self::create_condition_entry(&who, metadata)?;
            let plaza_tag = PlazaTagOf::<T>::try_from(proposal.plaza_tag)
                .map_err(|_| Error::<T>::MetadataTooLong)?;
            let record = OpengovProposalMetadata {
                network: proposal.network,
                parachain_id: proposal.parachain_id,
                track_id: proposal.track_id,
                referendum_index: proposal.referendum_index,
                plaza_tag,
            };
            let network = record.network;
            let parachain_id = record.parachain_id;
            let track_id = record.track_id;
            let referendum_index = record.referendum_index;
            OpengovConditions::<T>::insert(condition_id, record.clone());
            T::PlazaIntegration::on_opengov_condition(condition_id, &record);
            Self::deposit_event(Event::OpengovConditionCreated {
                condition_id,
                network,
                parachain_id,
                track_id,
                referendum_index,
            });
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

        /// Create a market for a registered condition and seed it with canonical stable collateral.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::create_market())]
        #[transactional]
        pub fn create_market(
            origin: OriginFor<T>,
            condition_id: ConditionId,
            close_block: BlockNumberFor<T>,
            seed_liquidity: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_authorized_creator(&who)?;
            ensure!(
                Conditions::<T>::contains_key(condition_id),
                Error::<T>::ConditionNotFound
            );
            ensure!(!seed_liquidity.is_zero(), Error::<T>::ZeroSeedLiquidity);
            let now = <frame_system::Pallet<T>>::block_number();
            let min_close = now
                .checked_add(&T::MinMarketDuration::get())
                .ok_or(Error::<T>::Overflow)?;
            ensure!(close_block >= min_close, Error::<T>::MarketDurationTooShort);

            Self::withdraw_creation_fee(&who, seed_liquidity)?;

            let market_id =
                NextMarketId::<T>::try_mutate(|next_id| -> Result<MarketId, DispatchError> {
                    let id = *next_id;
                    *next_id = next_id
                        .checked_add(One::one())
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(id)
                })?;

            let deposited = Self::escrow_seed_liquidity(&who, seed_liquidity)?;
            let data = Market {
                creator: who.clone(),
                condition_id,
                close_block,
                collateral_asset: T::CanonicalStableAssetId::get(),
                seed_liquidity: deposited,
                status: MarketStatus::Open,
            };
            Markets::<T>::insert(market_id, data);
            MarketPools::<T>::insert(
                market_id,
                MarketPool {
                    collateral: deposited,
                    yes: deposited,
                    no: deposited,
                },
            );
            Self::lock_creator_bond_for_market(&who, market_id)?;
            Self::deposit_event(Event::MarketCreated {
                market_id,
                seed_liquidity: deposited,
            });
            Self::deposit_event(Event::CollateralSeeded {
                market_id,
                amount: deposited,
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
            let _market = Self::ensure_market_tradable(market_id)?;
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

                T::Assets::transfer(
                    T::CanonicalStableAssetId::get(),
                    &who,
                    &Self::account_id(),
                    collateral_in,
                )?;
                let fee_split = Self::apply_trade_fees(market_id, total_fee)?;
                let updated_pool =
                    Self::pool_after_buy(pool, outcome, pricing_input, fee_split.pool)?;
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

                let fee_split = Self::apply_trade_fees(market_id, total_fee)?;
                let updated_pool = Self::pool_after_sell(
                    pool,
                    outcome,
                    shares_in,
                    gross_collateral_out,
                    fee_split.pool,
                )?;
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

        /// Bond canonical stable as creator collateral.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::bond_governance())]
        pub fn bond_governance(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            let min_bond = Self::governance_bond_minimum();
            ensure!(amount >= min_bond, Error::<T>::GovernanceBondTooLow);
            let received =
                Self::deposit_canonical(&who, &Self::creator_bond_escrow_account(), amount)?;
            GovernanceBonds::<T>::mutate(&who, |bond| {
                *bond = bond.saturating_add(received);
            });
            Self::deposit_event(Event::GovernanceBonded {
                who,
                amount: received,
            });
            Ok(())
        }

        /// Withdraw bonded creator collateral that is not locked by active markets.
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::unbond_governance())]
        #[transactional]
        pub fn unbond_governance(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            GovernanceBonds::<T>::try_mutate(&who, |bond| -> DispatchResult {
                ensure!(*bond >= amount, Error::<T>::AccountNotBonded);
                let remaining = bond.saturating_sub(amount);
                ensure!(
                    remaining >= CreatorLockedBond::<T>::get(&who),
                    Error::<T>::GovernanceBondLocked
                );
                *bond = remaining;
                Ok(())
            })?;
            T::Assets::transfer(
                T::CanonicalStableAssetId::get(),
                &Self::creator_bond_escrow_account(),
                &who,
                amount,
            )?;
            Self::deposit_event(Event::GovernanceUnbonded { who, amount });
            Ok(())
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
            let (market, _) = Self::ensure_market_can_finalize(market_id)?;
            let creator = market.creator;
            let condition_id = market.condition_id;
            with_storage_transaction(|| -> DispatchResult {
                Markets::<T>::try_mutate(market_id, |market| -> DispatchResult {
                    let market = market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    market.status = MarketStatus::Resolved;
                    Ok(())
                })?;
                MarketResolution::<T>::insert(market_id, outcome);
                Self::clear_linked_opengov_condition(condition_id);
                Self::unlock_market_bond(market_id, &creator)?;
                Self::deposit_event(Event::MarketResolved { market_id, outcome });
                Ok(())
            })
        }

        /// Cancel an expired market and unlock cancellation refunds.
        #[pallet::call_index(21)]
        #[pallet::weight(T::WeightInfo::cancel_market())]
        pub fn cancel_market(origin: OriginFor<T>, market_id: MarketId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            let (market, _) = Self::ensure_market_can_finalize(market_id)?;
            let creator = market.creator;
            let condition_id = market.condition_id;
            with_storage_transaction(|| -> DispatchResult {
                Markets::<T>::try_mutate(market_id, |market| -> DispatchResult {
                    let market = market.as_mut().ok_or(Error::<T>::MarketUnknown)?;
                    market.status = MarketStatus::Cancelled;
                    Ok(())
                })?;
                MarketResolution::<T>::remove(market_id);
                Self::clear_linked_opengov_condition(condition_id);
                Self::unlock_market_bond(market_id, &creator)?;
                Self::deposit_event(Event::MarketCancelled { market_id });
                Ok(())
            })
        }

        /// Claim a resolved payout or cancellation refund.
        #[pallet::call_index(22)]
        #[pallet::weight(T::WeightInfo::claim_market())]
        pub fn claim_market(origin: OriginFor<T>, market_id: MarketId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            let position =
                MarketPositions::<T>::get(market_id, &who).ok_or(Error::<T>::NothingToClaim)?;
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

            with_storage_transaction(|| -> DispatchResult {
                MarketPositions::<T>::remove(market_id, &who);
                Self::debit_market_totals(market_id, &position);
                Self::debit_market_collateral(market_id, payout)?;
                if !payout.is_zero() {
                    T::Assets::transfer(
                        market.collateral_asset,
                        &Self::account_id(),
                        &who,
                        payout,
                    )?;
                }
                Self::deposit_event(Event::MarketClaimed {
                    market_id,
                    trader: who.clone(),
                    payout,
                });
                Ok(())
            })
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
            ensure!(market.creator == who, Error::<T>::NotMarketCreator);
            let amount = Self::creator_liquidity_claimable(market_id, &market)?;
            ensure!(!amount.is_zero(), Error::<T>::NothingToClaim);
            with_storage_transaction(|| -> DispatchResult {
                Self::debit_market_collateral(market_id, amount)?;
                T::Assets::transfer(market.collateral_asset, &Self::account_id(), &who, amount)?;
                Self::deposit_event(Event::CreatorLiquidityClaimed {
                    market_id,
                    creator: who.clone(),
                    amount,
                });
                Ok(())
            })
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

        fn creator_bond_escrow_account() -> T::AccountId {
            T::CreatorBondEscrowAccount::get()
        }

        fn governance_bond_minimum() -> T::Balance {
            GovernanceBondMinimumOverride::<T>::get().unwrap_or_else(T::GovernanceBondMinimum::get)
        }

        fn withdraw_creation_fee(who: &T::AccountId, seed: T::Balance) -> DispatchResult {
            let bps = T::CreationFeeBps::get();
            let ratio = Perbill::from_rational(bps, 10_000u32);
            let fee_from_bps = ratio * seed;
            let min_fee = T::MinCreationFee::get();
            let fee = if fee_from_bps < min_fee {
                min_fee
            } else {
                fee_from_bps
            };

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

        fn escrow_seed_liquidity(
            who: &T::AccountId,
            amount: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            Self::deposit_canonical(who, &Self::account_id(), amount)
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

        fn ensure_market_tradable(market_id: MarketId) -> Result<MarketOf<T>, DispatchError> {
            let (market, _) = Self::sync_market_status_if_needed(market_id)?;
            ensure!(
                matches!(market.status, MarketStatus::Open),
                Error::<T>::MarketNotOpen
            );
            Ok(market)
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

        fn apply_trade_fees(
            market_id: MarketId,
            total_fee: T::Balance,
        ) -> Result<TradeFeeSplit<T::Balance>, DispatchError> {
            let split = Self::split_trade_fee(total_fee);
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
            Ok(split)
        }

        fn quote_buy(
            pool: &MarketPoolOf<T>,
            outcome: BinaryOutcome,
            collateral_in: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let (selected, opposite) = Self::pool_reserves(pool, outcome);
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

        fn quote_sell(
            pool: &MarketPoolOf<T>,
            outcome: BinaryOutcome,
            shares_in: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            let (selected, opposite) = Self::pool_reserves(pool, outcome);
            let selected_u = selected.saturated_into::<u128>();
            let opposite_u = opposite.saturated_into::<u128>();
            let shares_u = shares_in.saturated_into::<u128>();
            let invariant = U256::from(selected_u) * U256::from(opposite_u);

            let mut low = 0u128;
            let mut high = opposite_u;
            while low < high {
                let mid = low + (high - low + 1) / 2;
                let lhs = U256::from(selected_u.saturating_add(shares_u).saturating_sub(mid))
                    * U256::from(opposite_u.saturating_sub(mid));
                if lhs >= invariant {
                    low = mid;
                } else {
                    high = mid.saturating_sub(1);
                }
            }
            Ok(low.saturated_into::<T::Balance>())
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

        fn credit_position_on_buy(
            market_id: MarketId,
            who: &T::AccountId,
            outcome: BinaryOutcome,
            shares: T::Balance,
            collateral_paid: T::Balance,
        ) -> DispatchResult {
            MarketPositions::<T>::mutate(market_id, who, |position| {
                let entry = position.get_or_insert_with(Default::default);
                match outcome {
                    BinaryOutcome::Yes => {
                        entry.yes_shares = entry.yes_shares.saturating_add(shares);
                    }
                    BinaryOutcome::No => {
                        entry.no_shares = entry.no_shares.saturating_add(shares);
                    }
                }
                entry.net_collateral_paid =
                    entry.net_collateral_paid.saturating_add(collateral_paid);
            });
            MarketPositionTotals::<T>::mutate(market_id, |totals| {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.total_yes_shares = totals.total_yes_shares.saturating_add(shares);
                    }
                    BinaryOutcome::No => {
                        totals.total_no_shares = totals.total_no_shares.saturating_add(shares);
                    }
                }
                totals.total_net_collateral_paid = totals
                    .total_net_collateral_paid
                    .saturating_add(collateral_paid);
            });
            Ok(())
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
            MarketPositionTotals::<T>::mutate(market_id, |totals| {
                match outcome {
                    BinaryOutcome::Yes => {
                        totals.total_yes_shares = totals.total_yes_shares.saturating_sub(shares_in);
                    }
                    BinaryOutcome::No => {
                        totals.total_no_shares = totals.total_no_shares.saturating_sub(shares_in);
                    }
                }
                totals.total_net_collateral_paid = totals
                    .total_net_collateral_paid
                    .saturating_sub(net_paid_reduction);
            });
            Ok(())
        }

        fn debit_market_totals(market_id: MarketId, position: &MarketPositionOf<T>) {
            MarketPositionTotals::<T>::mutate(market_id, |totals| {
                totals.total_yes_shares =
                    totals.total_yes_shares.saturating_sub(position.yes_shares);
                totals.total_no_shares = totals.total_no_shares.saturating_sub(position.no_shares);
                totals.total_net_collateral_paid = totals
                    .total_net_collateral_paid
                    .saturating_sub(position.net_collateral_paid);
            });
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

        fn unlock_market_bond(market_id: MarketId, creator: &T::AccountId) -> DispatchResult {
            let Some(amount) = MarketBondLock::<T>::take(market_id) else {
                return Ok(());
            };
            CreatorLockedBond::<T>::mutate(creator, |locked| {
                *locked = locked.saturating_sub(amount);
            });
            Self::deposit_event(Event::MarketBondLockReleased {
                market_id,
                creator: creator.clone(),
                amount,
            });
            Ok(())
        }

        fn div_ceil_u256(numerator: U256, denominator: U256) -> U256 {
            if numerator.is_zero() {
                return U256::zero();
            }
            ((numerator - U256::one()) / denominator) + U256::one()
        }

        fn u256_to_balance(value: U256) -> Result<T::Balance, DispatchError> {
            let raw = u128::try_from(value).map_err(|_| Error::<T>::Overflow)?;
            Ok(raw.saturated_into::<T::Balance>())
        }

        fn clear_linked_opengov_condition(condition_id: ConditionId) {
            if OpengovConditions::<T>::take(condition_id).is_some() {
                Self::deposit_event(Event::OpengovConditionCleared { condition_id });
            }
        }

        fn ensure_authorized_creator(who: &T::AccountId) -> DispatchResult {
            let min_bond = Self::governance_bond_minimum();
            ensure!(
                GovernanceBonds::<T>::get(who) >= min_bond,
                Error::<T>::AccountNotBonded
            );
            Ok(())
        }

        fn create_condition_entry(
            _who: &T::AccountId,
            metadata: ConditionInput,
        ) -> Result<ConditionId, DispatchError> {
            ensure!(
                metadata.question.len() as u32 >= T::MinQuestionLength::get(),
                Error::<T>::QuestionTooShort
            );
            let bounded = ConditionMetadata {
                question: MetadataString::<T>::try_from(metadata.question)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
                oracle: MetadataString::<T>::try_from(metadata.oracle)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
                resolution_source: MetadataString::<T>::try_from(metadata.resolution_source)
                    .map_err(|_| Error::<T>::MetadataTooLong)?,
            };

            let condition_id = NextConditionId::<T>::try_mutate(
                |next_id| -> Result<ConditionId, DispatchError> {
                    let id = *next_id;
                    *next_id = next_id
                        .checked_add(One::one())
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(id)
                },
            )?;

            Conditions::<T>::insert(condition_id, bounded);
            Self::deposit_event(Event::ConditionCreated { condition_id });
            Ok(condition_id)
        }

        fn lock_creator_bond_for_market(
            creator: &T::AccountId,
            market_id: MarketId,
        ) -> DispatchResult {
            let lock_amount = Self::governance_bond_minimum();
            if lock_amount.is_zero() {
                return Ok(());
            }
            let total_locked = CreatorLockedBond::<T>::get(creator);
            let required_locked = total_locked
                .checked_add(&lock_amount)
                .ok_or(Error::<T>::Overflow)?;
            ensure!(
                GovernanceBonds::<T>::get(creator) >= required_locked,
                Error::<T>::GovernanceBondTooLow
            );
            CreatorLockedBond::<T>::insert(creator, required_locked);
            MarketBondLock::<T>::insert(market_id, lock_amount);
            Ok(())
        }
    }
}

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
