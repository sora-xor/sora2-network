#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{dispatch::DispatchResult, weights::Weight, BoundedVec, PalletId};
use frame_system::pallet_prelude::BlockNumberFor;
use scale_info::TypeInfo;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::{
    AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, MaybeSerializeDeserialize, One,
    SaturatedConversion, Saturating, Zero,
};
use sp_runtime::{DispatchError, Perbill, RuntimeDebug};
use sp_std::{marker::PhantomData, vec::Vec};

mod weights;
pub use weights::{HydraWeight, SubstrateWeight};

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub type ConditionId = u32;
pub type MarketId = u32;
pub type CommitmentHash = [u8; 32];
pub type JurisdictionCode = [u8; 3];

pub trait WeightInfo {
    fn create_condition() -> Weight;
    fn create_market(routed_transfers: u32) -> Weight;
    fn commit_order() -> Weight;
    fn reveal_order() -> Weight;
    fn set_bridge_wallet() -> Weight;
    fn bridge_deposit() -> Weight;
    fn bridge_withdraw() -> Weight;
}

/// Abstract interface for the orderbook pallet integration.
pub trait OrderbookBridge<AccountId, AssetId, Balance> {
    fn on_market_created(
        market_id: MarketId,
        creator: &AccountId,
        collateral_asset: AssetId,
        seed_liquidity: Balance,
    ) -> DispatchResult;

    fn on_order_revealed(
        market_id: MarketId,
        trader: &AccountId,
        collateral_asset: AssetId,
        order_payload: Vec<u8>,
        order_value: Balance,
    ) -> DispatchResult;
}

impl<AccountId, AssetId, Balance> OrderbookBridge<AccountId, AssetId, Balance> for () {
    fn on_market_created(
        _market_id: MarketId,
        _creator: &AccountId,
        _collateral_asset: AssetId,
        _seed_liquidity: Balance,
    ) -> DispatchResult {
        Ok(())
    }

    fn on_order_revealed(
        _market_id: MarketId,
        _trader: &AccountId,
        _collateral_asset: AssetId,
        _order_payload: Vec<u8>,
        _order_value: Balance,
    ) -> DispatchResult {
        Ok(())
    }
}

/// Converts arbitrary collateral assets into the canonical stable used by the pallet.
pub trait CollateralRouter<AccountId, AssetId, Balance> {
    fn quote_to_canonical(
        asset: AssetId,
        canonical_amount: Balance,
    ) -> Result<Balance, DispatchError>;
    fn to_canonical(
        who: &AccountId,
        asset: AssetId,
        amount: Balance,
        dest: &AccountId,
    ) -> Result<Balance, DispatchError>;
}

impl<AccountId, AssetId, Balance> CollateralRouter<AccountId, AssetId, Balance> for () {
    fn quote_to_canonical(
        _asset: AssetId,
        _canonical_amount: Balance,
    ) -> Result<Balance, DispatchError> {
        Err(sp_runtime::DispatchError::Other(
            "collateral-router-disabled",
        ))
    }

    fn to_canonical(
        _who: &AccountId,
        _asset: AssetId,
        _amount: Balance,
        _dest: &AccountId,
    ) -> Result<Balance, DispatchError> {
        Err(sp_runtime::DispatchError::Other(
            "collateral-router-disabled",
        ))
    }
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
    Encode, Decode, TypeInfo, Clone, Copy, PartialEq, Eq, RuntimeDebug, Default, MaxEncodedLen,
)]
pub enum RelayNetwork {
    #[default]
    Polkadot,
    Kusama,
}

#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen)]
pub struct OpengovProposalMetadata<Tag> {
    pub network: RelayNetwork,
    pub parachain_id: u32,
    pub track_id: u16,
    pub referendum_index: u32,
    pub plaza_tag: Tag,
}

#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, Default)]
pub struct OpengovProposalInput {
    pub network: RelayNetwork,
    pub parachain_id: u32,
    pub track_id: u16,
    pub referendum_index: u32,
    pub plaza_tag: Vec<u8>,
}

#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen)]
pub struct ConditionMetadata<BoundedString, BlockNumber> {
    pub question: BoundedString,
    pub oracle: BoundedString,
    pub resolution_source: BoundedString,
    pub submission_deadline: BlockNumber,
}

#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, Default)]
pub struct ConditionInput<BlockNumber> {
    pub question: Vec<u8>,
    pub oracle: Vec<u8>,
    pub resolution_source: Vec<u8>,
    pub submission_deadline: BlockNumber,
}

pub struct OrderbookEventEmitter<T>(PhantomData<T>);

impl<T: Config> OrderbookBridge<T::AccountId, T::AssetId, T::Balance> for OrderbookEventEmitter<T> {
    fn on_market_created(
        market_id: MarketId,
        _creator: &T::AccountId,
        collateral_asset: T::AssetId,
        _seed_liquidity: T::Balance,
    ) -> DispatchResult {
        Pallet::<T>::deposit_event(Event::OrderbookMarketRegistered {
            market_id,
            collateral_asset,
        });
        Ok(())
    }

    fn on_order_revealed(
        market_id: MarketId,
        trader: &T::AccountId,
        _collateral_asset: T::AssetId,
        _order_payload: Vec<u8>,
        order_value: T::Balance,
    ) -> DispatchResult {
        Pallet::<T>::deposit_event(Event::OrderbookOrderPlaced {
            market_id,
            trader: trader.clone(),
            order_value,
        });
        Ok(())
    }
}

#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen)]
pub enum MarketStatus {
    Open,
    Locked,
    Resolved,
    Cancelled,
}

#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug, MaxEncodedLen)]
pub struct Market<ClassId, AccountId, BlockNumber, Balance> {
    pub creator: AccountId,
    pub condition_id: ConditionId,
    pub close_block: BlockNumber,
    pub collateral_asset: ClassId,
    pub seed_liquidity: Balance,
    pub status: MarketStatus,
}

#[derive(
    Encode, Decode, TypeInfo, Clone, PartialEq, Eq, sp_runtime::RuntimeDebug, MaxEncodedLen,
)]
pub struct OrderCommitment<BlockNumber> {
    pub committed_at: BlockNumber,
    pub expires_at: BlockNumber,
}

pub type MetadataString<T> = BoundedVec<u8, <T as pallet::Config>::MaxMetadataLength>;
pub type ConditionMetadataOf<T> = ConditionMetadata<MetadataString<T>, BlockNumberFor<T>>;

pub type PlazaTagOf<T> = BoundedVec<u8, <T as pallet::Config>::MaxPlazaTagLength>;
pub type OpengovProposalOf<T> = OpengovProposalMetadata<PlazaTagOf<T>>;

pub type MarketOf<T> = Market<
    <T as Config>::AssetId,
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::Balance,
>;

pub type StoredCommitmentOf<T> =
    StoredCommitment<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>;

pub trait AssetTransfer<AccountId, AssetId, Balance> {
    fn transfer(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        amount: Balance,
    ) -> DispatchResult;
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
        traits::{EnsureOrigin, Get},
    };
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Canonical censorship-resistant stablecoin used as collateral (KUSD by default).
        type CanonicalStableAssetId: Get<Self::AssetId>;

        /// Asset handler used for collateral transfers.
        type Assets: AssetTransfer<Self::AccountId, Self::AssetId, Self::Balance>;

        type AssetId: Parameter + Copy + Ord + MaxEncodedLen + TypeInfo;

        type Balance: Parameter
            + AtLeast32BitUnsigned
            + MaybeSerializeDeserialize
            + Default
            + Copy
            + MaxEncodedLen
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

        /// Orderbook integration layer (Polkaswap on-chain orderbook).
        type OrderbookIntegration: OrderbookBridge<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Router used to swap arbitrary assets into the canonical stable.
        type CollateralRouter: CollateralRouter<Self::AccountId, Self::AssetId, Self::Balance>;

        /// Worst-case weight cost for a single collateral routing call.
        type CollateralRouterWeight: Get<Weight>;

        /// Minimum number of blocks between market creation and close block.
        #[pallet::constant]
        type MinMarketDuration: Get<BlockNumberFor<Self>>;

        /// Blocks that must elapse between commitment and reveal to mitigate front-running.
        #[pallet::constant]
        type CommitmentRevealDelay: Get<BlockNumberFor<Self>>;

        /// Maximum blocks before a commitment expires unused.
        #[pallet::constant]
        type CommitmentExpiry: Get<BlockNumberFor<Self>>;

        /// Maximum metadata length (question/oracle/source).
        #[pallet::constant]
        type MaxMetadataLength: Get<u32>;
        /// Maximum length for Polkadot Plaza tags associated with OpenGov proposals.
        #[pallet::constant]
        type MaxPlazaTagLength: Get<u32>;
        /// Weight information for extrinsics.
        type WeightInfo: crate::WeightInfo;

        /// Open-interest threshold activating creator rewards.
        #[pallet::constant]
        type OpenInterestThreshold: Get<Self::Balance>;

        /// Creator reward basis points applied once threshold is reached.
        #[pallet::constant]
        type CreatorRewardBps: Get<u32>;

        /// Fork tax account receiving 0.1% usage royalties.
        #[pallet::constant]
        type ForkTaxAccount: Get<Self::AccountId>;

        /// Asset id for bridged USDC.
        #[pallet::constant]
        type UsdcAssetId: Get<Self::AssetId>;

        /// Asset id for bridged USDT.
        #[pallet::constant]
        type UsdtAssetId: Get<Self::AssetId>;

        /// Asset id for the HydraDX Hollar stablecoin (auto-swapped into canonical KUSD).
        #[pallet::constant]
        type HollarAssetId: Get<Self::AssetId>;

        /// Account holding the maintenance liquidity pool reserves.
        #[pallet::constant]
        type MaintenancePoolAccount: Get<Self::AccountId>;

        /// Portion of each creation fee routed into the maintenance pool (basis points).
        #[pallet::constant]
        type MaintenanceFeeBps: Get<u32>;

        /// Minimum bond required to join the governance whitelist (canonical stable units).
        #[pallet::constant]
        type GovernanceBondMinimum: Get<Self::Balance>;

        /// Governance bonding safety floor expressed in basis points (e.g., 8500 == 85%).
        #[pallet::constant]
        type LiquiditySafetyBps: Get<u32>;

        /// Origin allowed to administer the governance whitelist and emergency tooling.
        type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Daily bridge cap per user expressed in canonical units.
        #[pallet::constant]
        type BridgeDailyCap: Get<Self::Balance>;

        /// Number of blocks comprising a day for bridge accounting.
        #[pallet::constant]
        type BlocksPerDay: Get<BlockNumberFor<Self>>;

        /// Cooldown between wallet updates.
        #[pallet::constant]
        type WalletCooldown: Get<BlockNumberFor<Self>>;

        /// Payout tax basis points for bridge withdrawals.
        #[pallet::constant]
        type PayoutTaxBps: Get<u32>;

        /// Maximum lifetime for submitted credentials.
        #[pallet::constant]
        type CredentialTtl: Get<BlockNumberFor<Self>>;

        /// Whether credentials are enforced by default (governance can override).
        #[pallet::constant]
        type CredentialsRequired: Get<bool>;

        /// Hook for notifying third-party integrations (e.g., Polkadot Plaza).
        type PlazaIntegration: PlazaIntegrationHook<OpengovProposalOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
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
    #[pallet::getter(fn market_collateral)]
    pub type MarketCollateral<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn market_open_interest)]
    pub type MarketOpenInterest<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn creator_reward_active)]
    pub type CreatorRewardActivated<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn creator_rewards)]
    pub type CreatorRewards<T: Config> =
        StorageMap<_, Blake2_128Concat, MarketId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn fork_tax_owed)]
    pub type ForkTaxOwed<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn bridge_wallet)]
    pub type BridgeWallet<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn bridge_wallet_updated)]
    pub type BridgeWalletUpdated<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BlockNumberFor<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn daily_bridge_amount)]
    pub type DailyBridgeAmount<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        u64,
        T::Balance,
        ValueQuery,
    >;

    #[derive(
        Encode, Decode, TypeInfo, Clone, PartialEq, Eq, sp_runtime::RuntimeDebug, MaxEncodedLen,
    )]
    pub struct StoredCommitment<AccountId, BlockNumber> {
        pub owner: AccountId,
        pub info: OrderCommitment<BlockNumber>,
    }

    #[pallet::storage]
    #[pallet::getter(fn commitments)]
    pub type Commitments<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MarketId,
        Blake2_128Concat,
        CommitmentHash,
        StoredCommitmentOf<T>,
        OptionQuery,
    >;

    #[pallet::type_value]
    pub fn CredentialsEnforcedDefault<T: Config>() -> bool {
        Pallet::<T>::credentials_required_default()
    }

    #[pallet::storage]
    #[pallet::getter(fn credentials_enforced)]
    pub type CredentialsEnforced<T: Config> =
        StorageValue<_, bool, ValueQuery, CredentialsEnforcedDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn governance_bonds)]
    pub type GovernanceBonds<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn maintenance_pool_balance)]
    pub type MaintenancePoolBalance<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn maintenance_pool_total)]
    pub type MaintenancePoolTotal<T: Config> = StorageValue<_, T::Balance, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn is_flagged)]
    pub type FlaggedAccounts<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn credentials)]
    pub type Credentials<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        (BlockNumberFor<T>, [u8; 32], JurisdictionCode),
        OptionQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn is_jurisdiction_blocked)]
    pub type BlockedJurisdictions<T> =
        StorageMap<_, Blake2_128Concat, JurisdictionCode, (), OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn opengov_proposals)]
    pub type OpengovConditions<T: Config> =
        StorageMap<_, Blake2_128Concat, ConditionId, OpengovProposalOf<T>, OptionQuery>;

    #[pallet::storage]
    pub type FeeCollectorOverride<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type MaintenancePoolOverride<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type ForkTaxAccountOverride<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    #[pallet::storage]
    pub type GovernanceBondMinimumOverride<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

    #[pallet::storage]
    pub type MaintenanceFeeBpsOverride<T> = StorageValue<_, u32, OptionQuery>;

    #[pallet::storage]
    pub type LiquiditySafetyBpsOverride<T> = StorageValue<_, u32, OptionQuery>;

    #[pallet::storage]
    pub type BridgeDailyCapOverride<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

    #[pallet::storage]
    pub type BlocksPerDayOverride<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    #[pallet::storage]
    pub type WalletCooldownOverride<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    #[pallet::storage]
    pub type PayoutTaxBpsOverride<T> = StorageValue<_, u32, OptionQuery>;

    #[pallet::storage]
    pub type CredentialTtlOverride<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

    #[pallet::storage]
    pub type CredentialsRequiredOverride<T> = StorageValue<_, bool, OptionQuery>;

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub fee_collector: Option<T::AccountId>,
        pub maintenance_pool_account: Option<T::AccountId>,
        pub fork_tax_account: Option<T::AccountId>,
        pub governance_bond_minimum: Option<T::Balance>,
        pub maintenance_fee_bps: Option<u32>,
        pub liquidity_safety_bps: Option<u32>,
        pub bridge_daily_cap: Option<T::Balance>,
        pub blocks_per_day: Option<BlockNumberFor<T>>,
        pub wallet_cooldown: Option<BlockNumberFor<T>>,
        pub payout_tax_bps: Option<u32>,
        pub credential_ttl: Option<BlockNumberFor<T>>,
        pub credentials_required: Option<bool>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                fee_collector: None,
                maintenance_pool_account: None,
                fork_tax_account: None,
                governance_bond_minimum: None,
                maintenance_fee_bps: None,
                liquidity_safety_bps: None,
                bridge_daily_cap: None,
                blocks_per_day: None,
                wallet_cooldown: None,
                payout_tax_bps: None,
                credential_ttl: None,
                credentials_required: None,
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            if let Some(ref account) = self.fee_collector {
                FeeCollectorOverride::<T>::put(account.clone());
            }
            if let Some(ref account) = self.maintenance_pool_account {
                MaintenancePoolOverride::<T>::put(account.clone());
            }
            if let Some(ref account) = self.fork_tax_account {
                ForkTaxAccountOverride::<T>::put(account.clone());
            }
            if let Some(value) = self.governance_bond_minimum {
                GovernanceBondMinimumOverride::<T>::put(value);
            }
            if let Some(bps) = self.maintenance_fee_bps {
                MaintenanceFeeBpsOverride::<T>::put(bps);
            }
            if let Some(bps) = self.liquidity_safety_bps {
                LiquiditySafetyBpsOverride::<T>::put(bps);
            }
            if let Some(value) = self.bridge_daily_cap {
                BridgeDailyCapOverride::<T>::put(value);
            }
            if let Some(value) = self.blocks_per_day {
                BlocksPerDayOverride::<T>::put(value);
            }
            if let Some(value) = self.wallet_cooldown {
                WalletCooldownOverride::<T>::put(value);
            }
            if let Some(bps) = self.payout_tax_bps {
                PayoutTaxBpsOverride::<T>::put(bps);
            }
            if let Some(value) = self.credential_ttl {
                CredentialTtlOverride::<T>::put(value);
            }
            if let Some(required) = self.credentials_required {
                CredentialsRequiredOverride::<T>::put(required);
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
        OrderCommitted {
            market_id: MarketId,
            trader: T::AccountId,
            commitment: CommitmentHash,
        },
        OrderRevealed {
            market_id: MarketId,
            trader: T::AccountId,
        },
        CreatorRewardActivated {
            market_id: MarketId,
        },
        CreatorRewardAccrued {
            market_id: MarketId,
            amount: T::Balance,
        },
        OrderbookMarketRegistered {
            market_id: MarketId,
            collateral_asset: T::AssetId,
        },
        OrderbookOrderPlaced {
            market_id: MarketId,
            trader: T::AccountId,
            order_value: T::Balance,
        },
        ForkTaxAccrued {
            amount: T::Balance,
        },
        BridgeWalletUpdated {
            user: T::AccountId,
            wallet: T::AccountId,
        },
        BridgeDeposited {
            user: T::AccountId,
            asset: T::AssetId,
            amount: T::Balance,
            day: u64,
        },
        BridgeWithdrawalRequested {
            user: T::AccountId,
            wallet: T::AccountId,
            amount: T::Balance,
            tax: T::Balance,
        },
        GovernanceBonded {
            who: T::AccountId,
            amount: T::Balance,
        },
        GovernanceUnbonded {
            who: T::AccountId,
            amount: T::Balance,
        },
        GovernanceSlashed {
            who: T::AccountId,
            amount: T::Balance,
        },
        AccountFlagged {
            who: T::AccountId,
        },
        AccountCleared {
            who: T::AccountId,
        },
        FlaggedAccountDrained {
            who: T::AccountId,
            amount: T::Balance,
        },
        MaintenancePoolFunded {
            amount: T::Balance,
        },
        MaintenancePoolWithdrawn {
            amount: T::Balance,
            destination: T::AccountId,
        },
        HollarRouted {
            user: T::AccountId,
            amount: T::Balance,
        },
        CredentialSubmitted {
            who: T::AccountId,
            expires_at: BlockNumberFor<T>,
            jurisdiction: JurisdictionCode,
        },
        JurisdictionStatusUpdated {
            code: JurisdictionCode,
            blocked: bool,
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
        CredentialsRequirementUpdated {
            enabled: bool,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        QuestionTooShort,
        ConditionNotFound,
        InvalidCollateralAsset,
        Overflow,
        MarketDurationTooShort,
        MarketNotOpen,
        CommitmentExists,
        CommitmentUnknown,
        RevealTooSoon,
        CommitmentExpired,
        EmptyOrderPayload,
        MetadataTooLong,
        BridgeAssetNotAllowed,
        BridgeDailyLimitReached,
        BridgeWalletLocked,
        BridgeWalletMissing,
        UnsupportedCollateralAsset,
        InsufficientCreationFee,
        GovernanceBondTooLow,
        AccountNotBonded,
        AccountFlagged,
        PoolBelowSafetyThreshold,
        CredentialMissing,
        JurisdictionBlocked,
        InvalidOpengovProposal,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register a new prediction condition/oracle.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::create_condition())]
        pub fn create_condition(
            origin: OriginFor<T>,
            metadata: ConditionInput<BlockNumberFor<T>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_authorized_creator(&who)?;
            Self::create_condition_entry(&who, metadata)?;
            Ok(())
        }

        /// Register an OpenGov-linked condition compatible with Polkadot Plaza feeds.
        #[pallet::call_index(16)]
        #[pallet::weight(T::WeightInfo::create_condition())]
        pub fn create_opengov_condition(
            origin: OriginFor<T>,
            metadata: ConditionInput<BlockNumberFor<T>>,
            proposal: OpengovProposalInput,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_authorized_creator(&who)?;
            ensure!(
                proposal.parachain_id > 0 && proposal.track_id > 0 && proposal.referendum_index > 0,
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

        /// Remove stored OpenGov metadata (e.g., after market closure).
        #[pallet::call_index(17)]
        #[pallet::weight(Weight::from_parts(30_000, 0))]
        pub fn clear_opengov_condition(
            origin: OriginFor<T>,
            condition_id: ConditionId,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            OpengovConditions::<T>::remove(condition_id);
            Self::deposit_event(Event::OpengovConditionCleared { condition_id });
            Ok(())
        }

        /// Toggle credential enforcement (SORA governance control).
        #[pallet::call_index(18)]
        #[pallet::weight(Weight::from_parts(30_000, 0))]
        pub fn set_credentials_required(origin: OriginFor<T>, enabled: bool) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            CredentialsEnforced::<T>::put(enabled);
            Self::deposit_event(Event::CredentialsRequirementUpdated { enabled });
            Ok(())
        }

        /// Create a market for a registered condition and seed it with canonical stable collateral.
        #[pallet::call_index(1)]
        #[pallet::weight({
            let amount = *seed_liquidity;
            let fee = *fee_asset;
            Pallet::<T>::create_market_weight(amount, fee)
        })]
        pub fn create_market(
            origin: OriginFor<T>,
            condition_id: ConditionId,
            close_block: BlockNumberFor<T>,
            seed_liquidity: T::Balance,
            fee_asset: Option<T::AssetId>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_authorized_creator(&who)?;
            ensure!(
                Conditions::<T>::contains_key(condition_id),
                Error::<T>::ConditionNotFound
            );
            let now = <frame_system::Pallet<T>>::block_number();
            let min_close = now
                .checked_add(&T::MinMarketDuration::get())
                .ok_or(Error::<T>::Overflow)?;
            ensure!(close_block >= min_close, Error::<T>::MarketDurationTooShort);

            Self::withdraw_creation_fee(&who, seed_liquidity, fee_asset)?;

            let market_id =
                NextMarketId::<T>::try_mutate(|next_id| -> Result<MarketId, DispatchError> {
                    let id = *next_id;
                    *next_id = next_id
                        .checked_add(One::one())
                        .ok_or(Error::<T>::Overflow)?;
                    Ok(id)
                })?;

            let deposited =
                Self::escrow_seed_liquidity(&who, market_id, seed_liquidity, fee_asset)?;
            let data = Market {
                creator: who.clone(),
                condition_id,
                close_block,
                collateral_asset: T::CanonicalStableAssetId::get(),
                seed_liquidity: deposited,
                status: MarketStatus::Open,
            };
            Markets::<T>::insert(market_id, data);
            T::OrderbookIntegration::on_market_created(
                market_id,
                &who,
                T::CanonicalStableAssetId::get(),
                deposited,
            )?;
            Self::deposit_event(Event::MarketCreated {
                market_id,
                seed_liquidity: deposited,
            });
            Ok(())
        }

        /// Commit to an order off-chain to mitigate front-running. Order details remain hidden.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::commit_order())]
        pub fn commit_order(
            origin: OriginFor<T>,
            market_id: MarketId,
            commitment: CommitmentHash,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            let _market = Self::ensure_market_open(market_id)?;
            ensure!(
                !Commitments::<T>::contains_key(market_id, commitment),
                Error::<T>::CommitmentExists
            );
            let now = <frame_system::Pallet<T>>::block_number();
            let expires_at = now
                .checked_add(&T::CommitmentExpiry::get())
                .ok_or(Error::<T>::Overflow)?;
            let record = StoredCommitment {
                owner: who.clone(),
                info: OrderCommitment {
                    committed_at: now,
                    expires_at,
                },
            };
            Commitments::<T>::insert(market_id, commitment, record);
            Self::deposit_event(Event::OrderCommitted {
                market_id,
                trader: who,
                commitment,
            });
            Ok(())
        }

        /// Reveal the committed order after the required delay; forwards payload to the orderbook.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::reveal_order())]
        pub fn reveal_order(
            origin: OriginFor<T>,
            market_id: MarketId,
            order_payload: Vec<u8>,
            salt: Vec<u8>,
            order_value: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            ensure!(!order_payload.is_empty(), Error::<T>::EmptyOrderPayload);
            let market = Self::ensure_market_open(market_id)?;

            let hash = Self::compute_commitment_hash(&who, market_id, &order_payload, &salt);
            let stored =
                Commitments::<T>::get(market_id, hash).ok_or(Error::<T>::CommitmentUnknown)?;
            ensure!(who == stored.owner, Error::<T>::CommitmentUnknown);

            let now = <frame_system::Pallet<T>>::block_number();
            let min_reveal = stored
                .info
                .committed_at
                .checked_add(&T::CommitmentRevealDelay::get())
                .ok_or(Error::<T>::Overflow)?;
            ensure!(now >= min_reveal, Error::<T>::RevealTooSoon);
            ensure!(now <= stored.info.expires_at, Error::<T>::CommitmentExpired);

            Commitments::<T>::remove(market_id, hash);

            T::OrderbookIntegration::on_order_revealed(
                market_id,
                &who,
                market.collateral_asset,
                order_payload.clone(),
                order_value,
            )?;

            Self::record_open_interest(market_id, order_value);

            Self::deposit_event(Event::OrderRevealed {
                market_id,
                trader: who,
            });
            Ok(())
        }

        /// Set or update the destination bridge wallet with cooldown.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::set_bridge_wallet())]
        pub fn set_bridge_wallet(origin: OriginFor<T>, wallet: T::AccountId) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let last = BridgeWalletUpdated::<T>::get(&who);
            if last != BlockNumberFor::<T>::zero() {
                let cooldown = Self::wallet_cooldown();
                ensure!(
                    now >= last.saturating_add(cooldown),
                    Error::<T>::BridgeWalletLocked
                );
            }
            BridgeWallet::<T>::insert(&who, &wallet);
            BridgeWalletUpdated::<T>::insert(&who, now);
            Self::deposit_event(Event::BridgeWalletUpdated { user: who, wallet });
            Ok(())
        }

        /// Register a bridged stablecoin deposit subject to daily cap.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::bridge_deposit())]
        pub fn bridge_deposit(
            origin: OriginFor<T>,
            asset: T::AssetId,
            amount: T::Balance,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            ensure!(
                Self::is_bridge_asset(&asset),
                Error::<T>::BridgeAssetNotAllowed
            );
            let day = Self::apply_daily_bridge_cap(&who, amount)?;
            Self::deposit_event(Event::BridgeDeposited {
                user: who,
                asset,
                amount,
                day,
            });
            Ok(())
        }

        /// Bridge tokens out to the registered wallet, applying payout tax.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::bridge_withdraw())]
        pub fn bridge_withdraw(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            Self::ensure_has_credential(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            let wallet = BridgeWallet::<T>::get(&who).ok_or(Error::<T>::BridgeWalletMissing)?;
            let tax = Perbill::from_rational(Self::payout_tax_bps(), 10_000u32) * amount;
            if !tax.is_zero() {
                ForkTaxOwed::<T>::mutate(|total| {
                    *total = total.saturating_add(tax);
                });
                Self::deposit_event(Event::ForkTaxAccrued { amount: tax });
            }
            Self::deposit_event(Event::BridgeWithdrawalRequested {
                user: who,
                wallet,
                amount,
                tax,
            });
            Ok(())
        }

        /// Bond canonical stable to join the governance whitelist and maintenance pool.
        #[pallet::call_index(7)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn bond_governance(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            Self::ensure_has_credential(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            ensure!(
                amount >= Self::governance_bond_minimum(),
                Error::<T>::GovernanceBondTooLow
            );
            let received = Self::deposit_canonical(
                &who,
                T::CanonicalStableAssetId::get(),
                &Self::maintenance_pool_account(),
                amount,
            )?;
            GovernanceBonds::<T>::mutate(&who, |bond| {
                *bond = bond.saturating_add(received);
            });
            Self::credit_pool(received);
            Self::deposit_event(Event::GovernanceBonded {
                who,
                amount: received,
            });
            Ok(())
        }

        /// Withdraw bonded governance stake (subject to safety threshold).
        #[pallet::call_index(8)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn unbond_governance(origin: OriginFor<T>, amount: T::Balance) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            Self::ensure_has_credential(&who)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            GovernanceBonds::<T>::try_mutate(&who, |bond| -> DispatchResult {
                ensure!(*bond >= amount, Error::<T>::AccountNotBonded);
                *bond = bond.saturating_sub(amount);
                Ok(())
            })?;
            Self::debit_pool(amount)?;
            T::Assets::transfer(
                T::CanonicalStableAssetId::get(),
                &Self::maintenance_pool_account(),
                &who,
                amount,
            )?;
            Self::deposit_event(Event::GovernanceUnbonded { who, amount });
            Ok(())
        }

        /// Slash a bonded governance participant for misbehaviour.
        #[pallet::call_index(9)]
        #[pallet::weight(Weight::from_parts(40_000, 0))]
        pub fn slash_governance(
            origin: OriginFor<T>,
            target: T::AccountId,
            amount: T::Balance,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            GovernanceBonds::<T>::try_mutate(&target, |bond| -> DispatchResult {
                ensure!(*bond >= amount, Error::<T>::AccountNotBonded);
                *bond = bond.saturating_sub(amount);
                Ok(())
            })?;
            Self::deposit_event(Event::GovernanceSlashed {
                who: target,
                amount,
            });
            Ok(())
        }

        /// Flag an account for fraudulent activity, preventing participation.
        #[pallet::call_index(10)]
        #[pallet::weight(Weight::from_parts(30_000, 0))]
        pub fn flag_account(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            FlaggedAccounts::<T>::insert(&target, ());
            Self::deposit_event(Event::AccountFlagged { who: target });
            Ok(())
        }

        /// Remove a flag once an investigation is resolved.
        #[pallet::call_index(11)]
        #[pallet::weight(Weight::from_parts(30_000, 0))]
        pub fn clear_flag(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            FlaggedAccounts::<T>::remove(&target);
            Self::deposit_event(Event::AccountCleared { who: target });
            Ok(())
        }

        /// Drain assets from a flagged account into the maintenance pool.
        #[pallet::call_index(12)]
        #[pallet::weight(Weight::from_parts(60_000, 0))]
        pub fn drain_flagged_account(
            origin: OriginFor<T>,
            target: T::AccountId,
            amount: T::Balance,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(
                FlaggedAccounts::<T>::contains_key(&target),
                Error::<T>::AccountFlagged
            );
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            T::Assets::transfer(
                T::CanonicalStableAssetId::get(),
                &target,
                &Self::maintenance_pool_account(),
                amount,
            )?;
            Self::credit_pool(amount);
            Self::deposit_event(Event::FlaggedAccountDrained {
                who: target,
                amount,
            });
            Ok(())
        }

        /// Withdraw funds from the maintenance pool while respecting the safety floor.
        #[pallet::call_index(13)]
        #[pallet::weight(Weight::from_parts(50_000, 0))]
        pub fn withdraw_maintenance_pool(
            origin: OriginFor<T>,
            destination: T::AccountId,
            amount: T::Balance,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            ensure!(!amount.is_zero(), Error::<T>::InvalidCollateralAsset);
            Self::debit_pool(amount)?;
            T::Assets::transfer(
                T::CanonicalStableAssetId::get(),
                &Self::maintenance_pool_account(),
                &destination,
                amount,
            )?;
            Self::deposit_event(Event::MaintenancePoolWithdrawn {
                amount,
                destination,
            });
            Ok(())
        }

        /// Submit or refresh a zk-credential hash.
        #[pallet::call_index(14)]
        #[pallet::weight(Weight::from_parts(30_000, 0))]
        pub fn submit_credential(
            origin: OriginFor<T>,
            credential_hash: [u8; 32],
            expires_at: BlockNumberFor<T>,
            jurisdiction: JurisdictionCode,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::ensure_account_is_clear(&who)?;
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(expires_at >= now, Error::<T>::CredentialMissing);
            Self::ensure_jurisdiction_allowed(&jurisdiction)?;
            let max_expiry = now
                .checked_add(&Self::credential_ttl())
                .ok_or(Error::<T>::Overflow)?;
            let bounded = core::cmp::min(expires_at, max_expiry);
            Credentials::<T>::insert(&who, (bounded, credential_hash, jurisdiction));
            Self::deposit_event(Event::CredentialSubmitted {
                who,
                expires_at: bounded,
                jurisdiction,
            });
            Ok(())
        }

        /// Update the blocked status for a jurisdiction code (ISO alpha-3).
        #[pallet::call_index(15)]
        #[pallet::weight(Weight::from_parts(30_000, 0))]
        pub fn set_jurisdiction_block(
            origin: OriginFor<T>,
            jurisdiction: JurisdictionCode,
            blocked: bool,
        ) -> DispatchResult {
            T::GovernanceOrigin::ensure_origin(origin)?;
            if blocked {
                BlockedJurisdictions::<T>::insert(jurisdiction, ());
            } else {
                BlockedJurisdictions::<T>::remove(jurisdiction);
            }
            Self::deposit_event(Event::JurisdictionStatusUpdated {
                code: jurisdiction,
                blocked,
            });
            Ok(())
        }
    }

    impl<T: Config> Pallet<T> {
        fn fee_collector_account() -> T::AccountId {
            FeeCollectorOverride::<T>::get().unwrap_or_else(T::FeeCollector::get)
        }

        fn maintenance_pool_account() -> T::AccountId {
            MaintenancePoolOverride::<T>::get().unwrap_or_else(T::MaintenancePoolAccount::get)
        }

        #[allow(dead_code)]
        fn fork_tax_account() -> T::AccountId {
            ForkTaxAccountOverride::<T>::get().unwrap_or_else(T::ForkTaxAccount::get)
        }

        fn governance_bond_minimum() -> T::Balance {
            GovernanceBondMinimumOverride::<T>::get().unwrap_or_else(T::GovernanceBondMinimum::get)
        }

        fn maintenance_fee_bps() -> u32 {
            MaintenanceFeeBpsOverride::<T>::get().unwrap_or_else(T::MaintenanceFeeBps::get)
        }

        fn liquidity_safety_bps() -> u32 {
            LiquiditySafetyBpsOverride::<T>::get().unwrap_or_else(T::LiquiditySafetyBps::get)
        }

        fn bridge_daily_cap() -> T::Balance {
            BridgeDailyCapOverride::<T>::get().unwrap_or_else(T::BridgeDailyCap::get)
        }

        fn blocks_per_day() -> BlockNumberFor<T> {
            BlocksPerDayOverride::<T>::get().unwrap_or_else(T::BlocksPerDay::get)
        }

        fn wallet_cooldown() -> BlockNumberFor<T> {
            WalletCooldownOverride::<T>::get().unwrap_or_else(T::WalletCooldown::get)
        }

        fn payout_tax_bps() -> u32 {
            PayoutTaxBpsOverride::<T>::get().unwrap_or_else(T::PayoutTaxBps::get)
        }

        fn credential_ttl() -> BlockNumberFor<T> {
            CredentialTtlOverride::<T>::get().unwrap_or_else(T::CredentialTtl::get)
        }

        fn credentials_required_default() -> bool {
            CredentialsRequiredOverride::<T>::get().unwrap_or_else(T::CredentialsRequired::get)
        }

        fn routed_transfers(amount: T::Balance, fee_asset: &Option<T::AssetId>) -> u32 {
            if amount.is_zero() {
                return 0;
            }
            let canonical = T::CanonicalStableAssetId::get();
            match fee_asset {
                Some(asset) if *asset != canonical => 4,
                _ => 0,
            }
        }

        fn create_market_weight(amount: T::Balance, fee_asset: Option<T::AssetId>) -> Weight {
            let routed = Self::routed_transfers(amount, &fee_asset);
            T::WeightInfo::create_market(routed)
        }

        fn withdraw_creation_fee(
            who: &T::AccountId,
            seed: T::Balance,
            fee_asset: Option<T::AssetId>,
        ) -> DispatchResult {
            if seed.is_zero() {
                return Ok(());
            }

            let bps = T::CreationFeeBps::get();
            let ratio = Perbill::from_rational(bps, 10_000u32);
            let fee_from_bps = ratio * seed;
            let min_fee = T::MinCreationFee::get();
            let fee = if fee_from_bps < min_fee {
                min_fee
            } else {
                fee_from_bps
            };

            let asset = fee_asset.unwrap_or(T::CanonicalStableAssetId::get());
            let collector = Self::fee_collector_account();
            let deposited = Self::deposit_canonical(who, asset, &collector, fee)?;
            ensure!(deposited >= fee, Error::<T>::InsufficientCreationFee);

            let maintenance_ratio = Perbill::from_rational(Self::maintenance_fee_bps(), 10_000u32);
            let maintenance_amount = maintenance_ratio * fee;
            if !maintenance_amount.is_zero() {
                T::Assets::transfer(
                    T::CanonicalStableAssetId::get(),
                    &collector,
                    &Self::maintenance_pool_account(),
                    maintenance_amount,
                )?;
                Self::credit_pool(maintenance_amount);
            }
            Ok(())
        }

        fn escrow_seed_liquidity(
            who: &T::AccountId,
            market_id: MarketId,
            amount: T::Balance,
            collateral_asset: Option<T::AssetId>,
        ) -> Result<T::Balance, DispatchError> {
            if amount.is_zero() {
                MarketCollateral::<T>::insert(market_id, amount);
                return Ok(amount);
            }
            let canonical = T::CanonicalStableAssetId::get();
            let asset = collateral_asset.unwrap_or(canonical);
            let input_amount = if asset == canonical {
                amount
            } else {
                T::CollateralRouter::quote_to_canonical(asset, amount)?
            };
            let deposited = Self::deposit_canonical(who, asset, &Self::account_id(), input_amount)?;

            MarketCollateral::<T>::insert(market_id, deposited);
            Self::record_open_interest(market_id, deposited);

            Self::deposit_event(Event::CollateralSeeded {
                market_id,
                amount: deposited,
            });
            Ok(deposited)
        }

        pub(crate) fn account_id() -> T::AccountId {
            T::PalletId::get().into_account_truncating()
        }

        fn deposit_canonical(
            who: &T::AccountId,
            asset: T::AssetId,
            dest: &T::AccountId,
            amount: T::Balance,
        ) -> Result<T::Balance, DispatchError> {
            if amount.is_zero() {
                return Ok(amount);
            }
            let canonical = T::CanonicalStableAssetId::get();
            if asset == canonical {
                T::Assets::transfer(canonical, who, dest, amount)?;
                Ok(amount)
            } else if asset == T::HollarAssetId::get() {
                T::CollateralRouter::to_canonical(who, asset, amount, dest)
                    .map_err(|_| Error::<T>::UnsupportedCollateralAsset.into())
                    .map(|received| {
                        Self::deposit_event(Event::HollarRouted {
                            user: who.clone(),
                            amount: received,
                        });
                        received
                    })
            } else {
                T::CollateralRouter::to_canonical(who, asset, amount, dest)
                    .map_err(|_| Error::<T>::UnsupportedCollateralAsset.into())
            }
        }

        fn ensure_market_open(market_id: MarketId) -> Result<MarketOf<T>, DispatchError> {
            let market = Markets::<T>::get(market_id).ok_or(Error::<T>::MarketNotOpen)?;
            let now = <frame_system::Pallet<T>>::block_number();
            ensure!(
                matches!(market.status, MarketStatus::Open) && now < market.close_block,
                Error::<T>::MarketNotOpen
            );
            Ok(market)
        }

        pub(crate) fn compute_commitment_hash(
            who: &T::AccountId,
            market_id: MarketId,
            payload: &[u8],
            salt: &[u8],
        ) -> CommitmentHash {
            let mut data = who.encode();
            data.extend_from_slice(&market_id.encode());
            data.extend_from_slice(payload);
            data.extend_from_slice(salt);
            blake2_256(&data)
        }

        fn record_open_interest(market_id: MarketId, amount: T::Balance) {
            if amount.is_zero() {
                return;
            }
            MarketOpenInterest::<T>::mutate(market_id, |oi| {
                *oi = oi.saturating_add(amount);
            });
            let tax = Perbill::from_rational(10u32, 10_000u32) * amount;
            if !tax.is_zero() {
                ForkTaxOwed::<T>::mutate(|total| {
                    *total = total.saturating_add(tax);
                });
                Self::deposit_event(Event::ForkTaxAccrued { amount: tax });
            }

            let activated = CreatorRewardActivated::<T>::get(market_id);
            let threshold = T::OpenInterestThreshold::get();
            let mut now_activated = activated;
            if !activated && MarketOpenInterest::<T>::get(market_id) >= threshold {
                CreatorRewardActivated::<T>::insert(market_id, true);
                Self::deposit_event(Event::CreatorRewardActivated { market_id });
                now_activated = true;
            }

            if now_activated {
                let reward = Perbill::from_rational(T::CreatorRewardBps::get(), 10_000u32) * amount;
                if !reward.is_zero() {
                    CreatorRewards::<T>::mutate(market_id, |total| {
                        *total = total.saturating_add(reward);
                    });
                    Self::deposit_event(Event::CreatorRewardAccrued {
                        market_id,
                        amount: reward,
                    });
                }
            }
        }

        fn is_bridge_asset(asset: &T::AssetId) -> bool {
            *asset == T::UsdcAssetId::get() || *asset == T::UsdtAssetId::get()
        }

        fn current_day() -> u64 {
            let now = <frame_system::Pallet<T>>::block_number();
            let per_day = Self::blocks_per_day().max(BlockNumberFor::<T>::one());
            let now_u = now.saturated_into::<u128>();
            let per_day_u = per_day.saturated_into::<u128>().max(1);
            (now_u / per_day_u).min(u64::MAX as u128) as u64
        }

        fn apply_daily_bridge_cap(
            user: &T::AccountId,
            amount: T::Balance,
        ) -> Result<u64, DispatchError> {
            let day = Self::current_day();
            DailyBridgeAmount::<T>::try_mutate(user, day, |usage| -> DispatchResult {
                let new_total = usage.saturating_add(amount);
                ensure!(
                    new_total <= Self::bridge_daily_cap(),
                    Error::<T>::BridgeDailyLimitReached
                );
                *usage = new_total;
                Ok(())
            })?;
            Ok(day)
        }

        fn ensure_account_is_clear(who: &T::AccountId) -> DispatchResult {
            ensure!(
                !FlaggedAccounts::<T>::contains_key(who),
                Error::<T>::AccountFlagged
            );
            Ok(())
        }

        fn ensure_jurisdiction_allowed(code: &JurisdictionCode) -> DispatchResult {
            ensure!(*code != [0; 3], Error::<T>::JurisdictionBlocked);
            ensure!(
                !BlockedJurisdictions::<T>::contains_key(code),
                Error::<T>::JurisdictionBlocked
            );
            Ok(())
        }

        fn ensure_has_credential(who: &T::AccountId) -> DispatchResult {
            if !CredentialsEnforced::<T>::get() {
                return Ok(());
            }
            let now = <frame_system::Pallet<T>>::block_number();
            let Some((expires_at, _hash, jurisdiction)) = Credentials::<T>::get(who) else {
                return Err(Error::<T>::CredentialMissing.into());
            };
            ensure!(now <= expires_at, Error::<T>::CredentialMissing);
            Self::ensure_jurisdiction_allowed(&jurisdiction)?;
            Ok(())
        }

        fn credit_pool(amount: T::Balance) {
            if amount.is_zero() {
                return;
            }
            MaintenancePoolBalance::<T>::mutate(|bal| {
                *bal = bal.saturating_add(amount);
            });
            MaintenancePoolTotal::<T>::mutate(|total| {
                *total = total.saturating_add(amount);
            });
            Self::deposit_event(Event::MaintenancePoolFunded { amount });
        }

        fn debit_pool(amount: T::Balance) -> DispatchResult {
            if amount.is_zero() {
                return Ok(());
            }
            let total_before = MaintenancePoolTotal::<T>::get();
            let floor = Self::pool_floor_from_total(total_before);
            MaintenancePoolBalance::<T>::try_mutate(|bal| -> DispatchResult {
                ensure!(*bal >= amount, Error::<T>::PoolBelowSafetyThreshold);
                let remaining = bal.saturating_sub(amount);
                ensure!(remaining >= floor, Error::<T>::PoolBelowSafetyThreshold);
                *bal = remaining;
                Ok(())
            })?;
            MaintenancePoolTotal::<T>::mutate(|total| {
                *total = total.saturating_sub(amount);
            });
            Ok(())
        }

        fn pool_floor_from_total(total: T::Balance) -> T::Balance {
            if total.is_zero() {
                return total;
            }
            let floor_bps = Self::liquidity_safety_bps().min(10_000u32);
            let ratio = Perbill::from_rational(floor_bps, 10_000u32);
            ratio * total
        }

        #[cfg(feature = "runtime-benchmarks")]
        fn ensure_authorized_creator(_who: &T::AccountId) -> DispatchResult {
            Ok(())
        }

        #[cfg(not(feature = "runtime-benchmarks"))]
        fn ensure_authorized_creator(who: &T::AccountId) -> DispatchResult {
            Self::ensure_account_is_clear(who)?;
            Self::ensure_has_credential(who)?;
            ensure!(
                GovernanceBonds::<T>::get(who) >= Self::governance_bond_minimum(),
                Error::<T>::AccountNotBonded
            );
            Ok(())
        }

        fn create_condition_entry(
            _who: &T::AccountId,
            metadata: ConditionInput<BlockNumberFor<T>>,
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
                submission_deadline: metadata.submission_deadline,
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
    }
}

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;
