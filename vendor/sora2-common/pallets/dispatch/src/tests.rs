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

#[test]
fn trailing_payload_is_a_failed_application() {
    new_test_ext().execute_with(|| {
        let id = types::MessageId::batched(
            SubNetworkId::Mainnet.into(),
            SubNetworkId::Rococo.into(),
            1,
            0,
        );
        let mut message =
            RuntimeCall::System(frame_system::Call::<Test>::remark { remark: vec![] }).encode();
        message.push(0);
        System::set_block_number(1);
        assert_eq!(
            Dispatch::dispatch(
                H256::from_low_u64_be(2).into(),
                id,
                Default::default(),
                &message,
                AdditionalEVMInboundData {
                    source: H160::repeat_byte(7)
                }
                .into(),
            ),
            bridge_types::traits::MessageDispatchOutcome::Failed,
        );
        System::assert_last_event(crate::Event::<Test>::MessageDecodeFailed(id).into());
    });
}

#[test]
fn dispatch_weights_include_decoding_and_call_execution_costs() {
    use frame_support::dispatch::GetDispatchInfo;
    let call = RuntimeCall::System(frame_system::Call::<Test>::remark {
        remark: vec![1, 2, 3],
    });
    assert_eq!(
        Dispatch::dispatch_weight(&call.encode()),
        call.get_dispatch_info()
            .call_weight
            .saturating_add(<() as WeightInfo>::dispatch_success())
    );
    assert_eq!(
        Dispatch::dispatch_weight(&[255]),
        <() as WeightInfo>::dispatch_decode_failed()
    );
    let mut trailing = call.encode();
    trailing.push(0);
    assert_eq!(
        Dispatch::dispatch_weight(&trailing),
        <() as WeightInfo>::dispatch_decode_failed()
    );
}

#[test]
fn ensure_account_accepts_only_the_bridge_origin_and_preserves_its_context() {
    type Output = <Test as Config>::OriginOutput;
    let context = Output {
        network_id: SubNetworkId::Kusama.into(),
        message_id: H256::repeat_byte(9),
        timepoint: bridge_types::GenericTimepoint::Sora(12),
        additional: AdditionalEVMInboundData {
            source: H160::repeat_byte(8),
        }
        .into(),
    };
    let origin: RuntimeOrigin = RawOrigin::new(context).into();
    assert_eq!(
        EnsureAccount::<Output>::try_origin(origin).unwrap(),
        context
    );
    assert!(EnsureAccount::<Output>::try_origin(RuntimeOrigin::root()).is_err());
    assert!(EnsureAccount::<Output>::try_origin(RuntimeOrigin::signed(1)).is_err());
}

#[test]
fn failed_dispatched_calls_record_the_error_and_return_failed_outcome() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let id = types::MessageId::batched(
            SubNetworkId::Mainnet.into(),
            SubNetworkId::Rococo.into(),
            1,
            0,
        );
        let message =
            RuntimeCall::System(frame_system::Call::<Test>::remark_with_event { remark: vec![1] })
                .encode();
        assert_eq!(
            Dispatch::dispatch(
                SubNetworkId::Mainnet.into(),
                id,
                Default::default(),
                &message,
                AdditionalEVMInboundData {
                    source: H160::zero()
                }
                .into()
            ),
            bridge_types::traits::MessageDispatchOutcome::Failed
        );
        System::assert_last_event(
            Event::<Test>::MessageDispatched(id, Err(sp_runtime::DispatchError::BadOrigin)).into(),
        );
    });
}
