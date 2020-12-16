#![cfg_attr(not(feature = "std"), no_std)]

use common::{fixed, prelude::*};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    weights::Pays,
};
use frame_system::ensure_signed;
use sp_arithmetic::traits::Saturating;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

type Assets<T> = assets::Module<T>;
type System<T> = frame_system::Module<T>;
type Technical<T> = technical::Module<T>;
type BlockNumberOf<T> = <T as frame_system::Trait>::BlockNumber;

pub const TECH_ACCOUNT_PREFIX: &[u8] = b"faucet";
pub const TECH_ACCOUNT_MAIN: &[u8] = b"main";

pub fn balance_limit() -> Balance {
    Balance(fixed!(100))
}

pub fn transfer_limit_block_count<T: frame_system::Trait>() -> BlockNumberOf<T> {
    14400.into()
}

pub trait Trait: technical::Trait + assets::Trait + frame_system::Trait {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as FaucetStorage
    {
        ReservesAcc get(fn reserves_account_id) config(reserves_account_id): T::TechAccountId;
        Transfers: double_map hasher(identity) T::AccountId, hasher(opaque_blake2_256) T::AssetId => Option<(BlockNumberOf<T>, Balance)>;
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
    {
        // The amount is transferred to the account. [account, amount]
        Transferred(AccountId, Balance),
    }
);

decl_error! {
    pub enum Error for Module<T: Trait> {
        AssetNotSupported,
        AmountAboveLimit,
        NotEnoughReserves,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

        /// Transfers the specified amount of asset to the specified account.
        /// The supported assets are: XOR, VAL, PSWAP.
        ///
        /// # Errors
        ///
        /// AssetNotSupported is returned if `asset_id` is something the function doesn't support.
        /// AmountAboveLimit is returned if `target` has already received their daily limit of `asset_id`.
        /// NotEnoughReserves is returned if `amount` is greater than the reserves
        #[weight = (0, Pays::No)]
        pub fn transfer(origin, asset_id: T::AssetId, target: T::AccountId, amount: Balance) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            Self::ensure_asset_supported(asset_id)?;
            let block_number = System::<T>::block_number();
            let (block_number, taken_amount) = Self::prepare_transfer(&target, asset_id, amount, block_number)?;
            let reserves_tech_account_id = Self::reserves_account_id();
            let reserves_account_id =
            Technical::<T>::tech_account_id_to_account_id(&reserves_tech_account_id)?;
            let reserves_amount = Assets::<T>::total_balance(&asset_id, &reserves_account_id)?;
            ensure!(amount <= reserves_amount, Error::<T>::NotEnoughReserves);
            technical::Module::<T>::transfer_out(
                &asset_id,
                &reserves_tech_account_id,
                &target,
                amount,
            )?;
            Transfers::<T>::insert(target.clone(), asset_id, (block_number, taken_amount));
            Self::deposit_event(RawEvent::Transferred(target, amount));
            Ok(())
        }
    }
}

impl<T: Trait> Module<T> {
    fn ensure_asset_supported(asset_id: T::AssetId) -> Result<(), Error<T>> {
        let xor = XOR.into();
        let val = VAL.into();
        let pswap = PSWAP.into();
        if asset_id == xor || asset_id == val || asset_id == pswap {
            Ok(())
        } else {
            Err(Error::AssetNotSupported)
        }
    }

    /// Checks if new transfer is allowed, considering previous transfers.
    ///
    /// If new transfer is allowed, returns content to put in `Transfers` if the transfer is succeeded
    fn prepare_transfer(
        target: &T::AccountId,
        asset_id: T::AssetId,
        amount: Balance,
        current_block_number: BlockNumberOf<T>,
    ) -> Result<(BlockNumberOf<T>, Balance), Error<T>> {
        let balance_limit = balance_limit();
        ensure!(amount <= balance_limit, Error::AmountAboveLimit);
        if let Some((initial_block_number, taken_amount)) = Transfers::<T>::get(target, asset_id) {
            let transfer_limit_block_count = transfer_limit_block_count::<T>();
            if transfer_limit_block_count
                <= current_block_number.saturating_sub(initial_block_number)
            {
                // The previous transfer has happened a long time ago
                Ok((current_block_number, amount))
            } else if amount <= balance_limit.saturating_sub(taken_amount) {
                // Use `initial_block_number` because the previous transfer has happened recently.
                Ok((initial_block_number, taken_amount + amount))
            } else {
                Err(Error::AmountAboveLimit)
            }
        } else {
            Ok((current_block_number, amount))
        }
    }
}
