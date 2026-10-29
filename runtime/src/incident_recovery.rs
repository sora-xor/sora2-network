//! Admission and permanent lineage of the final SORA Nexus recovery runtime.
//!
//! The final async-staking runtime is not a spec-134 bootstrap. Installing it on
//! unprepared mainnet state must fail before Executive runs any upgrade hooks.
//! The legacy bootstrap must finish account recovery and its committed audits
//! first. An unbound development build cannot authorize mainnet activation.

use codec::{Decode, DecodeAll, Encode};
use common::recovery::{
    self, RecoveryError, RecoveryPhase, RecoveryPolicy, CHECKPOINT_HASH, CHECKPOINT_NUMBER,
    GENESIS_HASH, POLICY_STORAGE_KEY,
};
use frame_support::{
    traits::{OnRuntimeUpgrade, PreInherents},
    weights::Weight,
};
use sp_runtime::{traits::Header as _, DigestItem};

use crate::{AccountId, Babe, BlockNumber, Header, Runtime, System};

mod generated;

#[cfg(test)]
std::thread_local! {
    static TEST_POLICY: std::cell::RefCell<Option<RecoveryPolicy>> = const { std::cell::RefCell::new(None) };
}

/// Native fixtures bind the same policy used by their transactional executor.
/// This hook does not exist in any Wasm or node release build.
#[cfg(test)]
pub(crate) fn with_test_policy<R>(policy: &RecoveryPolicy, run: impl FnOnce() -> R) -> R {
    policy.validate().expect("invalid test policy");
    struct Restore(Option<RecoveryPolicy>);
    impl Drop for Restore {
        fn drop(&mut self) {
            TEST_POLICY.with(|cell| *cell.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(TEST_POLICY.with(|cell| cell.replace(Some(policy.clone()))));
    run()
}

pub fn compiled_policy() -> Result<RecoveryPolicy, RecoveryError> {
    #[cfg(test)]
    if let Some(policy) = TEST_POLICY.with(|cell| cell.borrow().clone()) {
        return Ok(policy);
    }
    if generated::POLICY_SCALE.is_empty() || generated::POLICY_COMMITMENT == [0; 32] {
        return Err(RecoveryError::MissingCompiledPolicy);
    }
    let mut bytes = generated::POLICY_SCALE;
    let policy = RecoveryPolicy::decode(&mut bytes).map_err(|_| RecoveryError::InvalidPolicy)?;
    if !bytes.is_empty() || policy.commitment() != generated::POLICY_COMMITMENT {
        return Err(RecoveryError::InvalidPolicy);
    }
    policy.validate()?;
    if policy.replacements.len() != generated::REPLACEMENTS.len()
        || !policy
            .replacements
            .iter()
            .zip(generated::REPLACEMENTS)
            .all(|(entry, (old, new))| {
                entry.old == AccountId::from(*old) && entry.new == AccountId::from(*new)
            })
    {
        return Err(RecoveryError::InvalidMapping);
    }
    Ok(policy)
}

fn mainnet_or_recovery() -> bool {
    System::block_hash(0).as_bytes() == &GENESIS_HASH || sp_io::storage::exists(POLICY_STORAGE_KEY)
}

/// Invoked by BOTH authoring and block import before any ordinary runtime hook.
pub fn assert_final_runtime_admission(header: &Header) {
    if !mainnet_or_recovery() {
        return;
    }
    assert!(
        recovery::TRANSIENT_CONTEXT_KEYS
            .iter()
            .all(|key| !sp_io::storage::exists(key)),
        "unfinished recovery context at final activation"
    );
    assert!(
        *header.number() > CHECKPOINT_NUMBER,
        "incident runtime cannot execute preserved checkpoint history"
    );
    let policy = compiled_policy().expect("incident runtime has no verified compiled policy");
    assert_eq!(
        sp_io::storage::get(POLICY_STORAGE_KEY).as_deref(),
        Some(policy.commitment().as_slice()),
        "incident policy differs from completed bootstrap"
    );
    let status = recovery::storage::status()
        .expect("invalid incident recovery state")
        .expect("missing incident recovery state");
    assert_eq!(status.policy, policy.commitment(), "wrong recovery lineage");
    assert!(
        matches!(status.phase, RecoveryPhase::Ready | RecoveryPhase::Complete),
        "account recovery must finish before ordinary upgrade hooks"
    );
    assert_eq!(
        sp_io::storage::get(recovery::INCOMING_AUDIT_PENDING_KEY).as_deref(),
        Some(0u32.encode().as_slice()),
        "missing or incomplete authenticated incoming audits"
    );
    let terminal = sp_io::storage::get(recovery::TERMINAL_AUDIT_STORAGE_KEY)
        .expect("missing completed terminal invariants");
    let (work_policy, _audit, completed_chunks) =
        <([u8; 32], [u8; 32], u32)>::decode_all(&mut &terminal[..])
            .expect("invalid terminal audit record");
    assert_eq!(
        sp_io::storage::get(recovery::ACTIVE_POLICY_STORAGE_KEY).as_deref(),
        Some(work_policy.as_slice()),
        "unverified active recovery work"
    );
    assert_eq!(
        status.next_chunk, completed_chunks,
        "recovery chunk sequence incomplete"
    );
    // The checkpoint hash may age out of System::BlockHash. Its permanent
    // binding is the compiled policy plus the node's enforced header lineage.
    let retained = System::block_hash(CHECKPOINT_NUMBER);
    assert!(
        retained == sp_core::H256::zero() || retained.as_bytes() == &CHECKPOINT_HASH,
        "retained checkpoint hash mismatch"
    );
}

pub struct RecoveryLineage;

impl PreInherents for RecoveryLineage {
    fn pre_inherents() {
        if let Some(value) = sp_io::storage::get(POLICY_STORAGE_KEY) {
            let policy: [u8; 32] = (&value[..])
                .try_into()
                .expect("invalid recovery policy hash");
            assert!(
                System::block_number() > CHECKPOINT_NUMBER,
                "premature recovery marker"
            );
            System::deposit_log(DigestItem::Other(recovery::lineage_marker(policy)));
            System::register_extra_weight_unchecked(
                Weight::from_parts(1_000_000, 128),
                frame_support::dispatch::DispatchClass::Mandatory,
            );
        }
    }
}

/// Canonical replacement resolution is used only AFTER authenticating original
/// historical messages. Ordinary user destinations must be checked, not silently
/// redirected, so a signed transaction cannot unexpectedly change its recipient.
pub fn replacement(account: &AccountId) -> Result<Option<AccountId>, RecoveryError> {
    if !sp_io::storage::exists(POLICY_STORAGE_KEY) {
        return Ok(None);
    }
    #[cfg(test)]
    if let Some(policy) = TEST_POLICY.with(|cell| cell.borrow().clone()) {
        if sp_io::storage::get(POLICY_STORAGE_KEY).as_deref()
            != Some(policy.commitment().as_slice())
        {
            return Err(RecoveryError::WrongPolicy);
        }
        return Ok(policy
            .replacements
            .iter()
            .find(|entry| &entry.old == account)
            .map(|entry| entry.new.clone()));
    }
    // Use the admission-verified static map, not a repeated decode of every
    // committed recovery chunk for each transfer or entitlement lookup.
    if generated::POLICY_COMMITMENT == [0; 32]
        || sp_io::storage::get(POLICY_STORAGE_KEY).as_deref()
            != Some(generated::POLICY_COMMITMENT.as_slice())
    {
        return Err(RecoveryError::WrongPolicy);
    }
    Ok(generated::REPLACEMENTS
        .binary_search_by(|entry| entry.0.cmp(AsRef::<[u8; 32]>::as_ref(account)))
        .ok()
        .map(|index| AccountId::from(generated::REPLACEMENTS[index].1)))
}

pub fn account_retired(account: &AccountId) -> bool {
    if !sp_io::storage::exists(POLICY_STORAGE_KEY) {
        return false;
    }
    account == &AccountId::from(recovery::ATTACKER_ACCOUNT)
        || replacement(account).map_or(true, |mapped| mapped.is_some())
}

/// Resolve authenticated offence identities with one policy check for a batch.
/// This does not rewrite signed calls or historical proof roots.
pub(crate) fn resolve_authenticated_accounts<'a>(
    accounts: impl Iterator<Item = &'a mut AccountId>,
) -> Result<(), RecoveryError> {
    let Some(stored) = sp_io::storage::get(POLICY_STORAGE_KEY) else {
        return Ok(());
    };
    #[cfg(test)]
    if let Some(policy) = TEST_POLICY.with(|cell| cell.borrow().clone()) {
        if stored.as_ref() != policy.commitment().as_slice() {
            return Err(RecoveryError::WrongPolicy);
        }
        for account in accounts {
            if let Ok(index) = policy
                .replacements
                .binary_search_by(|entry| entry.old.cmp(account))
            {
                *account = policy.replacements[index].new.clone();
            }
        }
        return Ok(());
    }
    if generated::POLICY_COMMITMENT == [0; 32]
        || stored.as_ref() != generated::POLICY_COMMITMENT.as_slice()
    {
        return Err(RecoveryError::WrongPolicy);
    }
    for account in accounts {
        if let Ok(index) = generated::REPLACEMENTS
            .binary_search_by(|entry| entry.0.cmp(AsRef::<[u8; 32]>::as_ref(account)))
        {
            *account = AccountId::from(generated::REPLACEMENTS[index].1);
        }
    }
    Ok(())
}

fn ensure_active(account: &AccountId) -> frame_support::dispatch::DispatchResult {
    if account_retired(account) {
        Err(sp_runtime::DispatchError::Other("Retired recovery account"))
    } else {
        Ok(())
    }
}

/// Enforce retirement at the currency boundary too, including transfers from
/// contracts, derived origins and internal pallet operations.
pub struct RecoveryCurrencyHooks;

impl traits::currency::MutationHooks<AccountId, crate::AssetId, crate::Balance>
    for RecoveryCurrencyHooks
{
    type OnDust = ();
    type OnSlash = ();
    type PreDeposit = Self;
    type PostDeposit = ();
    type PreTransfer = Self;
    type PostTransfer = ();
    type OnNewTokenAccount = ();
    type OnKilledTokenAccount = ();
}

impl traits::currency::OnDeposit<AccountId, crate::AssetId, crate::Balance>
    for RecoveryCurrencyHooks
{
    fn on_deposit(
        _: crate::AssetId,
        who: &AccountId,
        _: crate::Balance,
    ) -> frame_support::dispatch::DispatchResult {
        ensure_active(who)
    }
}

impl traits::currency::OnTransfer<AccountId, crate::AssetId, crate::Balance>
    for RecoveryCurrencyHooks
{
    fn on_transfer(
        _: crate::AssetId,
        from: &AccountId,
        to: &AccountId,
        _: crate::Balance,
    ) -> frame_support::dispatch::DispatchResult {
        ensure_active(from)?;
        ensure_active(to)
    }
}

pub struct RecoveryNativeAccounts;

impl frame_support::traits::StoredMap<AccountId, pallet_balances::AccountData<crate::Balance>>
    for RecoveryNativeAccounts
{
    fn get(account: &AccountId) -> pallet_balances::AccountData<crate::Balance> {
        <System as frame_support::traits::StoredMap<_, _>>::get(account)
    }

    fn try_mutate_exists<R, E: From<sp_runtime::DispatchError>>(
        account: &AccountId,
        f: impl FnOnce(&mut Option<pallet_balances::AccountData<crate::Balance>>) -> Result<R, E>,
    ) -> Result<R, E> {
        ensure_active(account).map_err(E::from)?;
        <System as frame_support::traits::StoredMap<_, _>>::try_mutate_exists(account, f)
    }
}

/// Do not start bridge work merely because ordinary runtime execution resumes.
pub fn bridge_halted() -> bool {
    sp_io::storage::exists(recovery::BRIDGE_HALT_STORAGE_KEY)
}

/// Shift seconds, not milliseconds; retain fee accrual preceding the checkpoint.
pub fn shifted_fee_baseline(
    last_update_seconds: u64,
    checkpoint_seconds: u64,
    activation_seconds: u64,
) -> Result<u64, RecoveryError> {
    if last_update_seconds > checkpoint_seconds {
        return Err(RecoveryError::WrongPreimage);
    }
    let downtime = activation_seconds
        .checked_sub(checkpoint_seconds)
        .ok_or(RecoveryError::WrongPreimage)?;
    last_update_seconds
        .checked_add(downtime)
        .ok_or(RecoveryError::ChunkBounds)
}

pub fn first_recovery_block(number: BlockNumber) -> bool {
    number == CHECKPOINT_NUMBER + 1
}

const LIVENESS_RESUMED: &[u8] = b":sora-nexus:recovery:liveness-resumed:v1";
const FEE_CLOCK_SHIFTED: &[u8] = b":sora-nexus:recovery:fee-clock-shifted:v1";
const CHECKPOINT_TIME: &[u8] = b":sora-nexus:recovery:checkpoint-time:v1";
const ACTIVATION_TIME: &[u8] = b":sora-nexus:recovery:activation-time:v1";
const MAX_COLLATERAL_CLOCKS: usize = 4096;

/// Suppressed heartbeats during the frozen session are not evidence of an
/// offline offence. Clear the partial observation immediately before the first
/// actual session rotation, once, and let normal tracking resume afterwards.
pub struct RecoverySessionBoundary;

impl pallet_session::ShouldEndSession<BlockNumber> for RecoverySessionBoundary {
    fn should_end_session(now: BlockNumber) -> bool {
        let end = <Babe as pallet_session::ShouldEndSession<BlockNumber>>::should_end_session(now);
        if end
            && sp_io::storage::exists(POLICY_STORAGE_KEY)
            && !sp_io::storage::exists(LIVENESS_RESUMED)
        {
            crate::liveness::DisabledDuringSession::<Runtime>::kill();
            sp_io::storage::set(LIVENESS_RESUMED, &now.encode());
        }
        end
    }
}

/// The bootstrap writes both times as SCALE u64 milliseconds. Validate the
/// complete bounded inventory before any write, preserving accrued pre-incident
/// interest and preventing a second shift on a later runtime upgrade.
pub struct RecoverFeeClocks;

impl OnRuntimeUpgrade for RecoverFeeClocks {
    fn on_runtime_upgrade() -> Weight {
        let db = <Runtime as frame_system::Config>::DbWeight::get();
        if !sp_io::storage::exists(POLICY_STORAGE_KEY) || sp_io::storage::exists(FEE_CLOCK_SHIFTED)
        {
            return db.reads(2);
        }
        let time = |key| {
            let bytes = sp_io::storage::get(key).expect("missing committed recovery time");
            u64::decode_all(&mut &bytes[..]).expect("invalid recovery timestamp") / 1000
        };
        let checkpoint = time(CHECKPOINT_TIME);
        let activation = time(ACTIVATION_TIME);
        let updates: alloc::vec::Vec<_> = kensetsu::CollateralInfos::<Runtime>::iter()
            .take(MAX_COLLATERAL_CLOCKS + 1)
            .map(|(key, mut info)| {
                info.last_fee_update_time =
                    shifted_fee_baseline(info.last_fee_update_time, checkpoint, activation)
                        .expect("inconsistent recovery fee clock");
                (key, info)
            })
            .collect();
        assert!(
            updates.len() <= MAX_COLLATERAL_CLOCKS,
            "unbounded recovery fee clocks"
        );
        let count = updates.len() as u64;
        for (key, info) in updates {
            kensetsu::CollateralInfos::<Runtime>::insert(key, info);
        }
        sp_io::storage::set(FEE_CLOCK_SHIFTED, &(checkpoint, activation).encode());
        db.reads_writes(count + 5, count + 1)
            .saturating_add(Weight::from_parts(count * 1_000_000, count * 512))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_runtime_admission_rejects_missing_or_nonzero_pending_counter() {
        use sp_runtime::traits::Header as _;
        for value in [None, Some(1u32), Some(0u32)] {
            sp_io::TestExternalities::default().execute_with(|| {
                let policy = RecoveryPolicy {
                    format_version: 2,
                    authorization_domain: common::recovery::RecoveryDomain::public_fixture(),
                    checkpoint_hash: recovery::CHECKPOINT_HASH,
                    checkpoint_state_root: recovery::CHECKPOINT_STATE_ROOT,
                    replacements: vec![recovery::AccountReplacement {
                        old: AccountId::new([7; 32]),
                        new: AccountId::new([8; 32]),
                    }],
                    council: vec![AccountId::new([5; 32])],
                    chunk_hashes: vec![[3; 32]],
                    audit_chunk_hashes: vec![[4; 32]],
                    evidence_commitment: [6; 32],
                    terminal_audit_commitment: [9; 32],
                };
                recovery::storage::initialize(&policy).unwrap();
                let mut status = recovery::storage::status().unwrap().unwrap();
                status.phase = RecoveryPhase::Complete;
                sp_io::storage::set(recovery::STATUS_STORAGE_KEY, &status.encode());
                sp_io::storage::set(
                    recovery::TERMINAL_AUDIT_STORAGE_KEY,
                    &(
                        policy.commitment(),
                        policy.terminal_audit_commitment,
                        status.next_chunk,
                    )
                        .encode(),
                );
                match value {
                    None => sp_io::storage::clear(recovery::INCOMING_AUDIT_PENDING_KEY),
                    Some(n) => {
                        sp_io::storage::set(recovery::INCOMING_AUDIT_PENDING_KEY, &n.encode())
                    }
                }
                let header = crate::Header::new(
                    recovery::CHECKPOINT_NUMBER + 100,
                    Default::default(),
                    Default::default(),
                    Default::default(),
                    Default::default(),
                );
                let admitted = crate::incident_recovery::with_test_policy(&policy, || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        crate::incident_recovery::assert_final_runtime_admission(&header)
                    }))
                    .is_ok()
                });
                assert_eq!(admitted, value == Some(0));
            });
        }
    }

    #[test]
    fn release_without_actual_inputs_is_not_admissible() {
        assert_eq!(compiled_policy(), Err(RecoveryError::MissingCompiledPolicy));
    }

    #[test]
    fn fee_clock_retains_pre_checkpoint_elapsed_time() {
        assert_eq!(shifted_fee_baseline(90, 100, 1_000), Ok(990));
        assert_eq!(1_000 - shifted_fee_baseline(90, 100, 1_000).unwrap(), 10);
        assert_eq!(
            shifted_fee_baseline(101, 100, 1_000),
            Err(RecoveryError::WrongPreimage)
        );
        assert_eq!(
            shifted_fee_baseline(90, 100, 99),
            Err(RecoveryError::WrongPreimage)
        );
    }

    #[test]
    fn downtime_fee_shift_is_once_and_preserves_debt_and_interest() {
        sp_io::TestExternalities::default().execute_with(|| {
            let id = kensetsu::StablecoinCollateralIdentifier {
                collateral_asset_id: crate::XOR,
                stablecoin_asset_id: common::TBCD,
            };
            let info = kensetsu::CollateralInfo {
                risk_parameters: Default::default(),
                total_collateral: 99,
                stablecoin_supply: 17,
                last_fee_update_time: 90u64,
                interest_coefficient: sp_runtime::FixedU128::from_inner(7),
            };
            kensetsu::CollateralInfos::<Runtime>::insert(&id, info);
            sp_io::storage::set(POLICY_STORAGE_KEY, &[1; 32]);
            sp_io::storage::set(CHECKPOINT_TIME, &100_000u64.encode());
            sp_io::storage::set(ACTIVATION_TIME, &1_000_000u64.encode());
            RecoverFeeClocks::on_runtime_upgrade();
            let result = kensetsu::CollateralInfos::<Runtime>::get(&id).unwrap();
            assert_eq!(result.last_fee_update_time, 990);
            assert_eq!(result.total_collateral, 99);
            assert_eq!(result.stablecoin_supply, 17);
            assert_eq!(result.interest_coefficient.into_inner(), 7);
            let root = sp_io::storage::root(sp_runtime::StateVersion::V1);
            RecoverFeeClocks::on_runtime_upgrade();
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V1), root);
        });
    }

    #[test]
    fn retired_currency_mutation_rejects_before_the_balance_closure() {
        use frame_support::traits::StoredMap;
        sp_io::TestExternalities::default().execute_with(|| {
            let attacker = AccountId::from(recovery::ATTACKER_ACCOUNT);
            assert!(ensure_active(&attacker).is_ok());
            sp_io::storage::set(POLICY_STORAGE_KEY, &[1; 32]);
            let mut called = false;
            let result: Result<(), sp_runtime::DispatchError> =
                RecoveryNativeAccounts::try_mutate_exists(&attacker, |_| {
                    called = true;
                    Ok(())
                });
            assert!(result.is_err());
            assert!(!called);
            assert!(
                <RecoveryCurrencyHooks as traits::currency::OnDeposit<_, _, _>>::on_deposit(
                    crate::XOR,
                    &attacker,
                    1
                )
                .is_err()
            );
        });
    }
}
