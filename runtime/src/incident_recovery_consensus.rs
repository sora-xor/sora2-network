//! Consensus ownership retained across the SORA Nexus frozen recovery session.
//!
//! This module does not run on ordinary startup. The legacy bootstrap invokes
//! installation once in its first child, with a checkpoint-derived key inventory.
//! Runtime 138 retains the proof adapter after ordinary sessions resume.

use crate::fg_primitives as sp_consensus_grandpa;
use alloc::{collections::BTreeSet, vec, vec::Vec};
use codec::{Decode, DecodeAll, DecodeWithMemLimit, DecodeWithMemTracking, Encode};
use common::recovery::{CHECKPOINT_HASH, CHECKPOINT_NUMBER, POLICY_STORAGE_KEY};
use frame_support::traits::{KeyOwnerProofSystem, OnFinalize, OnInitialize};
use sp_core::{
    crypto::{ByteArray, KeyTypeId},
    H256,
};
use sp_runtime::traits::{Convert, OpaqueKeys};
use sp_session::MembershipProof;

use crate::{AccountId, Babe, Balance, GrandpaId, Historical, Runtime, Session, System};

type Identification = (AccountId, pallet_staking::Exposure<AccountId, Balance>);
const OWNER_PREFIX: &[u8] = b":sora-nexus:recovery:frozen-owner:v1:";
const BABE_PREFIX: &[u8] = b":sora-nexus:recovery:babe-owner:v1:";
const GRANDPA_PREFIX: &[u8] = b":sora-nexus:recovery:retained-grandpa-owner:v2:";
const PROOF_MAGIC: &[u8] = b"soranx-grandpa-proof-v1";
const MAX_RECORD_BYTES: usize = 2 * 1024 * 1024;
/// Finalized abandoned-chain set at the recovery checkpoint. Keys are retained,
/// so historical votes from this set must never become recovery slash evidence.
pub const LAST_ABANDONED_GRANDPA_SET: u64 = 7956;

fn grandpa_report_allowed(set: u64) -> bool {
    !sp_io::storage::exists(POLICY_STORAGE_KEY) || set > LAST_ABANDONED_GRANDPA_SET
}

type GrandpaEvidence = (
    sp_consensus_grandpa::EquivocationProof<H256, u32>,
    MembershipProof,
);
type NativeGrandpaReports = pallet_grandpa::EquivocationReportSystem<
    Runtime,
    crate::liveness::EquivocationReports,
    RecoveryOwnership,
    crate::ReportLongevity,
>;

/// Guard every report entry point, including signed dispatch. Proof generation
/// alone cannot protect against a caller supplying a preserved historical proof.
pub struct RecoveryGrandpaReports;
impl sp_staking::offence::OffenceReportSystem<Option<AccountId>, GrandpaEvidence>
    for RecoveryGrandpaReports
{
    type Longevity = crate::ReportLongevity;

    fn publish_evidence(evidence: GrandpaEvidence) -> Result<(), ()> {
        if !grandpa_report_allowed(evidence.0.set_id()) {
            return Err(());
        }
        NativeGrandpaReports::publish_evidence(evidence)
    }

    fn check_evidence(
        evidence: GrandpaEvidence,
    ) -> Result<(), sp_runtime::transaction_validity::TransactionValidityError> {
        if !grandpa_report_allowed(evidence.0.set_id()) {
            return Err(sp_runtime::transaction_validity::InvalidTransaction::BadProof.into());
        }
        NativeGrandpaReports::check_evidence(evidence)
    }

    fn process_evidence(
        reporter: Option<AccountId>,
        evidence: GrandpaEvidence,
    ) -> Result<(), sp_runtime::DispatchError> {
        if !grandpa_report_allowed(evidence.0.set_id()) {
            return Err(pallet_grandpa::Error::<Runtime>::InvalidEquivocationProof.into());
        }
        NativeGrandpaReports::process_evidence(reporter, evidence)
    }
}

#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking)]
struct OwnerRecord {
    policy: [u8; 32],
    session: u32,
    count: u32,
    root: H256,
    index: u32,
    identification: Identification,
    proof: Vec<Vec<u8>>,
}

/// Public inventory compiled into the bootstrap; contains no signing material.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking)]
pub struct CheckpointOperator {
    pub owner: AccountId,
    /// Exact, sorted union of this owner's active, queued and NextKeys keys.
    /// These can differ when a key change was announced before the checkpoint.
    pub grandpa_keys: frame_support::BoundedVec<GrandpaId, frame_support::traits::ConstU32<3>>,
    /// Exact sorted union of retained active, queued and NextKeys signing keys.
    pub babe_keys: frame_support::BoundedVec<
        sp_consensus_babe::AuthorityId,
        frame_support::traits::ConstU32<3>,
    >,
    pub im_online_keys:
        frame_support::BoundedVec<crate::ImOnlineId, frame_support::traits::ConstU32<3>>,
}

fn owner_key(prefix: &[u8], key_type: KeyTypeId, key: &[u8]) -> Vec<u8> {
    let mut storage_key = prefix.to_vec();
    storage_key.extend((key_type, key).encode());
    storage_key
}

fn load(prefix: &[u8], key_type: KeyTypeId, key: &[u8]) -> Option<OwnerRecord> {
    let bytes = sp_io::storage::get(&owner_key(prefix, key_type, key))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return None;
    }
    let mut input = bytes.as_ref();
    let record = OwnerRecord::decode_with_mem_limit(&mut input, MAX_RECORD_BYTES * 2).ok()?;
    if !input.is_empty()
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(record.policy.as_slice())
    {
        return None;
    }
    Some(record)
}

fn babe_prefix(count: u32) -> Vec<u8> {
    let mut prefix = BABE_PREFIX.to_vec();
    prefix.extend(count.encode());
    prefix
}

fn certificate(record: &OwnerRecord) -> Vec<u8> {
    let mut bytes = PROOF_MAGIC.to_vec();
    bytes.extend(sp_io::hashing::blake2_256(&record.encode()));
    bytes
}

/// Keep the deployed proof type and all historical roots. The recovery-only
/// certificate proves a retained active key from authenticated runtime state; equivocation
/// signatures remain checked by GRANDPA, whose range hook confines this proof
/// to S+1 in the frozen session.
pub struct RecoveryOwnership;

impl<D: AsRef<[u8]>> KeyOwnerProofSystem<(KeyTypeId, D)> for RecoveryOwnership {
    type Proof = MembershipProof;
    type IdentificationTuple = Identification;

    fn prove(key: (KeyTypeId, D)) -> Option<Self::Proof> {
        if key.0 == sp_consensus_grandpa::KEY_TYPE {
            if let Some(record) = load(GRANDPA_PREFIX, key.0, key.1.as_ref()) {
                return Some(MembershipProof {
                    session: record.session,
                    validator_count: record.count,
                    trie_nodes: vec![certificate(&record)],
                });
            }
        }
        if let Some(record) = load(OWNER_PREFIX, key.0, key.1.as_ref()) {
            if record.session == Session::current_index() && !record.proof.is_empty() {
                return Some(MembershipProof {
                    session: record.session,
                    validator_count: record.count,
                    trie_nodes: record.proof,
                });
            }
        }
        Historical::prove(key)
    }

    fn check_proof(key: (KeyTypeId, D), proof: Self::Proof) -> Option<Identification> {
        if key.0 == sp_consensus_babe::KEY_TYPE && proof.validator_count <= 1000 {
            if let Some(record) = load(&babe_prefix(proof.validator_count), key.0, key.1.as_ref()) {
                if proof.session == record.session && proof.trie_nodes == vec![certificate(&record)]
                {
                    return effective_identification(record.identification);
                }
            }
        }
        if key.0 == sp_consensus_grandpa::KEY_TYPE {
            if let Some(record) = load(GRANDPA_PREFIX, key.0, key.1.as_ref()) {
                if proof.session == record.session
                    && proof.validator_count == record.count
                    && proof.trie_nodes == vec![certificate(&record)]
                {
                    return effective_identification(record.identification);
                }
            }
        }
        if let Some(record) = load(OWNER_PREFIX, key.0, key.1.as_ref()) {
            if proof.session == record.session && proof.validator_count == record.count {
                if let Some(identification) =
                    check_preserved_proof(&record, key.0, key.1.as_ref(), &proof)
                {
                    return effective_identification(identification);
                }
            }
        }
        // Session::check_proof regenerates the current session's trie from
        // NextKeys. Keep accepting its independently stored historical root
        // during the frozen session, even if a queued key change means that
        // root differs from the checkpoint's regenerated NextKeys root.
        if pallet_grandpa::recovery::load().is_some_and(|set| set.session == proof.session) {
            if let Some((root, count)) =
                pallet_session::historical::HistoricalSessions::<Runtime>::get(proof.session)
            {
                if count == proof.validator_count {
                    if let Some((_, identification)) =
                        check_root_proof(root, key.0, key.1.as_ref(), &proof)
                    {
                        return effective_identification(identification);
                    }
                }
            }
        }
        Historical::check_proof(key, proof).and_then(effective_identification)
    }
}

fn effective_identification(mut value: Identification) -> Option<Identification> {
    if !sp_io::storage::exists(POLICY_STORAGE_KEY) {
        return Some(value);
    }
    let policy = crate::incident_recovery::compiled_policy().ok()?;
    let map = |account: &AccountId| {
        policy
            .replacements
            .iter()
            .find(|entry| &entry.old == account)
            .map(|entry| entry.new.clone())
            .unwrap_or_else(|| account.clone())
    };
    value.0 = map(&value.0);
    for nominator in &mut value.1.others {
        nominator.who = map(&nominator.who);
    }
    Some(value)
}

fn check_preserved_proof(
    record: &OwnerRecord,
    key_type: KeyTypeId,
    key: &[u8],
    proof: &MembershipProof,
) -> Option<Identification> {
    let (index, identification) = check_root_proof(record.root, key_type, key, proof)?;
    (index == record.index && identification == record.identification).then_some(identification)
}

fn check_root_proof(
    root: H256,
    key_type: KeyTypeId,
    key: &[u8],
    proof: &MembershipProof,
) -> Option<(u32, Identification)> {
    // MembershipProof contains raw trie nodes, not a compact inclusion proof.
    // Reject duplicate/unused nodes exactly as pallet-session's verifier does.
    let storage =
        sp_trie::StorageProof::new_with_duplicate_nodes_check(proof.trie_nodes.clone()).ok()?;
    let mut tracker =
        sp_trie::accessed_nodes_tracker::AccessedNodesTracker::<H256>::new(storage.len());
    let db = storage.into_memory_db::<sp_runtime::traits::BlakeTwo256>();
    let index = sp_trie::read_trie_value::<sp_trie::LayoutV0<sp_runtime::traits::BlakeTwo256>, _>(
        &db,
        &root,
        &(key_type, key).encode(),
        Some(&mut tracker),
        None,
    )
    .ok()??;
    let index = u32::decode_all(&mut index.as_slice()).ok()?;
    let identification = sp_trie::read_trie_value::<
        sp_trie::LayoutV0<sp_runtime::traits::BlakeTwo256>,
        _,
    >(&db, &root, &index.encode(), Some(&mut tracker), None)
    .ok()??;
    let identification = Identification::decode_all(&mut identification.as_slice()).ok()?;
    tracker.ensure_no_unused_nodes().ok()?;
    Some((index, identification))
}

/// The API must not return a frozen-session certificate for another GRANDPA set.
pub fn prove_grandpa(set_id: u64, authority: GrandpaId) -> Option<MembershipProof> {
    if !grandpa_report_allowed(set_id) {
        return None;
    }
    if let Some(record) = load(
        GRANDPA_PREFIX,
        sp_consensus_grandpa::KEY_TYPE,
        authority.as_ref(),
    ) {
        let set = pallet_grandpa::recovery::load()?;
        if set.accepts(set_id, record.session, &authority) {
            return RecoveryOwnership::prove((sp_consensus_grandpa::KEY_TYPE, authority));
        }
        // After a normal session rotation the same retained key may legitimately
        // belong to a later set; generate the normal current-session proof.
        if Session::current_index() == record.session {
            return None;
        }
    }
    if let Some(set) = pallet_grandpa::recovery::load() {
        if set_id == set.previous_set {
            if let Some(owner) = load(
                OWNER_PREFIX,
                sp_consensus_grandpa::KEY_TYPE,
                authority.as_slice(),
            ) {
                return Some(MembershipProof {
                    session: owner.session,
                    validator_count: owner.count,
                    trie_nodes: owner.proof,
                });
            }
        }
    }
    Historical::prove((sp_consensus_grandpa::KEY_TYPE, authority))
}

/// Generate maintenance BABE proofs from the preserved root, even after normal
/// sessions have resumed. Other requested epochs retain the upstream API path.
pub fn prove_babe(
    slot: sp_consensus_babe::Slot,
    authority: sp_consensus_babe::AuthorityId,
) -> Option<MembershipProof> {
    if let Some(bytes) = sp_io::storage::get(pallet_babe::recovery::STORAGE_KEY) {
        let epochs = pallet_babe::recovery::RecoveryEpochs::decode_all(&mut bytes.as_ref()).ok()?;
        let epoch = sp_consensus_babe::epoch_index(
            slot,
            pallet_babe::GenesisSlot::<Runtime>::get(),
            crate::EpochDuration::get(),
        );
        if let Some(interval) = epochs
            .intervals
            .iter()
            .find(|interval| (interval.first_epoch..=interval.last_epoch).contains(&epoch))
        {
            if !interval.authorities.contains(&authority) {
                return None;
            }
            let count = interval.authorities.len() as u32;
            let owner = load(
                &babe_prefix(count),
                sp_consensus_babe::KEY_TYPE,
                authority.as_slice(),
            )?;
            if owner.session != interval.session || owner.count != count {
                return None;
            }
            return Some(MembershipProof {
                session: owner.session,
                validator_count: owner.count,
                trie_nodes: vec![certificate(&owner)],
            });
        }
    }
    Historical::prove((sp_consensus_babe::KEY_TYPE, authority))
}

/// Versioned, public data derived from the authenticated checkpoint. No operator
/// approval or new signing key is an input to the restart release.
pub const OPERATOR_MAGIC: &[u8; 8] = b"snxops02";
const KEYS_SNAPSHOT: &[u8] = b":sora-nexus:recovery:checkpoint-session-keys:v2";

pub fn decode_operators(bytes: &[u8]) -> Result<Vec<CheckpointOperator>, &'static str> {
    let mut input = bytes
        .strip_prefix(OPERATOR_MAGIC)
        .ok_or("wrong checkpoint key inventory format")?;
    let operators = Vec::<CheckpointOperator>::decode_with_mem_limit(&mut input, 1024 * 1024)
        .map_err(|_| "invalid checkpoint key inventory")?;
    if !input.is_empty() || operators.len() > 1000 {
        return Err("checkpoint key inventory bounds");
    }
    Ok(operators)
}

pub fn encode_operators(operators: &[CheckpointOperator]) -> Vec<u8> {
    let mut bytes = OPERATOR_MAGIC.to_vec();
    bytes.extend(operators.encode());
    bytes
}

pub struct CheckpointConsensus {
    pub operators: Vec<CheckpointOperator>,
    pub previous_set: u64,
    pub authorities: Vec<(GrandpaId, u64)>,
    pub slot: u64,
    pub council: Vec<AccountId>,
}

/// Native release tooling invokes this only after verifying the checkpoint
/// state root and authenticating its metadata with the deployed Wasm.
pub fn checkpoint_consensus() -> Result<CheckpointConsensus, &'static str> {
    let previous_set = pallet_grandpa::CurrentSetId::<Runtime>::get();
    if previous_set != LAST_ABANDONED_GRANDPA_SET
        || pallet_grandpa::PendingChange::<Runtime>::exists()
    {
        return Err("checkpoint GRANDPA state differs from recovery boundary");
    }
    Ok(CheckpointConsensus {
        operators: checkpoint_operators()?,
        previous_set,
        authorities: pallet_grandpa::Authorities::<Runtime>::get().to_vec(),
        slot: *pallet_babe::CurrentSlot::<Runtime>::get(),
        council: pallet_collective::Members::<Runtime, pallet_collective::Instance1>::get(),
    })
}

/// Derive exact unions from NextKeys and the active/queued consensus rosters.
/// Unknown owners, duplicate keys and missing rows fail before recovery writes.
pub fn checkpoint_operators() -> Result<Vec<CheckpointOperator>, &'static str> {
    type Keys = (
        BTreeSet<GrandpaId>,
        BTreeSet<sp_consensus_babe::AuthorityId>,
        BTreeSet<crate::ImOnlineId>,
    );
    let mut owners = alloc::collections::BTreeMap::<AccountId, Keys>::new();
    for (owner, keys) in pallet_session::NextKeys::<Runtime>::iter() {
        if owners.len() >= 1000 {
            return Err("checkpoint key inventory bounds");
        }
        owners.insert(
            owner,
            (
                BTreeSet::from([keys.grandpa]),
                BTreeSet::from([keys.babe]),
                BTreeSet::from([keys.im_online]),
            ),
        );
    }
    for (owner, keys) in pallet_session::QueuedKeys::<Runtime>::get() {
        let entry = owners
            .get_mut(&owner)
            .ok_or("queued owner has no NextKeys")?;
        entry.0.insert(keys.grandpa);
        entry.1.insert(keys.babe);
        entry.2.insert(keys.im_online);
    }
    let validators = Session::validators();
    let grandpa = pallet_grandpa::Authorities::<Runtime>::get();
    let babe = pallet_babe::Authorities::<Runtime>::get();
    let online = pallet_im_online::Keys::<Runtime>::get();
    if validators.is_empty()
        || validators.len() > 1000
        || validators.len() != grandpa.len()
        || validators.len() != babe.len()
        || validators.len() != online.len()
    {
        return Err("checkpoint active roster bounds");
    }
    let mut seen_owners = BTreeSet::new();
    for (((owner, (grandpa, _)), (babe, _)), online) in
        validators.iter().zip(&grandpa).zip(&babe).zip(&online)
    {
        if !seen_owners.insert(owner) {
            return Err("duplicate checkpoint validator");
        }
        let entry = owners
            .get_mut(owner)
            .ok_or("active owner has no NextKeys")?;
        entry.0.insert(grandpa.clone());
        entry.1.insert(babe.clone());
        entry.2.insert(online.clone());
    }
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();
    for (owner, (grandpa, babe, online)) in owners {
        for (kind, raw) in grandpa
            .iter()
            .map(|key| (sp_consensus_grandpa::KEY_TYPE, key.as_slice()))
            .chain(
                babe.iter()
                    .map(|key| (sp_consensus_babe::KEY_TYPE, key.as_slice())),
            )
            .chain(
                online
                    .iter()
                    .map(|key| (sp_core::crypto::key_types::IM_ONLINE, key.as_slice())),
            )
        {
            if !seen.insert((kind, raw.to_vec()))
                || pallet_session::KeyOwner::<Runtime>::get((kind, raw.to_vec()))
                    .is_some_and(|value| value != owner)
            {
                return Err("ambiguous checkpoint signing-key ownership");
            }
        }
        records.push(CheckpointOperator {
            owner,
            grandpa_keys: grandpa
                .into_iter()
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "too many checkpoint GRANDPA keys")?,
            babe_keys: babe
                .into_iter()
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "too many checkpoint BABE keys")?,
            im_online_keys: online
                .into_iter()
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "too many checkpoint im-online keys")?,
        });
    }
    Ok(records)
}

/// Install the new set identifier while retaining every checkpoint signing key. All fallible validation happens
/// before writes; the enclosing maintenance block is transactional as well.
pub fn install(
    policy: [u8; 32],
    previous_set: u64,
    operators: &[CheckpointOperator],
) -> Result<(), &'static str> {
    if System::block_number() != CHECKPOINT_NUMBER + 1
        || System::parent_hash().as_bytes() != &CHECKPOINT_HASH
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref() != Some(policy.as_slice())
    {
        return Err("wrong first-child consensus context");
    }
    if sp_io::storage::exists(pallet_grandpa::recovery::STORAGE_KEY) {
        return Err("consensus already installed");
    }
    if operators.is_empty() || operators.len() > 1000 {
        return Err("operator inventory bounds");
    }
    if checkpoint_operators()?.as_slice() != operators {
        return Err("key inventory differs from authenticated checkpoint");
    }
    let new_set = previous_set.checked_add(1).ok_or("GRANDPA set overflow")?;
    if previous_set != LAST_ABANDONED_GRANDPA_SET
        || pallet_grandpa::CurrentSetId::<Runtime>::get() != previous_set
        || pallet_grandpa::PendingChange::<Runtime>::exists()
    {
        return Err("unreconciled GRANDPA state");
    }
    let session = Session::current_index();
    let validators = Session::validators();
    let authorities = pallet_grandpa::Authorities::<Runtime>::get().to_vec();
    let queued = pallet_session::QueuedKeys::<Runtime>::get();
    if queued.len() > 1000 || queued.iter().any(|(owner, _)| !validators.contains(owner)) {
        return Err("queued BABE owner is outside the frozen economic session");
    }
    let next_keys = pallet_session::NextKeys::<Runtime>::iter().collect::<Vec<_>>();
    let count = validators.len() as u32;
    let mut records = Vec::new();
    let mut trie = Vec::new();
    for (index, owner) in validators.iter().enumerate() {
        let exposure =
            <Runtime as pallet_session::historical::Config>::FullIdentificationOf::convert(
                owner.clone(),
            )
            .ok_or("missing checkpoint exposure")?;
        let identification = (owner.clone(), exposure);
        let keys = Session::load_keys(owner).ok_or("missing checkpoint session keys")?;
        trie.push(((index as u32).encode(), identification.encode()));
        for key_type in crate::opaque::SessionKeys::key_ids() {
            let key = keys.get_raw(*key_type).to_vec();
            trie.push((
                (*key_type, key.as_slice()).encode(),
                (index as u32).encode(),
            ));
            records.push((
                *key_type,
                key,
                OwnerRecord {
                    policy,
                    session,
                    count,
                    root: H256::zero(),
                    index: index as u32,
                    identification: identification.clone(),
                    proof: Vec::new(),
                },
            ));
        }
    }
    use sp_trie::{recorder_ext::RecorderExt, TrieMut};
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type Layout = sp_trie::LayoutV0<Hashing>;
    // Match MemoryDB's canonical null node without its std-only Default impl.
    let mut proof_db = sp_trie::MemoryDB::<Hashing>::new(&[0]);
    let mut root = H256::zero();
    {
        let mut builder =
            sp_trie::TrieDBMutBuilder::<Layout>::new(&mut proof_db, &mut root).build();
        for (key, value) in &trie {
            builder
                .insert(key, value)
                .map_err(|_| "checkpoint ownership trie failed")?;
        }
    }
    let recovery_set = pallet_grandpa::recovery::RecoverySet {
        checkpoint: CHECKPOINT_HASH,
        policy,
        previous_set,
        session,
        authorities: authorities
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "too many authorities")?,
    };
    let mut writes = Vec::new();
    for (key_type, key, mut record) in records {
        record.root = root;
        let mut recorder = sp_trie::Recorder::<Layout>::new();
        sp_trie::read_trie_value::<Layout, _>(
            &proof_db,
            &root,
            &(key_type, key.as_slice()).encode(),
            Some(&mut recorder),
            None,
        )
        .map_err(|_| "checkpoint ownership proof failed")?;
        sp_trie::read_trie_value::<Layout, _>(
            &proof_db,
            &root,
            &record.index.encode(),
            Some(&mut recorder),
            None,
        )
        .map_err(|_| "checkpoint owner proof failed")?;
        record.proof = recorder.into_raw_storage_proof();
        if record.encoded_size() > MAX_RECORD_BYTES {
            return Err("oversized checkpoint exposure");
        }
        writes.push((owner_key(OWNER_PREFIX, key_type, &key), record.encode()));
        if key_type == sp_consensus_grandpa::KEY_TYPE {
            let active_key = &authorities[record.index as usize].0;
            writes.push((
                owner_key(GRANDPA_PREFIX, key_type, active_key.as_slice()),
                record.encode(),
            ));
        }
    }
    // Keep BABE's already-announced roster changes within the approved frozen
    // economic membership, using the same historical identification as this
    // pinned runtime. Epoch membership/count checks live in pallet-babe.
    let current_babe = pallet_babe::Authorities::<Runtime>::get();
    let next_babe = pallet_babe::NextAuthorities::<Runtime>::get();
    if current_babe.len() != validators.len() || next_babe.len() != queued.len() {
        return Err("BABE roster/session mismatch");
    }
    let mut babe_records = alloc::collections::BTreeMap::new();
    for (authorities, next) in [
        (current_babe.as_slice(), false),
        (next_babe.as_slice(), true),
    ] {
        let count = authorities.len() as u32;
        for (index, (key, _)) in authorities.iter().enumerate() {
            let owner = if next {
                let (owner, keys) = &queued[index];
                if keys.babe != *key {
                    return Err("announced BABE key differs from queued owner");
                }
                owner.clone()
            } else {
                validators[index].clone()
            };
            if !operators.iter().any(|operator| operator.owner == owner) {
                return Err("BABE owner absent from approved inventory");
            }
            let record = OwnerRecord {
                policy,
                session,
                count,
                root: H256::zero(),
                index: index as u32,
                identification: (owner.clone(), Default::default()),
                proof: Vec::new(),
            };
            let storage_key = owner_key(
                &babe_prefix(count),
                sp_consensus_babe::KEY_TYPE,
                key.as_slice(),
            );
            if let Some(previous) = babe_records.get(&storage_key) {
                let previous: &OwnerRecord = previous;
                if previous.identification.0 != owner {
                    return Err("ambiguous BABE key ownership");
                }
            } else {
                babe_records.insert(storage_key, record);
            }
        }
    }
    writes.extend(
        babe_records
            .into_iter()
            .map(|(key, record)| (key, record.encode())),
    );
    for (key, value) in writes {
        sp_io::storage::set(&key, &value);
    }
    sp_io::storage::set(KEYS_SNAPSHOT, &(next_keys, queued, authorities).encode());
    pallet_grandpa::CurrentSetId::<Runtime>::put(new_set);
    pallet_grandpa::SetIdSession::<Runtime>::insert(new_set, session);
    sp_io::storage::set(
        pallet_grandpa::recovery::STORAGE_KEY,
        &recovery_set.encode(),
    );
    Ok(())
}

/// Run BABE alone: preserve already-announced authority/randomness/configuration
/// transitions, without Session/Staking/Authorship economic hooks.
pub fn initialize_maintenance_epoch() -> Result<(), &'static str> {
    let number = System::block_number();
    Babe::on_initialize(number);
    if Babe::should_epoch_change(number) {
        let next = pallet_babe::NextAuthorities::<Runtime>::get();
        Babe::enact_epoch_change(next.clone(), next, Some(Session::current_index()));
    }
    let epoch = pallet_babe::EpochIndex::<Runtime>::get();
    let policy: [u8; 32] = sp_io::storage::get(POLICY_STORAGE_KEY)
        .ok_or("missing recovery policy")?
        .as_ref()
        .try_into()
        .map_err(|_| "invalid recovery policy")?;
    let mut record = match sp_io::storage::get(pallet_babe::recovery::STORAGE_KEY) {
        Some(bytes) => pallet_babe::recovery::RecoveryEpochs::decode_all(&mut bytes.as_ref())
            .map_err(|_| "invalid maintenance epochs")?,
        None => pallet_babe::recovery::RecoveryEpochs {
            policy,
            intervals: Default::default(),
        },
    };
    if record.policy != policy {
        return Err("wrong maintenance policy");
    }
    let session = Session::current_index();
    let authorities: frame_support::BoundedVec<_, frame_support::traits::ConstU32<1000>> =
        pallet_babe::Authorities::<Runtime>::get()
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "maintenance authority bounds")?;
    if let Some(last) = record.intervals.last_mut() {
        if last.session != session || epoch < last.last_epoch {
            return Err("economic session changed during maintenance");
        }
        if last.authorities == authorities {
            last.last_epoch = epoch;
        } else {
            if epoch <= last.last_epoch {
                return Err("mid-epoch authority change");
            }
            record
                .intervals
                .try_push(pallet_babe::recovery::EpochInterval {
                    first_epoch: epoch,
                    last_epoch: epoch,
                    session,
                    authorities,
                })
                .map_err(|_| "maintenance interval bounds")?;
        }
    } else {
        record
            .intervals
            .try_push(pallet_babe::recovery::EpochInterval {
                first_epoch: epoch,
                last_epoch: epoch,
                session,
                authorities,
            })
            .map_err(|_| "maintenance interval bounds")?;
    }
    sp_io::storage::set(pallet_babe::recovery::STORAGE_KEY, &record.encode());
    Ok(())
}

pub fn finalize_maintenance_epoch() {
    Babe::on_finalize(System::block_number());
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use sp_trie::{recorder_ext::RecorderExt, TrieMut};

    fn policy() -> common::recovery::RecoveryPolicy {
        common::recovery::RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: CHECKPOINT_HASH,
            checkpoint_state_root: common::recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![],
            council: vec![AccountId::new([8; 32])],
            chunk_hashes: vec![[2; 32]],
            audit_chunk_hashes: vec![[3; 32]],
            evidence_commitment: [4; 32],
            terminal_audit_commitment: [5; 32],
        }
    }

    pub(crate) fn pending_key_fixture(
        policy: &common::recovery::RecoveryPolicy,
    ) -> CheckpointOperator {
        let grandpa = |n| GrandpaId::from_slice(&[n; 32]).unwrap();
        let babe = |n| sp_consensus_babe::AuthorityId::from_slice(&[n; 32]).unwrap();
        let owner = AccountId::new([1; 32]);
        System::set_block_number(CHECKPOINT_NUMBER);
        System::initialize(
            &(CHECKPOINT_NUMBER + 1),
            &CHECKPOINT_HASH.into(),
            &Default::default(),
        );
        sp_io::storage::set(POLICY_STORAGE_KEY, &policy.commitment());
        pallet_session::CurrentIndex::<Runtime>::put(7);
        pallet_session::Validators::<Runtime>::put(vec![owner.clone()]);
        let next = crate::opaque::SessionKeys {
            babe: babe(33),
            grandpa: grandpa(13),
            im_online: sp_core::sr25519::Public::from_raw([34; 32]).into(),
            beefy: sp_core::ecdsa::Public::from_raw([35; 33]).into(),
        };
        let mut queued = next.clone();
        queued.grandpa = grandpa(12);
        queued.babe = babe(32);
        for kind in crate::opaque::SessionKeys::key_ids() {
            pallet_session::KeyOwner::<Runtime>::insert(
                (*kind, next.get_raw(*kind).to_vec()),
                &owner,
            );
        }
        pallet_session::NextKeys::<Runtime>::insert(&owner, next);
        pallet_session::QueuedKeys::<Runtime>::put(vec![(owner.clone(), queued)]);
        pallet_session::KeyOwner::<Runtime>::insert(
            (
                sp_consensus_grandpa::KEY_TYPE,
                grandpa(13).as_slice().to_vec(),
            ),
            &owner,
        );
        pallet_grandpa::Authorities::<Runtime>::put(
            frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::try_from(vec![(
                grandpa(11),
                1,
            )])
            .unwrap(),
        );
        pallet_grandpa::CurrentSetId::<Runtime>::put(LAST_ABANDONED_GRANDPA_SET);
        pallet_grandpa::SetIdSession::<Runtime>::insert(LAST_ABANDONED_GRANDPA_SET, 7);
        pallet_session::historical::HistoricalSessions::<Runtime>::insert(
            7,
            (H256::repeat_byte(99), 1),
        );
        pallet_babe::Authorities::<Runtime>::put(
            frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::try_from(vec![(
                babe(31),
                1,
            )])
            .unwrap(),
        );
        pallet_babe::NextAuthorities::<Runtime>::put(
            frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::try_from(vec![(
                babe(32),
                1,
            )])
            .unwrap(),
        );
        pallet_im_online::Keys::<Runtime>::put(
            frame_support::WeakBoundedVec::<_, crate::MaxKeys>::try_from(vec![
                crate::ImOnlineId::from_slice(&[34; 32]).unwrap(),
            ])
            .unwrap(),
        );
        CheckpointOperator {
            owner,
            grandpa_keys: vec![grandpa(11), grandpa(12), grandpa(13)]
                .try_into()
                .unwrap(),
            babe_keys: vec![babe(31), babe(32), babe(33)].try_into().unwrap(),
            im_online_keys: vec![crate::ImOnlineId::from_slice(&[34; 32]).unwrap()]
                .try_into()
                .unwrap(),
        }
    }

    #[test]
    fn retained_inventory_is_versioned_and_checkpoint_derived() {
        sp_io::TestExternalities::default().execute_with(|| {
            let policy = policy();
            let operator = pending_key_fixture(&policy);
            let records = checkpoint_operators().unwrap();
            assert_eq!(records, vec![operator]);
            assert_eq!(
                decode_operators(&encode_operators(&records)).unwrap(),
                records
            );
            assert!(decode_operators(&records.encode()).is_err());
            let report = checkpoint_consensus().unwrap();
            assert_eq!(report.previous_set, LAST_ABANDONED_GRANDPA_SET);
            assert_eq!(report.operators, records);
        });
    }

    #[test]
    fn abandoned_set_rejection_persists_after_frozen_session_and_record_retirement() {
        sp_io::TestExternalities::default().execute_with(|| {
            assert!(grandpa_report_allowed(LAST_ABANDONED_GRANDPA_SET));
            sp_io::storage::set(POLICY_STORAGE_KEY, &[1; 32]);
            pallet_session::CurrentIndex::<Runtime>::put(u32::MAX);
            // The protection does not depend on keeping a historical owner
            // certificate, session trie or GRANDPA range record alive.
            sp_io::storage::clear(pallet_grandpa::recovery::STORAGE_KEY);
            for set in [
                0,
                1,
                LAST_ABANDONED_GRANDPA_SET - 1,
                LAST_ABANDONED_GRANDPA_SET,
            ] {
                assert!(!grandpa_report_allowed(set));
                assert!(prove_grandpa(set, GrandpaId::from_slice(&[11; 32]).unwrap()).is_none());
            }
            assert!(grandpa_report_allowed(LAST_ABANDONED_GRANDPA_SET + 1));
            assert!(grandpa_report_allowed(LAST_ABANDONED_GRANDPA_SET + 13));
        });
    }

    #[test]
    fn installation_preserves_all_signing_keys_and_historical_roots() {
        sp_io::TestExternalities::default().execute_with(|| {
            let policy = policy();
            crate::incident_recovery::with_test_policy(&policy, || {
                let operator = pending_key_fixture(&policy);
                let babe_before = pallet_babe::Authorities::<Runtime>::get();
                let babe_next_before = pallet_babe::NextAuthorities::<Runtime>::get();
                install(
                    policy.commitment(),
                    LAST_ABANDONED_GRANDPA_SET,
                    core::slice::from_ref(&operator),
                )
                .unwrap();
                assert_eq!(
                    pallet_grandpa::CurrentSetId::<Runtime>::get(),
                    LAST_ABANDONED_GRANDPA_SET + 1
                );
                assert_eq!(
                    Session::load_keys(&operator.owner).unwrap().grandpa,
                    operator.grandpa_keys[2]
                );
                assert_eq!(
                    Session::load_keys(&operator.owner).unwrap().babe.as_slice(),
                    &[33; 32]
                );
                let queued = pallet_session::QueuedKeys::<Runtime>::get();
                assert_eq!(queued[0].1.grandpa, operator.grandpa_keys[1]);
                assert_eq!(queued[0].1.babe.as_slice(), &[32; 32]);
                assert_eq!(pallet_babe::Authorities::<Runtime>::get(), babe_before);
                assert_eq!(
                    pallet_babe::NextAuthorities::<Runtime>::get(),
                    babe_next_before
                );
                assert_eq!(
                    pallet_session::historical::HistoricalSessions::<Runtime>::get(7),
                    Some((H256::repeat_byte(99), 1))
                );
                assert_eq!(
                    pallet_session::KeyOwner::<Runtime>::get((
                        sp_consensus_grandpa::KEY_TYPE,
                        operator.grandpa_keys[2].as_slice().to_vec()
                    )),
                    Some(operator.owner.clone())
                );
                assert!(!pallet_session::KeyOwner::<Runtime>::contains_key((
                    sp_consensus_grandpa::KEY_TYPE,
                    operator.grandpa_keys[0].as_slice().to_vec()
                )));
                let proof = prove_grandpa(
                    LAST_ABANDONED_GRANDPA_SET + 1,
                    operator.grandpa_keys[0].clone(),
                )
                .unwrap();
                assert_eq!(
                    RecoveryOwnership::check_proof(
                        (
                            sp_consensus_grandpa::KEY_TYPE,
                            operator.grandpa_keys[0].clone()
                        ),
                        proof
                    )
                    .unwrap()
                    .0,
                    operator.owner
                );
            });
        });
    }

    #[test]
    fn retained_grandpa_report_uses_new_set_and_effective_owner_and_rejects_replay() {
        use sp_core::Pair;
        sp_io::TestExternalities::default().execute_with(|| {
            let mut policy = policy();
            policy
                .replacements
                .push(common::recovery::AccountReplacement {
                    old: AccountId::new([1; 32]),
                    new: AccountId::new([2; 32]),
                });
            crate::incident_recovery::with_test_policy(&policy, || {
                let mut operator = pending_key_fixture(&policy);
                let fresh = sp_core::ed25519::Pair::from_seed(&[20; 32]);
                let active: GrandpaId = fresh.public().into();
                pallet_grandpa::Authorities::<Runtime>::put(
                    frame_support::WeakBoundedVec::<_, crate::MaxAuthorities>::try_from(vec![(
                        active.clone(),
                        1,
                    )])
                    .unwrap(),
                );
                let mut keys = vec![
                    active.clone(),
                    operator.grandpa_keys[1].clone(),
                    operator.grandpa_keys[2].clone(),
                ];
                keys.sort();
                operator.grandpa_keys = keys.try_into().unwrap();
                install(
                    policy.commitment(),
                    LAST_ABANDONED_GRANDPA_SET,
                    core::slice::from_ref(&operator),
                )
                .unwrap();
                pallet_grandpa::SetIdSession::<Runtime>::insert(LAST_ABANDONED_GRANDPA_SET - 1, 6);
                let membership = prove_grandpa(LAST_ABANDONED_GRANDPA_SET + 1, active).unwrap();
                let proof = |set: u64| {
                    let vote = |marker| {
                        let prevote = (H256::repeat_byte(marker), CHECKPOINT_NUMBER + 2);
                        let payload =
                            sp_consensus_grandpa::localized_payload(42, set, &(0u8, prevote));
                        (prevote, fresh.sign(&payload))
                    };
                    sp_consensus_grandpa::EquivocationProof::<H256, u32>::decode_all(
                        &mut (set, 0u8, 42u64, fresh.public(), vote(1), vote(2))
                            .encode()
                            .as_slice(),
                    )
                    .unwrap()
                };
                let reporter = crate::RuntimeOrigin::signed(AccountId::new([9; 32]));
                // The old set also ends at session7. Without the recovery key
                // range check its ordinary historical range would accept this.
                assert!(crate::Grandpa::report_equivocation(
                    reporter.clone(),
                    Box::new(proof(LAST_ABANDONED_GRANDPA_SET)),
                    membership.clone(),
                )
                .is_err());
                assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 0);
                frame_support::assert_ok!(crate::Grandpa::report_equivocation(
                    reporter.clone(),
                    Box::new(proof(LAST_ABANDONED_GRANDPA_SET + 1)),
                    membership.clone(),
                ));
                let reports = pallet_offences::Reports::<Runtime>::iter().collect::<Vec<_>>();
                assert_eq!(reports.len(), 1);
                assert_eq!(reports[0].1.offender.0, AccountId::new([2; 32]));
                assert!(crate::Grandpa::report_equivocation(
                    reporter,
                    Box::new(proof(LAST_ABANDONED_GRANDPA_SET + 1)),
                    membership,
                )
                .is_err());
                assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
            });
        });
    }

    #[test]
    fn activation_rejects_retained_authority_weight_changes() {
        sp_io::TestExternalities::default().execute_with(|| {
            let policy = policy();
            crate::incident_recovery::with_test_policy(&policy, || {
                let operator = pending_key_fixture(&policy);
                install(
                    policy.commitment(),
                    LAST_ABANDONED_GRANDPA_SET,
                    core::slice::from_ref(&operator),
                )
                .unwrap();
                pallet_grandpa::Authorities::<Runtime>::mutate(|authorities| authorities[0].1 += 1);
                assert_eq!(
                    validate_activation(&[operator]),
                    Err("retained GRANDPA authority weights changed during recovery")
                );
            });
        });
    }

    #[test]
    fn incomplete_pending_key_inventory_is_rejected_before_any_write() {
        sp_io::TestExternalities::default().execute_with(|| {
            let policy = policy();
            let mut operator = pending_key_fixture(&policy);
            operator.grandpa_keys.remove(1);
            let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
            assert!(install(policy.commitment(), LAST_ABANDONED_GRANDPA_SET, &[operator]).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
        });
    }

    #[test]
    fn unapproved_or_unused_retained_signing_keys_reject_without_writes() {
        for mutation in 0..5 {
            sp_io::TestExternalities::default().execute_with(|| {
                let policy = policy();
                let mut operator = pending_key_fixture(&policy);
                match mutation {
                    0 => {
                        operator.babe_keys.remove(0);
                    }
                    1 => {
                        operator.babe_keys.remove(1);
                    }
                    2 => {
                        operator.im_online_keys.clear();
                    }
                    3 => {
                        operator
                            .im_online_keys
                            .try_push(crate::ImOnlineId::from_slice(&[35; 32]).unwrap())
                            .unwrap();
                    }
                    _ => {
                        let mut keys = operator.babe_keys.clone().into_inner();
                        keys.swap(0, 1);
                        operator.babe_keys = keys.try_into().unwrap();
                    }
                }
                let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
                assert!(
                    install(policy.commitment(), LAST_ABANDONED_GRANDPA_SET, &[operator]).is_err()
                );
                assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
            });
        }
    }

    #[test]
    fn preserved_root_proof_survives_key_changes_and_rejects_wrong_key() {
        type Hashing = sp_runtime::traits::BlakeTwo256;
        type Layout = sp_trie::LayoutV0<Hashing>;
        let key = [2u8; 32];
        let key_type = sp_consensus_grandpa::KEY_TYPE;
        let identification = (
            AccountId::from([3; 32]),
            pallet_staking::Exposure {
                total: 100,
                own: 100,
                others: vec![],
            },
        );
        let mut db = sp_trie::MemoryDB::<Hashing>::default();
        let mut root = H256::zero();
        {
            let mut trie = sp_trie::TrieDBMutBuilder::<Layout>::new(&mut db, &mut root).build();
            trie.insert(&(key_type, key.as_slice()).encode(), &0u32.encode())
                .unwrap();
            trie.insert(&0u32.encode(), &identification.encode())
                .unwrap();
        }
        let mut recorder = sp_trie::Recorder::<Layout>::new();
        sp_trie::read_trie_value::<Layout, _>(
            &db,
            &root,
            &(key_type, key.as_slice()).encode(),
            Some(&mut recorder),
            None,
        )
        .unwrap();
        sp_trie::read_trie_value::<Layout, _>(
            &db,
            &root,
            &0u32.encode(),
            Some(&mut recorder),
            None,
        )
        .unwrap();
        let proof = MembershipProof {
            session: 7,
            validator_count: 1,
            trie_nodes: recorder.into_raw_storage_proof(),
        };
        let record = OwnerRecord {
            policy: [1; 32],
            session: 7,
            count: 1,
            root,
            index: 0,
            identification: identification.clone(),
            proof: Vec::new(),
        };
        assert_eq!(
            check_preserved_proof(&record, key_type, &key, &proof),
            Some(identification)
        );
        assert_eq!(
            check_preserved_proof(&record, key_type, &[4; 32], &proof),
            None
        );
        let mut wrong = record.clone();
        wrong.root = H256::repeat_byte(8);
        assert_eq!(check_preserved_proof(&wrong, key_type, &key, &proof), None);
        let mut duplicate = proof;
        duplicate.trie_nodes.push(duplicate.trie_nodes[0].clone());
        assert_eq!(
            check_preserved_proof(&record, key_type, &key, &duplicate),
            None
        );
    }

    #[test]
    fn certificates_are_bound_to_policy_session_count_and_identification() {
        let record = OwnerRecord {
            policy: [1; 32],
            session: 7,
            count: 25,
            root: H256::repeat_byte(3),
            index: 0,
            proof: Vec::new(),
            identification: (
                AccountId::from([4; 32]),
                pallet_staking::Exposure {
                    total: 100,
                    own: 100,
                    others: vec![],
                },
            ),
        };
        let mut altered = record.clone();
        altered.session += 1;
        assert_ne!(certificate(&record), certificate(&altered));
        altered = record.clone();
        altered.policy[0] += 1;
        assert_ne!(certificate(&record), certificate(&altered));
        altered = record.clone();
        altered.identification.0 = AccountId::from([5; 32]);
        assert_ne!(certificate(&record), certificate(&altered));
        altered = record.clone();
        altered.count += 1;
        assert_ne!(certificate(&record), certificate(&altered));
    }

    #[test]
    fn unbound_or_premature_installation_writes_nothing() {
        sp_io::TestExternalities::default().execute_with(|| {
            let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
            assert!(install([1; 32], 7, &[]).is_err());
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), before);
        });
    }
}

/// Refuse automatic handoff if any maintenance consensus record was lost or
/// changed. This runs after all account mapping chunks and before installing 138.
pub fn validate_activation(operators: &[CheckpointOperator]) -> Result<(), &'static str> {
    let record = pallet_grandpa::recovery::load().ok_or("missing recovery GRANDPA set")?;
    let current = pallet_grandpa::CurrentSetId::<Runtime>::get();
    if record.previous_set.checked_add(1) != Some(current)
        || pallet_grandpa::SetIdSession::<Runtime>::get(current) != Some(record.session)
        || Session::current_index() != record.session
        || pallet_grandpa::PendingChange::<Runtime>::exists()
    {
        return Err("GRANDPA/session progression during maintenance");
    }
    let current_keys: Vec<_> = pallet_grandpa::Authorities::<Runtime>::get()
        .iter()
        .map(|(key, _)| key.clone())
        .collect();
    if current_keys.as_slice() != record.authorities.as_slice() {
        return Err("wrong recovery authorities");
    }
    if operators.is_empty() || operators.len() > 1000 {
        return Err("operator inventory bounds");
    }
    let policy =
        crate::incident_recovery::compiled_policy().map_err(|_| "missing recovery policy")?;
    let effective = |account: &AccountId| {
        policy
            .replacements
            .iter()
            .find(|entry| &entry.old == account)
            .map(|entry| entry.new.clone())
            .unwrap_or_else(|| account.clone())
    };
    let encoded =
        sp_io::storage::get(KEYS_SNAPSHOT).ok_or("missing checkpoint session-key snapshot")?;
    type SessionKeyRows = Vec<(AccountId, crate::opaque::SessionKeys)>;
    let mut bytes = encoded.as_ref();
    let (next_snapshot, queued_snapshot, authority_snapshot) =
        <(SessionKeyRows, SessionKeyRows, Vec<(GrandpaId, u64)>)>::decode_with_mem_limit(
            &mut bytes,
            MAX_RECORD_BYTES,
        )
        .map_err(|_| "invalid checkpoint session-key snapshot")?;
    if !bytes.is_empty() || next_snapshot.len() != operators.len() || queued_snapshot.len() > 1000 {
        return Err("checkpoint session-key snapshot bounds");
    }
    if pallet_grandpa::Authorities::<Runtime>::get().as_slice() != authority_snapshot.as_slice() {
        return Err("retained GRANDPA authority weights changed during recovery");
    }
    let expected_next = next_snapshot
        .into_iter()
        .map(|(owner, keys)| (effective(&owner), keys))
        .collect::<alloc::collections::BTreeMap<_, _>>();
    let actual_next =
        pallet_session::NextKeys::<Runtime>::iter().collect::<alloc::collections::BTreeMap<_, _>>();
    if expected_next != actual_next || expected_next.len() != operators.len() {
        return Err("retained NextKeys changed during recovery");
    }
    let expected_queued = queued_snapshot
        .into_iter()
        .map(|(owner, keys)| (effective(&owner), keys))
        .collect::<Vec<_>>();
    if pallet_session::QueuedKeys::<Runtime>::get() != expected_queued {
        return Err("retained queued keys changed during recovery");
    }
    for (owner, keys) in &actual_next {
        for key_type in crate::opaque::SessionKeys::key_ids() {
            if pallet_session::KeyOwner::<Runtime>::get((
                *key_type,
                keys.get_raw(*key_type).to_vec(),
            )) != Some(owner.clone())
            {
                return Err("retained session-key owner differs from replacement owner");
            }
        }
    }
    let queued = pallet_session::QueuedKeys::<Runtime>::get();
    if queued.len() > 1000 {
        return Err("queued operator bounds");
    }
    for (owner, keys) in queued {
        let operator = operators
            .iter()
            .find(|entry| {
                policy
                    .replacements
                    .iter()
                    .find(|mapping| mapping.old == entry.owner)
                    .map(|mapping| &mapping.new)
                    .unwrap_or(&entry.owner)
                    == &owner
            })
            .ok_or("unapproved queued operator")?;
        if !operator.grandpa_keys.contains(&keys.grandpa)
            || !operator.babe_keys.contains(&keys.babe)
            || !operator.im_online_keys.contains(&keys.im_online)
        {
            return Err("unapproved queued signing key");
        }
    }
    let validators = Session::validators();
    if validators.len() != current_keys.len() {
        return Err("validator count changed during maintenance");
    }
    for key in &current_keys {
        let owner = load(
            GRANDPA_PREFIX,
            sp_consensus_grandpa::KEY_TYPE,
            key.as_slice(),
        )
        .ok_or("missing retained GRANDPA ownership record")?;
        let effective = effective_identification(owner.identification)
            .ok_or("invalid replacement ownership")?
            .0;
        if owner.session != record.session
            || owner.count as usize != validators.len()
            || validators.get(owner.index as usize) != Some(&effective)
        {
            return Err("retained GRANDPA ownership differs from recovered session");
        }
    }
    let online = pallet_im_online::Keys::<Runtime>::get();
    if online.len() != validators.len() {
        return Err("im-online roster changed during maintenance");
    }
    for (owner, key) in validators.iter().zip(&online) {
        let operator = operators
            .iter()
            .find(|entry| {
                policy
                    .replacements
                    .iter()
                    .find(|mapping| mapping.old == entry.owner)
                    .map(|mapping| &mapping.new)
                    .unwrap_or(&entry.owner)
                    == owner
            })
            .ok_or("unapproved active im-online owner")?;
        if !operator.im_online_keys.contains(key) {
            return Err("unapproved active im-online key");
        }
    }
    let babe = pallet_babe::Authorities::<Runtime>::get();
    for (key, _) in &babe {
        let owner = load(
            &babe_prefix(babe.len() as u32),
            sp_consensus_babe::KEY_TYPE,
            key.as_slice(),
        )
        .ok_or("missing retained BABE ownership")?;
        if !operators
            .iter()
            .any(|entry| entry.owner == owner.identification.0 && entry.babe_keys.contains(key))
        {
            return Err("unapproved active BABE key");
        }
    }
    let epochs = sp_io::storage::get(pallet_babe::recovery::STORAGE_KEY)
        .ok_or("missing maintenance epoch intervals")?;
    let epochs = pallet_babe::recovery::RecoveryEpochs::decode_all(&mut epochs.as_ref())
        .map_err(|_| "invalid maintenance intervals")?;
    if !epochs.valid()
        || epochs.policy != record.policy
        || !epochs.intervals.last().is_some_and(|interval| {
            interval.last_epoch == pallet_babe::EpochIndex::<Runtime>::get()
                && interval.session == record.session
        })
    {
        return Err("maintenance epoch attribution mismatch");
    }
    Ok(())
}

/// During maintenance a previously announced BABE roster can differ from the
/// frozen economic session's index order. Preserve disablement by validator
/// identity instead of accidentally disabling another validator at that index.
pub struct RecoveryDisabledValidators;
impl frame_support::traits::DisabledValidators for RecoveryDisabledValidators {
    fn disabled_validators() -> Vec<u32> {
        if !pallet_grandpa::recovery::load()
            .is_some_and(|set| set.session == Session::current_index())
        {
            return <Session as frame_support::traits::DisabledValidators>::disabled_validators();
        }
        (0..pallet_babe::Authorities::<Runtime>::get().len() as u32)
            .filter(|index| Self::is_disabled(*index))
            .collect()
    }

    fn is_disabled(index: u32) -> bool {
        let Some(set) = pallet_grandpa::recovery::load() else {
            return <Session as frame_support::traits::DisabledValidators>::is_disabled(index);
        };
        if Session::current_index() != set.session {
            return <Session as frame_support::traits::DisabledValidators>::is_disabled(index);
        }
        let authorities = pallet_babe::Authorities::<Runtime>::get();
        let Some((key, _)) = authorities.get(index as usize) else {
            return true;
        };
        let Some(owner) = load(
            &babe_prefix(authorities.len() as u32),
            sp_consensus_babe::KEY_TYPE,
            key.as_slice(),
        ) else {
            return true;
        };
        let original = owner.identification.0.clone();
        let Some((effective, _)) = effective_identification(owner.identification) else {
            return true;
        };
        match Session::validators()
            .iter()
            .position(|validator| validator == &original || validator == &effective)
        {
            Some(economic_index) => {
                <Session as frame_support::traits::DisabledValidators>::is_disabled(
                    economic_index as u32,
                )
            }
            None => false, // queued newcomer was not in the frozen session's disabled index set
        }
    }
}
