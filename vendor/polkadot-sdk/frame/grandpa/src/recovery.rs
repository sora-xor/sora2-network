// SPDX-License-Identifier: Apache-2.0
//! The single manifest-bound GRANDPA set introduced inside a frozen session.
//! All ordinary/historical sets retain the upstream strict session range.

use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use frame_support::{traits::ConstU32, BoundedVec};
use sp_consensus_grandpa::AuthorityId;

pub const STORAGE_KEY: &[u8] = b":sora-nexus:recovery:grandpa-set:v1";
const POLICY_KEY: &[u8] = b":sora-nexus:recovery:policy:v1";
const CHECKPOINT: [u8; 32] = [
	0x7f, 0x7c, 0x25, 0x3f, 0xa7, 0x2f, 0x32, 0xe7, 0xb0, 0x3f, 0x32, 0xbd, 0xc9, 0xae, 0x45, 0x6f,
	0xd7, 0x17, 0x4f, 0xb1, 0x31, 0xb6, 0x89, 0xbe, 0x6d, 0x87, 0x60, 0x06, 0x12, 0x41, 0x29, 0xf0,
];

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct RecoverySet {
	pub checkpoint: [u8; 32],
	pub policy: [u8; 32],
	pub previous_set: u64,
	pub session: u32,
	pub authorities: BoundedVec<AuthorityId, ConstU32<1000>>,
}

impl RecoverySet {
	pub fn accepts(&self, set: u64, session: u32, key: &AuthorityId) -> bool {
		self.checkpoint == CHECKPOINT &&
			self.policy != [0; 32] &&
			self.previous_set.checked_add(1) == Some(set) &&
			self.session == session &&
			self.authorities.contains(key)
	}
}

pub fn load() -> Option<RecoverySet> {
	let bytes = sp_io::storage::get(STORAGE_KEY)?;
	if bytes.len() > 33000 {
		return None;
	}
	let mut input = bytes.as_ref();
	let record = RecoverySet::decode_with_mem_limit(&mut input, 66000).ok()?;
	if !input.is_empty() ||
		sp_io::storage::get(POLICY_KEY).as_deref() != Some(record.policy.as_slice())
	{
		return None;
	}
	Some(record)
}

#[cfg(test)]
fn accepts(set: u64, session: u32, key: &AuthorityId) -> bool {
	load().is_some_and(|record| record.accepts(set, session, key))
}

/// Fresh-key membership in the frozen session belongs ONLY to the new set.
/// In particular it must not make an old-set report valid merely because the
/// old set also ends at that session. Later ordinary sessions use normal ranges.
pub(crate) fn override_range(set: u64, session: u32, key: &AuthorityId) -> Option<bool> {
	let record = load()?;
	if record.checkpoint != CHECKPOINT ||
		record.session != session ||
		!record.authorities.contains(key)
	{
		return None;
	}
	Some(record.accepts(set, session, key))
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::crypto::ByteArray;

	fn record() -> RecoverySet {
		RecoverySet {
			checkpoint: CHECKPOINT,
			policy: [1; 32],
			previous_set: 7,
			session: 12,
			authorities: alloc::vec![AuthorityId::from_slice(&[2; 32]).unwrap()]
				.try_into()
				.unwrap(),
		}
	}

	#[test]
	fn exception_is_exactly_the_fresh_set_session_and_key() {
		let record = record();
		let key = &record.authorities[0];
		assert!(record.accepts(8, 12, key));
		assert!(!record.accepts(7, 12, key));
		assert!(!record.accepts(9, 12, key));
		assert!(!record.accepts(8, 11, key));
		assert!(!record.accepts(8, 12, &AuthorityId::from_slice(&[3; 32]).unwrap()));
	}

	#[test]
	fn exception_cannot_be_enabled_without_the_committed_policy() {
		sp_io::TestExternalities::default().execute_with(|| {
			let record = record();
			sp_io::storage::set(STORAGE_KEY, &record.encode());
			assert!(!accepts(8, 12, &record.authorities[0]));
			sp_io::storage::set(POLICY_KEY, &[1; 32]);
			assert!(accepts(8, 12, &record.authorities[0]));
			assert_eq!(override_range(7, 12, &record.authorities[0]), Some(false));
			assert_eq!(override_range(8, 13, &record.authorities[0]), None);
			sp_io::storage::set(POLICY_KEY, &[4; 32]);
			assert!(!accepts(8, 12, &record.authorities[0]));
		});
	}
}
