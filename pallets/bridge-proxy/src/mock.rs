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
use bridge_types::{EVMChainId, U256};
use bridge_types::{GenericNetworkId, H160};
use common::mock::ExistentialDeposits;
use common::{
    balance, mock_assets_config, mock_common_config, mock_currencies_config,
    mock_pallet_balances_config, mock_pallet_timestamp_config, mock_permissions_config,
    mock_technical_config, mock_tokens_config, Amount, AssetId32, AssetName, AssetSymbol, Balance,
    DEXId, FromGenericPair, PredefinedAssetId, DAI, ETH, XOR, XST,
};
use frame_support::parameter_types;
use frame_support::traits::Everything;
use frame_system as system;
use sp_core::{ConstU128, ConstU64};
use sp_keyring::sr25519::Keyring;
use sp_runtime::traits::{
    BlakeTwo256, Convert, IdentifyAccount, IdentityLookup, Keccak256, Verify,
};
use sp_runtime::BuildStorage;
use sp_runtime::{AccountId32, DispatchResult, MultiSignature};
use system::EnsureRoot;

use crate as proxy;

pub type Block = frame_system::mocking::MockBlock<Test>;
pub type AssetId = AssetId32<common::PredefinedAssetId>;
pub type BlockNumber = u64;
type Moment = u64;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
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

pub const BASE_EVM_NETWORK_ID: EVMChainId = EVMChainId::zero();

mock_pallet_balances_config!(Test);
mock_technical_config!(Test);
mock_currencies_config!(Test);
mock_common_config!(Test);
mock_tokens_config!(Test);
mock_assets_config!(Test);
mock_pallet_timestamp_config!(Test);
mock_permissions_config!(Test);

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl system::Config for Test {
    type BaseCallFilter = Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    type Block = Block;
    type Nonce = u64;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
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

parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub const GetBuyBackAssetId: AssetId = XST;
}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

impl dispatch::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OriginOutput = CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>;
    type Origin = RuntimeOrigin;
    type MessageId = MessageId;
    type Hashing = Keccak256;
    type Call = RuntimeCall;
    type CallFilter = Everything;
    type WeightInfo = ();
}

parameter_types! {
    pub const MaxMessagePayloadSize: u32 = 2048;
    pub const MaxMessagesPerCommit: u32 = 3;
    pub const Decimals: u32 = 12;
}

#[allow(dead_code)]
pub struct FeeConverter;
impl Convert<U256, Balance> for FeeConverter {
    fn convert(amount: U256) -> Balance {
        common::eth::unwrap_balance(amount, Decimals::get())
            .expect("Should not panic unless runtime is misconfigured")
    }
}

parameter_types! {
    pub const FeeCurrency: AssetId32<PredefinedAssetId> = XOR;
    pub const MaxTotalGasLimit: u64 = 5_000_000;
    pub const ThisNetworkId: bridge_types::GenericNetworkId = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
}

impl bridge_channel::outbound::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type MessageStatusNotifier = BridgeProxy;
    type AuxiliaryDigestHandler = ();
    type ThisNetworkId = ThisNetworkId;
    type AssetId = AssetId;
    type Balance = Balance;
    type MaxGasPerCommit = MaxTotalGasLimit;
    type MaxGasPerMessage = MaxTotalGasLimit;
    type TimepointProvider = GenericTimepointProvider;
    type WeightInfo = ();
}

parameter_types! {
    pub GetTrustlessBridgeTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetTrustlessBridgeAccountId: AccountId = {
        let tech_account_id = GetTrustlessBridgeTechAccountId::get();
        let account_id =
            technical::Pallet::<Test>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
    pub GetTrustlessBridgeFeesTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_FEES.to_vec(),
        );
        tech_account_id
    };
    pub GetTrustlessBridgeFeesAccountId: AccountId = {
        let tech_account_id = GetTrustlessBridgeFeesTechAccountId::get();
        let account_id =
            technical::Pallet::<Test>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
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

impl evm_fungible_app::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = BridgeOutboundChannel;
    type CallOrigin = dispatch::EnsureAccount<
        bridge_types::types::CallOriginOutput<GenericNetworkId, H256, GenericAdditionalInboundData>,
    >;
    type MessageStatusNotifier = BridgeProxy;
    type AppRegistry = AppRegistryImpl;
    type AssetRegistry = BridgeProxy;
    type AssetIdConverter = sp_runtime::traits::ConvertInto;
    type BalancePrecisionConverter = BalancePrecisionConverterImpl;
    type BridgeAssetLocker = BridgeProxy;
    type BaseFeeLifetime = ConstU64<100>;
    type PriorityFee = ConstU128<100>;
    type WeightInfo = ();
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

impl proxy::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type FAApp = FungibleApp;
    type ParachainApp = ();
    type HashiBridge = ();
    type LiberlandApp = ();
    type TimepointProvider = GenericTimepointProvider;
    type ReferencePriceProvider = ReferencePriceProvider;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type WeightInfo = ();
    type AccountIdConverter = sp_runtime::traits::Identity;
}

pub fn new_tester() -> sp_io::TestExternalities {
    let mut storage = system::GenesisConfig::<Test>::default()
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

    let _ = &evm_fungible_app::GenesisConfig::<Test> {
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
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    assets::GenesisConfig::<Test> {
        endowed_assets: vec![
            (
                XOR.into(),
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
                DAI.into(),
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
                ETH.into(),
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
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}
