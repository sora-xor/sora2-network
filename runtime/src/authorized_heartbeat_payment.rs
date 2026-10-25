//! Preserve the existing payment extension while admitting authorized validator heartbeats.

use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{traits::OriginTrait, weights::Weight};
use scale_info::TypeInfo;
use sp_runtime::{
    traits::{
        DispatchInfoOf, Implication, PostDispatchInfoOf, TransactionExtension,
        TransactionExtensionMetadata, ValidateResult, Zero,
    },
    transaction_validity::{
        InvalidTransaction, TransactionSource, TransactionValidity, TransactionValidityError,
        ValidTransaction,
    },
    DispatchResult,
};
use sp_std::vec::Vec;

use crate::{Runtime, RuntimeCall, RuntimeOrigin};
use xor_fee::extension::ChargeTransactionPayment;

#[expect(
    deprecated,
    reason = "Keep the reviewed signed fee and staking payout implementation during the SDK migration"
)]
type LegacySignedPayment = sp_runtime::traits::transaction_extension::AsTransactionExtension<
    ChargeTransactionPayment<Runtime>,
>;

type LegacyPayment = pallet_skip_feeless_payment::SkipCheckIfFeeless<Runtime, LegacySignedPayment>;

/// A transparent SCALE wrapper around the existing compact-tip payment extension.
///
/// Only a zero-tip ImOnline heartbeat already authenticated by AuthorizeCall may
/// proceed without a signed fee payer. Every signed path keeps the legacy feeless
/// checks, post-dispatch refunds and VAL payouts. Bare heartbeats are rejected before
/// dispatch; other bare calls retain the existing feeless wrapper behavior.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, Debug)]
pub struct AuthorizedHeartbeatPayment(LegacyPayment);

// SCALE encodes this newtype exactly as its inner value. Preserve that same
// metadata identity even when the extrinsic's generic Extra type is inspected.
impl TypeInfo for AuthorizedHeartbeatPayment {
    type Identity = <LegacyPayment as TypeInfo>::Identity;

    fn type_info() -> scale_info::Type {
        LegacyPayment::type_info()
    }
}

impl From<ChargeTransactionPayment<Runtime>> for AuthorizedHeartbeatPayment {
    fn from(payment: ChargeTransactionPayment<Runtime>) -> Self {
        Self(LegacyPayment::from(LegacySignedPayment::from(payment)))
    }
}

impl AuthorizedHeartbeatPayment {
    fn is_heartbeat(call: &RuntimeCall) -> bool {
        matches!(
            call,
            RuntimeCall::ImOnline(pallet_im_online::Call::heartbeat { .. })
        )
    }

    fn is_authorized_heartbeat(origin: &RuntimeOrigin, call: &RuntimeCall) -> bool {
        matches!(
            origin.as_system_ref(),
            Some(frame_system::RawOrigin::Authorized)
        ) && Self::is_heartbeat(call)
    }

    #[expect(
        deprecated,
        reason = "Inspect the preserved signed-payment tip before admitting an authorized heartbeat"
    )]
    fn ensure_zero_tip(&self) -> Result<(), TransactionValidityError> {
        if self.0 .0 .0.tip().is_zero() {
            Ok(())
        } else {
            Err(InvalidTransaction::Payment.into())
        }
    }
}

impl TransactionExtension<RuntimeCall> for AuthorizedHeartbeatPayment {
    const IDENTIFIER: &'static str = LegacyPayment::IDENTIFIER;

    type Implicit = <LegacyPayment as TransactionExtension<RuntimeCall>>::Implicit;
    type Val = Option<<LegacyPayment as TransactionExtension<RuntimeCall>>::Val>;
    type Pre = Option<<LegacyPayment as TransactionExtension<RuntimeCall>>::Pre>;

    fn implicit(&self) -> Result<Self::Implicit, TransactionValidityError> {
        self.0.implicit()
    }

    fn metadata() -> Vec<TransactionExtensionMetadata> {
        LegacyPayment::metadata()
    }

    fn weight(&self, call: &RuntimeCall) -> Weight {
        self.0.weight(call)
    }

    fn validate(
        &self,
        origin: RuntimeOrigin,
        call: &RuntimeCall,
        info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
        self_implicit: Self::Implicit,
        inherited_implication: &impl Implication,
        source: TransactionSource,
    ) -> ValidateResult<Self::Val, RuntimeCall> {
        if Self::is_authorized_heartbeat(&origin, call) {
            self.ensure_zero_tip()?;
            return Ok((ValidTransaction::default(), None, origin));
        }

        self.0
            .validate(
                origin,
                call,
                info,
                len,
                self_implicit,
                inherited_implication,
                source,
            )
            .map(|(validity, val, origin)| (validity, Some(val), origin))
    }

    fn prepare(
        self,
        val: Self::Val,
        origin: &RuntimeOrigin,
        call: &RuntimeCall,
        info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Result<Self::Pre, TransactionValidityError> {
        match val {
            Some(val) => self.0.prepare(val, origin, call, info, len).map(Some),
            None if Self::is_authorized_heartbeat(origin, call) => {
                self.ensure_zero_tip()?;
                Ok(None)
            }
            None => Err(InvalidTransaction::BadSigner.into()),
        }
    }

    fn post_dispatch_details(
        pre: Self::Pre,
        info: &DispatchInfoOf<RuntimeCall>,
        post_info: &PostDispatchInfoOf<RuntimeCall>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<Weight, TransactionValidityError> {
        match pre {
            Some(pre) => LegacyPayment::post_dispatch_details(pre, info, post_info, len, result),
            None => Ok(Weight::zero()),
        }
    }

    fn post_dispatch(
        pre: Self::Pre,
        info: &DispatchInfoOf<RuntimeCall>,
        post_info: &mut PostDispatchInfoOf<RuntimeCall>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        match pre {
            Some(pre) => LegacyPayment::post_dispatch(pre, info, post_info, len, result),
            None => Ok(()),
        }
    }

    fn bare_validate(
        call: &RuntimeCall,
        info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> TransactionValidity {
        if Self::is_heartbeat(call) {
            return InvalidTransaction::Call.into();
        }
        LegacyPayment::bare_validate(call, info, len)
    }

    fn bare_validate_and_prepare(
        call: &RuntimeCall,
        info: &DispatchInfoOf<RuntimeCall>,
        len: usize,
    ) -> Result<(), TransactionValidityError> {
        // This adapter must remain outside SkipCheckIfFeeless: its default bare
        // callbacks do not delegate to the wrapped extension.
        if Self::is_heartbeat(call) {
            return Err(InvalidTransaction::Call.into());
        }
        LegacyPayment::bare_validate_and_prepare(call, info, len)
    }

    fn bare_post_dispatch(
        info: &DispatchInfoOf<RuntimeCall>,
        post_info: &mut PostDispatchInfoOf<RuntimeCall>,
        len: usize,
        result: &DispatchResult,
    ) -> Result<(), TransactionValidityError> {
        LegacyPayment::bare_post_dispatch(info, post_info, len, result)
    }
}
