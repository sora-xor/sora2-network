// This file is part of Substrate.
// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! SORA Nexus compatibility for commitments made by synchronous staking v16.
//!
//! Legacy queues and slashing maxima retain their original storage prefixes and encoding.
//! New async queues use a distinct prefix. This module never guesses missing fraction/page
//! information, merges slash records, drops reporters, or executes historically stranded queues.

use crate::{
	asset, BalanceOf, Config, Exposure, NegativeImbalanceOf, Pallet, Perbill, ValidatorSlashInEra,
	WeightInfo,
};
use alloc::vec::Vec;
use codec::{Decode, Encode, HasCompact, MaxEncodedLen};
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Defensive, DefensiveSaturating, Get, Imbalance, OnUnbalanced},
	weights::Weight,
	StorageDoubleMap as _,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{Saturating, Zero},
	Debug, DispatchResult,
};
use sp_staking::{EraIndex, SessionIndex};

/// Exact v16 record encoding, including every reporter and its position.
#[derive(Encode, Decode, Debug, TypeInfo, PartialEq, Eq, Clone, DecodeWithMemTracking)]
pub struct LegacyUnappliedSlash<AccountId, Balance: HasCompact> {
	pub validator: AccountId,
	pub own: Balance,
	pub others: Vec<(AccountId, Balance)>,
	pub reporters: Vec<AccountId>,
	pub payout: Balance,
}

#[frame_support::storage_alias]
pub type UnappliedSlashes<T: Config> = StorageMap<
	Pallet<T>,
	Twox64Concat,
	EraIndex,
	Vec<LegacyUnappliedSlash<<T as frame_system::Config>::AccountId, BalanceOf<T>>>,
	ValueQuery,
>;
#[frame_support::storage_alias]
pub type CutoffEra<T: Config> = StorageValue<Pallet<T>, EraIndex>;
/// First execution era admitted at activation, independent of an already planned era.
#[frame_support::storage_alias]
pub type FirstExecutionEra<T: Config> = StorageValue<Pallet<T>, EraIndex>;
#[frame_support::storage_alias]
pub type LegacySlashCursor<T: Config> =
	StorageMap<Pallet<T>, Twox64Concat, EraIndex, u32, ValueQuery>;
#[frame_support::storage_alias]
pub type ErasStakers<T: Config> = StorageDoubleMap<
	Pallet<T>,
	Twox64Concat,
	EraIndex,
	Twox64Concat,
	<T as frame_system::Config>::AccountId,
	Exposure<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
	ValueQuery,
>;
#[frame_support::storage_alias]
pub type ErasStakersClipped<T: Config> = StorageDoubleMap<
	Pallet<T>,
	Twox64Concat,
	EraIndex,
	Twox64Concat,
	<T as frame_system::Config>::AccountId,
	Exposure<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
	ValueQuery,
>;
#[frame_support::storage_alias]
pub type ErasStartSessionIndex<T: Config> =
	StorageMap<Pallet<T>, Twox64Concat, EraIndex, SessionIndex>;
#[frame_support::storage_alias]
pub type CurrentPlannedSession<T: Config> = StorageValue<Pallet<T>, SessionIndex, ValueQuery>;
#[frame_support::storage_alias]
pub type Invulnerables<T: Config> =
	StorageValue<Pallet<T>, Vec<<T as frame_system::Config>::AccountId>, ValueQuery>;
#[frame_support::storage_alias]
pub type MinimumValidatorCount<T: Config> = StorageValue<Pallet<T>, u32, ValueQuery>;
#[frame_support::storage_alias]
pub type NominatorSlashInEra<T: Config> = StorageDoubleMap<
	Pallet<T>,
	Twox64Concat,
	EraIndex,
	Twox64Concat,
	<T as frame_system::Config>::AccountId,
	BalanceOf<T>,
>;
#[frame_support::storage_alias]
pub type SlashingSpans<T: Config> =
	StorageMap<Pallet<T>, Twox64Concat, <T as frame_system::Config>::AccountId, SlashingSpansData>;
#[frame_support::storage_alias]
pub type SpanSlash<T: Config> = StorageMap<
	Pallet<T>,
	Twox64Concat,
	(<T as frame_system::Config>::AccountId, SpanIndex),
	SpanRecord<BalanceOf<T>>,
	ValueQuery,
>;

/// Frozen v16 queues are inspected once before activation. New historical reports use
/// individually keyed records and never enlarge these bounded legacy values.
pub const MAX_LEGACY_ERA_BYTES: u32 = 512 * 1024;

#[derive(Encode, Decode, TypeInfo)]
pub struct HistoricalSlash<AccountId, Balance: HasCompact> {
	pub offence_era: EraIndex,
	pub fraction: Perbill,
	pub slash: LegacyUnappliedSlash<AccountId, Balance>,
}
#[frame_support::storage_alias]
pub type HistoricalSlashes<T: Config> = StorageDoubleMap<
	Pallet<T>,
	Twox64Concat,
	EraIndex,
	Twox64Concat,
	u64,
	HistoricalSlash<<T as frame_system::Config>::AccountId, BalanceOf<T>>,
>;
#[frame_support::storage_alias]
pub type HistoricalSlashHead<T: Config> =
	StorageMap<Pallet<T>, Twox64Concat, EraIndex, u64, ValueQuery>;
#[frame_support::storage_alias]
pub type HistoricalSlashTail<T: Config> =
	StorageMap<Pallet<T>, Twox64Concat, EraIndex, u64, ValueQuery>;

pub fn validate_pending_era<T: Config>(era: EraIndex) -> Result<(), &'static str> {
	let key = UnappliedSlashes::<T>::hashed_key_for(era);
	let bytes = sp_io::storage::read(&key, &mut [], 0).unwrap_or(0);
	ensure!(bytes <= MAX_LEGACY_ERA_BYTES, "legacy slash era exceeds byte bound");
	if let Some(bytes) = sp_io::storage::get(&key) {
		let records = <Vec<LegacyUnappliedSlash<T::AccountId, BalanceOf<T>>> as codec::DecodeAll>
            ::decode_all(&mut &bytes[..]).map_err(|_| "legacy slash queue cannot decode")?;
		for record in records {
			ensure!(
				record.others.len() <= T::MaxExposurePageSize::get() as usize,
				"legacy slash record exceeds a bounded page; staged migration required"
			);
		}
	}
	Ok(())
}

/// Older stranded queues deliberately retain their v16 semantics and do not become executable.
pub fn has_pending_slashes<T: Config>(era: EraIndex) -> bool {
	FirstExecutionEra::<T>::get()
		.or_else(CutoffEra::<T>::get)
		.is_some_and(|first| era >= first) &&
		(UnappliedSlashes::<T>::decode_len(era)
			.is_some_and(|len| len > LegacySlashCursor::<T>::get(era) as usize) ||
			HistoricalSlashHead::<T>::get(era) < HistoricalSlashTail::<T>::get(era))
}

/// Every eligible queue remains guarded even if processing spans multiple era transitions.
/// The range is fixed by the migration cutoff, never by an unbounded chain age.
pub fn has_pending_due_slashes<T: Config>(active: EraIndex) -> bool {
	let Some(cutoff) = CutoffEra::<T>::get() else {
		return false;
	};
	let last = cutoff
		.saturating_add(T::SlashDeferDuration::get())
		.saturating_add(1)
		.min(active);
	let first = FirstExecutionEra::<T>::get().unwrap_or(cutoff);
	(first..=last).any(has_pending_slashes::<T>)
}

fn clear_cancelled_if_drained<T: Config>(era: EraIndex) {
	if !crate::AsyncUnappliedSlashes::<T>::contains_prefix(era) && !has_pending_slashes::<T>(era) {
		crate::CancelledSlashes::<T>::remove(era);
	}
}

/// One original slash per block. Each full exposure is admitted at upgrade under the
/// page bound; all reporters are retained and their actual read/write cost is charged.
pub fn apply_pending_slashes<T: Config>(active: EraIndex) -> Weight {
	let Some(cutoff) = CutoffEra::<T>::get() else {
		return T::DbWeight::get().reads(1);
	};
	let last = cutoff
		.saturating_add(T::SlashDeferDuration::get())
		.saturating_add(1)
		.min(active);
	let first = FirstExecutionEra::<T>::get().unwrap_or(cutoff);
	let mut weight = T::DbWeight::get().reads(2);
	for execution in first..=last {
		weight.saturating_accrue(T::DbWeight::get().reads(4));
		let cursor = LegacySlashCursor::<T>::get(execution) as usize;
		if UnappliedSlashes::<T>::decode_len(execution).is_some_and(|len| cursor < len) {
			let queue = UnappliedSlashes::<T>::get(execution);
			let Some(slash) = queue.get(cursor).cloned() else {
				return weight;
			};
			let nominators = slash.others.len() as u32;
			let reporters = slash.reporters.len() as u64;
			let original = execution.saturating_sub(T::SlashDeferDuration::get()).saturating_sub(1);
			apply_slash::<T>(slash, original);
			if cursor + 1 == queue.len() {
				UnappliedSlashes::<T>::remove(execution);
				LegacySlashCursor::<T>::remove(execution);
			} else {
				LegacySlashCursor::<T>::insert(execution, (cursor + 1) as u32);
			}
			clear_cancelled_if_drained::<T>(execution);
			return weight
				.saturating_add(T::WeightInfo::apply_slash(nominators))
				.saturating_add(
					T::DbWeight::get().reads_writes(7 + 4 * reporters, 3 + 4 * reporters),
				)
				.saturating_add(Weight::from_parts(
					MAX_LEGACY_ERA_BYTES as u64 * 1_000_000,
					MAX_LEGACY_ERA_BYTES as u64,
				));
		}
		let head = HistoricalSlashHead::<T>::get(execution);
		if head < HistoricalSlashTail::<T>::get(execution) {
			if let Some(record) = HistoricalSlashes::<T>::take(execution, head) {
				let nominators = record.slash.others.len() as u32;
				let reporters = record.slash.reporters.len() as u64;
				let bytes = record.encoded_size() as u64;
				// Post-upgrade historical reports use the new validator/fraction cancellation
				// call. Call 17 retains only the original v16 vector-index ABI.
				let cancelled = crate::CancelledSlashes::<T>::get(execution).iter().any(
					|(validator, fraction)| {
						validator == &record.slash.validator && *fraction >= record.fraction
					},
				);
				if !cancelled {
					apply_slash::<T>(record.slash, record.offence_era);
				}
				HistoricalSlashHead::<T>::insert(execution, head + 1);
				clear_cancelled_if_drained::<T>(execution);
				return weight
					.saturating_add(T::WeightInfo::apply_slash(nominators))
					.saturating_add(
						T::DbWeight::get().reads_writes(8 + 4 * reporters, 3 + 4 * reporters),
					)
					.saturating_add(Weight::from_parts(0, bytes));
			}
			// A missing entry is an invariant violation, never a reason to skip or release
			// stake. Retain the head so that try-runtime detects and reports it.
			return weight;
		}
	}
	weight
}

/// Structural checks accept a due backlog while ensuring no entry can be skipped or
/// treated as withdrawn. They are deliberately exhaustive only in rehearsal/test builds.
#[cfg(any(feature = "try-runtime", test))]
pub fn try_state<T: Config>() -> Result<(), sp_runtime::TryRuntimeError> {
	let Some(cutoff) = CutoffEra::<T>::get() else {
		ensure!(!FirstExecutionEra::<T>::exists(), "legacy execution marker without cutoff");
		ensure!(
			HistoricalSlashTail::<T>::iter().next().is_none(),
			"historical queue without cutoff"
		);
		return Ok(());
	};
	let first = FirstExecutionEra::<T>::get().unwrap_or(cutoff);
	ensure!(
		cutoff == first || cutoff == first.saturating_add(1),
		"invalid legacy activation markers"
	);
	let last = cutoff.saturating_add(T::SlashDeferDuration::get()).saturating_add(1);
	for (era, cursor) in LegacySlashCursor::<T>::iter() {
		ensure!(era >= first && era <= last, "legacy cursor outside admitted range");
		ensure!(
			UnappliedSlashes::<T>::decode_len(era).is_some_and(|len| (cursor as usize) < len),
			"legacy cursor does not identify a pending original record"
		);
	}
	for (era, tail) in HistoricalSlashTail::<T>::iter() {
		ensure!(era >= first && era <= last, "historical queue outside admitted range");
		let head = HistoricalSlashHead::<T>::get(era);
		ensure!(head <= tail, "historical queue head exceeds tail");
		let mut count = 0u64;
		for (index, record) in HistoricalSlashes::<T>::iter_prefix(era) {
			ensure!(index >= head && index < tail, "historical record outside pending range");
			ensure!(
				record.offence_era <= cutoff &&
					record
						.offence_era
						.saturating_add(T::SlashDeferDuration::get())
						.saturating_add(1) ==
						era,
				"historical record changed its original offence era"
			);
			ensure!(
				record.slash.others.len() <= T::MaxExposurePageSize::get() as usize,
				"historical record exceeds admitted full exposure bound"
			);
			count = count.saturating_add(1);
		}
		ensure!(count == tail - head, "historical queue has a missing record");
	}
	Ok(())
}

/// Clear the legacy spans when a stash is completely removed, preserving v16's
/// supplied span-count admission and allowing a later re-bond to start fresh.
pub fn clear_stash_metadata<T: Config>(
	stash: &T::AccountId,
	num_slashing_spans: u32,
) -> DispatchResult {
	let Some(spans) = SlashingSpans::<T>::get(stash) else {
		return Ok(());
	};
	ensure!(
		num_slashing_spans as usize >= spans.iter().count(),
		crate::Error::<T>::IncorrectSlashingSpans
	);
	SlashingSpans::<T>::remove(stash);
	for span in spans.iter() {
		SpanSlash::<T>::remove((stash.clone(), span.index));
	}
	Ok(())
}

pub fn clear_stash_weight<T: Config>(num_slashing_spans: u32) -> Weight {
	T::DbWeight::get()
		.reads_writes(1, 1 + num_slashing_spans as u64)
		.saturating_add(Weight::from_parts(
			1_000_000u64.saturating_mul(num_slashing_spans as u64),
			4096u64.saturating_add(4u64.saturating_mul(num_slashing_spans as u64)),
		))
}

/// Cancellation performs a linear decode/filter/encode of the frozen vector. Account
/// for its actual proof bytes and conservative CPU overhead in addition to the SDK weight.
/// This also covers governance cancelling historical stranded queues outside the cutoff.
pub fn cancel_weight<T: Config>(era: EraIndex, targets: u32) -> Weight {
	let bytes = sp_io::storage::read(&UnappliedSlashes::<T>::hashed_key_for(era), &mut [], 0)
		.unwrap_or(0) as u64;
	T::WeightInfo::cancel_deferred_slash(targets)
		.saturating_add(T::DbWeight::get().reads_writes(3, 2))
		.saturating_add(Weight::from_parts(bytes.saturating_mul(1_000_000), bytes))
}

/// Upper bound used before accepting a batch of historical reports. Every pre-cutoff
/// full exposure fits MaxExposurePageSize by migration admission, including all pages.
pub fn offence_weight<T: Config>(reporters_per_offence: impl IntoIterator<Item = u32>) -> Weight {
	let nominators = T::MaxExposurePageSize::get();
	reporters_per_offence.into_iter().fold(Weight::zero(), |weight, reporters| {
		weight
			.saturating_add(T::WeightInfo::rc_on_offence(1))
			.saturating_add(T::WeightInfo::apply_slash(nominators))
			.saturating_add(T::DbWeight::get().reads_writes(
				5 + 4 * nominators as u64 + 4 * reporters as u64,
				4 + 4 * nominators as u64 + 4 * reporters as u64,
			))
	})
}

/// Keeps the v16 call 17 argument encoding and sorted/unique index semantics.
pub fn cancel_deferred_slash<T: Config>(era: EraIndex, indices: Vec<u32>) -> DispatchResult {
	ensure!(!indices.is_empty(), crate::Error::<T>::EmptyTargets);
	ensure!(indices.windows(2).all(|p| p[0] < p[1]), crate::Error::<T>::NotSortedAndUnique);
	let mut queue = UnappliedSlashes::<T>::get(era);
	let cursor = LegacySlashCursor::<T>::get(era);
	ensure!(
		indices[0] >= cursor && indices[indices.len() - 1] < queue.len() as u32,
		crate::Error::<T>::InvalidSlashIndex
	);
	let mut removed = indices.into_iter().peekable();
	queue = queue
		.into_iter()
		.enumerate()
		.filter_map(|(index, slash)| {
			if removed.peek().is_some_and(|target| *target as usize == index) {
				removed.next();
				None
			} else {
				Some(slash)
			}
		})
		.collect();
	if queue.len() == cursor as usize {
		UnappliedSlashes::<T>::remove(era);
		LegacySlashCursor::<T>::remove(era);
	} else {
		UnappliedSlashes::<T>::insert(era, queue);
	}
	Ok(())
}

pub fn get_paged_exposure<T: Config>(
	era: EraIndex,
	validator: &T::AccountId,
	page: u32,
) -> Option<crate::PagedExposure<T::AccountId, BalanceOf<T>>> {
	(page == 0 && ErasStakersClipped::<T>::contains_key(era, validator))
		.then(|| crate::PagedExposure::from_clipped(ErasStakersClipped::<T>::get(era, validator)))
}

pub fn get_full_exposure<T: Config>(
	era: EraIndex,
	validator: &T::AccountId,
) -> Exposure<T::AccountId, BalanceOf<T>> {
	if ErasStakers::<T>::contains_key(era, validator) {
		ErasStakers::<T>::get(era, validator)
	} else if crate::ErasStakersOverview::<T>::contains_key(era, validator) {
		crate::session_rotation::Eras::<T>::get_full_exposure(era, validator)
	} else {
		Exposure::default()
	}
}

/// `Some` means this report belongs to the historical max/span policy and was handled here.
/// Historical exposure bounds are checked by the upgrade, never asserted against a valid report.
/// New records have independent keys, so arbitrarily many reports cannot grow one queue value.
pub fn on_new_offences<T: Config>(
	era: EraIndex,
	offences: &[pallet_staking_async_rc_client::Offence<T::AccountId>],
) -> Option<Weight> {
	let cutoff = CutoffEra::<T>::get()?;
	if era > cutoff {
		return None;
	}
	let active = crate::ActiveEra::<T>::get()?.index;
	let mut weight = T::WeightInfo::rc_on_offence(offences.len() as u32);
	if era < active.saturating_sub(T::BondingDuration::get()) {
		return Some(weight);
	}
	let execution = era.saturating_add(T::SlashDeferDuration::get()).saturating_add(1);
	for offence in offences {
		if Invulnerables::<T>::get().contains(&offence.offender) {
			continue;
		}
		let exposure = get_full_exposure::<T>(era, &offence.offender);
		Pallet::<T>::deposit_event(crate::Event::<T>::SlashReported {
			validator: offence.offender.clone(),
			fraction: offence.slash_fraction,
			slash_era: era,
		});
		let nominators = exposure.others.len() as u32;
		let reporters = offence.reporters.len() as u64;
		// The old calculator touches per-nominator maxima and span records in addition to
		// the actual debit. Charge all those reads/writes even when no additional slash is due.
		weight.saturating_accrue(T::DbWeight::get().reads_writes(
			5 + 4 * nominators as u64 + 4 * reporters,
			4 + 4 * nominators as u64 + 4 * reporters,
		));
		if let Some(mut slash) = compute_slash::<T>(SlashParams {
			stash: &offence.offender,
			slash: offence.slash_fraction,
			exposure: &exposure,
			slash_era: era,
			window_start: active.saturating_sub(T::BondingDuration::get()),
			now: active,
			reward_proportion: crate::SlashRewardFraction::<T>::get(),
		}) {
			slash.reporters = offence.reporters.clone();
			weight.saturating_accrue(T::WeightInfo::apply_slash(nominators));
			if T::SlashDeferDuration::get() == 0 || execution <= active {
				// Retain the custom immediate application for first/incremental late reports.
				apply_slash::<T>(slash, era);
			} else {
				let tail = HistoricalSlashTail::<T>::get(execution);
				// Reaching u64::MAX requires more reports than can exist in the lifetime of
				// a 32-bit block-number chain; every report consumes at least one block byte.
				HistoricalSlashes::<T>::insert(
					execution,
					tail,
					HistoricalSlash { offence_era: era, fraction: offence.slash_fraction, slash },
				);
				HistoricalSlashTail::<T>::insert(execution, tail.saturating_add(1));
			}
		}
	}
	Some(weight)
}

/// The proportion of the slashing reward to be paid out on the first slashing detection.
/// This is f_1 in the paper.
const REWARD_F1: Perbill = Perbill::from_percent(50);

/// The index of a slashing span - unique to each stash.
pub type SpanIndex = u32;

// A range of start..end eras for a slashing span.
#[derive(Encode, Decode, Clone, TypeInfo, Debug, PartialEq, Eq)]
pub struct SlashingSpan {
	pub index: SpanIndex,
	pub start: EraIndex,
	pub length: Option<EraIndex>, // the ongoing slashing span has indeterminate length.
}

impl SlashingSpan {
	fn contains_era(&self, era: EraIndex) -> bool {
		self.start <= era && self.length.map_or(true, |l| self.start.saturating_add(l) > era)
	}
}

/// An encoding of all of a nominator's slashing spans.
#[derive(Encode, Decode, Clone, TypeInfo, Debug, PartialEq, Eq)]
pub struct SlashingSpansData {
	// the index of the current slashing span of the nominator. different for
	// every stash, resets when the account hits free balance 0.
	pub span_index: SpanIndex,
	// the start era of the most recent (ongoing) slashing span.
	pub last_start: EraIndex,
	// the last era at which a non-zero slash occurred.
	pub last_nonzero_slash: EraIndex,
	// all prior slashing spans' start indices, in reverse order (most recent first)
	// encoded as offsets relative to the slashing span after it.
	pub prior: Vec<EraIndex>,
}

impl SlashingSpansData {
	// creates a new record of slashing spans for a stash, starting at the beginning
	// of the bonding period, relative to now.
	pub(crate) fn new(window_start: EraIndex) -> Self {
		SlashingSpansData {
			span_index: 0,
			last_start: window_start,
			// initialize to zero, as this structure is lazily created until
			// the first slash is applied. setting equal to `window_start` would
			// put a time limit on nominations.
			last_nonzero_slash: 0,
			prior: Vec::new(),
		}
	}

	// update the slashing spans to reflect the start of a new span at the era after `now`
	// returns `true` if a new span was started, `false` otherwise. `false` indicates
	// that internal state is unchanged.
	pub(crate) fn end_span(&mut self, now: EraIndex) -> bool {
		let next_start = now.defensive_saturating_add(1);
		if next_start <= self.last_start {
			return false;
		}

		let last_length = next_start.defensive_saturating_sub(self.last_start);
		self.prior.insert(0, last_length);
		self.last_start = next_start;
		self.span_index.defensive_saturating_accrue(1);
		true
	}

	// an iterator over all slashing spans in _reverse_ order - most recent first.
	pub(crate) fn iter(&'_ self) -> impl Iterator<Item = SlashingSpan> + '_ {
		let mut last_start = self.last_start;
		let mut index = self.span_index;
		let last = SlashingSpan { index, start: last_start, length: None };
		let prior = self.prior.iter().cloned().map(move |length| {
			let start = last_start.defensive_saturating_sub(length);
			last_start = start;
			index.defensive_saturating_reduce(1);

			SlashingSpan { index, start, length: Some(length) }
		});

		core::iter::once(last).chain(prior)
	}

	/// Yields the era index where the most recent non-zero slash occurred.
	pub fn last_nonzero_slash(&self) -> EraIndex {
		self.last_nonzero_slash
	}

	// prune the slashing spans against a window, whose start era index is given.
	//
	// If this returns `Some`, then it includes a range start..end of all the span
	// indices which were pruned.
	fn prune(&mut self, window_start: EraIndex) -> Option<(SpanIndex, SpanIndex)> {
		let old_idx = self
			.iter()
			.skip(1) // skip ongoing span.
			.position(|span| {
				span.length
					.map_or(false, |len| span.start.defensive_saturating_add(len) <= window_start)
			});

		let earliest_span_index =
			self.span_index.defensive_saturating_sub(self.prior.len() as SpanIndex);
		let pruned = match old_idx {
			Some(o) => {
				self.prior.truncate(o);
				let new_earliest =
					self.span_index.defensive_saturating_sub(self.prior.len() as SpanIndex);
				Some((earliest_span_index, new_earliest))
			},
			None => None,
		};

		// readjust the ongoing span, if it started before the beginning of the window.
		self.last_start = core::cmp::max(self.last_start, window_start);
		pruned
	}
}

/// A slashing-span record for a particular stash.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Clone,
	Default,
	TypeInfo,
	MaxEncodedLen,
	PartialEq,
	Eq,
	Debug,
)]
pub struct SpanRecord<Balance> {
	pub slashed: Balance,
	pub paid_out: Balance,
}

/// Parameters for performing a slash.
#[derive(Clone)]
pub(crate) struct SlashParams<'a, T: 'a + Config> {
	/// The stash account being slashed.
	pub(crate) stash: &'a T::AccountId,
	/// The proportion of the slash.
	pub(crate) slash: Perbill,
	/// The exposure of the stash and all nominators.
	pub(crate) exposure: &'a Exposure<T::AccountId, BalanceOf<T>>,
	/// The era where the offence occurred.
	pub(crate) slash_era: EraIndex,
	/// The first era in the current bonding period.
	pub(crate) window_start: EraIndex,
	/// The current era.
	pub(crate) now: EraIndex,
	/// The maximum percentage of a slash that ever gets paid out.
	/// This is f_inf in the paper.
	pub(crate) reward_proportion: Perbill,
}

/// Computes a slash of a validator and nominators. It returns an unapplied
/// record to be applied at some later point. Slashing metadata is updated in storage,
/// since unapplied records are only rarely intended to be dropped.
///
/// The pending slash record returned does not have initialized reporters. Those have
/// to be set at a higher level, if any.
pub(crate) fn compute_slash<T: Config>(
	params: SlashParams<T>,
) -> Option<LegacyUnappliedSlash<T::AccountId, BalanceOf<T>>> {
	let mut reward_payout = Zero::zero();
	let mut val_slashed = Zero::zero();

	// is the slash amount here a maximum for the era?
	let own_slash = params.slash * params.exposure.own;
	if params.slash * params.exposure.total == Zero::zero() {
		// kick out the validator even if they won't be slashed,
		// as long as the misbehavior is from their most recent slashing span.
		kick_out_if_recent::<T>(params);
		return None;
	}

	let prior_slash_p = ValidatorSlashInEra::<T>::get(&params.slash_era, params.stash)
		.map_or(Zero::zero(), |(prior_slash_proportion, _)| prior_slash_proportion);

	// compare slash proportions rather than slash values to avoid issues due to rounding
	// error.
	if params.slash.deconstruct() > prior_slash_p.deconstruct() {
		ValidatorSlashInEra::<T>::insert(
			&params.slash_era,
			params.stash,
			&(params.slash, own_slash),
		);
	} else {
		// we slash based on the max in era - this new event is not the max,
		// so neither the validator or any nominators will need an update.
		//
		// this does lead to a divergence of our system from the paper, which
		// pays out some reward even if the latest report is not max-in-era.
		// we opt to avoid the nominator lookups and edits and leave more rewards
		// for more drastic misbehavior.
		return None;
	}

	// apply slash to validator.
	{
		let mut spans = fetch_spans::<T>(
			params.stash,
			params.window_start,
			&mut reward_payout,
			&mut val_slashed,
			params.reward_proportion,
		);

		let target_span = spans.compare_and_update_span_slash(params.slash_era, own_slash);

		if target_span == Some(spans.span_index()) {
			// misbehavior occurred within the current slashing span - end current span.
			// Check <https://github.com/paritytech/polkadot-sdk/issues/2650> for details.
			spans.end_span(params.now);
		}
	}

	let mut nominators_slashed = Vec::new();
	reward_payout += slash_nominators::<T>(params.clone(), prior_slash_p, &mut nominators_slashed);

	Some(LegacyUnappliedSlash {
		validator: params.stash.clone(),
		own: val_slashed,
		others: nominators_slashed,
		reporters: Vec::new(),
		payout: reward_payout,
	})
}

// doesn't apply any slash, but kicks out the validator if the misbehavior is from
// the most recent slashing span.
fn kick_out_if_recent<T: Config>(params: SlashParams<T>) {
	// these are not updated by era-span or end-span.
	let mut reward_payout = Zero::zero();
	let mut val_slashed = Zero::zero();
	let mut spans = fetch_spans::<T>(
		params.stash,
		params.window_start,
		&mut reward_payout,
		&mut val_slashed,
		params.reward_proportion,
	);

	if spans.era_span(params.slash_era).map(|s| s.index) == Some(spans.span_index()) {
		// Check https://github.com/paritytech/polkadot-sdk/issues/2650 for details
		spans.end_span(params.now);
	}
}

/// Slash nominators. Accepts general parameters and the prior slash percentage of the validator.
///
/// Returns the amount of reward to pay out.
fn slash_nominators<T: Config>(
	params: SlashParams<T>,
	prior_slash_p: Perbill,
	nominators_slashed: &mut Vec<(T::AccountId, BalanceOf<T>)>,
) -> BalanceOf<T> {
	let mut reward_payout = Zero::zero();

	nominators_slashed.reserve(params.exposure.others.len());
	for nominator in &params.exposure.others {
		let stash = &nominator.who;
		let mut nom_slashed = Zero::zero();

		// the era slash of a nominator always grows, if the validator
		// had a new max slash for the era.
		let era_slash = {
			let own_slash_prior = prior_slash_p * nominator.value;
			let own_slash_by_validator = params.slash * nominator.value;
			let own_slash_difference = own_slash_by_validator.saturating_sub(own_slash_prior);

			let mut era_slash =
				NominatorSlashInEra::<T>::get(&params.slash_era, stash).unwrap_or_else(Zero::zero);
			era_slash += own_slash_difference;
			NominatorSlashInEra::<T>::insert(&params.slash_era, stash, &era_slash);

			era_slash
		};

		// compare the era slash against other eras in the same span.
		{
			let mut spans = fetch_spans::<T>(
				stash,
				params.window_start,
				&mut reward_payout,
				&mut nom_slashed,
				params.reward_proportion,
			);

			let target_span = spans.compare_and_update_span_slash(params.slash_era, era_slash);

			if target_span == Some(spans.span_index()) {
				// end the span, but don't chill the nominator.
				spans.end_span(params.now);
			}
		}
		nominators_slashed.push((stash.clone(), nom_slashed));
	}

	reward_payout
}

// helper struct for managing a set of spans we are currently inspecting.
// writes alterations to disk on drop, but only if a slash has been carried out.
//
// NOTE: alterations to slashing metadata should not be done after this is dropped.
// dropping this struct applies any necessary slashes, which can lead to free balance
// being 0, and the account being garbage-collected -- a dead account should get no new
// metadata.
struct InspectingSpans<'a, T: Config + 'a> {
	dirty: bool,
	window_start: EraIndex,
	stash: &'a T::AccountId,
	spans: SlashingSpansData,
	paid_out: &'a mut BalanceOf<T>,
	slash_of: &'a mut BalanceOf<T>,
	reward_proportion: Perbill,
	_marker: core::marker::PhantomData<T>,
}

// fetches the slashing spans record for a stash account, initializing it if necessary.
fn fetch_spans<'a, T: Config + 'a>(
	stash: &'a T::AccountId,
	window_start: EraIndex,
	paid_out: &'a mut BalanceOf<T>,
	slash_of: &'a mut BalanceOf<T>,
	reward_proportion: Perbill,
) -> InspectingSpans<'a, T> {
	let spans = SlashingSpans::<T>::get(stash).unwrap_or_else(|| {
		let spans = SlashingSpansData::new(window_start);
		SlashingSpans::<T>::insert(stash, &spans);
		spans
	});

	InspectingSpans {
		dirty: false,
		window_start,
		stash,
		spans,
		slash_of,
		paid_out,
		reward_proportion,
		_marker: core::marker::PhantomData,
	}
}

impl<'a, T: 'a + Config> InspectingSpans<'a, T> {
	fn span_index(&self) -> SpanIndex {
		self.spans.span_index
	}

	fn end_span(&mut self, now: EraIndex) {
		self.dirty = self.spans.end_span(now) || self.dirty;
	}

	// add some value to the slash of the staker.
	// invariant: the staker is being slashed for non-zero value here
	// although `amount` may be zero, as it is only a difference.
	fn add_slash(&mut self, amount: BalanceOf<T>, slash_era: EraIndex) {
		*self.slash_of += amount;
		self.spans.last_nonzero_slash = core::cmp::max(self.spans.last_nonzero_slash, slash_era);
	}

	// find the span index of the given era, if covered.
	fn era_span(&self, era: EraIndex) -> Option<SlashingSpan> {
		self.spans.iter().find(|span| span.contains_era(era))
	}

	// compares the slash in an era to the overall current span slash.
	// if it's higher, applies the difference of the slashes and then updates the span on disk.
	//
	// returns the span index of the era where the slash occurred, if any.
	fn compare_and_update_span_slash(
		&mut self,
		slash_era: EraIndex,
		slash: BalanceOf<T>,
	) -> Option<SpanIndex> {
		let target_span = self.era_span(slash_era)?;
		let span_slash_key = (self.stash.clone(), target_span.index);
		let mut span_record = SpanSlash::<T>::get(&span_slash_key);
		let mut changed = false;

		let reward = if span_record.slashed < slash {
			// new maximum span slash. apply the difference.
			let difference = slash.defensive_saturating_sub(span_record.slashed);
			span_record.slashed = slash;

			// compute reward.
			let reward =
				REWARD_F1 * (self.reward_proportion * slash).saturating_sub(span_record.paid_out);

			self.add_slash(difference, slash_era);
			changed = true;

			reward
		} else if span_record.slashed == slash {
			// compute reward. no slash difference to apply.
			REWARD_F1 * (self.reward_proportion * slash).saturating_sub(span_record.paid_out)
		} else {
			Zero::zero()
		};

		if !reward.is_zero() {
			changed = true;
			span_record.paid_out += reward;
			*self.paid_out += reward;
		}

		if changed {
			self.dirty = true;
			SpanSlash::<T>::insert(&span_slash_key, &span_record);
		}

		Some(target_span.index)
	}
}

impl<'a, T: 'a + Config> Drop for InspectingSpans<'a, T> {
	fn drop(&mut self) {
		// only update on disk if we slashed this account.
		if !self.dirty {
			return;
		}

		if let Some((start, end)) = self.spans.prune(self.window_start) {
			for span_index in start..end {
				SpanSlash::<T>::remove(&(self.stash.clone(), span_index));
			}
		}

		SlashingSpans::<T>::insert(self.stash, &self.spans);
	}
}

// apply the slash to a stash account, deducting any missing funds from the reward
// payout, saturating at 0. this is mildly unfair but also an edge-case that
// can only occur when overlapping locked funds have been slashed.
pub fn do_slash<T: Config>(
	stash: &T::AccountId,
	value: BalanceOf<T>,
	reward_payout: &mut BalanceOf<T>,
	slashed_imbalance: &mut NegativeImbalanceOf<T>,
	slash_era: EraIndex,
) {
	let mut ledger =
		match Pallet::<T>::ledger(sp_staking::StakingAccount::Stash(stash.clone())).defensive() {
			Ok(ledger) => ledger,
			Err(_) => return, // nothing to do.
		};

	let value = ledger.slash(value, asset::existential_deposit::<T>(), slash_era);
	if value.is_zero() {
		// nothing to do
		return;
	}

	// Skip slashing for virtual stakers. The pallets managing them should handle the slashing.
	if !crate::VirtualStakers::<T>::contains_key(stash) {
		let (imbalance, missing) = asset::slash::<T>(stash, value);
		slashed_imbalance.subsume(imbalance);

		if !missing.is_zero() {
			// deduct overslash from the reward payout
			*reward_payout = reward_payout.saturating_sub(missing);
		}
	}

	let _ = ledger
		.update()
		.defensive_proof("ledger fetched from storage so it exists in storage; qed.");

	// trigger the event
	<Pallet<T>>::deposit_event(crate::Event::<T>::Slashed { staker: stash.clone(), amount: value });
}

/// Apply a previously-unapplied slash.
pub(crate) fn apply_slash<T: Config>(
	unapplied_slash: LegacyUnappliedSlash<T::AccountId, BalanceOf<T>>,
	slash_era: EraIndex,
) {
	let mut slashed_imbalance = NegativeImbalanceOf::<T>::zero();
	let mut reward_payout = unapplied_slash.payout;

	do_slash::<T>(
		&unapplied_slash.validator,
		unapplied_slash.own,
		&mut reward_payout,
		&mut slashed_imbalance,
		slash_era,
	);

	for &(ref nominator, nominator_slash) in &unapplied_slash.others {
		do_slash::<T>(
			nominator,
			nominator_slash,
			&mut reward_payout,
			&mut slashed_imbalance,
			slash_era,
		);
	}

	pay_reporters::<T>(reward_payout, slashed_imbalance, &unapplied_slash.reporters);
}

/// Apply a reward payout to some reporters, paying the rewards out of the slashed imbalance.
fn pay_reporters<T: Config>(
	reward_payout: BalanceOf<T>,
	slashed_imbalance: NegativeImbalanceOf<T>,
	reporters: &[T::AccountId],
) {
	if reward_payout.is_zero() || reporters.is_empty() {
		// nobody to pay out to or nothing to pay;
		// just treat the whole value as slashed.
		T::Slash::on_unbalanced(slashed_imbalance);
		return;
	}

	// take rewards out of the slashed imbalance.
	let reward_payout = reward_payout.min(slashed_imbalance.peek());
	let (mut reward_payout, mut value_slashed) = slashed_imbalance.split(reward_payout);

	let per_reporter = reward_payout.peek() / (reporters.len() as u32).into();
	for reporter in reporters {
		let (reporter_reward, rest) = reward_payout.split(per_reporter);
		reward_payout = rest;

		// this cancels out the reporter reward imbalance internally, leading
		// to no change in total issuance.
		asset::deposit_slashed::<T>(reporter, reporter_reward);
	}

	// the rest goes to the on-slash imbalance handler (e.g. treasury)
	value_slashed.subsume(reward_payout); // remainder of reward division remains.
	T::Slash::on_unbalanced(value_slashed);
}
