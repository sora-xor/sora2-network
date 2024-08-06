use rustc_hex::FromHex;
pub fn parse_hex_string(s: &str) -> Option<Vec<u8>> {
    s.strip_prefix("0x").and_then(|x| x.from_hex().ok())
}
