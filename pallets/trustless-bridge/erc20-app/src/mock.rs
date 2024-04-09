use bridge_types::traits::{AppRegistry, BalancePrecisionConverter, BridgeAssetRegistry};
use currencies::BasicCurrencyAdapter;

// Mock runtime
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::AssetKind;
use bridge_types::H160;
use bridge_types::H256;
use bridge_types::{EVMChainId, GenericNetworkId, U256};
use common::mock::ExistentialDeposits;
use common::{
    balance, Amount, AssetId32, AssetName, AssetSymbol, Balance, DEXId, FromGenericPair,
    PredefinedAssetId, DAI, ETH, PSWAP, VAL, XOR, XST,
};
use frame_support::dispatch::DispatchResult;
use frame_support::parameter_types;
use frame_support::traits::{Everything, GenesisBuild};
use frame_system as system;
use hex_literal::hex;
use sp_keyring::sr25519::Keyring;
use sp_runtime::testing::Header;
use sp_runtime::traits::{
    BlakeTwo256, Convert, IdentifyAccount, IdentityLookup, Keccak256, Verify,
};
use sp_runtime::{DispatchError, MultiSignature};

use crate as erc20_app;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AssetId = AssetId32<common::PredefinedAssetId>;

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

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Dispatch: dispatch::{Pallet, Call, Storage, Origin<T>, Event<T>},
        BridgeOutboundChannel: bridge_outbound_channel::{Pallet, Config<T>, Storage, Event<T>},
        Erc20App: erc20_app::{Pallet, Call, Config<T>, Storage, Event<T>},
    }
);

pub type Signature = MultiSignature;

pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub const BASE_NETWORK_ID: EVMChainId = EVMChainId::zero();

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl system::Config for Test {
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

impl common::Config for Test {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
}

impl permissions::Config for Test {
    type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
}

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type RuntimeEvent = RuntimeEvent;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
}

impl tokens::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type CurrencyHooks = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type DustRemovalWhitelist = Everything;
}

impl currencies::Config for Test {
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u64>;
    type GetNativeCurrencyId = <Test as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}
parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
}

parameter_types! {
    pub const GetBuyBackAssetId: AssetId = XST;
    pub GetBuyBackSupplyAssets: Vec<AssetId> = vec![VAL, PSWAP];
    pub const GetBuyBackPercentage: u8 = 10;
    pub const GetBuyBackAccountId: AccountId = AccountId::new(hex!(
            "0000000000000000000000000000000000000000000000000000000000000023"
    ));
    pub const GetBuyBackDexId: DEXId = DEXId::Polkaswap;
}

impl assets::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type GetBuyBackAssetId = GetBuyBackAssetId;
    type GetBuyBackSupplyAssets = GetBuyBackSupplyAssets;
    type GetBuyBackPercentage = GetBuyBackPercentage;
    type GetBuyBackAccountId = GetBuyBackAccountId;
    type GetBuyBackDexId = GetBuyBackDexId;
    type BuyBackLiquidityProxy = ();
    type Currency = currencies::Pallet<Test>;
    type WeightInfo = ();
    type GetTotalBalance = ();
}

impl dispatch::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OriginOutput =
        bridge_types::types::CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>;
    type Origin = RuntimeOrigin;
    type MessageId = u64;
    type Hashing = Keccak256;
    type Call = RuntimeCall;
    type CallFilter = Everything;
    type WeightInfo = ();
}

parameter_types! {
    pub const MaxMessagePayloadSize: u32 = 2048;
    pub const MaxMessagesPerCommit: u32 = 3;
    pub const MaxTotalGasLimit: u64 = 5_000_000;
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
    pub const ThisNetworkId: bridge_types::GenericNetworkId = bridge_types::GenericNetworkId::Sub(bridge_types::SubNetworkId::Mainnet);
}

impl bridge_outbound_channel::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type MaxTotalGasLimit = MaxTotalGasLimit;
    type FeeTechAccountId = GetTrustlessBridgeFeesTechAccountId;
    type FeeCurrency = FeeCurrency;
    type MessageStatusNotifier = ();
    type AuxiliaryDigestHandler = ();
    type ThisNetworkId = ThisNetworkId;
    type WeightInfo = ();
}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

impl technical::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
    type AssetInfoProvider = assets::Pallet<Test>;
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

pub struct BridgeAssetRegistryImpl;

impl BridgeAssetRegistry<AccountId, AssetId> for BridgeAssetRegistryImpl {
    type AssetName = common::AssetName;
    type AssetSymbol = common::AssetSymbol;

    fn register_asset(
        network_id: GenericNetworkId,
        name: Self::AssetName,
        symbol: Self::AssetSymbol,
    ) -> Result<AssetId, DispatchError> {
        let owner =
            bridge_types::test_utils::BridgeAssetLockerImpl::<()>::bridge_account(network_id);
        frame_system::Pallet::<Test>::inc_providers(&owner);
        let asset_id = Assets::register_from(&owner, symbol, name, 18, 0, true, None, None)?;
        Ok(asset_id)
    }

    fn manage_asset(
        network_id: GenericNetworkId,
        asset_id: AssetId,
    ) -> frame_support::pallet_prelude::DispatchResult {
        let manager =
            bridge_types::test_utils::BridgeAssetLockerImpl::<()>::bridge_account(network_id);
        frame_system::Pallet::<Test>::inc_providers(&manager);
        let scope = permissions::Scope::Limited(common::hash(&asset_id));
        for permission_id in [permissions::BURN, permissions::MINT] {
            if permissions::Pallet::<Test>::check_permission_with_scope(
                manager.clone(),
                permission_id,
                &scope,
            )
            .is_err()
            {
                permissions::Pallet::<Test>::assign_permission(
                    manager.clone(),
                    &manager,
                    permission_id,
                    scope,
                )?;
            }
        }
        Ok(())
    }

    fn get_raw_info(_asset_id: AssetId) -> bridge_types::types::RawAssetInfo {
        bridge_types::types::RawAssetInfo {
            name: Default::default(),
            symbol: Default::default(),
            precision: 18,
        }
    }

    fn ensure_asset_exists(_: AssetId) -> bool {
        true
    }
}

impl erc20_app::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type OutboundChannel = BridgeOutboundChannel;
    type CallOrigin = dispatch::EnsureAccount<
        bridge_types::types::CallOriginOutput<EVMChainId, H256, AdditionalEVMInboundData>,
    >;
    type WeightInfo = ();
    type MessageStatusNotifier = ();
    type BalancePrecisionConverter = BalancePrecisionConverterImpl;
    type AppRegistry = AppRegistryImpl;
    type AssetRegistry = BridgeAssetRegistryImpl;
    type AssetIdConverter = sp_runtime::traits::ConvertInto;
    type BridgeAssetLocker = bridge_types::test_utils::BridgeAssetLockerImpl<Currencies>;
}

pub fn new_tester() -> sp_io::TestExternalities {
    let mut storage = system::GenesisConfig::default()
        .build_storage::<Test>()
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
                bob,
                AssetSymbol(b"ETH".to_vec()),
                AssetName(b"ETH".to_vec()),
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

    GenesisBuild::<Test>::assimilate_storage(
        &bridge_outbound_channel::GenesisConfig {
            fee: 10000,
            interval: 10,
        },
        &mut storage,
    )
    .unwrap();

    GenesisBuild::<Test>::assimilate_storage(
        &erc20_app::GenesisConfig {
            apps: vec![
                (BASE_NETWORK_ID, H160::repeat_byte(1), AssetKind::Sidechain),
                (BASE_NETWORK_ID, H160::repeat_byte(2), AssetKind::Thischain),
            ],
            assets: vec![
                (
                    BASE_NETWORK_ID,
                    XOR,
                    H160::repeat_byte(3),
                    AssetKind::Thischain,
                    18,
                ),
                (
                    BASE_NETWORK_ID,
                    DAI,
                    H160::repeat_byte(4),
                    AssetKind::Sidechain,
                    18,
                ),
            ],
        },
        &mut storage,
    )
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}
