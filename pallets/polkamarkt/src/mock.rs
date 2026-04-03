use crate::{
    self as pallet_polkamarkt, AssetTransfer, ConditionId, OpengovProposalOf, PlazaIntegrationHook,
};
use common::BuyBackHandler;
use frame_support::{
    construct_runtime, parameter_types,
    traits::{ConstU32, Everything},
    weights::Weight,
    PalletId,
};
use frame_system as system;
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
    traits::{BlakeTwo256, IdentityLookup},
    BuildStorage, DispatchError,
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
pub const BUYBACK_ASSET: AssetId = 2;
pub const USDC_ASSET: AssetId = 100;

thread_local! {
    static ASSET_BALANCES: RefCell<BTreeMap<(AccountId, AssetId), Balance>> = RefCell::new(BTreeMap::new());
    static PLAZA_NOTIFIED: RefCell<Option<ConditionId>> = RefCell::new(None);
    static LAST_BUYBACK_CALL: RefCell<Option<(AccountId, AssetId, AssetId, Balance)>> = const { RefCell::new(None) };
    static XOR_BURNED: RefCell<Balance> = const { RefCell::new(0) };
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
    pub const MaxMetadataLengthConst: u32 = 128;
    pub const TradeFeeBpsConst: u32 = 50;
    pub const BuyBackAssetConst: AssetId = BUYBACK_ASSET;
    pub const CreatorBondEscrowAccountConst: AccountId = MAINTENANCE_ACCOUNT;
    pub const GovernanceBondMinimumConst: Balance = 1_000;
    pub const MaxPlazaTagLenConst: u32 = 32;
}

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system::{Pallet, Call, Storage, Config<T>, Event<T>},
        Polkamarkt: pallet_polkamarkt::{Pallet, Call, Storage, Config<T>, Event<T>},
    }
);

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type RuntimeEvent = RuntimeEvent;
    type RuntimeTask = ();
    type Nonce = u64;
    type Block = Block;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<AccountId>;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type ExtensionsWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
    type MaxConsumers = ConstU32<16>;
}

pub struct MockAssets;

pub struct TestWeightInfo;
impl crate::WeightInfo for TestWeightInfo {
    fn create_condition() -> Weight {
        Weight::zero()
    }
    fn create_opengov_condition() -> Weight {
        Weight::zero()
    }
    fn create_market() -> Weight {
        Weight::zero()
    }
    fn buy() -> Weight {
        Weight::zero()
    }
    fn sell() -> Weight {
        Weight::zero()
    }
    fn sync_market_status() -> Weight {
        Weight::zero()
    }
    fn bond_governance() -> Weight {
        Weight::zero()
    }
    fn unbond_governance() -> Weight {
        Weight::zero()
    }
    fn resolve_market() -> Weight {
        Weight::zero()
    }
    fn cancel_market() -> Weight {
        Weight::zero()
    }
    fn claim_market() -> Weight {
        Weight::zero()
    }
    fn claim_creator_fees() -> Weight {
        Weight::zero()
    }
    fn claim_creator_liquidity() -> Weight {
        Weight::zero()
    }
    fn sweep_xor_buyback_and_burn() -> Weight {
        Weight::zero()
    }
}

pub struct MockBuyBackHandler;

impl BuyBackHandler<AccountId, AssetId> for MockBuyBackHandler {
    fn mint_buy_back_and_burn(
        _mint_asset_id: &AssetId,
        buy_back_asset_id: &AssetId,
        amount: common::Balance,
    ) -> Result<common::Balance, DispatchError> {
        LAST_BUYBACK_CALL.with(|call| {
            *call.borrow_mut() = Some((0, 0, *buy_back_asset_id, amount));
        });
        XOR_BURNED.with(|value| {
            let current = *value.borrow();
            *value.borrow_mut() = current.saturating_add(amount);
        });
        Ok(amount)
    }

    fn buy_back_and_burn(
        account_id: &AccountId,
        asset_id: &AssetId,
        buy_back_asset_id: &AssetId,
        amount: common::Balance,
    ) -> Result<common::Balance, DispatchError> {
        MockAssets::transfer(*asset_id, account_id, &999, amount)?;
        LAST_BUYBACK_CALL.with(|call| {
            *call.borrow_mut() = Some((*account_id, *asset_id, *buy_back_asset_id, amount));
        });
        XOR_BURNED.with(|value| {
            let current = *value.borrow();
            *value.borrow_mut() = current.saturating_add(amount);
        });
        Ok(amount)
    }
}

impl pallet_polkamarkt::Config for Test {
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
    type BuyBackHandler = MockBuyBackHandler;
    type GetBuyBackAssetId = BuyBackAssetConst;
    type MinMarketDuration = MinMarketDurationConst;
    type MaxMetadataLength = MaxMetadataLengthConst;
    type TradeFeeBps = TradeFeeBpsConst;
    type GovernanceBondMinimum = GovernanceBondMinimumConst;
    type CreatorBondEscrowAccount = CreatorBondEscrowAccountConst;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type MaxPlazaTagLength = MaxPlazaTagLenConst;
    type PlazaIntegration = MockPlazaIntegration;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
    ASSET_BALANCES.with(|balances| balances.borrow_mut().clear());
    LAST_BUYBACK_CALL.with(|call| *call.borrow_mut() = None);
    PLAZA_NOTIFIED.with(|cell| *cell.borrow_mut() = None);
    XOR_BURNED.with(|value| *value.borrow_mut() = 0);
    set_balance(ALICE, CANONICAL_ASSET, 1_000_000_000_000);
    set_balance(BOB, CANONICAL_ASSET, 1_000_000_000_000);
    set_balance(ALICE, USDC_ASSET, 1_000_000_000_000);
    set_balance(BOB, USDC_ASSET, 1_000_000_000_000);

    let t = SystemConfig::default()
        .build_storage()
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

pub fn xor_burned() -> Balance {
    XOR_BURNED.with(|value| *value.borrow())
}

pub fn last_buyback_call() -> Option<(AccountId, AssetId, AssetId, Balance)> {
    LAST_BUYBACK_CALL.with(|call| *call.borrow())
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
