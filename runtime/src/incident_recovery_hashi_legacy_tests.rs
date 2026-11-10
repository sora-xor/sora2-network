//! Export-only test for the production spec-134 bootstrap.
//! Expected RuntimeVersion was retained at production block 28,190,927;
//! current-production-identity.json SHA256 c9a06f130ffb0b7710f72837a6869a80f7e39426545c59b855c6a1959d23fb56
//! The preparer includes this module only in the spec-134 export.

#[test]
fn legacy_hashi_source_api_matches_original_request_before_native_payload_preparation() {
    use crate::incident_recovery_incoming::witness;
    use codec::Encode;
    sp_io::TestExternalities::default().execute_with(|| {
        let (policy, operation, source) = witness::source::tests::fixture();
        crate::incident_recovery::with_test_policy(&policy, || {
            let header = witness::tests::parent(&policy).encode();
            let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
            let result =
                witness::source::prepare_source(header, operation.encode(), source.encode())
                    .unwrap();
            assert_eq!(result.preparation.spec_version, 134);
            assert_eq!(result.preparation.transaction_version, 131);
            assert!(result.association.complete_original_request_matches);
            assert!(!result.association.source_inclusion_verified);
            assert!(!result.association.external_finality_verified);
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
        });
    });
}

#[test]
fn legacy_hashi_public_api_executes_native_admission_without_persistent_writes() {
    use crate::incident_recovery_incoming::witness;
    use codec::Encode;
    use common::recovery;
    let mut ext = sp_io::TestExternalities::default();
    ext.execute_with(|| {
        let (policy, operation, _) = witness::tests::fixture();
        crate::incident_recovery::with_test_policy(&policy, || {
            let header = witness::tests::parent(&policy).encode();
            let before = sp_io::storage::root(sp_runtime::StateVersion::V0);
            let prepared = witness::prepare(header.clone(), operation.encode()).unwrap();
            assert_eq!(prepared.spec_version, 134);
            assert_eq!(prepared.transaction_version, 131);
            assert_eq!(prepared.execution_number, recovery::CHECKPOINT_NUMBER + 3);
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
            let admitted = witness::admit(header, operation.encode()).unwrap();
            assert_eq!(admitted.preparation, prepared);
            assert!(admitted.native_signatures_checked && admitted.native_incoming_executed);
            assert!(!admitted.persistent_state_changed && !admitted.external_finality_verified);
            assert_eq!(sp_io::storage::root(sp_runtime::StateVersion::V0), before);
        });
    });
    assert_eq!(
        ext.overlayed_changes()
            .offchain()
            .overlay()
            .changes()
            .count(),
        0
    );
}

#[cfg(not(any(
    feature = "wip",
    feature = "runtime-benchmarks",
    feature = "try-runtime"
)))]
#[test]
fn legacy_hashi_api_preserves_deployed_runtime_version_and_complete_api_inventory() {
    let version = &crate::VERSION;
    assert_eq!(version.spec_name.as_ref(), "sora-substrate");
    assert_eq!(version.impl_name.as_ref(), "sora-substrate");
    assert_eq!(version.authoring_version, 1);
    assert_eq!(version.spec_version, 134);
    assert_eq!(version.impl_version, 1);
    assert_eq!(version.transaction_version, 131);
    assert_eq!(version.system_version, 0);
    let expected: &[([u8; 8], u32)] = &[
        ([0xdf, 0x6a, 0xcb, 0x68, 0x99, 0x07, 0x60, 0x9b], 5),
        ([0x37, 0xe3, 0x97, 0xfc, 0x7c, 0x91, 0xf5, 0xe4], 2),
        ([0x40, 0xfe, 0x3a, 0xd4, 0x01, 0xf8, 0x95, 0x9a], 6),
        ([0xd2, 0xbc, 0x98, 0x97, 0xee, 0xd0, 0x8f, 0x15], 3),
        ([0xf7, 0x8b, 0x27, 0x8b, 0xe5, 0x3f, 0x45, 0x4c], 2),
        ([0xab, 0x3c, 0x05, 0x72, 0x29, 0x1f, 0xeb, 0x8b], 2),
        ([0x37, 0xc8, 0xbb, 0x13, 0x50, 0xa9, 0xa2, 0xa8], 4),
        ([0x49, 0xa1, 0x51, 0x04, 0xd1, 0x4d, 0xe4, 0x33], 1),
        ([0x3c, 0x17, 0x4d, 0x77, 0x64, 0x06, 0x9e, 0x61], 2),
        ([0x0f, 0x34, 0x89, 0x93, 0x7d, 0x1c, 0x20, 0x64], 1),
        ([0x7b, 0x08, 0xda, 0xa9, 0xd1, 0x17, 0xc8, 0x9b], 1),
        ([0xd8, 0x3d, 0xc5, 0x1e, 0x16, 0xa8, 0x13, 0xfc], 1),
        ([0xf8, 0xe0, 0xc2, 0x52, 0x00, 0xc4, 0x66, 0xcc], 1),
        ([0x77, 0x14, 0xae, 0x71, 0x97, 0xb0, 0x46, 0x74], 1),
        ([0xfa, 0x5d, 0xc1, 0x15, 0xe8, 0xdd, 0x75, 0x9b], 3),
        ([0x8f, 0xd8, 0x9f, 0xd4, 0x1f, 0xae, 0xf8, 0x76], 1),
        ([0xc8, 0x90, 0xb8, 0x83, 0x73, 0x95, 0xd7, 0x03], 1),
        ([0x5c, 0xb9, 0x28, 0xf4, 0x41, 0x0f, 0x41, 0xfc], 1),
        ([0xcb, 0xca, 0x25, 0xe3, 0x9f, 0x14, 0x23, 0x87], 2),
        ([0xbc, 0x9d, 0x89, 0x90, 0x4f, 0x5b, 0x92, 0x3f], 1),
        ([0x49, 0xea, 0xaf, 0x1b, 0x54, 0x8a, 0x0c, 0xb0], 6),
        ([0x91, 0xd5, 0xdf, 0x18, 0xb0, 0xd2, 0xcf, 0x58], 3),
        ([0xed, 0x99, 0xc5, 0xac, 0xb2, 0x5e, 0xed, 0xf5], 3),
        ([0xeb, 0x4d, 0x48, 0x5a, 0xe8, 0x9f, 0xbc, 0xb5], 1),
        ([0x8e, 0x68, 0x8f, 0xe9, 0x45, 0x4a, 0xc9, 0x70], 1),
        ([0x4e, 0xe6, 0xf9, 0x74, 0xcd, 0x3d, 0x43, 0xf9], 2),
        ([0x98, 0xde, 0xae, 0xd7, 0x65, 0x2a, 0xef, 0xf9], 1),
        ([0xfb, 0xc5, 0x77, 0xb9, 0xd7, 0x47, 0xef, 0xd6], 1),
    ];
    assert_eq!(version.apis.as_ref(), expected);
}
