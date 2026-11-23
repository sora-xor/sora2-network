use super::*;
use beefy_light_client::weights::WeightInfo;
use frame_support::dispatch::DispatchClass;

#[test]
fn beefy_maximum_admitted_shape_fits_normal_extrinsic_with_reserve() {
    let maximum: u32 = <Runtime as beefy_light_client::Config>::MaxValidators::get();
    let signatures = (u64::from(maximum) * u64::from(beefy_light_client::THRESHOLD_NUMERATOR)
        + u64::from(beefy_light_client::THRESHOLD_DENOMINATOR)
        - 1)
        / u64::from(beefy_light_client::THRESHOLD_DENOMINATOR);
    let attempts = maximum
        .saturating_add(7)
        .saturating_mul(beefy_light_client::SAMPLING_ATTEMPTS_PER_VALIDATOR)
        .saturating_add(beefy_light_client::SAMPLING_ATTEMPTS_BASE);
    let charged = <Runtime as beefy_light_client::Config>::WeightInfo::submit_signature_commitment(
        attempts,
        signatures as u32,
        signatures as u32 * beefy_light_client::MAX_VALIDATOR_PROOF_ITEMS as u32 + 63,
        beefy_light_client::MAX_COMMITMENT_BYTES as u32,
    );
    // Runtime DB charges are included above. Reserve another 100 ms for signed
    // extensions; max_extrinsic already subtracts initialization and the base.
    let with_reserve = charged.saturating_add(Weight::from_parts(100_000_000_000, 0));
    let maximum_extrinsic = BlockWeights::get()
        .get(DispatchClass::Normal)
        .max_extrinsic
        .expect("normal extrinsics have a configured limit");
    assert!(
        with_reserve.all_lte(maximum_extrinsic),
        "BEEFY maximum plus extension reserve {with_reserve:?} exceeds {maximum_extrinsic:?}"
    );
}
