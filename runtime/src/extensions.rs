use codec::{Decode, Encode};
use frame_support::dispatch::{DispatchInfo, Dispatchable, PostDispatchInfo};
use frame_support::unsigned::TransactionValidityError;
use pallet_transaction_payment as ptp;
use sp_runtime::traits::SignedExtension;
use sp_runtime::FixedPointOperand;

type PtpBalanceOf<T> =
    <<T as ptp::Config>::OnChargeTransaction as ptp::OnChargeTransaction<T>>::Balance;

/// The copy of pallet_transaction_payment::ChargeTransactionPayment, but the tip is always 0.
/// We don't want some users to have leverage over other because it could be abused in trading
#[derive(Encode, Clone, Eq, PartialEq)]
pub struct ChargeTransactionPayment<T: ptp::Config>(ptp::ChargeTransactionPayment<T>);

impl<T: ptp::Config> ChargeTransactionPayment<T>
where
    PtpBalanceOf<T>: Send + Sync + FixedPointOperand,
    T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    pub fn new() -> Self {
        Self(ptp::ChargeTransactionPayment::<T>::from(0u32.into()))
    }
}

impl<T: ptp::Config> sp_std::fmt::Debug for ChargeTransactionPayment<T> {
    fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
        Ok(())
    }
}

impl<T: ptp::Config> Decode for ChargeTransactionPayment<T>
where
    PtpBalanceOf<T>: Send + Sync + FixedPointOperand,
    T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
        // The input needs to be checked, but the result is irrelevant
        let _ = ptp::ChargeTransactionPayment::<T>::decode(input)?;
        Ok(Self(ptp::ChargeTransactionPayment::<T>::from(0u32.into())))
    }
}

// Copied from pallet-transaction-payment
impl<T: ptp::Config> SignedExtension for ChargeTransactionPayment<T>
where
    PtpBalanceOf<T>: Send + Sync + From<u64> + FixedPointOperand,
    T::Call: Dispatchable<Info = DispatchInfo, PostInfo = PostDispatchInfo>,
{
    const IDENTIFIER: &'static str =
        <ptp::ChargeTransactionPayment<T> as SignedExtension>::IDENTIFIER;

    type AccountId = <ptp::ChargeTransactionPayment<T> as SignedExtension>::AccountId;

    type Call = <T as frame_system::Config>::Call;

    type AdditionalSigned = <ptp::ChargeTransactionPayment<T> as SignedExtension>::AdditionalSigned;

    type Pre = <ptp::ChargeTransactionPayment<T> as SignedExtension>::Pre;

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        self.0.additional_signed()
    }

    fn validate(
        &self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> sp_api::TransactionValidity {
        self.0.validate(who, call, info, len)
    }

    fn pre_dispatch(
        self,
        who: &Self::AccountId,
        call: &Self::Call,
        info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        self.0.pre_dispatch(who, call, info, len)
    }

    fn post_dispatch(
        pre: Self::Pre,
        info: &sp_runtime::traits::DispatchInfoOf<Self::Call>,
        post_info: &sp_runtime::traits::PostDispatchInfoOf<Self::Call>,
        len: usize,
        result: &sp_runtime::DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        <ptp::ChargeTransactionPayment<T> as SignedExtension>::post_dispatch(
            pre, info, post_info, len, result,
        )
    }
}
