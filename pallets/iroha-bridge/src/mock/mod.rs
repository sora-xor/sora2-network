// Creating mock runtime here

use crate as iroha_bridge;
use frame_support::traits::StorageMapShim;
use frame_support::{construct_runtime, parameter_types, weights::Weight};
use frame_system as system;
use frame_system::offchain::{Account, SigningTypes};
use parity_scale_codec::{Codec, Decode, Encode};
use sp_runtime::serde::{Serialize, Serializer};
use sp_runtime::traits::{
    Applyable, Checkable, DispatchInfoOf, Dispatchable, PostDispatchInfoOf, SignedExtension,
    ValidateUnsigned,
};
use sp_runtime::traits::{Block, IdentifyAccount, Verify};
use sp_runtime::transaction_validity::TransactionValidityError;
use sp_runtime::{
    generic,
    testing::Header,
    traits::{self, BlakeTwo256, IdentityLookup},
    transaction_validity::{TransactionSource, TransactionValidity},
    AccountId32, ApplyExtrinsicResultWithInfo, MultiSignature, Perbill,
};
use sp_std::fmt::Debug;
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo};
use frame_support::weights::Pays;
pub mod offchain_testing;
pub use offchain_testing::*;

/// An index to a block.
pub type BlockNumber = u64;

pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.

pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// The type for looking up accounts. We don't expect more than 4 billion of them, but you
/// never know...
pub type AccountIndex = u32;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Index = u64;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub struct MyExtra;
pub type TestExtrinsic = MyTestXt<Call, MyExtra>;
type NodeBlock = generic::Block<Header, TestExtrinsic>;
pub type DigestItem = generic::DigestItem<Hash>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ExistentialDeposit: u128 = 0;
}

impl system::Trait for Test {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = Call;
    type Index = Index;
    type BlockNumber = BlockNumber;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
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
    type ModuleToIndex = ModuleToIndex;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
}

impl treasury::Trait for Test {
    type Event = Event;
    type XOR = pallet_balances::Module<Test, pallet_balances::Instance1>;
    type DOT = pallet_balances::Module<Test, pallet_balances::Instance2>;
    type KSM = pallet_balances::Module<Test, pallet_balances::Instance3>;
}

impl<T: SigningTypes> system::offchain::SignMessage<T> for Test {
    type SignatureData = ();

    fn sign_message(&self, _message: &[u8]) -> Self::SignatureData {
        unimplemented!()
    }

    fn sign<TPayload, F>(&self, _f: F) -> Self::SignatureData
    where
        F: Fn(&Account<T>) -> TPayload,
        TPayload: system::offchain::SignedPayload<T>,
    {
        unimplemented!()
    }
}

impl<LocalCall> system::offchain::CreateSignedTransaction<LocalCall> for Test
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        _public: <Signature as Verify>::Signer,
        account: <Test as system::Trait>::AccountId,
        _index: <Test as system::Trait>::Index,
    ) -> Option<(
        Call,
        <TestExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
    )> {
        Some((call, (account, MyExtra {})))
    }
}

impl frame_system::offchain::SigningTypes for Test {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
    Call: From<C>,
{
    type OverarchingCall = Call;
    type Extrinsic = TestExtrinsic;
}

impl pallet_balances::Trait for Test {
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event; //pallet_balances::Event<Test>;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
}

/// XOR
impl pallet_balances::Trait<pallet_balances::Instance1> for Test {
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event; //pallet_balances::Event<Test, pallet_balances::Instance1>;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = StorageMapShim<
        pallet_balances::Account<Test, pallet_balances::Instance1>,
        system::CallOnCreatedAccount<Test>,
        system::CallKillAccount<Test>,
        AccountId,
        pallet_balances::AccountData<Balance>,
    >;
}

/// DOT
impl pallet_balances::Trait<pallet_balances::Instance2> for Test {
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = pallet_balances::Event<Test, pallet_balances::Instance2>;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = StorageMapShim<
        pallet_balances::Account<Test, pallet_balances::Instance1>,
        system::CallOnCreatedAccount<Test>,
        system::CallKillAccount<Test>,
        AccountId,
        pallet_balances::AccountData<Balance>,
    >;
}

/// KSM
impl pallet_balances::Trait<pallet_balances::Instance3> for Test {
    /// The type for recording an account's balance.
    type Balance = Balance;
    /// The ubiquitous event type.
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = StorageMapShim<
        pallet_balances::Account<Test, pallet_balances::Instance1>,
        system::CallOnCreatedAccount<Test>,
        system::CallKillAccount<Test>,
        AccountId,
        pallet_balances::AccountData<Balance>,
    >;
}

parameter_types! {
    pub const UnsignedPriority: u64 = 100;
}

impl iroha_bridge::Trait for Test {
    type AuthorityId = iroha_bridge::crypto::TestAuthId;
    type AuthorityIdEd = iroha_bridge::crypto_ed::TestAuthId;
    type Call = Call;
    type Event = Event;
    type UnsignedPriority = UnsignedPriority;
}

impl sp_runtime::traits::ExtrinsicMetadata for TestExtrinsic {
    const VERSION: u8 = 1;
    type SignedExtensions = ();
    // type SignedExtensions = (TestExtension, TestExtension2);
}

construct_runtime!(
    pub enum Test where
        Block = NodeBlock,
        NodeBlock = NodeBlock,
        UncheckedExtrinsic = TestExtrinsic
    {
        System: system::{Module, Call, Config, Storage, Event<T>},
        Treasury: treasury::{Module, Call, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        XOR: pallet_balances::<Instance1>::{Module, Call, Storage, Config<T>, Event<T>},
        DOT: pallet_balances::<Instance2>::{Module, Call, Storage, Config<T>, Event<T>},
        KSM: pallet_balances::<Instance3>::{Module, Call, Storage, Config<T>, Event<T>},
        IrohaBridge: iroha_bridge::{Module, Call, Storage, Config<T>, Event<T>},
    }
);

/// Executive: handles dispatch to the various modules.
pub type Executive =
    frame_executive::Executive<Test, NodeBlock, system::ChainContext<Test>, Test, AllModules>;

/*
/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("substrate-iroha-bridge"),
    impl_name: create_runtime_str!("substrate-iroha-bridge"),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
};

use sp_offchain::runtime_decl_for_OffchainWorkerApi::OffchainWorkerApi;
impl_runtime_apis! {
    impl sp_api::Core<NodeBlock> for Test {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: NodeBlock) {
            // Executive::execute_block(block)
        }

        fn initialize_block(header: &<NodeBlock as Block>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl sp_offchain::OffchainWorkerApi<NodeBlock> for Test {
        fn offchain_worker(header: &<NodeBlock as Block>::Header) {
            Executive::offchain_worker(header)
        }
    }
}
*/
use sp_runtime::BuildStorage;
pub fn new_test_ext(
    _root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
) -> sp_io::TestExternalities {
    GenesisConfig {
        system: Some(system::GenesisConfig::default()),
        // frame_system: Some(SystemConfig {
        //     code: WASM_BINARY.to_vec(),
        //     changes_trie_config: Default::default(),
        // }),
        pallet_balances_Instance1: Some(XORConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .filter(|x| {
                    x != &AccountId32::from([
                        52u8, 45, 84, 67, 137, 84, 47, 252, 35, 59, 237, 44, 144, 70, 71, 206, 243,
                        67, 8, 115, 247, 189, 204, 26, 181, 226, 232, 81, 123, 12, 81, 120,
                    ])
                })
                // .map(|k| (k, 1 << 60))
                .map(|k| (k, 0))
                .collect(),
        }),
        pallet_balances_Instance2: Some(DOTConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1 << 8))
                .collect(),
        }),
        pallet_balances_Instance3: Some(KSMConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1 << 8))
                .collect(),
        }),
        pallet_balances: Some(BalancesConfig {
            balances: endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, 1 << 60))
                .collect(),
        }),
        // pallet_sudo: Some(SudoConfig { key: root_key }),
        iroha_bridge: Some(IrohaBridgeConfig {
            authorities: endowed_accounts.clone(),
        }),
    }
    .build_storage()
    .unwrap()
    .into()
    // system::GenesisConfig::default()
    //     .build_storage::<Test>()
    //     .unwrap()
    //     .into()
}
