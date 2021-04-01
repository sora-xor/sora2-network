// Creating mock Runtime here

use crate::{AssetConfig, Config, NetworkConfig};
use codec::{Codec, Decode, Encode};
use common::mock::ExistentialDeposits;
use common::prelude::Balance;
use common::{Amount, AssetId, AssetId32, AssetName, AssetSymbol, DEFAULT_BALANCE_PRECISION, VAL};
use currencies::BasicCurrencyAdapter;
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo};
use frame_support::sp_io::TestExternalities;
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::sp_runtime::app_crypto::sp_core::crypto::AccountId32;
use frame_support::sp_runtime::app_crypto::sp_core::offchain::{OffchainExt, TransactionPoolExt};
use frame_support::sp_runtime::app_crypto::sp_core::{ecdsa, sr25519, Pair, Public};
use frame_support::sp_runtime::offchain::testing::{
    OffchainState, PoolState, TestOffchainExt, TestTransactionPoolExt,
};
use frame_support::sp_runtime::serde::{Serialize, Serializer};
use frame_support::sp_runtime::testing::Header;
use frame_support::sp_runtime::traits::{
    self, Applyable, BlakeTwo256, Checkable, DispatchInfoOf, Dispatchable, IdentifyAccount,
    IdentityLookup, PostDispatchInfoOf, SignedExtension, ValidateUnsigned, Verify,
};
use frame_support::sp_runtime::transaction_validity::{
    TransactionSource, TransactionValidity, TransactionValidityError,
};
use frame_support::sp_runtime::{
    self, ApplyExtrinsicResultWithInfo, MultiSignature, MultiSigner, Perbill, Percent,
};
use frame_support::traits::GenesisBuild;
use frame_support::weights::{Pays, Weight};
use frame_support::{construct_runtime, parameter_types};
use frame_system::offchain::{Account, SigningTypes};
use parking_lot::RwLock;
use sp_core::H256;
use sp_keystore::testing::KeyStore;
use sp_keystore::KeystoreExt;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::fmt::Debug;
use sp_std::str::FromStr;
use sp_std::sync::Arc;
use std::collections::HashMap;
use {crate as eth_bridge, frame_system};

pub const PSWAP: AssetId = AssetId::PSWAP;
pub const XOR: AssetId = AssetId::XOR;

/// An index to a block.
pub type BlockNumber = u64;

pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const GetBaseAssetId: AssetId32<AssetId> = AssetId32::from_asset_id(XOR);
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
    pub const UnsignedPriority: u64 = 100;
    pub const EthNetworkId: <Runtime as Config>::NetworkId = 0;
}

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

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const ExistentialDeposit: u128 = 0;
}

impl frame_system::Config for Runtime {
    type BaseCallFilter = ();
    type BlockWeights = ();
    type BlockLength = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = PalletInfo;
    type SS58Prefix = ();
}

impl<T: SigningTypes> frame_system::offchain::SignMessage<T> for Runtime {
    type SignatureData = ();

    fn sign_message(&self, _message: &[u8]) -> Self::SignatureData {
        unimplemented!()
    }

    fn sign<TPayload, F>(&self, _f: F) -> Self::SignatureData
    where
        F: Fn(&Account<T>) -> TPayload,
        TPayload: frame_system::offchain::SignedPayload<T>,
    {
        unimplemented!()
    }
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
    Call: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: Call,
        _public: <Signature as Verify>::Signer,
        account: <Runtime as frame_system::Config>::AccountId,
        _index: <Runtime as frame_system::Config>::Index,
    ) -> Option<(
        Call,
        <TestExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
    )> {
        Some((call, (account, MyExtra {})))
    }
}

impl frame_system::offchain::SigningTypes for Runtime {
    type Public = <Signature as Verify>::Signer;
    type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
    Call: From<C>,
{
    type OverarchingCall = Call;
    type Extrinsic = TestExtrinsic;
}

impl pallet_balances::Config for Runtime {
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

impl tokens::Config for Runtime {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
}

impl currencies::Config for Runtime {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
    type GetNativeCurrencyId = <Runtime as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}

impl assets::Config for Runtime {
    type Event = Event;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<common::DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = common::AssetId32<AssetId>;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Module<Runtime>;
    type WeightInfo = ();
}

impl common::Config for Runtime {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
}

impl permissions::Config for Runtime {
    type Event = Event;
}

impl bridge_multisig::Config for Runtime {
    type Call = Call;
    type Event = Event;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl pallet_sudo::Config for Runtime {
    type Call = Call;
    type Event = Event;
}

impl crate::Config for Runtime {
    type PeerId = crate::crypto::TestAuthId;
    type Call = Call;
    type Event = Event;
    type NetworkId = u32;
    type GetEthNetworkId = EthNetworkId;
    type WeightInfo = common::weights::PresetWeightInfo;
}

impl sp_runtime::traits::ExtrinsicMetadata for TestExtrinsic {
    const VERSION: u8 = 1;
    type SignedExtensions = ();
}

construct_runtime!(
    pub enum Runtime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic
    {
        System: frame_system::{Module, Call, Config, Storage, Event<T>},
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
    pub networks: HashMap<u32, ExtendedNetworkConfig>,
    pub authority_account_id: AccountId32,
    pub pool_state: Arc<RwLock<PoolState>>,
    pub offchain_state: Arc<RwLock<OffchainState>>,
}

#[derive(Clone, Debug)]
pub struct ExtendedNetworkConfig {
    pub ocw_keypairs: Vec<(MultiSigner, AccountId32, [u8; 32])>,
    pub config: NetworkConfig<Runtime>,
}

pub struct ExtBuilder {
    pub networks: HashMap<u32, ExtendedNetworkConfig>,
    last_network_id: u32,
    root_account_id: AccountId32,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        let mut builder = Self {
            networks: Default::default(),
            last_network_id: Default::default(),
            root_account_id: get_account_id_from_seed::<sr25519::Public>("Alice"),
        };
        builder.add_network(
            vec![
                AssetConfig::Thischain { id: PSWAP.into() },
                AssetConfig::Sidechain {
                    id: XOR.into(),
                    sidechain_id: sp_core::H160::from_str(
                        "40fd72257597aa14c7231a7b1aaa29fce868f677",
                    )
                    .unwrap(),
                    owned: true,
                    precision: DEFAULT_BALANCE_PRECISION,
                },
                AssetConfig::Sidechain {
                    id: VAL.into(),
                    sidechain_id: sp_core::H160::from_str(
                        "3f9feac97e5feb15d8bf98042a9a01b515da3dfb",
                    )
                    .unwrap(),
                    owned: true,
                    precision: DEFAULT_BALANCE_PRECISION,
                },
            ],
            Some(vec![
                (XOR.into(), common::balance!(350000)),
                (VAL.into(), common::balance!(33900000)),
            ]),
            Some(4),
        );
        builder
    }
}

impl ExtBuilder {
    pub fn new() -> Self {
        Self {
            networks: Default::default(),
            last_network_id: Default::default(),
            root_account_id: get_account_id_from_seed::<sr25519::Public>("Alice"),
        }
    }

    pub fn add_currency(&mut self, network_id: u32, currency: AssetConfig<AssetId32<AssetId>>) {
        self.networks
            .get_mut(&network_id)
            .unwrap()
            .config
            .assets
            .push(currency);
    }

    pub fn add_network(
        &mut self,
        assets: Vec<AssetConfig<AssetId32<AssetId>>>,
        reserves: Option<Vec<(AssetId32<AssetId>, Balance)>>,
        peers_num: Option<usize>,
    ) -> u32 {
        let net_id = self.last_network_id;
        let multisig_account_id = bridge_multisig::Module::<Runtime>::multi_account_id(
            &self.root_account_id,
            1,
            net_id as u64 + 10,
        );
        let peers_keys = gen_peers_keys(&format!("OCW{}", net_id), peers_num.unwrap_or(4));
        self.networks.insert(
            net_id,
            ExtendedNetworkConfig {
                config: NetworkConfig {
                    initial_peers: peers_keys.iter().map(|(_, id, _)| id).cloned().collect(),
                    bridge_account_id: multisig_account_id.clone(),
                    assets,
                    bridge_contract_address: Default::default(),
                    reserves: reserves.unwrap_or_default(),
                },
                ocw_keypairs: peers_keys,
            },
        );
        self.last_network_id += 1;
        net_id
    }

    pub fn build(self) -> (TestExternalities, State) {
        let (offchain, offchain_state) = TestOffchainExt::new();
        let (pool, pool_state) = TestTransactionPoolExt::new();
        let authority_account_id =
            bridge_multisig::Module::<Runtime>::multi_account_id(&self.root_account_id, 1, 0);

        let mut bridge_accounts = Vec::new();
        let mut bridge_network_configs = Vec::new();
        let mut endowed_accounts: Vec<(_, AssetId32<AssetId>, _)> = Vec::new();
        let mut networks: Vec<_> = self.networks.clone().into_iter().collect();
        networks.sort_by(|(x, _), (y, _)| x.cmp(y));
        for (_net_id, ext_network) in networks {
            bridge_network_configs.push(ext_network.config.clone());
            endowed_accounts.extend(ext_network.config.assets.iter().cloned().map(
                |asset_config| {
                    (
                        ext_network.config.bridge_account_id.clone(),
                        asset_config.asset_id().clone(),
                        0,
                    )
                },
            ));
            endowed_accounts.extend(ext_network.config.reserves.iter().cloned().map(
                |(asset_id, _balance)| (ext_network.config.bridge_account_id.clone(), asset_id, 0),
            ));
            bridge_accounts.push((
                ext_network.config.bridge_account_id.clone(),
                bridge_multisig::MultisigAccount::new(
                    ext_network
                        .ocw_keypairs
                        .iter()
                        .map(|x| x.1.clone())
                        .collect(),
                    Percent::from_parts(67),
                ),
            ));
        }

        // pallet_balances and orml_tokens no longer accept duplicate elements.
        let mut unique_endowed_accounts: Vec<(_, AssetId32<AssetId>, _)> = Vec::new();
        for acc in endowed_accounts {
            if let Some(unique_acc) = unique_endowed_accounts.iter_mut().find(|a| a.1 == acc.1) {
                unique_acc.2 += acc.2;
            } else {
                unique_endowed_accounts.push(acc);
            }
        }
        let endowed_accounts = unique_endowed_accounts;

        let endowed_assets: BTreeSet<_> = endowed_accounts
            .iter()
            .map(|x| {
                (
                    x.1,
                    self.root_account_id.clone(),
                    AssetSymbol(b"".to_vec()),
                    AssetName(b"".to_vec()),
                    18,
                    Balance::from(0u32),
                    true,
                )
            })
            .collect();

        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        if !endowed_accounts.is_empty() {
            SudoConfig {
                key: endowed_accounts[0].0.clone(),
            }
            .assimilate_storage(&mut storage)
            .unwrap();
        }

        BalancesConfig {
            balances: Default::default(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        MultisigConfig {
            accounts: bridge_accounts,
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        PermissionsConfig {
            initial_permission_owners: vec![],
            initial_permissions: Vec::new(),
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
            networks: bridge_network_configs,
            authority_account: authority_account_id.clone(),
            val_master_contract_address: sp_core::H160::from_str(
                "47e229aa491763038f6a505b4f85d8eb463f0962",
            )
            .unwrap(),
            xor_master_contract_address: sp_core::H160::from_str(
                "12c6a709925783f49fcca0b398d13b0d597e6e1c",
            )
            .unwrap(),
        }
        .assimilate_storage(&mut storage)
        .unwrap();

        let mut t = TestExternalities::from(storage);
        t.register_extension(OffchainExt::new(offchain));
        t.register_extension(TransactionPoolExt::new(pool));
        t.register_extension(KeystoreExt(Arc::new(KeyStore::new())));
        t.execute_with(|| System::set_block_number(1));

        let state = State {
            networks: self.networks,
            authority_account_id,
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

pub fn gen_peers_keys(
    prefix: &str,
    peers_num: usize,
) -> Vec<(AccountPublic, AccountId32, [u8; 32])> {
    (0..peers_num)
        .map(|i| {
            let kp = ecdsa::Pair::from_string(&format!("//{}{}", prefix, i), None).unwrap();
            let signer = AccountPublic::from(kp.public());
            (signer.clone(), signer.into_account(), kp.seed())
        })
        .collect()
}
