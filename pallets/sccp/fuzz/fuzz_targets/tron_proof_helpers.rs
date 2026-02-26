#![no_main]

use libfuzzer_sys::fuzz_target;
use sccp::tron_proof::{
    block_id_from_raw_hash, parse_tron_header_raw, raw_data_hash, recover_eth_address_from_sig,
};

fuzz_target!(|data: &[u8]| {
    let _ = parse_tron_header_raw(data);

    let raw_hash = raw_data_hash(data);

    let mut number_bytes = [0u8; 8];
    let number_copy = core::cmp::min(8, data.len());
    number_bytes[..number_copy].copy_from_slice(&data[..number_copy]);
    let number = u64::from_le_bytes(number_bytes);
    let _ = block_id_from_raw_hash(number, &raw_hash);

    let mut msg_hash = [0u8; 32];
    let msg_start = 8usize.min(data.len());
    let msg_end = data.len().min(msg_start.saturating_add(32));
    let msg_copy = msg_end.saturating_sub(msg_start);
    msg_hash[..msg_copy].copy_from_slice(&data[msg_start..msg_end]);

    let mut sig65 = [0u8; 65];
    let sig_start = msg_end;
    let sig_end = data.len().min(sig_start.saturating_add(65));
    let sig_copy = sig_end.saturating_sub(sig_start);
    sig65[..sig_copy].copy_from_slice(&data[sig_start..sig_end]);

    let _ = recover_eth_address_from_sig(&msg_hash, &sig65, &sccp::SECP256K1N_HALF_ORDER);
});
