use crate::types::{Address, H256, U256};
use crate::{AssetIdOf, AssetKind, Error, Module, PswapOwners, Trait};
use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
};
use codec::{Decode, Encode};
use common::{prelude::Balance, PSWAP};
use common::{AssetSymbol, BalancePrecision};
use ethabi::{FixedBytes, Token};
#[allow(unused_imports)]
use frame_support::debug;
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::{dispatch::DispatchError, ensure, RuntimeDebug, StorageMap, StorageValue};
use frame_system::RawOrigin;
use sp_std::prelude::*;

pub const MIN_PEERS: usize = 4;
pub const MAX_PEERS: usize = 100;

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct IncomingAddToken<T: Trait> {
    pub token_address: Address,
    pub asset_id: T::AssetId,
    pub precision: BalancePrecision,
    pub symbol: AssetSymbol,
    pub tx_hash: sp_core::H256,
    pub at_height: u64,
}

impl<T: Trait> IncomingAddToken<T> {
    pub fn finalize(&self) -> Result<sp_core::H256, DispatchError> {
        crate::Module::<T>::register_sidechain_asset(
            self.token_address,
            self.precision,
            self.symbol.clone(),
        )?;
        Ok(self.tx_hash)
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct IncomingChangePeers<T: Trait> {
    pub peer_account_id: T::AccountId,
    pub peer_address: Address,
    pub added: bool,
    pub tx_hash: sp_core::H256,
    pub at_height: u64,
}

impl<T: Trait> IncomingChangePeers<T> {
    pub fn finalize(&self) -> Result<sp_core::H256, DispatchError> {
        let pending_peer = crate::PendingPeer::<T>::get().ok_or(Error::<T>::NoPendingPeer)?;
        ensure!(
            pending_peer == self.peer_account_id,
            Error::<T>::WrongPendingPeer
        );
        if self.added {
            let account_id = self.peer_account_id.clone();
            multisig::Module::<T>::add_signatory(
                RawOrigin::Signed(crate::BridgeAccount::<T>::get()).into(),
                account_id.clone(),
            )?;
            crate::Peers::<T>::mutate(|set| set.insert(account_id));
        }
        crate::PendingPeer::<T>::set(None);
        Ok(self.tx_hash)
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct IncomingTransfer<T: Trait> {
    pub from: Address,
    pub to: T::AccountId,
    pub asset_id: AssetIdOf<T>,
    pub asset_kind: AssetKind,
    pub amount: Balance,
    pub tx_hash: sp_core::H256,
    pub at_height: u64,
}

impl<T: Trait> IncomingTransfer<T> {
    pub fn prepare(&self) -> Result<(), DispatchError> {
        if self.asset_kind.is_owned() {
            let bridge_account = crate::BridgeAccount::<T>::get();
            assets::Module::<T>::reserve(self.asset_id, &bridge_account, self.amount)?;
        }
        Ok(())
    }

    pub fn unreserve(&self) {
        if self.asset_kind.is_owned() {
            let bridge_acc = &crate::Module::<T>::bridge_account();
            if let Err(e) = assets::Module::<T>::unreserve(self.asset_id, bridge_acc, self.amount) {
                debug::error!("Unpredictable error: {:?}", e);
            }
        }
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        self.unreserve();
        Ok(())
    }

    pub fn finalize(&self) -> Result<sp_core::H256, DispatchError> {
        let bridge_account_id = crate::Module::<T>::bridge_account();
        if self.asset_kind.is_owned() {
            self.unreserve();
            assets::Module::<T>::ensure_can_withdraw(
                &self.asset_id,
                &bridge_account_id,
                self.amount,
            )?;
            assets::Module::<T>::transfer_from(
                &self.asset_id,
                &bridge_account_id,
                &self.to,
                self.amount,
            )?;
        } else {
            assets::Module::<T>::mint_to(
                &self.asset_id,
                &bridge_account_id,
                &self.to,
                self.amount,
            )?;
        }
        Ok(self.tx_hash)
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct IncomingClaimPswap<T: Trait> {
    pub account_id: T::AccountId,
    pub eth_address: Address,
    pub tx_hash: sp_core::H256,
    pub at_height: u64,
}

impl<T: Trait> IncomingClaimPswap<T> {
    pub fn finalize(&self) -> Result<sp_core::H256, DispatchError> {
        let bridge_account_id = Module::<T>::bridge_account();
        let amount = PswapOwners::get(&self.eth_address).ok_or(Error::<T>::AccountNotFound)?;
        ensure!(!amount.is_zero(), Error::<T>::AlreadyClaimed);
        PswapOwners::insert(&self.eth_address, Balance::from(0u128));
        assets::Module::<T>::mint_to(&PSWAP.into(), &bridge_account_id, &self.account_id, amount)?;
        Ok(self.tx_hash.clone())
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
        if let Some(token_address) = Module::<T>::registered_sidechain_token(&self.asset_id) {
            currency_id = CurrencyIdEncoded::TokenAddress(token_address);
        } else {
            let x = <T::AssetId as Into<sp_core::H256>>::into(self.asset_id);
            currency_id = CurrencyIdEncoded::AssetId(H256(x.0));
        }
        let amount = U256::from(*self.amount.0.as_bits());
        let tx_hash = H256(tx_hash.0);
        let raw = ethabi::encode_packed(&[
            currency_id.to_token(),
            Token::Uint(amount),
            Token::Address(to),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::Address(from),
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
        let bridge_account = crate::BridgeAccount::<T>::get();
        assets::Module::<T>::transfer_from(
            &self.asset_id,
            &self.from,
            &bridge_account,
            self.amount,
        )?;
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
        if let Some(AssetKind::Sidechain) = Module::<T>::registered_asset(&self.asset_id) {
            let bridge_acc = &Module::<T>::bridge_account();
            assets::Module::<T>::unreserve(self.asset_id, bridge_acc, self.amount)?;
            assets::Module::<T>::burn_from(&self.asset_id, bridge_acc, bridge_acc, self.amount)?;
        }
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        let bridge_account = crate::BridgeAccount::<T>::get();
        assets::Module::<T>::unreserve(self.asset_id, &bridge_account, self.amount)?;
        assets::Module::<T>::transfer_from(
            &self.asset_id,
            &crate::Module::<T>::bridge_account(),
            &self.from,
            self.amount,
        )?;
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

// TODO: lock the adding token to prevent double-adding.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct AddAssetOutgoingRequest<T: Trait> {
    pub author: T::AccountId,
    pub asset_id: AssetIdOf<T>,
    pub nonce: T::Index,
}

impl<T: Trait> AddAssetOutgoingRequest<T> {
    pub fn to_eth_abi(&self, tx_hash: sp_core::H256) -> Result<AddAssetRequestEncoded, Error<T>> {
        let hash = H256(tx_hash.0);
        let name = "".to_string();
        let asset_id_code = <AssetIdOf<T> as Into<sp_core::H256>>::into(self.asset_id);
        let (symbol, precision) = assets::Module::<T>::get_asset_info(&self.asset_id);
        let symbol: String = String::from_utf8_lossy(&symbol.0).into();
        let supply: U256 = Default::default();
        let sidechain_asset_id = asset_id_code.0.to_vec();
        let raw = ethabi::encode_packed(&[
            Token::String(name.clone()),
            Token::String(symbol.clone()),
            Token::UintSized(precision.into(), 8),
            Token::Uint(supply.clone()),
            Token::FixedBytes(sidechain_asset_id.clone()),
        ]);
        Ok(AddAssetRequestEncoded {
            name,
            symbol,
            decimal: precision,
            supply, // TODO: supply
            sidechain_asset_id,
            hash,
            raw,
        })
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        ensure!(
            assets::Module::<T>::is_asset_owner(&self.asset_id, &self.author),
            Error::<T>::TokenIsNotOwnedByTheAuthor
        );
        ensure!(
            crate::RegisteredAsset::<T>::get(&self.asset_id).is_none(),
            Error::<T>::TokenIsAlreadyAdded
        );
        Ok(())
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        self.validate()?;
        // TODO: will it work?
        crate::RegisteredAsset::<T>::insert(&self.asset_id, AssetKind::Thischain);
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub struct AddAssetRequestEncoded {
    pub name: String,
    pub symbol: String,
    pub decimal: u8,
    pub supply: U256,
    pub sidechain_asset_id: FixedBytes,
    pub hash: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct AddTokenOutgoingRequest<T: Trait> {
    pub author: T::AccountId,
    pub token_address: Address,
    pub ticker: String,
    pub name: String,
    pub decimals: u8,
    pub nonce: T::Index,
}

impl<T: Trait> AddTokenOutgoingRequest<T> {
    pub fn to_eth_abi(&self, tx_hash: sp_core::H256) -> Result<AddTokenRequestEncoded, Error<T>> {
        let hash = H256(tx_hash.0);
        let token_address = self.token_address.clone();
        let ticker = self.ticker.clone();
        let name = self.name.clone();
        let decimals = self.decimals;
        let raw = ethabi::encode_packed(&[
            Token::Address(token_address),
            Token::String(ticker.clone()),
            Token::String(name.clone()),
            Token::UintSized(decimals.into(), 8),
        ]);
        Ok(AddTokenRequestEncoded {
            token_address,
            name,
            ticker,
            decimals,
            hash,
            raw,
        })
    }

    pub fn validate(&self) -> Result<AssetSymbol, DispatchError> {
        ensure!(
            crate::RegisteredSidechainAsset::<T>::get(&self.token_address).is_none(),
            Error::<T>::Other
        );
        let symbol = AssetSymbol(self.ticker.as_bytes().to_vec());
        ensure!(
            assets::is_symbol_valid(&symbol),
            assets::Error::<T>::InvalidAssetSymbol
        );
        Ok(symbol)
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        let symbol = self.validate()?;
        crate::Module::<T>::register_sidechain_asset(self.token_address, self.decimals, symbol)?;
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub struct AddTokenRequestEncoded {
    pub token_address: Address,
    pub ticker: String,
    pub name: String,
    pub decimals: u8,
    pub hash: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct AddPeerOutgoingRequest<T: Trait> {
    pub author: T::AccountId,
    pub peer_address: Address,
    pub peer_account_id: T::AccountId,
    pub nonce: T::Index,
}

impl<T: Trait> AddPeerOutgoingRequest<T> {
    pub fn to_eth_abi(
        &self,
        tx_hash: sp_core::H256,
    ) -> Result<AddPeerOutgoingRequestEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let peer_address = self.peer_address;
        let raw = ethabi::encode_packed(&[
            Token::Address(peer_address.clone()),
            Token::FixedBytes(tx_hash.0.to_vec()),
        ]);
        Ok(AddPeerOutgoingRequestEncoded {
            peer_address,
            tx_hash,
            raw,
        })
    }

    pub fn validate(&self) -> Result<BTreeSet<T::AccountId>, DispatchError> {
        let peers = crate::Peers::<T>::get();
        ensure!(peers.len() <= MAX_PEERS, Error::<T>::CantAddMorePeers);
        ensure!(
            !peers.contains(&self.peer_account_id),
            Error::<T>::UnknownPeerId
        );
        Ok(peers)
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        let pending_peer = crate::PendingPeer::<T>::get();
        ensure!(pending_peer.is_none(), Error::<T>::TooManyPendingPeers);
        crate::PendingPeer::<T>::set(Some(self.peer_account_id.clone()));
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        let _peers = self.validate()?;
        crate::PeerAccountId::<T>::insert(self.peer_address, self.peer_account_id.clone());
        crate::PeerAddress::<T>::insert(&self.peer_account_id, self.peer_address.clone());
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        crate::PendingPeer::<T>::set(None);
        Ok(())
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct RemovePeerOutgoingRequest<T: Trait> {
    pub author: T::AccountId,
    pub peer_account_id: T::AccountId,
    pub peer_address: Address,
    pub nonce: T::Index,
}

impl<T: Trait> RemovePeerOutgoingRequest<T> {
    pub fn to_eth_abi(
        &self,
        tx_hash: sp_core::H256,
    ) -> Result<RemovePeerOutgoingRequestEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let peer_address = self.peer_address;
        let raw = ethabi::encode_packed(&[
            Token::Address(peer_address.clone()),
            Token::FixedBytes(tx_hash.0.to_vec()),
        ]);
        Ok(RemovePeerOutgoingRequestEncoded {
            peer_address,
            tx_hash,
            raw,
        })
    }

    pub fn validate(&self) -> Result<BTreeSet<T::AccountId>, DispatchError> {
        let peers = crate::Peers::<T>::get();
        ensure!(peers.len() >= MIN_PEERS, Error::<T>::CantRemoveMorePeers);
        ensure!(
            peers.contains(&self.peer_account_id),
            Error::<T>::UnknownPeerId
        );
        Ok(peers)
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        let pending_peer = crate::PendingPeer::<T>::get();
        ensure!(pending_peer.is_none(), Error::<T>::TooManyPendingPeers);
        crate::PendingPeer::<T>::set(Some(self.peer_account_id.clone()));
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        let mut peers = self.validate()?;
        multisig::Module::<T>::remove_signatory(
            RawOrigin::Signed(crate::BridgeAccount::<T>::get()).into(),
            self.peer_account_id.clone(),
        )?;
        peers.remove(&self.peer_account_id);
        crate::Peers::<T>::set(peers);
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        crate::PendingPeer::<T>::set(None);
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub struct AddPeerOutgoingRequestEncoded {
    pub peer_address: Address,
    pub tx_hash: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
pub struct RemovePeerOutgoingRequestEncoded {
    pub peer_address: Address,
    pub tx_hash: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}
