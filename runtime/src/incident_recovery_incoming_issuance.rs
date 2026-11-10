//! Capture exact native supply changes in the same incoming transaction.
use super::*;
use common::recovery::issuance;

pub(super) fn consumption_weight() -> Weight {
    let n = u64::from(BridgeMaxMessagesPerCommit::get());
    Weight::from_parts(100_000_000 * n, 8192 * n).saturating_add(
        <Runtime as frame_system::Config>::DbWeight::get().reads_writes(16 + 12 * n, 1 + 2 * n),
    )
}
pub(super) fn prepare(
    operation: &IncomingOperation,
    active: &ActiveOperation,
    policy: &RecoveryPolicy,
) -> Result<issuance::Prepared, RecoveryError> {
    // Excluded transfers still identify affected assets: prepared HASHI claims
    // can burn previously issued units. This copy never authorizes a delivery.
    let mut financial = active.clone();
    for transfer in &mut financial.transfers {
        transfer.excluded = false;
    }
    let assets: BTreeSet<_> = audit::derive_requests(operation, &financial)?
        .into_iter()
        .flatten()
        .map(|request| request.asset_id)
        .collect();
    let rows = assets
        .into_iter()
        .map(|asset| {
            let key = if asset == common::XOR {
                pallet_balances::TotalIssuance::<Runtime>::hashed_key().to_vec()
            } else {
                tokens::TotalIssuance::<Runtime>::hashed_key_for(asset)
            };
            let id: [u8; 32] = asset
                .encode()
                .try_into()
                .map_err(|_| RecoveryError::WrongPreimage)?;
            Ok((id, key))
        })
        .collect::<Result<Vec<_>, RecoveryError>>()?;
    issuance::prepare(policy, operation.index, rows)
}
pub(super) fn record(
    policy: &RecoveryPolicy,
    prepared: issuance::Prepared,
) -> Result<(), RecoveryError> {
    issuance::record_consumption(policy, prepared)
}
