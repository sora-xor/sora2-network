use crate::types::{Address, H256, U256};
use crate::{AssetIdOf, AssetKind, Error, IncomingAsset, Trait};
use codec::{Decode, Encode};
use common::prelude::Balance;
use ethabi::Token;
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::{
    dispatch::DispatchError, ensure, sp_runtime::FixedPointNumber, RuntimeDebug, StorageMap,
    StorageValue,
};
use sp_std::prelude::*;

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct IncomingTransfer<T: Trait> {
    pub from: Address,
    pub to: T::AccountId,
    pub incoming_asset: IncomingAsset<T>,
    pub amount: Balance,
    pub tx_hash: sp_core::H256,
    pub at_height: u64,
}

impl<T: Trait> IncomingTransfer<T> {
    pub fn finalize(self) -> Result<sp_core::H256, DispatchError> {
        let (asset_id, asset_kind) = match self.incoming_asset {
            IncomingAsset::Loaded(asset_id, asset_kind) => (asset_id, asset_kind),
            IncomingAsset::ToRegister(addr, precision, symbol) => {
                if let Ok(asset) =
                    crate::Module::<T>::register_sidechain_asset(addr, precision, symbol)
                        .map(|asset_id| (asset_id, AssetKind::Sidechain))
                {
                    asset
                } else {
                    crate::Module::<T>::get_asset_by_raw_asset_id(H256::zero(), &addr)?
                        .ok_or(Error::<T>::FailedToGetAssetById)?
                }
            }
        };

        let bridge_account_id = crate::Module::<T>::bridge_account();
        match asset_kind {
            AssetKind::Thischain | AssetKind::SidechainOwned => {
                assets::Module::<T>::ensure_can_withdraw(
                    &asset_id,
                    &bridge_account_id,
                    self.amount,
                )?;
                assets::Module::<T>::transfer_from(
                    &asset_id,
                    &bridge_account_id,
                    &self.to,
                    self.amount,
                )?;
            }
            AssetKind::Sidechain => {
                assets::Module::<T>::mint_to(&asset_id, &bridge_account_id, &self.to, self.amount)?;
            }
        }
        Ok(self.tx_hash)
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct OutgoingTransfer<T: Trait> {
    pub from: T::AccountId,
    pub to: Address,
    pub asset_id: AssetIdOf<T>,
    pub amount: Balance,
    pub nonce: T::Index,
}

impl<T: Trait> OutgoingTransfer<T> {
    pub fn to_eth_abi(
        &self,
        tx_hash: sp_core::H256,
    ) -> Result<OutgoingTransferEthEncoded, Error<T>> {
        let from = Address::from_slice(&self.from.encode()[..20]);
        let to = self.to;
        let currency_id;
        if let Some(token_address) = crate::Module::<T>::registered_sidechain_token(&self.asset_id)
        {
            currency_id = CurrencyIdEncoded::TokenAddress(token_address);
        } else {
            let x: sp_core::H256 = self.asset_id.into();
            currency_id = CurrencyIdEncoded::AssetId(H256(x.0));
        }
        let amount = U256::from(self.amount.0.into_inner());
        let tx_hash = H256(tx_hash.0);
        let raw = ethabi::encode_packed(&[
            currency_id.to_token(),
            Token::Uint(amount),
            Token::Address(to.clone()),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::Address(from.clone()),
        ]);
        Ok(OutgoingTransferEthEncoded {
            from,
            to,
            currency_id,
            amount,
            tx_hash,
            raw,
        })
    }

    pub fn prepare(&mut self) -> Result<(), DispatchError> {
        assets::Module::<T>::ensure_can_withdraw(&self.asset_id, &self.from, self.amount)?;
        assets::Module::<T>::transfer_from(
            &self.asset_id,
            &self.from,
            &crate::Module::<T>::bridge_account(),
            self.amount,
        )?;
        let bridge_account = crate::BridgeAccount::<T>::get();
        assets::Module::<T>::reserve(self.asset_id, &bridge_account, self.amount)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        ensure!(
            crate::RegisteredAsset::<T>::get(&self.asset_id).is_some(),
            Error::<T>::UnsupportedToken
        );
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        self.validate()?;
        if let Some(AssetKind::Sidechain) = crate::Module::<T>::registered_asset(&self.asset_id) {
            let bridge_acc = &crate::Module::<T>::bridge_account();
            assets::Module::<T>::unreserve(self.asset_id, bridge_acc, self.amount)?;
            assets::Module::<T>::burn_from(&self.asset_id, bridge_acc, bridge_acc, self.amount)?;
        }
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        let bridge_account = crate::BridgeAccount::<T>::get();
        assets::Module::<T>::unreserve(self.asset_id, &bridge_account, self.amount)?;
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub enum CurrencyIdEncoded {
    AssetId(H256),
    TokenAddress(Address),
}

impl CurrencyIdEncoded {
    pub fn to_token(&self) -> Token {
        match self {
            CurrencyIdEncoded::AssetId(asset_id) => Token::FixedBytes(asset_id.encode()),
            CurrencyIdEncoded::TokenAddress(address) => Token::Address(address.clone()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub struct OutgoingTransferEthEncoded {
    pub currency_id: CurrencyIdEncoded,
    pub amount: U256,
    pub to: Address,
    pub tx_hash: H256,
    pub from: Address,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}
