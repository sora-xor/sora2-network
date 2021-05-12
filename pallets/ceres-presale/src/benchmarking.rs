use frame_benchmarking::{account, benchmarks};
use frame_system::{EventRecord, RawOrigin};
use orml_currencies as currencies;
use orml_currencies::Pallet;
use sp_runtime::Storage;

use crate::{*, Module as PalletModule};

fn get_authority() -> T::AccountId {
    return T::AccountId::decode(&mut "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".as_bytes())
        .unwrap_or_default();
}

fn start_sale_fun<T: Config>(start: bool)
{
    let caller: T::AccountId = get_authority();
    let max_value: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::max_value();
    let min_value: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::zero();
    PalletModule::<T>::set_price(caller.clone(), max_value);
    PalletModule::<T>::set_minimum(caller.clone(), min_value + (1u128).saturated_into());
    PalletModule::<T>::set_maximum(caller.clone(), max_value);
    PalletModule::<T>::set_cap(caller.clone(), max_value);
    if start {
        PalletModule::<T>::start_presale(caller.clone(), T::BlockNumber::from(1u8));
    }
}

fn claim_fun<T: Config>(demeter: bool) {
    let caller: T::AccountId = get_authority();
    PalletModule::<T>::set_ends(caller.clone(), T::BlockNumber::from(1u8));
    let mut details = <Investors<T>>::get(&caller);
    let value: BalanceOf<T> = (1u128).saturated_into();
    if demeter {
        details.percent30 = value;
        PalletModule::<T>::set_demeter_multiplier(caller.clone(), (1u128).saturated_into());
    } else {
        details.percent70 = value;
    }
    <Investors<T>>::insert(&caller, details);
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::Event) {
    let events = frame_system::Module::<T>::events();
    let system_event: <T as frame_system::Config>::Event = generic_event.into();
    // compare to the last event record
    let EventRecord { event, .. } = events.last().unwrap();
    assert_eq!(event, &system_event);
}

benchmarks! {
	pause {
		let caller: T::AccountId = get_authority();
	}: _(SystemOrigin::Signed(caller.clone()), true)
	verify {
		assert_last_event::<T>(RawEvent::PausedSet(true).into());
	}

	set_price {
		let caller: T::AccountId = get_authority();
		let price: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::max_value();
	}: _(SystemOrigin::Signed(caller.clone()), price)
    verify {
		assert_last_event::<T>(RawEvent::PriceSet(price).into());
	}

	set_demeter_multiplier {
		let caller: T::AccountId = get_authority();
		let multiplier: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::max_value;
	}: _(SystemOrigin::Signed(caller.clone()), multiplier)
    verify {
		assert_last_event::<T>(RawEvent::DemeterMultiplierSet(multiplier).into());
	}

    set_minimum {
		let caller: T::AccountId = get_authority();
		let minimum: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::max_value();
	}: _(SystemOrigin::Signed(caller.clone()), minimum)
	verify {
		assert_last_event::<T>(RawEvent::MinimumSet(minimum).into());
	}

    set_maximum {
		let caller: T::AccountId = get_authority();
		let maximum: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::max_value();
	}: _(SystemOrigin::Signed(caller.clone()), maximum)
    verify {
		assert_last_event::<T>(RawEvent::MaximumSet(maximum).into());
	}

    set_cap {
		let caller: T::AccountId = get_authority();
		let cap: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::max_value();
	}: _(SystemOrigin::Signed(caller.clone()), cap)
    verify {
		assert_last_event::<T>(RawEvent::CapSet(cap).into());
	}

	set_demeter_block {
		let caller: T::AccountId = get_authority();
		let demeter_block: T::BlockNumber = T::BlockNumber::from(1u32);
	}: _(SystemOrigin::Signed(caller.clone()), demeter_block)
    verify {
		assert_last_event::<T>(RawEvent::DemeterBlockSet(demeter_block).into());
	}

	set_ends {
		let caller: T::AccountId = get_authority();
		let ends: T::BlockNumber = T::BlockNumber::from(1u32);
	}: _(SystemOrigin::Signed(caller.clone()), ends)
	verify {
		assert_last_event::<T>(RawEvent::EndsSet(ends).into());
	}

	unlock {
		let caller: T::AccountId = get_authority();
	}: _(SystemOrigin::Signed(caller.clone()))
	verify {
		assert_last_event::<T>(RawEvent::Unlocked().into());
	}

	withdraw_xor {
		let caller: T::AccountId = get_authority();
		let amount: BalanceOf<T> = <currencies::Module<T> as traits::MultiCurrency<_>>::Balance::zero();
	}: _(SystemOrigin::Signed(caller.clone()), caller.clone(), Default::default(), amount)
	verify {
		assert_last_event::<T>(RawEvent::FundsWithdrawn(caller.clone(), Default::default(), amount).into());
	}

	withdraw_xor_all {
	    let caller: T::AccountId = get_authority();
	}: _(SystemOrigin::Signed(caller.clone()), caller.clone(), Default::default())
	verify {
		assert_last_event::<T>(RawEvent::AllFundsWithdrawn(caller.clone(), Default::default()).into());
	}

	start_presale {
	    let caller: T::AccountId = get_authority();
	    let ends: T::BlockNumber = T::BlockNumber::from(1u8);
	    start_sale_fun::<T>(false);
	}: _(SystemOrigin::Signed(caller.clone()), ends)
	verify {
		assert_last_event::<T>(RawEvent::SaleStarted(ends).into());
	}

    claim {
	    let caller: T::AccountId = get_authority();
	    claim_fun::<T>(false);
	}: _(SystemOrigin::Signed(caller.clone()), Default::default())
	verify {
		assert_last_event::<T>(RawEvent::ClaimedTokens(Default::default(), (1u128).saturated_into()).into());
	}

	claim_demeter {
	    let caller: T::AccountId = get_authority();
	    claim_fun::<T>(true);
	}: _(SystemOrigin::Signed(caller.clone()), Default::default())
	verify {
		assert_last_event::<T>(RawEvent::ClaimedDemeterTokens((1u128).saturated_into()).into());
	}

	buy {
	    let caller: T::AccountId = get_authority();
	    let value: BalanceOf<T> = (0u128).saturated_into();
	    start_sale_fun::<T>(true);
	}: _(SystemOrigin::Signed(caller.clone()), value, Default::default(), Default::default())
	verify {
		assert_last_event::<T>(RawEvent::BoughtTokens(value, Default::default()).into());
	}
}