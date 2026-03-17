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

#[allow(unused_imports)]
pub use ethereum_types::{
    BigEndianHash, Bloom as H2048, H128, H160, H256, H512, H520, H64, U128, U256, U64,
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    type Res = Result<U256, serde_json::Error>;

    #[test]
    fn should_compare_correctly() {
        let mut arr = [0u8; 32];
        arr[31] = 0;
        arr[30] = 15;
        arr[29] = 1;
        arr[28] = 0;
        arr[27] = 10;
        let a = U256::from(arr.as_ref());
        arr[27] = 9;
        let b = U256::from(arr.as_ref());
        let c = U256::from(0);
        let d = U256::from(10_000);

        assert!(b < a);
        assert!(d < a);
        assert!(d < b);
        assert!(c < a);
        assert!(c < b);
        assert!(c < d);
    }

    #[test]
    fn should_display_correctly() {
        let mut arr = [0u8; 32];
        arr[31] = 0;
        arr[30] = 15;
        arr[29] = 1;
        arr[28] = 0;
        arr[27] = 10;
        let a = U256::from(arr.as_ref());
        let b = U256::from(1023);
        let c = U256::from(0);
        let d = U256::from(10000);

        // Debug
        assert_eq!(&format!("{:?}", a), "42949742336");
        assert_eq!(&format!("{:?}", b), "1023");
        assert_eq!(&format!("{:?}", c), "0");
        assert_eq!(&format!("{:?}", d), "10000");

        // Display
        assert_eq!(&format!("{}", a), "42949742336");
        assert_eq!(&format!("{}", b), "1023");
        assert_eq!(&format!("{}", c), "0");
        assert_eq!(&format!("{}", d), "10000");

        // Lowerhex
        assert_eq!(&format!("{:x}", a), "a00010f00");
        assert_eq!(&format!("{:x}", b), "3ff");
        assert_eq!(&format!("{:x}", c), "0");
        assert_eq!(&format!("{:x}", d), "2710");

        // Prefixed Lowerhex
        assert_eq!(&format!("{:#x}", a), "0xa00010f00");
        assert_eq!(&format!("{:#x}", b), "0x3ff");
        assert_eq!(&format!("{:#x}", c), "0x0");
        assert_eq!(&format!("{:#x}", d), "0x2710");
    }

    #[test]
    fn should_display_hash_correctly() {
        let mut arr = [0; 16];
        arr[15] = 0;
        arr[14] = 15;
        arr[13] = 1;
        arr[12] = 0;
        arr[11] = 10;
        let a = H128::from_uint(&arr.into());
        let b = H128::from_uint(&1023.into());
        let c = H128::from_uint(&0.into());
        let d = H128::from_uint(&10000.into());

        // Debug
        assert_eq!(&format!("{:?}", a), "0x00000000000000000000000a00010f00");
        assert_eq!(&format!("{:?}", b), "0x000000000000000000000000000003ff");
        assert_eq!(&format!("{:?}", c), "0x00000000000000000000000000000000");
        assert_eq!(&format!("{:?}", d), "0x00000000000000000000000000002710");

        // Display
        assert_eq!(&format!("{}", a), "0x0000…0f00");
        assert_eq!(&format!("{}", b), "0x0000…03ff");
        assert_eq!(&format!("{}", c), "0x0000…0000");
        assert_eq!(&format!("{}", d), "0x0000…2710");

        // Lowerhex
        assert_eq!(&format!("{:x}", a), "00000000000000000000000a00010f00");
        assert_eq!(&format!("{:x}", b), "000000000000000000000000000003ff");
        assert_eq!(&format!("{:x}", c), "00000000000000000000000000000000");
        assert_eq!(&format!("{:x}", d), "00000000000000000000000000002710");
    }

    #[test]
    fn should_deserialize_hash_correctly() {
        let deserialized1: H128 =
            serde_json::from_str(r#""0x00000000000000000000000a00010f00""#).unwrap();

        assert_eq!(deserialized1, H128::from_low_u64_be(0xa00010f00));
    }

    #[test]
    fn should_serialize_u256() {
        let serialized1 = serde_json::to_string(&U256::from(0)).unwrap();
        let serialized2 = serde_json::to_string(&U256::from(1)).unwrap();
        let serialized3 = serde_json::to_string(&U256::from(16)).unwrap();
        let serialized4 = serde_json::to_string(&U256::from(256)).unwrap();

        assert_eq!(serialized1, r#""0x0""#);
        assert_eq!(serialized2, r#""0x1""#);
        assert_eq!(serialized3, r#""0x10""#);
        assert_eq!(serialized4, r#""0x100""#);
    }

    #[test]
    fn should_fail_to_deserialize_decimals() {
        let deserialized1: Res = serde_json::from_str(r#""""#);
        let deserialized2: Res = serde_json::from_str(r#""0""#);
        let deserialized3: Res = serde_json::from_str(r#""10""#);
        let deserialized4: Res = serde_json::from_str(r#""1000000""#);
        let deserialized5: Res = serde_json::from_str(r#""1000000000000000000""#);

        assert!(deserialized1.is_err(), "{:?}", deserialized1);
        assert_eq!(deserialized2.unwrap(), U256::from(0u128));
        assert_eq!(deserialized3.unwrap(), U256::from(16u128));
        assert_eq!(deserialized4.unwrap(), U256::from(16777216u128));
        assert_eq!(
            deserialized5.unwrap(),
            U256::from(4722366482869645213696u128)
        );
    }

    #[test]
    fn should_deserialize_u256() {
        let deserialized1: Result<U256, _> = serde_json::from_str(r#""0x""#);
        let deserialized2: U256 = serde_json::from_str(r#""0x0""#).unwrap();
        let deserialized3: U256 = serde_json::from_str(r#""0x1""#).unwrap();
        let deserialized4: U256 = serde_json::from_str(r#""0x01""#).unwrap();
        let deserialized5: U256 = serde_json::from_str(r#""0x100""#).unwrap();

        assert!(deserialized1.is_err());
        assert_eq!(deserialized2, 0.into());
        assert_eq!(deserialized3, 1.into());
        assert_eq!(deserialized4, 1.into());
        assert_eq!(deserialized5, 256.into());
    }

    #[test]
    fn test_to_from_u64() {
        assert_eq!(1u64, U256::from(1u64).low_u64());
        assert_eq!(11u64, U256::from(11u64).low_u64());
        assert_eq!(111u64, U256::from(111u64).low_u64());
    }
}
