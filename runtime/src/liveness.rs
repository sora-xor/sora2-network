// This file is part of the SORA network and Polkaswap app.
// SPDX-License-Identifier: BSD-4-Clause

use crate::{AccountId, Historical, ImOnline, ImOnlineId, Offences, Runtime, Session};
use codec::Encode;
use frame_support::{
    traits::OneSessionHandler,
    weights::{
        constants::{WEIGHT_REF_TIME_PER_MICROS, WEIGHT_REF_TIME_PER_NANOS},
        Weight,
    },
};
use sp_staking::{
    offence::{Offence, OffenceError, OffenceSeverity, ReportOffence},
    SessionIndex,
};
use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::BlockNumberFor;
    use sp_staking::SessionIndex;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Bound session history even if validators are repeatedly disabled and re-enabled.
        #[pallet::constant]
        type MaxTrackedValidators: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Validators prevented from authoring at any point in the tagged session.
    /// Absence means that a complete session has not been observed (for example,
    /// immediately after the runtime upgrade), so offline penalties must wait.
    #[pallet::storage]
    pub type DisabledDuringSession<T: Config> =
        StorageValue<_, (SessionIndex, BoundedBTreeSet<u32, T::MaxTrackedValidators>), OptionQuery>;

    /// A complete, bounded scan of immutable exposures in the retained eras.
    /// Absence keeps the configured static equivocation-weight fallback in force.
    #[pallet::storage]
    pub type ExposureWork<T: Config> = StorageValue<_, super::ExposureWorkSnapshot, OptionQuery>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            // This pallet is runtime-local. Rebuild known immutable exposure
            // metadata immediately, within the same explicit scan bound used
            // by session rotation, and charge its complete bounded work.
            super::refresh_exposure_work()
        }
    }
}

const MAX_EXPOSURE_SCAN_ENTRIES: u32 = 4_096;
type RetainedEras = frame_support::BoundedVec<u32, frame_support::traits::ConstU32<128>>;

#[derive(
    codec::Encode, codec::Decode, codec::MaxEncodedLen, scale_info::TypeInfo, PartialEq, Eq,
)]
pub struct ExposureContext {
    session: SessionIndex,
    active_era: u32,
    planned_era: u32,
    eras: RetainedEras,
}

#[derive(codec::Encode, codec::Decode, codec::MaxEncodedLen, scale_info::TypeInfo)]
pub struct ExposureWorkSnapshot {
    context: ExposureContext,
    /// Count exposure rows, including the same nominator backing multiple validators.
    pub(crate) max_nomination_edges: u32,
    pub(crate) max_validators: u32,
}

fn exposure_context() -> Option<ExposureContext> {
    let session = Session::current_index();
    let active_era = pallet_staking::ActiveEra::<Runtime>::get()?.index;
    let planned_era = pallet_staking::CurrentEra::<Runtime>::get()?;
    let mut eras = pallet_staking::BondedEras::<Runtime>::get()
        .into_iter()
        .map(|(era, _)| era)
        .collect::<BTreeSet<_>>();
    if planned_era < active_era || !eras.contains(&active_era) {
        return None;
    }
    // Staking may already have planned the next era. Include its immutable
    // exposure snapshot so the next active-era transition cannot undercount work.
    eras.insert(planned_era);
    Some(ExposureContext {
        session,
        active_era,
        planned_era,
        eras: eras.into_iter().collect::<Vec<_>>().try_into().ok()?,
    })
}

fn cached_exposure_work() -> Option<ExposureWorkSnapshot> {
    let snapshot = ExposureWork::<Runtime>::get()?;
    (snapshot.context == exposure_context()?).then_some(snapshot)
}

/// Refresh only under the session rotation's reserved block budget. Inspect at
/// most 4,097 small overview values and 128 legacy prefixes, never legacy vectors.
/// Returns the DB weight consumed for callers that require explicit accounting.
#[allow(deprecated)]
pub(crate) fn refresh_exposure_work() -> Weight {
    ExposureWork::<Runtime>::kill();
    let db = <Runtime as frame_system::Config>::DbWeight::get();
    let mut reads = 4u64;
    // The DB budget covers next-key/value IO. Add an explicit conservative
    // parsing/iteration allowance for the entire bounded scan on every path.
    let scan_cpu = Weight::from_parts(
        u64::from(MAX_EXPOSURE_SCAN_ENTRIES + 1) * 10 * WEIGHT_REF_TIME_PER_MICROS,
        0,
    );
    let Some(context) = exposure_context() else {
        return db.reads_writes(reads, 1).saturating_add(scan_cpu);
    };
    let mut scanned = 0u32;
    let mut max_nomination_edges = 0;
    let mut max_validators = 0;
    for &era in &context.eras {
        reads = reads.saturating_add(1);
        if pallet_staking::ErasStakers::<Runtime>::iter_key_prefix(era)
            .next()
            .is_some()
        {
            // Legacy full-vector lengths are not covered by paged metadata.
            // Keep the static fallback rather than guessing or decoding an
            // unbounded exposure value in a hook.
            return db.reads_writes(reads, 1).saturating_add(scan_cpu);
        }
        let mut nomination_edges = 0u32;
        let mut validators = 0u32;
        let mut overviews = pallet_staking::ErasStakersOverview::<Runtime>::iter_prefix(era);
        loop {
            // One next-key lookup and, when present, one value read.
            reads = reads.saturating_add(2);
            let Some((_, overview)) = overviews.next() else {
                break;
            };
            scanned = scanned.saturating_add(1);
            if scanned > MAX_EXPOSURE_SCAN_ENTRIES {
                return db.reads_writes(reads, 1).saturating_add(scan_cpu);
            }
            let Some(total) = nomination_edges.checked_add(overview.nominator_count) else {
                return db.reads_writes(reads, 1).saturating_add(scan_cpu);
            };
            nomination_edges = total;
            validators += 1;
        }
        max_nomination_edges = max_nomination_edges.max(nomination_edges);
        max_validators = max_validators.max(validators);
    }
    ExposureWork::<Runtime>::put(ExposureWorkSnapshot {
        context,
        max_nomination_edges,
        max_validators,
    });
    db.reads_writes(reads, 2).saturating_add(scan_cpu)
}

type Identification = pallet_session::historical::IdentificationTuple<Runtime>;
type OfflineOffence = pallet_im_online::UnresponsivenessOffence<Identification>;

/// Preserve ImOnline's key type and hooks while recording every disablement.
/// Recording in `on_disabled` also covers disable/re-enable churn within a block.
pub struct LivenessImOnline;

impl sp_runtime::BoundToRuntimeAppPublic for LivenessImOnline {
    type Public = ImOnlineId;
}

impl LivenessImOnline {
    fn start_tracking_session() {
        let disabled: Result<
            frame_support::BoundedBTreeSet<u32, <Runtime as Config>::MaxTrackedValidators>,
            _,
        > = Session::disabled_validators()
            .into_iter()
            .collect::<BTreeSet<_>>()
            .try_into();
        match disabled {
            Ok(disabled) => {
                DisabledDuringSession::<Runtime>::put((Session::current_index(), disabled))
            }
            // An incomplete history cannot safely support financial penalties.
            Err(_) => DisabledDuringSession::<Runtime>::kill(),
        }
    }
}

impl OneSessionHandler<AccountId> for LivenessImOnline {
    type Key = ImOnlineId;

    fn on_genesis_session<'a, I: 'a>(validators: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
    {
        <ImOnline as OneSessionHandler<AccountId>>::on_genesis_session(validators);
        Self::start_tracking_session();
    }

    fn on_new_session<'a, I: 'a>(changed: bool, validators: I, queued_validators: I)
    where
        I: Iterator<Item = (&'a AccountId, Self::Key)>,
    {
        // Session has already reported the outgoing session, rotated its index
        // and cleared obsolete disablements before invoking this hook.
        Self::start_tracking_session();
        <ImOnline as OneSessionHandler<AccountId>>::on_new_session(
            changed,
            validators,
            queued_validators,
        );
        let _ = refresh_exposure_work();
    }

    fn on_before_session_ending() {
        <ImOnline as OneSessionHandler<AccountId>>::on_before_session_ending();
    }

    fn on_disabled(validator_index: u32) {
        let session = Session::current_index();
        DisabledDuringSession::<Runtime>::mutate(|history| {
            if let Some((tracked_session, disabled)) = history {
                if *tracked_session != session || disabled.try_insert(validator_index).is_err() {
                    // Do not turn partial or overflowing history into an apparently
                    // complete session. The next session hook starts a fresh history.
                    *history = None;
                }
            }
        });
        <ImOnline as OneSessionHandler<AccountId>>::on_disabled(validator_index);
    }
}

/// Staking applies financial penalties, while equivocation reporters below
/// explicitly retain consensus disabling. Offline reports must not disable a
/// returning author while the outgoing session is being finalized.
pub struct SlashOnlySessionInterface;

impl pallet_staking::SessionInterface<AccountId> for SlashOnlySessionInterface {
    fn report_offence(_validator: AccountId, _severity: OffenceSeverity) {}

    fn validators() -> Vec<AccountId> {
        Session::validators()
    }

    fn prune_historical_up_to(up_to: SessionIndex) {
        Historical::prune_up_to(up_to);
    }
}

struct BorrowedOffence<'a, O>(&'a O);

impl<O: Offence<Identification>> Offence<Identification> for BorrowedOffence<'_, O> {
    const ID: sp_staking::offence::Kind = O::ID;
    type TimeSlot = O::TimeSlot;

    fn offenders(&self) -> Vec<Identification> {
        self.0.offenders()
    }

    fn session_index(&self) -> SessionIndex {
        self.0.session_index()
    }

    fn validator_set_count(&self) -> u32 {
        self.0.validator_set_count()
    }

    fn time_slot(&self) -> Self::TimeSlot {
        self.0.time_slot()
    }

    fn slash_fraction(&self, offenders_count: u32) -> sp_runtime::Perbill {
        self.0.slash_fraction(offenders_count)
    }
}

/// Keep BABE/GRANDPA offence bookkeeping and financial penalties unchanged,
/// and apply the same consensus disablements that staking previously applied.
pub struct EquivocationReports;

impl<O: Offence<Identification>> ReportOffence<AccountId, Identification, O>
    for EquivocationReports
{
    fn report_offence(reporters: Vec<AccountId>, offence: O) -> Result<(), OffenceError> {
        <Offences as ReportOffence<AccountId, Identification, BorrowedOffence<'_, O>>>::report_offence(
            reporters,
            BorrowedOffence(&offence),
        )?;

        let Some(active_era) = pallet_staking::ActiveEra::<Runtime>::get() else {
            return Ok(());
        };
        let active_era_start =
            pallet_staking::ErasStartSessionIndex::<Runtime>::get(active_era.index).unwrap_or(0);
        // Historical offences still slash their original exposure, but must not
        // disable validators for an offence outside the active era.
        if offence.session_index() < active_era_start {
            return Ok(());
        }

        // Offences recalculates severity for all concurrent offenders whenever
        // one new report arrives. Replay that complete set in the same order.
        let concurrent = pallet_offences::ConcurrentReportsIndex::<Runtime>::get(
            O::ID,
            offence.time_slot().encode(),
        )
        .into_iter()
        .filter_map(pallet_offences::Reports::<Runtime>::get)
        .collect::<Vec<_>>();
        let severity = OffenceSeverity(offence.slash_fraction(concurrent.len() as u32));
        let invulnerables = pallet_staking::Invulnerables::<Runtime>::get();
        for details in concurrent {
            let validator = details.offender.0;
            if !invulnerables.contains(&validator) {
                Session::report_offence(validator, severity);
            }
        }
        Ok(())
    }

    fn is_known_offence(offenders: &[Identification], time_slot: &O::TimeSlot) -> bool {
        <Offences as ReportOffence<AccountId, Identification, O>>::is_known_offence(
            offenders, time_slot,
        )
    }
}

/// Include history maintenance in the declared weight of equivocation extrinsics.
/// Session rotation already reserves the maximum block weight for its hooks.
pub struct EquivocationWeights;

impl EquivocationWeights {
    pub(crate) fn full_block_queue_growth(validators: u32, edges: u32) -> u64 {
        // Utility/multisig can price their children before an earlier child
        // appends a slash. Every report necessarily pays at least this existing
        // history charge, so a block cannot contain more reports than this bound.
        // Use cached exposure validators, rather than the current proof count:
        // earlier proofs in the same block may come from a smaller authority set.
        let minimum_report = Self::disabled_history(validators.max(1)).ref_time().max(1);
        let reports = crate::BlockWeights::get().max_block.ref_time() / minimum_report;
        let payload = 106u64
            .saturating_mul(u64::from(validators))
            .saturating_add(48u64.saturating_mul(u64::from(edges)))
            .saturating_add(5);
        reports.saturating_mul(payload)
    }

    fn queued_payloads(
        context: &ExposureContext,
        validators: u64,
        edges: u64,
        block_growth: u64,
    ) -> Weight {
        let defer = <Runtime as pallet_staking::Config>::SlashDeferDuration::get();
        let execution_eras = context
            .eras
            .iter()
            .map(|era| era.saturating_add(defer).saturating_add(1))
            .filter(|era| *era > context.active_era)
            .collect::<BTreeSet<_>>();
        let mut largest_queue = 0u64;
        let mut all_queue_bytes = 0u64;
        for era in &execution_eras {
            let key = pallet_staking::UnappliedSlashes::<Runtime>::hashed_key_for(era);
            // Obtain the encoded length without decoding an unbounded queue.
            // Probe live state: this value can grow between session boundaries.
            let bytes = u64::from(sp_io::storage::read(&key, &mut [], 0).unwrap_or(0));
            largest_queue = largest_queue.max(bytes);
            all_queue_bytes = all_queue_bytes.saturating_add(bytes);
        }
        // SCALE upper bound: validator, own amount, two Vec prefixes, one
        // reporter, payout; each nomination row contains AccountId + Balance.
        // BABE/GRANDPA accept one reporter per unique offence report.
        let new_payload = 106u64
            .saturating_mul(validators)
            .saturating_add(48u64.saturating_mul(edges))
            .saturating_add(5);
        let repeated_bytes = largest_queue
            .saturating_add(block_growth)
            .saturating_add(new_payload)
            .saturating_mul(validators)
            .saturating_mul(2);
        let probes = execution_eras.len() as u64;
        // Conservative 25 ns/byte encode/decode/copy budget (40 MB/s), including
        // repeated whole-vector queue mutations and the live length probes.
        Weight::from_parts(
            repeated_bytes
                .saturating_add(all_queue_bytes)
                .saturating_mul(25 * WEIGHT_REF_TIME_PER_NANOS),
            all_queue_bytes.saturating_add(4_096u64.saturating_mul(probes)),
        )
        .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(probes))
    }

    fn report_weight(
        validator_count: u32,
        configured_nominators: u32,
        upstream: fn(u32, u32) -> Weight,
    ) -> Weight {
        let cached = cached_exposure_work();
        let context = cached.as_ref().map(|snapshot| &snapshot.context);
        let fallback_context;
        let context = if let Some(context) = context {
            context
        } else {
            fallback_context = exposure_context();
            let Some(context) = fallback_context.as_ref() else {
                return Weight::MAX;
            };
            context
        };
        let validators = cached
            .as_ref()
            .map(|snapshot| snapshot.max_validators.max(validator_count))
            .unwrap_or(validator_count)
            .max(1);
        let edges = cached
            .as_ref()
            .map(|snapshot| snapshot.max_nomination_edges)
            // Preserve the full configured bound for every concurrent validator
            // when history is missing, stale, legacy, or exceeds the scan bound.
            .unwrap_or_else(|| configured_nominators.saturating_mul(validators));
        let block_growth = Self::full_block_queue_growth(
            cached
                .as_ref()
                .map(|snapshot| snapshot.max_validators)
                .unwrap_or(validators),
            edges,
        );
        let base = upstream(validator_count, 0);
        let nomination_work = upstream(validator_count, edges).saturating_sub(base);
        let participants = u64::from(edges).saturating_add(u64::from(validators));
        let db = <Runtime as frame_system::Config>::DbWeight::get();

        base.saturating_mul(u64::from(validators))
            .saturating_add(nomination_work)
            // Late reports apply immediately. Charge this path even if a
            // particular report will defer: 6 reads / 5 writes per participant
            // plus 2 reads / writes for each possible reporter.
            .saturating_add(
                db.reads_writes(
                    participants
                        .saturating_mul(6)
                        .saturating_add(u64::from(validators) * 2),
                    participants
                        .saturating_mul(5)
                        .saturating_add(u64::from(validators) * 2),
                ),
            )
            .saturating_add(Weight::from_parts(
                25 * WEIGHT_REF_TIME_PER_MICROS * participants,
                0,
            ))
            .saturating_add(Self::disabled_history(validators))
            // Cache plus live session/active/planned/retained-era context reads.
            .saturating_add(db.reads(9))
            .saturating_add(Weight::from_parts(0, 16_384))
            .saturating_add(Self::queued_payloads(
                context,
                u64::from(validators),
                u64::from(edges),
                block_growth,
            ))
    }

    fn disabled_history(validator_count: u32) -> Weight {
        // Offences may revisit every concurrent offender, not just the newly
        // submitted one. The history value itself never exceeds its storage bound.
        let callbacks = u64::from(validator_count.max(1));
        let capacity = <Runtime as Config>::MaxTrackedValidators::get();
        // The proof's historical set can differ from the current session's
        // history; use the storage capacity rather than assuming equal lengths.
        let tracked = u64::from(capacity);
        let db = <Runtime as frame_system::Config>::DbWeight::get();

        // Conservative encode/decode budget, in addition to the storage IO costs.
        // The shared history key and CurrentIndex proof are charged once, even
        // when the handler reads and writes them repeatedly in the same report.
        Weight::from_parts(
            WEIGHT_REF_TIME_PER_MICROS
                .saturating_mul(tracked.saturating_add(1))
                .saturating_mul(callbacks),
            4_096u64
                .saturating_add(9)
                .saturating_add(4u64.saturating_mul(u64::from(capacity))),
        )
        .saturating_add(db.reads_writes(callbacks.saturating_mul(2), callbacks))
        // Active era, its starting session, invulnerables, concurrent index and
        // each concurrent report are read again to preserve disabling semantics.
        // These keys were already read by Offences/Staking, so no new proof is needed.
        .saturating_add(db.reads(callbacks.saturating_add(4)))
    }
}

impl pallet_babe::WeightInfo for EquivocationWeights {
    fn plan_config_change() -> Weight {
        <() as pallet_babe::WeightInfo>::plan_config_change()
    }

    fn report_equivocation(validator_count: u32, max_nominators_per_validator: u32) -> Weight {
        Self::report_weight(
            validator_count,
            max_nominators_per_validator,
            <() as pallet_babe::WeightInfo>::report_equivocation,
        )
    }
}

impl pallet_grandpa::WeightInfo for EquivocationWeights {
    fn note_stalled() -> Weight {
        <() as pallet_grandpa::WeightInfo>::note_stalled()
    }

    fn report_equivocation(validator_count: u32, max_nominators_per_validator: u32) -> Weight {
        Self::report_weight(
            validator_count,
            max_nominators_per_validator,
            <() as pallet_grandpa::WeightInfo>::report_equivocation,
        )
    }
}

/// Forward offline reports only for a strict majority of the full active set.
/// ImOnline has already emitted SomeOffline before calling this reporter.
pub struct MajorityOfflineReports;

impl ReportOffence<AccountId, Identification, OfflineOffence> for MajorityOfflineReports {
    fn report_offence(
        reporters: Vec<AccountId>,
        mut offence: OfflineOffence,
    ) -> Result<(), OffenceError> {
        let validators = Session::validators();
        // The session-ending hook runs before the session and disabled set rotate.
        // Do not apply current-session history to an inconsistent report.
        const INVALID_SESSION_CONTEXT: u8 = 1;
        if offence.session_index != Session::current_index()
            || offence.validator_set_count as usize != validators.len()
        {
            return Err(OffenceError::Other(INVALID_SESSION_CONTEXT));
        }

        let Some((tracked_session, disabled_during_session)) =
            DisabledDuringSession::<Runtime>::get()
        else {
            // The upgrade's partially observed session gets one session of grace.
            return Ok(());
        };
        if tracked_session != offence.session_index {
            return Ok(());
        }

        let disabled = Session::disabled_validators();
        let mut eligible: BTreeSet<AccountId> = validators
            .into_iter()
            .enumerate()
            .filter(|(index, _)| {
                let index = *index as u32;
                disabled.binary_search(&index).is_err() && !disabled_during_session.contains(&index)
            })
            .map(|(_, account)| account)
            .collect();

        // A validator disabled at any time in this session had its opportunity to
        // author restricted. Re-enabling it must not manufacture a coordinated outage.
        // Removing from the set also excludes duplicate or non-validator accounts
        // from both the count and the forwarded offenders.
        offence
            .offenders
            .retain(|(account, _)| eligible.remove(account));

        // Keep disabled validators in the denominator: shrinking the threshold
        // could penalize a minority of the actual validator set. Exactly half does not qualify.
        if offence.offenders.len() <= offence.validator_set_count as usize / 2 {
            return Ok(());
        }

        // Preserve the upstream fraction, deduplication and deferred-slash handling.
        // For 13 of 25 validators, the existing offline fraction is 7%.
        <Offences as ReportOffence<AccountId, Identification, OfflineOffence>>::report_offence(
            reporters, offence,
        )
    }

    fn is_known_offence(offenders: &[Identification], time_slot: &SessionIndex) -> bool {
        <Offences as ReportOffence<AccountId, Identification, OfflineOffence>>::is_known_offence(
            offenders, time_slot,
        )
    }
}
