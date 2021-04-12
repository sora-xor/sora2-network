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

use alloc::string::String;
use core::fmt;
use rustc_hex::{FromHex, ToHex};
use serde::de::{Error, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sp_std::borrow::ToOwned;
use sp_std::prelude::*;

/// Raw bytes wrapper
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bytes(pub Vec<u8>);

impl<T: Into<Vec<u8>>> From<T> for Bytes {
    fn from(data: T) -> Self {
        Bytes(data.into())
    }
}

impl Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serialized = "0x".to_owned();
        serialized.push_str(self.0.to_hex::<String>().as_ref());
        serializer.serialize_str(serialized.as_ref())
    }
}

impl<'a> Deserialize<'a> for Bytes {
    fn deserialize<D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'a>,
    {
        deserializer.deserialize_identifier(BytesVisitor)
    }
}

struct BytesVisitor;

impl<'a> Visitor<'a> for BytesVisitor {
    type Value = Bytes;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a 0x-prefixed hex-encoded vector of bytes")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        if let Some(val) = value.strip_prefix("0x") {
            let bytes =
                FromHex::from_hex(val).map_err(|e| Error::custom(format!("Invalid hex: {}", e)))?;
            Ok(Bytes(bytes))
        } else {
            Err(Error::invalid_value(Unexpected::Str(value), &"0x prefix"))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_str(value.as_ref())
    }
}
