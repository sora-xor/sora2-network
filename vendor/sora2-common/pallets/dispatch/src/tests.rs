use super::*;
use crate::mock::*;
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::traits::MessageDispatch as _;
use bridge_types::H160;
use bridge_types::{types, SubNetworkId};
use frame_system::{EventRecord, Phase};

#[test]
fn test_dispatch_bridge_message() {
    new_test_ext().execute_with(|| {
        let id = types::MessageId::batched(
            SubNetworkId::Mainnet.into(),
            SubNetworkId::Rococo.into(),
            1,
            37,
        );
        let source = H160::repeat_byte(7);

        let message =
            RuntimeCall::System(frame_system::pallet::Call::<Test>::remark { remark: vec![] })
                .encode();

        System::set_block_number(1);
        Dispatch::dispatch(
            H256::from_low_u64_be(2).into(),
            id,
            Default::default(),
            &message,
            AdditionalEVMInboundData { source }.into(),
        );

        assert_eq!(
            System::events(),
            vec![EventRecord {
                phase: Phase::Initialization,
                event: RuntimeEvent::Dispatch(crate::Event::<Test>::MessageDispatched(id, Ok(()))),
                topics: vec![],
            }],
        );
    })
}

#[test]
fn test_message_decode_failed() {
    new_test_ext().execute_with(|| {
        let id = types::MessageId::batched(
            SubNetworkId::Mainnet.into(),
            SubNetworkId::Rococo.into(),
            1,
            37,
        );
        let source = H160::repeat_byte(7);

        let message: Vec<u8> = vec![1, 2, 3];

        System::set_block_number(1);
        Dispatch::dispatch(
            H256::from_low_u64_be(2).into(),
            id,
            Default::default(),
            &message,
            AdditionalEVMInboundData { source }.into(),
        );

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
        let id = types::MessageId::batched(
            SubNetworkId::Mainnet.into(),
            SubNetworkId::Rococo.into(),
            1,
            37,
        );
        let source = H160::repeat_byte(7);

        let message =
            RuntimeCall::System(frame_system::pallet::Call::<Test>::set_code { code: vec![] })
                .encode();

        System::set_block_number(1);
        Dispatch::dispatch(
            H256::from_low_u64_be(2).into(),
            id,
            Default::default(),
            &message,
            AdditionalEVMInboundData { source }.into(),
        );

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
