//! # Assets Module
//!
//! ## Overview
//!
//! The assets module serves as an extension of `currencies` pallet.
//! It allows to explicitly register new assets and store their owners' account IDs.
//! This allows to restrict some actions on assets for non-owners.
//!
//! ### Dispatchable Functions
//!
//! - `register` - registers new asset by a given ID.

// TODO: add info about weight

#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

mod weights;

mod benchmarking;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use common::{hash, prelude::Balance, Amount, AssetSymbol, BalancePrecision};
use frame_support::dispatch::{DispatchError, DispatchResult};
use frame_support::sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, weights::Weight,
    IterableStorageMap, Parameter,
};
use frame_system::ensure_signed;
use permissions::{Scope, BURN, MINT, SLASH, TRANSFER};
use sp_core::hash::H512;
use sp_core::H256;
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;
use tiny_keccak::{Hasher, Keccak};
use traits::{
    MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency, MultiReservableCurrency,
};

pub trait WeightInfo {
    fn register() -> Weight;
    fn transfer() -> Weight;
    fn mint() -> Weight;
    fn burn() -> Weight;
}

pub type AssetIdOf<T> = <T as Trait>::AssetId;
pub type Permissions<T> = permissions::Module<T>;

type CurrencyIdOf<T> =
    <<T as Trait>::Currency as MultiCurrency<<T as frame_system::Trait>::AccountId>>::CurrencyId;

const ASSET_SYMBOL_MAX_LENGTH: usize = 7;
const MAX_ALLOWED_PRECISION: u8 = 18;

#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode)]
pub enum TupleArg<T: Trait> {
    GenericI32(i32),
    GenericU64(u64),
    GenericU128(u128),
    GenericU8x32([u8; 32]),
    GenericH256(H256),
    GenericH512(H512),
    LeafAssetId(AssetIdOf<T>),
    TupleAssetId(AssetIdOf<T>),
    Extra(T::ExtraTupleArg),
}

#[derive(Clone, Copy, Eq, PartialEq, Encode, Decode)]
pub enum Tuple<T: Trait> {
    Arity0,
    Arity1(TupleArg<T>),
    Arity2(TupleArg<T>, TupleArg<T>),
    Arity3(TupleArg<T>, TupleArg<T>, TupleArg<T>),
    Arity4(TupleArg<T>, TupleArg<T>, TupleArg<T>, TupleArg<T>),
    Arity5(
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
    ),
    Arity6(
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
    ),
    Arity7(
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
    ),
    Arity8(
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
    ),
    Arity9(
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
        TupleArg<T>,
    ),
}

pub trait Trait: frame_system::Trait + permissions::Trait + tokens::Trait + common::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

    type ExtraAccountId: Clone
        + Copy
        + Encode
        + Decode
        + Eq
        + PartialEq
        + From<Self::AccountId>
        + Into<Self::AccountId>;
    type ExtraTupleArg: Clone
        + Copy
        + Encode
        + Decode
        + Eq
        + PartialEq
        + From<common::AssetIdExtraTupleArg<Self::DEXId, Self::LstId, Self::ExtraAccountId>>
        + Into<common::AssetIdExtraTupleArg<Self::DEXId, Self::LstId, Self::ExtraAccountId>>;

    /// DEX assets (currency) identifier.
    type AssetId: Parameter
        + Member
        + Copy
        + MaybeSerializeDeserialize
        + Ord
        + Default
        + Into<CurrencyIdOf<Self>>
        + From<common::AssetId32<common::AssetId>>
        + From<H256>
        + Into<H256>
        + Into<<Self as tokens::Trait>::CurrencyId>;

    /// The base asset as the core asset in all trading pairs
    type GetBaseAssetId: Get<Self::AssetId>;

    /// Currency to transfer, reserve/unreserve, lock/unlock assets
    type Currency: MultiLockableCurrency<
            Self::AccountId,
            Moment = Self::BlockNumber,
            CurrencyId = Self::AssetId,
            Balance = Balance,
        > + MultiReservableCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>
        + MultiCurrencyExtended<Self::AccountId, Amount = Amount>;

    /// Weight information for extrinsics in this pallet.
    type WeightInfo: WeightInfo;
}

decl_storage! {
    trait Store for Module<T: Trait> as AssetsModule {
        /// Asset Id -> Owner Account Id
        AssetOwners get(fn asset_owners): map hasher(twox_64_concat) T::AssetId => T::AccountId;
        /// Asset Id -> (Symbol, Precision, Is Mintable)
        pub AssetInfos get(fn asset_infos): map hasher(twox_64_concat) T::AssetId => (AssetSymbol, BalancePrecision, bool);
        /// Asset Id -> Tuple<T>
        pub TupleAssetId get(fn tuple_from_asset_id): map hasher(twox_64_concat) T::AssetId => Option<Tuple<T>>;
    }
    add_extra_genesis {
        config(endowed_assets): Vec<(T::AssetId, T::AccountId, AssetSymbol, BalancePrecision, Balance, bool)>;

        build(|config: &GenesisConfig<T>| {
            config.endowed_assets.iter().cloned().for_each(|(asset_id, account_id, symbol, precision, initial_supply, is_mintable)| {
                Module::<T>::register_asset_id(account_id, asset_id, symbol, precision, initial_supply, is_mintable)
                    .expect("Failed to register asset.");
            })
        })
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        AssetId = <T as Trait>::AssetId,
    {
        /// New asset has been registered. [Asset Id, Asset Owner Account]
        AssetRegistered(AssetId, AccountId),
        /// Asset amount has been transfered. [From Account, To Account, Tranferred Asset Id, Amount Transferred]
        Transfer(AccountId, AccountId, AssetId, Balance),
        /// Asset amount has been minted. [Issuer Account, Target Account, Minted Asset Id, Amount Minted]
        Mint(AccountId, AccountId, AssetId, Balance),
        /// Asset amount has been burned. [Issuer Account, Burned Asset Id, Amount Burned]
        Burn(AccountId, AssetId, Balance),
        /// Asset is set as non-mintable. [Target Asset Id]
        AssetSetNonMintable(AssetId),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        /// An asset with a given ID already exists.
        AssetIdAlreadyExists,
        /// An asset with a given ID not exists.
        AssetIdNotExists,
        /// A number is out of range of the balance type.
        InsufficientBalance,
        /// Symbol is not valid. It must contain only uppercase latin characters, length <= 7.
        InvalidAssetSymbol,
        /// Precision value is not valid, it should represent a number of decimal places for number, max is 30.
        InvalidPrecision,
        /// Minting for particular asset id is disabled.
        AssetSupplyIsNotMintable,
        /// Caller does not own requested asset.
        InvalidAssetOwner,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;

        fn deposit_event() = default;

        /// Performs an asset registration.
        ///
        /// Registers new `AssetId` for the given `origin`.
        /// AssetSymbol should represent string with only uppercase latin chars with max length of 5.
        #[weight = <T as Trait>::WeightInfo::register()]
        pub fn register(origin, symbol: AssetSymbol, precision: BalancePrecision, initial_supply: Balance, is_mintable: bool) -> DispatchResult {
            let author = ensure_signed(origin)?;
            let _asset_id = Self::register_from(&author, symbol, precision, initial_supply, is_mintable)?;
            Ok(())
        }

        /// Performs a checked Asset transfer.
        ///
        /// - `origin`: caller Account, from which Asset amount is withdrawn,
        /// - `asset_id`: Id of transferred Asset,
        /// - `to`: Id of Account, to which Asset amount is deposited,
        /// - `amount`: transferred Asset amount.
        #[weight = <T as Trait>::WeightInfo::transfer()]
        pub fn transfer(
            origin,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance
        ) -> DispatchResult {
            let from = ensure_signed(origin.clone())?;
            Self::transfer_from(&asset_id, &from, &to, amount)?;
            Self::deposit_event(RawEvent::Transfer(from, to, asset_id, amount));
            Ok(())
        }

        /// Performs a checked Asset mint, can only be done
        /// by corresponding asset owner account.
        ///
        /// - `origin`: caller Account, which issues Asset minting,
        /// - `asset_id`: Id of minted Asset,
        /// - `to`: Id of Account, to which Asset amount is minted,
        /// - `amount`: minted Asset amount.
        #[weight = <T as Trait>::WeightInfo::mint()]
        pub fn mint(
            origin,
            asset_id: T::AssetId,
            to: T::AccountId,
            amount: Balance,
        ) -> DispatchResult {
            let issuer = ensure_signed(origin.clone())?;
            Self::mint_to(&asset_id, &issuer, &to, amount)?;
            Self::deposit_event(RawEvent::Mint(issuer, to, asset_id.clone(), amount));
            Ok(())
        }

        /// Performs a checked Asset burn, can only be done
        /// by corresponding asset owner with own account.
        ///
        /// - `origin`: caller Account, from which Asset amount is burned,
        /// - `asset_id`: Id of burned Asset,
        /// - `amount`: burned Asset amount.
        #[weight = <T as Trait>::WeightInfo::burn()]
        pub fn burn(
            origin,
            asset_id: T::AssetId,
            amount: Balance,
        ) -> DispatchResult {
            let issuer = ensure_signed(origin.clone())?;
            Self::burn_from(&asset_id, &issuer, &issuer, amount)?;
            Self::deposit_event(RawEvent::Burn(issuer, asset_id.clone(), amount));
            Ok(())
        }

        /// Set given asset to be non-mintable, i.e. it can no longer be minted, only burned.
        /// Operation can not be undone.
        ///
        /// - `origin`: caller Account, should correspond to Asset owner
        /// - `asset_id`: Id of burned Asset,
        #[weight = 0]
        pub fn set_non_mintable(
            origin,
            asset_id: T::AssetId,
        ) -> DispatchResult {
            let who = ensure_signed(origin.clone())?;
            Self::set_non_mintable_from(&asset_id, &who)?;
            Self::deposit_event(RawEvent::AssetSetNonMintable(asset_id.clone()));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    /// Generates an `AssetId` for the given `Tuple<T>`, and insert record to storage map.
    pub fn register_asset_id_from_tuple(tuple: &Tuple<T>) -> T::AssetId {
        let mut keccak = Keccak::v256();
        keccak.update(b"From Tuple");
        keccak.update(&tuple.encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        // More safe to escape.
        output[0] = 0u8;
        let asset_id = T::AssetId::from(H256(output));
        TupleAssetId::<T>::insert(asset_id, tuple);
        asset_id
    }

    /// Generates an `AssetId` for the given `AccountId`.
    pub fn gen_asset_id(account_id: &T::AccountId) -> T::AssetId {
        let mut keccak = Keccak::v256();
        keccak.update(b"Sora Asset Id");
        keccak.update(&account_id.encode());
        keccak.update(&frame_system::Module::<T>::account_nonce(&account_id).encode());
        let mut output = [0u8; 32];
        keccak.finalize(&mut output);
        // More safe to escape.
        output[0] = 0u8;
        T::AssetId::from(H256(output))
    }

    /// Register the given `AssetId`.
    pub fn register_asset_id(
        account_id: T::AccountId,
        asset_id: T::AssetId,
        symbol: AssetSymbol,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
    ) -> DispatchResult {
        ensure!(
            Self::asset_owner(&asset_id).is_none(),
            Error::<T>::AssetIdAlreadyExists
        );
        AssetOwners::<T>::insert(asset_id, account_id.clone());
        ensure!(
            crate::is_symbol_valid(&symbol),
            Error::<T>::InvalidAssetSymbol
        );
        AssetInfos::<T>::insert(asset_id, (symbol, precision, is_mintable));
        ensure!(
            precision <= MAX_ALLOWED_PRECISION,
            Error::<T>::InvalidPrecision
        );
        let scope = Scope::Limited(hash(&asset_id));
        let permission_ids = [TRANSFER, MINT, BURN, SLASH];
        for permission_id in &permission_ids {
            Permissions::<T>::assign_permission(
                account_id.clone(),
                &account_id,
                *permission_id,
                scope,
            )?;
        }
        if !initial_supply.is_zero() {
            T::Currency::deposit(asset_id.clone(), &account_id, initial_supply)?;
        }
        frame_system::Module::<T>::inc_account_nonce(&account_id);
        Self::deposit_event(RawEvent::AssetRegistered(asset_id, account_id));
        Ok(())
    }

    /// Generates new `AssetId` and registers it from the `account_id`.
    pub fn register_from(
        account_id: &T::AccountId,
        symbol: AssetSymbol,
        precision: BalancePrecision,
        initial_supply: Balance,
        is_mintable: bool,
    ) -> Result<T::AssetId, DispatchError> {
        common::with_transaction(|| {
            let asset_id = Self::gen_asset_id(account_id);
            Self::register_asset_id(
                account_id.clone(),
                asset_id,
                symbol,
                precision,
                initial_supply,
                is_mintable,
            )?;
            Ok(asset_id)
        })
    }

    pub fn asset_owner(asset_id: &T::AssetId) -> Option<T::AccountId> {
        let account_id = Self::asset_owners(&asset_id);
        if account_id == T::AccountId::default() {
            None
        } else {
            Some(account_id)
        }
    }

    #[inline]
    pub fn asset_exists(asset_id: &T::AssetId) -> bool {
        Self::asset_owner(asset_id).is_some()
    }

    pub fn ensure_asset_exists(asset_id: &T::AssetId) -> DispatchResult {
        if !Self::asset_exists(asset_id) {
            return Err(Error::<T>::AssetIdNotExists.into());
        }
        Ok(())
    }

    #[inline]
    pub fn is_asset_owner(asset_id: &T::AssetId, account_id: &T::AccountId) -> bool {
        Self::asset_owner(asset_id)
            .map(|x| &x == account_id)
            .unwrap_or(false)
    }

    fn check_permission_maybe_with_parameters(
        issuer: &T::AccountId,
        permission_id: u32,
        asset_id: &T::AssetId,
    ) -> Result<(), DispatchError> {
        Permissions::<T>::check_permission_with_scope(
            issuer.clone(),
            permission_id,
            &Scope::Limited(hash(asset_id)),
        )
        .or_else(|_| {
            Permissions::<T>::check_permission_with_scope(
                issuer.clone(),
                permission_id,
                &Scope::Unlimited,
            )
        })?;
        Ok(())
    }

    pub fn total_issuance(asset_id: &T::AssetId) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(T::Currency::total_issuance(asset_id.clone()))
    }

    pub fn total_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(T::Currency::total_balance(asset_id.clone(), who))
    }

    pub fn free_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Ok(T::Currency::free_balance(asset_id.clone(), who))
    }

    pub fn ensure_can_withdraw(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        T::Currency::ensure_can_withdraw(asset_id.clone(), who, amount)
    }

    pub fn transfer_from(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(from, TRANSFER, asset_id)?;
        T::Currency::transfer(asset_id.clone(), from, to, amount)
    }

    pub fn force_transfer(
        asset_id: &T::AssetId,
        from: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        T::Currency::transfer(asset_id.clone(), from, to, amount)
    }

    pub fn mint_to(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(issuer, MINT, asset_id)?;
        let (_, _, is_mintable) = AssetInfos::<T>::get(asset_id);
        ensure!(is_mintable, Error::<T>::AssetSupplyIsNotMintable);
        T::Currency::deposit(asset_id.clone(), to, amount)
    }

    pub fn burn_from(
        asset_id: &T::AssetId,
        issuer: &T::AccountId,
        to: &T::AccountId,
        amount: Balance,
    ) -> DispatchResult {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(issuer, BURN, asset_id)?;
        T::Currency::withdraw(asset_id.clone(), to, amount)
    }

    pub fn can_slash(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<bool, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(who, SLASH, asset_id)?;
        Ok(T::Currency::can_slash(asset_id.clone(), who, amount))
    }

    pub fn slash(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(asset_id)?;
        Self::check_permission_maybe_with_parameters(who, SLASH, asset_id)?;
        Ok(T::Currency::slash(asset_id.clone(), who, amount))
    }

    pub fn update_balance(
        asset_id: &T::AssetId,
        who: &T::AccountId,
        by_amount: Amount,
    ) -> DispatchResult {
        Self::check_permission_maybe_with_parameters(who, MINT, asset_id)?;
        Self::check_permission_maybe_with_parameters(who, BURN, asset_id)?;
        if by_amount.is_positive() {
            let (_, _, is_mintable) = AssetInfos::<T>::get(asset_id);
            ensure!(is_mintable, Error::<T>::AssetSupplyIsNotMintable);
        }
        T::Currency::update_balance(asset_id.clone(), who, by_amount)
    }

    pub fn can_reserve(asset_id: T::AssetId, who: &T::AccountId, amount: Balance) -> bool {
        T::Currency::can_reserve(asset_id, who, amount)
    }

    pub fn reserve(
        asset_id: T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<(), DispatchError> {
        Self::ensure_asset_exists(&asset_id)?;
        T::Currency::reserve(asset_id, who, amount)
    }

    pub fn unreserve(
        asset_id: T::AssetId,
        who: &T::AccountId,
        amount: Balance,
    ) -> Result<Balance, DispatchError> {
        Self::ensure_asset_exists(&asset_id)?;
        let amount = T::Currency::unreserve(asset_id, who, amount);
        Ok(amount)
    }

    pub fn set_non_mintable_from(asset_id: &T::AssetId, who: &T::AccountId) -> DispatchResult {
        ensure!(
            Self::is_asset_owner(asset_id, who),
            Error::<T>::InvalidAssetOwner
        );
        AssetInfos::<T>::mutate(asset_id, |(_, _, ref mut is_mintable)| {
            ensure!(*is_mintable, Error::<T>::AssetSupplyIsNotMintable);
            *is_mintable = false;
            Ok(())
        })
    }

    pub fn list_registered_asset_ids() -> Vec<T::AssetId> {
        AssetInfos::<T>::iter().map(|(key, _)| key).collect()
    }

    pub fn list_registered_asset_infos() -> Vec<(T::AssetId, AssetSymbol, BalancePrecision, bool)> {
        AssetInfos::<T>::iter()
            .map(|(key, (symbol, precision, is_mintable))| (key, symbol, precision, is_mintable))
            .collect()
    }

    pub fn get_asset_info(asset_id: &T::AssetId) -> (AssetSymbol, BalancePrecision, bool) {
        AssetInfos::<T>::get(asset_id)
    }
}

/// According to UTF-8 encoding, graphemes that start with byte 0b0XXXXXXX belong
/// to ASCII range and are of single byte, therefore passing check in range 'A' to 'Z'
/// guarantees that all graphemes are of length 1, therefore length check is valid.
pub fn is_symbol_valid(symbol: &AssetSymbol) -> bool {
    symbol.0.len() <= ASSET_SYMBOL_MAX_LENGTH
        && symbol.0.iter().all(|byte| (b'A'..=b'Z').contains(&byte))
}
