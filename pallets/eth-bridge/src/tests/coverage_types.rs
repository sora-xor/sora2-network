use super::{mock::*, Error};
use crate::types::{BlockId, BlockNumber, Bytes, FilterBuilder, Log, SubstrateBlockLimited, U64};
use crate::util::{serialize, to_string, Decoder};
use codec::Encode;
use common::mock::alice;
use ethabi::Token;
use ethereum_types::{H160, H256, U256};
use serde::Serialize;
use serde_json::json;

#[test]
fn rpc_bytes_require_even_prefixed_hex_and_roundtrip_empty_and_binary_data() {
    assert_eq!(
        serde_json::from_value::<Bytes>(json!("0x00ff")).unwrap(),
        Bytes(vec![0, 255])
    );
    for bytes in [vec![], vec![0, 1, 127, 128, 255]] {
        let value = Bytes::from(bytes);
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Bytes>(&encoded).unwrap(), value);
    }
    for invalid in [r#""ff""#, r#""0x0""#, r#""0xzz""#, "17", "null", "[]"] {
        assert!(serde_json::from_str::<Bytes>(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn block_identifiers_preserve_eip1898_and_named_block_semantics() {
    for (block, expected) in [
        (BlockNumber::Latest, json!("latest")),
        (BlockNumber::Earliest, json!("earliest")),
        (BlockNumber::Pending, json!("pending")),
        (BlockNumber::from(255u64), json!("0xff")),
    ] {
        assert_eq!(serialize(&block), expected);
        assert_eq!(serialize(&BlockId::from(block)), expected);
    }
    assert_eq!(serialize(&BlockId::from(U64::from(16))), json!("0x10"));
    let hash = H256::repeat_byte(0x42);
    assert_eq!(
        serialize(&BlockId::from(hash)),
        json!({"blockHash": format!("{hash:?}")})
    );
}

#[test]
fn log_removal_prefers_explicit_flag_over_legacy_type() {
    for (removed, kind, expected) in [
        (Some(true), None, true),
        (Some(false), Some("removed"), false),
        (None, Some("removed"), true),
        (None, Some("mined"), false),
        (None, None, false),
    ] {
        let log = Log {
            removed,
            log_type: kind.map(str::to_owned),
            ..Default::default()
        };
        assert_eq!(log.is_removed(), expected);
    }
}

#[test]
fn log_filters_keep_null_wildcards_and_collapse_single_addresses() {
    let a = H160::repeat_byte(1);
    let b = H160::repeat_byte(2);
    let topic = H256::repeat_byte(3);
    let filter = FilterBuilder::default()
        .address(vec![a, b])
        .topics(None, Some(vec![topic]), Some(vec![]), None)
        .limit(7)
        .build();
    assert_eq!(
        serialize(&filter),
        json!({"address": [a,b], "topics": [null,topic,null], "limit":7})
    );
    let filter = FilterBuilder::default()
        .address(vec![])
        .topics(None, None, None, None)
        .build();
    assert_eq!(serialize(&filter), json!({"address":null,"topics":[]}));
    assert_eq!(
        serialize(&FilterBuilder::default().address(vec![a]).build()),
        json!({"address":a})
    );
}

#[test]
fn opaque_extrinsics_validate_hex_and_scale_length() {
    let block: SubstrateBlockLimited = serde_json::from_value(json!({
        "header":{"number":"0x1"},"extrinsics":["0x1001020304"]
    }))
    .unwrap();
    assert_eq!(block.extrinsics[0].encode(), vec![1u8, 2, 3, 4].encode());
    assert_eq!(serialize(&block)["extrinsics"], json!(["0x1001020304"]));
    for invalid in [r#""nonsense""#, r#""0xzz""#, r#""0x10ff""#] {
        let encoded = format!(r#"{{"header":{{"number":"0x1"}},"extrinsics":[{invalid}]}}"#);
        assert!(serde_json::from_str::<SubstrateBlockLimited>(&encoded).is_err());
    }
}

#[test]
fn abi_decoder_checks_types_bounds_and_truncated_identifiers() {
    let mut d = Decoder::<Runtime>::new(vec![Token::String("asset".into())]);
    assert_eq!(d.next_string().unwrap(), "asset");
    assert!(d.is_empty());
    assert_eq!(d.next_string(), Err(Error::InvalidString));
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Bool(true)]).next_bool(),
        Ok(true)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![]).next_bool(),
        Err(Error::InvalidBool)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Uint(255.into())]).next_u8(),
        Ok(255)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Uint(256.into())]).next_u8(),
        Err(Error::InvalidByte)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Address(H160::repeat_byte(7))]).next_address(),
        Ok(sp_core::H160::repeat_byte(7))
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![]).next_address(),
        Err(Error::InvalidAddress)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Uint(u128::MAX.into())]).next_balance(),
        Ok(u128::MAX)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Uint(U256::MAX)]).next_balance(),
        Err(Error::InvalidBalance)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![]).next_balance(),
        Err(Error::InvalidUint)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Uint(U256::MAX)]).next_amount(),
        Ok(U256::MAX)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![]).next_amount(),
        Err(Error::InvalidUint)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::FixedBytes(alice().encode())]).next_account_id(),
        Ok(alice())
    );
    for token in [Token::Bool(false), Token::FixedBytes(vec![0; 31])] {
        assert_eq!(
            Decoder::<Runtime>::new(vec![token]).next_account_id(),
            Err(Error::InvalidAccountId)
        );
    }
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::FixedBytes(common::XOR.encode())]).next_asset_id(),
        Ok(common::XOR.into())
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::FixedBytes(vec![4; 32])]).next_h256(),
        Ok(sp_core::H256::repeat_byte(4))
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::FixedBytes(vec![4; 31])]).next_h256(),
        Err(Error::InvalidH256)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Array(vec![])]).next_array(),
        Ok(vec![])
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![]).next_array(),
        Err(Error::InvalidArray)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Array(vec![Token::Bool(false)])])
            .next_array_map(|d| d.next_h256()),
        Err(Error::InvalidH256)
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![]).next_signature_params(),
        Err(Error::InvalidArray)
    );
}

#[test]
fn precision_conversion_preserves_remainder_and_rejects_balance_overflow() {
    assert_eq!(EthBridge::convert_precision(18, 18, 123), Ok((123, 0)));
    assert_eq!(EthBridge::convert_precision(6, 8, 123), Ok((12300, 0)));
    assert_eq!(EthBridge::convert_precision(8, 6, 12345), Ok((123, 45)));
    assert_eq!(
        EthBridge::convert_precision(6, 18, u128::MAX),
        Err(Error::UnsupportedAssetPrecision)
    );
}

#[test]
fn rpc_serialization_errors_return_null_without_panicking() {
    struct Fails;
    impl Serialize for Fails {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("deliberate invalid RPC value"))
        }
    }
    assert_eq!(serialize(&Fails), json!(null));
    assert_eq!(to_string(&Fails), "null");
    assert_eq!(to_string(&vec![1, 2]), "[1,2]");
}

#[test]
fn abi_arrays_stop_at_their_boundary_and_decode_signature_components() {
    use crate::offchain::SignatureParams;
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Array(vec![])]).next_array_map(|d| d.next_u8()),
        Ok(vec![])
    );
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Array(vec![
            Token::Uint(1.into()),
            Token::Uint(2.into())
        ])])
        .next_array_map(|d| d.next_u8()),
        Ok(vec![2, 1])
    );
    let signature = |r, s, v| SignatureParams {
        r: [r; 32],
        s: [s; 32],
        v,
    };
    let signatures = vec![signature(1, 2, 27), signature(3, 4, 28)];
    let mut tokens = crate::requests::signature_params_to_tokens(signatures.clone());
    // The parser pops r, s and v from its argument stack.
    tokens.swap(1, 2);
    let mut decoder = Decoder::<Runtime>::new(tokens);
    assert_eq!(
        decoder.next_signature_params(),
        Ok(signatures.into_iter().rev().collect())
    );
    assert!(decoder.is_empty());
    assert_eq!(
        Decoder::<Runtime>::new(vec![Token::Array(vec![]); 3]).next_signature_params(),
        Ok(vec![])
    );
    for invalid in [
        vec![
            Token::Array(vec![]),
            Token::Bool(false),
            Token::Array(vec![]),
        ],
        vec![
            Token::Bool(false),
            Token::Array(vec![]),
            Token::Array(vec![]),
        ],
    ] {
        assert_eq!(
            Decoder::<Runtime>::new(invalid).next_signature_params(),
            Err(Error::InvalidArray)
        );
    }
    for (rs, ss, vs) in [(1, 0, 1), (1, 1, 0), (0, 1, 0)] {
        let tokens = vec![
            Token::Array(vec![Token::Uint(27.into()); vs]),
            Token::Array(vec![Token::FixedBytes(vec![2; 32]); ss]),
            Token::Array(vec![Token::FixedBytes(vec![1; 32]); rs]),
        ];
        assert_eq!(
            Decoder::<Runtime>::new(tokens).next_signature_params(),
            Err(Error::InvalidArray)
        );
    }
}

#[test]
fn peer_method_initialization_matches_contract_function_selectors() {
    let add = crate::contract::init_add_peer_by_peer_fn();
    let remove = crate::contract::init_remove_peer_by_peer_fn();
    assert_ne!(add, remove);
    assert_eq!(
        *add,
        crate::contract::ADD_PEER_BY_PEER_FN
            .get()
            .unwrap()
            .short_signature()
    );
    assert_eq!(
        *remove,
        crate::contract::REMOVE_PEER_BY_PEER_FN
            .get()
            .unwrap()
            .short_signature()
    );
}
