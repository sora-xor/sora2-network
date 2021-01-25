// Creating mock Test here

use crate as eth_bridge;
use crate::{AssetKind, KEY_TYPE};
use codec::{Codec, Decode, Encode};
use common::{balance::Balance, Amount, AssetId, AssetId32, AssetSymbol, VAL};
use currencies::BasicCurrencyAdapter;
use frame_support::{
    assert_ok, construct_runtime,
    dispatch::{DispatchInfo, GetDispatchInfo},
    parameter_types,
    sp_io::TestExternalities,
    sp_runtime::{
        self,
        app_crypto::{
            sp_core,
            sp_core::{
                crypto::AccountId32,
                ecdsa,
                offchain::{OffchainExt, TransactionPoolExt},
                sr25519,
                testing::KeyStore,
                traits::KeystoreExt,
                Pair, Public,
            },
            CryptoTypePublicPair,
        },
        offchain::{
            testing::{OffchainState, PoolState, TestOffchainExt, TestTransactionPoolExt},
            OffchainStorage,
        },
        serde::{Serialize, Serializer},
        traits::{
            Applyable, Block, Checkable, DispatchInfoOf, Dispatchable, IdentifyAccount,
            PostDispatchInfoOf, SignedExtension, ValidateUnsigned, Verify,
        },
        transaction_validity::TransactionValidityError,
        MultiSigner, Percent,
        {
            generic,
            testing::Header,
            traits::{self, BlakeTwo256, IdentityLookup},
            transaction_validity::{TransactionSource, TransactionValidity},
            ApplyExtrinsicResultWithInfo, MultiSignature, Perbill,
        },
    },
    weights::{Pays, Weight},
};
use frame_system as system;
use frame_system::offchain::{Account, SigningTypes};
use parking_lot::RwLock;
use permissions::{Scope, MINT};
use sp_std::{convert::TryFrom, fmt::Debug, str::FromStr, sync::Arc};
use std::collections::HashSet;

pub const PSWAP: AssetId = AssetId::PSWAP;
pub const XOR: AssetId = AssetId::XOR;

/// An index to a block.
pub type BlockNumber = u64;

pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

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
    type PalletInfo = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
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
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl tokens::Trait for Test {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Trait>::AssetId;
    type OnReceived = ();
    type WeightInfo = ();
}

parameter_types! {
    pub const GetBaseAssetId: AssetId32<AssetId> = AssetId32::from_asset_id(XOR);
}

impl currencies::Trait for Test {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Test as assets::Trait>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Trait for Test {
    type Event = Event;
    type AssetId = common::AssetId32<AssetId>;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Test>;
    type WeightInfo = ();
}

impl permissions::Trait for Test {
    type Event = Event;
}

parameter_types! {
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
}

impl bridge_multisig::Trait for Test {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl pallet_sudo::Trait for Test {
    type Call = Call;
    type Event = Event;
}

parameter_types! {
    pub const UnsignedPriority: u64 = 100;
}

impl crate::Trait for Test {
    type PeerId = crate::crypto::TestAuthId;
    type Call = Call;
    type Event = Event;
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
        System: system::{Module, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
        Multisig: bridge_multisig::{Module, Call, Storage, Config<T>, Event<T>},
        Tokens: tokens::{Module, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Module, Call, Storage,  Event<T>},
        Assets: assets::{Module, Call, Storage, Config<T>, Event<T>},
        Permissions: permissions::{Module, Call, Storage, Config<T>, Event<T>},
        Sudo: pallet_sudo::{Module, Call, Storage, Config<T>, Event<T>},
        EthBridge: eth_bridge::{Module, Call, Storage, Config<T>, Event<T>},
    }
);

pub type SubstrateAccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub struct State {
    pub bridge_account_id: AccountId32,
    pub authority_account_id: AccountId32,
    pub ocw_keypairs: Vec<(MultiSigner, AccountId32, [u8; 32])>,
    pub pool_state: Arc<RwLock<PoolState>>,
    pub offchain_state: Arc<RwLock<OffchainState>>,
}

pub struct ExtBuilder {
    peers_num: usize,
    tokens: Vec<(common::AssetId32<AssetId>, Option<sp_core::H160>, AssetKind)>,
    reserves: Vec<(common::AssetId32<AssetId>, Balance)>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            peers_num: 4,
            tokens: vec![
                (
                    AssetId::XOR.into(),
                    Some(
                        sp_core::H160::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677")
                            .unwrap(),
                    ),
                    AssetKind::SidechainOwned,
                ),
                (
                    AssetId::VAL.into(),
                    Some(
                        sp_core::H160::from_str("3f9feac97e5feb15d8bf98042a9a01b515da3dfb")
                            .unwrap(),
                    ),
                    AssetKind::SidechainOwned,
                ),
            ],
            reserves: vec![
                (XOR.into(), Balance::from(350_000u32)),
                (VAL.into(), Balance::from(33_900_000u32)),
            ],
        }
    }
}

impl ExtBuilder {
    pub fn new() -> (TestExternalities, State) {
        Self::default().build()
    }

    pub fn peers_num(mut self, n: usize) -> Self {
        self.peers_num = n;
        self
    }

    pub fn with_reserves(mut self, reserves: Vec<(common::AssetId32<AssetId>, Balance)>) -> Self {
        self.reserves = reserves;
        self
    }

    pub fn build(self) -> (TestExternalities, State) {
        let (offchain, offchain_state) = TestOffchainExt::new();
        let (pool, pool_state) = TestTransactionPoolExt::new();
        let keystore = KeyStore::new();
        let _offchain_keys: Vec<_> = {
            let mut guard = keystore.write();
            guard.ecdsa_generate_new(KEY_TYPE, Some("//Alice")).unwrap();
            let kp = ecdsa::Pair::from_string("//Alice", None).unwrap();

            offchain_state
                .write()
                .local_storage
                .set(b"", b"key", &kp.to_raw_vec().encode());

            guard
                .keys(KEY_TYPE)
                .unwrap()
                .into_iter()
                .map(|CryptoTypePublicPair(_, raw)| AccountId32::try_from(&raw[1..]).unwrap())
                .collect()
        };

        let ocw_kps: Vec<_> = (0..self.peers_num)
            .map(|i| {
                let kp = ecdsa::Pair::from_string(&format!("//OCW{}", i), None).unwrap();
                let signer = AccountPublic::from(kp.public());
                (signer.clone(), signer.into_account(), kp.seed())
            })
            .collect();

        let root_account = get_account_id_from_seed::<sr25519::Public>("Alice");
        let multisig_account_id =
            bridge_multisig::Module::<Test>::multi_account_id(&root_account, 1, 0);
        let authority_account_id =
            bridge_multisig::Module::<Test>::multi_account_id(&root_account, 1, 1);
        let mut endowed_accounts: Vec<(_, AssetId32<AssetId>, _)> = vec![
            (
                multisig_account_id.clone(),
                PSWAP.into(),
                Balance::from(0u32),
            ),
            (multisig_account_id.clone(), XOR.into(), Balance::from(0u32)),
            (
                multisig_account_id.clone(),
                VAL.into(),
                Balance::from(33_900_000u32),
            ),
        ];
        endowed_accounts.extend(
            self.reserves
                .into_iter()
                .map(|(asset_id, balance)| (multisig_account_id.clone(), asset_id, balance)),
        );

        let endowed_assets: HashSet<_> = endowed_accounts
            .iter()
            .map(|x| (x.1, root_account.clone(), AssetSymbol(b"".to_vec()), 18))
            .collect();

        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();

        SudoConfig {
            key: endowed_accounts[0].0.clone(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        BalancesConfig {
            balances: endowed_accounts
                .iter()
                .filter_map(|(account_id, asset_id, balance)| {
                    if asset_id == &GetBaseAssetId::get() {
                        Some((account_id.clone(), balance.clone()))
                    } else {
                        None
                    }
                })
                .collect(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        MultisigConfig {
            accounts: core::iter::once((
                multisig_account_id.clone(),
                bridge_multisig::MultisigAccount::new(
                    ocw_kps.iter().map(|x| x.1.clone()).collect(),
                    Percent::from_parts(67),
                ),
            ))
            .collect(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![],
            initial_permissions: vec![(multisig_account_id.clone(), Scope::Unlimited, vec![MINT])],
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        TokensConfig {
            endowed_accounts: endowed_accounts.clone(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        AssetsConfig {
            endowed_assets: endowed_assets.into_iter().collect(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        EthBridgeConfig {
            peers: Default::default(),
            bridge_account: multisig_account_id.clone(),
            authority_account: authority_account_id.clone(),
            tokens: self.tokens,
            pswap_owners: vec![(
                sp_core::H160::from_str("40fd72257597aa14c7231a7b1aaa29fce868f677").unwrap(),
                Balance::from(300u128),
            )],
            ..Default::default()
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        let mut t = TestExternalities::from(storage);
        t.register_extension(OffchainExt::new(offchain));
        t.register_extension(TransactionPoolExt::new(pool));
        t.register_extension(KeystoreExt(keystore));
        t.execute_with(|| System::set_block_number(1));

        t.execute_with(|| {
            for (_, account_id, _) in &ocw_kps {
                assert_ok!(EthBridge::force_add_peer(
                    Origin::root(),
                    account_id.clone()
                ));
            }
        });

        let state = State {
            bridge_account_id: multisig_account_id,
            authority_account_id,
            ocw_keypairs: ocw_kps,
            pool_state,
            offchain_state,
        };
        (t, state)
    }
}

pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

pub type AccountPublic = <Signature as Verify>::Signer;

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> SubstrateAccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}
