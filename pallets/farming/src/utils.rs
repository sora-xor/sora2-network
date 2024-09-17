use codec::Decode;
use hex_literal::hex;

use crate::Config;

#[allow(dead_code)]
pub fn account<T: Config>(shift: u32) -> T::AccountId {
    let mut bytes = hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d");
    for _ in 0..shift {
        let mut shifted = false;
        let mut byte_index = bytes.len() - 1;
        while !shifted {
            if bytes[byte_index] != 0x00 {
                bytes[byte_index] -= 1;
                shifted = true;
            } else {
                byte_index -= 1;
            }
        }
    }
    T::AccountId::decode(&mut &bytes[..]).expect("Failed to decode account ID")
}
