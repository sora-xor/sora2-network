use crate::Trait;
use core::convert::TryFrom;
use frame_system::offchain::SigningTypes;
use iroha_client_no_std::crypto as iroha_crypto;
use parity_scale_codec::{Decode, Encode};

macro_rules! dbg {
    () => {
        debug::info!("[{}]", $crate::line!());
    };
    ($val:expr) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                debug::info!("[{}] {} = {:#?}",
                    $crate::line!(), $crate::stringify!($val), &tmp);
                tmp
            }
        }
    };
    // Trailing comma with single argument is ignored
    ($val:expr,) => { debug::info!($val) };
    ($($val:expr),+ $(,)?) => {
        ($(debug::info!($val)),+,)
    };
}

pub fn substrate_sig_to_iroha_sig<T: Trait>(
    (pk, sig): (T::Public, <T as SigningTypes>::Signature),
) -> iroha_crypto::Signature {
    let public_key = iroha_crypto::PublicKey::try_from(pk.encode()[1..].to_vec()).unwrap();
    let sig_bytes = sig.encode();
    let mut signature = [0u8; 64];
    signature.copy_from_slice(&sig_bytes[1..]);
    iroha_crypto::Signature {
        public_key,
        signature,
    }
}

pub fn iroha_sig_to_substrate_sig<T: Trait>(
    iroha_crypto::Signature {
        public_key,
        mut signature,
    }: iroha_crypto::Signature,
) -> (T::Public, <T as SigningTypes>::Signature) {
    (
        <T::Public>::decode(&mut &(*public_key)[..]).unwrap(),
        <T as SigningTypes>::Signature::decode(&mut &signature[..]).unwrap(),
    )
}

pub fn substrate_account_id_from_iroha_pk<T: Trait>(
    public_key: &iroha_crypto::PublicKey,
) -> T::AccountId {
    <T::AccountId>::decode(&mut &(*public_key)[..]).unwrap()
}
