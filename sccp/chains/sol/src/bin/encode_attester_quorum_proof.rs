use sccp_sol::{attest_hash, H256};
use std::env;

fn decode_hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn decode_hex_0x(s: &str) -> Result<Vec<u8>, String> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() % 2 != 0 {
        return Err("hex string must have even length".to_owned());
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..bytes.len()).step_by(2) {
        let hi = decode_hex_nibble(bytes[i]).ok_or("invalid hex")?;
        let lo = decode_hex_nibble(bytes[i + 1]).ok_or("invalid hex")?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn encode_compact_u32(n: u32) -> Result<Vec<u8>, String> {
    // SCALE compact encoding for values < 2^30.
    if n < (1 << 6) {
        return Ok(vec![((n << 2) as u8) | 0]);
    }
    if n < (1 << 14) {
        let v = (n << 2) | 1;
        return Ok(vec![(v & 0xff) as u8, ((v >> 8) & 0xff) as u8]);
    }
    if n < (1 << 30) {
        let v = (n << 2) | 2;
        return Ok(vec![
            (v & 0xff) as u8,
            ((v >> 8) & 0xff) as u8,
            ((v >> 16) & 0xff) as u8,
            ((v >> 24) & 0xff) as u8,
        ]);
    }
    Err("value too large for compact u32 encoding".to_owned())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn usage() -> ! {
    eprintln!(
        "Usage:\n  cargo run --bin encode_attester_quorum_proof -- --message-id 0x<32b> --sig 0x<65b> [--sig ...]"
    );
    std::process::exit(2);
}

fn main() {
    let mut message_id: Option<String> = None;
    let mut sigs: Vec<String> = Vec::new();

    let mut it = env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--message-id" => message_id = it.next(),
            "--sig" => sigs.push(it.next().unwrap_or_default()),
            "--help" | "-h" => usage(),
            _ => usage(),
        }
    }

    let message_id = message_id.unwrap_or_else(|| usage());
    if sigs.is_empty() {
        usage();
    }

    let msg_bytes = decode_hex_0x(&message_id).unwrap_or_else(|e| {
        eprintln!("invalid message id: {e}");
        std::process::exit(2);
    });
    if msg_bytes.len() != 32 {
        eprintln!("message id must be 32 bytes");
        std::process::exit(2);
    }
    let mut mid: H256 = [0u8; 32];
    mid.copy_from_slice(&msg_bytes);

    let ah = attest_hash(&mid);

    let mut proof: Vec<u8> = Vec::new();
    proof.push(1u8); // version
    proof.extend_from_slice(&encode_compact_u32(sigs.len() as u32).unwrap_or_else(|e| {
        eprintln!("encode compact failed: {e}");
        std::process::exit(2);
    }));
    for s in sigs.iter() {
        let sb = decode_hex_0x(s).unwrap_or_else(|e| {
            eprintln!("invalid signature: {e}");
            std::process::exit(2);
        });
        if sb.len() != 65 {
            eprintln!("signature must be 65 bytes");
            std::process::exit(2);
        }
        proof.extend_from_slice(&sb);
    }

    println!("{{");
    println!("  \"message_id\": \"{}\",", hex_encode(&mid));
    println!("  \"attest_hash\": \"{}\",", hex_encode(&ah));
    println!("  \"proof_hex\": \"{}\"", hex_encode(&proof));
    println!("}}");
}
