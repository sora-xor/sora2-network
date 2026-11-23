//! Explicit public-key ordinary transactions after automatic recovery activation.
//! This opt-in fixture never changes storage directly or accepts a private key.
use super::*;
use frame_support::{dispatch::DispatchClass, traits::Get, weights::Weight};
use framenode_runtime::{AccountId, Runtime, RuntimeCall, System};
use serde_json::{json, Value};
use sp_core::{ed25519, Pair};
use sp_runtime::{generic::Era, MultiSignature};

const PLAN_SCHEMA: &str = "sora-nexus-public-fixture-staking-transactions-v1";
const MAX_TRANSACTIONS: usize = 128;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Plan {
    schema: String,
    fixture_only: bool,
    policy_commitment: String,
    signer_scheme: String,
    /// Index in the sorted, already published public signing-key list; unrelated to policy council.
    signer_index: usize,
    replacement_ownership_signature: String,
    funding: Vec<Funding>,
    transactions: Vec<Transaction>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Funding {
    operation_index: u32,
    operation_commitment: String,
    minimum_amount: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Transaction {
    step: usize,
    expected_nonce: u32,
    minimum_free_balance: String,
    maximum_fee: String,
    call: Call,
    /// SCALE DispatchError, or null for a successful dispatch. Transaction validity
    /// failures never enter a block and are never accepted as dispatch failures.
    expected_dispatch_error: Option<String>,
    before_ledger: Option<Ledger>,
    after_ledger: Option<Ledger>,
}
#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Call {
    Bond {
        value: String,
    },
    BondExtra {
        value: String,
    },
    Unbond {
        value: String,
    },
    Rebond {
        value: String,
    },
    WithdrawUnbonded {
        num_slashing_spans: u32,
    },
    SetPayeeOwner {},
    PayoutStakers {
        validator_stash: String,
        era: u32,
    },
    PayoutStakersByPage {
        validator_stash: String,
        era: u32,
        page: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ledger {
    stash: String,
    controller: String,
    total: String,
    active: String,
    unlocking: Vec<(String, u32)>,
    legacy_claimed_rewards: Vec<u32>,
    payee_scale: Option<String>,
}

fn amount(value: &str) -> Result<u128> {
    if value.is_empty()
        || value.len() > 39
        || !value.bytes().all(|b| b.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err("noncanonical bounded unsigned balance".into());
    }
    value.parse().map_err(|_| "balance exceeds u128".into())
}
fn account(value: &str) -> Result<AccountId> {
    let bytes: [u8; 32] = hex_bytes(value)?
        .try_into()
        .map_err(|_| "invalid account bytes")?;
    if hx(&bytes) != value {
        return Err("account must use canonical lowercase hex".into());
    }
    Ok(bytes.into())
}
fn hx(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}
fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

impl Call {
    fn runtime(&self, signer: &AccountId) -> Result<RuntimeCall> {
        use pallet_staking::Call as C;
        Ok(RuntimeCall::Staking(match self {
            Self::Bond { value } => C::bond {
                value: amount(value)?,
                payee: pallet_staking::RewardDestination::Account(signer.clone()),
            },
            Self::BondExtra { value } => C::bond_extra {
                max_additional: amount(value)?,
            },
            Self::Unbond { value } => C::unbond {
                value: amount(value)?,
            },
            Self::Rebond { value } => C::rebond {
                value: amount(value)?,
            },
            Self::WithdrawUnbonded { num_slashing_spans } => C::withdraw_unbonded {
                num_slashing_spans: *num_slashing_spans,
            },
            Self::SetPayeeOwner {} => C::set_payee {
                payee: pallet_staking::RewardDestination::Account(signer.clone()),
            },
            Self::PayoutStakers {
                validator_stash,
                era,
            } => C::payout_stakers {
                validator_stash: account(validator_stash)?,
                era: *era,
            },
            Self::PayoutStakersByPage {
                validator_stash,
                era,
                page,
            } => C::payout_stakers_by_page {
                validator_stash: account(validator_stash)?,
                era: *era,
                page: *page,
            },
        }))
    }
}
fn ledger(ext: &mut sp_io::TestExternalities, signer: &AccountId) -> Result<Option<Ledger>> {
    ext.execute_with(|| {
        let Some(controller) = pallet_staking::Bonded::<Runtime>::get(signer) else {
            return Ok(None);
        };
        let value = pallet_staking::Ledger::<Runtime>::get(&controller)
            .ok_or("bonded actor lacks ledger")?;
        if value.stash != *signer {
            return Err("controller ledger points to another stash".into());
        }
        Ok(Some(Ledger {
            stash: hx(value.stash.as_ref()),
            controller: hx(controller.as_ref()),
            total: value.total.to_string(),
            active: value.active.to_string(),
            unlocking: value
                .unlocking
                .iter()
                .map(|c| (c.value.to_string(), c.era))
                .collect(),
            legacy_claimed_rewards: value.legacy_claimed_rewards.to_vec(),
            payee_scale: pallet_staking::Payee::<Runtime>::get(signer).map(|v| hx(&v.encode())),
        }))
    })
}

fn expected_unbond_deadline(ext: &mut sp_io::TestExternalities, signer: &AccountId) -> Result<u32> {
    ext.execute_with(|| {
        let active = pallet_staking::ActiveEra::<Runtime>::get()
            .ok_or("active era missing for signed observation")?
            .index;
        let planned = pallet_staking::CurrentEra::<Runtime>::get().unwrap_or(active);
        let duration = <Runtime as pallet_staking::Config>::BondingDuration::get();
        let recent = pallet_staking::LastValidatorEra::<Runtime>::get(signer)
            .is_some_and(|last| active.saturating_sub(last) < duration);
        let delay = if recent || pallet_staking::AreNominatorsSlashable::<Runtime>::get() {
            duration
        } else {
            <Runtime as pallet_staking::Config>::NominatorFastUnbondDuration::get()
        };
        let base = if <<Runtime as pallet_staking::Config>::SlashDeferOffset as Get<u32>>::get() > 0
        {
            planned
        } else {
            active
        };
        base.checked_add(delay)
            .ok_or_else(|| "observed unbond deadline overflow".into())
    })
}

fn ledger_totals(
    ledger: &Ledger,
    signer: &AccountId,
) -> Result<(u128, u128, std::collections::BTreeMap<u32, u128>)> {
    if account(&ledger.stash)? != *signer || account(&ledger.controller)? != *signer {
        return Err("public replacement ledger owner/controller mismatch".into());
    }
    let total = amount(&ledger.total)?;
    let active = amount(&ledger.active)?;
    let mut chunks = std::collections::BTreeMap::new();
    let mut sum = 0u128;
    let mut previous = None;
    for (value, era) in &ledger.unlocking {
        let value = amount(value)?;
        if value == 0 || previous.is_some_and(|p| p >= *era) || chunks.insert(*era, value).is_some()
        {
            return Err("unlocking chunks must be positive and strictly era-ordered".into());
        }
        sum = sum.checked_add(value).ok_or("unlocking sum overflow")?;
        previous = Some(*era);
    }
    if active.checked_add(sum) != Some(total) {
        return Err("staking total differs from active plus unlocking".into());
    }
    Ok((total, active, chunks))
}

fn ledger_equations(
    call: &Call,
    before: &Option<Ledger>,
    after: &Option<Ledger>,
    success: bool,
    signer: &AccountId,
    deadline: u32,
) -> Result<Value> {
    let prior = before
        .as_ref()
        .map(|v| ledger_totals(v, signer))
        .transpose()?;
    let next = after
        .as_ref()
        .map(|v| ledger_totals(v, signer))
        .transpose()?;
    if !success {
        if before != after {
            return Err("failed public staking call changed its ledger".into());
        }
        return Ok(
            json!({"dispatchSucceeded":false,"ledgerUnchanged":true,"totalEqualsActivePlusUnlocking":true,"callEquationVerified":false}),
        );
    }
    let mut covered = true;
    let name;
    match call {
        Call::Bond { value } => {
            name = "bond";
            let value = amount(value)?;
            if prior.is_some() || next != Some((value, value, Default::default())) {
                return Err("bond does not create the exact funded ledger".into());
            }
            let expected = hx(&pallet_staking::RewardDestination::<AccountId>::Account(
                signer.clone(),
            )
            .encode());
            if after.as_ref().and_then(|l| l.payee_scale.as_ref()) != Some(&expected) {
                return Err("bond payout does not point to replacement owner".into());
            }
        }
        Call::BondExtra { value } => {
            name = "bondExtra";
            let (bt, ba, bu) = prior.as_ref().ok_or("bond_extra prior ledger absent")?;
            let (at, aa, au) = next.as_ref().ok_or("bond_extra resulting ledger absent")?;
            let delta = at.checked_sub(*bt).ok_or("bond_extra reduced total")?;
            if aa.checked_sub(*ba) != Some(delta) || bu != au || delta > amount(value)? {
                return Err("bond_extra active/total/unlocking equation".into());
            }
        }
        Call::Unbond { value } => {
            name = "unbond";
            let (bt, ba, bu) = prior.as_ref().ok_or("unbond prior ledger absent")?;
            let (at, aa, au) = next.as_ref().ok_or("unbond resulting ledger absent")?;
            if bu.len()
                >= <<Runtime as pallet_staking::Config>::MaxUnlockingChunks as Get<u32>>::get()
                    as usize
            {
                return Err(
                    "observer does not cover unbond auto-withdraw at full chunk capacity".into(),
                );
            }
            let ed = <<Runtime as pallet_staking::Config>::Currency as frame_support::traits::fungible::Inspect<AccountId>>::minimum_balance();
            let remainder = ba.saturating_sub(amount(value)?);
            let expected_active = if remainder < ed { 0 } else { remainder };
            if *aa != expected_active {
                return Err(
                    "unbond active amount differs from request and actual existential deposit"
                        .into(),
                );
            }
            let delta = ba.checked_sub(*aa).ok_or("unbond increased active stake")?;
            let mut expected = bu.clone();
            if delta > 0 {
                let old = expected.get(&deadline).copied().unwrap_or(0);
                expected.insert(
                    deadline,
                    old.checked_add(delta)
                        .ok_or("unbond deadline amount overflow")?,
                );
            }
            if bt != at || *au != expected {
                return Err("unbond must preserve total and move active stake to the actual planned-era deadline".into());
            }
        }
        Call::Rebond { value } => {
            name = "rebond";
            let (bt, ba, bu) = prior.as_ref().ok_or("rebond prior ledger absent")?;
            let (at, aa, au) = next.as_ref().ok_or("rebond resulting ledger absent")?;
            let delta = aa.checked_sub(*ba).ok_or("rebond reduced active stake")?;
            if bt != at
                || delta > amount(value)?
                || au
                    .iter()
                    .any(|(era, value)| bu.get(era).is_none_or(|old| value > old))
            {
                return Err("rebond total/active/unlocking equation".into());
            }
        }
        Call::SetPayeeOwner {} => {
            name = "setPayeeOwner";
            if prior != next {
                return Err("set_payee changed stake amounts".into());
            }
            let expected = hx(&pallet_staking::RewardDestination::<AccountId>::Account(
                signer.clone(),
            )
            .encode());
            if after.as_ref().and_then(|l| l.payee_scale.as_ref()) != Some(&expected) {
                return Err("payee is not replacement owner".into());
            }
        }
        Call::WithdrawUnbonded { .. } => {
            name = "withdrawUnbonded";
            covered = false;
        }
        Call::PayoutStakers { .. } | Call::PayoutStakersByPage { .. } => {
            name = "payoutStakers";
            covered = false;
        }
    }
    Ok(
        json!({"call":name,"dispatchSucceeded":true,"totalEqualsActivePlusUnlocking":true,"callEquationVerified":covered,
        "actualUnbondDeadline":if matches!(call,Call::Unbond{..}) {Some(deadline)} else {None},
        "rewardDistributionVerified":false,"productionReady":false}),
    )
}

/// This state is built only after the schema and exact policy admit the public fixture.
/// Its serialized output is diagnostic; no release gate consumes it as a certificate.
pub(super) struct Session {
    plan: Plan,
    plan_sha256: String,
    signer: AccountId,
    pair: ed25519::Pair,
    rows: Vec<Value>,
}
/// Public ordinary signing is never a route to author council escapes. Validate
/// every raw committed envelope before any block execution or signing occurs.
fn normal_actions(base: &Path, steps: &[Step]) -> Result<()> {
    for step in steps {
        if step.fixture_escape.is_some() || step.reject_previous_escapes {
            return Err("public signed fixture forbids council escape fields".into());
        }
        if let Some(reference) = &step.recovery {
            let bytes = replay::artifact(base, reference, recovery::MAX_INHERENT_BYTES)?;
            let envelope = recovery::RecoveryInherent::decode_remark(&bytes)
                .map_err(|e| format!("signed fixture work envelope: {e:?}"))?;
            if matches!(envelope.action, recovery::RecoveryAction::Escape { .. }) {
                return Err("public signed fixture forbids raw council escape actions".into());
            }
        }
    }
    Ok(())
}

impl Session {
    pub fn load(
        base: &Path,
        reference: &replay::Artifact,
        policy: &RecoveryPolicy,
        steps: &[Step],
    ) -> Result<Self> {
        normal_actions(base, steps)?;
        let bytes = replay::artifact(base, reference, 1024 * 1024)?;
        let plan: Plan = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let mut keys = fixture::pairs();
        if plan.schema != PLAN_SCHEMA
            || !plan.fixture_only
            || plan.signer_scheme != fixture::SCHEME
            || hex_bytes(&plan.policy_commitment)?.as_slice() != policy.commitment()
            || plan.signer_index >= keys.len()
            || plan.transactions.is_empty()
            || plan.transactions.len() > MAX_TRANSACTIONS
            || plan.funding.is_empty()
            || plan.funding.len() > 32
            || policy.replacements.len() != 1
        {
            return Err("invalid explicit public signed fixture identity or bounds".into());
        }
        let (signer, pair) = keys.remove(plan.signer_index);
        let mapping = &policy.replacements[0];
        if mapping.new != signer || mapping.old == signer {
            return Err("public signing key must be the exact replacement owner".into());
        }
        let signature_bytes = hex_bytes(&plan.replacement_ownership_signature)?;
        let signature = MultiSignature::decode_all(&mut signature_bytes.as_slice())
            .map_err(|e| e.to_string())?;
        if !recovery::verify_replacement_ownership(mapping, &signature) {
            return Err("replacement public key ownership proof is invalid".into());
        }
        let mut prior = None;
        let mut per_step = BTreeMap::new();
        for tx in &plan.transactions {
            if tx.step >= steps.len()
                || steps[tx.step].recovery.is_some()
                || steps[tx.step].expected_spec != 138
                || steps[tx.step].expected_phase != "Complete"
                || prior.is_some_and(|p: usize| p > tx.step)
                || amount(&tx.minimum_free_balance)? == 0
                || amount(&tx.maximum_fee)? == 0
            {
                return Err("signed transactions require ordered ordinary Complete138 steps and positive budgets".into());
            }
            prior = Some(tx.step);
            let count = per_step.entry(tx.step).or_insert(0usize);
            *count += 1;
            if *count > 4 {
                return Err("at most four public transactions per block".into());
            }
            tx.call.runtime(&signer)?;
            if let Some(error) = &tx.expected_dispatch_error {
                let bytes = hex_bytes(error)?;
                sp_runtime::DispatchError::decode_all(&mut bytes.as_slice())
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut seen = BTreeSet::new();
        for funding in &plan.funding {
            let commitment = hex_bytes(&funding.operation_commitment)?;
            if commitment.len() != 32
                || amount(&funding.minimum_amount)? == 0
                || !seen.insert(funding.operation_index)
                || policy
                    .chunk_hashes
                    .get(funding.operation_index as usize)
                    .map(|h| h.as_slice())
                    != Some(commitment.as_slice())
            {
                return Err(
                    "funding must select unique exact committed incoming operations".into(),
                );
            }
        }
        Ok(Self {
            plan,
            plan_sha256: sha(&bytes),
            signer,
            pair,
            rows: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn apply(
        &mut self,
        step_index: usize,
        number: u32,
        parent: H256,
        executor: &replay::Executor,
        ext: &mut sp_io::TestExternalities,
        code: &[u8],
        version: &sp_version::RuntimeVersion,
        extrinsics: &mut Vec<OpaqueExtrinsic>,
        verify_observations: bool,
    ) -> Result<()> {
        if version.spec_version != 138
            || version.transaction_version != 134
            || status(ext)?.phase != RecoveryPhase::Complete
        {
            return Err(
                "ordinary fixture signer requires actual final138/tx134 Complete execution".into(),
            );
        }
        for tx in self
            .plan
            .transactions
            .iter()
            .filter(|tx| tx.step == step_index)
        {
            let before = ext.execute_with(|| System::account(&self.signer));
            let actual_before_ledger = ledger(ext, &self.signer)?;
            let unbond_deadline = if verify_observations {
                Some(expected_unbond_deadline(ext, &self.signer)?)
            } else {
                None
            };
            if before.nonce != tx.expected_nonce
                || before.data.free < amount(&tx.minimum_free_balance)?
                || actual_before_ledger != tx.before_ledger
            {
                return Err(
                    "public actor nonce, funds or before-ledger differs from committed plan".into(),
                );
            }
            let call = tx.call.runtime(&self.signer)?;
            let (opaque, context) = signed(ext, call, &self.pair, number, parent, version)?;
            let encoded = opaque.encode();
            let before_query = replay::root(ext);
            let info = replay::call(
                executor,
                ext,
                code,
                "TransactionPaymentApi_query_info",
                &(opaque.clone(), encoded.len() as u32).encode(),
            )?;
            let (_, _, fee) = <(Weight, DispatchClass, u128)>::decode_all(&mut info.as_slice())
                .map_err(|e| e.to_string())?;
            if replay::root(ext) != before_query
                || fee == 0
                || fee > amount(&tx.maximum_fee)?
                || before.data.free.saturating_sub(before.data.frozen) < fee
            {
                return Err(
                    "exact Wasm fee query exceeds public fixture funding/budget or changes state"
                        .into(),
                );
            }
            let observed = super::apply(executor, ext, code, &opaque)?
                .map_err(|error| format!("real public transaction is invalid: {error:?}"))?;
            let observed_error = observed.err().map(|error| hx(&error.encode()));
            let after = ext.execute_with(|| System::account(&self.signer));
            let actual_after_ledger = ledger(ext, &self.signer)?;
            if observed_error != tx.expected_dispatch_error
                || after.nonce != before.nonce.checked_add(1).ok_or("nonce overflow")?
                || actual_after_ledger != tx.after_ledger
            {
                return Err(
                    "signed dispatch, nonce advance or after-ledger differs from plan".into(),
                );
            }
            let position = extrinsics.len();
            extrinsics.push(opaque);
            let mut row = json!({"step":step_index,"blockNumber":number,"extrinsicIndex":position,
                "executingCodeBlake2":hx(&sp_io::hashing::blake2_256(code)),"signature":context,
                "feeQueryScale":hx(&info),"quotedFee":fee.to_string(),"dispatchError":observed_error,
                "systemAccountBeforeScale":hx(&before.encode()),"systemAccountAfterScale":hx(&after.encode()),
                "ledgerBefore":tx.before_ledger,"ledgerAfter":tx.after_ledger,
                "postExtrinsicStateRoot":format!("{:#x}",replay::root(ext))});
            if verify_observations {
                row["ledgerEquations"] = ledger_equations(
                    &tx.call,
                    &actual_before_ledger,
                    &actual_after_ledger,
                    observed_error.is_none(),
                    &self.signer,
                    unbond_deadline.ok_or("deadline observation absent")?,
                )?;
            }
            self.rows.push(row);
        }
        Ok(())
    }

    pub fn finish(
        mut self,
        policy: &RecoveryPolicy,
        final_root: H256,
        evidence: &crate::incoming_replay::VerifiedReplay,
        blocks: &[BlockWitness],
    ) -> Result<Value> {
        if self.rows.len() != self.plan.transactions.len() {
            return Err("signed fixture omitted a committed transaction".into());
        }
        let credits = evidence.public_fixture_funding(policy, final_root)?;
        let first = self
            .rows
            .first()
            .and_then(|r| r["blockNumber"].as_u64())
            .ok_or("signed fixture has no executed transactions")?;
        let mut funding_rows = Vec::new();
        for funding in &self.plan.funding {
            let matches: Vec<_> = credits
                .iter()
                .filter(|(operation, _, _)| {
                    operation.index == funding.operation_index
                        && hx(&operation.commitment()) == funding.operation_commitment
                })
                .collect();
            if matches.len() != 1 {
                return Err(
                    "funding operation lacks one opaque accepted native audit proof".into(),
                );
            }
            let (_, audit, audited_block) = matches[0];
            let credit = audit
                .requests
                .iter()
                .flatten()
                .filter(|r| r.beneficiary == self.signer && r.asset_id == common::XOR)
                .try_fold(0u128, |sum, r| {
                    sum.checked_add(r.amount).ok_or("funding amount overflow")
                })?;
            if u64::from(*audited_block) >= first || credit < amount(&funding.minimum_amount)? {
                return Err("funding audit does not precede signed execution or lacks exact replacement credit".into());
            }
            funding_rows.push(json!({"operationIndex":funding.operation_index,"operationCommitment":funding.operation_commitment,
                "incomingAuditCommitment":hx(&audit.commitment()),"auditedBlock":audited_block,"nativeNotifierAmount":credit.to_string()}));
        }
        for row in &mut self.rows {
            let block = blocks
                .iter()
                .find(|b| Some(u64::from(b.number)) == row["blockNumber"].as_u64())
                .ok_or("signed block witness absent")?;
            if !block.independent_import_root_matches
                || !block.disk_restart_root_matches
                || block.resulting_spec != 138
                || block.phase != "Complete"
            {
                return Err(
                    "signed fixture block lacks final-runtime import/restart agreement".into(),
                );
            }
            row["block"] = serde_json::to_value(block).map_err(|e| e.to_string())?;
        }
        let mut report = json!({"schema":"sora-nexus-public-fixture-signed-maintenance-witness-v1","fixtureOnly":true,
            "planSha256":self.plan_sha256,"policyCommitment":hx(&policy.commitment()),"signerScheme":fixture::SCHEME,
            "signer":hx(self.signer.as_ref()),"replacementOwnershipVerified":true,"funding":funding_rows,
            "incomingReplayBinding":evidence.coverage_binding(),"transactions":self.rows,
            "realSignaturesExecuted":true,"storageOverridesUsed":false,"finalStateRoot":format!("{final_root:#x}"),
            "maintenanceEpochLivenessAndStakingVerified":false,"heartbeatSignaturesVerified":false,
            "consensusSealsVerified":false,"externalFinalityVerified":false,"operatorApprovalVerified":false,"productionReady":false});
        if report["transactions"]
            .as_array()
            .is_some_and(|rows| rows.iter().all(|r| r.get("ledgerEquations").is_some()))
        {
            report["ledgerEquations"] = json!({"schema":"sora-nexus-public-staking-ledger-equations-v1",
                "actualBeforeAfterLedgersVerified":true,"plannedEraUnbondingDeadlineVerified":report["transactions"].as_array().is_some_and(|rows|rows.iter().any(|r|r["ledgerEquations"]["actualUnbondDeadline"].is_u64())),
                "calls":report["transactions"].as_array().ok_or("transaction rows absent")?.iter().map(|r|r["ledgerEquations"].clone()).collect::<Vec<_>>(),
                "rewardDistributionVerified":false,"electionAndExposureCorrectnessVerified":false,"productionReady":false});
        }
        Ok(report)
    }
}

fn signed(
    ext: &mut sp_io::TestExternalities,
    call: RuntimeCall,
    pair: &ed25519::Pair,
    number: u32,
    parent: H256,
    version: &sp_version::RuntimeVersion,
) -> Result<(OpaqueExtrinsic, Value)> {
    let signer = AccountId::from(pair.public().0);
    let parent_number = number.checked_sub(1).ok_or("signed block has no parent")?;
    let (genesis, birth, nonce) = ext.execute_with(|| {
        (
            System::block_hash(0),
            System::block_hash(parent_number),
            System::account_nonce(&signer),
        )
    });
    if genesis != H256::from(recovery::GENESIS_HASH) || birth != parent {
        return Err("actual signed genesis or mortal birth header differs".into());
    }
    let era = Era::mortal(64, u64::from(parent_number));
    if era.birth(u64::from(number)) != u64::from(parent_number) {
        return Err("mortal era birth does not select parent".into());
    }
    let extra_bytes = (era, Compact(nonce), Compact(0u128)).encode();
    let extra = framenode_runtime::SignedExtra::decode_all(&mut extra_bytes.as_slice())
        .map_err(|e| e.to_string())?;
    let preimage = [
        call.encode(),
        extra_bytes.clone(),
        version.spec_version.encode(),
        version.transaction_version.encode(),
        genesis.encode(),
        parent.encode(),
    ]
    .concat();
    let payload = if preimage.len() > 256 {
        sp_io::hashing::blake2_256(&preimage).to_vec()
    } else {
        preimage.clone()
    };
    let signature = pair.sign(&payload);
    if !ed25519::Pair::verify(&signature, &payload, &pair.public()) {
        return Err("public signature self-check failed".into());
    }
    let tx = framenode_runtime::UncheckedExtrinsic::new_signed(
        call.clone(),
        signer.clone(),
        MultiSignature::Ed25519(signature),
        extra,
    );
    let bytes = tx.encode();
    Ok((
        OpaqueExtrinsic::decode_all(&mut bytes.as_slice()).map_err(|e| e.to_string())?,
        json!({"signer":hx(signer.as_ref()),"nonce":nonce,"eraScale":hx(&era.encode()),"tip":"0",
            "specVersion":version.spec_version,"transactionVersion":version.transaction_version,"genesis":format!("{genesis:#x}"),
            "birthHeaderHash":format!("{parent:#x}"),"birthHeaderNumber":parent_number,
            "callScale":hx(&call.encode()),"signedExtraScale":hx(&extra_bytes),"signingPreimageSha256":sha(&preimage),"extrinsicSha256":sha(&bytes)}),
    ))
}

#[cfg(test)]
mod tests {
    fn observed_ledger(owner: &AccountId, active: u128, unlocking: Vec<(String, u32)>) -> Ledger {
        let total = active
            + unlocking
                .iter()
                .map(|(v, _)| v.parse::<u128>().unwrap())
                .sum::<u128>();
        Ledger {
            stash: hx(owner.as_ref()),
            controller: hx(owner.as_ref()),
            total: total.to_string(),
            active: active.to_string(),
            unlocking,
            legacy_claimed_rewards: vec![],
            payee_scale: Some(hx(
                &pallet_staking::RewardDestination::<AccountId>::Account(owner.clone()).encode(),
            )),
        }
    }
    #[test]
    fn observed_ledger_totals_reject_wrong_owner_and_inconsistent_debt() {
        let owner = AccountId::new([3; 32]);
        let mut ledger = observed_ledger(&owner, 10, vec![("2".into(), 50)]);
        assert!(ledger_totals(&ledger, &owner).is_ok());
        ledger.total = "99".into();
        assert!(ledger_totals(&ledger, &owner).is_err());
        ledger.total = "12".into();
        assert!(ledger_totals(&ledger, &AccountId::new([4; 32])).is_err());
    }
    #[test]
    fn observed_unbond_deadline_and_rebond_equations_are_not_plan_assertions() {
        let owner = AccountId::new([3; 32]);
        let before = Some(observed_ledger(&owner, 120, vec![]));
        let after = Some(observed_ledger(&owner, 110, vec![("10".into(), 50)]));
        let call = Call::Unbond { value: "10".into() };
        assert!(ledger_equations(&call, &before, &after, true, &owner, 50).is_ok());
        assert!(ledger_equations(&call, &before, &after, true, &owner, 51).is_err());
        assert!(ledger_equations(&call, &before, &after, false, &owner, 50).is_err());
        let rebond = Some(observed_ledger(&owner, 115, vec![("5".into(), 50)]));
        assert!(ledger_equations(
            &Call::Rebond { value: "5".into() },
            &after,
            &rebond,
            true,
            &owner,
            50
        )
        .is_ok());
        assert!(ledger_equations(
            &Call::Rebond { value: "4".into() },
            &after,
            &rebond,
            true,
            &owner,
            50
        )
        .is_err());
    }
    use super::*;
    use sp_runtime::{generic::Preamble, traits::Verify};

    #[test]
    fn bounded_balances_and_calls_reject_ambiguous_inputs() {
        for value in [
            "",
            "01",
            "+1",
            "-1",
            "1.0",
            " 1",
            "340282366920938463463374607431768211456",
        ] {
            assert!(amount(value).is_err(), "{value}");
        }
        assert_eq!(
            amount("340282366920938463463374607431768211455").unwrap(),
            u128::MAX
        );
        for value in [
            json!({"kind":"sudo"}),
            json!({"kind":"bond","value":"1","privateKey":"x"}),
            json!({"kind":"setPayeeOwner","owner":"x"}),
        ] {
            assert!(
                serde_json::from_value::<Call>(value.clone()).is_err(),
                "{value}"
            );
        }
        assert!(matches!(
            serde_json::from_value::<Call>(json!({"kind":"setPayeeOwner"})).unwrap(),
            Call::SetPayeeOwner {}
        ));
        let call: Call =
            serde_json::from_value(json!({"kind":"withdrawUnbonded","numSlashingSpans":0}))
                .unwrap();
        assert!(matches!(
            call,
            Call::WithdrawUnbonded {
                num_slashing_spans: 0
            }
        ));
    }

    #[test]
    fn real_signature_binds_runtime_genesis_birth_nonce_and_call() {
        let (account, pair) = fixture::pairs().remove(0);
        let parent = H256::repeat_byte(73);
        let number = recovery::CHECKPOINT_NUMBER + 10;
        let mut ext = sp_io::TestExternalities::default();
        ext.execute_with(|| {
            frame_system::BlockHash::<Runtime>::insert(0, H256::from(recovery::GENESIS_HASH));
            frame_system::BlockHash::<Runtime>::insert(number - 1, parent);
            frame_system::Account::<Runtime>::mutate(&account, |a| a.nonce = 7);
        });
        let version = framenode_runtime::VERSION;
        let call = Call::Bond {
            value: "1000000000000000000".into(),
        }
        .runtime(&account)
        .unwrap();
        let before = replay::root(&mut ext);
        let (opaque, report) = signed(&mut ext, call, &pair, number, parent, &version).unwrap();
        assert_eq!(
            replay::root(&mut ext),
            before,
            "signing may not inject state"
        );
        assert_eq!(report["nonce"], 7);
        let encoded = opaque.encode();
        let tx =
            framenode_runtime::UncheckedExtrinsic::decode_all(&mut encoded.as_slice()).unwrap();
        let Preamble::Signed(signer, signature, extra) = tx.preamble else {
            panic!("not signed")
        };
        let make = |spec: u32,
                    transaction: u32,
                    genesis: H256,
                    birth: H256,
                    call: Vec<u8>,
                    extra: Vec<u8>| {
            let full = [
                call,
                extra,
                spec.encode(),
                transaction.encode(),
                genesis.encode(),
                birth.encode(),
            ]
            .concat();
            if full.len() > 256 {
                sp_io::hashing::blake2_256(&full).to_vec()
            } else {
                full
            }
        };
        let valid = make(
            version.spec_version,
            version.transaction_version,
            recovery::GENESIS_HASH.into(),
            parent,
            tx.function.encode(),
            extra.encode(),
        );
        assert!(signature.verify(valid.as_slice(), &signer));
        for altered in [
            make(
                137,
                version.transaction_version,
                recovery::GENESIS_HASH.into(),
                parent,
                tx.function.encode(),
                extra.encode(),
            ),
            make(
                version.spec_version,
                133,
                recovery::GENESIS_HASH.into(),
                parent,
                tx.function.encode(),
                extra.encode(),
            ),
            make(
                version.spec_version,
                version.transaction_version,
                H256::repeat_byte(1),
                parent,
                tx.function.encode(),
                extra.encode(),
            ),
            make(
                version.spec_version,
                version.transaction_version,
                recovery::GENESIS_HASH.into(),
                H256::repeat_byte(2),
                tx.function.encode(),
                extra.encode(),
            ),
            make(
                version.spec_version,
                version.transaction_version,
                recovery::GENESIS_HASH.into(),
                parent,
                Call::Unbond { value: "1".into() }
                    .runtime(&account)
                    .unwrap()
                    .encode(),
                extra.encode(),
            ),
            make(
                version.spec_version,
                version.transaction_version,
                recovery::GENESIS_HASH.into(),
                parent,
                tx.function.encode(),
                (
                    Era::mortal(64, u64::from(number - 1)),
                    Compact(8u32),
                    Compact(0u128),
                )
                    .encode(),
            ),
        ] {
            assert!(!signature.verify(altered.as_slice(), &signer));
        }
    }

    #[test]
    fn signing_rejects_missing_or_substituted_birth_state() {
        let (account, pair) = fixture::pairs().remove(0);
        let mut ext = sp_io::TestExternalities::default();
        let call = Call::SetPayeeOwner {}.runtime(&account).unwrap();
        assert!(signed(
            &mut ext,
            call.clone(),
            &pair,
            recovery::CHECKPOINT_NUMBER + 2,
            H256::repeat_byte(4),
            &framenode_runtime::VERSION
        )
        .is_err());
        ext.execute_with(|| {
            frame_system::BlockHash::<Runtime>::insert(0, H256::from(recovery::GENESIS_HASH))
        });
        assert!(signed(
            &mut ext,
            call,
            &pair,
            recovery::CHECKPOINT_NUMBER + 2,
            H256::repeat_byte(4),
            &framenode_runtime::VERSION
        )
        .is_err());
    }

    #[test]
    fn plan_admission_requires_public_replacement_ownership_and_ordinary_steps() {
        let keys = fixture::pairs();
        let replacement = recovery::AccountReplacement {
            old: AccountId::new([91; 32]),
            new: keys[0].0.clone(),
        };
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![replacement.clone()],
            council: keys.iter().map(|(a, _)| a.clone()).collect(),
            chunk_hashes: vec![[7; 32]],
            audit_chunk_hashes: vec![[8; 32]],
            evidence_commitment: [9; 32],
            terminal_audit_commitment: [10; 32],
        };
        let signature = MultiSignature::from(
            keys[0]
                .1
                .sign(&recovery::replacement_ownership_message(&replacement)),
        );
        let plan = json!({"schema":PLAN_SCHEMA,"fixtureOnly":true,"policyCommitment":hx(&policy.commitment()),
            "signerScheme":fixture::SCHEME,"signerIndex":0,"replacementOwnershipSignature":hx(&signature.encode()),
            "funding":[{"operationIndex":0,"operationCommitment":hx(&[7;32]),"minimumAmount":"1"}],
            "transactions":[{"step":0,"expectedNonce":0,"minimumFreeBalance":"100","maximumFee":"10",
                "call":{"kind":"bond","value":"1"},"expectedDispatchError":null,"beforeLedger":null,"afterLedger":null}]});
        let step: Step = serde_json::from_value(json!({"slot":1,"timestamp":1,"authorityIndex":0,
            "recovery":null,"fixtureEscape":null,"expectedSpec":138,"expectedPhase":"Complete"}))
        .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "sora-nexus-public-signed-admission-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let check = |value: &Value, steps: &[Step]| {
            let bytes = serde_json::to_vec(value).unwrap();
            fs::write(directory.join("plan.json"), &bytes).unwrap();
            Session::load(
                &directory,
                &replay::Artifact {
                    file: "plan.json".into(),
                    sha256: sha(&bytes),
                },
                &policy,
                steps,
            )
        };
        assert!(check(&plan, std::slice::from_ref(&step)).is_ok());
        let mut wrong = plan.clone();
        wrong["fixtureOnly"] = json!(false);
        assert!(check(&wrong, std::slice::from_ref(&step)).is_err());
        wrong = plan.clone();
        wrong["signerIndex"] = json!(1);
        assert!(check(&wrong, std::slice::from_ref(&step)).is_err());
        wrong = plan.clone();
        wrong["replacementOwnershipSignature"] =
            json!(hx(&MultiSignature::from(keys[1].1.sign(
                &recovery::replacement_ownership_message(&replacement)
            ))
            .encode()));
        assert!(check(&wrong, std::slice::from_ref(&step)).is_err());
        wrong = plan.clone();
        wrong["funding"][0]["operationCommitment"] = json!(hx(&[6; 32]));
        assert!(check(&wrong, std::slice::from_ref(&step)).is_err());
        wrong = plan.clone();
        wrong["transactions"][0]["minimumFreeBalance"] = json!("0");
        assert!(check(&wrong, std::slice::from_ref(&step)).is_err());
        let mut maintenance = step;
        maintenance.expected_spec = 134;
        maintenance.expected_phase = "Applying".into();
        assert!(check(&plan, &[maintenance]).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn independent_council_keeps_public_owner_signature_requirement() {
        let keys = fixture::pairs();
        let replacement = recovery::AccountReplacement {
            old: AccountId::new([91; 32]),
            new: keys[0].0.clone(),
        };
        let policy = RecoveryPolicy {
            format_version: 2,
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            checkpoint_hash: recovery::CHECKPOINT_HASH,
            checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
            replacements: vec![replacement.clone()],
            council: vec![AccountId::new([31; 32]), AccountId::new([32; 32])],
            chunk_hashes: vec![[7; 32]],
            audit_chunk_hashes: vec![[8; 32]],
            evidence_commitment: [9; 32],
            terminal_audit_commitment: [10; 32],
        };
        let proof = MultiSignature::from(
            keys[0]
                .1
                .sign(&recovery::replacement_ownership_message(&replacement)),
        );
        let plan = json!({"schema":PLAN_SCHEMA,"fixtureOnly":true,"policyCommitment":hx(&policy.commitment()),
            "signerScheme":fixture::SCHEME,"signerIndex":0,"replacementOwnershipSignature":hx(&proof.encode()),
            "funding":[{"operationIndex":0,"operationCommitment":hx(&[7;32]),"minimumAmount":"1"}],
            "transactions":[{"step":0,"expectedNonce":0,"minimumFreeBalance":"100","maximumFee":"10",
                "call":{"kind":"bond","value":"1"},"expectedDispatchError":null,"beforeLedger":null,"afterLedger":null}]});
        let step: Step = serde_json::from_value(json!({"slot":1,"timestamp":1,"authorityIndex":0,
            "recovery":null,"fixtureEscape":null,"expectedSpec":138,"expectedPhase":"Complete"}))
        .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "sora-nexus-independent-council-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let check = |value: &Value| {
            let bytes = serde_json::to_vec(value).unwrap();
            fs::write(directory.join("plan.json"), &bytes).unwrap();
            Session::load(
                &directory,
                &replay::Artifact {
                    file: "plan.json".into(),
                    sha256: sha(&bytes),
                },
                &policy,
                std::slice::from_ref(&step),
            )
        };
        let accepted = check(&plan).unwrap();
        assert_eq!(accepted.signer, replacement.new);
        assert!(!policy.council.contains(&accepted.signer));
        let mut wrong = plan.clone();
        wrong["signerIndex"] = json!(1);
        assert!(check(&wrong).is_err());
        wrong = plan.clone();
        wrong["replacementOwnershipSignature"] =
            json!(hx(&MultiSignature::from(keys[1].1.sign(
                &recovery::replacement_ownership_message(&replacement)
            ))
            .encode()));
        assert!(check(&wrong).is_err());
        wrong = plan;
        wrong["policyCommitment"] = json!(hx(&[4; 32]));
        assert!(check(&wrong).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn normal_signed_corridor_rejects_raw_escape_before_any_signing() {
        let directory = std::env::temp_dir().join(format!(
            "sora-nexus-signed-no-escape-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let mut step: Step =
            serde_json::from_value(json!({"slot":1,"timestamp":1,"authorityIndex":0,
            "recovery":null,"fixtureEscape":null,"expectedSpec":134,"expectedPhase":"Applying"}))
            .unwrap();
        let envelope = |action| {
            recovery::RecoveryInherent {
                format_version: 1,
                action,
            }
            .encode_remark()
        };
        let tick = envelope(recovery::RecoveryAction::Tick);
        fs::write(directory.join("work.scale"), &tick).unwrap();
        step.recovery = Some(replay::Artifact {
            file: "work.scale".into(),
            sha256: sha(&tick),
        });
        assert!(normal_actions(&directory, std::slice::from_ref(&step)).is_ok());
        let (signer, pair) = fixture::pairs().remove(0);
        let authorization = recovery::RepairAuthorization {
            recovery_anchor: [0x42; 32],
            authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
            policy: [1; 32],
            phase: RecoveryPhase::Applying,
            cursor: 0,
            operation_counter: 0,
            current_code_hash: [2; 32],
            current_executor_hash: [3; 32],
            destination_code_hash: [4; 32],
            destination_spec: 135,
            revised_policy: [0; 32],
        };
        let signature = MultiSignature::from(pair.sign(&authorization.signing_message()));
        let escape = envelope(recovery::RecoveryAction::Escape {
            authorization: Box::new(authorization),
            signatures: vec![(signer, signature)],
            work_policy: None,
        });
        assert!(recovery::RecoveryInherent::decode_remark(&escape).is_ok());
        fs::write(directory.join("work.scale"), &escape).unwrap();
        step.recovery.as_mut().unwrap().sha256 = sha(&escape);
        assert!(
            matches!(normal_actions(&directory,std::slice::from_ref(&step)),Err(e) if e.contains("raw council escape"))
        );
        step.recovery = None;
        step.fixture_escape = Some(fixture::Escape {
            destination_spec: 135,
            work_policy: None,
        });
        assert!(normal_actions(&directory, std::slice::from_ref(&step)).is_err());
        step.fixture_escape = None;
        step.reject_previous_escapes = true;
        assert!(normal_actions(&directory, std::slice::from_ref(&step)).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
