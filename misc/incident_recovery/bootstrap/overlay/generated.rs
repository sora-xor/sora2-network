//! Unbound source template. The release compiler replaces this file in an
//! isolated export; zero/empty inputs can replay history but cannot recover.
pub const POLICY_SCALE: &[u8] = &[];
pub const POLICY_COMMITMENT: [u8; 32] = [0; 32];
pub const OPERATORS_SCALE: &[u8] = &[];
pub const PREVIOUS_GRANDPA_SET: u64 = 0;
pub const FIRST_RECOVERY_SLOT: u64 = 0;
pub const FINAL_CODE_HASH: [u8; 32] = [0; 32];
pub const FINAL_CODE_LEN: u32 = 0;
pub const HOLD_CODE_HASH: [u8; 32] = [0; 32];
pub const HOLD_CODE_LEN: u32 = 0;
pub const FINAL_CHUNK_HASHES: &[[u8; 32]] = &[];
pub const HOLD_CHUNK_HASHES: &[[u8; 32]] = &[];
pub const HOLD_WASM: &[u8] = &[];
pub const ORIGINAL_POLICY_SCALE: &[u8] = POLICY_SCALE;
pub const ORIGINAL_POLICY_COMMITMENT: [u8; 32] = POLICY_COMMITMENT;
