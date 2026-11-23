//! Native liability regressions for recovered staking ownership.
//!
//! Fixtures transform typed current economic records through the real guarded
//! recovery engine. Historical session trie bytes are retained and verified by
//! the actual ownership-proof implementation. Penalties and rewards use native
//! runtime handlers, never a test replacement for their accounting.

use crate::{
    AccountId, Balance, Balances, Currencies, Historical, Runtime, RuntimeCall, RuntimeOrigin,
    Staking, System,
};
use codec::Encode;
use common::{
    balance,
    recovery::{
        self, AccountReplacement, GuardedStorageEdit, RecoveryChunk, RecoveryPolicy, StorageScope,
    },
    VAL,
};
use frame_support::{
    assert_ok,
    traits::{Currency, KeyOwnerProofSystem, OnRuntimeUpgrade, StorageVersion},
};
use pallet_staking::{legacy, Exposure, IndividualExposure};
use sp_core::H256;
use sp_runtime::{traits::Dispatchable, Perbill};
use sp_staking::offence::{OffenceDetails, OnOffenceHandler};
use sp_trie::{recorder_ext::RecorderExt, TrieMut};
use traits::MultiCurrency;

const ERA: u32 = 3;
const SLASH_SESSION: u32 = 18;
fn account(byte: u8) -> AccountId {
    AccountId::new([byte; 32])
}
fn replacements() -> Vec<AccountReplacement> {
    [(201, 211), (202, 212), (203, 213)]
        .into_iter()
        .map(|(old, new)| AccountReplacement {
            old: account(old),
            new: account(new),
        })
        .collect()
}
fn effective(owner: &AccountId) -> AccountId {
    replacements()
        .into_iter()
        .find(|r| r.old == *owner)
        .map_or_else(|| owner.clone(), |r| r.new)
}
fn exposure() -> Exposure<AccountId, Balance> {
    Exposure {
        total: balance!(200),
        own: balance!(100),
        others: vec![
            IndividualExposure {
                who: account(202),
                value: balance!(50),
            },
            IndividualExposure {
                who: account(204),
                value: balance!(50),
            },
        ],
    }
}
fn bond(stash: &AccountId) {
    drop(Balances::deposit_creating(stash, balance!(2000)));
    assert_ok!(
        <Balances as frame_support::traits::fungible::hold::Mutate<AccountId>>::set_on_hold(
            &pallet_staking::HoldReason::Staking.into(),
            stash,
            balance!(1000)
        )
    );
    pallet_staking::Bonded::<Runtime>::insert(stash, stash);
    pallet_staking::Ledger::<Runtime>::insert(
        stash,
        pallet_staking::StakingLedger::<Runtime> {
            stash: stash.clone(),
            total: balance!(1000),
            active: balance!(1000),
            unlocking: Default::default(),
            legacy_claimed_rewards: vec![1].try_into().unwrap(),
            controller: Some(stash.clone()),
        },
    );
    pallet_staking::Payee::<Runtime>::insert(stash, pallet_staking::RewardDestination::Stash);
}
fn seed_v16() {
    System::set_block_number(1);
    StorageVersion::new(16).put::<Staking>();
    pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
        index: ERA,
        start: Some(1),
    });
    pallet_staking::CurrentEra::<Runtime>::put(ERA);
    let bonded_eras: frame_support::BoundedVec<_, pallet_staking::BondedErasBound<Runtime>> = (0
        ..=ERA)
        .map(|era| (era, era * 6))
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    pallet_staking::BondedEras::<Runtime>::put(bonded_eras);
    legacy::CurrentPlannedSession::<Runtime>::put(21);
    legacy::ErasStartSessionIndex::<Runtime>::insert(ERA, SLASH_SESSION);
    pallet_session::CurrentIndex::<Runtime>::put(21);
    pallet_staking::SlashRewardFraction::<Runtime>::put(Perbill::from_percent(10));
    for who in [account(201), account(202), account(204)] {
        bond(&who);
    }
    drop(Balances::deposit_creating(&account(203), balance!(10)));
    legacy::ErasStakers::<Runtime>::insert(ERA, account(201), exposure());
    legacy::ErasStakersClipped::<Runtime>::insert(ERA, account(201), exposure());
}

fn history_proof() -> (Vec<u8>, sp_session::MembershipProof, Vec<u8>) {
    type Hashing = sp_runtime::traits::BlakeTwo256;
    type Layout = sp_trie::LayoutV0<Hashing>;
    let authority = vec![81; 32];
    let key_type = sp_consensus_babe::KEY_TYPE;
    let identification = (account(201), exposure());
    let mut db = sp_trie::MemoryDB::<Hashing>::default();
    let mut root = H256::zero();
    {
        let mut trie = sp_trie::TrieDBMutBuilder::<Layout>::new(&mut db, &mut root).build();
        trie.insert(&(key_type, authority.as_slice()).encode(), &0u32.encode())
            .unwrap();
        trie.insert(&0u32.encode(), &identification.encode())
            .unwrap();
    }
    let mut recorder = sp_trie::Recorder::<Layout>::new();
    for key in [(key_type, authority.as_slice()).encode(), 0u32.encode()] {
        sp_trie::read_trie_value::<Layout, _>(&db, &root, &key, Some(&mut recorder), None).unwrap();
    }
    let proof = sp_session::MembershipProof {
        session: SLASH_SESSION,
        validator_count: 1,
        trie_nodes: recorder.into_raw_storage_proof(),
    };
    pallet_session::historical::HistoricalSessions::<Runtime>::insert(SLASH_SESSION, (root, 1));
    let root_bytes = sp_io::storage::get(
        &pallet_session::historical::HistoricalSessions::<Runtime>::hashed_key_for(SLASH_SESSION),
    )
    .unwrap()
    .to_vec();
    assert_eq!(
        Historical::check_proof((key_type, authority.clone()), proof.clone()),
        Some(identification)
    );
    (authority, proof, root_bytes)
}

fn move_record(
    edits: &mut Vec<GuardedStorageEdit>,
    source: Vec<u8>,
    destination: Vec<u8>,
    changed: Option<Vec<u8>>,
) {
    let Some(before) = sp_io::storage::get(&source) else {
        assert!(changed.is_none());
        return;
    };
    assert!(!sp_io::storage::exists(&destination));
    edits.extend([
        GuardedStorageEdit {
            scope: StorageScope::Top,
            key: source,
            before: Some(sp_io::hashing::blake2_256(&before)),
            after: None,
        },
        GuardedStorageEdit {
            scope: StorageScope::Top,
            key: destination,
            before: None,
            after: Some(changed.unwrap_or_else(|| before.to_vec())),
        },
    ]);
}
fn map_exposure(mut value: Exposure<AccountId, Balance>) -> Exposure<AccountId, Balance> {
    for nominator in &mut value.others {
        nominator.who = effective(&nominator.who);
    }
    value
}

/// Exact typed fixture equivalent of the control/history compilers' edits. It
/// performs no debit, reward, slashing calculation or historical-root mutation.
fn recover_current_state() -> RecoveryPolicy {
    let mut edits = Vec::new();
    for pair in replacements() {
        for (source, destination) in [
            (
                frame_system::Account::<Runtime>::hashed_key_for(&pair.old),
                frame_system::Account::<Runtime>::hashed_key_for(&pair.new),
            ),
            (
                pallet_balances::Holds::<Runtime>::hashed_key_for(&pair.old),
                pallet_balances::Holds::<Runtime>::hashed_key_for(&pair.new),
            ),
            (
                pallet_balances::Locks::<Runtime>::hashed_key_for(&pair.old),
                pallet_balances::Locks::<Runtime>::hashed_key_for(&pair.new),
            ),
            (
                tokens::Accounts::<Runtime>::hashed_key_for(&pair.old, crate::AssetId::from(VAL)),
                tokens::Accounts::<Runtime>::hashed_key_for(&pair.new, crate::AssetId::from(VAL)),
            ),
            (
                pallet_staking::Payee::<Runtime>::hashed_key_for(&pair.old),
                pallet_staking::Payee::<Runtime>::hashed_key_for(&pair.new),
            ),
            (
                legacy::SlashingSpans::<Runtime>::hashed_key_for(&pair.old),
                legacy::SlashingSpans::<Runtime>::hashed_key_for(&pair.new),
            ),
        ] {
            move_record(&mut edits, source, destination, None);
        }
        if pallet_staking::Bonded::<Runtime>::contains_key(&pair.old) {
            move_record(
                &mut edits,
                pallet_staking::Bonded::<Runtime>::hashed_key_for(&pair.old),
                pallet_staking::Bonded::<Runtime>::hashed_key_for(&pair.new),
                Some(pair.new.encode()),
            );
            let mut ledger = pallet_staking::Ledger::<Runtime>::get(&pair.old).unwrap();
            ledger.stash = pair.new.clone();
            ledger.controller = Some(pair.new.clone());
            move_record(
                &mut edits,
                pallet_staking::Ledger::<Runtime>::hashed_key_for(&pair.old),
                pallet_staking::Ledger::<Runtime>::hashed_key_for(&pair.new),
                Some(ledger.encode()),
            );
        }
        for era in 0..=ERA {
            for (source, destination) in [
                (
                    pallet_staking::ValidatorSlashInEra::<Runtime>::hashed_key_for(era, &pair.old),
                    pallet_staking::ValidatorSlashInEra::<Runtime>::hashed_key_for(era, &pair.new),
                ),
                (
                    legacy::NominatorSlashInEra::<Runtime>::hashed_key_for(era, &pair.old),
                    legacy::NominatorSlashInEra::<Runtime>::hashed_key_for(era, &pair.new),
                ),
                (
                    pallet_staking::ClaimedRewards::<Runtime>::hashed_key_for(era, &pair.old),
                    pallet_staking::ClaimedRewards::<Runtime>::hashed_key_for(era, &pair.new),
                ),
                (
                    pallet_staking::ErasValidatorPrefs::<Runtime>::hashed_key_for(era, &pair.old),
                    pallet_staking::ErasValidatorPrefs::<Runtime>::hashed_key_for(era, &pair.new),
                ),
            ] {
                move_record(&mut edits, source, destination, None);
            }
            for (source, destination, value) in [
                (
                    legacy::ErasStakers::<Runtime>::hashed_key_for(era, &pair.old),
                    legacy::ErasStakers::<Runtime>::hashed_key_for(era, &pair.new),
                    legacy::ErasStakers::<Runtime>::get(era, &pair.old),
                ),
                (
                    legacy::ErasStakersClipped::<Runtime>::hashed_key_for(era, &pair.old),
                    legacy::ErasStakersClipped::<Runtime>::hashed_key_for(era, &pair.new),
                    legacy::ErasStakersClipped::<Runtime>::get(era, &pair.old),
                ),
            ] {
                if sp_io::storage::exists(&source) {
                    move_record(
                        &mut edits,
                        source,
                        destination,
                        Some(map_exposure(value).encode()),
                    );
                }
            }
        }
        for ((owner, index), _) in legacy::SpanSlash::<Runtime>::iter()
            .filter(|((owner, _), _)| owner == &pair.old)
            .collect::<Vec<_>>()
        {
            move_record(
                &mut edits,
                legacy::SpanSlash::<Runtime>::hashed_key_for((owner, index)),
                legacy::SpanSlash::<Runtime>::hashed_key_for((pair.new.clone(), index)),
                None,
            );
        }
    }
    for era in 0..=ERA {
        if pallet_staking::ErasRewardPoints::<Runtime>::contains_key(era) {
            let mut points = pallet_staking::ErasRewardPoints::<Runtime>::get(era);
            points.individual = points
                .individual
                .into_inner()
                .into_iter()
                .map(|(owner, value)| (effective(&owner), value))
                .collect::<std::collections::BTreeMap<_, _>>()
                .try_into()
                .unwrap();
            let key = pallet_staking::ErasRewardPoints::<Runtime>::hashed_key_for(era);
            let before = sp_io::storage::get(&key).unwrap();
            edits.push(GuardedStorageEdit {
                scope: StorageScope::Top,
                key,
                before: Some(sp_io::hashing::blake2_256(&before)),
                after: Some(points.encode()),
            });
        }
    }
    edits.sort_by(|a, b| a.key.cmp(&b.key));
    let chunk = RecoveryChunk { index: 0, edits };
    chunk.validate().unwrap();
    let policy = RecoveryPolicy {
        format_version: 2,
        authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
        checkpoint_hash: recovery::CHECKPOINT_HASH,
        checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
        replacements: replacements(),
        council: vec![account(250)],
        chunk_hashes: vec![chunk.commitment()],
        audit_chunk_hashes: vec![[1; 32]],
        evidence_commitment: [2; 32],
        terminal_audit_commitment: [3; 32],
    };
    recovery::storage::initialize(&policy).unwrap();
    recovery::storage::apply_chunk(&policy, &chunk).unwrap();
    policy
}
fn report(identification: (AccountId, Exposure<AccountId, Balance>), percent: u32) {
    crate::staking_async::LocalOffences::on_offence(
        &[OffenceDetails {
            offender: identification,
            reporters: vec![account(203)],
        }],
        &[Perbill::from_percent(percent)],
        SLASH_SESSION,
    );
}
fn drain() {
    let due = ERA + <Runtime as pallet_staking::Config>::SlashDeferDuration::get() + 1;
    for _ in 0..8 {
        if !legacy::has_pending_due_slashes::<Runtime>(due) {
            break;
        }
        legacy::apply_pending_slashes::<Runtime>(due);
    }
    assert!(!legacy::has_pending_due_slashes::<Runtime>(due));
}
fn active(owner: u8) -> Balance {
    pallet_staking::Ledger::<Runtime>::get(account(owner))
        .unwrap()
        .active
}
fn economic_snapshot() -> (Balance, Balance, Balance, Balance, Balance) {
    (
        active(211),
        active(212),
        active(204),
        Balances::free_balance(account(213)),
        Balances::total_issuance(),
    )
}
fn assert_root_preserved(bytes: &[u8]) {
    assert_eq!(
        sp_io::storage::get(
            &pallet_session::historical::HistoricalSessions::<Runtime>::hashed_key_for(
                SLASH_SESSION
            )
        )
        .unwrap()
        .as_ref(),
        bytes
    );
}

#[test]
fn authenticated_historical_offence_debits_replacement_ledgers_without_rewriting_proof_root() {
    sp_io::TestExternalities::default().execute_with(|| {
        seed_v16();
        let (authority, proof, root) = history_proof();
        let policy = recover_current_state();
        crate::incident_recovery::with_test_policy(&policy, || {
            crate::staking_async_migration::MigrateSyncStaking::on_runtime_upgrade();
            assert_eq!(StorageVersion::get::<Staking>(), StorageVersion::new(18));
            assert_eq!(
                Historical::check_proof(
                    (sp_consensus_babe::KEY_TYPE, authority.clone()),
                    proof.clone()
                ),
                Some((account(201), exposure()))
            );
            let verified = crate::incident_recovery_consensus::RecoveryOwnership::check_proof(
                (sp_consensus_babe::KEY_TYPE, authority),
                proof,
            )
            .expect("retained historical proof authenticates before mapping");
            assert_eq!(verified.0, account(211));
            assert_eq!(verified.1.others[0].who, account(212));
            assert_eq!(verified.1.others[1].who, account(204));
            report(verified, 10);
            drain();
            assert_eq!(active(211), balance!(990));
            assert_eq!(active(212), balance!(995));
            assert_eq!(active(204), balance!(995));
            assert_eq!(Balances::free_balance(account(213)), balance!(11));
            for old in [201, 202, 203] {
                assert_eq!(Balances::total_balance(&account(old)), 0);
                assert!(!pallet_staking::Ledger::<Runtime>::contains_key(account(
                    old
                )));
            }
            assert_root_preserved(&root);
        });
    });
}

#[test]
fn remapped_historical_maxima_reject_repeat_and_only_charge_higher_native_increment() {
    sp_io::TestExternalities::default().execute_with(|| {
        seed_v16();
        let (authority, proof, root) = history_proof();
        crate::staking_async_migration::MigrateSyncStaking::on_runtime_upgrade();
        report((account(201), exposure()), 10);
        drain();
        assert_eq!(active(201), balance!(990));
        assert_eq!(Balances::free_balance(account(203)), balance!(11));
        let old_span = legacy::SpanSlash::<Runtime>::get((account(201), 0));
        let old_nominator = legacy::NominatorSlashInEra::<Runtime>::get(ERA, account(202));
        let policy = recover_current_state();
        crate::incident_recovery::with_test_policy(&policy, || {
            assert_eq!(
                legacy::SpanSlash::<Runtime>::get((account(211), 0)),
                old_span
            );
            assert_eq!(
                legacy::NominatorSlashInEra::<Runtime>::get(ERA, account(212)),
                old_nominator
            );
            assert_eq!(
                pallet_staking::ValidatorSlashInEra::<Runtime>::get(ERA, account(211))
                    .unwrap()
                    .0,
                Perbill::from_percent(10)
            );
            let before = economic_snapshot();
            let raw = Historical::check_proof(
                (sp_consensus_babe::KEY_TYPE, authority.clone()),
                proof.clone(),
            )
            .unwrap();
            // Also exercise the idempotent final financial boundary on an
            // already-authenticated historical identity, before owner resolution.
            report(raw, 10);
            drain();
            assert_eq!(
                economic_snapshot(),
                before,
                "repeat fraction must not debit or reward again"
            );
            let resolved = crate::incident_recovery_consensus::RecoveryOwnership::check_proof(
                (sp_consensus_babe::KEY_TYPE, authority),
                proof,
            )
            .unwrap();
            report(resolved, 20);
            drain();
            let after = economic_snapshot();
            assert_eq!(before.0 - after.0, balance!(10));
            assert_eq!(before.1 - after.1, balance!(5));
            assert_eq!(before.2 - after.2, balance!(5));
            assert_eq!(after.3 - before.3, balance!(1.5));
            assert_eq!(before.4 - after.4, balance!(18.5));
            assert_eq!(
                legacy::SpanSlash::<Runtime>::get((account(211), 0)).slashed,
                balance!(20)
            );
            assert_eq!(
                legacy::SpanSlash::<Runtime>::get((account(211), 0)).paid_out,
                balance!(1.25)
            );
            let after_higher = economic_snapshot();
            report((account(201), exposure()), 20);
            drain();
            assert_eq!(economic_snapshot(), after_higher);
            assert_root_preserved(&root);
        });
    });
}

fn reward_era(era: u32) {
    pallet_staking::ErasValidatorReward::<Runtime>::insert(era, balance!(1000));
    xor_fee::ValStakingEraReward::<Runtime>::insert(era, balance!(1000));
    pallet_staking::ErasRewardPoints::<Runtime>::insert(
        era,
        pallet_staking::EraRewardPoints::<Runtime> {
            total: 10,
            individual: std::collections::BTreeMap::from([(account(201), 10)])
                .try_into()
                .unwrap(),
        },
    );
    legacy::ErasStakersClipped::<Runtime>::insert(era, account(201), exposure());
    pallet_staking::ErasValidatorPrefs::<Runtime>::insert(
        era,
        account(201),
        pallet_staking::ValidatorPrefs::default(),
    );
}
fn payout(owner: u8, era: u32, page: bool) -> frame_support::dispatch::DispatchResultWithPostInfo {
    let call = if page {
        pallet_staking::Call::payout_stakers_by_page {
            validator_stash: account(owner),
            era,
            page: 0,
        }
    } else {
        pallet_staking::Call::payout_stakers {
            validator_stash: account(owner),
            era,
        }
    };
    RuntimeCall::Staking(call).dispatch(RuntimeOrigin::signed(account(250)))
}

#[test]
fn remapped_claimed_pages_and_legacy_claim_markers_cannot_mint_rewards_again() {
    sp_io::TestExternalities::default().execute_with(|| {
        seed_v16();
        let (_, _, root) = history_proof();
        for era in 0..3 {
            reward_era(era);
        }
        assert_ok!(payout(201, 2, false));
        assert_eq!(
            pallet_staking::ClaimedRewards::<Runtime>::get(2, account(201)).as_slice(),
            &[0]
        );
        assert_eq!(Currencies::total_issuance(VAL.into()), balance!(1000));
        let policy = recover_current_state();
        crate::incident_recovery::with_test_policy(&policy, || {
            crate::staking_async_migration::MigrateSyncStaking::on_runtime_upgrade();
            let before = (
                Balances::total_issuance(),
                Currencies::total_issuance(VAL.into()),
                Currencies::free_balance(VAL.into(), &account(211)),
                Currencies::free_balance(VAL.into(), &account(212)),
                Currencies::free_balance(VAL.into(), &account(204)),
            );
            assert_eq!(before.2, balance!(500));
            assert_eq!(before.3, balance!(250));
            assert_eq!(before.4, balance!(250));
            for era in [1, 2] {
                for page in [false, true] {
                    assert_eq!(
                        payout(211, era, page).unwrap_err().error,
                        pallet_staking::Error::<Runtime>::AlreadyClaimed.into()
                    );
                }
            }
            assert!(payout(201, 2, false).is_err());
            assert_eq!(
                (
                    Balances::total_issuance(),
                    Currencies::total_issuance(VAL.into()),
                    Currencies::free_balance(VAL.into(), &account(211)),
                    Currencies::free_balance(VAL.into(), &account(212)),
                    Currencies::free_balance(VAL.into(), &account(204))
                ),
                before
            );
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(account(211))
                    .unwrap()
                    .legacy_claimed_rewards
                    .as_slice(),
                &[1]
            );
            assert_eq!(
                pallet_staking::ClaimedRewards::<Runtime>::get(2, account(211)).as_slice(),
                &[0]
            );
            assert!(!pallet_staking::ClaimedRewards::<Runtime>::contains_key(
                2,
                account(201)
            ));
            assert_eq!(Currencies::free_balance(VAL.into(), &account(201)), 0);
            assert_ok!(payout(211, 0, false));
            assert_eq!(
                Currencies::total_issuance(VAL.into()),
                before.1 + balance!(1000)
            );
            assert_eq!(Balances::total_issuance(), before.0);
            assert!(payout(211, 0, false).is_err());
            assert_eq!(
                Currencies::total_issuance(VAL.into()),
                before.1 + balance!(1000)
            );
            assert_root_preserved(&root);
        });
    });
}

#[derive(Clone)]
struct RecoveryDuplicateOffence((AccountId, Exposure<AccountId, Balance>));
impl sp_staking::offence::Offence<(AccountId, Exposure<AccountId, Balance>)>
    for RecoveryDuplicateOffence
{
    const ID: sp_staking::offence::Kind = *b"sora-nexus:test0";
    type Slot = u64;
    fn offenders(&self) -> Vec<(AccountId, Exposure<AccountId, Balance>)> {
        vec![self.0.clone()]
    }
    fn session_index(&self) -> u32 {
        SLASH_SESSION
    }
    fn validator_set_count(&self) -> u32 {
        1
    }
    fn slot(&self) -> Self::Slot {
        123
    }
    fn slash_fraction(&self, count: u32) -> Perbill {
        // An accidental duplicate changes severity as well as graph cardinality.
        Perbill::from_percent(count.min(10) * 10)
    }
}

#[test]
fn old_and_effective_historical_identities_share_offence_deduplication_before_slashing() {
    use sp_staking::offence::{Offence, OffenceError, ReportOffence};
    type Identification = (AccountId, Exposure<AccountId, Balance>);
    type Reports = crate::liveness::EquivocationReports;
    sp_io::TestExternalities::default().execute_with(|| {
        seed_v16();
        let (authority, proof, root) = history_proof();
        let policy = recover_current_state();
        crate::incident_recovery::with_test_policy(&policy, || {
            crate::staking_async_migration::MigrateSyncStaking::on_runtime_upgrade();
            let original = Historical::check_proof(
                (sp_consensus_babe::KEY_TYPE, authority.clone()),
                proof.clone(),
            )
            .expect("retained proof must still authenticate its original identity");
            let resolved = (account(211), map_exposure(exposure()));
            assert_eq!(original, (account(201), exposure()));
            assert_ok!(<Reports as ReportOffence<
                AccountId,
                Identification,
                RecoveryDuplicateOffence,
            >>::report_offence(
                vec![account(203)],
                RecoveryDuplicateOffence(original.clone()),
            ));
            let id = H256::from(sp_io::hashing::blake2_256(
                &(RecoveryDuplicateOffence::ID, 123u64.encode(), &resolved).encode(),
            ));
            assert_eq!(
                pallet_offences::ConcurrentReportsIndex::<Runtime>::get(
                    RecoveryDuplicateOffence::ID,
                    123u64.encode(),
                ),
                vec![id]
            );
            let stored = pallet_offences::Reports::<Runtime>::get(id).unwrap();
            assert_eq!(stored.offender, resolved);
            assert_eq!(stored.reporters, vec![account(213)]);
            assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
            for identity in [original.clone(), resolved.clone()] {
                assert!(<Reports as ReportOffence<
                    AccountId,
                    Identification,
                    RecoveryDuplicateOffence,
                >>::is_known_offence(&[identity], &123));
            }
            drain();
            assert_eq!(active(211), balance!(990));
            assert_eq!(active(212), balance!(995));
            assert_eq!(active(204), balance!(995));
            let before = economic_snapshot();
            for identity in [resolved.clone(), original.clone()] {
                assert_eq!(
                    <Reports as ReportOffence<
                        AccountId,
                        Identification,
                        RecoveryDuplicateOffence,
                    >>::report_offence(
                        vec![account(203)], RecoveryDuplicateOffence(identity),
                    ),
                    Err(OffenceError::DuplicateReport)
                );
                drain();
                assert_eq!(economic_snapshot(), before);
            }
            assert_eq!(
                pallet_offences::ConcurrentReportsIndex::<Runtime>::get(
                    RecoveryDuplicateOffence::ID,
                    123u64.encode(),
                ),
                vec![id]
            );
            assert_eq!(pallet_offences::Reports::<Runtime>::iter().count(), 1);
            assert_eq!(
                Historical::check_proof((sp_consensus_babe::KEY_TYPE, authority), proof),
                Some(original)
            );
            assert_root_preserved(&root);
        });
    });
}
