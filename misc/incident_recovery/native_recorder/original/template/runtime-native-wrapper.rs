// Host-only entry points for the separately bound native-operation recorder.
// No Wasm code or release runtime API is added by these std-only definitions.
#[cfg(feature = "std")]
pub fn native_recovery_recorder_policies() -> Result<
    (
        common::recovery::RecoveryPolicy,
        common::recovery::RecoveryPolicy,
    ),
    common::recovery::RecoveryError,
> {
    Ok((
        incident_recovery::compiled_policy()?,
        incident_recovery::original_policy()?,
    ))
}
#[cfg(feature = "std")]
pub fn native_recovery_recorder_apply(
    extrinsic: UncheckedExtrinsic,
) -> sp_runtime::ApplyExtrinsicResult {
    incident_recovery::apply_extrinsic(extrinsic)
}
