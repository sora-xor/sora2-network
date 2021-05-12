#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, dispatch, ensure, traits::Get};
use frame_support::traits::IsType;
use frame_system::ensure_signed;
use orml_currencies as currencies;
use orml_traits as traits;
use orml_traits::MultiCurrency;
use sp_runtime::{FixedPointNumber, FixedU128, ModuleId, RuntimeDebug,
                 SaturatedConversion, traits::{Saturating, Zero}};
use sp_runtime::traits::AccountIdConversion;

pub use weights::WeightInfo;

const PALLET_ID: ModuleId = ModuleId(*b"cpresale");

pub trait Config: frame_system::Config + currencies::Config {
    type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
}

type BalanceOf<T> =
<<T as currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;
type CurrencyIdOf<T> =
<<T as currencies::Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId;
type InvestorDetailsOf<T> = InvestorDetails<BalanceOf<T>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default)]
pub struct InvestorDetails<Balance> {
    ceres_balance: Balance,
    xor_funded: Balance,
    percent70: Balance,
    percent30: Balance,
}

decl_storage! {
	trait Store for Module<T: Config> as CeresPresale {

	    // authority
	    Authority get(fn authority) config() : T::AccountId;

        // sale params
        Started get(fn started): bool;
        Paused get(fn paused): bool;
        Price get(fn price): BalanceOf<T>;
        Cap get(fn cap): BalanceOf<T>;
        Minimum get(fn minimum): BalanceOf<T>;
        Maximum get(fn maximum): BalanceOf<T>;
        Ends get(fn ends): T::BlockNumber;

        // demeter params
        DemeterMultiplier get(fn demeter_multiplier): BalanceOf<T>;
        DemeterBlock get(fn demeter_block): T::BlockNumber;

        // stats
        TotalOwed get(fn total_owed): BalanceOf<T>;
        XorRaised get(fn xor_raised): BalanceOf<T>;

        Investors get(fn investors): map hasher(blake2_128_concat) T::AccountId => InvestorDetailsOf<T>;
        ClaimBlocks get(fn claim_blocks): map hasher(blake2_128_concat) u32 => T::BlockNumber;
    }
}

decl_error! {
	pub enum Error for Module<T: Config> {
	    /// No permission
	    NoPermission,
		/// Claim block should be after the sale ending
		InvalidClaimBlock,
		/// Entity does not exist
		Unknown,
		/// End date is capped
		EndDateCapped,
		/// Sale already started
		SaleAlreadyStarted,
		/// Price is zero
		PriceNotSet,
		/// Minimum is not greater than zero
		InvalidMinimum,
		/// Maximum is not greater than minimum
		InvalidMaximum,
		/// Maximum end date is not greater than end date
		InvalidEndDate,
		/// Cap is zero
		CapNotSet,
		/// Calculating overflow
		Overflow,
		/// Sale has not ended yet
	    SaleNotEnded,
	    /// Sale is paused
	    SaleIsPaused,
	    /// Small amount
	    SmallAmount,
	    /// Cap filled
	    CapFilled,
	    /// Maximum purchase cap hit
	    MaximumOverflow,
	    /// Sold out
	    SoldOut,
	    /// Period already claimed
	    PeriodAlreadyClaimed,
	    /// Forbidden access
	    Forbidden,
		/// Fifteen percent cannot be claimed anymore
		CannotClaimFifteenPercent,
		/// Tokens are already claimed
		TokensAlreadyClaimed,
		/// Demeter airdop is not live yet
		DemeterAirdropIsNotLive,
	}
}

decl_event! {
    pub enum Event<T> where BlockNumber = <T as frame_system::Config>::BlockNumber, Balance = BalanceOf<T>,
            AccountId = <T as frame_system::Config>::AccountId, CurrencyId = CurrencyIdOf<T> {
        /// Pause is set. \[paused\]
        PausedSet(bool),
        /// Price is set. \[price\]
        PriceSet(Balance),
        /// Demeter multiplier is set. \[multiplier\]
        DemeterMultiplierSet(Balance),
        /// Minimum is set. \[minimum\]
        MinimumSet(Balance),
        /// Maximum is set. \[maximum\]
        MaximumSet(Balance),
        /// Cap is set. \[cap\]
        CapSet(Balance),
        /// Ends is set. \[ends\]
        EndsSet(BlockNumber),
        /// Demeter block is set. \[demeter_block\]
        DemeterBlockSet(BlockNumber),
        /// Pallet is unlocked.
        Unlocked(),
        /// Funds withdrawn. \[address, currency_id, amount\]
        FundsWithdrawn(AccountId, CurrencyId, Balance),
        /// All funds withdrawn. \[address, currency_id\]
        AllFundsWithdrawn(AccountId, CurrencyId),
        /// Start sale. \[ends\]
        SaleStarted(BlockNumber),
        /// Claim purchased tokens. \[currency_id, amount\]
        ClaimedTokens(CurrencyId, Balance),
        /// Claim Demeter tokens. \[amount\]
        ClaimedDemeterTokens(Balance),
        /// Bought tokens. \[value, currency_id\]
        BoughtTokens(Balance, CurrencyId),
    }
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
        type Error = Error<T>;
        fn deposit_event() = default;

		/// Pause sale
		#[weight = weights::WeightInfo::pause()]
        pub fn pause(origin, paused: bool) -> dispatch::DispatchResult {
            let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			Paused::put(paused);
			Self::deposit_event(RawEvent::PausedSet(paused));
			Ok(())
		}

		/// Set sale price
		#[weight = weights::WeightInfo::set_price()]
		pub fn set_price(origin, price: BalanceOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<Price<T>>::put(price);
			Self::deposit_event(RawEvent::PriceSet(price));
			Ok(())
		}

		/// Set demeter multiplier
		#[weight = weights::WeightInfo::set_demeter_multiplier()]
		pub fn set_demeter_multiplier(origin, multiplier: BalanceOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<DemeterMultiplier<T>>::put(multiplier);
			Self::deposit_event(RawEvent::DemeterMultiplierSet(multiplier));
			Ok(())
		}

		/// Set minimum
		#[weight = weights::WeightInfo::set_minimum()]
		pub fn set_minimum(origin, minimum: BalanceOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<Minimum<T>>::put(minimum);
			Self::deposit_event(RawEvent::MinimumSet(minimum));
			Ok(())
		}

		/// Set maximum
		#[weight = weights::WeightInfo::set_maximum()]
		pub fn set_maximum(origin, maximum: BalanceOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<Maximum<T>>::put(maximum);
			Self::deposit_event(RawEvent::MaximumSet(maximum));
			Ok(())
		}

		/// Set cap
		#[weight = weights::WeightInfo::set_cap()]
		pub fn set_cap(origin, cap: BalanceOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<Cap<T>>::put(cap);
			Self::deposit_event(RawEvent::CapSet(cap));
			Ok(())
		}

        /// Set demeter block
		#[weight = weights::WeightInfo::set_demeter_block()]
		pub fn set_demeter_block(origin, demeter_block: T::BlockNumber) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<DemeterBlock<T>>::put(demeter_block);
			Self::deposit_event(RawEvent::DemeterBlockSet(demeter_block));
			Ok(())
		}

		/// Set ends
		#[weight = weights::WeightInfo::set_ends()]
		pub fn set_ends(origin, ends: T::BlockNumber) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<Ends<T>>::put(ends);
			Self::deposit_event(RawEvent::EndsSet(ends));
			Ok(())
		}

		/// Unlock pallet
		#[weight = weights::WeightInfo::unlock()]
		pub fn unlock(origin) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<Ends<T>>::put(T::BlockNumber::zero());
			Paused::put(true);
			Self::deposit_event(RawEvent::Unlocked());
			Ok(())
		}

		/// Withdraw funds
		#[weight = weights::WeightInfo::withdraw_xor()]
		pub fn withdraw_xor(origin, address: T::AccountId, currency_id: CurrencyIdOf<T>, amount: BalanceOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			<currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(currency_id, &Self::account_id(), &address, amount)?;
			Self::deposit_event(RawEvent::FundsWithdrawn(address, currency_id, amount));
			Ok(())
		}

		/// Withdraw all funds
		#[weight = weights::WeightInfo::withdraw_xor_all()]
		pub fn withdraw_xor_all(origin, address: T::AccountId, currency_id: CurrencyIdOf<T>) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
		    let balance = <currencies::Module<T> as traits::MultiCurrency<_>>::free_balance(currency_id, &Self::account_id());
			<currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(currency_id, &Self::account_id(), &address, balance)?;
			Self::deposit_event(RawEvent::AllFundsWithdrawn(address, currency_id));
			Ok(())
		}

		/// Start presale
		#[weight = weights::WeightInfo::start_presale()]
		pub fn start_presale(origin, ends: T::BlockNumber) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
            let zero = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::zero();
			ensure!(!Self::started(), Error::<T>::SaleAlreadyStarted);
			ensure!(Self::price() > zero, Error::<T>::PriceNotSet);
			ensure!(Self::minimum() > zero, Error::<T>::InvalidMinimum);
			ensure!(Self::maximum() > Self::minimum(), Error::<T>::InvalidMaximum);
			ensure!(Self::cap() > zero, Error::<T>::CapNotSet);

			Started::put(true);
			Paused::put(false);
			<Ends<T>>::put(ends);
			Self::deposit_event(RawEvent::SaleStarted(ends));
			Ok(())
		}

		/// Claim purchased tokens
		#[weight = weights::WeightInfo::claim()]
		pub fn claim(origin, currency_id: CurrencyIdOf<T>) -> dispatch::DispatchResult {
			let current_block = frame_system::Module::<T>::block_number();
			ensure!(current_block > Self::ends(), Error::<T>::SaleNotEnded);
			let user = ensure_signed(origin)?;
			let zero = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::zero();
			let mut details = <Investors<T>>::get(&user);
			ensure!(details.percent70 > zero, Error::<T>::TokensAlreadyClaimed);

			let amount = details.percent70;
			details.ceres_balance = details.ceres_balance - amount;
			details.percent70 = zero;
			<Investors<T>>::insert(&user, details);

			<TotalOwed<T>>::put(Self::total_owed().saturating_sub(amount));
			<currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(currency_id, &Self::account_id(), &user, amount)?;
			Self::deposit_event(RawEvent::ClaimedTokens(currency_id, amount));
			Ok(())
		}

		/// Claim Demeter
		#[weight = weights::WeightInfo::claim_demeter()]
		pub fn claim_demeter(origin, ceres_currency_id: CurrencyIdOf<T>, demeter_currency_id: CurrencyIdOf<T>) -> dispatch::DispatchResult{
			let current_block = frame_system::Module::<T>::block_number();
			ensure!(current_block > Self::demeter_block(), Error::<T>::DemeterAirdropIsNotLive);
			let user = ensure_signed(origin)?;
			let zero = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::zero();
			let mut details = <Investors<T>>::get(&user);
		    let amount = details.percent30;
			ensure!(amount > zero, Error::<T>::TokensAlreadyClaimed);

			let demeter_to_claim = (FixedU128::from_inner(amount
                .saturated_into::<u128>()) * FixedU128::from_inner(Self::demeter_multiplier()
                .saturated_into::<u128>())).into_inner().saturated_into();
            details.ceres_balance = zero;
			details.percent30 = zero;
			<Investors<T>>::insert(&user, details);

            <currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(ceres_currency_id, &Self::account_id(), &user, amount)?;
			<currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(demeter_currency_id, &Self::account_id(), &user, demeter_to_claim)?;
			Self::deposit_event(RawEvent::ClaimedDemeterTokens(demeter_to_claim));
			Ok(())
		}

		/// Buy tokens
		#[weight = weights::WeightInfo::claim_demeter()]
		pub fn buy(origin, value: BalanceOf<T>, ceres_currency_id: CurrencyIdOf<T>, xor_currency_id: CurrencyIdOf<T>) -> dispatch::DispatchResult {
		    ensure!(!Self::paused(), Error::<T>::SaleIsPaused);
		    ensure!(value >= Self::minimum(), Error::<T>::SmallAmount);
		    ensure!(Self::xor_raised().saturating_add(value) <= Self::cap(), Error::<T>::CapFilled);

			let user = ensure_signed(origin)?;
            let amount = Self::calculate_purchased_amount(value);
            let account_id = &Self::account_id();

            let balance = <currencies::Module<T> as traits::MultiCurrency<_>>::free_balance(ceres_currency_id, account_id);
			ensure!(Self::total_owed().saturating_add(amount) <= balance, Error::<T>::SoldOut);

            let mut details = <Investors<T>>::get(&user);
			let potential_ceres_amount = details.ceres_balance.saturating_add(amount);
			let potential_xor_funded = details.xor_funded.saturating_add(value);
			ensure!(potential_xor_funded <= Self::maximum(), Error::<T>::MaximumOverflow);
			<currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(xor_currency_id, &user, account_id, value)?;

            details.ceres_balance = potential_ceres_amount;
            details.xor_funded = potential_xor_funded;
            details.percent70 = ((FixedU128::from_inner(70) / FixedU128::from_inner(100)) *
                FixedU128::from_inner(potential_ceres_amount.saturated_into::<u128>())).into_inner()
                .saturated_into();
            details.percent30 = potential_ceres_amount.saturating_sub(details.percent70);
            <Investors<T>>::insert(&user, details);

			<TotalOwed<T>>::put(Self::total_owed().saturating_add(amount));
            <XorRaised<T>>::put(Self::xor_raised().saturating_add(value));

            Self::deposit_event(RawEvent::BoughtTokens(value, xor_currency_id));
			Ok(())
		}
	}
}

impl<T: Config> Module<T> {
    /// The account ID of presale wallet
    fn account_id() -> T::AccountId {
        PALLET_ID.into_account()
    }

    /// Calculate purchased amount of Ceres
    fn calculate_purchased_amount(value: BalanceOf<T>) -> BalanceOf<T> {
        return (FixedU128::from_inner(value.saturated_into::<u128>()) / FixedU128::from_inner(
            Self::price().saturated_into::<u128>())).into_inner().saturated_into();
    }

    /// Get Seventy Ceres balance
    pub fn get_seventy_ceres_balance(address: T::AccountId) -> BalanceOf<T> {
        return Self::investors(address).percent70;
    }

    /// Get Thirty Ceres balance
    pub fn get_thirty_ceres_balance(address: T::AccountId) -> BalanceOf<T> {
        return Self::investors(address).percent30;
    }

    /// Get Total Ceres balance
    pub fn get_ceres_balance(address: T::AccountId) -> BalanceOf<T> {
        return Self::investors(address).ceres_balance;
    }

    /// Get Xor raised
    pub fn get_xor_raised() -> BalanceOf<T> {
        return Self::xor_raised();
    }

    /// Get Ceres owed
    pub fn get_total_owed() -> BalanceOf<T> {
        return Self::total_owed();
    }
}