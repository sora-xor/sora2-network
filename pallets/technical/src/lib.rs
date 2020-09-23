#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use common::{PureOrWrapped, SwapAction, SwapRulesValidation};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, Parameter};
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::Member;
use sp_runtime::RuntimeDebug;
use sp_std::marker::PhantomData;

use common::TECH_ACCOUNT_MAGIC_PREFIX;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type AccountIdOf<T> = <T as frame_system::Trait>::AccountId;

type AssetIdOf<T> = <T as assets::Trait>::AssetId;

type BalanceOf<T> = <<T as currencies::Trait>::MultiCurrency as MultiCurrency<
    <T as frame_system::Trait>::AccountId,
>>::Balance;

type AmountOf<T> = <<T as currencies::Trait>::MultiCurrency as MultiCurrencyExtended<
    <T as frame_system::Trait>::AccountId,
>>::Amount;

type TechAccountIdOf<T> = TechAccountIdReprCompat<T, <T as Trait>::TechAccountIdPrimitive>;

type TechAccountIdPrimitiveOf<T> = <T as Trait>::TechAccountIdPrimitive;

/// Pending atomic swap operation.
#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct PendingSwap<T: Trait> {
    /// Source of the swap.
    pub source: T::AccountId,
    /// Action of this swap.
    pub action: T::SwapAction,
    /// Condition is time or block number, or something logical.
    pub condition: T::Condition,
}

/// For case if TechAccountId can be encoded as 128bit hash link in AccountId as representative.
#[derive(Clone, Eq, PartialEq, Encode, Decode, PartialOrd, Ord)]
pub struct TechAccountIdReprCompat<T: Trait, Primitive>(pub Primitive, PhantomData<T>);

/// It is needed because PartialOrd is not implemented for Runtime.
impl<T: Trait, Primitive: sp_std::fmt::Debug> sp_std::fmt::Debug
    for TechAccountIdReprCompat<T, Primitive>
{
    fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> Result<(), sp_std::fmt::Error> {
        self.0.fmt(f)
    }
}

/// This implementation adds ability to lookup TechAccountId from represeitative (AccountId).
impl<T: Trait> From<AccountId32> for TechAccountIdOf<T>
where
    AccountIdOf<T>: From<AccountId32>,
    AccountId32: From<AccountIdOf<T>>,
    TechAccountIdPrimitiveOf<T>: common::WrappedRepr<AccountId32>,
{
    fn from(a: AccountId32) -> Self {
        let b: [u8; 32] = a.clone().into();
        let c = AccountIdOf::<T>::from(a.clone());
        if b[0..16] == TECH_ACCOUNT_MAGIC_PREFIX {
            match Module::<T>::lookup_pure_tech_account_id_from_repr(AccountIdOf::<T>::from(
                a.clone(),
            )) {
                Some(x) => x,
                None => TechAccountIdReprCompat(
                    common::WrappedRepr::<AccountId32>::wrapped_repr(a),
                    PhantomData,
                ),
            }
        } else {
            TechAccountIdReprCompat(TechAccountIdPrimitiveOf::<T>::from(c), PhantomData)
        }
    }
}

/// This implementation adds ability to encode TechAccountId into representative.
impl<T: Trait> From<TechAccountIdOf<T>> for AccountId32
where
    AccountId32: From<AccountIdOf<T>>,
{
    fn from(a: TechAccountIdOf<T>) -> Self {
        let b: Option<AccountIdOf<T>> = a.clone().0.into();
        match b.clone() {
            Some(x) => x.into(),
            // Encode not wrapped technical account into hash repesentation for
            // compatibility with AccountId
            // random magic prefix is used, size is 128 bit for prefix
            // using twox because cryptographically hash is not needed
            // quality of distribution of twox is good enouth, and it is fast.
            None => {
                use ::core::hash::Hasher;
                let data = &a.clone().encode();
                let mut h0 = twox_hash::XxHash::with_seed(0);
                let mut h1 = twox_hash::XxHash::with_seed(1);
                h0.write(data);
                h1.write(data);
                let r0 = h0.finish();
                let r1 = h1.finish();
                let mut repr: [u8; 32] = [0; 32];
                repr[0..16].copy_from_slice(&TECH_ACCOUNT_MAGIC_PREFIX);
                repr[16..24].copy_from_slice(&r0.to_le_bytes());
                repr[24..32].copy_from_slice(&r1.to_le_bytes());
                repr.into()
            }
        }
    }
}

/// This implementation adds Option compatibility
impl<T: Trait> From<TechAccountIdOf<T>> for Option<AccountId32>
where
    AccountId32: From<TechAccountIdOf<T>>,
    TechAccountIdPrimitiveOf<T>: common::WrappedRepr<AccountId32>,
{
    fn from(a: TechAccountIdOf<T>) -> Self {
        Some(AccountId32::from(a.into()))
    }
}

/// This is just PureOrWrapped wrapper and some type comstraints for other dependant
/// implementations.
impl<T: Trait> common::PureOrWrapped<AccountId32> for TechAccountIdOf<T>
where
    AccountIdOf<T>: From<AccountId32>,
    AccountId32: From<AccountIdOf<T>>,
    TechAccountIdPrimitiveOf<T>: common::WrappedRepr<AccountId32>,
{
    fn is_pure(&self) -> bool {
        self.0.is_pure()
    }
    fn is_wrapped_regular(&self) -> bool {
        self.0.is_wrapped_regular()
    }
    fn is_wrapped(&self) -> bool {
        self.0.is_wrapped()
    }
}

/// Configure the pallet by specifying the parameters and types on which it depends.
pub trait Trait: common::Trait + assets::Trait {
    /// Because this pallet emits events, it depends on the runtime's definition of an event.
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    /// Like Asset but deterministically maked from purpose.
    type TechAssetId: Ord + Member + Parameter + PureOrWrapped<<Self as assets::Trait>::AssetId>;

    /// Like AccountId but controlled by consensus, not signing by user.
    type TechAccountIdPrimitive: Ord
        + Member
        + Parameter
        + PureOrWrapped<<Self as frame_system::Trait>::AccountId>;

    /// The units in which we record amount.
    type TechAmount: Default + Copy + PureOrWrapped<AmountOf<Self>>;

    /// The units in which we record amount.
    type TechBalance: Default + Copy + Member + Parameter + PureOrWrapped<BalanceOf<Self>>;

    /// Trigger for auto claim.
    type Trigger: Default + Copy + Member + Parameter;

    /// Condition for auto claim.
    type Condition: Default + Copy + Member + Parameter;

    /// Swap action.
    type SwapAction: common::SwapRulesValidation<Self::AccountId, TechAccountIdOf<Self>, Self>
        + Parameter;
}

decl_storage! {
    trait Store for Module<T: Trait> as Technical
    {

        /// Map from repr (AccountId) into pure (TechAccountId).
        TechAccounts: map hasher(blake2_128_concat) AccountIdOf<T> => Option<TechAccountIdOf<T>>;

        /// Swaps waiting for triggers.
        PendingSwaps: double_map hasher(blake2_128_concat) (TechAccountIdOf<T>, T::TechAssetId), hasher(blake2_128_concat) T::Trigger => Option<PendingSwap<T>>;

    }
}

impl<T: Trait> Module<T>
where
    AccountIdOf<T>: From<AccountId32>,
    AccountId32: From<AccountIdOf<T>>,
    TechAccountIdPrimitiveOf<T>: common::WrappedRepr<AccountId32>,
{
    /// Get `TechAccountId` used in technical pallet from primitive constructor.
    /// For example it can be primitive for technical account from common pallet.
    pub fn tech_acc_id_from_primitive(
        primitive: TechAccountIdPrimitiveOf<T>,
    ) -> TechAccountIdOf<T> {
        TechAccountIdReprCompat(primitive, PhantomData)
    }

    /// Check `TechAccountId` for registration in storage map.
    pub fn is_tech_account_id_registered(
        tech_account_id: TechAccountIdOf<T>,
    ) -> Result<bool, DispatchError> {
        if !common::PureOrWrapped::<AccountId32>::is_pure(&tech_account_id.clone()) {
            return Ok(false);
        }
        let repr32 = tech_account_id.clone().into();
        let repr = AccountIdOf::<T>::from(repr32);
        match Self::lookup_pure_tech_account_id_from_repr(repr) {
            Some(_) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Register `TechAccountId` in storate map.
    pub fn register_tech_account_id(tech_account_id: TechAccountIdOf<T>) -> DispatchResult {
        ensure!(
            common::PureOrWrapped::<AccountId32>::is_pure(&tech_account_id.clone()),
            Error::<T>::TechAccountIdMustBePure
        );
        let repr32 = tech_account_id.clone().into();
        let repr = AccountIdOf::<T>::from(repr32);
        <TechAccounts<T>>::insert(repr, tech_account_id);
        Ok(())
    }

    /// Lookup `TechAccountId` from storage map by `AccountId` as representation.
    pub fn lookup_pure_tech_account_id_from_repr(
        repr: AccountIdOf<T>,
    ) -> Option<TechAccountIdOf<T>> {
        <TechAccounts<T>>::get(&repr)
    }

    /// Set storage changes in assets to transfer specific asset from regular `AccountId` into pure `TechAccountId`.
    pub fn set_transfer_in(
        asset: AssetIdOf<T>,
        source: <T as frame_system::Trait>::AccountId,
        tech_dest: TechAccountIdOf<T>,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        ensure!(
            common::PureOrWrapped::<AccountId32>::is_pure(&tech_dest.clone()),
            Error::<T>::OnlyPureTechnicalAccount
        );
        ensure!(
            Self::is_tech_account_id_registered(tech_dest.clone())?,
            Error::<T>::TechAccountIdIsNotRegistered
        );
        let repr32 = tech_dest.clone().into();
        let repr = AccountIdOf::<T>::from(repr32);
        assets::Module::<T>::transfer(&asset, &source, &repr, amount)?;
        Ok(())
    }

    /// Set storage changes in assets to transfer specific asset from pure `TechAccountId` into pure `AccountId`.
    pub fn set_transfer_out(
        asset: AssetIdOf<T>,
        tech_source: TechAccountIdOf<T>,
        dest: <T as frame_system::Trait>::AccountId,
        amount: BalanceOf<T>,
    ) -> DispatchResult {
        ensure!(
            common::PureOrWrapped::<AccountId32>::is_pure(&tech_source.clone()),
            Error::<T>::OnlyPureTechnicalAccount
        );
        ensure!(
            Self::is_tech_account_id_registered(tech_source.clone())?,
            Error::<T>::TechAccountIdIsNotRegistered
        );
        let repr32 = tech_source.clone().into();
        let repr = AccountIdOf::<T>::from(repr32);
        assets::Module::<T>::transfer(&asset, &repr, &dest, amount)?;
        Ok(())
    }
}

decl_event!(
    pub enum Event<T> where AccountId = AccountIdOf<T>,
        TechAccountId = TechAccountIdOf<T>,
        <T as Trait>::TechAssetId,
        <T as Trait>::TechBalance,
        <T as Trait>::TechAmount,

    {
        /// Some pure technical assets were minted. [asset, owner, minted_amount, total_exist].
        /// This is not only for pure TechAccountId.
        /// TechAccountId can be just wrapped AccountId.
        Minted(TechAssetId, TechAccountId, TechAmount, TechBalance),

        /// Some pure technical assets were burned. [asset, owner, burned_amount, total_exist].
        /// For full kind of accounts like in Minted.
        Burned(TechAssetId, TechAccountId, TechAmount, TechBalance),

        /// Some assets were transferred out. [asset, from, to, amount].
        /// TechAccountId is only pure TechAccountId.
        OutputTransferred(TechAssetId, TechAccountId, AccountId, TechAmount),

        /// Some assets were transferred in. [asset, from, to, amount].
        /// TechAccountId is only pure TechAccountId.
        InputTransferred(TechAssetId, AccountId, TechAccountId, TechAmount),

        /// Swap operaction is finalised [initiator, finaliser].
        /// TechAccountId is only pure TechAccountId.
        SwapSuccess(AccountId),

    }
);

// All this errors is needed and used or will be used
decl_error! {
    pub enum Error for Module<T: Trait> {
        /// Errors should have helpful documentation associated with them.
        StorageOverflow,
        /// Balance too low to send value.
        InsufficientBalance,
        /// Swap already exists.
        AlreadyExist,
        /// Swap proof is invalid.
        InvalidProof,
        /// Source does not match.
        SourceMismatch,
        /// Swap has already been claimed.
        AlreadyClaimed,
        /// Claim action mismatch.
        ClaimActionMismatch,
        /// Duration has not yet passed for the swap to be cancelled.
        DurationNotPassed,
        /// If argument must be technical, and only regular values inside it is allowed
        OnlyRegularAsset,
        /// If argument must be technical, and only regular values inside it is allowed
        OnlyRegularAccount,
        /// If argument must be technical, and only regular values inside it is allowed
        OnlyRegularBalance,
        /// If argument must be technical, and only regular values inside it is allowed
        OnlyPureTechnicalAccount,
        /// Got an overflow after adding.
        Overflow,
        /// If argument must be technical, and only pure technical value is allowed
        TechAccountIdMustBePure,
        /// It is not posible to extract code from `AccountId32` as representation
        /// or find it in storage.
        UnableToGetReprFromTechAccountId,
        /// Type must sport mapping from hash to special subset of `AccountId32`
        RepresentativeMustBeSupported,
        /// It is not posible to find record in storage map about `AccountId32` representation for
        /// technical account.
        TechAccountIdIsNotRegistered,
        /// This function or ablility is still not implemented.
        NotImplemented,
    }
}

//Keep it bacause posible will be needed.
//pub type Reasons = pallet_balances::Reasons;
//pub type BalanceLock<T: Trait> = pallet_balances::BalanceLock<T::TechBalance>;
//pub type AccountData<T: Trait> = pallet_balances::AccountData<T::TechBalance>;

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        #[weight = 0]
        fn create_swap(
            origin,
            action: T::SwapAction,
        ) -> DispatchResult {
            let source = ensure_signed(origin)?;
            if action.validate(&source) {
                action.reserve(&source)?;
                if action.is_able_to_claim() {
                    if action.instant_auto_claim_used() {
                        if action.claim(&source) {
                            Self::deposit_event(RawEvent::SwapSuccess(source));
                        } else if !action.triggered_auto_claim_used() {
                            action.cancel(&source);
                        } else {
                            return Err(Error::<T>::NotImplemented)?;
                        }
                    } else {
                        return Err(Error::<T>::NotImplemented)?;
                    }
                } else if action.triggered_auto_claim_used() {
                    return Err(Error::<T>::NotImplemented)?;
                } else {
                    return Err(Error::<T>::NotImplemented)?;
                }
            }
            Ok(())
        }
    }
}
