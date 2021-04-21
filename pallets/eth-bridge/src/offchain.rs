use crate::Config;
use codec::{Decode, Encode};
use frame_support::debug;
use frame_support::dispatch::GetCallMetadata;
use frame_support::sp_io::hashing::blake2_256;
use frame_support::sp_runtime::offchain::storage_lock::BlockNumberProvider;
use frame_system::offchain::{
    Account, CreateSignedTransaction, SendSignedTransaction, SendTransactionTypes, Signer,
};
use sp_core::H256;
type Call<T> = <T as Config>::Call;

/// Information about an extrinsic sent by an off-chain worker. Used to identify extrinsics in
/// finalized blocks.
#[derive(Encode, Decode)]
pub struct SignedTransactionData<T>
where
    T: Config,
{
    pub extrinsic_hash: H256,
    pub submitted_at: Option<T::BlockNumber>,
    pub call: Call<T>,
}

impl<T: Config> SignedTransactionData<T> {
    pub fn new(
        extrinsic_hash: H256,
        submitted_at: Option<T::BlockNumber>,
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
        submitted_at: Option<T::BlockNumber>,
    ) -> Option<Self>
    where
        T: CreateSignedTransaction<LocalCall>,
    {
        use frame_support::inherent::Extrinsic;
        let overarching_call: Call<T> = call.clone().into();
        let account_data = frame_system::Account::<T>::get(&account.id);
        let (call, signature) =
            <T as CreateSignedTransaction<LocalCall>>::create_transaction::<T::PeerId>(
                <T as SendTransactionTypes<LocalCall>>::OverarchingCall::from(call),
                account.public.clone(),
                account.id.clone(),
                account_data.nonce - 1u32.into(),
            )?;
        let xt = <T as SendTransactionTypes<LocalCall>>::Extrinsic::new(call, Some(signature))?;
        let vec = xt.encode();
        let ext_hash = H256(blake2_256(&vec));
        Some(Self::new(ext_hash, submitted_at, overarching_call))
    }

    /// Re-sends current call and updates self.
    pub fn resend(&mut self, signer: &Signer<T, T::PeerId>)
    where
        T: CreateSignedTransaction<Call<T>>,
    {
        debug::debug!(
            "Re-sending signed transaction: {:?}",
            self.call.get_call_metadata()
        );
        let result = signer.send_signed_transaction(|_acc| self.call.clone());

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
        }
    }
}
