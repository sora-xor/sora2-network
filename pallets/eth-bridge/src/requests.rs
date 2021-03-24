use crate::contract::{MethodId, FUNCTIONS, METHOD_ID_SIZE};
use crate::{
    get_bridge_account, types, Address, AssetIdOf, AssetKind, BridgeStatus, Config, Decoder, Error,
    OffchainRequest, OutgoingRequest, Pallet, RequestStatus, SignatureParams, Timepoint,
    WeightInfo,
};
use alloc::collections::BTreeSet;
use alloc::string::String;
use codec::{Decode, Encode};
use common::prelude::{Balance, WeightToFixedFee};
#[cfg(feature = "std")]
use common::utils::string_serialization;
use common::{AssetSymbol, BalancePrecision, VAL, XOR};
use ethabi::{FixedBytes, Token};
#[allow(unused_imports)]
use frame_support::debug;
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::traits::Get;
use frame_support::weights::WeightToFeePolynomial;
use frame_support::{ensure, RuntimeDebug};
use frame_system::RawOrigin;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{H256, U256};
use sp_std::convert::TryInto;
use sp_std::prelude::*;

pub const MIN_PEERS: usize = 4;
pub const MAX_PEERS: usize = 100;

type Assets<T> = assets::Pallet<T>;

/// Incoming request for adding Sidechain token to a bridge.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub struct IncomingAddToken<T: Config> {
    pub token_address: Address,
    pub asset_id: T::AssetId,
    pub precision: BalancePrecision,
    pub symbol: AssetSymbol,
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
}

impl<T: Config> IncomingAddToken<T> {
    /// Registers the sidechain asset.
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        common::with_transaction(|| {
            crate::Pallet::<T>::register_sidechain_asset(
                self.token_address,
                self.precision,
                self.symbol.clone(),
                self.network_id,
            )
        })?;
        Ok(self.tx_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }
}

/// Incoming request for adding/removing peer in a bridge.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IncomingChangePeers<T: Config> {
    pub peer_account_id: T::AccountId,
    pub peer_address: Address,
    pub added: bool,
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
}

impl<T: Config> IncomingChangePeers<T> {
    /// Checks if the pending peer matches with a peer in the request, then adds a signatory to the
    /// bridge multisig account and to the peers set (if `added` is true), otherwise does nothing,
    /// because the peer was removed early (in the corresponding outgoing request). Finally, it
    /// cleans the pending peer value.
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        let pending_peer =
            crate::PendingPeer::<T>::get(self.network_id).ok_or(Error::<T>::NoPendingPeer)?;
        ensure!(
            pending_peer == self.peer_account_id,
            Error::<T>::WrongPendingPeer
        );
        let is_eth_network = self.network_id == T::GetEthNetworkId::get();
        let eth_sync_peers_opt = if is_eth_network {
            let mut eth_sync_peers: EthPeersSync = crate::PendingEthPeersSync::<T>::get();
            eth_sync_peers.bridge_ready();
            Some(eth_sync_peers)
        } else {
            None
        };
        let is_ready = eth_sync_peers_opt
            .as_ref()
            .map(|x| x.is_ready())
            .unwrap_or(true);
        if is_ready {
            if self.added {
                let account_id = self.peer_account_id.clone();
                bridge_multisig::Module::<T>::add_signatory(
                    RawOrigin::Signed(get_bridge_account::<T>(self.network_id)).into(),
                    account_id.clone(),
                )
                .map_err(|e| e.error)?;
                crate::Peers::<T>::mutate(self.network_id, |set| set.insert(account_id));
            }
            crate::PendingPeer::<T>::take(self.network_id);
        }
        if let Some(mut eth_sync_peers) = eth_sync_peers_opt {
            if is_ready {
                eth_sync_peers.reset();
            }
            crate::PendingEthPeersSync::<T>::set(eth_sync_peers);
        }
        Ok(self.tx_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum ChangePeersContract {
    XOR,
    VAL,
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IncomingChangePeersCompat<T: Config> {
    pub peer_account_id: T::AccountId,
    pub peer_address: Address,
    pub added: bool,
    pub contract: ChangePeersContract,
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
}

impl<T: Config> IncomingChangePeersCompat<T> {
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        let pending_peer =
            crate::PendingPeer::<T>::get(self.network_id).ok_or(Error::<T>::NoPendingPeer)?;
        ensure!(
            pending_peer == self.peer_account_id,
            Error::<T>::WrongPendingPeer
        );
        let is_eth_network = self.network_id == T::GetEthNetworkId::get();
        let eth_sync_peers_opt = if is_eth_network {
            let mut eth_sync_peers: EthPeersSync = crate::PendingEthPeersSync::<T>::get();
            match self.contract {
                ChangePeersContract::XOR => eth_sync_peers.xor_ready(),
                ChangePeersContract::VAL => eth_sync_peers.val_ready(),
            };
            Some(eth_sync_peers)
        } else {
            None
        };
        let is_ready = eth_sync_peers_opt
            .as_ref()
            .map(|x| x.is_ready())
            .unwrap_or(true);
        if is_ready {
            if self.added {
                let account_id = self.peer_account_id.clone();
                bridge_multisig::Module::<T>::add_signatory(
                    RawOrigin::Signed(get_bridge_account::<T>(self.network_id)).into(),
                    account_id.clone(),
                )
                .map_err(|e| e.error)?;
                crate::Peers::<T>::mutate(self.network_id, |set| set.insert(account_id));
            }
            crate::PendingPeer::<T>::take(self.network_id);
        }
        if let Some(mut eth_sync_peers) = eth_sync_peers_opt {
            if is_ready {
                eth_sync_peers.reset();
            }
            crate::PendingEthPeersSync::<T>::set(eth_sync_peers);
        }
        Ok(self.tx_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }
}

/// Incoming request for transferring token from Sidechain to Thischain.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct IncomingTransfer<T: Config> {
    pub from: Address,
    pub to: T::AccountId,
    pub asset_id: AssetIdOf<T>,
    pub asset_kind: AssetKind,
    #[cfg_attr(feature = "std", serde(with = "string_serialization"))]
    pub amount: Balance,
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
    pub should_take_fee: bool,
}

impl<T: Config> IncomingTransfer<T> {
    pub fn fee_amount() -> Balance {
        let weight = <T as Config>::WeightInfo::request_from_sidechain();
        WeightToFixedFee::calc(&weight)
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        if self.should_take_fee {
            let transfer_fee = Self::fee_amount();
            ensure!(self.amount >= transfer_fee, Error::<T>::UnableToPayFees);
        }
        Ok(())
    }

    /// If the asset kind is owned, then the `amount` of funds is reserved on the bridge account.
    pub fn prepare(&self) -> Result<(), DispatchError> {
        if self.asset_kind.is_owned() {
            let bridge_account = get_bridge_account::<T>(self.network_id);
            Assets::<T>::reserve(self.asset_id, &bridge_account, self.amount)?;
        }
        Ok(())
    }

    /// Unreserves previously reserved amount of funds if the asset kind is owned.
    pub fn unreserve(&self) -> DispatchResult {
        if self.asset_kind.is_owned() {
            let bridge_acc = &get_bridge_account::<T>(self.network_id);
            let remainder = Assets::<T>::unreserve(self.asset_id, bridge_acc, self.amount)?;
            ensure!(remainder == 0, Error::<T>::FailedToUnreserve);
        }
        Ok(())
    }

    /// Calls `.unreserve`.
    pub fn cancel(&self) -> Result<(), DispatchError> {
        self.unreserve()
    }

    /// If the transferring asset kind is owned, the funds are transferred from the bridge account,
    /// otherwise the amount is minted.
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        self.validate()?;
        let bridge_account_id = get_bridge_account::<T>(self.network_id);
        let transfer_fee = Self::fee_amount();
        let amount = if self.should_take_fee {
            self.amount - transfer_fee
        } else {
            self.amount
        };
        if self.asset_kind.is_owned() {
            common::with_transaction(|| -> Result<_, DispatchError> {
                self.unreserve()?;
                if self.should_take_fee {
                    assets::Pallet::<T>::burn_from(
                        &self.asset_id,
                        &bridge_account_id,
                        &bridge_account_id,
                        transfer_fee,
                    )?;
                }
                Assets::<T>::transfer_from(&self.asset_id, &bridge_account_id, &self.to, amount)?;
                Ok(())
            })?;
        } else {
            Assets::<T>::mint_to(&self.asset_id, &bridge_account_id, &self.to, amount)?;
        }
        Ok(self.tx_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }

    pub fn enable_taking_fee(&mut self) {
        self.should_take_fee = true;
    }
}

/// Encodes the given outgoing request as it should look when it gets called on Sidechain.
pub fn encode_outgoing_request_eth_call<T: Config>(
    method_id: MethodId,
    request: &OutgoingRequest<T>,
) -> Result<Vec<u8>, Error<T>> {
    let fun_metas = &FUNCTIONS.get().unwrap();
    let fun_meta = fun_metas.get(&method_id).ok_or(Error::UnknownMethodId)?;
    let request_hash = request.hash();
    let request_encoded = request.to_eth_abi(request_hash)?;
    let approvals: BTreeSet<SignatureParams> =
        crate::RequestApprovals::<T>::get(request.network_id(), &request_hash);
    let input_tokens = request_encoded.input_tokens(Some(approvals.into_iter().collect()));
    fun_meta
        .function
        .encode_input(&input_tokens)
        .map_err(|_| Error::EthAbiEncodingError)
}

/// Incoming request for cancelling a broken outgoing request. "Broken" means that the request
/// signatures were collected, but something changed in the bridge state (e.g., peers set) and
/// the signatures became invalid. In this case we want to cancel the request to be able to
/// re-submit it later.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub struct IncomingCancelOutgoingRequest<T: Config> {
    pub request: OutgoingRequest<T>,
    pub initial_request_hash: H256,
    pub tx_input: Vec<u8>,
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
}

impl<T: Config> IncomingCancelOutgoingRequest<T> {
    /// Checks that the request status is `ApprovalsReady`, and encoded request's call matches
    /// with the `tx_input`, otherwise an error is thrown. After that, a status of the request
    /// is changed to `Frozen` to stop receiving approvals.
    pub fn prepare(&self) -> Result<(), DispatchError> {
        let request_hash = self.request.hash();
        let net_id = self.network_id;
        let req_status = crate::RequestStatuses::<T>::get(net_id, &request_hash)
            .ok_or(crate::Error::<T>::UnknownRequest)?;
        ensure!(
            req_status == RequestStatus::ApprovalsReady,
            crate::Error::<T>::RequestIsNotReady
        );
        let mut method_id = [0u8; METHOD_ID_SIZE];
        ensure!(self.tx_input.len() >= 4, Error::<T>::InvalidFunctionInput);
        method_id.clone_from_slice(&self.tx_input[..METHOD_ID_SIZE]);
        let expected_input = encode_outgoing_request_eth_call(method_id, &self.request)?;
        ensure!(
            expected_input == self.tx_input,
            crate::Error::<T>::InvalidContractInput
        );
        crate::RequestStatuses::<T>::insert(net_id, &request_hash, RequestStatus::Frozen);
        Ok(())
    }

    /// Changes the request's status back to `ApprovalsReady`.
    pub fn cancel(&self) -> Result<(), DispatchError> {
        crate::RequestStatuses::<T>::insert(
            self.network_id,
            &self.request.hash(),
            RequestStatus::ApprovalsReady,
        );
        Ok(())
    }

    /// Calls `cancel` on the request, changes its status to `Failed` and takes it approvals to
    /// make it available for resubmission.
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        // TODO: `common::with_transaction` should be removed in the future after stabilization.
        common::with_transaction(|| self.request.cancel())?;
        let hash = &self.request.hash();
        let net_id = self.network_id;
        crate::RequestStatuses::<T>::insert(net_id, hash, RequestStatus::Failed);
        crate::RequestApprovals::<T>::take(net_id, hash);
        Ok(self.initial_request_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }
}

/// Incoming request that acts as an acknowledgement to a corresponding
/// `OutgoingPrepareForMigration` request.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub struct IncomingPrepareForMigration<T: Config> {
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
}

impl<T: Config> IncomingPrepareForMigration<T> {
    /// Checks that the current bridge status is `Initialized`, otherwise an error is thrown.
    pub fn prepare(&self) -> Result<(), DispatchError> {
        ensure!(
            crate::BridgeStatuses::<T>::get(&self.network_id).ok_or(Error::<T>::UnknownNetwork)?
                == BridgeStatus::Initialized,
            Error::<T>::ContractIsAlreadyInMigrationStage
        );
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    /// Sets the bridge status to `Migrating`.
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        crate::BridgeStatuses::<T>::insert(self.network_id, BridgeStatus::Migrating);
        Ok(self.tx_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }
}

/// Incoming request that acts as an acknowledgement to a corresponding
/// `OutgoingMigrate` request.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize))]
pub struct IncomingMigrate<T: Config> {
    pub new_contract_address: Address,
    pub tx_hash: H256,
    pub at_height: u64,
    pub timepoint: Timepoint<T>,
    pub network_id: T::NetworkId,
}

impl<T: Config> IncomingMigrate<T> {
    /// Checks that the current bridge status is `Migrating`, otherwise an error is thrown.
    pub fn prepare(&self) -> Result<(), DispatchError> {
        ensure!(
            crate::BridgeStatuses::<T>::get(&self.network_id).ok_or(Error::<T>::UnknownNetwork)?
                == BridgeStatus::Migrating,
            Error::<T>::ContractIsNotInMigrationStage
        );
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    /// Updates the bridge's contract address and sets its status to `Initialized`.
    pub fn finalize(&self) -> Result<H256, DispatchError> {
        crate::BridgeContractAddress::<T>::insert(self.network_id, self.new_contract_address);
        crate::BridgeStatuses::<T>::insert(self.network_id, BridgeStatus::Initialized);
        Ok(self.tx_hash)
    }

    pub fn timepoint(&self) -> Timepoint<T> {
        self.timepoint
    }
}

/// Outgoing request for transferring the given asset from Thischain to Sidechain.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingTransfer<T: Config> {
    pub from: T::AccountId,
    pub to: Address,
    pub asset_id: AssetIdOf<T>,
    #[cfg_attr(feature = "std", serde(with = "string_serialization"))]
    pub amount: Balance,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingTransfer<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingTransferEncoded, Error<T>> {
        // TODO: Incorrect type (Address != AccountId).
        let from = Address::from_slice(&self.from.encode()[..20]);
        let to = self.to;
        let currency_id;
        if let Some(token_address) =
            Pallet::<T>::registered_sidechain_token(self.network_id, &self.asset_id)
        {
            currency_id = CurrencyIdEncoded::TokenAddress(token_address);
        } else {
            let x = <T::AssetId as Into<H256>>::into(self.asset_id);
            currency_id = CurrencyIdEncoded::AssetId(H256(x.0));
        }
        let amount = U256::from(self.amount);
        let tx_hash = H256(tx_hash.0);
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let is_old_contract = self.network_id == T::GetEthNetworkId::get()
            && (self.asset_id == XOR.into() || self.asset_id == VAL.into());
        let raw = if is_old_contract {
            ethabi::encode_packed(&[
                currency_id.to_token(),
                Token::Uint(types::U256(amount.0)),
                Token::Address(types::H160(to.0)),
                Token::FixedBytes(tx_hash.0.to_vec()),
                Token::Address(types::H160(from.0)),
            ])
        } else {
            ethabi::encode_packed(&[
                currency_id.to_token(),
                Token::Uint(types::U256(amount.0)),
                Token::Address(types::H160(to.0)),
                Token::Address(types::H160(from.0)),
                Token::FixedBytes(tx_hash.0.to_vec()),
                Token::FixedBytes(network_id.0.to_vec()),
            ])
        };
        Ok(OutgoingTransferEncoded {
            from,
            to,
            currency_id,
            amount,
            tx_hash,
            network_id,
            raw,
        })
    }

    /// Checks that the given asset can be transferred through the bridge.
    pub fn validate(&self) -> Result<(), DispatchError> {
        ensure!(
            crate::RegisteredAsset::<T>::get(self.network_id, &self.asset_id).is_some(),
            Error::<T>::UnsupportedToken
        );
        Ok(())
    }

    /// Transfers the given `amount` of `asset_id` to the bridge account and reserve it.
    pub fn prepare(&mut self) -> Result<(), DispatchError> {
        let bridge_account = get_bridge_account::<T>(self.network_id);
        common::with_transaction(|| {
            Assets::<T>::transfer_from(&self.asset_id, &self.from, &bridge_account, self.amount)?;
            Assets::<T>::reserve(self.asset_id, &bridge_account, self.amount)
        })
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        let bridge_account = get_bridge_account::<T>(self.network_id);
        common::with_transaction(|| {
            let remainder = Assets::<T>::unreserve(self.asset_id, &bridge_account, self.amount)?;
            ensure!(remainder == 0, Error::<T>::FailedToUnreserve);
            Assets::<T>::transfer_from(&self.asset_id, &bridge_account, &self.from, self.amount)
        })
    }

    /// Validates the request again, then, if the asset is originated in Sidechain, it gets burned.
    pub fn finalize(&self) -> Result<(), DispatchError> {
        self.validate()?;
        let bridge_acc = get_bridge_account::<T>(self.network_id);
        common::with_transaction(|| {
            let remainder = Assets::<T>::unreserve(self.asset_id, &bridge_acc, self.amount)?;
            ensure!(remainder == 0, Error::<T>::FailedToUnreserve);
            let asset_kind: AssetKind =
                crate::Module::<T>::registered_asset(self.network_id, &self.asset_id)
                    .ok_or(Error::<T>::UnknownAssetId)?;
            if !asset_kind.is_owned() {
                // The burn shouldn't fail, because we've just unreserved the needed amount of the asset,
                // the only case it can fail is if the bridge account doesn't have `BURN` permission,
                // but this permission is always granted when adding sidechain asset to bridge
                // (see `Module::register_sidechain_asset`).
                Assets::<T>::burn_from(&self.asset_id, &bridge_acc, &bridge_acc, self.amount)?;
            }
            Ok(())
        })
    }
}

/// Thischain or Sidechain asset id.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyIdEncoded {
    AssetId(H256),
    TokenAddress(Address),
}

impl CurrencyIdEncoded {
    pub fn to_token(&self) -> Token {
        match self {
            CurrencyIdEncoded::AssetId(asset_id) => Token::FixedBytes(asset_id.encode()),
            CurrencyIdEncoded::TokenAddress(address) => Token::Address(types::H160(address.0)),
        }
    }
}

/// Sidechain-compatible version of `OutgoingTransfer`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingTransferEncoded {
    pub currency_id: CurrencyIdEncoded,
    pub amount: U256,
    pub to: Address,
    pub from: Address,
    pub tx_hash: H256,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingTransferEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![
            self.currency_id.to_token(),
            Token::Uint(types::U256(self.amount.0)),
            Token::Address(types::H160(self.to.0)),
            Token::Address(types::H160(self.from.0)),
            Token::FixedBytes(self.tx_hash.0.to_vec()),
        ];

        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

/// Outgoing request for adding a Thischain asset.
// TODO: lock the adding token to prevent double-adding.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddAsset<T: Config> {
    pub author: T::AccountId,
    pub asset_id: AssetIdOf<T>,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingAddAsset<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingAddAssetEncoded, Error<T>> {
        let hash = H256(tx_hash.0);
        let (symbol, precision, _) = Assets::<T>::get_asset_info(&self.asset_id);
        let symbol: String = String::from_utf8_lossy(&symbol.0).into();
        let name = symbol.clone();
        let asset_id_code = <AssetIdOf<T> as Into<H256>>::into(self.asset_id);
        let sidechain_asset_id = asset_id_code.0.to_vec();
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let raw = ethabi::encode_packed(&[
            Token::String(name.clone()),
            Token::String(symbol.clone()),
            Token::UintSized(precision.into(), 8),
            Token::FixedBytes(sidechain_asset_id.clone()),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::FixedBytes(network_id.0.to_vec()),
        ]);

        Ok(OutgoingAddAssetEncoded {
            name,
            symbol,
            decimal: precision,
            sidechain_asset_id,
            hash,
            network_id,
            raw,
        })
    }

    /// Checks that the asset isn't registered yet.
    pub fn validate(&self) -> Result<(), DispatchError> {
        Assets::<T>::ensure_asset_exists(&self.asset_id)?;
        ensure!(
            crate::RegisteredAsset::<T>::get(self.network_id, &self.asset_id).is_none(),
            Error::<T>::TokenIsAlreadyAdded
        );
        Ok(())
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    /// Calls `validate` again and registers the asset.
    pub fn finalize(&self) -> Result<(), DispatchError> {
        self.validate()?;
        crate::RegisteredAsset::<T>::insert(self.network_id, &self.asset_id, AssetKind::Thischain);
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Sidechain-compatible version of `OutgoingAddAsset`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddAssetEncoded {
    pub name: String,
    pub symbol: String,
    pub decimal: u8,
    pub sidechain_asset_id: FixedBytes,
    pub hash: H256,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingAddAssetEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![
            Token::String(self.name.clone()),
            Token::String(self.symbol.clone()),
            Token::Uint(self.decimal.into()),
            Token::FixedBytes(self.sidechain_asset_id.clone()),
        ];
        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

/// Outgoing request for adding a Sidechain token.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddToken<T: Config> {
    pub author: T::AccountId,
    pub token_address: Address,
    pub ticker: String,
    pub name: String,
    pub decimals: u8,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

#[derive(Default)]
pub struct Encoder {
    tokens: Vec<Token>,
}

impl Encoder {
    pub fn new() -> Self {
        Encoder::default()
    }

    pub fn write_address(&mut self, val: &Address) {
        self.tokens.push(Token::Address(types::H160(val.0)));
    }

    pub fn write_string(&mut self, val: String) {
        self.tokens.push(Token::String(val));
    }

    pub fn write_u8(&mut self, val: u8) {
        self.tokens.push(Token::Uint(types::U256::from(val)));
    }

    pub fn into_inner(self) -> Vec<Token> {
        self.tokens
    }
}

/// Converts signature params to Sidechain-compatible tokens.
pub fn signature_params_to_tokens(sig_params: Vec<SignatureParams>) -> Vec<Token> {
    let mut vs = Vec::new();
    let mut rs = Vec::new();
    let mut ss = Vec::new();
    for sig_param in sig_params {
        vs.push(Token::Uint(types::U256::from(sig_param.v)));
        rs.push(Token::FixedBytes(sig_param.r.to_vec()));
        ss.push(Token::FixedBytes(sig_param.s.to_vec()));
    }
    vec![Token::Array(vs), Token::Array(rs), Token::Array(ss)]
}

impl<T: Config> OutgoingAddToken<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingAddTokenEncoded, Error<T>> {
        let hash = H256(tx_hash.0);
        let token_address = self.token_address;
        let ticker = self.ticker.clone();
        let name = self.name.clone();
        let decimals = self.decimals;
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::H160(token_address.0)),
            Token::String(ticker.clone()),
            Token::String(name.clone()),
            Token::UintSized(decimals.into(), 8),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::FixedBytes(network_id.0.to_vec()),
        ]);
        Ok(OutgoingAddTokenEncoded {
            token_address,
            name,
            ticker,
            decimals,
            hash,
            network_id,
            raw,
        })
    }

    /// Checks that the asset isn't registered yet and the given symbol is valid.
    pub fn validate(&self) -> Result<AssetSymbol, DispatchError> {
        ensure!(
            crate::RegisteredSidechainAsset::<T>::get(self.network_id, &self.token_address)
                .is_none(),
            Error::<T>::SidechainAssetIsAlreadyRegistered
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

    /// Calls `validate` again and registers the sidechain asset.
    pub fn finalize(&self) -> Result<(), DispatchError> {
        let symbol = self.validate()?;
        common::with_transaction(|| {
            crate::Pallet::<T>::register_sidechain_asset(
                self.token_address,
                self.decimals,
                symbol,
                self.network_id,
            )
        })?;
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Sidechain-compatible version of `OutgoingAddToken`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddTokenEncoded {
    pub token_address: Address,
    pub ticker: String,
    pub name: String,
    pub decimals: u8,
    pub hash: H256,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingAddTokenEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![
            Token::Address(types::H160(self.token_address.0)),
            Token::String(self.ticker.clone()),
            Token::String(self.name.clone()),
            Token::Uint(self.decimals.into()),
        ];
        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

/// Outgoing request for adding a peer.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddPeer<T: Config> {
    pub author: T::AccountId,
    pub peer_address: Address,
    pub peer_account_id: T::AccountId,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingAddPeer<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingAddPeerEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let peer_address = self.peer_address;
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::H160(peer_address.0)),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::FixedBytes(network_id.0.to_vec()),
        ]);
        Ok(OutgoingAddPeerEncoded {
            peer_address,
            tx_hash,
            network_id,
            raw,
        })
    }

    /// Checks that the current number of peers is not greater than `MAX_PEERS` and the given peer
    /// is not presented in the current peer set,
    pub fn validate(&self) -> Result<BTreeSet<T::AccountId>, DispatchError> {
        let peers = crate::Peers::<T>::get(self.network_id);
        ensure!(peers.len() <= MAX_PEERS, Error::<T>::CantAddMorePeers);
        ensure!(
            !peers.contains(&self.peer_account_id),
            Error::<T>::PeerIsAlreadyAdded
        );
        Ok(peers)
    }

    /// Checks that the current pending peer value is none and inserts the given one.
    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        let pending_peer = crate::PendingPeer::<T>::get(self.network_id);
        ensure!(pending_peer.is_none(), Error::<T>::TooManyPendingPeers);
        crate::PendingPeer::<T>::insert(self.network_id, self.peer_account_id.clone());
        Ok(())
    }

    /// Calls `validate` again and inserts the peer account ids on Thischain and Sidechain to
    /// have an association.
    pub fn finalize(&self) -> Result<(), DispatchError> {
        let _peers = self.validate()?;
        crate::PeerAccountId::<T>::insert(
            self.network_id,
            self.peer_address,
            self.peer_account_id.clone(),
        );
        crate::PeerAddress::<T>::insert(self.network_id, &self.peer_account_id, self.peer_address);
        Ok(())
    }

    /// Cleans the current pending peer value.
    pub fn cancel(&self) -> Result<(), DispatchError> {
        crate::PendingPeer::<T>::take(self.network_id);
        Ok(())
    }
}

/// Old contracts-compatible `add peer` request. Will be removed in the future.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddPeerCompat<T: Config> {
    pub author: T::AccountId,
    pub peer_address: Address,
    pub peer_account_id: T::AccountId,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingAddPeerCompat<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingAddPeerEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let peer_address = self.peer_address;
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::H160(peer_address.0)),
            Token::FixedBytes(tx_hash.0.to_vec()),
        ]);
        Ok(OutgoingAddPeerEncoded {
            peer_address,
            tx_hash,
            network_id,
            raw,
        })
    }

    pub fn validate(&self) -> Result<BTreeSet<T::AccountId>, DispatchError> {
        let peers = crate::Peers::<T>::get(self.network_id);
        ensure!(peers.len() <= MAX_PEERS, Error::<T>::CantAddMorePeers);
        ensure!(
            !peers.contains(&self.peer_account_id),
            Error::<T>::PeerIsAlreadyAdded
        );
        let pending_peer = crate::PendingPeer::<T>::get(self.network_id);
        // Previous `OutgoingAddPeer` should set the pending peer.
        ensure!(
            pending_peer.as_ref() == Some(&self.peer_account_id),
            Error::<T>::NoPendingPeer
        );
        Ok(peers)
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Outgoing request for removing a peer.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingRemovePeer<T: Config> {
    pub author: T::AccountId,
    pub peer_account_id: T::AccountId,
    pub peer_address: Address,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingRemovePeer<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingRemovePeerEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let peer_address = self.peer_address;
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::H160(peer_address.0)),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::FixedBytes(network_id.0.to_vec()),
        ]);
        Ok(OutgoingRemovePeerEncoded {
            peer_address,
            tx_hash,
            network_id,
            raw,
        })
    }

    /// Checks that the current number of peers is not less than `MIN_PEERS` and the given peer
    /// is presented in the current peer set,
    pub fn validate(&self) -> Result<BTreeSet<T::AccountId>, DispatchError> {
        let peers = crate::Peers::<T>::get(self.network_id);
        ensure!(peers.len() >= MIN_PEERS, Error::<T>::CantRemoveMorePeers);
        ensure!(
            peers.contains(&self.peer_account_id),
            Error::<T>::UnknownPeerId
        );
        Ok(peers)
    }

    /// Checks that the current pending peer value is none and inserts the given one.
    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        let pending_peer = crate::PendingPeer::<T>::get(self.network_id);
        ensure!(pending_peer.is_none(), Error::<T>::TooManyPendingPeers);
        crate::PendingPeer::<T>::insert(self.network_id, self.peer_account_id.clone());
        Ok(())
    }

    /// Calls `validate` again and removes the peer from the peer set and from the multisig bridge
    /// account.
    pub fn finalize(&self) -> Result<(), DispatchError> {
        let mut peers = self.validate()?;
        bridge_multisig::Pallet::<T>::remove_signatory(
            RawOrigin::Signed(get_bridge_account::<T>(self.network_id)).into(),
            self.peer_account_id.clone(),
        )
        .map_err(|e| e.error)?;
        peers.remove(&self.peer_account_id);
        crate::Peers::<T>::insert(self.network_id, peers);
        // TODO: check it's not conflicting with compat request
        crate::PeerAccountId::<T>::take(self.network_id, self.peer_address);
        crate::PeerAddress::<T>::take(self.network_id, &self.peer_account_id);
        Ok(())
    }

    /// Cleans the current pending peer value.
    pub fn cancel(&self) -> Result<(), DispatchError> {
        crate::PendingPeer::<T>::take(self.network_id);
        Ok(())
    }
}

/// Old contracts-compatible `add peer` request. Will be removed in the future.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingRemovePeerCompat<T: Config> {
    pub author: T::AccountId,
    pub peer_account_id: T::AccountId,
    pub peer_address: Address,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingRemovePeerCompat<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingRemovePeerEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let peer_address = self.peer_address;
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::H160(peer_address.0)),
            Token::FixedBytes(tx_hash.0.to_vec()),
        ]);
        Ok(OutgoingRemovePeerEncoded {
            peer_address,
            tx_hash,
            network_id,
            raw,
        })
    }

    pub fn validate(&self) -> Result<BTreeSet<T::AccountId>, DispatchError> {
        let peers = crate::Peers::<T>::get(self.network_id);
        ensure!(peers.len() >= MIN_PEERS, Error::<T>::CantRemoveMorePeers);
        ensure!(
            peers.contains(&self.peer_account_id),
            Error::<T>::UnknownPeerId
        );
        let pending_peer = crate::PendingPeer::<T>::get(self.network_id);
        // Previous `OutgoingRemovePeer` should set the pending peer.
        ensure!(
            pending_peer.as_ref() == Some(&self.peer_account_id),
            Error::<T>::NoPendingPeer
        );
        Ok(peers)
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Sidechain-compatible version of `OutgoingAddPeer`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingAddPeerEncoded {
    pub peer_address: Address,
    pub tx_hash: H256,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingAddPeerEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![
            Token::Address(types::H160(self.peer_address.0)),
            Token::FixedBytes(self.tx_hash.0.to_vec()),
        ];
        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

/// Sidechain-compatible version of `OutgoingRemovePeer`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingRemovePeerEncoded {
    pub peer_address: Address,
    pub tx_hash: H256,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingRemovePeerEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![
            Token::Address(types::H160(self.peer_address.0)),
            Token::FixedBytes(self.tx_hash.0.to_vec()),
        ];
        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

/// Outgoing request for preparing bridge for migration.
///
/// The migration is executed in 2 phases:
/// 1. Prepare both chains for migration. After the preparation, Thischain stops collecting
/// signatures for outgoing requests, but accepts all incoming requests. This phase is used to
/// get pending incoming requests to finish and to have both chains as much synchronised
/// as possible.
/// 2. Migrate the bridge. At this stage a new Sidechain contract should be deployed and Thischain
/// should be switched to it, so the old contract can't be used anymore.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingPrepareForMigration<T: Config> {
    pub author: T::AccountId,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingPrepareForMigration<T> {
    pub fn to_eth_abi(
        &self,
        tx_hash: H256,
    ) -> Result<OutgoingPrepareForMigrationEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let contract_address: Address = crate::BridgeContractAddress::<T>::get(&self.network_id);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::Address::from(contract_address.0)),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::FixedBytes(network_id.0.to_vec()),
        ]);
        Ok(OutgoingPrepareForMigrationEncoded {
            this_contract_address: contract_address,
            tx_hash,
            network_id,
            raw,
        })
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Sidechain-compatible version of `OutgoingPrepareForMigration`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingPrepareForMigrationEncoded {
    pub this_contract_address: Address,
    pub tx_hash: H256,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingPrepareForMigrationEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![
            Token::Address(types::Address::from(self.this_contract_address.0)),
            Token::FixedBytes(self.tx_hash.0.to_vec()),
        ];
        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

/// Outgoing request for migrating the bridge. For the full migration process description see
/// `OutgoingPrepareForMigration` request.
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingMigrate<T: Config> {
    pub author: T::AccountId,
    pub new_contract_address: Address,
    pub erc20_native_tokens: Vec<Address>,
    pub nonce: T::Index,
    pub network_id: T::NetworkId,
    pub timepoint: Timepoint<T>,
}

impl<T: Config> OutgoingMigrate<T> {
    pub fn to_eth_abi(&self, tx_hash: H256) -> Result<OutgoingMigrateEncoded, Error<T>> {
        let tx_hash = H256(tx_hash.0);
        let mut network_id: H256 = H256::default();
        U256::from(
            <T::NetworkId as TryInto<u128>>::try_into(self.network_id)
                .ok()
                .expect("NetworkId can be always converted to u128; qed"),
        )
        .to_big_endian(&mut network_id.0);
        let contract_address: Address = crate::BridgeContractAddress::<T>::get(&self.network_id);
        let raw = ethabi::encode_packed(&[
            Token::Address(types::Address::from(contract_address.0)),
            Token::Address(types::Address::from(self.new_contract_address.0)),
            Token::FixedBytes(tx_hash.0.to_vec()),
            Token::Array(
                self.erc20_native_tokens
                    .iter()
                    .map(|addr| Token::Address(types::Address::from(addr.0)))
                    .collect(),
            ),
            Token::FixedBytes(network_id.0.to_vec()),
        ]);
        Ok(OutgoingMigrateEncoded {
            this_contract_address: contract_address,
            tx_hash,
            new_contract_address: self.new_contract_address,
            erc20_native_tokens: self.erc20_native_tokens.clone(),
            network_id,
            raw,
        })
    }

    pub fn validate(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn prepare(&mut self, _validated_state: ()) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), DispatchError> {
        Ok(())
    }

    pub fn finalize(&self) -> Result<(), DispatchError> {
        Ok(())
    }
}

/// Sidechain-compatible version of `OutgoingMigrate`.
#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OutgoingMigrateEncoded {
    pub this_contract_address: Address,
    pub tx_hash: H256,
    pub new_contract_address: Address,
    pub erc20_native_tokens: Vec<Address>,
    pub network_id: H256,
    /// EABI-encoded data to be signed.
    pub raw: Vec<u8>,
}

impl OutgoingMigrateEncoded {
    pub fn input_tokens(&self, signatures: Option<Vec<SignatureParams>>) -> Vec<Token> {
        let mut tokens = vec![Token::FixedBytes(self.tx_hash.0.to_vec())];
        if let Some(sigs) = signatures {
            let sig_tokens = signature_params_to_tokens(sigs);
            tokens.extend(sig_tokens);
        }
        tokens
    }
}

// TODO: docs
#[derive(Clone, Default, PartialEq, Eq, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct EthPeersSync {
    is_bridge_ready: bool,
    is_xor_ready: bool,
    is_val_ready: bool,
}

impl EthPeersSync {
    pub fn is_ready(&self) -> bool {
        self.is_bridge_ready && self.is_xor_ready && self.is_val_ready
    }

    pub fn bridge_ready(&mut self) {
        self.is_bridge_ready = true;
    }

    pub fn xor_ready(&mut self) {
        self.is_xor_ready = true;
    }

    pub fn val_ready(&mut self) {
        self.is_val_ready = true;
    }

    pub fn reset(&mut self) {
        self.is_val_ready = false;
        self.is_xor_ready = false;
        self.is_bridge_ready = false;
    }
}

/// Parses a `tx_hash` argument of a contract call. `tx_hash` is usually a hash of a Thischain's
/// outgoing request (`OutgoingRequest`).
pub fn parse_hash_from_call<T: Config>(
    tokens: Vec<Token>,
    tx_hash_arg_pos: usize,
) -> Result<H256, Error<T>> {
    tokens
        .get(tx_hash_arg_pos)
        .cloned()
        .and_then(Decoder::<T>::parse_h256)
        .ok_or_else(|| Error::<T>::FailedToParseTxHashInCall.into())
}

macro_rules! impl_from_for_outgoing_requests {
    ($($req:ty, $var:ident);+ $(;)?) => {$(
        impl<T: Config> From<$req> for OutgoingRequest<T> {
            fn from(v: $req) -> Self {
                Self::$var(v)
            }
        }

        impl<T: Config> From<$req> for OffchainRequest<T> {
            fn from(v: $req) -> Self {
                Self::outgoing(v.into())
            }
        }
    )+};
}

impl_from_for_outgoing_requests! {
    OutgoingTransfer<T>, Transfer;
    OutgoingAddAsset<T>, AddAsset;
    OutgoingAddToken<T>, AddToken;
    OutgoingAddPeer<T>, AddPeer;
    OutgoingAddPeerCompat<T>, AddPeerCompat;
    OutgoingRemovePeer<T>, RemovePeer;
    OutgoingRemovePeerCompat<T>, RemovePeerCompat;
    OutgoingPrepareForMigration<T>, PrepareForMigration;
    OutgoingMigrate<T>, Migrate;
}

macro_rules! impl_from_for_incoming_requests {
    ($($req:ty, $var:ident);+ $(;)?) => {$(
        impl<T: Config> From<$req> for crate::IncomingRequest<T> {
            fn from(v: $req) -> Self {
                Self::$var(v)
            }
        }
    )+};
}

impl_from_for_incoming_requests! {
    IncomingTransfer<T>, Transfer;
    IncomingAddToken<T>, AddAsset;
    IncomingChangePeers<T>, ChangePeers;
    IncomingChangePeersCompat<T>, ChangePeersCompat;
    IncomingPrepareForMigration<T>, PrepareForMigration;
    IncomingMigrate<T>, Migrate;
    IncomingCancelOutgoingRequest<T>, CancelOutgoingRequest;
}
