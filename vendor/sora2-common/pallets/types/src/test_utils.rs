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

use core::marker::PhantomData;
use std::fs::File;
use std::path::Path;

use crate::{traits::BridgeAssetLocker, GenericNetworkId, H128, H256, H512};
use serde::{Deserialize, Deserializer};
use sp_runtime::{traits::Hash, AccountId32, DispatchResult};

#[derive(Clone)]
pub struct Hex(pub Vec<u8>);

impl<'de> Deserialize<'de> for Hex {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        let mut s = <String as Deserialize>::deserialize(deserializer)?;
        if s.starts_with("0x") {
            s = s[2..].to_string();
        }
        if s.len() % 2 == 1 {
            s.insert(0, '0');
        }
        let v: Vec<u8> = hex::FromHexIter::new(&s).map(|x| x.unwrap()).collect();
        Ok(Hex(v))
    }
}

impl From<&Hex> for H256 {
    fn from(item: &Hex) -> Self {
        let mut data = [0u8; 32];
        let size = item.0.len();
        for i in 0..size {
            data[31 - i] = item.0[size - 1 - i];
        }
        data.into()
    }
}

impl From<&Hex> for H128 {
    fn from(item: &Hex) -> Self {
        let mut data = [0u8; 16];
        let size = item.0.len();
        for i in 0..size {
            data[15 - i] = item.0[size - 1 - i];
        }
        data.into()
    }
}

#[derive(Deserialize)]
struct BlockWithProofsRaw {
    pub proof_length: u64,
    pub header_rlp: Hex,
    pub merkle_root: Hex,        // H128
    pub elements: Vec<Hex>,      // H256
    pub merkle_proofs: Vec<Hex>, // H128
}

pub struct BlockWithProofs {
    pub proof_length: u64,
    pub header_rlp: Hex,
    pub merkle_root: H128,
    pub elements: Vec<H256>,
    pub merkle_proofs: Vec<H128>,
}

impl From<BlockWithProofsRaw> for BlockWithProofs {
    fn from(item: BlockWithProofsRaw) -> Self {
        Self {
            proof_length: item.proof_length,
            header_rlp: item.header_rlp,
            merkle_root: (&item.merkle_root).into(),
            elements: item.elements.iter().map(|e| e.into()).collect(),
            merkle_proofs: item.merkle_proofs.iter().map(|e| e.into()).collect(),
        }
    }
}

impl BlockWithProofs {
    pub fn from_file(path: &Path) -> Self {
        let raw: BlockWithProofsRaw = serde_json::from_reader(File::open(path).unwrap()).unwrap();
        raw.into()
    }

    fn combine_dag_h256_to_h512(elements: Vec<H256>) -> Vec<H512> {
        elements
            .iter()
            .zip(elements.iter().skip(1))
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, (a, b))| {
                let mut buffer = [0u8; 64];
                buffer[..32].copy_from_slice(&(a.0));
                buffer[32..].copy_from_slice(&(b.0));
                buffer.into()
            })
            .collect()
    }

    pub fn to_double_node_with_merkle_proof_vec<T>(
        &self,
        mapper: fn([H512; 2], Vec<H128>) -> T,
    ) -> Vec<T> {
        let h512s = Self::combine_dag_h256_to_h512(self.elements.clone());
        h512s
            .iter()
            .zip(h512s.iter().skip(1))
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(i, (a, b))| {
                mapper(
                    [*a, *b],
                    self.merkle_proofs[i / 2 * self.proof_length as usize
                        ..(i / 2 + 1) * self.proof_length as usize]
                        .to_vec(),
                )
            })
            .collect()
    }
}

pub struct BridgeAssetLockerImpl<T>(PhantomData<T>);

impl<T> BridgeAssetLockerImpl<T> {
    pub fn bridge_account(network_id: GenericNetworkId) -> AccountId32 {
        let hash = sp_runtime::traits::BlakeTwo256::hash_of(&(b"bridge-lock-account", &network_id));
        AccountId32::new(hash.0)
    }
    pub fn bridge_fee_account(network_id: GenericNetworkId) -> AccountId32 {
        let hash = sp_runtime::traits::BlakeTwo256::hash_of(&(b"bridge-fee-account", &network_id));
        AccountId32::new(hash.0)
    }
}

impl<T: traits::MultiCurrency<AccountId32>> BridgeAssetLocker<AccountId32>
    for BridgeAssetLockerImpl<T>
where
    T::Balance: frame_support::Parameter
        + sp_runtime::traits::AtLeast32BitUnsigned
        + sp_runtime::traits::MaybeSerializeDeserialize,
{
    type AssetId = T::CurrencyId;
    type Balance = T::Balance;

    fn lock_asset(
        network_id: crate::GenericNetworkId,
        asset_kind: crate::types::AssetKind,
        who: &AccountId32,
        asset_id: &T::CurrencyId,
        amount: &T::Balance,
    ) -> DispatchResult {
        match asset_kind {
            crate::types::AssetKind::Thischain => {
                let bridge_acc = Self::bridge_account(network_id);
                T::transfer(*asset_id, who, &bridge_acc, *amount)?;
            }
            crate::types::AssetKind::Sidechain => {
                T::withdraw(*asset_id, who, *amount)?;
            }
        }
        Ok(())
    }

    fn unlock_asset(
        network_id: crate::GenericNetworkId,
        asset_kind: crate::types::AssetKind,
        who: &AccountId32,
        asset_id: &T::CurrencyId,
        amount: &T::Balance,
    ) -> frame_support::dispatch::DispatchResult {
        match asset_kind {
            crate::types::AssetKind::Thischain => {
                let bridge_acc = Self::bridge_account(network_id);
                T::transfer(*asset_id, &bridge_acc, who, *amount)?;
            }
            crate::types::AssetKind::Sidechain => {
                T::deposit(*asset_id, who, *amount)?;
            }
        }
        Ok(())
    }

    fn refund_fee(
        network_id: GenericNetworkId,
        who: &AccountId32,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> frame_support::dispatch::DispatchResult {
        let bridge_acc = Self::bridge_fee_account(network_id);
        T::transfer(*asset_id, &bridge_acc, who, *amount)?;
        Ok(())
    }

    fn withdraw_fee(
        network_id: GenericNetworkId,
        who: &AccountId32,
        asset_id: &Self::AssetId,
        amount: &Self::Balance,
    ) -> frame_support::dispatch::DispatchResult {
        let bridge_acc = Self::bridge_fee_account(network_id);
        T::transfer(*asset_id, who, &bridge_acc, *amount)?;
        Ok(())
    }
}
