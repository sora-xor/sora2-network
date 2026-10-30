use super::*;
use bridge_types::traits::{
    BridgeApp, BridgeAssetLocker, BridgeAssetRegistry, MessageStatusNotifier,
};
use bridge_types::types::{AssetKind, BridgeAppInfo, BridgeAssetInfo};
use bridge_types::SubNetworkId;
use frame_support::traits::{AsEnsureOriginWithArg, ConstU64};
use frame_support::{assert_noop, assert_ok, construct_runtime, derive_impl, parameter_types};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Assets: pallet_assets,
        Provider: crate,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId32;
    type Lookup = IdentityLookup<AccountId32>;
    type AccountData = pallet_balances::AccountData<u64>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type AccountStore = System;
}

#[derive_impl(pallet_assets::config_preludes::TestDefaultConfig)]
impl pallet_assets::Config for Test {
    type Currency = Balances;
    type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId32>>;
    type ForceOrigin = frame_system::EnsureRoot<AccountId32>;
}

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    codec::DecodeWithMemTracking,
    MaxEncodedLen,
    TypeInfo,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct AssetId(u64);

impl From<u32> for AssetId {
    fn from(value: u32) -> Self {
        Self(value.into())
    }
}

impl From<LiberlandAssetId> for AssetId {
    fn from(value: LiberlandAssetId) -> Self {
        match value {
            LiberlandAssetId::LLD => Self(u64::MAX),
            LiberlandAssetId::Asset(id) => Self(id.into()),
        }
    }
}

parameter_types! {
    pub TechAccount: AccountId32 = AccountId32::new([9; 32]);
}

pub struct Timepoint;
impl TimepointProvider for Timepoint {
    fn get_timepoint() -> GenericTimepoint {
        GenericTimepoint::Sora(System::block_number() as u32)
    }
}

pub struct App;
impl BridgeApp<AccountId32, GenericAccount, LiberlandAssetId, u64> for App {
    fn is_asset_supported(network: GenericNetworkId, asset: LiberlandAssetId) -> bool {
        network == network_id() && asset != LiberlandAssetId::Asset(u32::MAX)
    }
    fn transfer(
        _: GenericNetworkId,
        _: LiberlandAssetId,
        _: AccountId32,
        _: GenericAccount,
        _: u64,
    ) -> Result<H256, DispatchError> {
        Err(DispatchError::Unavailable)
    }
    fn refund(
        network: GenericNetworkId,
        id: H256,
        recipient: AccountId32,
        asset: LiberlandAssetId,
        amount: u64,
    ) -> DispatchResult {
        if sp_io::storage::exists(b"provider-test:refund-fails") {
            return Err(DispatchError::Unavailable);
        }
        sp_io::storage::set(
            b"provider-test:refund",
            &(network, id, recipient, asset, amount).encode(),
        );
        Ok(())
    }
    fn list_supported_assets(_: GenericNetworkId) -> Vec<BridgeAssetInfo> {
        vec![]
    }
    fn list_apps() -> Vec<BridgeAppInfo> {
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

impl Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type MinBalance = ConstU64<1>;
    type AssetId = AssetId;
    type Balances = Balances;
    type SoraApp = App;
    type AccountIdConverter = sp_runtime::traits::Identity;
    type TimepointProvider = Timepoint;
    type SoraMainnetTechAcc = TechAccount;
}

fn network_id() -> GenericNetworkId {
    SubNetworkId::Mainnet.into()
}

#[test]
fn provider_exposes_no_dispatchable_calls() {
    use frame_support::traits::{GetCallIndex, GetCallName};
    assert!(Call::<Test>::get_call_names().is_empty());
    assert!(Call::<Test>::get_call_indices().is_empty());
    assert_eq!(
        Provider::call_functions().ty,
        scale_info::meta_type::<Call<Test>>()
    );
}
fn alice() -> AccountId32 {
    AccountId32::new([1; 32])
}
fn ext() -> sp_io::TestExternalities {
    init_test_logging();
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    pallet_balances::GenesisConfig::<Test> {
        balances: vec![(alice(), 100_000), (TechAccount::get(), 100_000)],
        dev_accounts: Default::default(),
    }
    .assimilate_storage(&mut storage)
    .unwrap();
    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| System::set_block_number(1));
    ext
}

#[test]
fn asset_registry_creates_metadata_and_reports_native_and_missing_assets() {
    ext().execute_with(|| {
        let asset =
            Provider::register_asset(network_id(), b"Bridge Token".to_vec(), b"BRG".to_vec())
                .unwrap();
        assert_eq!(Provider::asset_nonce(), 1);
        assert!(Provider::ensure_asset_exists(asset));
        assert!(Provider::ensure_asset_exists(LiberlandAssetId::LLD));
        assert!(!Provider::ensure_asset_exists(LiberlandAssetId::Asset(
            u32::MAX
        )));
        let info = Provider::get_raw_info(asset);
        assert_eq!(
            (info.name, info.symbol, info.precision),
            (b"Bridge Token".to_vec(), b"BRG".to_vec(), 18)
        );
        let native = Provider::get_raw_info(LiberlandAssetId::LLD);
        assert_eq!(
            (native.name, native.symbol, native.precision),
            (b"Liberland".to_vec(), b"LLD".to_vec(), 12)
        );
        assert_ok!(Provider::manage_asset(network_id(), asset));
        assert!(System::events()
            .iter()
            .any(|event| event.event == RuntimeEvent::Provider(Event::AssetCreated(asset.into()))));
    });
}

fn candidate_id(name: &[u8], symbol: &[u8], nonce: u32) -> u32 {
    let mut input = name.to_vec();
    input.extend(symbol);
    input.extend(nonce.encode());
    u32::from_be_bytes(blake2_256(&input)[..4].try_into().unwrap())
}

#[test]
fn registration_retries_collisions_and_stops_after_three_candidates() {
    for collisions in [1u32, 3] {
        ext().execute_with(|| {
            let name = b"Collision";
            let symbol = b"COL";
            for nonce in 0..collisions {
                assert_ok!(<Assets as Create<AccountId32>>::create(
                    candidate_id(name, symbol, nonce),
                    TechAccount::get(),
                    true,
                    1
                ));
            }
            let result = Provider::register_asset(network_id(), name.to_vec(), symbol.to_vec());
            if collisions == 1 {
                assert_eq!(
                    result,
                    Ok(LiberlandAssetId::Asset(candidate_id(name, symbol, 1)))
                );
            } else {
                assert_eq!(result, Err(Error::<Test>::FailedToCreateAsset.into()));
            }
            assert_eq!(Provider::asset_nonce(), 1);
        });
    }
}

#[test]
fn native_custody_round_trip_and_sidechain_rejection() {
    ext().execute_with(|| {
        let owner = alice();
        let initial = Balances::free_balance(&owner);
        let custody = Balances::free_balance(TechAccount::get());
        assert_ok!(Provider::lock_asset(
            network_id(),
            AssetKind::Thischain,
            &owner,
            &LiberlandAssetId::LLD,
            &100
        ));
        assert_eq!(Balances::free_balance(&owner), initial - 100);
        assert_eq!(Balances::free_balance(TechAccount::get()), custody + 100);
        assert_ok!(Provider::unlock_asset(
            network_id(),
            AssetKind::Thischain,
            &owner,
            &LiberlandAssetId::LLD,
            &40
        ));
        assert_eq!(Balances::free_balance(&owner), initial - 60);
        assert_noop!(
            Provider::lock_asset(
                network_id(),
                AssetKind::Sidechain,
                &owner,
                &LiberlandAssetId::LLD,
                &1
            ),
            Error::<Test>::WrongSidechainAsset
        );
        assert_noop!(
            Provider::unlock_asset(
                network_id(),
                AssetKind::Sidechain,
                &owner,
                &LiberlandAssetId::LLD,
                &1
            ),
            Error::<Test>::WrongSidechainAsset
        );
    });
}

#[test]
fn asset_custody_and_sidechain_issuance_round_trip() {
    ext().execute_with(|| {
        assert_ok!(<Assets as Create<AccountId32>>::create(
            1,
            TechAccount::get(),
            true,
            1
        ));
        assert_ok!(<Assets as Mutate<AccountId32>>::mint_into(1, &alice(), 100));
        let asset = LiberlandAssetId::Asset(1);
        assert_ok!(Provider::lock_asset(
            network_id(),
            AssetKind::Thischain,
            &alice(),
            &asset,
            &40
        ));
        assert_eq!(Assets::balance(1, alice()), 60);
        assert_eq!(Assets::balance(1, TechAccount::get()), 40);
        assert_ok!(Provider::unlock_asset(
            network_id(),
            AssetKind::Thischain,
            &alice(),
            &asset,
            &10
        ));
        assert_eq!(Assets::balance(1, alice()), 70);
        assert_ok!(Provider::lock_asset(
            network_id(),
            AssetKind::Sidechain,
            &alice(),
            &asset,
            &20
        ));
        assert_eq!(Assets::total_supply(1), 80);
        assert_ok!(Provider::unlock_asset(
            network_id(),
            AssetKind::Sidechain,
            &alice(),
            &asset,
            &15
        ));
        assert_eq!(Assets::total_supply(1), 95);
        assert_eq!(Assets::balance(1, alice()), 65);
        assert_noop!(
            Provider::refund_fee(network_id(), &alice(), &asset, &1),
            DispatchError::Unavailable
        );
        assert_noop!(
            Provider::withdraw_fee(network_id(), &alice(), &asset, &1),
            DispatchError::Unavailable
        );
    });
}

#[test]
fn refund_routes_supported_assets_and_validates_account_kind() {
    ext().execute_with(|| {
        let id = H256::repeat_byte(1);
        assert_noop!(
            Provider::refund(
                network_id(),
                id,
                GenericAccount::Sora(alice()),
                LiberlandAssetId::LLD,
                10
            ),
            Error::<Test>::WrongAccount
        );
        assert_ok!(Provider::refund(
            network_id(),
            id,
            GenericAccount::Liberland(alice()),
            LiberlandAssetId::LLD,
            10
        ));
        assert_eq!(
            sp_io::storage::get(b"provider-test:refund")
                .unwrap()
                .as_ref(),
            (network_id(), id, alice(), LiberlandAssetId::LLD, 10u64).encode()
        );
        sp_io::storage::clear(b"provider-test:refund");
        assert_ok!(Provider::refund(
            network_id(),
            id,
            GenericAccount::Liberland(alice()),
            LiberlandAssetId::Asset(u32::MAX),
            10
        ));
        assert!(!sp_io::storage::exists(b"provider-test:refund"));
    });
}

#[test]
fn request_notifications_record_direction_timepoints_and_refund_outcomes() {
    ext().execute_with(|| {
        let network = network_id();
        let inbound_id = H256::repeat_byte(1);
        Provider::inbound_request(
            network,
            inbound_id,
            GenericAccount::Sora(alice()),
            alice(),
            LiberlandAssetId::LLD,
            20,
            GenericTimepoint::Sora(3),
            MessageStatus::Done,
        );
        let account = GenericAccount::Liberland(alice());
        let inbound = Provider::transactions((network, account.clone()), inbound_id).unwrap();
        assert_eq!(inbound.direction, MessageDirection::Inbound);
        assert_eq!(inbound.dest, account);
        assert_eq!(inbound.end_timepoint, GenericTimepoint::Sora(1));
        assert_eq!(Provider::sender(network, inbound_id), Some(account.clone()));
        Provider::update_status(
            network,
            inbound_id,
            MessageStatus::Failed,
            GenericTimepoint::Sora(4),
        );
        assert_eq!(
            Provider::transactions((network, account.clone()), inbound_id)
                .unwrap()
                .status,
            MessageStatus::Failed
        );
        assert!(!sp_io::storage::exists(b"provider-test:refund"));

        for fail in [false, true] {
            let id = H256::repeat_byte(if fail { 3 } else { 2 });
            Provider::outbound_request(
                network,
                id,
                alice(),
                GenericAccount::Sora(alice()),
                LiberlandAssetId::LLD,
                25,
                MessageStatus::InQueue,
            );
            let pending = Provider::transactions((network, account.clone()), id).unwrap();
            assert_eq!(pending.direction, MessageDirection::Outbound);
            assert_eq!(pending.start_timepoint, GenericTimepoint::Sora(1));
            assert_eq!(pending.end_timepoint, GenericTimepoint::Pending);
            if fail {
                sp_io::storage::set(b"provider-test:refund-fails", &[1]);
            }
            Provider::update_status(
                network,
                id,
                MessageStatus::Failed,
                GenericTimepoint::Sora(4),
            );
            let expected = if fail {
                MessageStatus::Failed
            } else {
                MessageStatus::Refunded
            };
            let updated = Provider::transactions((network, account.clone()), id).unwrap();
            assert_eq!(updated.status, expected);
            assert_eq!(updated.end_timepoint, GenericTimepoint::Sora(4));
            if fail {
                assert!(System::events()
                    .iter()
                    .any(|event| event.event == RuntimeEvent::Provider(Event::RefundFailed(id))));
            }
        }
        let unknown = H256::repeat_byte(99);
        let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
        Provider::update_status(
            network,
            unknown,
            MessageStatus::Done,
            GenericTimepoint::Sora(4),
        );
        assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        Senders::<Test>::insert(network, unknown, account.clone());
        Provider::update_status(
            network,
            unknown,
            MessageStatus::Done,
            GenericTimepoint::Sora(4),
        );
        assert!(Provider::transactions((network, account), unknown).is_none());
    });
}

// Exercise diagnostic formatting as it is enabled in an operator's debug build.
fn init_test_logging() {
    struct TestLogger;
    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            metadata.level() <= log::Level::Debug
        }
        fn log(&self, record: &log::Record<'_>) {
            if self.enabled(record.metadata()) {
                eprintln!("{}", record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: TestLogger = TestLogger;
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Debug);
    });
}
