// SPDX-License-Identifier: BSD-4-Clause
//! Local consensus adapter for SORA Nexus asynchronous staking.
//!
//! Keep the existing multi-phase election and its one-session authority queue.
//! Staking plans at the start of session five, the election is consumed while
//! planning session six, and that set becomes active at session six.

use alloc::vec::Vec;
use frame_election_provider_support::{
    data_provider, onchain, BoundedSupportsOf, DataProviderBounds, ElectionDataProvider,
    ElectionProvider, PageIndex, VoterOf,
};
use frame_support::{
    pallet_prelude::*,
    traits::{EstimateNextNewSession, Hooks},
    weights::WeightMeter,
};
use pallet_staking_async_rc_client::{
    AHStakingInterface, Offence, RcClientInterface, SessionReport,
};
use sp_runtime::{traits::Convert, Perbill};
use sp_staking::{
    offence::{OffenceDetails, OnOffenceHandler},
    SessionIndex,
};

use crate::{
    AccountId, Balance, BlockNumber, BlockWeights, ElectionProviderMultiPhase, Historical,
    MaxActiveValidators, Runtime, Session, SessionsPerEra, Staking, System, Timestamp,
};

#[derive(Clone, Default, Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Debug)]
pub struct BridgeState {
    pub pending: Option<(BoundedVec<AccountId, MaxActiveValidators>, u32, Option<u32>)>,
    pub queued_era: Option<(SessionIndex, u32)>,
    pub election_deadline: Option<BlockNumber>,
    pub forced: bool,
    pub timestamp_era: Option<u32>,
}

pub use pallet::*;
#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_system::pallet_prelude::BlockNumberFor;
    #[pallet::config]
    pub trait Config: frame_system::Config {}
    #[pallet::pallet]
    pub struct Pallet<T>(_);
    #[pallet::storage]
    pub type State<T: Config> = StorageValue<_, BridgeState, ValueQuery>;
    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_: BlockNumberFor<T>) {
            // Session rotates before the timestamp inherent. As in synchronous
            // staking, establish the incoming era's start at block finalization.
            if let Some(era) = State::<T>::get().timestamp_era {
                pallet_staking::ActiveEra::<Runtime>::mutate(|active| {
                    if let Some(active) = active.as_mut().filter(|active| active.index == era) {
                        active.start = Some(Timestamp::get());
                    }
                });
                State::<T>::mutate(|state| state.timestamp_era = None);
            }
        }
        fn on_initialize(_: BlockNumberFor<T>) -> Weight {
            // Include the finalization read and worst-case timestamp update.
            T::DbWeight::get()
                .reads_writes(3, 2)
                .saturating_add(Weight::from_parts(
                    0,
                    BridgeState::max_encoded_len() as u64 + 4_096,
                ))
        }
    }
}

/// Seed the already-announced synchronous election without running it again.
pub fn initialize_from_sync(
    active_era: u32,
    planned_era: u32,
    planned_start_session: Option<u32>,
    current_planned_session: u32,
) -> Result<Weight, &'static str> {
    let queued_era = if planned_era == active_era {
        None
    } else if planned_era == active_era.saturating_add(1) {
        let start = planned_start_session.ok_or("missing planned era start")?;
        if start <= Session::current_index() || start != current_planned_session {
            return Err("planned era is not the existing queued session");
        }
        Some((start, planned_era))
    } else {
        return Err("invalid active/planned era gap");
    };
    State::<Runtime>::put(BridgeState {
        queued_era,
        ..Default::default()
    });
    Ok(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(1, 1))
}

pub struct LocalSession;
impl RcClientInterface for LocalSession {
    type AccountId = AccountId;
    fn validator_set(validators: Vec<AccountId>, id: u32, prune_up_to: Option<u32>) {
        // The upstream election exporter accounts for exactly one storage write.
        // Pruning and consuming the report happen inside the session budget.
        let Ok(validators) = BoundedVec::try_from(validators) else {
            log::error!(target: "runtime::staking-async", "Rejected oversized local validator set for era {id}");
            return;
        };
        if validators.is_empty() {
            log::error!(target: "runtime::staking-async", "Election for era {id} returned no local validators; retaining the active set");
            return;
        }
        State::<Runtime>::mutate(|state| {
            // Never replace an announced set or accept another era's result.
            if state.pending.is_none()
                && state.queued_era.is_none()
                && pallet_staking::CurrentEra::<Runtime>::get() == Some(id)
                && pallet_staking::ActiveEra::<Runtime>::get()
                    .is_some_and(|active| active.index.checked_add(1) == Some(id))
            {
                state.pending = Some((validators, id, prune_up_to));
            } else {
                log::error!(target: "runtime::staking-async", "Rejected out-of-order local validator set for era {id}");
            }
        });
    }
}

impl pallet_session::SessionManager<AccountId> for LocalSession {
    fn new_session(index: SessionIndex) -> Option<Vec<AccountId>> {
        pallet_staking::legacy::CurrentPlannedSession::<Runtime>::put(index);
        // Session reserves the entire block for rotation. Poll here before it
        // commits the next queued keys, preserving the final unsigned block and
        // the old synchronous fallback deadline.
        let mut meter = WeightMeter::with_limit(BlockWeights::get().max_block);
        <Staking as Hooks<BlockNumber>>::on_poll(System::block_number(), &mut meter);
        let mut state = State::<Runtime>::get();
        let (validators, era, prune) = state.pending.take()?;
        state.queued_era = Some((index, era));
        pallet_staking::legacy::ErasStartSessionIndex::<Runtime>::insert(era, index);
        state.election_deadline = None;
        State::<Runtime>::put(state);
        if let Some(up_to) = prune {
            Historical::prune_up_to(up_to);
        }
        Some(validators.into_inner())
    }

    fn new_session_genesis(index: SessionIndex) -> Option<Vec<AccountId>> {
        pallet_staking::legacy::CurrentPlannedSession::<Runtime>::put(index);
        if index != 0 {
            return None;
        }
        State::<Runtime>::mutate(|state| {
            state.timestamp_era = pallet_staking::ActiveEra::<Runtime>::get().map(|era| era.index);
        });
        let supports =
            <onchain::OnChainExecution<crate::OnChainSeqPhragmen> as ElectionProvider>::elect(0)
                .ok()?;
        Some(Staking::initialize_local_genesis(supports))
    }

    fn start_session(index: SessionIndex) {
        if index == 0 {
            return;
        }
        let mut state = State::<Runtime>::get();
        let activation = state.queued_era.filter(|(session, _)| *session == index);
        if let Some((_, era)) = activation {
            state.queued_era = None;
            state.timestamp_era = Some(era);
        }
        state.forced = matches!(
            pallet_staking::ForceEra::<Runtime>::get(),
            pallet_staking::Forcing::ForceNew | pallet_staking::Forcing::ForceAlways
        );
        State::<Runtime>::put(state);
        // Authorship writes points directly to the correct active era, so no
        // points are copied or credited a second time in this local report.
        <Staking as AHStakingInterface>::on_relay_session_report(SessionReport::new_terminal(
            index - 1,
            Vec::new(),
            activation.map(|(_, era)| (Timestamp::get(), era)),
        ));
    }
    fn end_session(_: SessionIndex) {}
}

impl
    pallet_session::historical::SessionManager<
        AccountId,
        pallet_staking::Exposure<AccountId, Balance>,
    > for LocalSession
{
    fn new_session(
        index: SessionIndex,
    ) -> Option<Vec<(AccountId, pallet_staking::Exposure<AccountId, Balance>)>> {
        <Self as pallet_session::SessionManager<AccountId>>::new_session(index).map(|accounts| {
            accounts
                .into_iter()
                .map(|account| (account, Default::default()))
                .collect()
        })
    }
    fn new_session_genesis(
        index: SessionIndex,
    ) -> Option<Vec<(AccountId, pallet_staking::Exposure<AccountId, Balance>)>> {
        <Self as pallet_session::SessionManager<AccountId>>::new_session_genesis(index).map(
            |accounts| {
                accounts
                    .into_iter()
                    .map(|account| (account, Default::default()))
                    .collect()
            },
        )
    }
    fn start_session(index: SessionIndex) {
        <Self as pallet_session::SessionManager<AccountId>>::start_session(index);
    }
    fn end_session(index: SessionIndex) {
        <Self as pallet_session::SessionManager<AccountId>>::end_session(index);
    }
}

pub struct DefaultExposureOf;
impl Convert<AccountId, Option<pallet_staking::Exposure<AccountId, Balance>>>
    for DefaultExposureOf
{
    fn convert(account: AccountId) -> Option<pallet_staking::Exposure<AccountId, Balance>> {
        Session::validators()
            .contains(&account)
            .then(Default::default)
    }
}

/// Reports financial offences only. EquivocationReports retains the explicit
/// consensus-disabling policy; offline session finalization cannot disable an author.
pub struct LocalOffences;
impl OnOffenceHandler<AccountId, pallet_session::historical::IdentificationTuple<Runtime>, Weight>
    for LocalOffences
{
    fn on_offence(
        offenders: &[OffenceDetails<
            AccountId,
            pallet_session::historical::IdentificationTuple<Runtime>,
        >],
        fractions: &[Perbill],
        session: SessionIndex,
    ) -> Weight {
        // This handler receives authenticated offence reports. Historical proof
        // roots retain their original identities; financial liabilities and
        // reporter claims follow the recovery's verified effective owner.
        let resolved_accounts =
            offenders
                .iter()
                .zip(fractions)
                .fold(0u64, |count, (details, _)| {
                    count
                        .saturating_add(1)
                        .saturating_add(details.reporters.len() as u64)
                });
        let mut reports: Vec<_> = offenders
            .iter()
            .zip(fractions)
            .map(|(details, fraction)| Offence {
                offender: details.offender.0.clone(),
                reporters: details.reporters.clone(),
                slash_fraction: *fraction,
            })
            .collect();
        crate::incident_recovery::resolve_authenticated_accounts(reports.iter_mut().flat_map(
            |report| core::iter::once(&mut report.offender).chain(report.reporters.iter_mut()),
        ))
        .expect("authenticated offence requires a valid recovery policy");
        <Staking as AHStakingInterface>::on_new_offences(session, reports)
            .saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
            .saturating_add(Weight::from_parts(
                resolved_accounts.saturating_mul(2_000_000),
                128,
            ))
    }
}

/// A predictor is still required by the existing pull-based election pallet.
pub struct LocalElectionData;
impl ElectionDataProvider for LocalElectionData {
    type AccountId = AccountId;
    type BlockNumber = BlockNumber;
    type MaxVotesPerVoter = <Staking as ElectionDataProvider>::MaxVotesPerVoter;
    fn electable_targets(
        bounds: DataProviderBounds,
        page: PageIndex,
    ) -> data_provider::Result<Vec<AccountId>> {
        Staking::electable_targets(bounds, page)
    }
    fn electing_voters(
        bounds: DataProviderBounds,
        page: PageIndex,
    ) -> data_provider::Result<Vec<VoterOf<Self>>> {
        Staking::electing_voters(bounds, page)
    }
    fn electing_voters_stateless(
        bounds: DataProviderBounds,
    ) -> data_provider::Result<Vec<VoterOf<Self>>> {
        Staking::electing_voters_stateless(bounds)
    }
    fn electable_targets_stateless(
        bounds: DataProviderBounds,
    ) -> data_provider::Result<Vec<AccountId>> {
        Staking::electable_targets_stateless(bounds)
    }
    fn desired_targets() -> data_provider::Result<u32> {
        Staking::desired_targets()
    }
    fn next_election_prediction(now: BlockNumber) -> BlockNumber {
        if let Some(deadline) = State::<Runtime>::get().election_deadline {
            return deadline;
        }
        match pallet_staking::ForceEra::<Runtime>::get() {
            pallet_staking::Forcing::ForceNone => return BlockNumber::MAX,
            pallet_staking::Forcing::ForceNew | pallet_staking::Forcing::ForceAlways => return now,
            pallet_staking::Forcing::NotForcing => (),
        }
        let Some(active) = pallet_staking::ActiveEra::<Runtime>::get() else {
            return BlockNumber::MAX;
        };
        let Some((_, start)) = pallet_staking::BondedEras::<Runtime>::get()
            .into_iter()
            .find(|(era, _)| *era == active.index)
        else {
            return BlockNumber::MAX;
        };
        let target = start
            .saturating_add(SessionsPerEra::get())
            .saturating_sub(1);
        let current = Session::current_index();
        if current >= target {
            return now;
        }
        let next = Session::estimate_next_new_session(now).0.unwrap_or(now);
        next.saturating_add(
            target
                .saturating_sub(current)
                .saturating_sub(1)
                .saturating_mul(Session::average_session_length()),
        )
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn add_voter(
        voter: AccountId,
        weight: sp_npos_elections::VoteWeight,
        targets: BoundedVec<AccountId, Self::MaxVotesPerVoter>,
    ) {
        Staking::add_voter(voter, weight, targets);
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn add_target(target: AccountId) {
        Staking::add_target(target);
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn clear() {
        Staking::clear();
    }
    #[cfg(feature = "runtime-benchmarks")]
    fn set_desired_targets(count: u32) {
        Staking::set_desired_targets(count);
    }
}

#[derive(Debug, PartialEq)]
pub enum LocalElectionError {
    Provider(<ElectionProviderMultiPhase as ElectionProvider>::Error),
    TooFewValidators,
}

pub struct LocalElection;
impl ElectionProvider for LocalElection {
    type AccountId = AccountId;
    type BlockNumber = BlockNumber;
    type Error = LocalElectionError;
    type MaxWinnersPerPage = <ElectionProviderMultiPhase as ElectionProvider>::MaxWinnersPerPage;
    type MaxBackersPerWinner =
        <ElectionProviderMultiPhase as ElectionProvider>::MaxBackersPerWinner;
    type MaxBackersPerWinnerFinal =
        <ElectionProviderMultiPhase as ElectionProvider>::MaxBackersPerWinnerFinal;
    type Pages = <ElectionProviderMultiPhase as ElectionProvider>::Pages;
    type DataProvider = Staking;
    fn elect(page: PageIndex) -> Result<BoundedSupportsOf<Self>, Self::Error> {
        frame_support::storage::with_transaction_unchecked(|| {
            let result = <ElectionProviderMultiPhase as ElectionProvider>::elect(page)
                .map_err(LocalElectionError::Provider)
                .and_then(|supports| {
                    let minimum =
                        pallet_staking::legacy::MinimumValidatorCount::<Runtime>::get().max(1);
                    if supports.len() < minimum as usize {
                        Err(LocalElectionError::TooFewValidators)
                    } else {
                        Ok(supports)
                    }
                });
            if result.is_ok() {
                frame_support::storage::TransactionOutcome::Commit(result)
            } else {
                // Preserve the round, submitted solutions and balances for retry.
                frame_support::storage::TransactionOutcome::Rollback(result)
            }
        })
    }
    fn duration() -> BlockNumber {
        <ElectionProviderMultiPhase as ElectionProvider>::duration()
    }
    fn start() -> Result<(), Self::Error> {
        let now = System::block_number();
        let deadline = LocalElectionData::next_election_prediction(now);
        State::<Runtime>::mutate(|state| {
            state.election_deadline = Some(if state.forced { now } else { deadline });
        });
        Ok(())
    }
    fn status() -> Result<Option<Weight>, ()> {
        let state = State::<Runtime>::get();
        let deadline = state.election_deadline.ok_or(())?;
        if System::block_number() < deadline
            || state.pending.is_some()
            || state.queued_era.is_some()
        {
            return Ok(None);
        }
        // Even without an unsigned solution, elect must run signed-finalize
        // and the on-chain fallback, retrying at later sessions after failure.
        // Reserve the whole rotation block, including all adapter reads and
        // bounded validator-set proof sizes; the exporter adds this overhead.
        let overhead = <Runtime as frame_system::Config>::DbWeight::get().reads_writes(3, 2);
        Ok(Some(BlockWeights::get().max_block.saturating_sub(overhead)))
    }
    #[cfg(any(feature = "runtime-benchmarks", feature = "std"))]
    fn asap() {
        <ElectionProviderMultiPhase as ElectionProvider>::asap();
    }
}
