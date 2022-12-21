#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::{DispatchResult, Dispatchable, GetDispatchInfo, Parameter};
use frame_support::traits::{Contains, EnsureOrigin};

use sp_core::RuntimeDebug;

use sp_std::prelude::*;

use bridge_types::traits;
use bridge_types::H256;

use codec::{Decode, Encode};

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    RuntimeDebug,
    scale_info::TypeInfo,
    codec::MaxEncodedLen,
)]
pub struct RawOrigin<NetworkId, Source, OriginOutput: traits::OriginOutput<NetworkId, Source>> {
    pub origin: OriginOutput,
    network_id: sp_std::marker::PhantomData<NetworkId>,
    source: sp_std::marker::PhantomData<Source>,
}

impl<NetworkId, Source, OriginOutput: traits::OriginOutput<NetworkId, Source>>
    RawOrigin<NetworkId, Source, OriginOutput>
{
    pub fn new(origin: OriginOutput) -> Self {
        Self {
            origin,
            network_id: Default::default(),
            source: Default::default(),
        }
    }
}

#[derive(Default)]
pub struct EnsureAccount<NetworkId, Source, OriginOutput: traits::OriginOutput<NetworkId, Source>>(
    sp_std::marker::PhantomData<(NetworkId, Source, OriginOutput)>,
);

impl<NetworkId, Source, OuterOrigin, OriginOutput: traits::OriginOutput<NetworkId, Source>>
    EnsureOrigin<OuterOrigin> for EnsureAccount<NetworkId, Source, OriginOutput>
where
    OuterOrigin: Into<Result<RawOrigin<NetworkId, Source, OriginOutput>, OuterOrigin>>
        + From<RawOrigin<NetworkId, Source, OriginOutput>>,
{
    type Success = OriginOutput;

    fn try_origin(o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
        o.into().and_then(|o| Ok(o.origin))
    }

    #[cfg(feature = "runtime-benchmarks")]
    fn try_successful_origin() -> Result<OuterOrigin, ()> {
        Err(())
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_support::traits::StorageVersion;
    use frame_system::pallet_prelude::*;
    use sp_runtime::traits::Hash;

    /// The current storage version.
    const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::storage_version(STORAGE_VERSION)]
    #[pallet::without_storage_info]
    pub struct Pallet<T, I = ()>(_);

    #[pallet::config]
    pub trait Config<I: 'static = ()>: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self, I>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The Id of the network (i.e. Ethereum network id).
        type NetworkId;

        /// The source of origin.
        type Source;

        type OriginOutput: traits::OriginOutput<Self::NetworkId, Self::Source>;

        /// The overarching origin type.
        type RuntimeOrigin: From<RawOrigin<Self::NetworkId, Self::Source, Self::OriginOutput>>;

        /// Id of the message. Whenever message is passed to the dispatch module, it emits
        /// event with this id + dispatch result.
        type MessageId: Parameter;

        type Hashing: Hash<Output = H256>;

        /// The overarching dispatch call type.
        type RuntimeCall: Parameter
            + GetDispatchInfo
            + Dispatchable<
                RuntimeOrigin = <Self as Config<I>>::RuntimeOrigin,
                PostInfo = frame_support::dispatch::PostDispatchInfo,
            >;

        /// The pallet will filter all incoming calls right before they're dispatched. If this filter
        /// rejects the call, special event (`Event::MessageRejected`) is emitted.
        type CallFilter: Contains<<Self as Config<I>>::RuntimeCall>;
    }

    #[pallet::hooks]
    impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

    #[pallet::call]
    impl<T: Config<I>, I: 'static> Pallet<T, I> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config<I>, I: 'static = ()> {
        /// Message has been dispatched with given result.
        MessageDispatched(T::MessageId, DispatchResult),
        /// Message has been rejected
        MessageRejected(T::MessageId),
        /// We have failed to decode a Call from the message.
        MessageDecodeFailed(T::MessageId),
    }

    #[pallet::origin]
    #[allow(type_alias_bounds)]
    pub type Origin<T: Config<I>, I: 'static = ()> = RawOrigin<
        <T as Config<I>>::NetworkId,
        <T as Config<I>>::Source,
        <T as Config<I>>::OriginOutput,
    >;

    impl<T: Config> traits::MessageDispatch<T, T::NetworkId, T::Source, T::MessageId> for Pallet<T> {
        fn dispatch(
            network_id: T::NetworkId,
            source: T::Source,
            message_id: T::MessageId,
            timestamp: u64,
            payload: &[u8],
        ) {
            let call = match <T as Config>::RuntimeCall::decode(&mut &payload[..]) {
                Ok(call) => call,
                Err(_) => {
                    Self::deposit_event(Event::MessageDecodeFailed(message_id));
                    return;
                }
            };

            if !T::CallFilter::contains(&call) {
                Self::deposit_event(Event::MessageRejected(message_id));
                return;
            }

            let origin = RawOrigin::new(<T::OriginOutput as traits::OriginOutput<_, _>>::new(
                network_id,
                source,
                message_id.using_encoded(|v| <T as Config>::Hashing::hash(v)),
                timestamp,
            ))
            .into();
            let result = call.dispatch(origin);

            Self::deposit_event(Event::MessageDispatched(
                message_id,
                result.map(drop).map_err(|e| e.error),
            ));
        }

        #[cfg(feature = "runtime-benchmarks")]
        fn successful_dispatch_event(
            id: T::MessageId,
        ) -> Option<<T as frame_system::Config>::RuntimeEvent> {
            let event: <T as Config>::RuntimeEvent =
                Event::<T>::MessageDispatched(id, Ok(())).into();
            Some(event.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::traits::MessageDispatch as _;
    use bridge_types::types;
    use bridge_types::{EthNetworkId, H160, H256};
    use frame_support::dispatch::DispatchError;
    use frame_support::parameter_types;
    use frame_support::traits::{ConstU32, Everything};
    use frame_system::{EventRecord, Phase};
    use sp_runtime::testing::Header;
    use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Keccak256};

    use crate as dispatch;

    type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
    type Block = frame_system::mocking::MockBlock<Test>;

    frame_support::construct_runtime!(
        pub enum Test where
            Block = Block,
            NodeBlock = Block,
            UncheckedExtrinsic = UncheckedExtrinsic,
        {
            System: frame_system::{Pallet, Call, Storage, Event<T>},
            Dispatch: dispatch::{Pallet, Storage, Origin<T>, Event<T>},
        }
    );

    type AccountId = u64;

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
    }

    impl frame_system::Config for Test {
        type RuntimeOrigin = RuntimeOrigin;
        type Index = u64;
        type RuntimeCall = RuntimeCall;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type RuntimeEvent = RuntimeEvent;
        type BlockHashCount = BlockHashCount;
        type Version = ();
        type PalletInfo = PalletInfo;
        type AccountData = ();
        type OnNewAccount = ();
        type OnKilledAccount = ();
        type BaseCallFilter = Everything;
        type SystemWeightInfo = ();
        type BlockWeights = ();
        type BlockLength = ();
        type DbWeight = ();
        type SS58Prefix = ();
        type OnSetCode = ();
        type MaxConsumers = ConstU32<65536>;
    }

    pub struct CallFilter;
    impl frame_support::traits::Contains<RuntimeCall> for CallFilter {
        fn contains(call: &RuntimeCall) -> bool {
            match call {
                RuntimeCall::System(frame_system::pallet::Call::<Test>::remark { .. }) => true,
                _ => false,
            }
        }
    }

    impl dispatch::Config for Test {
        type RuntimeEvent = RuntimeEvent;
        type NetworkId = EthNetworkId;
        type Source = H160;
        type OriginOutput = types::CallOriginOutput<EthNetworkId, H160, H256>;
        type RuntimeOrigin = RuntimeOrigin;
        type MessageId = types::MessageId;
        type Hashing = Keccak256;
        type RuntimeCall = RuntimeCall;
        type CallFilter = CallFilter;
    }

    fn new_test_ext() -> sp_io::TestExternalities {
        let t = frame_system::GenesisConfig::default()
            .build_storage::<Test>()
            .unwrap();
        sp_io::TestExternalities::new(t)
    }

    #[test]
    fn test_dispatch_bridge_message() {
        new_test_ext().execute_with(|| {
            let id = types::MessageId::inbound(37);
            let source = H160::repeat_byte(7);

            let message =
                RuntimeCall::System(frame_system::pallet::Call::<Test>::remark { remark: vec![] })
                    .encode();

            System::set_block_number(1);
            Dispatch::dispatch(2u32.into(), source, id, 0, &message);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Dispatch(crate::Event::<Test>::MessageDispatched(
                        id,
                        Err(DispatchError::BadOrigin)
                    )),
                    topics: vec![],
                }],
            );
        })
    }

    #[test]
    fn test_message_decode_failed() {
        new_test_ext().execute_with(|| {
            let id = types::MessageId::inbound(37);
            let source = H160::repeat_byte(7);

            let message: Vec<u8> = vec![1, 2, 3];

            System::set_block_number(1);
            Dispatch::dispatch(2u32.into(), source, id, 0, &message);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Dispatch(crate::Event::<Test>::MessageDecodeFailed(id)),
                    topics: vec![],
                }],
            );
        })
    }

    #[test]
    fn test_message_rejected() {
        new_test_ext().execute_with(|| {
            let id = types::MessageId::inbound(37);
            let source = H160::repeat_byte(7);

            let message =
                RuntimeCall::System(frame_system::pallet::Call::<Test>::set_code { code: vec![] })
                    .encode();

            System::set_block_number(1);
            Dispatch::dispatch(2u32.into(), source, id, 0, &message);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: RuntimeEvent::Dispatch(crate::Event::<Test>::MessageRejected(id)),
                    topics: vec![],
                }],
            );
        })
    }
}
