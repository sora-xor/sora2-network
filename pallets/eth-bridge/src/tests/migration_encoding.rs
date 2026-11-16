use super::mock::{ExtBuilder, Runtime};
use crate::requests::encode_packed::encode_packed;
use crate::requests::{OffchainRequest, OutgoingMigrate, OutgoingRequest};
use crate::{BridgeContractAddress, BridgeSignatureVersion, BridgeSignatureVersions, EthAddress};
use bridge_multisig::{BridgeTimepoint, MultiChainHeight};
use codec::Encode;
use ethabi::Token;
use sp_core::{H256, U256};

fn address(index: u32, salt: u8) -> EthAddress {
    let mut bytes = [salt; 20];
    bytes[16..].copy_from_slice(&index.to_be_bytes());
    bytes.into()
}

fn migration(
    count: usize,
    network_id: u32,
    salt: u8,
    new_signature_version: BridgeSignatureVersion,
) -> OutgoingMigrate<Runtime> {
    OutgoingMigrate {
        author: [salt; 32].into(),
        new_contract_address: address(0x0102_0304, salt.wrapping_add(1)),
        erc20_native_tokens: (0..count)
            .map(|index| match index % 11 {
                0 | 1 => EthAddress::zero(), // Preserve zero and duplicate addresses.
                _ => address(index as u32, salt.wrapping_add(2)),
            })
            .collect(),
        nonce: u64::from(salt),
        network_id,
        timepoint: BridgeTimepoint {
            height: MultiChainHeight::Thischain(u64::from(salt)),
            index: u32::from(salt),
        },
        new_signature_version,
    }
}

// Retain the former generic encoder as a small-input differential oracle only.
fn old_encoding(
    request: &OutgoingMigrate<Runtime>,
    version: BridgeSignatureVersion,
    contract_address: EthAddress,
    tx_hash: H256,
) -> Vec<u8> {
    let tokens = request
        .erc20_native_tokens
        .iter()
        .map(|address| Token::Address(address.0.into()))
        .collect();
    let network = U256::from(request.network_id).to_big_endian().to_vec();
    match version {
        BridgeSignatureVersion::V1 | BridgeSignatureVersion::V2 => encode_packed(&[
            Token::Address(contract_address.0.into()).into(),
            Token::Address(request.new_contract_address.0.into()).into(),
            Token::FixedBytes(tx_hash.0.to_vec()).into(),
            Token::Array(tokens).into(),
            Token::FixedBytes(network).into(),
        ]),
        BridgeSignatureVersion::V3 => ethabi::encode(&[
            Token::String("migrate".into()),
            Token::Address(contract_address.0.into()),
            Token::Address(request.new_contract_address.0.into()),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::Array(tokens),
            Token::FixedBytes(network),
        ]),
    }
}

#[test]
fn migration_versions_match_old_encoders_and_prepared_hashes() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        for (network, salt, target_version) in [
            (0, 0, BridgeSignatureVersion::V1),
            (1, 0x55, BridgeSignatureVersion::V2),
            (u32::MAX, 0xff, BridgeSignatureVersion::V3),
        ] {
            let contract = address(0xfedc_ba98, salt);
            BridgeContractAddress::<Runtime>::insert(network, contract);
            for version in [
                BridgeSignatureVersion::V1,
                BridgeSignatureVersion::V2,
                BridgeSignatureVersion::V3,
            ] {
                // Select the current signing version, independently of the version
                // the migration will activate after settlement.
                BridgeSignatureVersions::<Runtime>::insert(network, version);
                for count in [0, 1, 2, 31, 32, 64, 255, 256, 1024] {
                    let request = migration(count, network, salt, target_version);
                    let request_bytes = request.encode();
                    let before =
                        OffchainRequest::outgoing(OutgoingRequest::Migrate(request.clone()));
                    let tx_hash = H256::from(sp_io::hashing::blake2_256(&request_bytes));
                    let expected = old_encoding(&request, version, contract, tx_hash);
                    let encoded = request.to_eth_abi(tx_hash).unwrap();
                    assert_eq!(encoded.raw, expected, "network {network}, count {count}");
                    assert_eq!(
                        common::eth::prepare_message(&encoded.raw).serialize(),
                        common::eth::prepare_message(&expected).serialize(),
                    );
                    assert_eq!(encoded.this_contract_address, contract);
                    assert_eq!(encoded.new_contract_address, request.new_contract_address);
                    assert_eq!(encoded.tx_hash, tx_hash);
                    assert_eq!(encoded.erc20_native_tokens, request.erc20_native_tokens);
                    assert_eq!(encoded.network_id.0, U256::from(network).to_big_endian());
                    assert_eq!(request.encode(), request_bytes);
                    let after = OffchainRequest::outgoing(OutgoingRequest::Migrate(request));
                    assert_eq!(after.hash(), before.hash());
                    assert_eq!(after.encode(), before.encode());
                }
            }
        }
    });
}

fn address_word(address: EthAddress) -> [u8; 32] {
    let mut word = [0; 32];
    word[12..].copy_from_slice(&address.0);
    word
}

#[test]
fn migration_versions_encode_full_block_address_envelope() {
    let (mut ext, _) = ExtBuilder::default().build();
    ext.execute_with(|| {
        let count = (7 * 1024 * 1024) / 20;
        assert_eq!(count, 367_001);
        let network = u32::MAX;
        let contract = address(0x1234_5678, 0xa5);
        let tx_hash = H256::repeat_byte(0x5a);
        BridgeContractAddress::<Runtime>::insert(network, contract);
        let request = migration(count, network, 0x7f, BridgeSignatureVersion::V1);
        for version in [
            BridgeSignatureVersion::V1,
            BridgeSignatureVersion::V2,
            BridgeSignatureVersion::V3,
        ] {
            BridgeSignatureVersions::<Runtime>::insert(network, version);
            let encoded = request.to_eth_abi(tx_hash).unwrap();
            let bytes = &encoded.raw;
            let array = match version {
                BridgeSignatureVersion::V1 | BridgeSignatureVersion::V2 => {
                    assert_eq!(bytes.len(), 104 + 32 * count);
                    assert_eq!(bytes.len(), 11_744_136);
                    assert_eq!(&bytes[..20], &contract.0);
                    assert_eq!(&bytes[20..40], &request.new_contract_address.0);
                    assert_eq!(&bytes[40..72], &tx_hash.0);
                    assert_eq!(
                        &bytes[bytes.len() - 32..],
                        &U256::from(network).to_big_endian()
                    );
                    &bytes[72..bytes.len() - 32]
                }
                BridgeSignatureVersion::V3 => {
                    assert_eq!(bytes.len(), 288 + 32 * count);
                    assert_eq!(bytes.len(), 11_744_320);
                    assert_eq!(&bytes[0..32], &U256::from(192u32).to_big_endian());
                    assert_eq!(&bytes[32..64], &address_word(contract));
                    assert_eq!(&bytes[64..96], &address_word(request.new_contract_address));
                    assert_eq!(&bytes[96..128], &tx_hash.0);
                    assert_eq!(&bytes[128..160], &U256::from(256u32).to_big_endian());
                    assert_eq!(&bytes[160..192], &U256::from(network).to_big_endian());
                    assert_eq!(&bytes[192..224], &U256::from(7u32).to_big_endian());
                    assert_eq!(&bytes[224..231], b"migrate");
                    assert_eq!(&bytes[231..256], &[0; 25]);
                    assert_eq!(&bytes[256..288], &U256::from(count as u64).to_big_endian());
                    &bytes[288..]
                }
            };
            assert_eq!(array.len(), count * 32);
            for (word, address) in array.chunks_exact(32).zip(&request.erc20_native_tokens) {
                assert_eq!(word, &address_word(*address));
            }
            assert_eq!(encoded.erc20_native_tokens, request.erc20_native_tokens);
        }
    });
}
