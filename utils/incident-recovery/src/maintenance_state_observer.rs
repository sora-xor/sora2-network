//! Read-only state-transition checks for real activation or explicit public signed replay.
//! All states come from this invocation's executed and restored externalities.
//! This module neither writes state nor accepts serialized verification claims.
use super::{replay, storage_key, value, BlockWitness, Header, Result};
use codec::{Decode, DecodeAll, Encode};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use sp_runtime::{traits::Header as _, DigestItem};

const INTERVALS: &[u8] = b":sora-nexus:recovery:babe-intervals:v1";
const RESUMED: &[u8] = b":sora-nexus:recovery:liveness-resumed:v1";
const MAX_VALUE: usize = 512 * 1024;
type Authorities = Vec<([u8; 32], u64)>;

fn need(ok: bool, why: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(why.into())
    }
}
fn decode<T: DecodeAll>(raw: &[u8]) -> Result<T> {
    need(raw.len() <= MAX_VALUE, "observer SCALE value bound")?;
    let mut input = raw;
    T::decode_all(&mut input).map_err(|e| e.to_string())
}
fn required<T: DecodeAll>(
    ext: &mut sp_io::TestExternalities,
    pallet: &[u8],
    item: &[u8],
) -> Result<T> {
    decode(&value(ext, &storage_key(pallet, item)).ok_or("observer missing required state")?)
}

// Exact stable2603/stable2609 wire layouts. Runtime API and digest checks below
// bind the decoded fields to the Wasm actually executed in this invocation.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct Config {
    c: (u64, u64),
    allowed: u8,
}
impl Config {
    fn validate(&self) -> Result<()> {
        need(
            self.c.1 != 0 && self.c.0 <= self.c.1 && self.allowed <= 2,
            "observer invalid BABE configuration",
        )
    }
}
#[derive(Decode)]
struct Configuration {
    slot_duration: u64,
    epoch_length: u64,
    c: (u64, u64),
    authorities: Authorities,
    randomness: [u8; 32],
    allowed: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct Interval {
    first: u64,
    last: u64,
    session: u32,
    authorities: Vec<[u8; 32]>,
}
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct Intervals {
    policy: [u8; 32],
    intervals: Vec<Interval>,
}

#[derive(Clone)]
struct State {
    epoch: u64,
    slot: u64,
    genesis_slot: u64,
    epoch_start: (u32, u32),
    session: u32,
    current_era: Option<Vec<u8>>,
    active_era: Option<Vec<u8>>,
    authorities: Authorities,
    next_authorities: Authorities,
    randomness: [u8; 32],
    next_randomness: [u8; 32],
    config: Config,
    next_config: Option<Config>,
    pending: Option<Vec<u8>>,
    repair_marker: Option<Vec<u8>>,
    intervals: Option<Intervals>,
    skipped: Vec<(u64, u32)>,
    tracking: Option<(u32, Vec<u32>)>,
    resumed: Option<u32>,
}
fn read(ext: &mut sp_io::TestExternalities) -> Result<State> {
    let optional =
        |ext: &mut sp_io::TestExternalities, pallet, item| value(ext, &storage_key(pallet, item));
    let state = State {
        epoch: required(ext, b"Babe", b"EpochIndex")?,
        slot: required(ext, b"Babe", b"CurrentSlot")?,
        genesis_slot: required(ext, b"Babe", b"GenesisSlot")?,
        epoch_start: required(ext, b"Babe", b"EpochStart")?,
        session: required(ext, b"Session", b"CurrentIndex")?,
        current_era: optional(ext, b"Staking", b"CurrentEra"),
        active_era: optional(ext, b"Staking", b"ActiveEra"),
        authorities: required(ext, b"Babe", b"Authorities")?,
        next_authorities: required(ext, b"Babe", b"NextAuthorities")?,
        randomness: required(ext, b"Babe", b"Randomness")?,
        next_randomness: required(ext, b"Babe", b"NextRandomness")?,
        config: required(ext, b"Babe", b"EpochConfig")?,
        next_config: optional(ext, b"Babe", b"NextEpochConfig")
            .map(|v| decode(&v))
            .transpose()?,
        pending: optional(ext, b"Babe", b"PendingEpochConfigChange"),
        repair_marker: optional(ext, b"Babe", b"MainnetPlainConfigRepairV1"),
        intervals: value(ext, INTERVALS).map(|v| decode(&v)).transpose()?,
        skipped: optional(ext, b"Babe", b"SkippedEpochs")
            .map(|v| decode(&v))
            .transpose()?
            .unwrap_or_default(),
        tracking: optional(ext, b"Liveness", b"DisabledDuringSession")
            .map(|v| decode(&v))
            .transpose()?,
        resumed: value(ext, RESUMED).map(|v| decode(&v)).transpose()?,
    };
    state.config.validate()?;
    if let Some(config) = &state.next_config {
        config.validate()?;
    }
    for list in [&state.authorities, &state.next_authorities] {
        need(
            !list.is_empty()
                && list.len() <= 1000
                && list.iter().all(|(k, w)| *k != [0; 32] && *w > 0),
            "observer authority bounds",
        )?;
        need(
            list.iter()
                .map(|(k, _)| k)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == list.len(),
            "observer duplicate authority",
        )?;
    }
    need(
        state.skipped.len() <= 100 && state.skipped.windows(2).all(|p| p[0].0 < p[1].0),
        "observer skipped epoch bounds/order",
    )?;
    if let Some((_, disabled)) = &state.tracking {
        need(
            disabled.len() <= 1000
                && disabled.windows(2).all(|p| p[0] < p[1])
                && disabled.iter().all(|i| *i < 1000),
            "observer liveness tracking bounds",
        )?;
    }
    Ok(state)
}
fn pending_config(raw: &[u8]) -> Result<Config> {
    need(
        raw.first() == Some(&1),
        "observer unknown BABE next config version",
    )?;
    let config: Config = decode(&raw[1..])?;
    config.validate()?;
    Ok(config)
}
fn configuration(
    executor: &replay::Executor,
    ext: &mut sp_io::TestExternalities,
    code: &[u8],
    state: &State,
) -> Result<Configuration> {
    let root = replay::root(ext);
    let raw = replay::call(executor, ext, code, "BabeApi_configuration", &[])?;
    let api: Configuration = decode(&raw)?;
    need(
        replay::root(ext) == root
            && api.slot_duration > 0
            && api.epoch_length > 0
            && api.epoch_length <= 1_000_000
            && api.c == state.config.c
            && api.allowed == state.config.allowed
            && api.authorities == state.authorities
            && api.randomness == state.randomness,
        "observer actual BABE API/state mismatch",
    )?;
    Ok(api)
}

#[allow(clippy::too_many_arguments)]
fn epoch_transition(
    before: &State,
    after: &State,
    number: u32,
    slot: u64,
    duration: u64,
    maintenance: bool,
    first_normal: bool,
    header: &Header,
) -> Result<bool> {
    let epoch = slot
        .checked_sub(before.genesis_slot)
        .ok_or("observer slot predates genesis")?
        / duration;
    need(
        after.genesis_slot == before.genesis_slot
            && after.slot == slot
            && after.epoch == epoch
            && epoch >= before.epoch,
        "observer epoch/slot equation",
    )?;
    let changed = epoch != before.epoch;
    let mut effective_pending = before.pending.clone();
    // The reviewed one-time mainnet repair can queue only this exact descriptor.
    // It runs before the first normal block's BABE/session initialization.
    if first_normal && before.repair_marker.is_none() {
        need(
            after.repair_marker.as_deref() == Some(&[1][..]),
            "observer mainnet repair marker missing",
        )?;
        let legacy = Config {
            c: (1, 4),
            allowed: 2,
        };
        if before.config == legacy
            && before.next_config.as_ref() == Some(&legacy)
            && effective_pending.is_none()
        {
            effective_pending = Some(
                [
                    vec![1],
                    Config {
                        c: (1, 4),
                        allowed: 1,
                    }
                    .encode(),
                ]
                .concat(),
            );
        }
    } else {
        need(
            after.repair_marker == before.repair_marker,
            "observer repair marker changed outside first normal block",
        )?;
    }
    let mut epoch_logs = Vec::new();
    let mut config_logs = Vec::new();
    for log in &header.digest().logs {
        if let DigestItem::Consensus(engine, data) = log {
            if engine == b"BABE" {
                match data.first() {
                    Some(1) => epoch_logs.push(decode::<(Authorities, [u8; 32])>(&data[1..])?),
                    Some(3) => config_logs.push(pending_config(&data[1..])?),
                    Some(2) => {
                        let index: u32 = decode(&data[1..])?;
                        need(
                            (index as usize) < after.authorities.len(),
                            "observer disabled authority index",
                        )?;
                    }
                    _ => return Err("observer unknown BABE consensus digest".into()),
                }
            }
        }
    }
    if changed {
        need(
            after.epoch_start == (before.epoch_start.1, number)
                && after.authorities == before.next_authorities
                && after.randomness == before.next_randomness,
            "observer announced epoch transition differs",
        )?;
        need(
            epoch_logs == vec![(after.next_authorities.clone(), after.next_randomness)],
            "observer next epoch digest differs from stored announcement",
        )?;
        need(
            after.config
                == before
                    .next_config
                    .clone()
                    .unwrap_or_else(|| before.config.clone())
                && after.pending.is_none(),
            "observer current config transition differs",
        )?;
        let pending = effective_pending
            .as_ref()
            .map(|v| pending_config(v))
            .transpose()?;
        need(
            after.next_config == pending.clone().or_else(|| before.next_config.clone())
                && config_logs == pending.into_iter().collect::<Vec<_>>(),
            "observer next config/digest transition differs",
        )?;
        if maintenance {
            need(
                after.next_authorities == before.next_authorities,
                "observer maintenance changed announced next authorities",
            )?;
        }
    } else {
        need(
            after.epoch_start == before.epoch_start
                && after.authorities == before.authorities
                && after.next_authorities == before.next_authorities
                && after.randomness == before.randomness
                && after.next_randomness == before.next_randomness
                && after.config == before.config
                && after.next_config == before.next_config
                && after.pending == effective_pending
                && epoch_logs.is_empty()
                && config_logs.is_empty(),
            "observer mid-epoch announcement/config change",
        )?;
    }
    let mut skipped = before.skipped.clone();
    let resumed = changed && !maintenance && before.resumed.is_none();
    if changed && (before.epoch.checked_add(1) != Some(epoch) || resumed) {
        need(
            epoch >= u64::from(after.session),
            "observer epoch precedes mapped session",
        )?;
        if skipped.len() == 100 {
            skipped.remove(0);
        }
        skipped.push((epoch, after.session));
    }
    need(
        after.skipped == skipped,
        "observer skipped-epoch history/offset changed",
    )?;
    Ok(changed)
}
fn intervals(before: Option<&Intervals>, after: &State, policy: [u8; 32]) -> Result<()> {
    let record = after
        .intervals
        .as_ref()
        .ok_or("observer maintenance attribution absent")?;
    need(
        record.policy == policy && !record.intervals.is_empty() && record.intervals.len() <= 8,
        "observer maintenance attribution identity/bounds",
    )?;
    need(
        record.intervals.iter().all(|r| {
            r.first <= r.last
                && r.session == after.session
                && !r.authorities.is_empty()
                && r.authorities.len() <= 1000
        }) && record.intervals.windows(2).all(|p| p[0].last < p[1].first),
        "observer overlapping/invalid maintenance intervals",
    )?;
    let last = record
        .intervals
        .last()
        .ok_or("observer attribution missing")?;
    let keys: Vec<_> = after.authorities.iter().map(|(k, _)| *k).collect();
    need(
        last.last == after.epoch && last.authorities == keys,
        "observer current epoch not attributed to exact frozen authorities",
    )?;
    if let Some(prior) = before {
        let mut expected = prior.clone();
        let previous = expected
            .intervals
            .last_mut()
            .ok_or("observer prior intervals empty")?;
        if previous.authorities == keys {
            previous.last = after.epoch;
        } else {
            expected.intervals.push(Interval {
                first: after.epoch,
                last: after.epoch,
                session: after.session,
                authorities: keys,
            });
        }
        need(
            expected == *record,
            "observer maintenance interval history rewritten",
        )?;
    } else {
        need(
            record.intervals
                == vec![Interval {
                    first: after.epoch,
                    last: after.epoch,
                    session: after.session,
                    authorities: keys,
                }],
            "observer first interval includes unobserved epochs",
        )?;
    }
    Ok(())
}

fn offences_digest(ext: &mut sp_io::TestExternalities) -> Result<String> {
    ext.execute_with(|| {
        let prefix = sp_io::hashing::twox_128(b"Offences");
        let mut key = sp_io::storage::next_key(&prefix);
        let mut hash = Sha256::new();
        let mut count = 0usize;
        while let Some(current) = key {
            if !current.starts_with(&prefix) {
                break;
            }
            let bytes = sp_io::storage::get(&current).ok_or("observer offence row disappeared")?;
            count += 1;
            need(
                count <= 1_000_000 && bytes.len() <= MAX_VALUE,
                "observer offence inventory bound",
            )?;
            hash.update((current.len() as u64).to_le_bytes());
            hash.update(&current);
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(&bytes);
            key = sp_io::storage::next_key(&current);
            need(
                key.as_ref().is_none_or(|k| k > &current),
                "observer offence inventory order",
            )?;
        }
        Ok(hex::encode(hash.finalize()))
    })
}

#[cfg(feature = "bridge-fixture")]
fn ordinary_staking_equations(ext: &mut sp_io::TestExternalities) -> Result<Value> {
    use framenode_runtime::Runtime;
    ext.execute_with(|| {
        let active=pallet_staking::ActiveEra::<Runtime>::get().ok_or("observer async active era absent")?.index;
        let planned=pallet_staking::CurrentEra::<Runtime>::get().ok_or("observer async planned era absent")?;
        need(planned==active || active.checked_add(1)==Some(planned),"observer async active/planned era gap")?;
        let duration=<Runtime as pallet_staking::Config>::BondingDuration::get();
        need(duration<=1000,"observer bonded-era bound")?;
        let bonded=pallet_staking::BondedEras::<Runtime>::get();
        need(bonded.iter().map(|(era,_)|*era).eq(active.saturating_sub(duration)..=active)
            && bonded.windows(2).all(|p|p[0].1<p[1].1),"observer bonded-era range/session history")?;
        let points=pallet_staking::ErasRewardPoints::<Runtime>::get(active);
        need(points.individual.len()<=1000,"observer reward points bound")?;
        let total=points.individual.values().try_fold(0u32,|sum,value|sum.checked_add(*value).ok_or("observer reward points overflow"))?;
        need(total==points.total,"observer reward points total differs from individual sum")?;
        let mut weights=std::collections::BTreeMap::new();let mut sum=0u128;
        for (account,weight) in pallet_staking::ErasValidatorIncentiveWeight::<Runtime>::iter_prefix(active) {
            need(weights.len()<1000,"observer incentive weight bound")?;
            sum=sum.checked_add(weight).ok_or("observer incentive sum overflow")?;
            need(weights.insert(account,weight).is_none(),"observer duplicate incentive weight")?;
        }
        need(sum==pallet_staking::ErasSumValidatorIncentiveWeight::<Runtime>::get(active),"observer incentive denominator differs from weight sum")?;
        let cutoff=pallet_staking::WeightedPointsFormulaStartEra::<Runtime>::get().ok_or("observer async weighted-points cutoff missing")?;
        let weighted=if active>=cutoff {
            let sum=points.individual.iter().try_fold(0u128,|sum,(account,points)| {
                let value=weights.get(account).copied().unwrap_or(0).checked_mul(u128::from(*points)).ok_or("observer weighted points multiplication overflow")?;
                sum.checked_add(value).ok_or("observer weighted points sum overflow")
            })?;
            need(sum==pallet_staking::ErasSumWeightedPoints::<Runtime>::get(active),"observer weighted-points denominator differs")?;
            Some(sum)
        } else {None};
        Ok(json!({"activeEra":active,"plannedEra":planned,"bondedEraRangeAndSessionOrderVerified":true,
            "rewardPointsSum":total,"incentiveWeightSum":sum.to_string(),"weightedPointsFormulaStartEra":cutoff,
            "weightedPointsSum":weighted.map(|v|v.to_string()),"weightedPointsFormulaExercised":weighted.is_some(),
            "fullRewardDistributionVerified":false,"electionOptimalityVerified":false,"fullExposureCoverageVerified":false}))
    })
}

pub(super) struct Track {
    nonfixture: bool,
    policy: [u8; 32],
    duration: u64,
    slot_duration: u64,
    checkpoint: State,
    previous: State,
    rows: Vec<Value>,
    maintenance_changes: usize,
    normal_blocks: usize,
    boundary: Option<u32>,
    tracking_followups: usize,
    before_offences: Option<String>,
}
impl Track {
    pub fn new(
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        policy: [u8; 32],
    ) -> Result<Self> {
        let state = read(ext)?;
        let api = configuration(executor, ext, code, &state)?;
        need(
            state.intervals.is_none() && state.resumed.is_none(),
            "observer checkpoint already has recovery state",
        )?;
        Ok(Self {
            nonfixture: false,
            policy,
            duration: api.epoch_length,
            slot_duration: api.slot_duration,
            checkpoint: state.clone(),
            previous: state,
            rows: Vec::new(),
            maintenance_changes: 0,
            normal_blocks: 0,
            boundary: None,
            tracking_followups: 0,
            before_offences: None,
        })
    }
    /// Real release observations are read-only and never use public signing keys.
    pub fn new_nonfixture(
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        policy: [u8; 32],
    ) -> Result<Self> {
        let mut track = Self::new(executor, ext, code, policy)?;
        track.nonfixture = true;
        Ok(track)
    }
    pub fn before_block(
        &mut self,
        ext: &mut sp_io::TestExternalities,
        slot: u64,
        maintenance: bool,
    ) -> Result<()> {
        let next = slot
            .checked_sub(self.previous.genesis_slot)
            .ok_or("observer slot predates genesis")?
            / self.duration;
        self.before_offences =
            if !maintenance && self.previous.resumed.is_none() && next > self.previous.epoch {
                Some(offences_digest(ext)?)
            } else {
                None
            };
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn accepted(
        &mut self,
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        witness: &BlockWitness,
        header: &Header,
        slot: u64,
        maintenance: bool,
    ) -> Result<()> {
        let root = replay::root(ext);
        need(
            format!("{root:#x}") == witness.state_root
                && format!("{:#x}", header.hash()) == witness.hash
                && witness.independent_import_root_matches
                && witness.disk_restart_root_matches,
            "observer restored block witness differs",
        )?;
        let after = read(ext)?;
        let api = configuration(executor, ext, code, &after)?;
        need(
            api.epoch_length == self.duration && api.slot_duration == self.slot_duration,
            "observer BABE runtime constant changed",
        )?;
        let changed = epoch_transition(
            &self.previous,
            &after,
            witness.number,
            slot,
            self.duration,
            maintenance,
            self.normal_blocks == 0 && !maintenance,
            header,
        )?;
        if maintenance {
            need(
                after.session == self.checkpoint.session
                    && after.current_era == self.checkpoint.current_era
                    && after.active_era == self.checkpoint.active_era
                    && after.resumed.is_none(),
                "observer economic progression during maintenance",
            )?;
            intervals(self.previous.intervals.as_ref(), &after, self.policy)?;
            self.maintenance_changes += usize::from(changed);
        } else {
            self.normal_blocks += 1;
            need(
                after.intervals == self.previous.intervals,
                "observer normal execution rewrote maintenance attribution",
            )?;
            if changed {
                need(
                    after.session
                        == self
                            .previous
                            .session
                            .checked_add(1)
                            .ok_or("observer session overflow")?,
                    "observer normal epoch/session rotation differs",
                )?;
                if self.previous.resumed.is_none() {
                    need(
                        after.resumed == Some(witness.number)
                            && after.skipped.last() == Some(&(after.epoch, after.session)),
                        "observer missing deferred liveness/resume anchor",
                    )?;
                    need(
                        self.before_offences.as_ref() == Some(&offences_digest(ext)?),
                        "observer deferred boundary changed Offences state despite grace",
                    )?;
                    need(
                        after
                            .tracking
                            .as_ref()
                            .is_some_and(|(session, _)| *session == after.session),
                        "observer resumed tracking missing",
                    )?;
                    self.boundary = Some(witness.number);
                } else {
                    need(
                        after.resumed == self.previous.resumed,
                        "observer liveness grace repeated",
                    )?;
                }
            } else {
                need(
                    after.session == self.previous.session
                        && after.resumed == self.previous.resumed,
                    "observer session/grace changed without epoch",
                )?;
                if self.boundary.is_some() {
                    need(
                        after
                            .tracking
                            .as_ref()
                            .is_some_and(|(session, _)| *session == after.session),
                        "observer tracking did not continue",
                    )?;
                    self.tracking_followups += 1;
                }
            }
        }
        #[cfg(feature = "bridge-fixture")]
        let staking_equations = if maintenance || self.nonfixture {
            None
        } else {
            Some(ordinary_staking_equations(ext)?)
        };
        #[cfg(not(feature = "bridge-fixture"))]
        let staking_equations: Option<Value> = None;
        need(replay::root(ext) == root, "observer changed runtime state")?;
        self.rows.push(json!({"block":witness.number,"blockHash":witness.hash,"stateRoot":witness.state_root,
            "snapshotManifestSha256":witness.disk_snapshot_manifest_sha256,"executingCodeBlake2":witness.executing_code_blake2,
            "maintenance":maintenance,"epoch":after.epoch,"session":after.session,"slot":slot,"epochChanged":changed,
            "currentEraScale":after.current_era.as_ref().map(hex::encode),"activeEraScale":after.active_era.as_ref().map(hex::encode),
            "configScale":hex::encode(after.config.encode()),"nextConfigScale":after.next_config.as_ref().map(|c|hex::encode(c.encode())),
            "pendingConfigScale":after.pending.as_ref().map(hex::encode),"intervalsScale":after.intervals.as_ref().map(|v|hex::encode(v.encode())),
            "skippedEpochs":after.skipped,"livenessResumedAt":after.resumed,"tracking":after.tracking,
            "ordinaryStakingEquations":staking_equations,"readOnlyStateRootMatches":true}));
        self.previous = after;
        Ok(())
    }
    #[cfg(feature = "bridge-fixture")]
    pub fn finish(self, signed: &Value) -> Result<Value> {
        need(
            !self.nonfixture,
            "nonfixture observer cannot accept public signed claims",
        )?;
        need(
            signed["fixtureOnly"] == true
                && signed["realSignaturesExecuted"] == true
                && signed["storageOverridesUsed"] == false,
            "observer signed fixture result absent",
        )?;
        need(
            signed.get("ledgerEquations").is_some(),
            "observer fresh signed ledger equations absent",
        )?;
        Ok(self.finish_observations(
            "sora-nexus-public-fixture-maintenance-state-observations-v1",
            true,
            signed["ledgerEquations"].clone(),
        ))
    }
    pub fn finish_nonfixture(self) -> Result<Value> {
        need(
            self.nonfixture && self.normal_blocks > 0 && !self.rows.is_empty(),
            "nonfixture observer requires completed ordinary execution",
        )?;
        Ok(self.finish_observations(
            "sora-nexus-nonfixture-maintenance-state-observations-v1",
            false,
            Value::Null,
        ))
    }
    fn finish_observations(self, schema: &str, fixture_only: bool, ledger: Value) -> Value {
        json!({"schema":schema,"fixtureOnly":fixture_only,
            "epochDurationFromExecutedWasm":self.duration,"slotDurationFromExecutedWasm":self.slot_duration,
            "maintenanceEpochTransitionsVerified":self.maintenance_changes,"maintenanceEpochBoundaryExercised":self.maintenance_changes>0,
            "frozenEconomicSessionAndEraVerified":true,"announcedBabeTransitionsVerified":true,"boundedMaintenanceEpochAttributionVerified":true,
            "deferredBoundaryBlock":self.boundary,"deferredGraceStateEffectsVerified":self.boundary.is_some(),
            "postBoundaryTrackingBlocks":self.tracking_followups,"trackingContinuationExercised":self.tracking_followups>0,
            "signedStakingLedgerEquations":ledger,"rows":self.rows,
            "fullElectionCorrectnessVerified":false,"fullExposureAndRewardCorrectnessVerified":false,"heartbeatSignaturesVerified":false,
            "maintenanceEpochLivenessAndStakingVerified":false,"consensusSealsVerified":false,"operatorApprovalVerified":false,"productionReady":false})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::H256;
    use sp_runtime::generic::Digest;

    fn observed_track(nonfixture: bool, normal_blocks: usize) -> Track {
        let state = state();
        Track {
            nonfixture,
            policy: [1; 32],
            duration: 600,
            slot_duration: 6000,
            checkpoint: state.clone(),
            previous: state,
            rows: if normal_blocks > 0 {
                vec![json!({"syntheticUnitTest":true})]
            } else {
                vec![]
            },
            maintenance_changes: 0,
            normal_blocks,
            boundary: None,
            tracking_followups: 0,
            before_offences: None,
        }
    }
    #[test]
    fn nonfixture_finish_keeps_unexercised_and_signed_scope_explicit() {
        assert!(observed_track(false, 1).finish_nonfixture().is_err());
        assert!(observed_track(true, 0).finish_nonfixture().is_err());
        let report = observed_track(true, 1).finish_nonfixture().unwrap();
        assert_eq!(
            report["schema"],
            "sora-nexus-nonfixture-maintenance-state-observations-v1"
        );
        assert_eq!(report["fixtureOnly"], false);
        assert!(report["signedStakingLedgerEquations"].is_null());
        assert!(report["deferredBoundaryBlock"].is_null());
        for key in [
            "maintenanceEpochBoundaryExercised",
            "deferredGraceStateEffectsVerified",
            "trackingContinuationExercised",
            "fullElectionCorrectnessVerified",
            "fullExposureAndRewardCorrectnessVerified",
            "heartbeatSignaturesVerified",
            "maintenanceEpochLivenessAndStakingVerified",
            "productionReady",
        ] {
            assert_eq!(report[key], false);
        }
    }
    #[cfg(feature = "bridge-fixture")]
    #[test]
    fn nonfixture_observer_never_accepts_public_signed_projection() {
        let signed = json!({"fixtureOnly":true,"realSignaturesExecuted":true,
            "storageOverridesUsed":false,"ledgerEquations":{"syntheticUnitTest":true}});
        assert!(observed_track(true, 1).finish(&signed).is_err());
        let report = observed_track(false, 1).finish(&signed).unwrap();
        assert_eq!(
            report["schema"],
            "sora-nexus-public-fixture-maintenance-state-observations-v1"
        );
        assert_eq!(report["fixtureOnly"], true);
        assert_eq!(
            report["signedStakingLedgerEquations"],
            signed["ledgerEquations"]
        );
    }

    #[cfg(feature = "bridge-fixture")]
    #[test]
    fn ordinary_reward_denominators_and_bonded_history_are_recomputed() {
        use framenode_runtime::{AccountId, Runtime};
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            let active = 30u32;
            let owner = AccountId::new([9; 32]);
            pallet_staking::ActiveEra::<Runtime>::put(pallet_staking::ActiveEraInfo {
                index: active,
                start: None,
            });
            pallet_staking::CurrentEra::<Runtime>::put(active);
            let duration = <Runtime as pallet_staking::Config>::BondingDuration::get();
            let bonded: Vec<_> = (active - duration..=active)
                .map(|era| (era, era * 6))
                .collect();
            pallet_staking::BondedEras::<Runtime>::put(
                frame_support::BoundedVec::<_, pallet_staking::BondedErasBound<Runtime>>::try_from(
                    bonded,
                )
                .unwrap(),
            );
            let mut points = pallet_staking::EraRewardPoints::<Runtime> {
                total: 5,
                ..Default::default()
            };
            points.individual.try_insert(owner.clone(), 5).unwrap();
            pallet_staking::ErasRewardPoints::<Runtime>::insert(active, points);
            pallet_staking::ErasValidatorIncentiveWeight::<Runtime>::insert(active, owner, 3u128);
            pallet_staking::ErasSumValidatorIncentiveWeight::<Runtime>::insert(active, 3u128);
            pallet_staking::WeightedPointsFormulaStartEra::<Runtime>::put(active + 1);
        });
        assert_eq!(
            ordinary_staking_equations(&mut ext).unwrap()["weightedPointsFormulaExercised"],
            false
        );
        ext.execute_with(|| pallet_staking::WeightedPointsFormulaStartEra::<Runtime>::put(30));
        assert!(ordinary_staking_equations(&mut ext).is_err());
        ext.execute_with(|| pallet_staking::ErasSumWeightedPoints::<Runtime>::insert(30, 15u128));
        assert_eq!(
            ordinary_staking_equations(&mut ext).unwrap()["weightedPointsSum"],
            "15"
        );
        ext.execute_with(|| {
            pallet_staking::ErasRewardPoints::<Runtime>::mutate(30, |p| p.total = 6)
        });
        assert!(ordinary_staking_equations(&mut ext).is_err());
    }

    fn state() -> State {
        State {
            epoch: 10,
            slot: 100,
            genesis_slot: 0,
            epoch_start: (1, 2),
            session: 7,
            current_era: Some(2u32.encode()),
            active_era: Some((2u32, None::<u64>).encode()),
            authorities: vec![([1; 32], 1)],
            next_authorities: vec![([1; 32], 1)],
            randomness: [2; 32],
            next_randomness: [3; 32],
            config: Config {
                c: (1, 4),
                allowed: 1,
            },
            next_config: Some(Config {
                c: (1, 4),
                allowed: 1,
            }),
            pending: None,
            repair_marker: Some(vec![1]),
            intervals: None,
            skipped: vec![(10, 7)],
            tracking: Some((7, vec![])),
            resumed: None,
        }
    }
    fn header(next: &State, config: Option<Config>) -> Header {
        let mut logs = vec![DigestItem::Consensus(
            *b"BABE",
            [
                vec![1],
                (next.next_authorities.clone(), next.next_randomness).encode(),
            ]
            .concat(),
        )];
        if let Some(config) = config {
            logs.push(DigestItem::Consensus(
                *b"BABE",
                [vec![3, 1], config.encode()].concat(),
            ));
        }
        Header::new(3, H256::zero(), H256::zero(), H256::zero(), Digest { logs })
    }
    #[test]
    fn maintenance_epoch_uses_announced_values_and_rejects_rewritten_history() {
        let before = state();
        let mut after = before.clone();
        after.epoch = 12;
        after.slot = 120;
        after.epoch_start = (2, 3);
        after.randomness = before.next_randomness;
        after.next_randomness = [4; 32];
        after.skipped.push((12, 7));
        let h = header(&after, None);
        assert!(epoch_transition(&before, &after, 3, 120, 10, true, false, &h).unwrap());
        after.skipped.remove(0);
        assert!(epoch_transition(&before, &after, 3, 120, 10, true, false, &h).is_err());
        after = before.clone();
        after.epoch = 11;
        after.slot = 110;
        after.epoch_start = (2, 3);
        after.randomness = [9; 32];
        assert!(epoch_transition(
            &before,
            &after,
            3,
            110,
            10,
            true,
            false,
            &header(&after, None)
        )
        .is_err());
    }
    #[test]
    fn config_transition_requires_exact_announcement_and_no_mid_epoch_change() {
        let mut before = state();
        let next = Config {
            c: (1, 5),
            allowed: 2,
        };
        before.pending = Some([vec![1], next.encode()].concat());
        let mut after = before.clone();
        after.epoch = 11;
        after.slot = 110;
        after.epoch_start = (2, 3);
        after.randomness = before.next_randomness;
        after.next_randomness = [4; 32];
        after.next_config = Some(next.clone());
        after.pending = None;
        assert!(epoch_transition(
            &before,
            &after,
            3,
            110,
            10,
            true,
            false,
            &header(&after, Some(next))
        )
        .unwrap());
        assert!(epoch_transition(
            &before,
            &after,
            3,
            110,
            10,
            true,
            false,
            &header(&after, None)
        )
        .is_err());
        let mut same = before.clone();
        same.slot = 101;
        same.config.allowed = 2;
        let h = Header::new(
            3,
            H256::zero(),
            H256::zero(),
            H256::zero(),
            Digest::default(),
        );
        assert!(epoch_transition(&before, &same, 3, 101, 10, true, false, &h).is_err());
    }
    #[test]
    fn normal_resume_must_append_anchor_even_for_contiguous_epoch() {
        let before = state();
        let mut after = before.clone();
        after.epoch = 11;
        after.slot = 110;
        after.session = 8;
        after.epoch_start = (2, 3);
        after.randomness = before.next_randomness;
        after.next_randomness = [4; 32];
        after.resumed = Some(3);
        let h = header(&after, None);
        assert!(epoch_transition(&before, &after, 3, 110, 10, false, true, &h).is_err());
        after.skipped.push((11, 8));
        assert!(epoch_transition(&before, &after, 3, 110, 10, false, true, &h).unwrap());
    }
    #[test]
    fn maintenance_intervals_are_exact_append_or_extension_only() {
        let mut after = state();
        let p = [8; 32];
        let first = Intervals {
            policy: p,
            intervals: vec![Interval {
                first: 10,
                last: 10,
                session: 7,
                authorities: vec![[1; 32]],
            }],
        };
        after.intervals = Some(first.clone());
        assert!(intervals(None, &after, p).is_ok());
        after.epoch = 12;
        after.intervals.as_mut().unwrap().intervals[0].last = 12;
        assert!(intervals(Some(&first), &after, p).is_ok());
        after.intervals.as_mut().unwrap().intervals[0].first = 9;
        assert!(intervals(Some(&first), &after, p).is_err());
        after.intervals = Some(first);
        after.intervals.as_mut().unwrap().intervals[0].session = 8;
        assert!(intervals(None, &after, p).is_err());
    }
}
