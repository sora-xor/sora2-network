use super::*;
use currencies::BasicCurrencyAdapter;

use frame_support::dispatch::DispatchError;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::{assert_err, assert_ok, parameter_types};
use frame_system::RawOrigin;
use sp_core::{H160, H256};
use sp_keyring::AccountKeyring as Keyring;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, Convert, IdentifyAccount, IdentityLookup, Verify};
use sp_runtime::{AccountId32, MultiSignature, Perbill};
use sp_std::convert::From;
use sp_std::marker::PhantomData;

use bridge_types::traits::{AppRegistry, MessageDispatch, OutboundRouter};
use bridge_types::types::{Message, Proof};
use bridge_types::{Log, U256};

use common::mock::ExistentialDeposits;
use common::{balance, Amount, AssetId32, AssetName, AssetSymbol, DEXId, FromGenericPair, XOR};
use hex_literal::hex;

use crate::inbound::Error;

use crate::inbound as bridge_inbound_channel;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

const BASE_NETWORK_ID: EthNetworkId = EthNetworkId::zero();

frame_support::construct_runtime!(
    pub enum Test where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Storage, Event<T>},
        Balances: pallet_balances::{Pallet, Call, Storage, Event<T>},
        Assets: assets::{Pallet, Call, Storage, Event<T>},
        Tokens: tokens::{Pallet, Call, Config<T>, Storage, Event<T>},
        Currencies: currencies::{Pallet, Call, Storage},
        Technical: technical::{Pallet, Call, Config<T>, Event<T>},
        Permissions: permissions::{Pallet, Call, Config<T>, Storage, Event<T>},
        BridgeInboundChannel: bridge_inbound_channel::{Pallet, Call, Storage, Event<T>},
    }
);

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Balance = u128;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
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

parameter_types! {
    pub const ExistentialDeposit: u128 = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
    /// The ubiquitous event type.
    type Event = Event;
    type MaxLocks = MaxLocks;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxReserves = MaxReserves;
    type ReserveIdentifier = ();
}

impl common::Config for Test {
    type DEXId = common::DEXId;
    type LstId = common::LiquiditySourceType;
}

impl permissions::Config for Test {
    type Event = Event;
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
    type MaxReserves = ();
    type ReserveIdentifier = ();
    type OnNewTokenAccount = ();
    type OnKilledTokenAccount = ();
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
    pub GetTeamReservesAccountId: AccountId = AccountId32::from([0; 32]);
}

type AssetId = AssetId32<common::PredefinedAssetId>;

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

// Mock verifier
pub struct MockVerifier;

impl Verifier for MockVerifier {
    fn verify(_: EthNetworkId, message: &Message) -> Result<Log, DispatchError> {
        let log: Log = rlp::decode(&message.data).unwrap();
        Ok(log)
    }

    fn initialize_storage(
        _network_id: EthNetworkId,
        _headers: Vec<bridge_types::Header>,
        _difficulty: u128,
        _descendants_until_final: u8,
    ) -> Result<(), &'static str> {
        Ok(())
    }
}

// Mock Dispatch
pub struct MockMessageDispatch;

impl MessageDispatch<Test, MessageId> for MockMessageDispatch {
    fn dispatch(_: EthNetworkId, _: H160, _: MessageId, _: &[u8]) {}

    #[cfg(feature = "runtime-benchmarks")]
    fn successful_dispatch_event(_: MessageId) -> Option<<Test as frame_system::Config>::Event> {
        None
    }
}

parameter_types! {
    pub SourceAccount: AccountId = Keyring::Eve.into();
    pub TreasuryAccount: AccountId = Keyring::Dave.into();
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
    pub GetTreasuryTechAccountId: TechAccountId = {
        let tech_account_id = TechAccountId::from_generic_pair(
            bridge_types::types::TECH_ACCOUNT_TREASURY_PREFIX.to_vec(),
            bridge_types::types::TECH_ACCOUNT_MAIN.to_vec(),
        );
        tech_account_id
    };
    pub GetTreasuryAccountId: AccountId = {
        let tech_account_id = GetTreasuryTechAccountId::get();
        let account_id =
            technical::Pallet::<Test>::tech_account_id_to_account_id(&tech_account_id)
                .expect("Failed to get ordinary account id for technical account id.");
        account_id
    };
}

pub struct FeeConverter<T: Config>(PhantomData<T>);

impl<T: Config> Convert<U256, BalanceOf<T>> for FeeConverter<T> {
    fn convert(_: U256) -> BalanceOf<T> {
        100u32.into()
    }
}

impl bridge_inbound_channel::Config for Test {
    type Event = Event;
    type Verifier = MockVerifier;
    type MessageDispatch = MockMessageDispatch;
    type FeeConverter = FeeConverter<Self>;
    type FeeAssetId = ();
    type OutboundRouter = MockOutboundRouter<Self::AccountId>;
    type FeeTechAccountId = GetTrustlessBridgeFeesTechAccountId;
    type TreasuryTechAccountId = GetTreasuryTechAccountId;
    type WeightInfo = ();
}

pub struct MockOutboundRouter<AccountId>(PhantomData<AccountId>);

impl<AccountId> OutboundRouter<AccountId> for MockOutboundRouter<AccountId> {
    fn submit(
        _: EthNetworkId,
        _: &RawOrigin<AccountId>,
        _: H160,
        _: U256,
        _: &[u8],
    ) -> Result<H256, DispatchError> {
        Ok(Default::default())
    }
}

pub type TechAccountId = common::TechAccountId<AccountId, TechAssetId, DEXId>;
pub type TechAssetId = common::TechAssetId<common::PredefinedAssetId>;

impl technical::Config for Test {
    type Event = Event;
    type TechAssetId = TechAssetId;
    type TechAccountId = TechAccountId;
    type Trigger = ();
    type Condition = ();
    type SwapAction = ();
}

pub fn new_tester(source_channel: H160) -> sp_io::TestExternalities {
    new_tester_with_config(bridge_inbound_channel::GenesisConfig {
        networks: vec![(BASE_NETWORK_ID, source_channel)],
        reward_fraction: Perbill::from_percent(80),
    })
}

pub fn new_tester_with_config(
    config: bridge_inbound_channel::GenesisConfig,
) -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    technical::GenesisConfig::<Test> {
        register_tech_accounts: vec![
            (
                GetTrustlessBridgeFeesAccountId::get(),
                GetTrustlessBridgeFeesTechAccountId::get(),
            ),
            (GetTreasuryAccountId::get(), GetTreasuryTechAccountId::get()),
        ],
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    GenesisBuild::<Test>::assimilate_storage(&config, &mut storage).unwrap();

    let bob: AccountId = Keyring::Bob.into();
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

// The originating channel address for the messages below
const SOURCE_CHANNEL_ADDR: [u8; 20] = hex!["4130819912a398f4eb84e7f16ed443232ba638b5"];

// Message with nonce = 1
const MESSAGE_DATA_0: [u8; 317] = hex!(
    "
	f9013a944130819912a398f4eb84e7f16ed443232ba638b5e1a05e9ae1d7c484
	f74d554a503aa825e823725531d97e784dd9b1aacdb58d1f7076b90100000000
	000000000000000000c2c5d46481c291be111d5e3a0b52114bdf212a01000000
	0000000000000000000000000000000000000000000000000000000001000000
	0000000000000000000000000000000000000000000de0b6b3a7640000000000
	0000000000000000000000000000000000000000000000000000000080000000
	00000000000000000000000000000000000000000000000000000000570c0182
	13dae5f9c236beab905c8305cb159c5fa1aae500d43593c715fdd31c61141abd
	04a99fd6822c8558854ccde39a5684e7a56da27d0000d9e9ac2d780300000000
	0000000000000000000000000000000000000000000000000000000000
"
);

// Message with nonce = 2
const MESSAGE_DATA_1: [u8; 317] = hex!(
    "
	f9013a944130819912a398f4eb84e7f16ed443232ba638b5e1a05e9ae1d7c484
	f74d554a503aa825e823725531d97e784dd9b1aacdb58d1f7076b90100000000
	000000000000000000c2c5d46481c291be111d5e3a0b52114bdf212a01000000
	0000000000000000000000000000000000000000000000000000000002000000
	0000000000000000000000000000000000000000000de0b6b3a7640000000000
	0000000000000000000000000000000000000000000000000000000080000000
	00000000000000000000000000000000000000000000000000000000570c0182
	13dae5f9c236beab905c8305cb159c5fa1aae500d43593c715fdd31c61141abd
	04a99fd6822c8558854ccde39a5684e7a56da27d0000d9e9ac2d780300000000
	0000000000000000000000000000000000000000000000000000000000
"
);

#[test]
fn test_submit_with_invalid_source_channel() {
    new_tester(H160::zero()).execute_with(|| {
        let relayer: AccountId = Keyring::Bob.into();
        let origin = Origin::signed(relayer);

        // Submit message
        let message = Message {
            data: MESSAGE_DATA_0.into(),
            proof: Proof {
                block_hash: Default::default(),
                tx_index: Default::default(),
                data: Default::default(),
            },
        };
        common::assert_noop_transactional!(
            BridgeInboundChannel::submit(origin.clone(), BASE_NETWORK_ID, message.clone()),
            Error::<Test>::InvalidSourceChannel
        );
    });
}

#[test]
fn test_submit() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        let relayer: AccountId = Keyring::Bob.into();
        let origin = Origin::signed(relayer);

        // Submit message 1
        let message_1 = Message {
            data: MESSAGE_DATA_0.into(),
            proof: Proof {
                block_hash: Default::default(),
                tx_index: Default::default(),
                data: Default::default(),
            },
        };
        assert_ok!(BridgeInboundChannel::submit(
            origin.clone(),
            BASE_NETWORK_ID,
            message_1
        ));
        let nonce: u64 = <ChannelNonces<Test>>::get(BASE_NETWORK_ID);
        assert_eq!(nonce, 1);

        // Submit message 2
        let message_2 = Message {
            data: MESSAGE_DATA_1.into(),
            proof: Proof {
                block_hash: Default::default(),
                tx_index: Default::default(),
                data: Default::default(),
            },
        };
        assert_ok!(BridgeInboundChannel::submit(
            origin.clone(),
            BASE_NETWORK_ID,
            message_2
        ));
        let nonce: u64 = <ChannelNonces<Test>>::get(BASE_NETWORK_ID);
        assert_eq!(nonce, 2);
    });
}

#[test]
fn test_submit_with_invalid_nonce() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        let relayer: AccountId = Keyring::Bob.into();
        let origin = Origin::signed(relayer);

        // Submit message
        let message = Message {
            data: MESSAGE_DATA_0.into(),
            proof: Proof {
                block_hash: Default::default(),
                tx_index: Default::default(),
                data: Default::default(),
            },
        };
        assert_ok!(BridgeInboundChannel::submit(
            origin.clone(),
            BASE_NETWORK_ID,
            message.clone()
        ));
        let nonce: u64 = <ChannelNonces<Test>>::get(BASE_NETWORK_ID);
        assert_eq!(nonce, 1);

        // Submit the same again
        common::assert_noop_transactional!(
            BridgeInboundChannel::submit(origin.clone(), BASE_NETWORK_ID, message.clone()),
            Error::<Test>::InvalidNonce
        );
    });
}

#[test]
#[ignore] // TODO: fix test_handle_fee test
fn test_handle_fee() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        let relayer: AccountId = Keyring::Bob.into();
        let fee_asset_id = <Test as Config>::FeeAssetId::get();
        let treasury_acc = <Test as Config>::TreasuryTechAccountId::get();
        let fees_acc = <Test as Config>::FeeTechAccountId::get();

        technical::Pallet::<Test>::mint(&fee_asset_id, &fees_acc, balance!(10)).unwrap();

        let fee = balance!(1); // 1 DOT

        BridgeInboundChannel::handle_fee(fee, &relayer);
        assert_eq!(
            technical::Pallet::<Test>::total_balance(&fee_asset_id, &treasury_acc,).unwrap(),
            balance!(0.2)
        );
        assert_eq!(
            assets::Pallet::<Test>::total_balance(&fee_asset_id, &relayer).unwrap(),
            balance!(0.8)
        );
    });
}

#[test]
fn test_set_reward_fraction_not_authorized() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        let bob: AccountId = Keyring::Bob.into();
        common::assert_noop_transactional!(
            BridgeInboundChannel::set_reward_fraction(
                Origin::signed(bob),
                Perbill::from_percent(60)
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_submit_with_invalid_network_id() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        let relayer: AccountId = Keyring::Bob.into();
        let origin = Origin::signed(relayer);

        // Submit message
        let message = Message {
            data: MESSAGE_DATA_0.into(),
            proof: Proof {
                block_hash: Default::default(),
                tx_index: Default::default(),
                data: Default::default(),
            },
        };
        common::assert_noop_transactional!(
            BridgeInboundChannel::submit(origin.clone(), BASE_NETWORK_ID + 1, message.clone()),
            Error::<Test>::InvalidNetwork
        );
    });
}

#[test]
fn test_register_channel() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        assert_ok!(BridgeInboundChannel::register_channel(
            Origin::root(),
            BASE_NETWORK_ID + 1,
            H160::from(SOURCE_CHANNEL_ADDR),
        ));

        assert_eq!(
            ChannelAddresses::<Test>::get(BASE_NETWORK_ID + 1),
            Some(H160::from(SOURCE_CHANNEL_ADDR)),
        );
    });
}

#[test]
fn test_register_existing_channel() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        common::assert_noop_transactional!(
            BridgeInboundChannel::register_channel(
                Origin::root(),
                BASE_NETWORK_ID,
                H160::from(SOURCE_CHANNEL_ADDR),
            ),
            Error::<Test>::ContractExists
        );
    });
}

#[test]
fn test_register_app() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        assert_ok!(BridgeInboundChannel::register_app(
            BASE_NETWORK_ID,
            H160::repeat_byte(7)
        ));
    })
}

#[test]
fn test_register_app_invalid_network() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        assert_err!(
            BridgeInboundChannel::register_app(BASE_NETWORK_ID + 1, H160::repeat_byte(7)),
            Error::<Test>::InvalidNetwork
        );
    })
}

#[test]
fn test_deregister_app() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        assert_ok!(BridgeInboundChannel::deregister_app(
            BASE_NETWORK_ID,
            H160::repeat_byte(7)
        ));
    })
}

#[test]
fn test_deregister_app_invalid_network() {
    new_tester(SOURCE_CHANNEL_ADDR.into()).execute_with(|| {
        assert_err!(
            BridgeInboundChannel::deregister_app(BASE_NETWORK_ID + 1, H160::repeat_byte(7)),
            Error::<Test>::InvalidNetwork
        );
    })
}
