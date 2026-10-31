// SPDX-License-Identifier: BSD-4-Clause
//! Bounded activation of SORA Nexus asynchronous staking over the v16 state.
//!
//! The fork deliberately retains legacy ledger claims and storage encodings. In particular,
//! unapplied slashes are not guessed into the incompatible async key/value format. Only the
//! activation markers, validator recency and the local session bridge require writes.

use crate::{AccountId, Runtime, Staking};
use alloc::{collections::BTreeMap, vec::Vec};
#[cfg(feature = "try-runtime")]
use codec::Decode;
use codec::{DecodeAll, Encode};
use frame_support::{
    ensure,
    storage::storage_prefix,
    traits::{Get, OnRuntimeUpgrade, StorageVersion},
    weights::Weight,
    StorageHasher, Twox64Concat,
};
use pallet_staking::legacy;

type StakingConfig = Runtime;

pub struct MigrateSyncStaking;

struct Admission {
    active: u32,
    planned: u32,
    planned_start: Option<u32>,
    planned_session: u32,
    recency: Vec<(AccountId, u32)>,
    reads: u64,
    payload_bytes: u64,
}

fn raw_value<D: DecodeAll>(item: &[u8]) -> Result<Option<D>, &'static str> {
    let key = storage_prefix(b"Staking", item);
    ensure!(
        sp_io::storage::read(&key, &mut [], 0).unwrap_or(0) <= 4096,
        "staking scalar exceeds activation byte bound"
    );
    sp_io::storage::get(&key)
        .map(|value| D::decode_all(&mut &value[..]).map_err(|_| "undecodable staking value"))
        .transpose()
}

/// Iterate raw records rather than a typed iterator, which skips malformed values.
/// Both the number and size of values are bounded before decoding.
fn era_entries<D: DecodeAll>(
    item: &[u8],
    era: u32,
    max: usize,
    max_bytes: u32,
) -> Result<Vec<(Vec<u8>, D)>, &'static str> {
    let mut prefix = storage_prefix(b"Staking", item).to_vec();
    prefix.extend(Twox64Concat::hash(&era.to_le_bytes()));
    let mut cursor = prefix.clone();
    let mut records = Vec::new();
    while let Some(next) = sp_io::storage::next_key(&cursor).filter(|key| key.starts_with(&prefix))
    {
        ensure!(records.len() < max, "staking era exceeds record bound");
        ensure!(
            sp_io::storage::read(&next, &mut [], 0).unwrap_or(0) <= max_bytes,
            "staking era record exceeds byte bound"
        );
        let bytes = sp_io::storage::get(&next).ok_or("staking record disappeared")?;
        records.push((
            next.clone(),
            D::decode_all(&mut &bytes[..]).map_err(|_| "staking era record cannot decode")?,
        ));
        cursor = next;
    }
    Ok(records)
}

/// Every full historical exposure, including the sum across all its pages, must fit the
/// bounded old calculator. This admission rule applies to immutable pre-cutoff snapshots;
/// subsequent valid offence reports cannot trigger a new bound failure or runtime panic.
fn validate_historical_exposures(
    active: u32,
    planned: u32,
    duration: u32,
) -> Result<(u64, u64), &'static str> {
    let max = <Runtime as pallet_staking::Config>::MaxValidatorSet::get() as usize;
    let page_limit = <Runtime as pallet_staking::Config>::MaxExposurePageSize::get();
    let mut reads = 0;
    let mut payload_bytes = 0;
    for era in active.saturating_sub(duration)..=planned {
        for item in [b"ErasStakers".as_slice(), b"ErasStakersClipped"] {
            let exposures = era_entries::<sp_staking::Exposure<AccountId, crate::Balance>>(
                item,
                era,
                max,
                64 * 1024,
            )?;
            reads += 3 * exposures.len() as u64 + 1;
            payload_bytes += exposures
                .iter()
                .map(|(_, exposure)| exposure.encoded_size() as u64)
                .sum::<u64>();
            ensure!(
                exposures
                    .iter()
                    .all(|(_, exposure)| exposure.others.len() <= page_limit as usize),
                "legacy full exposure requires staged historical slashing"
            );
        }
        let overviews = era_entries::<sp_staking::PagedExposureMetadata<crate::Balance>>(
            b"ErasStakersOverview",
            era,
            max,
            128,
        )?;
        reads += 3 * overviews.len() as u64 + 1;
        payload_bytes += overviews
            .iter()
            .map(|(_, overview)| overview.encoded_size() as u64)
            .sum::<u64>();
        // Nominator count is the sum across all pages, not a per-page maximum.
        ensure!(
            overviews
                .iter()
                .all(|(_, overview)| overview.nominator_count <= page_limit
                    && overview.page_count <= 1),
            "historical paged exposure requires staged slashing"
        );
        let pages = era_entries::<sp_staking::ExposurePage<AccountId, crate::Balance>>(
            b"ErasStakersPaged",
            era,
            max,
            64 * 1024,
        )?;
        reads += 3 * pages.len() as u64 + 1;
        payload_bytes += pages
            .iter()
            .map(|(_, page)| page.encoded_size() as u64)
            .sum::<u64>();
        ensure!(
            pages
                .iter()
                .all(|(_, page)| page.others.len() <= page_limit as usize),
            "historical exposure page exceeds its bound"
        );
        let mut page_totals: BTreeMap<Vec<u8>, (usize, usize)> = BTreeMap::new();
        for (page_key, page) in &pages {
            ensure!(
                page_key.len() == 96 && page_key[92..96] == 0u32.to_le_bytes(),
                "historical exposure has unexpected page index"
            );
            let totals = page_totals.entry(page_key[32..84].to_vec()).or_default();
            totals.0 += 1;
            totals.1 += page.others.len();
        }
        for (key, overview) in &overviews {
            // Twox64Concat(era) is 12 bytes; Twox64Concat(AccountId32) is 40.
            ensure!(key.len() == 84, "invalid historical exposure key");
            let (count, nominators) = page_totals.remove(&key[32..84]).unwrap_or_default();
            ensure!(
                count == overview.page_count as usize,
                "historical exposure page count mismatch"
            );
            ensure!(
                nominators == overview.nominator_count as usize,
                "historical exposure nominator count mismatch"
            );
        }
        ensure!(
            page_totals.is_empty(),
            "historical exposure has an orphan page"
        );
    }
    Ok((reads, payload_bytes))
}

/// Admission checks run on chain as well as during the production rehearsal. No balance,
/// ledger, old slash, exposure, claim or reward budget is changed by this migration.
fn admission() -> Result<Admission, &'static str> {
    let active = raw_value::<pallet_staking::ActiveEraInfo>(b"ActiveEra")?
        .ok_or("v16 staking has no active era")?
        .index;
    let planned = raw_value::<u32>(b"CurrentEra")?.ok_or("v16 staking has no planned era")?;
    ensure!(
        planned == active || planned == active.saturating_add(1),
        "invalid planned era"
    );
    let bonded = raw_value::<Vec<(u32, u32)>>(b"BondedEras")?.unwrap_or_default();
    let duration = <StakingConfig as pallet_staking::Config>::BondingDuration::get();
    let expected: Vec<_> = (active.saturating_sub(duration)..=active).collect();
    ensure!(
        bonded.len() <= duration.saturating_add(1) as usize,
        "bonded eras exceed async bound"
    );
    ensure!(
        bonded.iter().map(|(era, _)| *era).eq(expected),
        "invalid bonded era history"
    );
    ensure!(
        bonded.windows(2).all(|pair| pair[0].1 < pair[1].1),
        "invalid bonded session history"
    );
    let planned_start = legacy::ErasStartSessionIndex::<Runtime>::get(planned);
    let planned_session = raw_value::<u32>(b"CurrentPlannedSession")?.unwrap_or_default();
    if planned != active {
        let start = planned_start.ok_or("missing queued era session")?;
        ensure!(
            start > crate::Session::current_index() && start == planned_session,
            "queued era is not the existing next session"
        );
    }
    ensure!(
        !legacy::FirstExecutionEra::<Runtime>::exists(),
        "first execution era already exists on v16"
    );
    ensure!(
        !legacy::CutoffEra::<Runtime>::exists(),
        "async cutoff already exists on v16"
    );
    ensure!(
        !pallet_staking::WeightedPointsFormulaStartEra::<Runtime>::exists(),
        "async incentive cutoff already exists on v16"
    );
    ensure!(
        pallet_staking::LastValidatorEra::<Runtime>::iter()
            .next()
            .is_none(),
        "async validator recency already exists on v16"
    );
    ensure!(
        pallet_staking::AsyncUnappliedSlashes::<Runtime>::iter()
            .next()
            .is_none(),
        "async slash queue already exists on v16"
    );

    let (historical_reads, mut payload_bytes) =
        validate_historical_exposures(active, planned, duration)?;
    let mut reads = 24u64 + historical_reads;
    // All eras capable of holding an executable v16 queue are checked. Older stranded
    // entries remain byte-for-byte intact, and are never made executable by the upgrade.
    let last_execution = active
        .saturating_add(<StakingConfig as pallet_staking::Config>::SlashDeferDuration::get())
        .saturating_add(1);
    for era in active..=last_execution {
        legacy::validate_pending_era::<Runtime>(era)?;
        payload_bytes += sp_io::storage::read(
            &legacy::UnappliedSlashes::<Runtime>::hashed_key_for(era),
            &mut [],
            0,
        )
        .unwrap_or(0) as u64;
        reads += 3;
    }

    let max = <StakingConfig as pallet_staking::Config>::MaxValidatorSet::get() as usize;
    let mut recency = Vec::new();
    for era in active..=planned {
        let validators: Vec<_> = pallet_staking::ErasStakersOverview::<Runtime>::iter_prefix(era)
            .take(max + 1)
            .map(|(validator, _)| validator)
            .collect();
        ensure!(
            validators.len() <= max,
            "era validator set exceeds async bound"
        );
        reads += validators.len() as u64 + 1;
        recency.extend(validators.into_iter().map(|validator| (validator, era)));
    }
    // BTreeMap -> BoundedBTreeMap is byte compatible only if the existing number of
    // rewarded validators is within the new bound. DecodeAll prevents silent defaults.
    let oldest =
        active.saturating_sub(<StakingConfig as pallet_staking::Config>::HistoryDepth::get());
    for era in oldest..=planned {
        let key = pallet_staking::ErasRewardPoints::<Runtime>::hashed_key_for(era);
        ensure!(
            sp_io::storage::read(&key, &mut [], 0).unwrap_or(0) <= 64 * 1024,
            "reward point map exceeds activation byte bound"
        );
        if let Some(bytes) = sp_io::storage::get(&key) {
            payload_bytes += bytes.len() as u64;
            pallet_staking::EraRewardPoints::<Runtime>::decode_all(&mut &bytes[..])
                .map_err(|_| "reward points exceed async bound or cannot decode")?;
        }
        reads += 2;
    }
    Ok(Admission {
        active,
        planned,
        planned_start,
        planned_session,
        recency,
        reads,
        payload_bytes,
    })
}

impl OnRuntimeUpgrade for MigrateSyncStaking {
    fn on_runtime_upgrade() -> Weight {
        let version = StorageVersion::get::<Staking>();
        if version == StorageVersion::new(18) {
            return <Runtime as frame_system::Config>::DbWeight::get().reads(1);
        }
        // Unsupported versions must not run async hooks against unconverted state.
        assert_eq!(
            version,
            StorageVersion::new(16),
            "unsupported synchronous staking version"
        );
        let Admission {
            active,
            planned,
            planned_start,
            planned_session,
            recency,
            reads,
            payload_bytes,
        } = admission().expect("synchronous staking state failed async upgrade admission");
        let bridge_weight = crate::staking_async::initialize_from_sync(
            active,
            planned,
            planned_start,
            planned_session,
        )
        .expect("synchronous staking session state failed async upgrade admission");
        let validators = recency.len() as u64;
        for (validator, era) in recency {
            pallet_staking::LastValidatorEra::<Runtime>::insert(validator, era);
        }
        legacy::CutoffEra::<Runtime>::put(planned);
        legacy::FirstExecutionEra::<Runtime>::put(active);
        pallet_staking::WeightedPointsFormulaStartEra::<Runtime>::put(active.saturating_add(1));
        StorageVersion::new(18).put::<Staking>();
        bridge_weight
            .saturating_add(
                <Runtime as frame_system::Config>::DbWeight::get()
                    .reads_writes(reads + 1, validators + 4),
            )
            .saturating_add(Weight::from_parts(
                payload_bytes.saturating_mul(100_000),
                payload_bytes.saturating_add(reads.saturating_mul(4096)),
            ))
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        let version = StorageVersion::get::<Staking>();
        if version == StorageVersion::new(18) {
            return Ok((false, [0u8; 32]).encode());
        }
        ensure!(
            version == StorageVersion::new(16),
            "unsupported synchronous staking version"
        );
        admission()?;
        audit_legacy_state()?;
        Ok((true, preserved_state_digest()).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        let (migrated, previous): (bool, [u8; 32]) =
            Decode::decode(&mut &state[..]).map_err(|_| "invalid async migration witness")?;
        ensure!(
            StorageVersion::get::<Staking>() == StorageVersion::new(18),
            "async version not installed"
        );
        if !migrated {
            return Ok(());
        }
        ensure!(
            previous == preserved_state_digest(),
            "legacy staking state was changed during async activation"
        );
        audit_legacy_state()?;
        let active = pallet_staking::ActiveEra::<Runtime>::get()
            .ok_or("missing active era")?
            .index;
        ensure!(
            legacy::CutoffEra::<Runtime>::get() == pallet_staking::CurrentEra::<Runtime>::get(),
            "wrong legacy cutoff"
        );
        ensure!(
            legacy::FirstExecutionEra::<Runtime>::get() == Some(active),
            "wrong first execution era"
        );
        ensure!(
            pallet_staking::WeightedPointsFormulaStartEra::<Runtime>::get()
                == Some(active.saturating_add(1)),
            "wrong incentive cutoff"
        );
        Ok(())
    }
}

/// Full raw storage validation is deliberately a rehearsal check. Runtime activation above
/// does only bounded work; try-runtime also checks every retained ledger, claim and exposure,
/// including legacy prefixes no longer present in upstream metadata.
#[cfg(feature = "try-runtime")]
pub fn audit_legacy_state() -> Result<(), sp_runtime::TryRuntimeError> {
    audit_prefix::<pallet_staking::StakingLedger<Runtime>>(b"Ledger")?;
    audit_prefix::<pallet_staking::EraRewardPoints<Runtime>>(b"ErasRewardPoints")?;
    audit_prefix::<Vec<u32>>(b"ClaimedRewards")?;
    audit_prefix::<sp_staking::Exposure<AccountId, crate::Balance>>(b"ErasStakers")?;
    audit_prefix::<sp_staking::Exposure<AccountId, crate::Balance>>(b"ErasStakersClipped")?;
    audit_prefix::<sp_staking::ExposurePage<AccountId, crate::Balance>>(b"ErasStakersPaged")?;
    audit_prefix::<sp_staking::PagedExposureMetadata<crate::Balance>>(b"ErasStakersOverview")?;
    audit_prefix::<Vec<legacy::LegacyUnappliedSlash<AccountId, crate::Balance>>>(
        b"UnappliedSlashes",
    )?;
    audit_prefix::<legacy::SlashingSpansData>(b"SlashingSpans")?;
    audit_prefix::<legacy::SpanRecord<crate::Balance>>(b"SpanSlash")?;
    audit_prefix::<crate::Balance>(b"NominatorSlashInEra")?;
    Ok(())
}

#[cfg(feature = "try-runtime")]
fn audit_prefix<D: DecodeAll>(item: &[u8]) -> Result<(), sp_runtime::TryRuntimeError> {
    let prefix = storage_prefix(b"Staking", item);
    let mut key = prefix.to_vec();
    let (mut count, mut max_bytes) = (0u64, 0usize);
    while let Some(next) = sp_io::storage::next_key(&key).filter(|next| next.starts_with(&prefix)) {
        let bytes = sp_io::storage::get(&next).ok_or("staking key vanished during audit")?;
        D::decode_all(&mut &bytes[..])
            .map_err(|_| "retained staking record failed full decoding")?;
        max_bytes = max_bytes.max(bytes.len());
        count += 1;
        key = next;
    }
    log::info!(target: "runtime::staking_async_migration", "legacy inventory {}: records={}, max_bytes={}",
        core::str::from_utf8(item).unwrap_or("?"), count, max_bytes);
    Ok(())
}

#[cfg(feature = "try-runtime")]
fn preserved_state_digest() -> [u8; 32] {
    let prefix = sp_io::hashing::twox_128(b"Staking");
    let excluded = [
        b":__STORAGE_VERSION__:".as_slice(),
        b"CutoffEra",
        b"FirstExecutionEra",
        b"WeightedPointsFormulaStartEra",
        b"LastValidatorEra",
    ]
    .map(|item| storage_prefix(b"Staking", item));
    let mut key = prefix.to_vec();
    let mut digest = [0u8; 32];
    while let Some(next) = sp_io::storage::next_key(&key).filter(|next| next.starts_with(&prefix)) {
        if !excluded.iter().any(|item| next.starts_with(item)) {
            let value = sp_io::storage::get(&next).expect("key exists during staking audit");
            digest = sp_io::hashing::blake2_256(&(digest, &next, &value[..]).encode());
        }
        key = next;
    }
    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Balance, Balances};
    use common::balance;
    use frame_support::{assert_ok, traits::Currency};
    use sp_runtime::Perbill;

    fn account(byte: u8) -> AccountId {
        AccountId::new([byte; 32])
    }

    fn seed_v16() {
        frame_system::Pallet::<Runtime>::set_block_number(1);
        StorageVersion::new(16).put::<Staking>();
        pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
            index: 3,
            start: Some(1),
        });
        pallet_staking::CurrentEra::<Runtime>::put(3);
        sp_io::storage::set(
            &storage_prefix(b"Staking", b"BondedEras"),
            &(0..=3u32)
                .map(|era| (era, era * 6))
                .collect::<Vec<_>>()
                .encode(),
        );
        legacy::CurrentPlannedSession::<Runtime>::put(21);
    }

    fn bond(stash: &AccountId, amount: Balance) {
        drop(Balances::deposit_creating(stash, amount));
        assert_ok!(
            <Balances as frame_support::traits::fungible::hold::Mutate<AccountId>>::set_on_hold(
                &pallet_staking::HoldReason::Staking.into(),
                stash,
                amount
            )
        );
        pallet_staking::Bonded::<Runtime>::insert(stash, stash);
        pallet_staking::Ledger::<Runtime>::insert(
            stash,
            pallet_staking::StakingLedger::<Runtime> {
                stash: stash.clone(),
                total: amount,
                active: amount,
                unlocking: Default::default(),
                legacy_claimed_rewards: vec![1].try_into().unwrap(),
                controller: Some(stash.clone()),
            },
        );
    }

    fn slash(
        stash: &AccountId,
        amount: Balance,
    ) -> legacy::LegacyUnappliedSlash<AccountId, Balance> {
        legacy::LegacyUnappliedSlash {
            validator: stash.clone(),
            own: amount,
            others: Vec::new(),
            reporters: Vec::new(),
            payout: 0,
        }
    }

    #[test]
    fn async_activation_preserves_legacy_ledger_claims_exposure_queues_and_is_idempotent() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let stash = account(201);
            bond(&stash, balance!(100));
            legacy::ErasStakersClipped::<Runtime>::insert(
                1,
                &stash,
                sp_staking::Exposure {
                    own: balance!(100),
                    total: balance!(100),
                    others: Vec::new(),
                },
            );
            pallet_staking::ClaimedRewards::<Runtime>::insert(
                2,
                &stash,
                frame_support::WeakBoundedVec::force_from(vec![0], None),
            );
            legacy::UnappliedSlashes::<Runtime>::insert(4, vec![slash(&stash, balance!(5))]);
            let before = [
                pallet_staking::Ledger::<Runtime>::hashed_key_for(&stash),
                legacy::ErasStakersClipped::<Runtime>::hashed_key_for(1, &stash),
                pallet_staking::ClaimedRewards::<Runtime>::hashed_key_for(2, &stash),
                legacy::UnappliedSlashes::<Runtime>::hashed_key_for(4),
            ]
            .map(|key| (key.clone(), sp_io::storage::get(&key)));
            MigrateSyncStaking::on_runtime_upgrade();
            assert_eq!(StorageVersion::get::<Staking>(), StorageVersion::new(18));
            assert_eq!(legacy::CutoffEra::<Runtime>::get(), Some(3));
            for (key, value) in before {
                assert_eq!(sp_io::storage::get(&key), value);
            }
            let root = sp_io::storage::root(sp_runtime::StateVersion::V0);
            MigrateSyncStaking::on_runtime_upgrade();
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), root);
        });
    }

    #[test]
    fn async_activation_rejects_full_exposure_over_bound_before_writing() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            pallet_staking::ErasStakersOverview::<Runtime>::insert(
                2,
                account(202),
                sp_staking::PagedExposureMetadata {
                    total: 257,
                    own: 0,
                    nominator_count: 257,
                    page_count: 2,
                },
            );
            assert!(admission().is_err());
            assert!(!legacy::CutoffEra::<Runtime>::exists());
            assert_eq!(StorageVersion::get::<Staking>(), StorageVersion::new(16));
        });
    }

    #[test]
    fn frozen_queue_drains_after_multiple_era_changes_and_preserves_cancellation_indices() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let stash = account(203);
            bond(&stash, balance!(100));
            legacy::UnappliedSlashes::<Runtime>::insert(
                3,
                vec![
                    slash(&stash, balance!(1)),
                    slash(&stash, balance!(2)),
                    slash(&stash, balance!(3)),
                ],
            );
            // This historically stranded entry must never become newly executable.
            legacy::UnappliedSlashes::<Runtime>::insert(2, vec![slash(&stash, balance!(50))]);
            MigrateSyncStaking::on_runtime_upgrade();
            legacy::apply_pending_slashes::<Runtime>(3);
            assert_eq!(legacy::LegacySlashCursor::<Runtime>::get(3), 1);
            assert!(legacy::cancel_deferred_slash::<Runtime>(3, vec![0]).is_err());
            assert_ok!(legacy::cancel_deferred_slash::<Runtime>(3, vec![2]));
            assert!(legacy::has_pending_due_slashes::<Runtime>(7));
            legacy::apply_pending_slashes::<Runtime>(7);
            assert!(!legacy::has_pending_due_slashes::<Runtime>(7));
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&stash)
                    .unwrap()
                    .active,
                balance!(97)
            );
            assert_eq!(legacy::UnappliedSlashes::<Runtime>::get(2).len(), 1);
        });
    }

    #[test]
    fn frozen_queue_accepts_more_than_sixty_four_records_without_truncation() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let stash = account(204);
            bond(&stash, balance!(100));
            legacy::UnappliedSlashes::<Runtime>::insert(3, vec![slash(&stash, balance!(1)); 65]);
            MigrateSyncStaking::on_runtime_upgrade();
            for _ in 0..65 {
                legacy::apply_pending_slashes::<Runtime>(8);
            }
            assert!(!legacy::has_pending_due_slashes::<Runtime>(8));
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&stash)
                    .unwrap()
                    .active,
                balance!(35)
            );
        });
    }

    #[test]
    fn historical_increment_reports_use_separate_records_without_a_sixty_four_report_cap() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let stash = account(205);
            bond(&stash, balance!(100));
            legacy::ErasStakers::<Runtime>::insert(
                3,
                &stash,
                sp_staking::Exposure {
                    own: balance!(100),
                    total: balance!(100),
                    others: Vec::new(),
                },
            );
            MigrateSyncStaking::on_runtime_upgrade();
            for fraction in 1..=70 {
                let offence = pallet_staking_async_rc_client::Offence {
                    offender: stash.clone(),
                    reporters: Vec::new(),
                    slash_fraction: Perbill::from_percent(fraction),
                };
                assert!(legacy::on_new_offences::<Runtime>(3, &[offence]).is_some());
            }
            let execution = 3 + <Runtime as pallet_staking::Config>::SlashDeferDuration::get() + 1;
            if <Runtime as pallet_staking::Config>::SlashDeferDuration::get() > 0 {
                assert_eq!(legacy::HistoricalSlashTail::<Runtime>::get(execution), 70);
                assert!(legacy::UnappliedSlashes::<Runtime>::get(execution).is_empty());
                for _ in 0..70 {
                    legacy::apply_pending_slashes::<Runtime>(execution + 3);
                }
                assert!(!legacy::has_pending_due_slashes::<Runtime>(execution + 3));
            }
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&stash)
                    .unwrap()
                    .active,
                balance!(30)
            );
        });
    }

    #[test]
    fn legacy_reporters_keep_exact_split_and_slash_issuance() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let stash = account(206);
            let reporter_a = account(207);
            let reporter_b = account(208);
            bond(&stash, balance!(100));
            drop(Balances::deposit_creating(&reporter_a, balance!(10)));
            drop(Balances::deposit_creating(&reporter_b, balance!(10)));
            let issuance = Balances::total_issuance();
            let mut pending = slash(&stash, balance!(10));
            pending.reporters = vec![reporter_a.clone(), reporter_b.clone()];
            pending.payout = balance!(6);
            legacy::UnappliedSlashes::<Runtime>::insert(3, vec![pending]);
            MigrateSyncStaking::on_runtime_upgrade();
            legacy::apply_pending_slashes::<Runtime>(5);
            assert_eq!(Balances::free_balance(&reporter_a), balance!(13));
            assert_eq!(Balances::free_balance(&reporter_b), balance!(13));
            assert_eq!(Balances::total_issuance(), issuance - balance!(4));
        });
    }

    #[test]
    fn historical_reports_preserve_cross_era_nominator_span_maximum() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let validator_a = account(209);
            let validator_b = account(210);
            let nominator = account(211);
            for stash in [&validator_a, &validator_b, &nominator] {
                bond(stash, balance!(1000));
            }
            for (era, validator) in [(1, &validator_a), (2, &validator_b)] {
                legacy::ErasStakers::<Runtime>::insert(
                    era,
                    validator,
                    sp_staking::Exposure {
                        own: balance!(100),
                        total: balance!(200),
                        others: vec![sp_staking::IndividualExposure {
                            who: nominator.clone(),
                            value: balance!(100),
                        }],
                    },
                );
            }
            MigrateSyncStaking::on_runtime_upgrade();
            for (era, validator, percent) in
                [(1, validator_a.clone(), 20), (2, validator_b.clone(), 10)]
            {
                legacy::on_new_offences::<Runtime>(
                    era,
                    &[pallet_staking_async_rc_client::Offence {
                        offender: validator,
                        reporters: Vec::new(),
                        slash_fraction: Perbill::from_percent(percent),
                    }],
                );
            }
            let execution = 3 + <Runtime as pallet_staking::Config>::SlashDeferDuration::get() + 1;
            for _ in 0..2 {
                legacy::apply_pending_slashes::<Runtime>(execution + 1);
            }
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&nominator)
                    .unwrap()
                    .active,
                balance!(980)
            );
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&validator_a)
                    .unwrap()
                    .active,
                balance!(980)
            );
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&validator_b)
                    .unwrap()
                    .active,
                balance!(990)
            );
        });
    }

    #[test]
    fn already_planned_era_keeps_legacy_policy_without_reviving_stranded_queues() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            pallet_staking::CurrentEra::<Runtime>::put(4);
            pallet_session::CurrentIndex::<Runtime>::put(20);
            legacy::ErasStartSessionIndex::<Runtime>::insert(4, 21);
            let stash = account(212);
            bond(&stash, balance!(100));
            legacy::ErasStakers::<Runtime>::insert(
                4,
                &stash,
                sp_staking::Exposure {
                    own: balance!(100),
                    total: balance!(100),
                    others: Vec::new(),
                },
            );
            legacy::UnappliedSlashes::<Runtime>::insert(2, vec![slash(&stash, balance!(50))]);
            legacy::UnappliedSlashes::<Runtime>::insert(3, vec![slash(&stash, balance!(1))]);
            MigrateSyncStaking::on_runtime_upgrade();
            assert_eq!(legacy::CutoffEra::<Runtime>::get(), Some(4));
            assert_eq!(legacy::FirstExecutionEra::<Runtime>::get(), Some(3));
            assert_eq!(
                crate::staking_async::State::<Runtime>::get().queued_era,
                Some((21, 4))
            );
            legacy::apply_pending_slashes::<Runtime>(4);
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&stash)
                    .unwrap()
                    .active,
                balance!(99)
            );
            assert_eq!(legacy::UnappliedSlashes::<Runtime>::get(2).len(), 1);
            pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
                index: 4,
                start: Some(2),
            });
            assert!(
                legacy::on_new_offences::<Runtime>(
                    4,
                    &[pallet_staking_async_rc_client::Offence {
                        offender: stash,
                        reporters: Vec::new(),
                        slash_fraction: Perbill::from_percent(5),
                    }]
                )
                .is_some(),
                "pre-announced era must retain historical calculator"
            );
        });
    }

    #[test]
    fn new_cancellation_survives_native_queue_drain_and_cancels_historical_report() {
        if <Runtime as pallet_staking::Config>::SlashDeferDuration::get() == 0 {
            return;
        }
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            let stash = account(213);
            bond(&stash, balance!(100));
            legacy::ErasStakers::<Runtime>::insert(
                3,
                &stash,
                sp_staking::Exposure {
                    own: balance!(100),
                    total: balance!(100),
                    others: Vec::new(),
                },
            );
            MigrateSyncStaking::on_runtime_upgrade();
            let fraction = Perbill::from_percent(10);
            legacy::on_new_offences::<Runtime>(
                3,
                &[pallet_staking_async_rc_client::Offence {
                    offender: stash.clone(),
                    reporters: Vec::new(),
                    slash_fraction: fraction,
                }],
            );
            let execution = 3 + <Runtime as pallet_staking::Config>::SlashDeferDuration::get() + 1;
            assert_ok!(Staking::cancel_async_deferred_slash(
                crate::RuntimeOrigin::root(),
                execution,
                vec![(stash.clone(), fraction)]
            ));
            let async_stash = account(214);
            pallet_staking::AsyncUnappliedSlashes::<Runtime>::insert(
                execution,
                (async_stash.clone(), fraction, 0),
                pallet_staking::UnappliedSlash::<Runtime> {
                    validator: async_stash,
                    own: 0,
                    others: Default::default(),
                    reporter: None,
                    payout: 0,
                },
            );
            Staking::apply_unapplied_slashes(execution);
            assert!(!pallet_staking::CancelledSlashes::<Runtime>::get(execution).is_empty());
            legacy::apply_pending_slashes::<Runtime>(execution);
            assert!(!legacy::has_pending_due_slashes::<Runtime>(execution));
            assert_eq!(
                pallet_staking::Ledger::<Runtime>::get(&stash)
                    .unwrap()
                    .active,
                balance!(100)
            );
        });
    }

    #[test]
    fn malformed_frozen_queue_is_rejected_without_silent_typed_default() {
        sp_io::TestExternalities::default().execute_with(|| {
            seed_v16();
            sp_io::storage::set(
                &legacy::UnappliedSlashes::<Runtime>::hashed_key_for(3),
                &[255],
            );
            assert!(admission().is_err());
            assert!(!legacy::FirstExecutionEra::<Runtime>::exists());
            assert_eq!(StorageVersion::get::<Staking>(), StorageVersion::new(16));
        });
    }
}
