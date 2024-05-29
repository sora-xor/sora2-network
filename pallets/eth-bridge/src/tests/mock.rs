// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Creating mock Runtime here

use crate::offchain::SignedTransactionData;
use crate::types::{
    SubstrateBlockLimited, SubstrateHeaderLimited, SubstrateSignedBlockLimited, U64,
};
use crate::{
    AssetConfig, Config, NetworkConfig, NodeParams, CONFIRMATION_INTERVAL, STORAGE_ETH_NODE_PARAMS,
    STORAGE_FAILED_PENDING_TRANSACTIONS_KEY, STORAGE_NETWORK_IDS_KEY, STORAGE_PEER_SECRET_KEY,
    STORAGE_PENDING_TRANSACTIONS_KEY, STORAGE_SUB_NODE_URL_KEY,
    STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY, SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK,
};
use codec::{Codec, Decode, Encode};
use common::mock::{ExistentialDeposits, WeightToFixedFee};
use common::prelude::Balance;
use common::{
    mock_currencies_config, mock_pallet_balances_config, Amount, AssetId32, AssetName, AssetSymbol,
    DEXId, LiquiditySourceType, PredefinedAssetId, DEFAULT_BALANCE_PRECISION, VAL, XOR, XST,
};
use core::cell::RefCell;
use currencies::BasicCurrencyAdapter;
use frame_support::dispatch::{DispatchInfo, GetDispatchInfo, Pays, UnfilteredDispatchable};
use frame_support::sp_io::TestExternalities;
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::sp_runtime::app_crypto::sp_core::crypto::AccountId32;
use frame_support::sp_runtime::app_crypto::sp_core::offchain::{OffchainDbExt, TransactionPoolExt};
use frame_support::sp_runtime::app_crypto::sp_core::{ecdsa, sr25519, Pair, Public};
use frame_support::sp_runtime::offchain::http;
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
    self, ApplyExtrinsicResultWithInfo, MultiSignature, MultiSigner, Perbill,
};
use frame_support::traits::{Everything, GenesisBuild, Get, PrivilegeCmp};
use frame_support::weights::Weight;
use frame_support::{construct_runtime, parameter_types};
use frame_system::offchain::{Account, SigningTypes};
use frame_system::EnsureRoot;
use hex_literal::hex;
use parking_lot::RwLock;
use rustc_hex::ToHex;
use sp_core::offchain::{OffchainStorage, OffchainWorkerExt};
use sp_core::{H160, H256};
use sp_keystore::testing::KeyStore;
use sp_keystore::{KeystoreExt, SyncCryptoStore};
use sp_std::cmp::Ordering;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::fmt::Debug;
use sp_std::str::FromStr;
use sp_std::sync::Arc;
use std::borrow::Cow;
use std::collections::HashMap;
use {crate as eth_bridge, frame_system};

pub const PSWAP: PredefinedAssetId = PredefinedAssetId::PSWAP;

/// An index to a block.
pub type BlockNumber = u64;

pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

parameter_types! {
    pub const GetBaseAssetId: AssetId32<PredefinedAssetId> = XOR;
    pub const DepositBase: u64 = 1;
    pub const DepositFactor: u64 = 1;
    pub const MaxSignatories: u16 = 4;
    pub const UnsignedPriority: u64 = 100;
    pub const EthNetworkId: <Runtime as Config>::NetworkId = 0;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Debug, scale_info::TypeInfo)]
pub struct MyTestXt<RuntimeCall, Extra> {
    /// Signature of the extrinsic.
    pub signature: Option<(AccountId, Extra)>,
    /// RuntimeCall of the extrinsic.
    pub call: RuntimeCall,
}

parity_util_mem::malloc_size_of_is_0!(any: MyTestXt<RuntimeCall, Extra>);

impl<RuntimeCall: Codec + Sync + Send, Context, Extra> Checkable<Context>
    for MyTestXt<RuntimeCall, Extra>
{
    type Checked = Self;
    fn check(self, _c: &Context) -> Result<Self::Checked, TransactionValidityError> {
        Ok(self)
    }

    #[cfg(feature = "try-runtime")]
    fn unchecked_into_checked_i_know_what_i_am_doing(
        self,
        _c: &Context,
    ) -> Result<Self::Checked, TransactionValidityError> {
        unreachable!();
    }
}

impl<RuntimeCall: Codec + Sync + Send, Extra> traits::Extrinsic for MyTestXt<RuntimeCall, Extra> {
    type Call = RuntimeCall;
    type SignaturePayload = (AccountId, Extra);

    fn is_signed(&self) -> Option<bool> {
        Some(self.signature.is_some())
    }

    fn new(c: RuntimeCall, sig: Option<Self::SignaturePayload>) -> Option<Self> {
        Some(MyTestXt {
            signature: sig,
            call: c,
        })
    }
}

impl SignedExtension for MyExtra {
    const IDENTIFIER: &'static str = "testextension";
    type AccountId = AccountId;
    type Call = RuntimeCall;
    type AdditionalSigned = ();
    type Pre = ();

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn pre_dispatch(
        self,
        _who: &Self::AccountId,
        _call: &Self::Call,
        _info: &DispatchInfoOf<Self::Call>,
        _len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        Ok(())
    }
}

impl<Origin, RuntimeCall, Extra> Applyable for MyTestXt<RuntimeCall, Extra>
where
    RuntimeCall: 'static
        + Sized
        + Send
        + Sync
        + Clone
        + Eq
        + Codec
        + Debug
        + Dispatchable<RuntimeOrigin = Origin>,
    Extra: SignedExtension<AccountId = AccountId, Call = RuntimeCall>,
    Origin: From<Option<AccountId32>>,
{
    type Call = RuntimeCall;

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

impl<RuntimeCall, Extra> Serialize for MyTestXt<RuntimeCall, Extra>
where
    MyTestXt<RuntimeCall, Extra>: Encode,
{
    fn serialize<S>(&self, seq: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.using_encoded(|bytes| seq.serialize_bytes(bytes))
    }
}

impl<RuntimeCall: Encode, Extra: Encode> GetDispatchInfo for MyTestXt<RuntimeCall, Extra> {
    fn get_dispatch_info(&self) -> DispatchInfo {
        // for testing: weight == size.
        DispatchInfo {
            weight: Weight::from_parts(self.encode().len() as u64, 0),
            pays_fee: Pays::No,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, scale_info::TypeInfo)]
pub struct MyExtra;
pub type TestExtrinsic = MyTestXt<RuntimeCall, MyExtra>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
    pub const RemovePendingOutgoingRequestsAfter: BlockNumber = 100;
    pub const TrackPendingIncomingRequestsAfter: (BlockNumber, u64) = (0, 0);
    pub const SchedulerMaxWeight: Weight = Weight::from_parts(1024, 0);
}

pub struct RemoveTemporaryPeerAccountId;
impl Get<Vec<(AccountId, H160)>> for RemoveTemporaryPeerAccountId {
    fn get() -> Vec<(AccountId, H160)> {
        vec![(
            AccountId32::new(hex!(
                "0000000000000000000000000000000000000000000000000000000000000001"
            )),
            H160(hex!("0000000000000000000000000000000000000001")),
        )]
    }
}

mock_pallet_balances_config!(Runtime);
mock_currencies_config!(Runtime);

impl frame_system::Config for Runtime {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type RuntimeEvent = RuntimeEvent;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = pallet_balances::AccountData<Balance>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = ();
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<65536>;
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
    RuntimeCall: From<LocalCall>,
{
    fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
        call: RuntimeCall,
        _public: <Signature as Verify>::Signer,
        account: <Runtime as frame_system::Config>::AccountId,
        _index: <Runtime as frame_system::Config>::Index,
    ) -> Option<(
        RuntimeCall,
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
    RuntimeCall: From<C>,
{
    type Extrinsic = TestExtrinsic;
    type OverarchingCall = RuntimeCall;
}

impl tokens::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Runtime as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

parameter_types! {
    pub const GetBuyBackAssetId: common::AssetId32<PredefinedAssetId> = XST;
    pub GetBuyBackSupplyAssets: Vec<common::AssetId32<PredefinedAssetId>> = vec![VAL, PSWAP.into()];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, LiquiditySourceType, [u8; 32]>;
    type AssetId = common::AssetId32<PredefinedAssetId>;
    type GetBaseAssetId = GetBaseAssetId;
    type GetBuyBackAssetId = GetBuyBackAssetId;
    type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
    type GetBuyBackPercentage = GetBuyBackPercentage;
    type GetBuyBackAccountId = GetBuyBackAccountId;
    type GetBuyBackDexId = GetBuyBackDexId;
    type BuyBackLiquidityProxy = ();
    type Currency = currencies::Pallet<Runtime>;
    type GetTotalBalance = ();
    type WeightInfo = ();
    type AssetRegulator = permissions::Pallet<Runtime>;
}

impl common::Config for Runtime {
    type DEXId = DEXId;
    type LstId = LiquiditySourceType;
    type AssetManager = assets::Pallet<Runtime>;
    type MultiCurrency = currencies::Pallet<Runtime>;
}

impl permissions::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
}

impl bridge_multisig::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type Currency = Balances;
    type DepositBase = DepositBase;
    type DepositFactor = DepositFactor;
    type MaxSignatories = MaxSignatories;
    type WeightInfo = ();
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
}

/// Used the compare the privilege of an origin inside the scheduler.
pub struct OriginPrivilegeCmp;

impl PrivilegeCmp<OriginCaller> for OriginPrivilegeCmp {
    fn cmp_privilege(left: &OriginCaller, right: &OriginCaller) -> Option<Ordering> {
        if left == right {
            return Some(Ordering::Equal);
        }

        match (left, right) {
            // Root is greater than anything.
            (OriginCaller::system(frame_system::RawOrigin::Root), _) => Some(Ordering::Greater),
            // For every other origin we don't care, as they are not used for `ScheduleOrigin`.
            _ => None,
        }
    }
}

impl pallet_scheduler::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeOrigin = RuntimeOrigin;
    type PalletsOrigin = OriginCaller;
    type RuntimeCall = RuntimeCall;
    type MaximumWeight = SchedulerMaxWeight;
    type ScheduleOrigin = EnsureRoot<AccountId>;
    type MaxScheduledPerBlock = ();
    type WeightInfo = ();
    type OriginPrivilegeCmp = OriginPrivilegeCmp;
    type Preimages = ();
}

impl crate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type PeerId = crate::offchain::crypto::TestAuthId;
    type RuntimeCall = RuntimeCall;
    type NetworkId = u32;
    type GetEthNetworkId = EthNetworkId;
    type WeightInfo = ();
    type Mock = State;
    type WeightToFee = WeightToFixedFee;
    type MessageStatusNotifier = ();
    type BridgeAssetLockChecker = ();
    type AssetInfoProvider = assets::Pallet<Runtime>;
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
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
        Multisig: bridge_multisig::{Pallet, Call, Storage, Config<T>, Event<T>},
        Tokens: tokens::{Pallet, Call, Storage, Config<T>, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Storage, Config<T>, Event<T>},
        Permissions: permissions::{Pallet, Call, Storage, Config<T>, Event<T>},
        Sudo: pallet_sudo::{Pallet, Call, Storage, Config<T>, Event<T>},
        EthBridge: eth_bridge::{Pallet, Call, Storage, Config<T>, Event<T>},
        Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
    }
);

pub type SubstrateAccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub trait Mock {
    fn on_request(pending_request: &http::PendingRequest, url: &str, body: Cow<'_, str>);
    fn should_fail_send_signed_transaction() -> bool;
}

thread_local! {
    pub static RESPONSES: RefCell<Vec<Vec<u8>>> = RefCell::new(Vec::new());
    pub static OFFCHAIN_STATE: RefCell<Option<Arc<RwLock<OffchainState>>>> = RefCell::new(None);
    pub static SHOULD_FAIL_SEND_SIGNED_TRANSACTION: RefCell<bool> = RefCell::new(false);
}

fn push_response(data: Vec<u8>) {
    RESPONSES.with(|ref_cell| {
        ref_cell.borrow_mut().push(data);
    });
}

fn json_rpc_response<T: Serialize>(value: T) -> jsonrpc_core::Response {
    use jsonrpc_core::{Output, Response, Success};
    Response::Single(Output::Success(Success {
        jsonrpc: Some(jsonrpc_core::Version::V2),
        result: serde_json::to_value(value).unwrap(),
        id: jsonrpc_core::Id::Num(0),
    }))
}

fn push_json_rpc_response<T: Serialize>(value: T) {
    let json_rpc_response = json_rpc_response(value);
    push_response(serde_json::to_vec(&json_rpc_response).unwrap());
}

pub struct State {
    pub networks: HashMap<u32, ExtendedNetworkConfig>,
    pub authority_account_id: AccountId32,
    pub pool_state: Arc<RwLock<PoolState>>,
    pub offchain_state: Arc<RwLock<OffchainState>>,
    responses: Vec<Vec<u8>>,
}

impl Mock for State {
    fn on_request(pending_request: &http::PendingRequest, url: &str, body: Cow<'_, str>) {
        OFFCHAIN_STATE.with(|oc_state_ref_cell| {
            RESPONSES.with(|ref_cell| {
                let oc_state_opt = oc_state_ref_cell.borrow();
                let mut offchain_state = oc_state_opt.as_ref().unwrap().write();
                let mut responses = ref_cell.borrow_mut();
                assert!(
                    !responses.is_empty(),
                    "expected response to {}:\n{}",
                    url,
                    body
                );
                let response = responses.remove(0);
                offchain_state
                    .requests
                    .get_mut(&pending_request.id)
                    .unwrap()
                    .response = Some(response);
            });
        });
    }

    fn should_fail_send_signed_transaction() -> bool {
        SHOULD_FAIL_SEND_SIGNED_TRANSACTION.with(|x| *x.borrow())
    }
}

impl State {
    pub fn push_response_raw(&mut self, data: Vec<u8>) {
        self.responses.push(data);
    }

    pub fn push_response<T: Serialize>(&mut self, value: T) {
        let data = serde_json::to_vec(&json_rpc_response(value)).unwrap();
        self.push_response_raw(data);
    }

    pub fn run_next_offchain_with_params(
        &mut self,
        sidechain_height: u64,
        finalized_thischain_height: BlockNumber,
        dispatch_txs: bool,
    ) {
        let finalized_block_hash = H256([0; 32]);
        // Thischain finalized head.
        push_json_rpc_response(finalized_block_hash);
        let sub_block_number = frame_system::Pallet::<Runtime>::block_number();
        frame_system::Pallet::<Runtime>::set_block_number(sub_block_number + 1);
        // Thischain finalized header.
        push_json_rpc_response(SubstrateHeaderLimited {
            parent_hash: Default::default(),
            number: finalized_thischain_height.into(),
            state_root: Default::default(),
            extrinsics_root: Default::default(),
            digest: (),
        });
        // Thischain block.
        let from_block = self
            .storage_read::<BlockNumber>(STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY)
            .unwrap_or(finalized_thischain_height);
        let handle_count = if finalized_thischain_height < from_block {
            0
        } else {
            (finalized_thischain_height - from_block + 1)
                .min(SUBSTRATE_HANDLE_BLOCK_COUNT_PER_BLOCK as u64)
        };
        for _ in 0..handle_count {
            push_json_rpc_response(finalized_block_hash);
            push_json_rpc_response(SubstrateSignedBlockLimited {
                block: SubstrateBlockLimited {
                    header: SubstrateHeaderLimited {
                        parent_hash: Default::default(),
                        number: finalized_thischain_height.into(),
                        state_root: Default::default(),
                        extrinsics_root: Default::default(),
                        digest: (),
                    },
                    extrinsics: vec![],
                },
            });
        }
        // Sidechain height.
        push_json_rpc_response(U64::from(sidechain_height));

        let mut responses = Vec::new();
        std::mem::swap(&mut self.responses, &mut responses);
        for resp in responses {
            push_response(resp);
        }
        EthBridge::offchain();
        if dispatch_txs {
            self.dispatch_offchain_transactions();
        }
    }

    pub fn run_next_offchain_and_dispatch_txs(&mut self) {
        self.run_next_offchain_with_params(
            CONFIRMATION_INTERVAL,
            frame_system::Pallet::<Runtime>::block_number() + 1,
            true,
        );
    }

    fn dispatch_offchain_transactions(&self) {
        let mut txs = Vec::new();
        let mut guard = self.pool_state.write();
        std::mem::swap(&mut guard.transactions, &mut txs);
        for tx in txs {
            let e = TestExtrinsic::decode(&mut &*tx).unwrap();
            let (who, _) = e.signature.unwrap();
            let call = e.call;
            // In reality you would do `e.apply`, but this is a test. we assume we don't care
            // about validation etc.
            let origin = Some(who).into();
            // set_caller_from
            println!("{:?} {:?}", origin, call);
            if let Err(e) = call.dispatch_bypass_filter(origin) {
                eprintln!("call dispatch error {:?}", e);
            }
        }
    }

    pub fn storage_read_or_default<T: Decode + Default>(&self, key: &[u8]) -> T {
        self.storage_read(key).unwrap_or_default()
    }

    pub fn storage_read<T: Decode + Default>(&self, key: &[u8]) -> Option<T> {
        self.offchain_state
            .read()
            .persistent_storage
            .get(key)
            .and_then(|x| Decode::decode(&mut &x[..]).ok())
    }

    pub fn storage_remove(&self, key: &[u8]) {
        self.offchain_state
            .write()
            .persistent_storage
            .remove(b"", key);
    }

    pub fn pending_txs(&self) -> BTreeMap<H256, SignedTransactionData<Runtime>> {
        self.storage_read_or_default(STORAGE_PENDING_TRANSACTIONS_KEY)
    }

    pub fn failed_pending_txs(&self) -> BTreeMap<H256, SignedTransactionData<Runtime>> {
        self.storage_read_or_default(STORAGE_FAILED_PENDING_TRANSACTIONS_KEY)
    }

    pub fn substrate_to_handle_from_height(&self) -> BlockNumber {
        self.storage_read_or_default(STORAGE_SUB_TO_HANDLE_FROM_HEIGHT_KEY)
    }

    pub fn set_should_fail_send_signed_transactions(&self, flag: bool) {
        SHOULD_FAIL_SEND_SIGNED_TRANSACTION.with(|x| *x.borrow_mut() = flag);
    }
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
            Default::default(),
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

    pub fn add_currency(
        &mut self,
        network_id: u32,
        currency: AssetConfig<AssetId32<PredefinedAssetId>>,
    ) {
        self.networks
            .get_mut(&network_id)
            .unwrap()
            .config
            .assets
            .push(currency);
    }

    pub fn add_network(
        &mut self,
        assets: Vec<AssetConfig<AssetId32<PredefinedAssetId>>>,
        reserves: Option<Vec<(AssetId32<PredefinedAssetId>, Balance)>>,
        peers_num: Option<usize>,
        contract_address: H160,
    ) -> u32 {
        let net_id = self.last_network_id;
        let multisig_account_id = bridge_multisig::Pallet::<Runtime>::multi_account_id(
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
                    bridge_contract_address: contract_address,
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
            bridge_multisig::Pallet::<Runtime>::multi_account_id(&self.root_account_id, 1, 0);

        let mut bridge_accounts = Vec::new();
        let mut bridge_network_configs = Vec::new();
        let mut endowed_accounts: Vec<(_, AssetId32<PredefinedAssetId>, _)> = Vec::new();
        let network_ids: Vec<_> = self.networks.iter().map(|(id, _)| *id).collect();
        let mut networks: Vec<_> = self.networks.clone().into_iter().collect();
        networks.sort_by(|(x, _), (y, _)| x.cmp(y));
        let mut offchain_guard = offchain.0.write();
        let offchain_storage = &mut offchain_guard.persistent_storage;
        for (net_id, ext_network) in networks {
            let key = format!("{}-{:?}", STORAGE_ETH_NODE_PARAMS, net_id);
            offchain_storage.set(
                b"",
                key.as_bytes(),
                &NodeParams {
                    url: "http://eth.node".to_string(),
                    credentials: None,
                }
                .encode(),
            );
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
                ),
            ));
        }
        offchain_storage.set(b"", STORAGE_NETWORK_IDS_KEY, &network_ids.encode());
        offchain_storage.set(
            b"",
            STORAGE_SUB_NODE_URL_KEY,
            &String::from("http://sub.node").encode(),
        );
        let ocw_keys = &self.networks[&0].ocw_keypairs[0];
        offchain_storage.set(
            b"",
            STORAGE_PEER_SECRET_KEY,
            &Vec::from(ocw_keys.2).encode(),
        );
        drop(offchain_guard);
        let key_store = KeyStore::new();
        key_store
            .insert_unknown(
                crate::KEY_TYPE,
                &format!("0x{}", ocw_keys.2.to_hex::<String>()),
                ocw_keys.0.as_ref(),
            )
            .unwrap();

        // pallet_balances and orml_tokens no longer accept duplicate elements.
        let mut unique_endowed_accounts: Vec<(_, AssetId32<PredefinedAssetId>, _)> = Vec::new();
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
                    AssetSymbol(b"T".to_vec()),
                    AssetName(b"T".to_vec()),
                    DEFAULT_BALANCE_PRECISION,
                    Balance::from(0u32),
                    true,
                    None,
                    None,
                )
            })
            .collect();

        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<Runtime>()
            .unwrap();

        let mut balances: Vec<_> = endowed_accounts
            .iter()
            .map(|(acc, ..)| acc)
            .chain(vec![&self.root_account_id, &authority_account_id])
            .map(|x| (x.clone(), Balance::from(0u32)))
            .collect();
        balances.extend(bridge_accounts.iter().map(|(acc, _)| (acc.clone(), 0)));
        for (_net_id, ext_network) in &self.networks {
            balances.extend(ext_network.ocw_keypairs.iter().map(|x| (x.1.clone(), 0)));
        }
        balances.sort_by_key(|x| x.0.clone());
        balances.dedup_by_key(|x| x.0.clone());
        BalancesConfig { balances }
            .assimilate_storage(&mut storage)
            .unwrap();

        if !endowed_accounts.is_empty() {
            SudoConfig {
                key: Some(endowed_accounts[0].0.clone()),
            }
            .assimilate_storage(&mut storage)
            .unwrap();
        }

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
            balances: endowed_accounts.clone(),
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
            authority_account: Some(authority_account_id.clone()),
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
        t.register_extension(OffchainDbExt::new(offchain.clone()));
        t.register_extension(OffchainWorkerExt::new(offchain));
        t.register_extension(TransactionPoolExt::new(pool));
        t.register_extension(KeystoreExt(Arc::new(key_store)));
        t.execute_with(|| System::set_block_number(1));

        let state = State {
            networks: self.networks,
            authority_account_id,
            pool_state,
            offchain_state,
            responses: vec![],
        };
        OFFCHAIN_STATE.with(|x| *x.borrow_mut() = Some(state.offchain_state.clone()));
        state.set_should_fail_send_signed_transactions(false);
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
