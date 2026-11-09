//! Guardian bridge halts, as bridge workers read them.
//!
//! The `bridge-halt` pallet numbers every guardian halt and writes the newest
//! one to [`GUARDIAN_HALT_STORAGE_KEY`]. A bridge worker (the HASHI offchain
//! worker in the node, or a relayer) compares it with the halt its operator
//! last confirmed: while they differ, the worker does nothing. Operators confirm
//! a halt on their own servers, so no on-chain call restarts the workers.
//!
//! This is separate from Root's recovery latch,
//! [`crate::recovery::BRIDGE_HALT_STORAGE_KEY`], which makes the bridge pallets
//! themselves refuse calls.

use codec::{Decode, DecodeAll, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// Raw storage key holding the newest guardian halt as a SCALE [`GuardianHalt`].
/// It is absent until the first halt. It lies outside
/// [`crate::recovery::STORAGE_PREFIX`], so recovery admission does not treat it
/// as recovery control.
pub const GUARDIAN_HALT_STORAGE_KEY: &[u8] = b":sora-nexus:bridge-halt:latest:v1";

/// One guardian halt: its number (1 for the first, then counting up) and the
/// block that recorded it. The block tells apart two halts with the same number
/// if the chain is ever rewound to an earlier block.
#[derive(
    Clone,
    Copy,
    Debug,
    Decode,
    DecodeWithMemTracking,
    Encode,
    Eq,
    MaxEncodedLen,
    PartialEq,
    TypeInfo,
)]
pub struct GuardianHalt {
    pub number: u32,
    pub block: u32,
}

/// The newest guardian halt in the current state.
///
/// `Err` means the stored value does not decode. Workers treat that as a halt
/// no operator can have confirmed.
pub fn latest_guardian_halt() -> Result<Option<GuardianHalt>, codec::Error> {
    sp_io::storage::get(GUARDIAN_HALT_STORAGE_KEY)
        .map(|bytes| GuardianHalt::decode_all(&mut &bytes[..]))
        .transpose()
}

/// Whether bridge workers may run, given the newest halt on chain and the halt
/// their operator last confirmed.
///
/// They may run before any halt, and after a halt only when the operator
/// confirmed exactly that halt. The next halt, or a value that does not decode,
/// stops them again.
pub fn workers_may_run(
    latest: &Result<Option<GuardianHalt>, codec::Error>,
    confirmed: Option<GuardianHalt>,
) -> bool {
    match latest {
        Ok(None) => true,
        Ok(Some(halt)) => confirmed == Some(*halt),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: GuardianHalt = GuardianHalt {
        number: 1,
        block: 70,
    };
    const SECOND: GuardianHalt = GuardianHalt {
        number: 2,
        block: 90,
    };

    #[test]
    fn workers_run_only_after_their_operator_confirms_the_newest_halt() {
        assert!(workers_may_run(&Ok(None), None));
        assert!(workers_may_run(&Ok(None), Some(FIRST)));
        assert!(!workers_may_run(&Ok(Some(FIRST)), None));
        assert!(workers_may_run(&Ok(Some(FIRST)), Some(FIRST)));
        // The next halt stops them again; an older confirmation does not cover it.
        assert!(!workers_may_run(&Ok(Some(SECOND)), Some(FIRST)));
        // A confirmation of a later number never covers an earlier halt.
        assert!(!workers_may_run(&Ok(Some(FIRST)), Some(SECOND)));
        // Same number, different block: a rewound chain's new halt.
        let rewound = GuardianHalt {
            number: 1,
            block: 71,
        };
        assert!(!workers_may_run(&Ok(Some(rewound)), Some(FIRST)));
        assert!(!workers_may_run(
            &Err(codec::Error::from("corrupt")),
            Some(FIRST)
        ));
    }

    #[test]
    fn the_stored_halt_must_decode_exactly() {
        sp_io::TestExternalities::default().execute_with(|| {
            assert_eq!(latest_guardian_halt(), Ok(None));
            sp_io::storage::set(GUARDIAN_HALT_STORAGE_KEY, &FIRST.encode());
            assert_eq!(latest_guardian_halt(), Ok(Some(FIRST)));
            for bad in [vec![], vec![1, 0, 0, 0], [FIRST.encode(), vec![0]].concat()] {
                sp_io::storage::set(GUARDIAN_HALT_STORAGE_KEY, &bad);
                assert!(latest_guardian_halt().is_err());
            }
        });
    }

    #[test]
    fn the_key_is_not_recovery_control() {
        assert!(!GUARDIAN_HALT_STORAGE_KEY.starts_with(crate::recovery::STORAGE_PREFIX));
        assert_ne!(
            GUARDIAN_HALT_STORAGE_KEY,
            crate::recovery::BRIDGE_HALT_STORAGE_KEY
        );
    }
}
