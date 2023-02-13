use ethabi::Bytes;
use ethabi::{Token, Uint};
use sp_std::prelude::*;

pub enum TokenWrapper {
    RawToken(Token),
    UintSized(Uint, usize),
    IntSized(Uint, usize),
}

impl From<Token> for TokenWrapper {
    fn from(token: Token) -> Self {
        TokenWrapper::RawToken(token)
    }
}

pub fn encode_packed(tokens: &[TokenWrapper]) -> Bytes {
    tokens.iter().flat_map(encode_token_packed).collect()
}

fn encode_token_packed(token: &TokenWrapper) -> Vec<u8> {
    use TokenWrapper::*;
    match *token {
        RawToken(Token::Address(ref address)) => address.as_ref().to_owned(),
        RawToken(Token::Bytes(ref bytes)) => bytes.to_owned(),
        RawToken(Token::String(ref s)) => s.as_bytes().to_owned(),
        RawToken(Token::FixedBytes(ref bytes)) => bytes.to_owned(),
        RawToken(Token::Int(int)) | RawToken(Token::Uint(int)) => <[u8; 32]>::from(int).to_vec(),
        IntSized(int, size) | UintSized(int, size) => {
            let size_bytes = size / 8;
            debug_assert_eq!(size_bytes * 8, size);
            let mut arr = vec![0u8; size_bytes];
            for i in 0..size_bytes {
                arr[size_bytes - i - 1] = int.byte(i);
            }
            arr
        }
        RawToken(Token::Bool(b)) => {
            vec![if b { 1 } else { 0 }]
        }
        // FIXME: the two cases below may work incorrectly on some inputs (e.g. multi-dimensional arrays).
        RawToken(Token::FixedArray(ref tokens)) | RawToken(Token::Tuple(ref tokens)) => tokens
            .iter()
            .cloned()
            .flat_map(|t| encode_token_packed(&t.into()))
            .collect(),
        RawToken(Token::Array(ref tokens)) => ethabi::encode(tokens),
    }
}
