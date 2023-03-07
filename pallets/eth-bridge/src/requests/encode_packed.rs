// This file is part of the SORA network and Polkaswap app.

// Copyright (c) 2020, 2021, Polka Biome Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-4-Clause

// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:

// Redistributions of source code must retain the above copyright notice, this list
// of conditions and the following disclaimer.
// Redistributions in binary form must reproduce the above copyright notice, this
// list of conditions and the following disclaimer in the documentation and/or other
// materials provided with the distribution.
//
// All advertising materials mentioning features or use of this software must display
// the following acknowledgement: This product includes software developed by Polka Biome
// Ltd., SORA, and Polkaswap.
//
// Neither the name of the Polka Biome Ltd. nor the names of its contributors may be used
// to endorse or promote products derived from this software without specific prior written permission.

// THIS SOFTWARE IS PROVIDED BY Polka Biome Ltd. AS IS AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL Polka Biome Ltd. BE LIABLE FOR ANY
// DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS;
// OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
// STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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
        // The two cases below may work incorrectly on some inputs (e.g. multi-dimensional arrays), but we don't use them.
        RawToken(Token::FixedArray(ref tokens)) | RawToken(Token::Tuple(ref tokens)) => tokens
            .iter()
            .cloned()
            .flat_map(|t| encode_token_packed(&t.into()))
            .collect(),
        RawToken(Token::Array(ref tokens)) => ethabi::encode(tokens),
    }
}
