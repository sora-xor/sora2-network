use crate::mock::{
    new_tester, AccountId, Assets, EthApp, RuntimeEvent, RuntimeOrigin, System, Test,
    BASE_NETWORK_ID,
};
use crate::{Addresses, Error};
use bridge_types::evm::AdditionalEVMInboundData;
use bridge_types::types::CallOriginOutput;
use bridge_types::H160;
use common::{balance, AssetInfoProvider, XOR};
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::dispatch::DispatchError;
use sp_keyring::AccountKeyring as Keyring;

fn last_event() -> RuntimeEvent {
    System::events().pop().expect("Event expected").event
}

#[test]
fn mints_after_handling_ethereum_event() {
    new_tester().execute_with(|| {
        let peer_contract = H160::default();
        let sender = H160::repeat_byte(7);
        let recipient: AccountId = Keyring::Bob.into();
        let amount = balance!(10);
        let old_balance = Assets::total_balance(&XOR, &recipient).unwrap();
        assert_ok!(EthApp::mint(
            dispatch::RawOrigin::new(CallOriginOutput {
                network_id: BASE_NETWORK_ID,
                additional: AdditionalEVMInboundData {
                    source: peer_contract,
                },
                ..Default::default()
            })
            .into(),
            sender,
            recipient.clone(),
            amount.into()
        ));
        assert_eq!(
            Assets::total_balance(&XOR, &recipient).unwrap(),
            old_balance + amount
        );

        assert_eq!(
            RuntimeEvent::EthApp(crate::Event::<Test>::Minted(
                BASE_NETWORK_ID,
                sender,
                recipient,
                amount
            )),
            last_event()
        );
    });
}

#[test]
fn mint_zero_amount_must_fail() {
    new_tester().execute_with(|| {
        let peer_contract = H160::default();
        let sender = H160::repeat_byte(7);
        let recipient: AccountId = Keyring::Bob.into();
        let amount = balance!(0);
        assert_noop!(
            EthApp::mint(
                dispatch::RawOrigin::new(CallOriginOutput {
                    network_id: BASE_NETWORK_ID,
                    additional: AdditionalEVMInboundData {
                        source: peer_contract,
                    },
                    ..Default::default()
                })
                .into(),
                sender,
                recipient.clone(),
                amount.into()
            ),
            Error::<Test>::WrongAmount
        );
    });
}

#[test]
fn burn_should_emit_bridge_event() {
    new_tester().execute_with(|| {
        let recipient = H160::repeat_byte(2);
        let bob: AccountId = Keyring::Bob.into();
        let amount = balance!(20);
        assert_ok!(Assets::mint_to(&XOR, &bob, &bob, balance!(500)));

        assert_ok!(EthApp::burn(
            RuntimeOrigin::signed(bob.clone()),
            BASE_NETWORK_ID,
            recipient.clone(),
            amount.into()
        ));

        assert_eq!(
            RuntimeEvent::EthApp(crate::Event::<Test>::Burned(
                BASE_NETWORK_ID,
                bob,
                recipient,
                amount
            )),
            last_event()
        );
    });
}

#[test]
fn should_not_burn_on_commitment_failure() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Eve.into();
        let recipient = H160::repeat_byte(9);
        let amount = balance!(20);

        assert_ok!(Assets::mint_to(
            &XOR,
            &Keyring::Bob.to_account_id(),
            &sender,
            balance!(500)
        ));

        assert_noop!(
            EthApp::burn(
                RuntimeOrigin::signed(sender.clone()),
                BASE_NETWORK_ID,
                recipient.clone(),
                amount
            ),
            DispatchError::Other("some error!")
        );
    });
}

#[test]
fn should_not_burn_zero_amount() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Eve.into();
        let recipient = H160::repeat_byte(9);
        let amount = balance!(0);

        assert_noop!(
            EthApp::burn(
                RuntimeOrigin::signed(sender.clone()),
                BASE_NETWORK_ID,
                recipient.clone(),
                amount
            ),
            Error::<Test>::WrongAmount
        );
    });
}

#[test]
fn test_register_network() {
    new_tester().execute_with(|| {
        assert!(!Addresses::<Test>::contains_key(BASE_NETWORK_ID + 1));
        assert_ok!(EthApp::register_network_with_existing_asset(
            RuntimeOrigin::root(),
            BASE_NETWORK_ID + 1,
            XOR,
            H160::repeat_byte(12),
            18
        ));
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID + 1));
    });
}

#[test]
fn test_existing_register_network() {
    new_tester().execute_with(|| {
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID));
        assert_noop!(
            EthApp::register_network_with_existing_asset(
                RuntimeOrigin::root(),
                BASE_NETWORK_ID,
                XOR,
                H160::repeat_byte(12),
                18
            ),
            Error::<Test>::AppAlreadyExists
        );
        assert!(Addresses::<Test>::contains_key(BASE_NETWORK_ID));
    });
}
