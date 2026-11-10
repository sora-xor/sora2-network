// Deterministic synthetic ownership proof, never an operator/private user key.
use ed25519_dalek_iroha::{Digest, Keypair, PublicKey, SecretKey};
use sha3::Sha3_256;
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() {
    let secret = SecretKey::from_bytes(&[71; 32]).unwrap();
    let key = Keypair {
        public: PublicKey::from(&secret),
        secret,
    };
    let public = hex(key.public.as_bytes());
    let account = hex(&[77; 32]);
    let address = "paid-wasm-migration@sora";
    let message = format!("SORA2-IROHA-MIGRATION-V2\ngenesis_hash=0x7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5\niroha_address={address}\niroha_public_key={public}\nsubstrate_account=0x{account}");
    let mut hash = Sha3_256::default();
    hash.update(message.as_bytes());
    let signature = key.sign_prehashed(hash.clone(), None).unwrap();
    key.public.verify_prehashed(hash, None, &signature).unwrap();
    println!("{{\"synthetic\":true,\"account\":\"0x{account}\",\"address\":\"{address}\",\"publicKey\":\"{public}\",\"signature\":\"{}\"}}", hex(&signature.to_bytes()));
}
