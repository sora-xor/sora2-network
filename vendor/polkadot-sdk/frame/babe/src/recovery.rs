// SPDX-License-Identifier: Apache-2.0
//! Bounded epoch attribution for SORA Nexus recovery maintenance.
//!
//! Economic sessions are frozen while BABE epochs continue. The ordinary
//! SkippedEpochs offset model cannot represent a constant session over many
//! epochs; retain these intervals independently of that rolling cache.

use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use frame_support::{traits::ConstU32, BoundedVec};

pub const STORAGE_KEY: &[u8] = b":sora-nexus:recovery:babe-intervals:v1";
const POLICY_KEY: &[u8] = b":sora-nexus:recovery:policy:v1";
pub const MAX_INTERVALS: u32 = 8;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct EpochInterval {
	pub first_epoch: u64,
	pub last_epoch: u64,
	pub session: u32,
	pub authorities: BoundedVec<sp_consensus_babe::AuthorityId, ConstU32<1000>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct RecoveryEpochs {
	pub policy: [u8; 32],
	pub intervals: BoundedVec<EpochInterval, ConstU32<MAX_INTERVALS>>,
}

impl RecoveryEpochs {
	pub fn valid(&self) -> bool {
		self.policy != [0; 32] &&
			!self.intervals.is_empty() &&
			self.intervals
				.iter()
				.all(|item| item.first_epoch <= item.last_epoch && !item.authorities.is_empty()) &&
			self.intervals.windows(2).all(|pair| pair[0].last_epoch < pair[1].first_epoch)
	}

	pub fn session_for_epoch(&self, epoch: u64) -> Option<u32> {
		self.valid()
			.then(|| {
				self.intervals
					.iter()
					.find(|item| (item.first_epoch..=item.last_epoch).contains(&epoch))
					.map(|item| item.session)
			})
			.flatten()
	}
}

fn load() -> Option<RecoveryEpochs> {
	let bytes = sp_io::storage::get(STORAGE_KEY)?;
	if bytes.len() > 512 * 1024 {
		return None;
	}
	let mut input = bytes.as_ref();
	let record = RecoveryEpochs::decode_with_mem_limit(&mut input, 1024 * 1024).ok()?;
	if !input.is_empty() ||
		sp_io::storage::get(POLICY_KEY).as_deref() != Some(record.policy.as_slice())
	{
		return None;
	}
	record.valid().then_some(record)
}

pub(crate) fn key_allowed(epoch: u64, key: &sp_consensus_babe::AuthorityId, count: u32) -> bool {
	let Some(record) = load() else {
		return true;
	};
	match record
		.intervals
		.iter()
		.find(|item| (item.first_epoch..=item.last_epoch).contains(&epoch))
	{
		Some(interval) => {
			interval.authorities.len() == count as usize && interval.authorities.contains(key)
		},
		None => true,
	}
}

pub(crate) fn session_for_epoch(epoch: u64) -> Option<u32> {
	load()?.session_for_epoch(epoch)
}

/// The first normal session after the frozen interval needs a fresh offset
/// anchor even when its epoch is contiguous. Otherwise the old SkippedEpochs
/// offset incorrectly counts every maintenance epoch as an economic session.
pub(crate) fn resume_anchor_needed(epoch: u64, session: Option<u32>) -> bool {
	load()
		.and_then(|record| record.intervals.last().cloned())
		.is_some_and(|last| last.session.checked_add(1) == session && epoch > last.last_epoch)
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::crypto::ByteArray;
	fn authorities() -> BoundedVec<sp_consensus_babe::AuthorityId, ConstU32<1000>> {
		alloc::vec![sp_consensus_babe::AuthorityId::from_slice(&[2; 32]).unwrap()]
			.try_into()
			.unwrap()
	}

	#[test]
	fn frozen_interval_does_not_increment_session_and_preserves_old_epochs() {
		let record = RecoveryEpochs {
			policy: [1; 32],
			intervals: alloc::vec![EpochInterval {
				first_epoch: 100,
				last_epoch: 10000,
				session: 80,
				authorities: authorities(),
			},]
			.try_into()
			.unwrap(),
		};
		assert_eq!(record.session_for_epoch(99), None);
		assert_eq!(record.session_for_epoch(100), Some(80));
		assert_eq!(record.session_for_epoch(5000), Some(80));
		assert_eq!(record.session_for_epoch(10000), Some(80));
		assert_eq!(record.session_for_epoch(10001), None);
	}

	#[test]
	fn malformed_or_overlapping_intervals_are_rejected() {
		let mut record = RecoveryEpochs {
			policy: [1; 32],
			intervals: alloc::vec![
				EpochInterval {
					first_epoch: 2,
					last_epoch: 4,
					session: 1,
					authorities: authorities(),
				},
				EpochInterval {
					first_epoch: 4,
					last_epoch: 8,
					session: 1,
					authorities: authorities(),
				},
			]
			.try_into()
			.unwrap(),
		};
		assert!(!record.valid());
		record.intervals[1].first_epoch = 5;
		assert!(record.valid());
		record.intervals[0].first_epoch = 5;
		assert!(!record.valid());
	}

	#[test]
	fn persisted_intervals_require_matching_policy_and_exact_encoding() {
		sp_io::TestExternalities::default().execute_with(|| {
			let record = RecoveryEpochs {
				policy: [1; 32],
				intervals: alloc::vec![EpochInterval {
					first_epoch: 2,
					last_epoch: 4,
					session: 1,
					authorities: authorities(),
				},]
				.try_into()
				.unwrap(),
			};
			sp_io::storage::set(STORAGE_KEY, &record.encode());
			assert_eq!(session_for_epoch(3), None);
			sp_io::storage::set(POLICY_KEY, &[1; 32]);
			assert_eq!(session_for_epoch(3), Some(1));
			assert!(key_allowed(3, &record.intervals[0].authorities[0], 1));
			assert!(!key_allowed(3, &record.intervals[0].authorities[0], 2));
			assert!(!key_allowed(
				3,
				&sp_consensus_babe::AuthorityId::from_slice(&[3; 32]).unwrap(),
				1
			));
			assert!(resume_anchor_needed(5, Some(2)));
			assert!(!resume_anchor_needed(5, Some(3)));
			let mut bad = record.encode();
			bad.push(0);
			sp_io::storage::set(STORAGE_KEY, &bad);
			assert_eq!(session_for_epoch(3), None);
		});
	}
}
