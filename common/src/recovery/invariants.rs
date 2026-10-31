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
        if self.issuance.is_empty() {
            return Err(RecoveryError::ChunkBounds);
        }
        self.validate_page_bounds()
    }

    fn field_count(&self) -> usize {
        self.issuance.len()
            + self.zero_claims.len()
            + self.absent.len()
            + self
                .collateral
                .iter()
                .map(|c| c.retained.len())
                .sum::<usize>()
    }

    fn validate_page_bounds(&self) -> Result<(), RecoveryError> {
        let fields = self.issuance.len()
            + self.zero_claims.len()
            + self.absent.len()
            + self
                .collateral
                .iter()
                .map(|c| c.retained.len())
                .sum::<usize>();
        if fields == 0 || fields > MAX_EDITS_PER_CHUNK || self.encoded_size() > MAX_CHUNK_BYTES {
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
        let mut absent_locations = BTreeSet::new();
        if self.collateral.iter().any(|c| c.retained.is_empty())
            || self.absent.iter().any(|w| {
                w.value_hash.is_some() || w.key.is_empty() || w.key.len() > MAX_STORAGE_KEY_BYTES
                    || matches!(&w.scope, StorageScope::Child(id) if id.is_empty() || id.len() > MAX_STORAGE_KEY_BYTES)
                    || !absent_locations.insert((w.scope.encode(), w.key.clone()))
            })
        {
            return Err(RecoveryError::WrongPreimage);
        }
        Ok(())
    }

    fn check(&self) -> Result<(), RecoveryError> {
        let covered = self.check_page()?;
        issuance::verify_coverage(covered)
    }

    fn check_page(&self) -> Result<u32, RecoveryError> {
        let mut covered = 0u32;
        let mut budget = MAX_CHUNK_BYTES;
        for supply in &self.issuance {
            if issuance::check(supply, &mut budget)? {
                covered += 1;
                continue;
            }
            let before =
                U256::from(supply.checkpoint_issuance) + U256::from(supply.authenticated_mints);
            let actual = if supply.current.scope == StorageScope::Top
                && supply.current.offset == 0
                && supply.current.record_hash == sp_io::hashing::blake2_256(&0u128.encode())
                && !sp_io::storage::exists(&supply.current.key)
            {
                0
            } else {
                supply.current.read(&mut budget)?
            };
            let after = U256::from(actual) + U256::from(supply.burned);
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
        Ok(covered)
    }
}

/// Paged terminal checks retain the same bounded work per block. Their manifest
/// commitment is separate from the single-envelope codec and binds every page.
pub const MAX_TERMINAL_PAGES: usize = 1024;
const PAGE_CURSOR: &[u8] = b":sora-nexus:recovery:terminal-pages:v1";
const PAGE_FIELD_PREFIX: &[u8] = b":sora-nexus:recovery:terminal-page-field:v1:";

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct TerminalAuditPage {
    pub index: u32,
    pub pages: Vec<[u8; 32]>,
    pub audit: TerminalAudit,
}
impl TerminalAuditPage {
    pub fn commitment(&self) -> [u8; 32] {
        sp_io::hashing::blake2_256(
            &(b"sora-nexus/terminal-pages/v1".as_slice(), &self.pages).encode(),
        )
    }
    pub fn validate(&self) -> Result<(), RecoveryError> {
        self.audit.validate_page_bounds()?;
        if self.pages.len() < 2
            || self.pages.len() > MAX_TERMINAL_PAGES
            || self.pages.get(self.index as usize) != Some(&self.audit.commitment())
            || self.encoded_size() > MAX_CHUNK_BYTES
        {
            return Err(RecoveryError::ChunkBounds);
        }
        Ok(())
    }
    pub fn is_last(&self) -> bool {
        self.index as usize + 1 == self.pages.len()
    }
}

/// Deterministically split a full compiler audit without splitting collateral
/// equations. This also validates duplicate assets/positions across all pages.
/// A single collateral equation larger than one block remains unsupported.
pub fn terminal_pages(audit: &TerminalAudit) -> Result<Vec<TerminalAuditPage>, RecoveryError> {
    if audit.issuance.is_empty() || audit.field_count() > MAX_TERMINAL_PAGES * MAX_EDITS_PER_CHUNK {
        return Err(RecoveryError::ChunkBounds);
    }
    // Structural validation uses the same checks as a page, with limits applied
    // separately below; never weaken the runtime's per-envelope bounds.
    let mut assets = BTreeSet::new();
    let mut fields = BTreeSet::new();
    for item in &audit.issuance {
        if !assets.insert(item.asset) {
            return Err(RecoveryError::DuplicateEdit);
        }
    }
    for item in audit
        .issuance
        .iter()
        .map(|i| &i.current)
        .chain(audit.collateral.iter().flat_map(|i| &i.retained))
        .chain(&audit.zero_claims)
    {
        if !fields.insert((item.scope.encode(), item.key.clone(), Some(item.offset))) {
            return Err(RecoveryError::DuplicateEdit);
        }
    }
    for item in &audit.absent {
        if !fields.insert((item.scope.encode(), item.key.clone(), None)) {
            return Err(RecoveryError::DuplicateEdit);
        }
    }
    let empty = || TerminalAudit {
        issuance: vec![],
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![],
    };
    let mut chunks = Vec::new();
    let mut current = empty();
    // Reserve the largest page manifest in every page to avoid a late overflow.
    let fits = |value: &TerminalAudit| {
        value.validate_page_bounds().is_ok()
            && value.encoded_size() + 8 + 32 * MAX_TERMINAL_PAGES <= MAX_CHUNK_BYTES
    };
    let mut push = |item: TerminalAudit| -> Result<(), RecoveryError> {
        let mut probe = current.clone();
        probe.issuance.extend(item.issuance.clone());
        probe.collateral.extend(item.collateral.clone());
        probe.zero_claims.extend(item.zero_claims.clone());
        probe.absent.extend(item.absent.clone());
        if !fits(&probe) {
            if current.field_count() == 0 || !fits(&item) {
                return Err(RecoveryError::ChunkBounds);
            }
            chunks.push(core::mem::replace(&mut current, empty()));
            current = item;
        } else {
            current = probe;
        }
        Ok(())
    };
    for value in &audit.issuance {
        let mut item = empty();
        item.issuance.push(value.clone());
        push(item)?;
    }
    for value in &audit.collateral {
        let mut item = empty();
        item.collateral.push(value.clone());
        push(item)?;
    }
    for value in &audit.zero_claims {
        let mut item = empty();
        item.zero_claims.push(value.clone());
        push(item)?;
    }
    for value in &audit.absent {
        let mut item = empty();
        item.absent.push(value.clone());
        push(item)?;
    }
    chunks.push(current);
    if chunks.len() > MAX_TERMINAL_PAGES {
        return Err(RecoveryError::ChunkBounds);
    }
    let hashes = chunks
        .iter()
        .map(TerminalAudit::commitment)
        .collect::<Vec<_>>();
    Ok(chunks
        .into_iter()
        .enumerate()
        .map(|(index, audit)| TerminalAuditPage {
            index: index as u32,
            pages: hashes.clone(),
            audit,
        })
        .collect())
}

/// Reassemble only the complete canonical compiler page sequence.
pub fn join_terminal_pages(pages: &[TerminalAuditPage]) -> Result<TerminalAudit, RecoveryError> {
    if pages.len() < 2 || pages.len() > MAX_TERMINAL_PAGES {
        return Err(RecoveryError::ChunkBounds);
    }
    let mut audit = TerminalAudit {
        issuance: vec![],
        collateral: vec![],
        zero_claims: vec![],
        absent: vec![],
    };
    for (index, page) in pages.iter().enumerate() {
        page.validate()?;
        if page.index as usize != index
            || page.pages.len() != pages.len()
            || page.pages != pages[0].pages
        {
            return Err(RecoveryError::WrongChunk);
        }
        audit.issuance.extend(page.audit.issuance.clone());
        audit.collateral.extend(page.audit.collateral.clone());
        audit.zero_claims.extend(page.audit.zero_claims.clone());
        audit.absent.extend(page.audit.absent.clone());
    }
    if terminal_pages(&audit)? != pages {
        return Err(RecoveryError::WrongChunk);
    }
    Ok(audit)
}
pub fn terminal_actions(audit: &TerminalAudit) -> Result<Vec<RecoveryAction>, RecoveryError> {
    let pages = terminal_pages(audit)?;
    Ok(if pages.len() == 1 {
        vec![RecoveryAction::TerminalAudit(audit.clone())]
    } else {
        pages
            .into_iter()
            .map(RecoveryAction::TerminalAuditPage)
            .collect()
    })
}

/// Full-audit commitment selected by the deterministic single/page transport.
pub fn terminal_commitment(audit: &TerminalAudit) -> Result<[u8; 32], RecoveryError> {
    let pages = terminal_pages(audit)?;
    Ok(if pages.len() == 1 {
        audit.commitment()
    } else {
        pages[0].commitment()
    })
}

#[derive(Encode, Decode, DecodeWithMemTracking)]
struct PageCursor {
    policy: [u8; 32],
    counter: u64,
    manifest: [u8; 32],
    next: u32,
    issuance: u32,
    journaled: u32,
}
fn field_keys(page: &TerminalAuditPage, policy: &RecoveryPolicy, counter: u64) -> Vec<Vec<u8>> {
    let mut identities = Vec::new();
    for supply in &page.audit.issuance {
        identities.push((0u8, supply.asset).encode());
    }
    for amount in page
        .audit
        .issuance
        .iter()
        .map(|i| &i.current)
        .chain(page.audit.collateral.iter().flat_map(|i| &i.retained))
        .chain(&page.audit.zero_claims)
    {
        identities.push((1u8, &amount.scope, &amount.key, Some(amount.offset)).encode());
    }
    for absent in &page.audit.absent {
        identities.push((1u8, &absent.scope, &absent.key, None::<u32>).encode());
    }
    // Hash the (up to ~1 MB) policy and the page manifest once per page, not per
    // field: the overlay's weight for this action is linear in fields only.
    let policy = policy.commitment();
    let manifest = page.commitment();
    identities
        .into_iter()
        .map(|id| {
            let mut key = PAGE_FIELD_PREFIX.to_vec();
            key.extend(sp_io::hashing::blake2_256(
                &(policy, counter, manifest, id).encode(),
            ));
            key
        })
        .collect()
}
fn audit_phase_complete(
    policy: &RecoveryPolicy,
    state: &RecoveryStatus,
) -> Result<(), RecoveryError> {
    if state.phase != RecoveryPhase::Auditing
        || state.next_chunk as usize != policy.chunk_hashes.len()
        || sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY).as_deref()
            != Some(&(policy.audit_chunk_hashes.len() as u32).encode()[..])
        || sp_io::storage::get(INCOMING_AUDIT_PENDING_KEY).as_deref() != Some(&0u32.encode()[..])
    {
        return Err(RecoveryError::IncompleteAudit);
    }
    Ok(())
}

/// Each page executes only after all mutation and ordinary audit work. Since
/// maintenance has no economic hooks, already checked pages cannot be invalidated
/// between pages. A forward repair starts a new policy/counter cursor; old page
/// receipts remain retained and cannot advance the revised manifest.
pub fn verify_page_and_record(
    policy: &RecoveryPolicy,
    page: &TerminalAuditPage,
) -> Result<RecoveryStatus, RecoveryError> {
    let mut state = storage::checked_status(policy)?;
    audit_phase_complete(policy, &state)?;
    page.validate()?;
    if page.commitment() != policy.terminal_audit_commitment {
        return Err(RecoveryError::WrongChunk);
    }
    let previous = sp_io::storage::get(PAGE_CURSOR)
        .map(|bytes| {
            let mut input = &bytes[..];
            let value = PageCursor::decode_with_mem_limit(&mut input, 1024)
                .map_err(|_| RecoveryError::IncompleteAudit)?;
            if !input.is_empty() {
                return Err(RecoveryError::IncompleteAudit);
            }
            Ok(value)
        })
        .transpose()?;
    let mut cursor = match previous {
        Some(value)
            if value.policy == policy.commitment() && value.counter == state.operation_counter =>
        {
            value
        }
        _ if page.index == 0 => PageCursor {
            policy: policy.commitment(),
            counter: state.operation_counter,
            manifest: page.commitment(),
            next: 0,
            issuance: 0,
            journaled: 0,
        },
        _ => return Err(RecoveryError::IncompleteAudit),
    };
    if cursor.manifest != page.commitment() || cursor.next != page.index {
        return Err(RecoveryError::WrongChunk);
    }
    let keys = field_keys(page, policy, state.operation_counter);
    if keys.iter().any(|key| sp_io::storage::exists(key)) {
        return Err(RecoveryError::DuplicateEdit);
    }
    let covered = page
        .audit
        .check_page()
        .map_err(|e| storage::freeze(state.clone(), e))?;
    cursor.journaled += covered;
    cursor.next += 1;
    cursor.issuance += page.audit.issuance.len() as u32;
    if page.is_last() && cursor.issuance == 0 {
        return Err(RecoveryError::IncompleteAudit);
    }
    if page.is_last() {
        issuance::verify_coverage(cursor.journaled)
            .map_err(|e| storage::freeze(state.clone(), e))?;
    }
    for key in keys {
        sp_io::storage::set(&key, &[1]);
    }
    sp_io::storage::set(PAGE_CURSOR, &cursor.encode());
    if page.is_last() {
        state.phase = RecoveryPhase::Ready;
        sp_io::storage::set(
            TERMINAL_AUDIT_STORAGE_KEY,
            &(policy.commitment(), page.commitment(), state.next_chunk).encode(),
        );
        storage::put_status(&state);
    }
    Ok(state)
}

/// Recheck the committed terminal equations against the freshly executed final
/// state. Host-only: this cannot change Wasm behavior or manufacture a terminal
/// receipt. Callers must obtain the state from an in-process exact-artifact run.
#[cfg(feature = "std")]
pub fn verify_completed_read_only(
    policy: &RecoveryPolicy,
    audit: &TerminalAudit,
) -> Result<(), RecoveryError> {
    policy.validate()?;
    let state = storage::checked_status(policy)?;
    let pages = terminal_pages(audit)?;
    let commitment = terminal_commitment(audit)?;
    if state.phase != RecoveryPhase::Complete
        || state.failure.is_some()
        || state.next_chunk as usize != policy.chunk_hashes.len()
        || commitment != policy.terminal_audit_commitment
        || sp_io::storage::get(AUDIT_CURSOR_STORAGE_KEY).as_deref()
            != Some(&(policy.audit_chunk_hashes.len() as u32).encode()[..])
        || sp_io::storage::get(INCOMING_AUDIT_PENDING_KEY).as_deref() != Some(&0u32.encode()[..])
        || sp_io::storage::get(TERMINAL_AUDIT_STORAGE_KEY).as_deref()
            != Some(&(policy.commitment(), commitment, state.next_chunk).encode()[..])
    {
        return Err(RecoveryError::IncompleteAudit);
    }
    if pages.len() > 1 {
        let bytes = sp_io::storage::get(PAGE_CURSOR).ok_or(RecoveryError::IncompleteAudit)?;
        let mut input = &bytes[..];
        let cursor = PageCursor::decode_with_mem_limit(&mut input, 1024)
            .map_err(|_| RecoveryError::IncompleteAudit)?;
        if !input.is_empty()
            || cursor.policy != policy.commitment()
            || cursor.counter != state.operation_counter
            || cursor.manifest != commitment
            || cursor.next as usize != pages.len()
            || cursor.issuance as usize != audit.issuance.len()
        {
            return Err(RecoveryError::IncompleteAudit);
        }
    }
    let mut covered = 0;
    for page in pages {
        covered += page.audit.check_page()?;
    }
    issuance::verify_coverage(covered)
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
    fn large_terminal_audit_is_paged_without_skips_replay_or_early_activation() {
        sp_io::TestExternalities::default().execute_with(|| {
            let audit = TerminalAudit {
                issuance: (0..257u32)
                    .map(|n| IssuanceInvariant {
                        asset: sp_io::hashing::blake2_256(&n.encode()),
                        current: amount(&n.encode(), u128::from(n)),
                        checkpoint_issuance: u128::from(n),
                        authenticated_mints: 0,
                        burned: 0,
                    })
                    .collect(),
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            };
            assert_eq!(audit.validate_bounds(), Err(RecoveryError::ChunkBounds));
            let pages = terminal_pages(&audit).unwrap();
            assert_eq!(pages.len(), 3);
            assert_eq!(join_terminal_pages(&pages).unwrap(), audit);
            let policy = RecoveryPolicy {
                format_version: 2,
                checkpoint_hash: CHECKPOINT_HASH,
                checkpoint_state_root: CHECKPOINT_STATE_ROOT,
                replacements: vec![],
                council: vec![AccountId32::new([8; 32])],
                chunk_hashes: vec![[2; 32]],
                audit_chunk_hashes: vec![[3; 32]],
                evidence_commitment: [4; 32],
                terminal_audit_commitment: terminal_commitment(&audit).unwrap(),
                authorization_domain: RecoveryDomain::public_fixture(),
            };
            let mut state = storage::initialize(&policy).unwrap();
            state.phase = RecoveryPhase::Auditing;
            state.next_chunk = 1;
            storage::put_status(&state);
            sp_io::storage::set(AUDIT_CURSOR_STORAGE_KEY, &1u32.encode());
            assert!(verify_page_and_record(&policy, &pages[1]).is_err());
            assert!(verify_page_and_record(&policy, &pages[0]).is_ok());
            assert!(storage::mark_complete(&policy).is_err());
            assert!(verify_page_and_record(&policy, &pages[0]).is_err());
            assert!(verify_page_and_record(&policy, &pages[2]).is_err());
            // Persisted cursor, not process memory, determines restart continuation.
            verify_page_and_record(&policy, &pages[1]).unwrap();
            assert_eq!(
                verify_page_and_record(&policy, &pages[2]).unwrap().phase,
                RecoveryPhase::Ready
            );
            storage::mark_complete(&policy).unwrap();
            verify_completed_read_only(&policy, &audit).unwrap();
            sp_io::storage::set(&0u32.encode(), &999u128.encode());
            assert!(verify_completed_read_only(&policy, &audit).is_err());
        });
    }

    #[test]
    fn paged_terminal_repair_reaudits_pages_without_resetting_native_issuance() {
        use sp_core::{sr25519, Pair};

        sp_io::TestExternalities::default().execute_with(|| {
            let audit = TerminalAudit {
                issuance: (0..129u32)
                    .map(|n| IssuanceInvariant {
                        asset: sp_io::hashing::blake2_256(&n.encode()),
                        current: amount(&n.encode(), 100),
                        checkpoint_issuance: 100,
                        authenticated_mints: 0,
                        burned: 0,
                    })
                    .collect(),
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            };
            let pages = terminal_pages(&audit).unwrap();
            assert_eq!(pages.len(), 2);
            let journaled = [0usize, 128]; // One native supply on each page.
            let static_audit = RecoveryAuditChunk {
                index: 0,
                witnesses: journaled
                    .iter()
                    .map(|&i| StorageWitness {
                        scope: StorageScope::Top,
                        key: audit.issuance[i].current.key.clone(),
                        value_hash: Some(audit.issuance[i].current.record_hash),
                    })
                    .collect(),
            };
            let signer = sr25519::Pair::from_seed(&[42; 32]);
            let account = AccountId32::from(signer.public().0);
            let policy = RecoveryPolicy {
                format_version: 2,
                checkpoint_hash: CHECKPOINT_HASH,
                checkpoint_state_root: CHECKPOINT_STATE_ROOT,
                replacements: vec![],
                council: vec![account.clone()],
                chunk_hashes: vec![[2; 32]],
                audit_chunk_hashes: vec![static_audit.commitment()],
                evidence_commitment: [4; 32],
                terminal_audit_commitment: terminal_commitment(&audit).unwrap(),
                authorization_domain: RecoveryDomain::public_fixture(),
            };
            // Model the native adapter's transactional before/after supply
            // observations; its runtime wiring is covered by the incoming test.
            let consume = |active: &RecoveryPolicy, minted: u128, burned: u128| {
                let rows = journaled
                    .iter()
                    .map(|&i| {
                        (
                            audit.issuance[i].asset,
                            audit.issuance[i].current.key.clone(),
                        )
                    })
                    .collect();
                let prepared = issuance::prepare(active, 0, rows).unwrap();
                sp_io::storage::set(&audit.issuance[0].current.key, &minted.encode());
                sp_io::storage::set(&audit.issuance[128].current.key, &burned.encode());
                issuance::record_consumption(active, prepared).unwrap();
                let mut state = storage::checked_status(active).unwrap();
                state.next_chunk = 1;
                state.phase = RecoveryPhase::Auditing;
                storage::put_status(&state);
                storage::audit_chunk(active, &static_audit).unwrap();
            };
            storage::initialize(&policy).unwrap();
            storage::record_recovery_anchor([0x42; 32]).unwrap();
            consume(&policy, 105, 97);
            verify_page_and_record(&policy, &pages[0]).unwrap();
            assert!(storage::mark_complete(&policy).is_err());

            storage::record_hold(storage::status().unwrap().unwrap()).unwrap();
            let state = storage::status().unwrap().unwrap();
            let mut revised = policy.clone();
            revised.evidence_commitment = [9; 32];
            revised.chunk_hashes = vec![[10; 32]];
            let authorization = RepairAuthorization {
                policy: policy.commitment(),
                phase: state.phase,
                cursor: state.next_chunk,
                operation_counter: state.operation_counter,
                current_code_hash: [11; 32],
                current_executor_hash: [11; 32],
                destination_code_hash: [12; 32],
                destination_spec: 136,
                revised_policy: revised.commitment(),
                authorization_domain: policy.authorization_domain.clone(),
                recovery_anchor: [0x42; 32],
            };
            let signatures = vec![(
                account,
                MultiSignature::from(signer.sign(&authorization.signing_message())),
            )];
            storage::install_work_policy(&policy, &revised, &authorization, &signatures).unwrap();
            consume(&revised, 112, 95);
            assert_eq!(
                storage::status().unwrap().unwrap().policy,
                policy.commitment()
            );
            assert_eq!(storage::status().unwrap().unwrap().operation_counter, 2);
            assert_eq!(
                verify_page_and_record(&policy, &pages[1]),
                Err(RecoveryError::WrongPolicy)
            );
            assert_eq!(
                verify_page_and_record(&revised, &pages[1]),
                Err(RecoveryError::IncompleteAudit)
            );
            assert!(storage::mark_complete(&revised).is_err());
            // Neither repair nor a new operation may adopt the current value
            // as a fresh baseline and forget earlier native mints/burns.
            let mut rebased = audit.issuance[0].clone();
            rebased.current.record_hash = sp_io::hashing::blake2_256(&112u128.encode());
            rebased.checkpoint_issuance = 112;
            let mut budget = MAX_CHUNK_BYTES;
            assert_eq!(
                issuance::check(&rebased, &mut budget),
                Err(RecoveryError::WrongPreimage)
            );
            verify_page_and_record(&revised, &pages[0]).unwrap();
            assert_eq!(
                verify_page_and_record(&revised, &pages[0]),
                Err(RecoveryError::WrongChunk)
            );
            assert_eq!(
                verify_page_and_record(&revised, &pages[1]).unwrap().phase,
                RecoveryPhase::Ready
            );
            storage::mark_complete(&revised).unwrap();
            verify_completed_read_only(&revised, &audit).unwrap();
        });
    }

    #[test]
    fn terminal_pages_reject_duplicate_assets_and_unsplittable_collateral() {
        sp_io::TestExternalities::default().execute_with(|| {
            let supply = IssuanceInvariant {
                asset: [1; 32],
                current: amount(b"s", 10),
                checkpoint_issuance: 10,
                authenticated_mints: 0,
                burned: 0,
            };
            let mut audit = TerminalAudit {
                issuance: vec![supply.clone(), supply],
                collateral: vec![],
                zero_claims: vec![],
                absent: vec![],
            };
            assert_eq!(terminal_pages(&audit), Err(RecoveryError::DuplicateEdit));
            audit.issuance.pop();
            audit.collateral.push(CollateralInvariant {
                asset: [2; 32],
                minimum: 0,
                retained: (0..129u32).map(|n| amount(&n.encode(), 1)).collect(),
            });
            assert_eq!(terminal_pages(&audit), Err(RecoveryError::ChunkBounds));
        });
    }

    #[test]
    fn completed_recheck_is_read_only_and_requires_the_exact_receipt_and_finances() {
        sp_io::TestExternalities::default().execute_with(|| {
            let audit = TerminalAudit {
                issuance: vec![IssuanceInvariant {
                    asset: [1; 32],
                    current: amount(b"issuance", 90),
                    checkpoint_issuance: 100,
                    authenticated_mints: 0,
                    burned: 10,
                }],
                collateral: vec![CollateralInvariant {
                    asset: [1; 32],
                    retained: vec![amount(b"custody", 30)],
                    minimum: 30,
                }],
                zero_claims: vec![amount(b"claim", 0)],
                absent: vec![StorageWitness {
                    scope: StorageScope::Top,
                    key: b"retired".to_vec(),
                    value_hash: None,
                }],
            };
            let policy = RecoveryPolicy {
                format_version: 2,
                authorization_domain: crate::recovery::RecoveryDomain::public_fixture(),
                checkpoint_hash: CHECKPOINT_HASH,
                checkpoint_state_root: CHECKPOINT_STATE_ROOT,
                replacements: vec![],
                council: vec![AccountId32::from([8; 32])],
                chunk_hashes: vec![[2; 32]],
                audit_chunk_hashes: vec![[3; 32]],
                evidence_commitment: [4; 32],
                terminal_audit_commitment: audit.commitment(),
            };
            let mut state = storage::initialize(&policy).unwrap();
            state.phase = RecoveryPhase::Complete;
            state.next_chunk = 1;
            storage::put_status(&state);
            sp_io::storage::set(AUDIT_CURSOR_STORAGE_KEY, &1u32.encode());
            sp_io::storage::set(
                TERMINAL_AUDIT_STORAGE_KEY,
                &(policy.commitment(), audit.commitment(), 1u32).encode(),
            );
            let before = sp_io::storage::root(sp_core::storage::StateVersion::V0);
            verify_completed_read_only(&policy, &audit).unwrap();
            assert_eq!(
                before,
                sp_io::storage::root(sp_core::storage::StateVersion::V0)
            );
            for key in [b"issuance".as_slice(), b"custody", b"claim", b"retired"] {
                let old = sp_io::storage::get(key);
                sp_io::storage::set(key, &1u128.encode());
                assert!(verify_completed_read_only(&policy, &audit).is_err());
                match old {
                    Some(v) => sp_io::storage::set(key, &v),
                    None => sp_io::storage::clear(key),
                }
            }
            // Completed work count must not be confused with audit cursor.
            sp_io::storage::set(
                TERMINAL_AUDIT_STORAGE_KEY,
                &(policy.commitment(), audit.commitment(), 2u32).encode(),
            );
            assert!(verify_completed_read_only(&policy, &audit).is_err());
        });
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
