// This file is part of Substrate.
// Copyright (C) 2026 SORA Nexus contributors.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

//! Apply queued authority changes before polling any signing voter.

use futures::Stream;
use std::{
	pin::Pin,
	task::{Context, Poll},
};

const COMMAND_BUDGET: usize = 64;

/// `true` permits voter polling only after the command stream returned Pending.
/// Exhausting the budget keeps voting paused for the next executor turn.
pub(super) fn poll_commands<S: Stream + Unpin>(
	commands: &mut S,
	cx: &mut Context<'_>,
) -> Result<(Vec<S::Item>, bool), ()> {
	let mut batch = Vec::new();
	for _ in 0..COMMAND_BUDGET {
		match Pin::new(&mut *commands).poll_next(cx) {
			Poll::Ready(Some(command)) => batch.push(command),
			Poll::Ready(None) => return Err(()),
			Poll::Pending => return Ok((batch, true)),
		}
	}
	Ok((batch, false))
}

/// A mismatched persisted round identity must never be reset implicitly or voted.
pub(super) fn validate_round_set(authority_set: u64, round_set: u64) -> Result<(), ()> {
	if authority_set == round_set {
		Ok(())
	} else {
		Err(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use futures::{stream, StreamExt};

	#[test]
	fn applies_every_queued_change_before_allowing_voting() {
		let mut commands = stream::iter([Some(7956), None, Some(7957)]).chain(stream::pending());
		let mut cx = Context::from_waker(futures::task::noop_waker_ref());
		let (batch, ready) = poll_commands(&mut commands, &mut cx).unwrap();
		assert_eq!(batch, vec![Some(7956), None, Some(7957)]);
		assert!(ready);
		let mut voting_set = None;
		for command in batch {
			voting_set = command;
		}
		assert_eq!(voting_set, Some(7957));
	}

	#[test]
	fn large_backlog_yields_without_polling_an_intermediate_voter() {
		let mut commands = stream::iter(0..COMMAND_BUDGET + 2).chain(stream::pending());
		let mut cx = Context::from_waker(futures::task::noop_waker_ref());
		let (first, ready) = poll_commands(&mut commands, &mut cx).unwrap();
		assert_eq!(first.len(), COMMAND_BUDGET);
		assert!(!ready);
		let (last, ready) = poll_commands(&mut commands, &mut cx).unwrap();
		assert_eq!(last, vec![COMMAND_BUDGET, COMMAND_BUDGET + 1]);
		assert!(ready);
	}

	#[test]
	fn permanently_ready_stream_has_bounded_poll_work_and_never_votes() {
		let mut commands = stream::repeat(7956u64);
		let mut cx = Context::from_waker(futures::task::noop_waker_ref());
		for _ in 0..3 {
			let (batch, ready) = poll_commands(&mut commands, &mut cx).unwrap();
			assert_eq!(batch.len(), COMMAND_BUDGET);
			assert!(!ready);
		}
	}

	#[test]
	fn mismatched_saved_rounds_fail_closed_without_a_reset() {
		assert!(validate_round_set(7957, 7956).is_err());
		assert!(validate_round_set(7957, 7958).is_err());
		assert!(validate_round_set(7957, 7957).is_ok());
	}

	#[test]
	fn closed_command_channel_fails_before_voting() {
		let mut commands = stream::empty::<u64>();
		let mut cx = Context::from_waker(futures::task::noop_waker_ref());
		assert!(poll_commands(&mut commands, &mut cx).is_err());
	}
}
