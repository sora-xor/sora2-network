use codec::{Decode, Encode};
use frame_support::unsigned::TransactionValidityError;
use sp_runtime::traits::{DispatchInfoOf, SignedExtension};

use crate::{AccountId, Call};

#[derive(Clone, Debug, Decode, Encode, Eq, PartialEq)]
pub struct PrintCall;

impl SignedExtension for PrintCall {
    const IDENTIFIER: &'static str = "PrintCall";

    type AccountId = AccountId;

    type Call = Call;

    type AdditionalSigned = ();

    type Pre = ();

    fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
        Ok(())
    }

    fn pre_dispatch(
        self,
        _who: &AccountId,
        call: &Call,
        _info: &DispatchInfoOf<Call>,
        _len: usize,
    ) -> Result<(), TransactionValidityError> {
        frame_support::debug::trace!(target: "calls", "{:?}", call);
        Ok(())
    }
}
