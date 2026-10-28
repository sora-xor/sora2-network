//! Bounded activation checks over exact, compiler-typed storage projections.
//!
//! A projection pins the hash of the complete SCALE value, not merely a byte
//! offset. The offline compiler must derive the field from the checkpoint type
//! registry and prove complete asset/ownership coverage. These checks enforce
//! the committed arithmetic on-chain; they do not invent ownership or prices.

use super::*;
use sp_core::U256;

/// A non-compact SCALE u128 field in an exactly committed typed record.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct StoredAmount {
    pub scope: StorageScope,
    pub key: Vec<u8>,
    pub record_hash: [u8; 32],
    pub offset: u32,
}

impl StoredAmount {
    fn read(&self, budget: &mut usize) -> Result<u128, RecoveryError> {
        let value = storage::read_bounded(&self.scope, &self.key, budget)?
            .ok_or(RecoveryError::WrongPreimage)?;
        if value.len() > MAX_CHUNK_BYTES || sp_io::hashing::blake2_256(&value) != self.record_hash {
            return Err(RecoveryError::WrongPreimage);
        }
        let start = self.offset as usize;
        let end = start.checked_add(16).ok_or(RecoveryError::ChunkBounds)?;
        let bytes = value.get(start..end).ok_or(RecoveryError::WrongPreimage)?;
        Ok(u128::from_le_bytes(
            bytes.try_into().map_err(|_| RecoveryError::WrongPreimage)?,
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct IssuanceInvariant {
    pub asset: [u8; 32],
    pub current: StoredAmount,
    pub checkpoint_issuance: u128,
    pub authenticated_mints: u128,
    pub burned: u128,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct CollateralInvariant {
    pub asset: [u8; 32],
    /// Individually guarded custody values. Shared positions are not duplicated.
    pub retained: Vec<StoredAmount>,
    /// Required backing from the typed, reviewed debt settlement inventory.
    pub minimum: u128,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct TerminalAudit {
    pub issuance: Vec<IssuanceInvariant>,
    pub collateral: Vec<CollateralInvariant>,
    pub zero_claims: Vec<StoredAmount>,
    /// Retired ownership/authorization rows that must no longer exist.
    pub absent: Vec<StorageWitness>,
}

impl TerminalAudit {
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(&self.encode())
    }

    pub fn validate_bounds(&self) -> Result<(), RecoveryError> {
        let fields = self.issuance.len()
            + self.zero_claims.len()
            + self.absent.len()
            + self
                .collateral
                .iter()
                .map(|c| c.retained.len())
                .sum::<usize>();
        if self.issuance.is_empty()
            || fields > MAX_EDITS_PER_CHUNK
            || self.encoded_size() > MAX_CHUNK_BYTES
        {
            return Err(RecoveryError::ChunkBounds);
        }
        let mut assets = BTreeSet::new();
        for supply in &self.issuance {
            if !assets.insert(supply.asset) {
                return Err(RecoveryError::DuplicateEdit);
            }
        }
        let mut positions = BTreeSet::new();
        for amount in self
            .issuance
            .iter()
            .map(|s| &s.current)
            .chain(self.collateral.iter().flat_map(|c| &c.retained))
            .chain(&self.zero_claims)
        {
            let scope = match &amount.scope {
                StorageScope::Top => None,
                StorageScope::Child(id) if !id.is_empty() && id.len() <= MAX_STORAGE_KEY_BYTES => {
                    Some(id.clone())
                }
                _ => return Err(RecoveryError::ChunkBounds),
            };
            if amount.key.is_empty()
                || amount.key.len() > MAX_STORAGE_KEY_BYTES
                || !positions.insert((scope, amount.key.clone(), amount.offset))
            {
                return Err(RecoveryError::DuplicateEdit);
            }
        }
        if self.collateral.iter().any(|c| c.retained.is_empty())
            || self.absent.iter().any(|w| {
                w.value_hash.is_some() || w.key.is_empty() || w.key.len() > MAX_STORAGE_KEY_BYTES
            })
        {
            return Err(RecoveryError::WrongPreimage);
        }
        Ok(())
    }

    fn check(&self) -> Result<(), RecoveryError> {
        let mut budget = MAX_CHUNK_BYTES;
        for supply in &self.issuance {
            let before =
                U256::from(supply.checkpoint_issuance) + U256::from(supply.authenticated_mints);
            let after = U256::from(supply.current.read(&mut budget)?) + U256::from(supply.burned);
            if before != after {
                return Err(RecoveryError::WrongPreimage);
            }
        }
        for custody in &self.collateral {
            let mut total = U256::zero();
            for amount in &custody.retained {
                total = total
                    .checked_add(U256::from(amount.read(&mut budget)?))
                    .ok_or(RecoveryError::ChunkBounds)?;
            }
            if total < U256::from(custody.minimum) {
                return Err(RecoveryError::WrongPreimage);
            }
        }
        for claim in &self.zero_claims {
            if claim.read(&mut budget)? != 0 {
                return Err(RecoveryError::WrongPreimage);
            }
        }
        for witness in &self.absent {
            if storage::read_bounded(&witness.scope, &witness.key, &mut budget)?.is_some() {
                return Err(RecoveryError::WrongPreimage);
            }
        }
        if !sp_io::storage::exists(BRIDGE_HALT_STORAGE_KEY)
            || TRANSIENT_CONTEXT_KEYS
                .iter()
                .any(|key| sp_io::storage::exists(key))
        {
            return Err(RecoveryError::WrongPreimage);
        }
        Ok(())
    }
}

/// Last activation gate. A committed failure freezes work but leaves council
/// escape available. An uncommitted proposal cannot freeze the chain.
pub fn verify_and_record(
    policy: &RecoveryPolicy,
    audit: &TerminalAudit,
) -> Result<RecoveryStatus, RecoveryError> {
    let mut state = storage::checked_status(policy)?;
    if state.phase != RecoveryPhase::Auditing {
        return Err(RecoveryError::WrongPhase);
    }
    audit.validate_bounds()?;
    if audit.commitment() != policy.terminal_audit_commitment {
        return Err(RecoveryError::WrongChunk);
    }
    let bytes =
        sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY).ok_or(RecoveryError::IncompleteAudit)?;
    let mut input = &bytes[..];
    let cursor = u32::decode(&mut input).map_err(|_| RecoveryError::IncompleteAudit)?;
    if !input.is_empty()
        || cursor as usize != policy.audit_chunk_hashes.len()
        || state.next_chunk as usize != policy.chunk_hashes.len()
    {
        return Err(RecoveryError::IncompleteAudit);
    }
    // Missing is deliberately not zero: legacy v1-only receipts cannot gain
    // retrospective execution-block evidence merely by installing a repair.
    if sp_io::storage::get(INCOMING_AUDIT_PENDING_KEY).as_deref() != Some(&0u32.encode()[..]) {
        return Err(RecoveryError::IncompleteAudit);
    }
    audit
        .check()
        .map_err(|error| storage::freeze(state.clone(), error))?;
    state.phase = RecoveryPhase::Ready;
    sp_io::storage::set(
        TERMINAL_AUDIT_STORAGE_KEY,
        &(policy.commitment(), audit.commitment(), state.next_chunk).encode(),
    );
    storage::put_status(&state);
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(key: &[u8], value: u128) -> StoredAmount {
        sp_io::storage::set(key, &value.encode());
        StoredAmount {
            scope: StorageScope::Top,
            key: key.to_vec(),
            record_hash: sp_io::hashing::blake2_256(&value.encode()),
            offset: 0,
        }
    }

    #[test]
    fn issuance_reduction_and_custody_are_checked_without_u128_sum_overflow() {
        sp_io::TestExternalities::default().execute_with(|| {
            sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
            let mut audit = TerminalAudit {
                issuance: vec![IssuanceInvariant {
                    asset: [1; 32],
                    current: amount(b"issuance", u128::MAX),
                    checkpoint_issuance: u128::MAX,
                    authenticated_mints: 100,
                    burned: 100,
                }],
                collateral: vec![CollateralInvariant {
                    asset: [1; 32],
                    retained: vec![amount(b"custody", 90)],
                    minimum: 90,
                }],
                zero_claims: vec![amount(b"claim", 0)],
                absent: vec![],
            };
            audit.validate_bounds().unwrap();
            audit.check().unwrap();
            audit.issuance[0].burned = 99;
            assert_eq!(audit.check(), Err(RecoveryError::WrongPreimage));
            audit.issuance[0].burned = 100;
            audit.collateral[0].minimum = 91;
            assert_eq!(audit.check(), Err(RecoveryError::WrongPreimage));
        });
    }

    #[test]
    fn whole_record_guard_rejects_changed_fields_and_transient_context() {
        sp_io::TestExternalities::default().execute_with(|| {
            sp_io::storage::set(BRIDGE_HALT_STORAGE_KEY, &[1]);
            let audit = TerminalAudit {
                issuance: vec![IssuanceInvariant {
                    asset: [1; 32],
                    current: amount(b"issuance", 10),
                    checkpoint_issuance: 10,
                    authenticated_mints: 0,
                    burned: 0,
                }],
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            };
            sp_io::storage::set(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY, &1u32.encode());
            assert_eq!(audit.check(), Err(RecoveryError::WrongPreimage));
            sp_io::storage::clear(BRIDGE_INBOUND_CONTEXT_STORAGE_KEY);
            let mut changed = 10u128.encode();
            changed.push(0);
            sp_io::storage::set(b"issuance", &changed);
            assert_eq!(audit.check(), Err(RecoveryError::WrongPreimage));
        });
    }
}
