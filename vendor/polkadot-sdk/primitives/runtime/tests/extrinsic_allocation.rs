//! Regressions for allocation decisions made before an extrinsic length is validated.

#![cfg(feature = "std")]

use codec::{Compact, Decode, Encode, Error, Input};
use sp_runtime::generic::UncheckedExtrinsic;
use std::{
	alloc::{GlobalAlloc, Layout, System},
	cell::Cell,
	sync::atomic::{AtomicUsize, Ordering},
};

struct RecordingAllocator;

thread_local! {
	static RECORD: Cell<bool> = const { Cell::new(false) };
}

static MAX_ALLOCATION: AtomicUsize = AtomicUsize::new(0);

fn record(size: usize) {
	if RECORD.try_with(Cell::get).unwrap_or(false) {
		MAX_ALLOCATION.fetch_max(size, Ordering::Relaxed);
	}
}

// SAFETY: Allocation and deallocation are delegated unchanged to the system
// allocator. Recording uses only preinitialized thread-local/atomic values.
unsafe impl GlobalAlloc for RecordingAllocator {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		record(layout.size());
		unsafe { System.alloc(layout) }
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		unsafe { System.dealloc(ptr, layout) }
	}

	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
		record(size);
		unsafe { System.realloc(ptr, layout, size) }
	}
}

#[global_allocator]
static ALLOCATOR: RecordingAllocator = RecordingAllocator;

struct UnknownLength<'a>(&'a [u8]);

impl Input for UnknownLength<'_> {
	fn remaining_len(&mut self) -> Result<Option<usize>, Error> {
		Ok(None)
	}

	fn read(&mut self, into: &mut [u8]) -> Result<(), Error> {
		self.0.read(into)
	}
}

#[test]
fn forged_extrinsic_length_cannot_drive_eager_allocation() {
	type Extrinsic = UncheckedExtrinsic<u64, Vec<u8>, (), ()>;

	// The call is an empty Vec and the complete payload is two bytes, but the
	// encoded prefix claims nearly four GiB. Exercise real decoding with both
	// slice and streaming Inputs while recording actual allocator requests.
	let mut encoded = Compact(u32::MAX).encode();
	encoded.extend([5, 0]);
	for unknown_length in [false, true] {
		MAX_ALLOCATION.store(0, Ordering::Relaxed);
		RECORD.with(|enabled| enabled.set(true));
		let result = if unknown_length {
			Extrinsic::decode(&mut UnknownLength(&encoded))
		} else {
			Extrinsic::decode(&mut encoded.as_slice())
		};
		RECORD.with(|enabled| enabled.set(false));
		assert_eq!(result.unwrap_err().to_string(), "Invalid length prefix");
		assert!(
			MAX_ALLOCATION.load(Ordering::Relaxed) <= 1024,
			"malformed tiny extrinsic triggered an excessive allocation"
		);
	}
}
