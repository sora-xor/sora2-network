use super::mock::{AssetId, ExtBuilder};
use common::balance;
use hex_literal::hex;
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_string();
    }

    "non-string panic payload".to_string()
}

fn expect_build_panic(builder: ExtBuilder) -> String {
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = builder.build();
    }))
    .expect_err("bridge genesis build should panic");

    panic_message(panic)
}

#[test]
fn genesis_requires_authority_account() {
    let panic = expect_build_panic(ExtBuilder::default().without_authority_account());

    assert!(panic.contains("EthBridge genesis authority account is not configured"));
}

#[test]
fn genesis_fails_when_bridge_permissions_are_already_assigned() {
    let panic = expect_build_panic(ExtBuilder::default().with_preseeded_bridge_permissions());

    assert!(panic.contains("EthBridge genesis failed to assign permission"));
}

#[test]
fn genesis_fails_when_reserve_asset_cannot_be_minted() {
    let reserve_asset = AssetId::from_bytes(hex!(
        "00998577153deb622b5d7faabf23846281a8b074e1d4eebd31bca9dbe2c23006"
    ));
    let mut builder = ExtBuilder::new().with_endowed_reserve_assets(false);
    builder.add_network(
        vec![],
        Some(vec![(reserve_asset, balance!(1))]),
        Some(4),
        Default::default(),
    );

    let panic = expect_build_panic(builder);

    assert!(panic.contains("EthBridge genesis failed to mint reserve"));
}
