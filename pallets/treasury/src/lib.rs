#![deny(warnings)]
#![cfg_attr(test, feature(proc_macro_hygiene))]
#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use core::convert::{TryFrom, TryInto};
use core::marker::PhantomData;
use frame_support::traits::{Currency, ExistenceRequirement::KeepAlive, ReservableCurrency};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
};
use iroha_client_no_std::prelude::AssetDefinitionId;
use sp_runtime::ModuleId;
use sp_std::str::FromStr;
use system as frame_system;
use system::ensure_signed;

/// The treasury's module id, used for deriving its sovereign account ID.
const _MODULE_ID: ModuleId = ModuleId(*b"ily/trsy");

#[derive(Encode, Decode, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    XOR,
    DOT,
    KSM,
}

impl AssetKind {
    pub fn definition_id(&self) -> AssetDefinitionId {
        match self {
            AssetKind::XOR => AssetDefinitionId::new("XOR", "global"),
            AssetKind::DOT => AssetDefinitionId::new("DOT", "polkadot"),
            AssetKind::KSM => AssetDefinitionId::new("KSM", "polkadot"),
        }
    }
}

impl<'a> TryFrom<&'a AssetDefinitionId> for AssetKind {
    type Error = ();

    fn try_from(asset_def_id: &'a AssetDefinitionId) -> Result<Self, Self::Error> {
        match asset_def_id {
            x if x == &AssetDefinitionId::new("XOR", "global") => Ok(AssetKind::XOR),
            x if x == &AssetDefinitionId::new("DOT", "polkadot") => Ok(AssetKind::DOT),
            x if x == &AssetDefinitionId::new("KSM", "polkadot") => Ok(AssetKind::KSM),
            _ => Err(()),
        }
    }
}

impl FromStr for AssetKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "XOR" => Ok(AssetKind::XOR),
            "DOT" => Ok(AssetKind::DOT),
            "KSM" => Ok(AssetKind::KSM),
            _ => Err(()),
        }
    }
}

/// The pallet's configuration trait.
/// Instantiation of this pallet requires the existence of a module that
/// implements Currency and ReservableCurrency. The Balances module can be used
/// for this. The Balances module then gives functions for total supply, balances
/// of accounts, and any function defined by the Currency and ReservableCurrency
/// traits.
pub trait Trait: system::Trait {
    type XOR: Currency<<Self as system::Trait>::AccountId>
        + ReservableCurrency<<Self as system::Trait>::AccountId>;
    type DOT: Currency<<Self as system::Trait>::AccountId>
        + ReservableCurrency<<Self as system::Trait>::AccountId>;
    type KSM: Currency<<Self as system::Trait>::AccountId>
        + ReservableCurrency<<Self as system::Trait>::AccountId>;

    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub fn balance_to_num<T: Trait, C: Currency<<T as system::Trait>::AccountId>>(
    amount: C::Balance,
) -> Result<u128, Error<T>> {
    amount
        .try_into()
        .map_err(|_| <Error<T>>::InvalidBalanceType)?
        .try_into()
        .map_err(|_| <Error<T>>::InvalidBalanceType)
}

pub fn num_to_balance<T: Trait, C: Currency<<T as system::Trait>::AccountId>>(
    amount_num: u128,
) -> Result<C::Balance, Error<T>> {
    C::Balance::try_from(usize::try_from(amount_num).map_err(|_| <Error<T>>::InvalidBalanceType)?)
        .map_err(|_| <Error<T>>::InvalidBalanceType)
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        InsufficientLockedFunds,
        InsufficientFunds,
        InvalidBalanceType,
        InvalidAssetName,
    }
}

// This pallet's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as Treasury {
        /// ## Storage
        /// Note that account's balances and locked balances are handled
        /// through the Balances module.
        ///
        /// Total locked PolkaDOT
        TotalLocked: u128;
    }
}

// The pallet's events
decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        Transfer(AccountId, AccountId, u128),
        Mint(AccountId, u128),
        Lock(AccountId, u128),
        Unlock(AccountId, u128),
        Burn(AccountId, u128),
    }
);

// The pallet's dispatchable functions.
decl_module! {
    /// The module declaration.
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // Initializing events
        // this is needed only if you are using events in your pallet
        fn deposit_event() = default;

        /// Transfer an amount of XOR (without fees)
        ///
        /// # Arguments
        ///
        /// * `origin` - sender of the transaction
        /// * `receiver` - receiver of the transaction
        /// * `amount` - amount of XOR
        #[weight = 1000]
        fn transfer(origin, asset_kind: AssetKind, receiver: T::AccountId, amount_num: u128)
            -> DispatchResult
        {
            let sender = ensure_signed(origin)?;

            match asset_kind {
                AssetKind::XOR => {
                    let amount = <T::XOR as Currency<_>>::Balance::try_from(usize::try_from(amount_num).map_err(|_| <Error<T>>::InvalidBalanceType)?).map_err(|_| <Error<T>>::InvalidBalanceType)?;
                    T::XOR::transfer(&sender, &receiver, amount, KeepAlive)
                }
                AssetKind::DOT => {
                    let amount = <T::DOT as Currency<_>>::Balance::try_from(usize::try_from(amount_num).map_err(|_| <Error<T>>::InvalidBalanceType)?).map_err(|_| <Error<T>>::InvalidBalanceType)?;
                    T::DOT::transfer(&sender, &receiver, amount, KeepAlive)
                }
                AssetKind::KSM => {
                    let amount = num_to_balance::<T, T::KSM>(amount_num)?; // <T::KSM as Currency<_>>::Balance::try_from(usize::try_from(amount_num).map_err(|_| <Error<T>>::InvalidBalanceType)?).map_err(|_| <Error<T>>::InvalidBalanceType)?;
                    T::KSM::transfer(&sender, &receiver, amount, KeepAlive)
                }
            }.map_err(|_| <Error<T>>::InsufficientFunds)?;

            Self::deposit_event(RawEvent::Transfer(sender, receiver, amount_num));

            Ok(())
        }
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq)]
pub struct Asset<T, C> {
    _t: PhantomData<T>,
    _c: PhantomData<C>,
}

impl<T, C> Asset<T, C>
where
    T: Trait,
    C: Currency<T::AccountId> + ReservableCurrency<T::AccountId>,
{
    /// Total supply of XOR
    #[allow(unused)]
    pub fn get_total_supply() -> C::Balance {
        C::total_issuance()
    }
    /// Balance of an account (wrapper)
    #[allow(unused)]
    pub fn get_balance_from_account(account: T::AccountId) -> C::Balance {
        C::free_balance(&account)
    }
    /// Locked balance of an account (wrapper)
    #[allow(unused)]
    pub fn get_locked_balance_from_account(account: T::AccountId) -> C::Balance {
        C::reserved_balance(&account)
    }
    /// Increase the supply of locked XOR
    #[allow(unused)]
    pub fn increase_total_locked(amount_num: u128) -> Result<(), Error<T>> {
        let new_locked = TotalLocked::get() + amount_num;
        TotalLocked::put(new_locked);
        Ok(())
    }
    /// Decrease the supply of locked XOR
    #[allow(unused)]
    pub fn decrease_total_locked(amount_num: u128) -> Result<(), Error<T>> {
        let new_locked = TotalLocked::get() - amount_num;
        TotalLocked::put(new_locked);
        Ok(())
    }
    /// Mint new tokens
    ///
    /// # Arguments
    ///
    /// * `requester` - XOR user requesting new tokens
    /// * `amount` - to be issued amount of XOR
    #[allow(unused)]
    pub fn mint(requester: T::AccountId, amount_num: u128) -> Result<(), Error<T>> {
        // adds the amount to the total balance of tokens
        let amount = num_to_balance::<T, C>(amount_num)?;
        let minted_tokens = C::issue(amount);
        // adds the added amount to the requester's balance
        C::resolve_creating(&requester, minted_tokens);

        Module::<T>::deposit_event(RawEvent::Mint(requester, amount_num));
        Ok(())
    }

    /// Lock XOR tokens to burn them. Note: this removes them from the
    /// free balance of XOR and adds them to the locked supply of XOR.
    ///
    /// # Arguments
    ///
    /// * `redeemer` - the account redeeming tokens
    /// * `amount` - to be locked amount of XOR
    #[allow(unused)]
    pub fn lock(redeemer: T::AccountId, amount_num: u128) -> Result<(), Error<T>> {
        let amount = num_to_balance::<T, C>(amount_num)?;
        C::reserve(&redeemer, amount).map_err(|_| <Error<T>>::InsufficientFunds)?;

        // update total locked balance
        Self::increase_total_locked(amount_num)?;
        Module::<T>::deposit_event(RawEvent::Lock(redeemer, amount_num));
        Ok(())
    }

    #[allow(unused)]
    pub fn unlock(redeemer: T::AccountId, amount_num: u128) -> Result<(), Error<T>> {
        let amount = num_to_balance::<T, C>(amount_num)?;
        C::unreserve(&redeemer, amount);

        // update total locked balance
        Self::decrease_total_locked(amount_num)?;
        Module::<T>::deposit_event(RawEvent::Unlock(redeemer, amount_num));
        Ok(())
    }

    /// Burn previously locked XOR tokens
    ///
    /// # Arguments
    ///
    /// * `redeemer` - the account redeeming tokens
    /// * `amount` - the to be burned amount of XOR
    #[allow(unused)]
    pub fn burn(redeemer: T::AccountId, amount_num: u128) -> Result<(), Error<T>> {
        let amount = num_to_balance::<T, C>(amount_num)?;
        ensure!(
            C::reserved_balance(&redeemer) >= amount,
            <Error<T>>::InsufficientLockedFunds
        );

        // burn the tokens from the locked balance
        Self::decrease_total_locked(amount_num)?;
        // burn the tokens for the redeemer
        // remainder should always be 0 and is checked above
        let (_burned_tokens, _remainder) = C::slash_reserved(&redeemer, amount);

        Module::<T>::deposit_event(RawEvent::Burn(redeemer, amount_num));
        Ok(())
    }
}

impl<T: Trait> Module<T> {
    pub fn mint(
        requester: T::AccountId,
        asset_kind: AssetKind,
        amount_num: u128,
    ) -> Result<(), Error<T>> {
        match asset_kind {
            AssetKind::XOR => Asset::<T, T::XOR>::mint(requester, amount_num),
            AssetKind::DOT => Asset::<T, T::DOT>::mint(requester, amount_num),
            AssetKind::KSM => Asset::<T, T::KSM>::mint(requester, amount_num),
        }
    }

    pub fn lock(
        redeemer: T::AccountId,
        asset_kind: AssetKind,
        amount_num: u128,
    ) -> Result<(), Error<T>> {
        match asset_kind {
            AssetKind::XOR => Asset::<T, T::XOR>::lock(redeemer, amount_num),
            AssetKind::DOT => Asset::<T, T::DOT>::lock(redeemer, amount_num),
            AssetKind::KSM => Asset::<T, T::KSM>::lock(redeemer, amount_num),
        }
    }

    pub fn unlock(
        redeemer: T::AccountId,
        asset_kind: AssetKind,
        amount_num: u128,
    ) -> Result<(), Error<T>> {
        match asset_kind {
            AssetKind::XOR => Asset::<T, T::XOR>::unlock(redeemer, amount_num),
            AssetKind::DOT => Asset::<T, T::DOT>::unlock(redeemer, amount_num),
            AssetKind::KSM => Asset::<T, T::KSM>::unlock(redeemer, amount_num),
        }
    }

    pub fn burn(
        redeemer: T::AccountId,
        asset_kind: AssetKind,
        amount_num: u128,
    ) -> Result<(), Error<T>> {
        match asset_kind {
            AssetKind::XOR => Asset::<T, T::XOR>::burn(redeemer, amount_num),
            AssetKind::DOT => Asset::<T, T::DOT>::burn(redeemer, amount_num),
            AssetKind::KSM => Asset::<T, T::KSM>::burn(redeemer, amount_num),
        }
    }

    pub fn get_balance_from_account(
        account: T::AccountId,
        asset_kind: AssetKind,
    ) -> Result<u128, Error<T>> {
        match asset_kind {
            AssetKind::XOR => {
                balance_to_num::<T, T::XOR>(Asset::<T, T::XOR>::get_balance_from_account(account))
            }
            AssetKind::DOT => {
                balance_to_num::<T, T::DOT>(Asset::<T, T::DOT>::get_balance_from_account(account))
            }
            AssetKind::KSM => {
                balance_to_num::<T, T::KSM>(Asset::<T, T::KSM>::get_balance_from_account(account))
            }
        }
    }
}
