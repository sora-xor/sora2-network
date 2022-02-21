use super::*;
use currencies::BasicCurrencyAdapter;

use common::mock::{alice, ExistentialDeposits};
use common::{Amount, AssetId32, AssetName, AssetSymbol, Balance, DEXId, XOR};
use frame_support::dispatch::DispatchError;
use frame_support::traits::{Everything, GenesisBuild};
use frame_support::{assert_noop, assert_ok, parameter_types};
use sp_core::{H160, H256};
use sp_keyring::AccountKeyring as Keyring;
use sp_runtime::testing::Header;
use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Keccak256, Verify};
use sp_runtime::MultiSignature;
use sp_std::convert::From;

use crate::outbound as incentivized_outbound_channel;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

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
        IncentivizedOutboundChannel: incentivized_outbound_channel::{Pallet, Call, Config<T>, Storage, Event<T>},
    }
);

pub type Signature = MultiSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

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
    pub GetTeamReservesAccountId: AccountId = alice();
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

parameter_types! {
    pub const MaxMessagePayloadSize: u64 = 128;
    pub const MaxMessagesPerCommit: u64 = 5;
}

impl incentivized_outbound_channel::Config for Test {
    const INDEXING_PREFIX: &'static [u8] = b"commitment";
    type Event = Event;
    type Hashing = Keccak256;
    type MaxMessagePayloadSize = MaxMessagePayloadSize;
    type MaxMessagesPerCommit = MaxMessagesPerCommit;
    type FeeCurrency = ();
    type SetFeeOrigin = frame_system::EnsureRoot<Self::AccountId>;
    type WeightInfo = ();
}

pub fn new_tester() -> sp_io::TestExternalities {
    let mut storage = frame_system::GenesisConfig::default()
        .build_storage::<Test>()
        .unwrap();

    let bob: AccountId = Keyring::Bob.into();
    let config: incentivized_outbound_channel::GenesisConfig<Test> =
        incentivized_outbound_channel::GenesisConfig {
            dest_account: Some(bob.clone()),
            interval: 1u64,
            fee: 100u32.into(),
        };
    config.assimilate_storage(&mut storage).unwrap();

    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(bob.clone(), 1u32.into())],
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

#[test]
fn test_submit() {
    new_tester().execute_with(|| {
        let target = H160::zero();
        let who: AccountId = Keyring::Bob.into();

        // Deposit enough money to cover fees
        Assets::mint_to(&XOR, &who, &who, 300u32.into()).unwrap();

        assert_ok!(IncentivizedOutboundChannel::submit(
            &who,
            target,
            &vec![0, 1, 2]
        ));
        assert_eq!(<Nonce<Test>>::get(), 1);

        assert_ok!(IncentivizedOutboundChannel::submit(
            &who,
            target,
            &vec![0, 1, 2]
        ));
        assert_eq!(<Nonce<Test>>::get(), 2);
    });
}

#[test]
#[ignore]
fn test_submit_fees_burned() {
    new_tester().execute_with(|| {
        let target = H160::zero();
        let who: AccountId = Keyring::Bob.into();

        // Deposit enough money to cover fees
        Assets::mint_to(&XOR, &who, &who, 300u32.into()).unwrap();
        let old_balance = Assets::total_balance(&XOR, &who).unwrap();

        assert_ok!(IncentivizedOutboundChannel::submit(
            &who,
            target,
            &vec![0, 1, 2]
        ));
        assert_eq!(
            Assets::total_balance(&XOR, &who).unwrap(),
            old_balance - 100
        );
    })
}

#[test]
#[ignore]
fn test_submit_not_enough_funds() {
    new_tester().execute_with(|| {
        let target = H160::zero();
        let who: AccountId = Keyring::Bob.into();

        Assets::mint_to(&XOR, &who, &who, 50u32.into()).unwrap();

        assert_noop!(
            IncentivizedOutboundChannel::submit(&who, target, &vec![0, 1, 2]),
            pallet_balances::Error::<Test>::InsufficientBalance
        );
    })
}

#[test]
fn test_submit_exceeds_queue_limit() {
    new_tester().execute_with(|| {
        let target = H160::zero();
        let who: AccountId = Keyring::Bob.into();

        // Deposit enough money to cover fees
        Assets::mint_to(&XOR, &who, &who, 1000u32.into()).unwrap();

        let max_messages = MaxMessagesPerCommit::get();
        (0..max_messages).for_each(|_| {
            IncentivizedOutboundChannel::submit(&who, target, &vec![0, 1, 2]).unwrap()
        });

        assert_noop!(
            IncentivizedOutboundChannel::submit(&who, target, &vec![0, 1, 2]),
            Error::<Test>::QueueSizeLimitReached,
        );
    })
}

#[test]
fn test_set_fee_not_authorized() {
    new_tester().execute_with(|| {
        let bob: AccountId = Keyring::Bob.into();
        assert_noop!(
            IncentivizedOutboundChannel::set_fee(Origin::signed(bob), 1000u32.into()),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_submit_exceeds_payload_limit() {
    new_tester().execute_with(|| {
        let target = H160::zero();
        let who: AccountId = Keyring::Bob.into();

        let max_payload_bytes = MaxMessagePayloadSize::get();
        let payload: Vec<u8> = (0..).take(max_payload_bytes as usize + 1).collect();

        assert_noop!(
            IncentivizedOutboundChannel::submit(&who, target, payload.as_slice()),
            Error::<Test>::PayloadTooLarge,
        );
    })
}

#[test]
fn test_submit_fails_on_nonce_overflow() {
    new_tester().execute_with(|| {
        let target = H160::zero();
        let who: AccountId = Keyring::Bob.into();

        <Nonce<Test>>::set(u64::MAX);
        assert_noop!(
            IncentivizedOutboundChannel::submit(&who, target, &vec![0, 1, 2]),
            Error::<Test>::Overflow,
        );
    });
}
