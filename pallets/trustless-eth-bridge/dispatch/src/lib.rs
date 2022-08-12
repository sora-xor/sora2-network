#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::{DispatchResult, Dispatchable, Parameter};
use frame_support::traits::{Contains, EnsureOrigin};
use frame_support::weights::GetDispatchInfo;

use sp_core::RuntimeDebug;

use sp_core::H160;
use sp_std::prelude::*;

use bridge_types::traits::MessageDispatch;

use bridge_types::{EthNetworkId, H256};
use codec::{Decode, Encode};

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, scale_info::TypeInfo)]
pub struct RawOrigin(pub EthNetworkId, pub H256, pub H160);

impl From<(EthNetworkId, H256, H160)> for RawOrigin {
    fn from(origin: (EthNetworkId, H256, H160)) -> RawOrigin {
        RawOrigin(origin.0, origin.1, origin.2)
    }
}

pub struct EnsureEthereumAccount;

impl<OuterOrigin> EnsureOrigin<OuterOrigin> for EnsureEthereumAccount
where
    OuterOrigin: Into<Result<RawOrigin, OuterOrigin>> + From<RawOrigin>,
{
    type Success = (EthNetworkId, H256, H160);

    fn try_origin(o: OuterOrigin) -> Result<Self::Success, OuterOrigin> {
        o.into().and_then(|o| Ok((o.0, o.1, o.2)))
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
    use bridge_types::types::MessageId;
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
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// The overarching origin type.
        type Origin: From<RawOrigin>;

        /// Id of the message. Whenever message is passed to the dispatch module, it emits
        /// event with this id + dispatch result.
        type MessageId: Parameter;

        type Hashing: Hash<Output = H256>;

        /// The overarching dispatch call type.
        type Call: Parameter
            + GetDispatchInfo
            + Dispatchable<
                Origin = <Self as Config>::Origin,
                PostInfo = frame_support::dispatch::PostDispatchInfo,
            >;

        /// The pallet will filter all incoming calls right before they're dispatched. If this filter
        /// rejects the call, special event (`Event::MessageRejected`) is emitted.
        type CallFilter: Contains<<Self as Config>::Call>;
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Message has been dispatched with given result.
        MessageDispatched(MessageId, DispatchResult),
        /// Message has been rejected
        MessageRejected(MessageId),
        /// We have failed to decode a Call from the message.
        MessageDecodeFailed(MessageId),
    }

    #[pallet::origin]
    pub type Origin = RawOrigin;

    impl<T: Config> MessageDispatch<T, MessageId> for Pallet<T> {
        fn dispatch(network_id: EthNetworkId, source: H160, message_id: MessageId, payload: &[u8]) {
            let call = match <T as Config>::Call::decode(&mut &payload[..]) {
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

            let origin = RawOrigin(
                network_id,
                message_id.using_encoded(|v| <T as Config>::Hashing::hash(v)),
                source,
            )
            .into();
            let result = call.dispatch(origin);

            Self::deposit_event(Event::MessageDispatched(
                message_id,
                result.map(drop).map_err(|e| e.error),
            ));
        }

        #[cfg(feature = "runtime-benchmarks")]
        fn successful_dispatch_event(id: MessageId) -> Option<<T as frame_system::Config>::Event> {
            let event: <T as Config>::Event = Event::<T>::MessageDispatched(id, Ok(())).into();
            Some(event.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bridge_types::types::MessageId;
    use frame_support::dispatch::DispatchError;
    use frame_support::parameter_types;
    use frame_support::traits::{ConstU32, Everything};
    use frame_system::{EventRecord, Phase};
    use sp_core::H256;
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
            Dispatch: dispatch::{Pallet, Storage, Origin, Event<T>},
        }
    );

    type AccountId = u64;

    parameter_types! {
        pub const BlockHashCount: u64 = 250;
    }

    impl frame_system::Config for Test {
        type Origin = Origin;
        type Index = u64;
        type Call = Call;
        type BlockNumber = u64;
        type Hash = H256;
        type Hashing = BlakeTwo256;
        type AccountId = AccountId;
        type Lookup = IdentityLookup<Self::AccountId>;
        type Header = Header;
        type Event = Event;
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
    impl frame_support::traits::Contains<Call> for CallFilter {
        fn contains(call: &Call) -> bool {
            match call {
                Call::System(frame_system::pallet::Call::<Test>::remark { .. }) => true,
                _ => false,
            }
        }
    }

    impl dispatch::Config for Test {
        type Origin = Origin;
        type Event = Event;
        type MessageId = u64;
        type Hashing = Keccak256;
        type Call = Call;
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
            let id = MessageId::inbound(37);
            let source = H160::repeat_byte(7);

            let message =
                Call::System(frame_system::pallet::Call::<Test>::remark { remark: vec![] })
                    .encode();

            System::set_block_number(1);
            Dispatch::dispatch(2u32.into(), source, id, &message);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: Event::Dispatch(crate::Event::<Test>::MessageDispatched(
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
            let id = MessageId::inbound(37);
            let source = H160::repeat_byte(7);

            let message: Vec<u8> = vec![1, 2, 3];

            System::set_block_number(1);
            Dispatch::dispatch(2u32.into(), source, id, &message);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: Event::Dispatch(crate::Event::<Test>::MessageDecodeFailed(id)),
                    topics: vec![],
                }],
            );
        })
    }

    #[test]
    fn test_message_rejected() {
        new_test_ext().execute_with(|| {
            let id = MessageId::inbound(37);
            let source = H160::repeat_byte(7);

            let message =
                Call::System(frame_system::pallet::Call::<Test>::set_code { code: vec![] })
                    .encode();

            System::set_block_number(1);
            Dispatch::dispatch(2u32.into(), source, id, &message);

            assert_eq!(
                System::events(),
                vec![EventRecord {
                    phase: Phase::Initialization,
                    event: Event::Dispatch(crate::Event::<Test>::MessageRejected(id)),
                    topics: vec![],
                }],
            );
        })
    }
}
