#![allow(deprecated, dead_code, unused_imports)]

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

use currencies::BasicCurrencyAdapter;

// Mock runtime
use bridge_types::traits::TimepointProvider;
use bridge_types::traits::{AppRegistry, BalancePrecisionConverter};
use bridge_types::types::{AssetKind, CallOriginOutput, GenericAdditionalInboundData, MessageId};
use bridge_types::H256;
use bridge_types::{EVMChainId, GenericAccount, U256};
use bridge_types::{GenericNetworkId, H160};
use common::mock::ExistentialDeposits;
use common::{
    balance, mock_assets_config, mock_bridge_channel_outbound_config, mock_common_config,
    mock_currencies_config, mock_dispatch_config, mock_evm_fungible_app_config,
    mock_frame_system_config, mock_pallet_balances_config, mock_pallet_timestamp_config,
    mock_permissions_config, mock_technical_config, mock_tokens_config, Amount, AssetId32,
    AssetName, AssetSymbol, Balance, DEXId, FromGenericPair, PredefinedAssetId, DAI, ETH, XOR, XST,
};
use frame_support::parameter_types;
use frame_support::traits::ConstU32;
use frame_support::weights::Weight;
use sp_keyring::sr25519::Keyring;
use sp_runtime::traits::{Convert, IdentifyAccount, Verify};
use sp_runtime::BuildStorage;
use sp_runtime::{DispatchResult, MultiSignature};
use std::cell::RefCell;

use crate as proxy;

pub type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
pub type Block = frame_system::mocking::MockBlock<Test>;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub type BlockNumber = u64;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage},
        Assets: assets::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Dispatch: dispatch::{Pallet, Call, Storage, Origin<T>, Event<T>},
        BridgeOutboundChannel: bridge_channel::outbound::{Pallet, Config<T>, Storage, Event<T>},
        FungibleApp: evm_fungible_app::{Pallet, Call, Config<T>, Storage, Event<T>},
        BridgeProxy: proxy::{Pallet, Call, Storage, Event},
    }
);

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type OriginOutput = CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>;
pub const BASE_EVM_NETWORK_ID: EVMChainId = EVMChainId::zero();

mock_assets_config!(Test);
mock_bridge_channel_outbound_config!(Test);
mock_common_config!(Test);
mock_currencies_config!(Test);
mock_evm_fungible_app_config!(Test);
mock_frame_system_config!(Test, (), ConstU32<65536>);
mock_pallet_balances_config!(Test);
mock_pallet_timestamp_config!(Test);
mock_permissions_config!(Test);
mock_technical_config!(Test);
mock_tokens_config!(Test);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

parameter_types! {
    pub const MaxMessagePayloadSize: u32 = 2048;
    pub const MaxMessagesPerCommit: u32 = 3;
    pub const Decimals: u32 = 12;
}
pub struct FeeConverter;

impl Convert<U256, Balance> for FeeConverter {
    fn convert(amount: U256) -> Balance {
        common::eth::unwrap_balance(amount, Decimals::get())
            .expect("Should not panic unless runtime is misconfigured")
    }
}

parameter_types! {
    pub const FeeCurrency: AssetId32<PredefinedAssetId> = XOR;
}

parameter_types! {
    pub GetTrustlessBridgeTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_MAIN.to_vec(),
        )
    };
    pub GetTrustlessBridgeAccountId: AccountId = {
        let tech_account_id = GetTrustlessBridgeTechAccountId::get();
        technical::Pallet::<Test>::tech_account_id_to_account_id(&tech_account_id)
            .expect("Failed to get ordinary account id for technical account id.")
    };
    pub GetTrustlessBridgeFeesTechAccountId: TechAccountId = {
        TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_FEES.to_vec(),
        )
    };
    pub GetTrustlessBridgeFeesAccountId: AccountId = {
        let tech_account_id = GetTrustlessBridgeFeesTechAccountId::get();
        technical::Pallet::<Test>::tech_account_id_to_account_id(&tech_account_id)
            .expect("Failed to get ordinary account id for technical account id.")
    };
}

pub struct AppRegistryImpl;

impl AppRegistry<EVMChainId, H160> for AppRegistryImpl {
    fn register_app(_network_id: EVMChainId, _app: H160) -> DispatchResult {
        Ok(())
    }

    fn deregister_app(_network_id: EVMChainId, _app: H160) -> DispatchResult {
        Ok(())
    }
}

pub struct BalancePrecisionConverterImpl;

impl BalancePrecisionConverter<AssetId, Balance, U256> for BalancePrecisionConverterImpl {
    fn from_sidechain(
        _asset_id: &AssetId,
        _sidechain_precision: u8,
        amount: U256,
    ) -> Option<(Balance, U256)> {
        amount.try_into().ok().map(|x| (x, amount))
    }

    fn to_sidechain(
        _asset_id: &AssetId,
        _sidechain_precision: u8,
        amount: Balance,
    ) -> Option<(Balance, U256)> {
        Some((amount, amount.into()))
    }
}

impl dispatch::Config for Test {
    type Call = RuntimeCall;
    type CallFilter = frame_support::traits::Everything;
    type Hashing = sp_runtime::traits::Keccak256;
    type MessageId = MessageId;
    type Origin = RuntimeOrigin;
    type OriginOutput = OriginOutput;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = ();
}

pub struct GenericTimepointProvider;

impl TimepointProvider for GenericTimepointProvider {
    fn get_timepoint() -> bridge_types::GenericTimepoint {
        bridge_types::GenericTimepoint::Sora(System::block_number() as u32)
    }
}

pub struct ReferencePriceProvider;

impl common::ReferencePriceProvider<AssetId, Balance> for ReferencePriceProvider {
    fn get_reference_price(_asset_id: &AssetId) -> Result<Balance, sp_runtime::DispatchError> {
        Ok(common::balance!(2.5))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MockBridgeCall {
    HashiTransfer,
    HashiRefund,
    ParachainTransfer,
    ParachainRefund,
    LiberlandTransfer,
    LiberlandRefund,
}

#[derive(Default)]
struct MockBridgeState {
    hashi_supported: Vec<(GenericNetworkId, AssetId)>,
    parachain_supported: Vec<(GenericNetworkId, AssetId)>,
    liberland_supported: Vec<(GenericNetworkId, AssetId)>,
    calls: Vec<MockBridgeCall>,
}

thread_local! {
    static MOCK_BRIDGE_STATE: RefCell<MockBridgeState> =
        RefCell::new(MockBridgeState::default());
}

fn set_supported_asset(
    routes: &mut Vec<(GenericNetworkId, AssetId)>,
    network_id: GenericNetworkId,
    asset_id: AssetId,
    supported: bool,
) {
    if supported {
        if !routes.contains(&(network_id, asset_id)) {
            routes.push((network_id, asset_id));
        }
    } else {
        routes.retain(|route| route != &(network_id, asset_id));
    }
}

fn is_supported_asset(
    routes: &[(GenericNetworkId, AssetId)],
    network_id: GenericNetworkId,
    asset_id: AssetId,
) -> bool {
    routes.contains(&(network_id, asset_id))
}

fn record_call(call: MockBridgeCall) {
    MOCK_BRIDGE_STATE.with(|state| state.borrow_mut().calls.push(call));
}

pub fn reset_mock_bridge_state() {
    MOCK_BRIDGE_STATE.with(|state| *state.borrow_mut() = MockBridgeState::default());
}

pub fn set_hashi_supported(network_id: GenericNetworkId, asset_id: AssetId, supported: bool) {
    MOCK_BRIDGE_STATE.with(|state| {
        set_supported_asset(
            &mut state.borrow_mut().hashi_supported,
            network_id,
            asset_id,
            supported,
        )
    });
}

pub fn set_parachain_supported(network_id: GenericNetworkId, asset_id: AssetId, supported: bool) {
    MOCK_BRIDGE_STATE.with(|state| {
        set_supported_asset(
            &mut state.borrow_mut().parachain_supported,
            network_id,
            asset_id,
            supported,
        )
    });
}

pub fn set_liberland_supported(network_id: GenericNetworkId, asset_id: AssetId, supported: bool) {
    MOCK_BRIDGE_STATE.with(|state| {
        set_supported_asset(
            &mut state.borrow_mut().liberland_supported,
            network_id,
            asset_id,
            supported,
        )
    });
}

pub fn mock_bridge_calls() -> Vec<MockBridgeCall> {
    MOCK_BRIDGE_STATE.with(|state| state.borrow().calls.clone())
}

pub struct HashiBridgeMock;

impl bridge_types::traits::BridgeApp<AccountId, H160, AssetId, Balance> for HashiBridgeMock {
    fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetId) -> bool {
        MOCK_BRIDGE_STATE
            .with(|state| is_supported_asset(&state.borrow().hashi_supported, network_id, asset_id))
    }

    fn transfer(
        _network_id: GenericNetworkId,
        _asset_id: AssetId,
        _sender: AccountId,
        _recipient: H160,
        _amount: Balance,
    ) -> Result<H256, sp_runtime::DispatchError> {
        record_call(MockBridgeCall::HashiTransfer);
        Ok(H256::repeat_byte(1))
    }

    fn refund(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _recipient: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
    ) -> DispatchResult {
        record_call(MockBridgeCall::HashiRefund);
        Ok(())
    }

    fn list_supported_assets(
        _network_id: GenericNetworkId,
    ) -> Vec<bridge_types::types::BridgeAssetInfo> {
        vec![]
    }

    fn list_apps() -> Vec<bridge_types::types::BridgeAppInfo> {
        vec![]
    }

    fn transfer_weight() -> Weight {
        Weight::zero()
    }

    fn refund_weight() -> Weight {
        Weight::zero()
    }

    fn is_asset_supported_weight() -> Weight {
        Weight::zero()
    }
}

pub struct ParachainAppMock;

impl
    bridge_types::traits::BridgeApp<
        AccountId,
        bridge_types::substrate::ParachainAccountId,
        AssetId,
        Balance,
    > for ParachainAppMock
{
    fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetId) -> bool {
        MOCK_BRIDGE_STATE.with(|state| {
            is_supported_asset(&state.borrow().parachain_supported, network_id, asset_id)
        })
    }

    fn transfer(
        _network_id: GenericNetworkId,
        _asset_id: AssetId,
        _sender: AccountId,
        _recipient: bridge_types::substrate::ParachainAccountId,
        _amount: Balance,
    ) -> Result<H256, sp_runtime::DispatchError> {
        record_call(MockBridgeCall::ParachainTransfer);
        Ok(H256::repeat_byte(2))
    }

    fn refund(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _recipient: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
    ) -> DispatchResult {
        record_call(MockBridgeCall::ParachainRefund);
        Ok(())
    }

    fn list_supported_assets(
        _network_id: GenericNetworkId,
    ) -> Vec<bridge_types::types::BridgeAssetInfo> {
        vec![]
    }

    fn list_apps() -> Vec<bridge_types::types::BridgeAppInfo> {
        vec![]
    }

    fn transfer_weight() -> Weight {
        Weight::zero()
    }

    fn refund_weight() -> Weight {
        Weight::zero()
    }

    fn is_asset_supported_weight() -> Weight {
        Weight::zero()
    }
}

pub struct LiberlandAppMock;

impl bridge_types::traits::BridgeApp<AccountId, GenericAccount, AssetId, Balance>
    for LiberlandAppMock
{
    fn is_asset_supported(network_id: GenericNetworkId, asset_id: AssetId) -> bool {
        MOCK_BRIDGE_STATE.with(|state| {
            is_supported_asset(&state.borrow().liberland_supported, network_id, asset_id)
        })
    }

    fn transfer(
        _network_id: GenericNetworkId,
        _asset_id: AssetId,
        _sender: AccountId,
        _recipient: GenericAccount,
        _amount: Balance,
    ) -> Result<H256, sp_runtime::DispatchError> {
        record_call(MockBridgeCall::LiberlandTransfer);
        Ok(H256::repeat_byte(3))
    }

    fn refund(
        _network_id: GenericNetworkId,
        _message_id: H256,
        _recipient: AccountId,
        _asset_id: AssetId,
        _amount: Balance,
    ) -> DispatchResult {
        record_call(MockBridgeCall::LiberlandRefund);
        Ok(())
    }

    fn list_supported_assets(
        _network_id: GenericNetworkId,
    ) -> Vec<bridge_types::types::BridgeAssetInfo> {
        vec![]
    }

    fn list_apps() -> Vec<bridge_types::types::BridgeAppInfo> {
        vec![]
    }

    fn transfer_weight() -> Weight {
        Weight::zero()
    }

    fn refund_weight() -> Weight {
        Weight::zero()
    }

    fn is_asset_supported_weight() -> Weight {
        Weight::zero()
    }
}

impl proxy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ManagerOrigin = frame_system::EnsureRoot<AccountId>;
    type AccountIdConverter = sp_runtime::traits::Identity;
    type FAApp = FungibleApp;
    type HashiBridge = HashiBridgeMock;
    type LiberlandApp = LiberlandAppMock;
    type ParachainApp = ParachainAppMock;
    type ReferencePriceProvider = ReferencePriceProvider;
    type TimepointProvider = GenericTimepointProvider;
    type WeightInfo = ();
}

pub fn new_tester() -> sp_io::TestExternalities {
    reset_mock_bridge_state();
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    technical::GenesisConfig::<Test> {
        register_tech_accounts: vec![
            (
                GetTrustlessBridgeAccountId::get(),
                GetTrustlessBridgeTechAccountId::get(),
            ),
            (
                GetTrustlessBridgeFeesAccountId::get(),
                GetTrustlessBridgeFeesTechAccountId::get(),
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    evm_fungible_app::GenesisConfig::<Test> {
        apps: vec![(BASE_EVM_NETWORK_ID, H160::repeat_byte(1))],
        assets: vec![
            (
                BASE_EVM_NETWORK_ID,
                XOR,
                H160::repeat_byte(3),
                AssetKind::Thischain,
                18,
            ),
            (
                BASE_EVM_NETWORK_ID,
                DAI,
                H160::repeat_byte(4),
                AssetKind::Sidechain,
                18,
            ),
            (
                BASE_EVM_NETWORK_ID,
                ETH,
                H160::repeat_byte(0),
                AssetKind::Sidechain,
                18,
            ),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let bob: AccountId = Keyring::Bob.into();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(bob.clone(), balance!(1))],
        dev_accounts: Default::default(),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    assets::GenesisConfig::<Test> {
        endowed_assets: vec![
            (
                XOR,
                bob.clone(),
                AssetSymbol(b"XOR".to_vec()),
                AssetName(b"SORA".to_vec()),
                18,
                0,
                true,
                None,
                None,
            ),
            (
                DAI,
                bob.clone(),
                AssetSymbol(b"DAI".to_vec()),
                AssetName(b"DAI".to_vec()),
                18,
                0,
                true,
                None,
                None,
            ),
            (
                ETH,
                bob.clone(),
                AssetSymbol(b"ETH".to_vec()),
                AssetName(b"Ether".to_vec()),
                18,
                0,
                true,
                None,
                None,
            ),
        ],
        regulated_assets: Default::default(),
        sbt_assets: Default::default(),
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}
