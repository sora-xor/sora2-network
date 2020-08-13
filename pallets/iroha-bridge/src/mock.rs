// Creating mock runtime here

use crate::{Module, Trait};
use frame_support::{impl_outer_origin, parameter_types, weights::Weight};
use frame_system as system;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};
// use crate::Call;
use system::Call;

// use sp_api::impl_runtime_apis;
use sp_core::{Encode, OpaqueMetadata};
use sp_runtime::traits::{
    Block as BlockT, IdentifyAccount, SaturatedConversion, Saturating, Verify,
};
use sp_runtime::{
    create_runtime_str, generic,
    transaction_validity::{TransactionSource, TransactionValidity},
    ApplyExtrinsicResult, MultiSignature,
};
use sp_std::prelude::*;

// #[cfg(feature = "std")]
// use sp_version::NativeVersion;
// use sp_version::RuntimeVersion;
// use codec::Decode;
// use system::mock::{Call};
use frame_system::offchain::*;
use sp_core::offchain::{testing, TransactionPoolExt};
use sp_runtime::testing::{TestSignature, TestXt, UintAuthorityId};

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

// For testing the pallet, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of pallets we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct TestRuntime;
parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const UnsignedPriority: u64 = 100;
}

impl system::Trait for TestRuntime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = ();
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type ModuleToIndex = ();
    type AccountData = ();
    type OnNewAccount = ();
    type OnKilledAccount = ();
}

impl Trait for TestRuntime {
    type Event = ();
    type AuthorityId = crate::crypto::TestAuthId;
    type Call = Call;
    // type Event = Event;
    type UnsignedPriority = UnsignedPriority;
}

impl frame_system::offchain::SigningTypes for TestRuntime {
    type Public = UintAuthorityId;
    type Signature = TestSignature;
}

type Extrinsic = TestXt<Call, ()>;

impl frame_system::offchain::SendTransactionTypes<Call> for TestRuntime {
    type Extrinsic = Extrinsic;
    type OverarchingCall = Call;
}

// #[derive(codec::Encode, codec::Decode)]
// struct SimplePayload {
// 	pub public: UintAuthorityId,
// 	pub data: Vec<u8>,
// }
//
// impl SignedPayload<TestRuntime> for SimplePayload {
// 	fn public(&self) -> UintAuthorityId {
// 		self.public.clone()
// 	}
// }

struct DummyAppCrypto;
// Bind together the `SigningTypes` with app-crypto and the wrapper types.
// here the implementation is pretty dummy, because we use the same type for
// both application-specific crypto and the runtime crypto, but in real-life
// runtimes it's going to use different types everywhere.
impl AppCrypto<UintAuthorityId, TestSignature> for DummyAppCrypto {
    type RuntimeAppPublic = UintAuthorityId;
    type GenericPublic = UintAuthorityId;
    type GenericSignature = TestSignature;
}

// impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
// 	where
// 		Call: From<LocalCall>,
// {
// 	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
// 		call: Call,
// 		public: <Signature as sp_runtime::traits::Verify>::Signer,
// 		account: AccountId,
// 		index: Index,
// 	) -> Option<(
// 		Call,
// 		<UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
// 	)> {
// 		let period = BlockHashCount::get() as u64;
// 		let current_block = System::block_number()
// 			.saturated_into::<u64>()
// 			.saturating_sub(1);
// 		let tip = 0;
// 		let extra: SignedExtra = (
// 			frame_system::CheckTxVersion::<Runtime>::new(),
// 			frame_system::CheckGenesis::<Runtime>::new(),
// 			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
// 			frame_system::CheckNonce::<Runtime>::from(index),
// 			frame_system::CheckWeight::<Runtime>::new(),
// 			pallet_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
// 		);
//
// 		#[cfg_attr(not(feature = "std"), allow(unused_variables))]
// 			let raw_payload = SignedPayload::new(call, extra)
// 			.map_err(|e| {
// 				// debug::native::warn!("SignedPayload error: {:?}", e);
// 			})
// 			.ok()?;
//
// 		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
//
// 		let address = account;
// 		let (call, extra, _) = raw_payload.deconstruct();
// 		Some((call, (address, signature, extra)))
// 	}
// }
//
// impl frame_system::offchain::SigningTypes for Runtime {
// 	type Public = <Signature as sp_runtime::traits::Verify>::Signer;
// 	type Signature = Signature;
// }
//
// impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
// 	where
// 		Call: From<C>,
// {
// 	type OverarchingCall = Call;
// 	type Extrinsic = UncheckedExtrinsic;
// }
//
// pub type TemplateModule = Module<TestRuntime>;

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
    system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap()
        .into()
}
