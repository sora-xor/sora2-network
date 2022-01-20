use crate::mock::{
    new_tester, AccountId, Assets, EthApp, Event, Origin, System, Test, BASE_NETWORK_ID,
};
use common::{balance, XOR};
use frame_support::dispatch::DispatchError;
use frame_support::{assert_noop, assert_ok};
use sp_core::H160;
use sp_keyring::AccountKeyring as Keyring;

use bridge_types::types::ChannelId;

fn last_event() -> Event {
    System::events().pop().expect("Event expected").event
}

#[test]
fn mints_after_handling_ethereum_event() {
    new_tester().execute_with(|| {
        let peer_contract = H160::repeat_byte(1);
        let sender = H160::repeat_byte(7);
        let recipient: AccountId = Keyring::Bob.into();
        let amount = balance!(10);
        let old_balance = Assets::total_balance(&XOR, &recipient).unwrap();
        assert_ok!(EthApp::mint(
            dispatch::RawOrigin(BASE_NETWORK_ID, peer_contract).into(),
            sender,
            recipient.clone(),
            amount.into()
        ));
        assert_eq!(
            Assets::total_balance(&XOR, &recipient).unwrap(),
            old_balance + amount
        );

        assert_eq!(
            Event::EthApp(crate::Event::<Test>::Minted(
                sender,
                recipient,
                amount.into()
            )),
            last_event()
        );
    });
}

#[test]
fn burn_should_emit_bridge_event() {
    new_tester().execute_with(|| {
        let recipient = H160::repeat_byte(2);
        let bob: AccountId = Keyring::Bob.into();
        assert_ok!(Assets::mint_to(&XOR, &bob, &bob, 500u32.into()));

        assert_ok!(EthApp::burn(
            Origin::signed(bob.clone()),
            ChannelId::Incentivized,
            BASE_NETWORK_ID,
            H160::repeat_byte(1),
            H160::repeat_byte(1),
            recipient.clone(),
            20u32.into()
        ));

        assert_eq!(
            Event::EthApp(crate::Event::<Test>::Burned(bob, recipient, 20.into())),
            last_event()
        );
    });
}

#[test]
fn should_not_burn_on_commitment_failure() {
    new_tester().execute_with(|| {
        let sender: AccountId = Keyring::Bob.into();
        let recipient = H160::repeat_byte(9);

        assert_ok!(Assets::mint_to(&XOR, &sender, &sender, 500u32.into()));

        assert_noop!(
            EthApp::burn(
                Origin::signed(sender.clone()),
                ChannelId::Basic,
                BASE_NETWORK_ID,
                H160::repeat_byte(1),
                H160::repeat_byte(1),
                recipient.clone(),
                20u32.into()
            ),
            DispatchError::Other("some error!")
        );
    });
}
