use secp256k1::{Message, PublicKey};
use sp_core::H160;
use sp_io::hashing::keccak_256;

pub type EthereumAddress = H160;

pub fn public_key_to_eth_address(pub_key: &PublicKey) -> EthereumAddress {
    let hash = keccak_256(&pub_key.serialize()[1..]);
    EthereumAddress::from_slice(&hash[12..])
}

pub fn prepare_message(msg: &[u8]) -> Message {
    let msg = keccak_256(msg);
    let mut prefix = b"\x19Ethereum Signed Message:\n32".to_vec();
    prefix.extend(&msg);
    let hash = keccak_256(&prefix);
    Message::parse_slice(&hash).expect("hash size == 256 bits; qed")
}
