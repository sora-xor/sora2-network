use crate::mock::{new_tester, AccountId, Erc20App, Event, Origin, System, Test, BASE_NETWORK_ID};
use crate::TokenAddresses;
use bridge_types::types::ChannelId;
use common::{balance, XOR};
use frame_support::{assert_noop, assert_ok};
use sp_core::H160;
use sp_keyring::AccountKeyring as Keyring;
use traits::MultiCurrency;

fn last_event() -> Event {
    System::events().pop().expect("Event expected").event
}

#[test]
fn mints_after_handling_ethereum_event() {
    new_tester().execute_with(|| {
        let peer_contract = H160::repeat_byte(2);
        let asset_id = XOR;
        let token = TokenAddresses::<Test>::get(BASE_NETWORK_ID, asset_id).unwrap();
        let sender = H160::repeat_byte(3);
        let recipient: AccountId = Keyring::Charlie.into();
        let bob: AccountId = Keyring::Bob.into();
        let amount = balance!(10);

        <Test as assets::Config>::Currency::deposit(asset_id, &bob, balance!(500)).unwrap();
        assert_ok!(Erc20App::burn(
            Origin::signed(bob.clone()),
            BASE_NETWORK_ID,
            ChannelId::Incentivized,
            asset_id,
            H160::repeat_byte(9),
            amount
        ));

        assert_ok!(Erc20App::mint(
            dispatch::RawOrigin(BASE_NETWORK_ID, peer_contract).into(),
            token,
            sender,
            recipient.clone(),
            amount.into(),
        ));
        assert_eq!(
            <Test as assets::Config>::Currency::total_balance(asset_id, &recipient),
            amount.into()
        );

        assert_eq!(
            Event::Erc20App(crate::Event::<Test>::Minted(
                BASE_NETWORK_ID,
                asset_id,
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
        let asset_id = XOR;
        let recipient = H160::repeat_byte(2);
        let bob: AccountId = Keyring::Bob.into();
        let amount = balance!(20);
        <Test as assets::Config>::Currency::deposit(asset_id, &bob, balance!(500)).unwrap();

        assert_ok!(Erc20App::burn(
            Origin::signed(bob.clone()),
            BASE_NETWORK_ID,
            ChannelId::Incentivized,
            asset_id,
            recipient.clone(),
            amount
        ));

        assert_eq!(
            Event::Erc20App(crate::Event::<Test>::Burned(
                BASE_NETWORK_ID,
                asset_id,
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
        let asset_id = XOR;
        let sender: AccountId = Keyring::Bob.into();
        let recipient = H160::repeat_byte(9);
        let amount = balance!(20);

        <Test as assets::Config>::Currency::deposit(asset_id, &sender, balance!(500)).unwrap();

        for _ in 0..3 {
            let _ = Erc20App::burn(
                Origin::signed(sender.clone()),
                BASE_NETWORK_ID,
                ChannelId::Incentivized,
                asset_id,
                recipient.clone(),
                amount,
            )
            .unwrap();
        }

        assert_noop!(
            Erc20App::burn(
                Origin::signed(sender.clone()),
                BASE_NETWORK_ID,
                ChannelId::Incentivized,
                asset_id,
                recipient.clone(),
                amount
            ),
            incentivized_channel::outbound::Error::<Test>::QueueSizeLimitReached
        );
    });
}
