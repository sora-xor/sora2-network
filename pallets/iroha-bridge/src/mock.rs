// Creating mock runtime here

use frame_support::traits::StorageMapShim;
use frame_support::{construct_runtime, impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use frame_system::offchain::{Account, SignMessage, SigningTypes};
use sp_core::{
    sr25519::{self},
    H256,
};
use sp_runtime::{
    generic,
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    MultiSignature, MultiSigner, Perbill,
};
use sp_runtime::{
    testing::TestXt,
    traits::{Block, IdentifyAccount, Verify},
};
// use crate::{Call, Event};
// use crate::{Module, Trait};
use crate as iroha_bridge;

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

// pub type System = system::Module<Test>;
type TestExtrinsic = TestXt<Call, ()>;
type NodeBlock = generic::Block<Header, TestExtrinsic>;
// type SubmitTransaction = system::offchain::TransactionSubmitter<UintAuthorityId, Call, Extrinsic>;
pub type DigestItem = generic::DigestItem<Hash>;

// impl_outer_origin! {
// 	pub enum Origin for Test {}
// }

// For testing the pallet, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of pallets we want to use.
// #[derive(Clone, Eq, PartialEq)]
// pub struct Test;

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

    fn sign_message(&self, message: &[u8]) -> Self::SignatureData {
        unimplemented!()
    }

    fn sign<TPayload, F>(&self, f: F) -> Self::SignatureData
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
        _account: <Test as system::Trait>::AccountId,
        index: <Test as system::Trait>::Index,
    ) -> Option<(
        Call,
        <TestExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
    )> {
        Some((call, (index, ())))
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

/*
impl<LocalCall> system::offchain::CreateSignedTransaction<LocalCall> for Test
    where
        Call: From<LocalCall>,
{
    fn create_transaction<C: system::offchain::AppCrypto<Self::Public, Self::Signature>>(
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
        let tip = 0;
        let extra: SignedExtra = (
            system::CheckSpecVersion::<Test>::new(),
            system::CheckTxVersion::<Test>::new(),
            system::CheckGenesis::<Test>::new(),
            system::CheckEra::<Test>::from(generic::Era::mortal(period, current_block)),
            system::CheckNonce::<Test>::from(index),
            system::CheckWeight::<Test>::new(),
            transaction_payment::ChargeTransactionPayment::<Test>::from(tip),
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
*/

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

pub fn new_test_ext(
    root_key: AccountId,
    endowed_accounts: Vec<AccountId>,
) -> sp_io::TestExternalities {
    // system::GenesisConfig {
    //     // frame_system: Some(SystemConfig {
    //     //     code: WASM_BINARY.to_vec(),
    //     //     changes_trie_config: Default::default(),
    //     // }),
    //     pallet_balances: Some(BalancesConfig {
    //         balances: endowed_accounts
    //             .iter()
    //             .cloned()
    //             .map(|k| (k, 1 << 60))
    //             .collect(),
    //     }),
    //     // pallet_sudo: Some(SudoConfig { key: root_key }),
    // }.build_storage::<Test>().unwrap().into()
    system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap()
        .into()
}
