use crate as iroha_migration; // for construct_runtime
use crate::{Trait, TECH_ACCOUNT_MAIN, TECH_ACCOUNT_PREFIX};
use codec::{Codec, Decode, Encode};
use common::{balance::Balance, Amount, AssetId, AssetId32, AssetSymbol, VAL};
use currencies::BasicCurrencyAdapter;
use frame_support::{
    construct_runtime,
    dispatch::{DispatchInfo, GetDispatchInfo},
    parameter_types,
    weights::{Pays, Weight},
};
use permissions::{Scope, MINT};
use sp_core::H256;
use sp_runtime::{
    self,
    app_crypto::sp_core::{self, crypto::AccountId32},
    generic,
    serde::{Serialize, Serializer},
    testing::Header,
    traits::{
        self, Applyable, BlakeTwo256, Block, Checkable, DispatchInfoOf, Dispatchable,
        IdentityLookup, PostDispatchInfoOf, SignedExtension, ValidateUnsigned,
    },
    transaction_validity::{TransactionSource, TransactionValidity, TransactionValidityError},
    ApplyExtrinsicResultWithInfo, Perbill,
};
use sp_std::fmt::Debug;

// Configure a mock runtime to test the pallet.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct MyExtra;
pub type TestExtrinsic = MyTestXt<Call, MyExtra>;
type NodeBlock = generic::Block<Header, TestExtrinsic>;
type DEXId = common::DEXId;
type AccountId = AccountId32;
type BlockNumber = u64;
type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
type TechAssetId = common::TechAssetId<common::AssetId, DEXId, common::LiquiditySourceType>;

pub const XOR: AssetId = AssetId::XOR;
pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const CHARLIE: u64 = 3;
pub const MINTING_ACCOUNT: u64 = 4;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug)]
pub struct MyTestXt<Call, Extra> {
    /// Signature of the extrinsic.
    pub signature: Option<(AccountId, Extra)>,
    /// Call of the extrinsic.
    pub call: Call,
}

parity_util_mem::malloc_size_of_is_0!(any: MyTestXt<Call, Extra>);

impl<Call: Codec + Sync + Send, Context, Extra> Checkable<Context> for MyTestXt<Call, Extra> {
    type Checked = Self;
    fn check(self, _c: &Context) -> Result<Self::Checked, TransactionValidityError> {
        Ok(self)
    }
}

impl<Call: Codec + Sync + Send, Extra> traits::Extrinsic for MyTestXt<Call, Extra> {
    type Call = Call;
    type SignaturePayload = (AccountId, Extra);

    fn is_signed(&self) -> Option<bool> {
        Some(self.signature.is_some())
    }

    fn new(c: Call, sig: Option<Self::SignaturePayload>) -> Option<Self> {
        Some(MyTestXt {
            signature: sig,
            call: c,
        })
    }
}

impl SignedExtension for MyExtra {
    type AccountId = AccountId;
    type Call = Call;
    type AdditionalSigned = ();
    type Pre = ();
    const IDENTIFIER: &'static str = "testextension";

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }
}

impl<Origin, Call, Extra> Applyable for MyTestXt<Call, Extra>
where
    Call:
        'static + Sized + Send + Sync + Clone + Eq + Codec + Debug + Dispatchable<Origin = Origin>,
    Extra: SignedExtension<AccountId = AccountId, Call = Call>,
    Origin: From<Option<AccountId32>>,
{
    type Call = Call;

    /// Checks to see if this is a valid *transaction*. It returns information on it if so.
    fn validate<U: ValidateUnsigned<Call = Self::Call>>(
        &self,
        _source: TransactionSource,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> TransactionValidity {
        Ok(Default::default())
    }

    /// Executes all necessary logic needed prior to dispatch and deconstructs into function call,
    /// index and sender.
    fn apply<U: ValidateUnsigned<Call = Self::Call>>(
        self,
        info: &DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> ApplyExtrinsicResultWithInfo<PostDispatchInfoOf<Self::Call>> {
        let maybe_who = if let Some((who, extra)) = self.signature {
            Extra::pre_dispatch(extra, &who, &self.call, info, len)?;
            Some(who)
        } else {
            Extra::pre_dispatch_unsigned(&self.call, info, len)?;
            None
        };

        Ok(self.call.dispatch(maybe_who.into()))
    }
}

impl<Call, Extra> Serialize for MyTestXt<Call, Extra>
where
    MyTestXt<Call, Extra>: Encode,
{
    fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.using_encoded(|bytes| seq.serialize_bytes(bytes))
    }
}

impl<Call: Encode, Extra: Encode> GetDispatchInfo for MyTestXt<Call, Extra> {
    fn get_dispatch_info(&self) -> DispatchInfo {
        // for testing: weight == size.
        DispatchInfo {
            weight: self.encode().len() as _,
            pays_fee: Pays::No,
            ..Default::default()
        }
    }
}

impl sp_runtime::traits::ExtrinsicMetadata for TestExtrinsic {
    const VERSION: u8 = 1;
    type SignedExtensions = ();
}

construct_runtime!(
    pub enum Test where
        Block = NodeBlock,
        NodeBlock = NodeBlock,
        UncheckedExtrinsic = TestExtrinsic
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        Multisig: multisig::{Module, Call, Storage, Config<T>, Event<T>},
        Tokens: tokens::{Module, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Module, Call, Storage,  Event<T>},
        Assets: assets::{Module, Call, Storage, Config<T>, Event<T>},
        Technical: technical::{Module, Call, Config<T>, Event<T>},
        Permissions: permissions::{Module, Call, Storage, Config<T>, Event<T>},
        ReferralSystem: referral_system::{Module, Call, Storage, Config<T>, Event},
        IrohaMigration: iroha_migration::{Module, Call, Storage, Config<T>, Event<T>}
    }
);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const GetBaseAssetId: AssetId32<AssetId> = AssetId32::from_asset_id(XOR);
    pub const ExistentialDeposit: u128 = 0;
}

impl frame_system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
}

impl technical::Trait for Test {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
    type WeightInfo = ();
}

impl assets::Trait for Test {
    type Event = Event;
    type AssetId = common::AssetId32<AssetId>;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Test>;
    type WeightInfo = ();
}

impl common::Trait for Test {
    type DEXId = DEXId;
}

impl permissions::Trait for Test {
    type Event = Event;
}

// Required by assets::Trait
impl currencies::Trait for Test {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Test as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

// Required by currencies::Trait
impl pallet_balances::Trait for Test {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

// Required by assets::Trait
impl tokens::Trait for Test {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

impl referral_system::Trait for Test {
    type Event = Event;
}

parameter_types! {
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
}

impl multisig::Trait for Test {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl Trait for Test {
    type Event = Event;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let tech_account_id =
        TechAccountId::Generic(TECH_ACCOUNT_PREFIX.to_vec(), TECH_ACCOUNT_MAIN.to_vec());

    let mut t = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    permissions::GenesisConfig::<Test> {
        initial_permission_owners: vec![(MINT, Scope::Unlimited, vec![MINTING_ACCOUNT])],
        initial_permissions: vec![(MINTING_ACCOUNT, Scope::Unlimited, vec![MINT])],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    assets::GenesisConfig::<Test> {
        endowed_assets: vec![(VAL, ALICE, AssetSymbol(b"VAL".to_vec()), 18)],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    println!("{:?}", VAL);

    tokens::GenesisConfig::<Test> {
        endowed_accounts: vec![(ALICE, VAL, 0u128.into())],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    technical::GenesisConfig::<Test> {
        account_ids_to_tech_account_ids: vec![(MINTING_ACCOUNT, tech_account_id.clone())],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    IrohaMigrationConfig {
        iroha_accounts: vec![
            (
                "did_sora_d9bda3688c6f608ab15c@sora".to_string(),
                Balance::from(0u128),
                None,
                1,
                vec![
                    "d9bda3688c6f608ab15c03a55b171da0413788a40a25722b4ae4d3672890bcd7".to_string(),
                ],
            ),
            (
                "did_sora_balance@sora".to_string(),
                Balance::from(300u128),
                None,
                1,
                vec![
                    "9a685d77bcd3f60e6cc1e91eedc7a48e11bbcf1a036b920f3bae0372a78a5432".to_string(),
                ],
            ),
            (
                "did_sora_referral@sora".to_string(),
                Balance::from(0u128),
                Some("did_sora_referrer@sora".to_string()),
                1,
                vec![
                    "cba1c8c2eeaf287d734bd167b10d762e89c0ee8327a29e04f064ae94086ef1e9".to_string(),
                ],
            ),
            (
                "did_sora_referrer@sora".to_string(),
                Balance::from(0u128),
                None,
                1,
                vec![
                    "dd54e9efb95531154316cf3e28e2232abab349296dde94353febc9ebbb3ff283".to_string(),
                ],
            ),
            (
                "did_sora_multi_sig@sora".to_string(),
                Balance::from(1000u128),
                None,
                2,
                vec![
                    "f7d89d39d48a67e4741a612de10650234f9148e84fe9e8b2a9fad322b0d8e5bc".to_string(),
                    "f56b4880ed91a25b257144acab749f615855c4b1b6a5d7891e1a6cdd9fd695e9".to_string(),
                    "57571ec82cff710143eba60c05d88de14a22799048137162d63c534a8b02dc20".to_string(),
                ],
            ),
        ],
        account_id: MINTING_ACCOUNT,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    multisig::GenesisConfig::<Test> { accounts: vec![] }
        .assimilate_storage(&mut t)
        .unwrap();

    t.into()
}
