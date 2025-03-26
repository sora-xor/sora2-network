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

use crate::offchain::SignatureParams;
use crate::{Config, Error, EthAddress, Pallet};
use alloc::string::String;
use codec::{Decode, FullCodec};
use common::prelude::Balance;
use common::BalancePrecision;
use core::convert::TryFrom;
use core::iter;
use ethabi::Token;
use ethereum_types::U256;
use frame_support::dispatch::{DispatchResult, PostDispatchInfo};
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::sp_runtime::DispatchErrorWithPostInfo;
use frame_support::{ensure, IterableStorageDoubleMap};
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use sp_core::{H160, H256};
use sp_std::marker::PhantomData;
use sp_std::prelude::*;

pub fn majority(peers_count: usize) -> usize {
    peers_count - (peers_count - 1) / 3
}

/// A helper for encoding bridge types into ethereum tokens.
#[derive(PartialEq)]
pub struct Decoder<T: Config> {
    tokens: Vec<Token>,
    _phantom: PhantomData<T>,
}

impl<T: Config> Decoder<T> {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            _phantom: PhantomData,
        }
    }

    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    #[allow(unused)]
    pub fn next_string(&mut self) -> Result<String, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_string())
            .ok_or_else(|| Error::<T>::InvalidString.into())
    }

    pub fn next_bool(&mut self) -> Result<bool, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_bool())
            .ok_or_else(|| Error::<T>::InvalidBool.into())
    }

    #[allow(unused)]
    pub fn next_u8(&mut self) -> Result<u8, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_uint())
            .filter(|x| x.as_u32() <= u8::MAX as u32)
            .map(|x| x.as_u32() as u8)
            .ok_or_else(|| Error::<T>::InvalidByte.into())
    }

    pub fn next_address(&mut self) -> Result<EthAddress, Error<T>> {
        Ok(H160(
            self.tokens
                .pop()
                .and_then(|x| x.into_address())
                .ok_or(Error::<T>::InvalidAddress)?
                .0,
        ))
    }

    #[allow(unused)]
    pub fn next_balance(&mut self) -> Result<Balance, Error<T>> {
        Ok(Balance::from(
            u128::try_from(
                self.tokens
                    .pop()
                    .and_then(|x| x.into_uint())
                    .ok_or(Error::<T>::InvalidUint)?,
            )
            .map_err(|_| Error::<T>::InvalidBalance)?,
        ))
    }

    pub fn next_amount(&mut self) -> Result<U256, Error<T>> {
        Ok(self
            .tokens
            .pop()
            .and_then(|x| x.into_uint())
            .ok_or(Error::<T>::InvalidUint)?)
    }

    pub fn next_account_id(&mut self) -> Result<T::AccountId, Error<T>> {
        Ok(T::AccountId::decode(
            &mut &self
                .tokens
                .pop()
                .and_then(|x| x.into_fixed_bytes())
                .ok_or(Error::<T>::InvalidAccountId)?[..],
        )
        .map_err(|_| Error::<T>::InvalidAccountId)?)
    }

    #[allow(unused)]
    pub fn next_asset_id(&mut self) -> Result<T::AssetId, Error<T>> {
        Ok(T::AssetId::decode(&mut &self.next_h256()?.0[..])
            .map_err(|_| Error::<T>::InvalidAssetId)?)
    }

    pub fn parse_h256(token: Token) -> Option<H256> {
        <[u8; 32]>::try_from(token.into_fixed_bytes()?)
            .ok()
            .map(H256)
    }

    pub fn next_h256(&mut self) -> Result<H256, Error<T>> {
        self.tokens
            .pop()
            .and_then(Self::parse_h256)
            .ok_or_else(|| Error::<T>::InvalidH256.into())
    }

    #[allow(unused)]
    pub fn next_array(&mut self) -> Result<Vec<Token>, Error<T>> {
        self.tokens
            .pop()
            .and_then(|x| x.into_array())
            .ok_or_else(|| Error::<T>::InvalidArray.into())
    }

    #[allow(unused)]
    pub fn next_array_map<U, F: FnMut(&mut Decoder<T>) -> Result<U, Error<T>>>(
        &mut self,
        mut f: F,
    ) -> Result<Vec<U>, Error<T>> {
        let mut decoder = Decoder::<T>::new(self.next_array()?);
        iter::repeat(())
            .map(|_| f(&mut decoder))
            .collect::<Result<Vec<_>, _>>()
    }

    #[allow(unused)]
    pub fn next_signature_params(&mut self) -> Result<Vec<SignatureParams>, Error<T>> {
        let rs = self.next_array_map(|d| d.next_h256().map(|x| x.0))?;
        let ss = self.next_array_map(|d| d.next_h256().map(|x| x.0))?;
        let vs = self.next_array_map(|d| d.next_u8())?;
        Ok(rs
            .into_iter()
            .zip(ss)
            .zip(vs)
            .map(|((r, s), v)| SignatureParams { r, s, v })
            .collect())
    }
}

pub fn get_bridge_account<T: Config>(network_id: T::NetworkId) -> T::AccountId {
    crate::BridgeAccount::<T>::get(network_id).expect("networks can't be removed; qed")
}

pub fn serialize<T: serde::Serialize>(t: &T) -> crate::jsonrpc::Value {
    serde_json::to_value(t).expect("Types never fail to serialize.")
}

#[allow(unused)]
pub fn to_string<T: serde::Serialize>(request: &T) -> String {
    serde_json::to_string(&request).expect("String serialization never fails.")
}

pub fn iter_storage<S, K1, K2, V, F, O>(k1: Option<K1>, f: F) -> Vec<O>
where
    K1: FullCodec + Copy,
    K2: FullCodec,
    V: FullCodec,
    S: IterableStorageDoubleMap<K1, K2, V>,
    F: FnMut((K1, K2, V)) -> O,
{
    if let Some(k1) = k1 {
        S::iter_prefix(k1)
            .map(|(k2, v)| (k1, k2, v))
            .map(f)
            .collect()
    } else {
        S::iter().map(f).collect()
    }
}

impl<T: Config> Pallet<T> {
    /// Checks if the account is a bridge peer.
    pub fn is_peer(who: &T::AccountId, network_id: T::NetworkId) -> bool {
        Self::peers(network_id).into_iter().any(|i| i == *who)
    }

    /// Ensures that the account is a bridge peer.
    pub(crate) fn ensure_peer(who: &T::AccountId, network_id: T::NetworkId) -> DispatchResult {
        ensure!(Self::is_peer(who, network_id), Error::<T>::Forbidden);
        Ok(())
    }

    /// Ensures that the account is a bridge multisig account.
    pub(crate) fn ensure_bridge_account(
        origin: OriginFor<T>,
        network_id: T::NetworkId,
    ) -> Result<T::AccountId, DispatchErrorWithPostInfo<PostDispatchInfo>> {
        let who = ensure_signed(origin)?;
        let bridge_account_id =
            Self::bridge_account(network_id).ok_or(Error::<T>::UnknownNetwork)?;
        ensure!(who == bridge_account_id, Error::<T>::Forbidden);
        Ok(bridge_account_id)
    }

    /// Converts amount from one precision to another and and returns it with a difference of the
    /// amounts. It also checks that no information was lost during multiplication, otherwise
    /// returns an error.
    pub fn convert_precision(
        precision_from: BalancePrecision,
        precision_to: BalancePrecision,
        amount: Balance,
    ) -> Result<(Balance, Balance), Error<T>> {
        if precision_from == precision_to {
            return Ok((amount, 0));
        }
        let pair = if precision_from < precision_to {
            let exp = (precision_to - precision_from) as u32;
            let coeff = 10_u128.pow(exp);
            let coerced_amount = amount.saturating_mul(coeff);
            ensure!(
                coerced_amount / coeff == amount,
                Error::<T>::UnsupportedAssetPrecision
            );
            (coerced_amount, 0)
        } else {
            let exp = (precision_from - precision_to) as u32;
            let coeff = 10_u128.pow(exp);
            let coerced_amount = amount / coeff;
            let diff = amount - coerced_amount * coeff;
            (coerced_amount, diff)
        };
        Ok(pair)
    }
}
