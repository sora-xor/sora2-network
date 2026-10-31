//! Release compiler output slot. No production recovery input has been supplied.
//!
//! Empty data is intentional and fails mainnet recovery admission. Never insert
//! example accounts, synthetic authority keys or rehearsal commitments here.
//! The reviewed recovery compiler replaces this file only after all release
//! inputs and independent checkpoint/economic verification are available.

pub const POLICY_SCALE: &[u8] = &[];
pub const POLICY_COMMITMENT: [u8; 32] = [0; 32];
pub const REPLACEMENTS: &[([u8; 32], [u8; 32])] = &[];
