use currencies::BasicCurrencyAdapter;
use sp_std::marker::PhantomData;

// Mock runtime
use bridge_types::traits::OutboundRouter;
use bridge_types::types::ChannelId;
use common::mock::ExistentialDeposits;
use common::{balance, Amount, AssetId32, AssetName, AssetSymbol, Balance, DEXId, XOR};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::parameter_types;
use frame_support::traits::{Everything, GenesisBuild};
use frame_system as system;
use sp_core::{H160, H256};
use sp_keyring::sr25519::Keyring;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify};
use sp_runtime::MultiSignature;

use crate as eth_app;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
type AssetId = AssetId32<common::PredefinedAssetId>;

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        Dispatch: dispatch::{Pallet, Call, Storage, Origin, Event<T>},
        EthApp: eth_app::{Pallet, Call, Config<T>, Storage, Event<T>},
    }
);

pub type Signature = MultiSignature;

pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl system::Config for Test {
    type BaseCallFilter = Everything;
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
    type Event = Event;
}

parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
}

impl pallet_balances::Config for Test {
    type Balance = Balance;
    type Event = Event;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
    type MaxReserves = ();
    type ReserveIdentifier = ();
}

impl tokens::Config for Test {
    type Event = Event;
    type Balance = Balance;
    type Amount = Amount;
    type CurrencyId = <Test as assets::Config>::AssetId;
    type WeightInfo = ();
    type ExistentialDeposits = ExistentialDeposits;
    type OnDust = ();
    type MaxLocks = ();
    type DustRemovalWhitelist = Everything;
}

impl currencies::Config for Test {
    type Event = Event;
    type MultiCurrency = Tokens;
    type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u64>;
    type GetNativeCurrencyId = <Test as assets::Config>::GetBaseAssetId;
    type WeightInfo = ();
}
parameter_types! {
    pub const GetBaseAssetId: AssetId = XOR;
    pub GetTeamReservesAccountId: AccountId = Default::default();
}

impl assets::Config for Test {
    type Event = Event;
    type ExtraAccountId = [u8; 32];
    type ExtraAssetRecordArg =
        common::AssetIdExtraAssetRecordArg<DEXId, common::LiquiditySourceType, [u8; 32]>;
    type AssetId = AssetId;
    type GetBaseAssetId = GetBaseAssetId;
    type Currency = currencies::Pallet<Test>;
    type GetTeamReservesAccountId = GetTeamReservesAccountId;
    type WeightInfo = ();
    type GetTotalBalance = ();
}

impl dispatch::Config for Test {
    type Origin = Origin;
    type Event = Event;
    type MessageId = u64;
    type Call = Call;
    type CallFilter = Everything;
}

pub struct MockOutboundRouter<AccountId>(PhantomData<AccountId>);

impl<AccountId> OutboundRouter<AccountId> for MockOutboundRouter<AccountId> {
    fn submit(channel: ChannelId, _: &AccountId, _: H160, _: &[u8]) -> DispatchResult {
        if channel == ChannelId::Basic {
            return Err(DispatchError::Other("some error!"));
        }
        Ok(())
    }
}

impl eth_app::Config for Test {
    type Event = Event;
    type OutboundRouter = MockOutboundRouter<Self::AccountId>;
    type CallOrigin = dispatch::EnsureEthereumAccount;
    type FeeCurrency = ();
    type WeightInfo = ();
}

pub fn new_tester() -> sp_io::TestExternalities {
    let mut storage = system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    let bob: AccountId = Keyring::Bob.into();
    GenesisBuild::<Test>::assimilate_storage(
        &eth_app::GenesisConfig {
            address: H160::repeat_byte(1),
            dest_account: Some(bob.clone()),
        },
        &mut storage,
    )
    .unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(bob.clone(), balance!(1))],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    assets::GenesisConfig::<Test> {
        endowed_assets: vec![(
            XOR.into(),
            bob,
            AssetSymbol(b"XOR".to_vec()),
            AssetName(b"SORA".to_vec()),
            18,
            0,
            true,
            None,
            None,
        )],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}
