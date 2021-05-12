#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_module, decl_storage, decl_event, dispatch, ensure, traits::Get};
use frame_system::{ensure_signed};
use orml_currencies as currencies;
use orml_traits as traits;
use orml_traits::MultiCurrency;
use sp_runtime::{ModuleId, RuntimeDebug, traits::{Saturating, Zero}, FixedU128,
                 SaturatedConversion, FixedPointNumber};
use sp_runtime::traits::{AccountIdConversion};
use frame_support::traits::IsType;

pub use weights::WeightInfo;

const PALLET_ID: ModuleId = ModuleId(*b"crsearly");

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
    percent20: Balance,
    periods: u8,
}

decl_storage! {
	trait Store for Module<T: Config> as CeresEarlySale {

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
	    /// Invalid period argument
	    InvalidPeriod
	}
}

decl_event! {
    pub enum Event<T> where BlockNumber = <T as frame_system::Config>::BlockNumber, Balance = BalanceOf<T>,
            AccountId = <T as frame_system::Config>::AccountId, CurrencyId = CurrencyIdOf<T> {
        /// Pause is set. \[paused\]
        PausedSet(bool),
        /// Price is set. \[price\]
        PriceSet(Balance),
        /// Minimum is set. \[minimum\]
        MinimumSet(Balance),
        /// Maximum is set. \[maximum\]
        MaximumSet(Balance),
        /// Claim block is set. \[period_number, claim_block\]
        ClaimBlockSet(u32, BlockNumber),
        /// Cap is set. \[cap\]
        CapSet(Balance),
        /// Ends is set. \[ends\]
        EndsSet(BlockNumber),
        /// Pallet is unlocked.
        Unlocked(),
        /// Funds withdrawn. \[address, currency_id, amount\]
        FundsWithdrawn(AccountId, CurrencyId, Balance),
        /// All funds withdrawn. \[address, currency_id\]
        AllFundsWithdrawn(AccountId, CurrencyId),
        /// Start sale. \[ends\]
        SaleStarted(BlockNumber),
        /// Claim purchased tokens. \[period, currency_id\]
        ClaimedTokens(u32, CurrencyId),
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

        /// Set claim block for given period
        #[weight = weights::WeightInfo::set_claim_block()]
		pub fn set_claim_block(origin, period_number: u32, claim_block: T::BlockNumber) -> dispatch::DispatchResult {
		    let user = ensure_signed(origin)?;
            ensure!(user == Self::authority(), Error::<T>::Forbidden);
			ensure!(Self::ends() < claim_block, Error::<T>::InvalidClaimBlock);
			<ClaimBlocks<T>>::insert(period_number, claim_block);
			Self::deposit_event(RawEvent::ClaimBlockSet(period_number, claim_block));
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

		/// Start sale
		#[weight = weights::WeightInfo::start_sale()]
		pub fn start_sale(origin, ends: T::BlockNumber) -> dispatch::DispatchResult {
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
		pub fn claim(origin, period: u32, currency_id: CurrencyIdOf<T>) -> dispatch::DispatchResult {
		    ensure!(period <= 4, Error::<T>::InvalidPeriod);
		    let current_block = frame_system::Module::<T>::block_number();
			ensure!(current_block > Self::ends(), Error::<T>::SaleNotEnded);
			ensure!(current_block > Self::claim_blocks(period), Error::<T>::SaleNotEnded);

			let user = ensure_signed(origin)?;
			let mut details = <Investors<T>>::get(&user);
            ensure!(Self::check_bit(details.periods, period, 0), Error::<T>::PeriodAlreadyClaimed);
            let amount = details.percent20;

            // update user and stats
            details.ceres_balance = details.ceres_balance.saturating_sub(amount);
            let temp = 1 << period;
            details.periods |= temp;
            <Investors<T>>::insert(&user, details);
            <TotalOwed<T>>::put(Self::total_owed().saturating_sub(amount));

			<currencies::Module<T> as traits::MultiCurrency<T::AccountId>>::transfer(currency_id, &Self::account_id(), &user, amount)?;
			Self::deposit_event(RawEvent::ClaimedTokens(period, currency_id));
			Ok(())
		}

		/// Buy tokens
		#[weight = weights::WeightInfo::buy()]
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
            details.percent20 = ((FixedU128::from_inner(20) / FixedU128::from_inner(100)) *
                FixedU128::from_inner(potential_ceres_amount.saturated_into::<u128>())).into_inner()
                .saturated_into();
            <Investors<T>>::insert(&user, details);

            <TotalOwed<T>>::put(Self::total_owed().saturating_add(amount));
            <XorRaised<T>>::put(Self::xor_raised().saturating_add(value));

            Self::deposit_event(RawEvent::BoughtTokens(value, xor_currency_id));
			Ok(())
		}
	}
}

impl<T: Config> Module<T> {
    /// The account ID of early sale wallet
    fn account_id() -> T::AccountId {
        PALLET_ID.into_account()
    }

    /// Calculate purchased amount of Ceres
    fn calculate_purchased_amount(value: BalanceOf<T>) -> BalanceOf<T> {
        return (FixedU128::from_inner(value.saturated_into::<u128>()) / FixedU128::from_inner(
            Self::price().saturated_into::<u128>())).into_inner().saturated_into();
    }

    /// Check bit value on 'period' position of 'periods'
    fn check_bit(periods: u8, period: u32, bit_value: u8) -> bool {
        periods & (1 << period) == bit_value
    }

    /// Calculate amount for claiming
    pub fn calculate_amount_for_claiming(address: &T::AccountId, period: u32) -> BalanceOf<T> {
        return if period > 4 || Self::check_bit(Self::investors(address).periods, period, 1) {
            <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::zero()
        } else {
            Self::investors(address).percent20
        };
    }

    /// Get Ceres balance
    pub fn get_ceres_balance(address: &T::AccountId) -> BalanceOf<T> {
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