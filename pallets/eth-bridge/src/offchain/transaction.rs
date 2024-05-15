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

use crate::requests::{IncomingRequest, LoadIncomingRequest, RequestStatus};
#[cfg(test)]
use crate::tests::mock::Mock;
use crate::util::get_bridge_account;
use crate::{
    Config, Error, Pallet, Timepoint, OFFCHAIN_TRANSACTION_WEIGHT_LIMIT,
    STORAGE_PENDING_TRANSACTIONS_KEY,
};
use alloc::boxed::Box;
use codec::{Decode, Encode};
use sp_runtime::DispatchError;
use log::{debug, error};
use sp_io::hashing::blake2_256;
use frame_support::sp_runtime::offchain::storage::StorageValueRef;
use frame_support::sp_runtime::traits::{BlockNumberProvider, IdentifyAccount, Saturating};
use frame_support::sp_runtime::RuntimeAppPublic;
use frame_support::traits::GetCallName;
use frame_support::{ensure, fail};
#[cfg(test)]
use frame_system::offchain::SignMessage;
use frame_system::offchain::{
    Account, AppCrypto, CreateSignedTransaction, SendSignedTransaction, SendTransactionTypes,
    Signer,
};
use sp_core::H256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

type Call<T> = <T as Config>::RuntimeCall;

/// Information about an extrinsic sent by an off-chain worker. Used to identify extrinsics in
/// finalized blocks.
#[derive(Encode, Decode, scale_info::TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct SignedTransactionData<T>
where
    T: Config,
{
    pub extrinsic_hash: H256,
    pub submitted_at: Option<BlockNumberFor<T>>,
    pub call: Call<T>,
}

impl<T: Config> SignedTransactionData<T> {
    pub fn new(
        extrinsic_hash: H256,
        submitted_at: Option<BlockNumberFor<T>>,
        call: impl Into<Call<T>>,
    ) -> Self {
        SignedTransactionData {
            extrinsic_hash,
            submitted_at,
            call: call.into(),
        }
    }

    /// Creates a `SignedTransactionData` from a Call.
    ///
    /// NOTE: this function should be called *only* after a `Signer::send_signed_transaction`
    /// success result.
    pub fn from_local_call<LocalCall: Clone + Encode + Into<Call<T>>>(
        call: LocalCall,
        account: &Account<T>,
        submitted_at: Option<BlockNumberFor<T>>,
    ) -> Option<Self>
    where
        T: CreateSignedTransaction<LocalCall>,
    {
        let overarching_call: Call<T> = call.clone().into();
        let account_data = frame_system::Account::<T>::get(&account.id);
        let nonce = if submitted_at.is_some() {
            account_data.nonce.saturating_sub(1u32.into())
        } else {
            account_data.nonce
        };
        let (call, signature) =
            <T as CreateSignedTransaction<LocalCall>>::create_transaction::<T::PeerId>(
                <T as SendTransactionTypes<LocalCall>>::OverarchingCall::from(call),
                account.public.clone(),
                account.id.clone(),
                nonce,
            )?;
        let xt = <T as SendTransactionTypes<LocalCall>>::Extrinsic::new(call, Some(signature))?;
        let vec = xt.encode();
        // TODO (optimize): consider skipping the hash calculation if the extrinsic weren't submitted.
        let ext_hash = H256(blake2_256(&vec));
        Some(Self::new(ext_hash, submitted_at, overarching_call))
    }

    /// Re-sends current call and updates self. Returns `true` if sent.
    pub fn resend(&mut self, signer: &Signer<T, T::PeerId>) -> bool
    where
        T: CreateSignedTransaction<Call<T>>,
    {
        debug!(
            "Re-sending signed transaction: {:?}",
            self.call.get_call_metadata()
        );
        let result = Pallet::<T>::send_signed_transaction(signer, &self.call);

        if let Some((account, res)) = result {
            let submitted_at = if res.is_err() {
                None
            } else {
                Some(frame_system::Pallet::<T>::current_block_number())
            };
            let signed_transaction_data =
                SignedTransactionData::from_local_call(self.call.clone(), &account, submitted_at)
                    .expect("we've just successfully signed the same data; qed");
            *self = signed_transaction_data;
            res.is_ok()
        } else {
            false
        }
    }
}

impl<T: Config> Pallet<T> {
    pub(crate) fn get_signer() -> Result<Signer<T, T::PeerId>, Error<T>> {
        let signer = Signer::<T, T::PeerId>::any_account();
        if !signer.can_sign() {
            error!("[Ethereum bridge] No local account available");
            fail!(<Error<T>>::NoLocalAccountForSigning);
        }
        Ok(signer)
    }

    pub(crate) fn get_keystore_accounts() -> Vec<T::AccountId> {
        <<T as Config>::PeerId as AppCrypto<T::Public, T::Signature>>::RuntimeAppPublic::all()
            .into_iter()
            .map(|key| {
                let generic_public = <<T as Config>::PeerId as AppCrypto<
                    T::Public,
                    T::Signature,
                >>::GenericPublic::from(key);
                let public: T::Public = generic_public.into();
                public.into_account()
            })
            .collect()
    }

    /// Sends a substrate transaction signed by an off-chain worker. After a successful signing
    /// information about the extrinsic is added to pending transactions storage, because according
    /// to [`sp_runtime::ApplyExtrinsicResult`](https://substrate.dev/rustdocs/v3.0.0/sp_runtime/type.ApplyExtrinsicResult.html)
    /// an extrinsic may not be imported to the block and thus should be re-sent.
    pub(crate) fn send_transaction<LocalCall>(call: LocalCall) -> Result<(), Error<T>>
    where
        T: CreateSignedTransaction<LocalCall>,
        LocalCall: Clone + GetCallName + Encode + Into<<T as Config>::RuntimeCall>,
    {
        let signer = Self::get_signer()?;
        debug!("Sending signed transaction: {}", call.get_call_name());
        let result = Self::send_signed_transaction(&signer, &call);

        match result {
            Some((account, res)) => {
                Self::add_pending_extrinsic(call, &account, res.is_ok());
                if let Err(e) = res {
                    error!(
                        "[{:?}] Failed to send signed transaction: {:?}",
                        account.id, e
                    );
                    fail!(<Error<T>>::FailedToSendSignedTransaction);
                }
            }
            _ => {
                error!("Failed to send signed transaction");
                fail!(<Error<T>>::NoLocalAccountForSigning);
            }
        };
        Ok(())
    }

    pub(crate) fn send_multisig_transaction(
        call: crate::Call<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let bridge_account = get_bridge_account::<T>(network_id);
        let threshold = bridge_multisig::Accounts::<T>::get(&bridge_account)
            .unwrap()
            .threshold_num();
        let call = if threshold == 1 {
            bridge_multisig::Call::as_multi_threshold_1 {
                id: bridge_account,
                call: Box::new(<<T as Config>::RuntimeCall>::from(call)),
                timepoint,
            }
        } else {
            let vec = <<T as Config>::RuntimeCall>::from(call).encode();
            bridge_multisig::Call::as_multi {
                id: bridge_account,
                maybe_timepoint: Some(timepoint),
                call: vec,
                store_call: true,
                max_weight: OFFCHAIN_TRANSACTION_WEIGHT_LIMIT,
            }
        };
        Self::send_transaction::<bridge_multisig::Call<T>>(call)
    }

    /// Send a transaction to finalize the incoming request.
    pub(crate) fn send_finalize_incoming_request(
        hash: H256,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug!("send_incoming_request_result: {:?}", hash);
        let transfer_call = crate::Call::<T>::finalize_incoming_request { hash, network_id };
        Self::send_multisig_transaction(transfer_call, timepoint, network_id)
    }

    pub(crate) fn send_import_incoming_request(
        load_incoming_request: LoadIncomingRequest<T>,
        incoming_request_result: Result<IncomingRequest<T>, DispatchError>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let timepoint = load_incoming_request.timepoint();
        debug!(
            "send_import_incoming_request: {:?}",
            incoming_request_result
        );
        let import_call = crate::Call::<T>::import_incoming_request {
            load_incoming_request,
            incoming_request_result,
        };
        Self::send_multisig_transaction(import_call, timepoint, network_id)
    }

    /// Send 'abort request' transaction.
    pub(crate) fn send_abort_request(
        request_hash: H256,
        request_error: Error<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        debug!("send_abort_request: {:?}", request_hash);
        ensure!(
            crate::RequestStatuses::<T>::get(network_id, request_hash)
                == Some(RequestStatus::Pending),
            Error::<T>::ExpectedPendingRequest
        );
        let abort_request_call = crate::Call::<T>::abort_request {
            hash: request_hash,
            error: request_error.into(),
            network_id,
        };
        Self::send_multisig_transaction(abort_request_call, timepoint, network_id)
    }

    pub(crate) fn add_pending_extrinsic<LocalCall>(
        call: LocalCall,
        account: &Account<T>,
        added_to_pool: bool,
    ) where
        T: CreateSignedTransaction<LocalCall>,
        LocalCall: Clone + GetCallName + Encode + Into<<T as Config>::RuntimeCall>,
    {
        let s_signed_txs = StorageValueRef::persistent(STORAGE_PENDING_TRANSACTIONS_KEY);
        let mut transactions = s_signed_txs
            .get::<BTreeMap<H256, SignedTransactionData<T>>>()
            .ok()
            .flatten()
            .unwrap_or_default();
        let submitted_at = if !added_to_pool {
            None
        } else {
            Some(frame_system::Pallet::<T>::current_block_number())
        };
        let signed_transaction_data =
            SignedTransactionData::from_local_call(call, account, submitted_at)
                .expect("we've just successfully signed the same data; qed");
        transactions.insert(
            signed_transaction_data.extrinsic_hash,
            signed_transaction_data,
        );
        s_signed_txs.set(&transactions);
    }

    /// Sends a multisig transaction to register the parsed (from pre-incoming) incoming request.
    /// (see `register_incoming_request`).
    pub(crate) fn send_register_incoming_request(
        incoming_request: IncomingRequest<T>,
        timepoint: Timepoint<T>,
        network_id: T::NetworkId,
    ) -> Result<(), Error<T>> {
        let register_call = crate::Call::<T>::register_incoming_request { incoming_request };
        Self::send_multisig_transaction(register_call, timepoint, network_id)
    }

    pub(crate) fn send_signed_transaction<LocalCall: Clone>(
        signer: &Signer<T, T::PeerId>,
        call: &LocalCall,
    ) -> <Signer<T, T::PeerId> as SendSignedTransaction<T, T::PeerId, LocalCall>>::Result
    where
        T: CreateSignedTransaction<LocalCall>,
    {
        #[cfg(test)]
        let result = {
            if T::Mock::should_fail_send_signed_transaction() {
                let account_id = signer.sign_message(&[]).unwrap().0;
                Some((account_id, Err(())))
            } else {
                signer.send_signed_transaction(|_acc| call.clone())
            }
        };
        #[cfg(not(test))]
        let result = signer.send_signed_transaction(|_acc| call.clone());
        result
    }
}
