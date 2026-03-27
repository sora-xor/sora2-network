use crate::{
    self as pallet_polkamarkt, AssetTransfer, ConditionId, OpengovProposalOf, PlazaIntegrationHook,
};
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstBool, ConstU32, Everything},
    weights::Weight,
    PalletId,
};
use frame_system as system;
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    DispatchError,
};
use sp_std::{cell::RefCell, collections::btree_map::BTreeMap, vec::Vec};

pub type AccountId = u64;
pub type AssetId = u32;
pub type Balance = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const FEE_COLLECTOR: AccountId = 99;
pub const MAINTENANCE_ACCOUNT: AccountId = 55;
pub const CANONICAL_ASSET: AssetId = 0;
pub const HOLLAR_ASSET: AssetId = 2;
pub const FORK_TAX_ACCOUNT: AccountId = 77;
pub const USDC_ASSET: AssetId = 100;
pub const USDT_ASSET: AssetId = 101;

thread_local! {
    static ASSET_BALANCES: RefCell<BTreeMap<(AccountId, AssetId), Balance>> = RefCell::new(BTreeMap::new());
    static PLAZA_NOTIFIED: RefCell<Option<ConditionId>> = RefCell::new(None);
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const CanonicalStable: AssetId = CANONICAL_ASSET;
    pub const FeeCollectorAccount: AccountId = FEE_COLLECTOR;
    pub const MinQuestionLengthConst: u32 = 4;
    pub const CreationFeeBpsConst: u32 = 35;
    pub const MinCreationFeeConst: Balance = 10;
    pub const TestPalletId: PalletId = PalletId(*b"pk/mktpl");
    pub const MinMarketDurationConst: BlockNumber = 5;
    pub const CommitmentDelayConst: BlockNumber = 2;
    pub const CommitmentExpiryConst: BlockNumber = 10;
    pub const MaxMetadataLengthConst: u32 = 128;
    pub const OpenInterestThresholdConst: Balance = 10_000;
    pub const CreatorRewardBpsConst: u32 = 10;
    pub const ForkTaxAccountConst: AccountId = FORK_TAX_ACCOUNT;
    pub const UsdcAssetConst: AssetId = USDC_ASSET;
    pub const UsdtAssetConst: AssetId = USDT_ASSET;
    pub const BridgeDailyCapConst: Balance = 5_000;
    pub const BlocksPerDayConst: BlockNumber = 10;
    pub const WalletCooldownConst: BlockNumber = 5;
    pub const PayoutTaxBpsConst: u32 = 10;
    pub const HollarAssetConst: AssetId = HOLLAR_ASSET;
    pub const MaintenancePoolAccountConst: AccountId = MAINTENANCE_ACCOUNT;
    pub const MaintenanceFeeBpsConst: u32 = 2000;
    pub const GovernanceBondMinimumConst: Balance = 1_000;
    pub const LiquiditySafetyBpsConst: u32 = 8_500;
    pub const CredentialTtlConst: BlockNumber = 1_000;
    pub const MaxPlazaTagLenConst: u32 = 32;
    pub const MaxOrderPayloadLengthConst: u32 = 1024;
    pub const MaxOrderSaltLengthConst: u32 = 128;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
        Polkamarkt: pallet_polkamarkt::{Pallet, Call, Storage, Event<T>},
    }
);

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type Header = Header;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = ConstU32<16>;
}

pub struct MockAssets;

pub struct TestWeightInfo;
impl crate::WeightInfo for TestWeightInfo {
    fn create_condition() -> Weight {
        Weight::zero()
    }
    fn create_market(_routed_transfers: u32) -> Weight {
        Weight::zero()
    }
    fn commit_order() -> Weight {
        Weight::zero()
    }
    fn reveal_order() -> Weight {
        Weight::zero()
    }
    fn set_bridge_wallet() -> Weight {
        Weight::zero()
    }
    fn bridge_deposit() -> Weight {
        Weight::zero()
    }
    fn bridge_withdraw() -> Weight {
        Weight::zero()
    }
    fn bond_governance() -> Weight {
        Weight::zero()
    }
    fn unbond_governance() -> Weight {
        Weight::zero()
    }
}

pub struct NoRouterWeight;
impl frame_support::traits::Get<Weight> for NoRouterWeight {
    fn get() -> Weight {
        Weight::zero()
    }
}

impl pallet_polkamarkt::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = TestWeightInfo;
    type CanonicalStableAssetId = CanonicalStable;
    type Assets = MockAssets;
    type AssetId = AssetId;
    type Balance = Balance;
    type FeeCollector = FeeCollectorAccount;
    type MinQuestionLength = MinQuestionLengthConst;
    type CreationFeeBps = CreationFeeBpsConst;
    type MinCreationFee = MinCreationFeeConst;
    type PalletId = TestPalletId;
    type OrderbookIntegration = pallet_polkamarkt::OrderbookEventEmitter<Test>;
    type CollateralRouter = ();
    type CollateralRouterWeight = NoRouterWeight;
    type MinMarketDuration = MinMarketDurationConst;
    type CommitmentRevealDelay = CommitmentDelayConst;
    type CommitmentExpiry = CommitmentExpiryConst;
    type MaxMetadataLength = MaxMetadataLengthConst;
    type OpenInterestThreshold = OpenInterestThresholdConst;
    type CreatorRewardBps = CreatorRewardBpsConst;
    type ForkTaxAccount = ForkTaxAccountConst;
    type UsdcAssetId = UsdcAssetConst;
    type UsdtAssetId = UsdtAssetConst;
    type BridgeDailyCap = BridgeDailyCapConst;
    type BlocksPerDay = BlocksPerDayConst;
    type WalletCooldown = WalletCooldownConst;
    type PayoutTaxBps = PayoutTaxBpsConst;
    type HollarAssetId = HollarAssetConst;
    type MaintenancePoolAccount = MaintenancePoolAccountConst;
    type MaintenanceFeeBps = MaintenanceFeeBpsConst;
    type GovernanceBondMinimum = GovernanceBondMinimumConst;
    type LiquiditySafetyBps = LiquiditySafetyBpsConst;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type CredentialTtl = CredentialTtlConst;
    type CredentialsRequired = ConstBool<true>;
    type MaxPlazaTagLength = MaxPlazaTagLenConst;
    type MaxOrderPayloadLength = MaxOrderPayloadLengthConst;
    type MaxOrderSaltLength = MaxOrderSaltLengthConst;
    type PlazaIntegration = MockPlazaIntegration;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    ASSET_BALANCES.with(|balances| balances.borrow_mut().clear());
    set_balance(ALICE, CANONICAL_ASSET, 1_000_000_000_000);
    set_balance(BOB, CANONICAL_ASSET, 1_000_000_000_000);
    set_balance(ALICE, USDC_ASSET, 1_000_000_000_000);
    set_balance(ALICE, USDT_ASSET, 1_000_000_000_000);
    set_balance(BOB, USDC_ASSET, 1_000_000_000_000);
    set_balance(BOB, USDT_ASSET, 1_000_000_000_000);

    let t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .expect("frame system storage build");
    t.into()
}

pub fn run_to_block(n: BlockNumber) {
    System::set_block_number(n);
}

pub fn set_balance(account: AccountId, asset: AssetId, amount: Balance) {
    ASSET_BALANCES.with(|balances| {
        balances.borrow_mut().insert((account, asset), amount);
    });
}

pub fn balance_of(account: AccountId, asset: AssetId) -> Balance {
    ASSET_BALANCES
        .with(|balances| balances.borrow().get(&(account, asset)).copied())
        .unwrap_or_default()
}

pub struct MockPlazaIntegration;

impl PlazaIntegrationHook<OpengovProposalOf<Test>> for MockPlazaIntegration {
    fn on_opengov_condition(condition_id: ConditionId, metadata: &OpengovProposalOf<Test>) {
        PLAZA_NOTIFIED.with(|cell| {
            *cell.borrow_mut() = Some(condition_id);
        });
        pallet_polkamarkt::PolkadotPlazaBridge::<Test>::on_opengov_condition(
            condition_id,
            metadata,
        );
    }
}

pub fn reset_plaza_notifications() {
    PLAZA_NOTIFIED.with(|cell| *cell.borrow_mut() = None);
}

pub fn last_plaza_condition() -> Option<ConditionId> {
    PLAZA_NOTIFIED.with(|cell| *cell.borrow())
}

impl AssetTransfer<AccountId, AssetId, Balance> for MockAssets {
    fn transfer(
        asset: AssetId,
        from: &AccountId,
        to: &AccountId,
        amount: Balance,
    ) -> frame_support::dispatch::DispatchResult {
        ASSET_BALANCES.with(|balances| {
            let mut map = balances.borrow_mut();
            let from_key = (*from, asset);
            let from_balance = map.get(&from_key).copied().unwrap_or_default();
            if from_balance < amount {
                return Err(DispatchError::Other("insufficient-balance"));
            }
            map.insert(from_key, from_balance - amount);
            let to_key = (*to, asset);
            let to_balance = map.get(&to_key).copied().unwrap_or_default();
            map.insert(to_key, to_balance + amount);
            Ok(())
        })
    }
}
