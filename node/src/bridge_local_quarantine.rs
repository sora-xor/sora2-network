//! Persistent local SORA Nexus signing policy for seven abandoned-branch requests.
//!
//! The reviewed on-chain Wasm already checks exact quarantine-marker existence
//! before signing and when processing retained retries. Only its OCW invocation
//! sees these additional markers. Block import, authoring, RPC and chain storage
//! retain the original host behavior. No balances or on-chain records change.
//! Unknown runtime code is denied HASHI key/pool access by WorkerGuard; changing
//! this host policy requires a new reviewed node build, not a runtime upgrade.

use sp_core::{offchain::OffchainWorkerExt, H256};
use sp_externalities::ExternalitiesExt;
use sp_runtime_interface::{pass_by::PassFatPointerAndRead, runtime_interface};

pub(crate) const REQUESTS: [&str; 7] = [
    "d3a1de97f04701cbd06c9e2381ede26625470cf139beef9914c9af2a8169142b",
    "bff3e45fa4199f3d6e1e86ca95b01af6638c6c96e8df8e2fe385e7943471e355",
    "a5d9c30273d4275051856bf10294c560b90d3cd1a0f4a9f6e30c1f304b990c32",
    "de74bb6dacaff9754610f126748f3ada99cd1126faaee197ef1094d013cb4446",
    "1f46405b429b364c701ad7b7c45b72a1ab34eda0c294eae928d8c2bdddc92619",
    "f2527f9d613ffab277baceb480e3df0c233dd46cc09bc7938f51b330c0690863",
    "4625cedbd2bca1c577e374564beab157bc6104ce42b867f8980d8b860fb38202",
];
const PREFIX: &[u8] = b":sora-nexus:recovery:outgoing-hashi:v1:";
const GENESIS: &str = "7e4e32d0feafd4f9c9414b0be86373f9a1efa904809b683453a9af6856d38ad5";
// Admit the published spec 142 and the withdrawal-scanner spec 143 during rollout.
const REVIEWED_CODES: [&str; 2] = [
    "d601b781566850086b9341552bcde75b43902665a01fc94897da2adf3cac9a26",
    "c4633932066e1d36ce307c476f76ce8a9833f2c8bc9ea97837febf52ed90da68",
];

sp_externalities::decl_extension! {
    /// Installed only by the node's offchain-worker runner; Wasm cannot create it.
    pub(crate) struct LocalQuarantineExt(());
}

pub(crate) fn extension() -> Box<dyn sp_externalities::Extension> {
    Box::new(LocalQuarantineExt(()))
}

pub(crate) fn reviewed_code(genesis: H256, code: Option<H256>) -> bool {
    format!("{genesis:x}") == GENESIS
        && code.is_some_and(|hash| REVIEWED_CODES.contains(&format!("{hash:x}").as_str()))
}

/// Key access requires the worker's runtime to match the currently reviewed best-chain code.
pub(crate) fn reviewed_worker_code(
    genesis: H256,
    best: Option<H256>,
    worker: Option<H256>,
) -> bool {
    best == worker && reviewed_code(genesis, best)
}

fn blocked_key(key: &[u8]) -> bool {
    let Some(identity) = key.strip_prefix(PREFIX) else {
        return false;
    };
    if identity.len() != 36 || identity[..4] != [0; 4] {
        return false;
    }
    REQUESTS.contains(&format!("{:x}", H256::from_slice(&identity[4..])).as_str())
}

/// Override only the Wasm import with the same name as sp_io::Storage::exists.
/// Both the trusted local marker and the SDK OCW capability are required.
#[runtime_interface]
pub trait Storage {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "the generated native wrapper is used by tests; production registers the Wasm host function"
        )
    )]
    fn exists(&mut self, key: PassFatPointerAndRead<&[u8]>) -> bool {
        let local = self.extension::<LocalQuarantineExt>().is_some()
            && self.extension::<OffchainWorkerExt>().is_some();
        (local && blocked_key(key)) || self.exists_storage(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::offchain::testing::TestOffchainExt;
    use std::str::FromStr;

    fn key(hash: &str) -> Vec<u8> {
        [PREFIX, &[0; 4], H256::from_str(hash).unwrap().as_bytes()].concat()
    }

    fn ext(local: bool, worker: bool) -> sp_io::TestExternalities {
        let mut ext = sp_io::TestExternalities::default();
        if local {
            ext.register_extension(extension());
        }
        if worker {
            let (offchain, _) = TestOffchainExt::new();
            ext.register_extension(OffchainWorkerExt::new(offchain));
        }
        ext
    }

    #[test]
    fn all_seven_are_blocked_only_in_the_local_worker_without_storage_writes() {
        for local in [false, true] {
            for worker in [false, true] {
                let mut ext = ext(local, worker);
                ext.execute_with(|| {
                    let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
                    for hash in REQUESTS {
                        let k = key(hash);
                        assert_eq!(storage::exists(&k), local && worker);
                        assert!(sp_io::storage::get(&k).is_none());
                    }
                    assert_eq!(before, sp_io::storage::root(sp_runtime::StateVersion::V1));
                    assert!(!storage::exists(common::recovery::BRIDGE_HALT_STORAGE_KEY));
                });
            }
        }
    }

    #[test]
    fn unrelated_hashes_networks_and_real_storage_keep_their_meaning() {
        ext(true, true).execute_with(|| {
            let mut wrong_network = key(REQUESTS[0]);
            wrong_network[PREFIX.len()] = 1;
            assert!(!storage::exists(&wrong_network));
            let mut wrong_hash = key(REQUESTS[0]);
            *wrong_hash.last_mut().unwrap() ^= 1;
            assert!(!storage::exists(&wrong_hash));
            assert!(!storage::exists(&key(REQUESTS[0])[..PREFIX.len() + 35]));
            let mut too_long = key(REQUESTS[0]);
            too_long.push(0);
            assert!(!storage::exists(&too_long));
            sp_io::storage::set(b"unrelated", b"present");
            assert!(storage::exists(b"unrelated"));
            assert_eq!(
                sp_io::storage::get(b"unrelated").as_deref(),
                Some(&b"present"[..])
            );
        });
    }

    #[test]
    fn worker_uses_the_live_runtime_during_the_rolling_upgrade() {
        let genesis = H256::from_str(GENESIS).unwrap();
        let [previous, current] = REVIEWED_CODES.map(|hash| Some(H256::from_str(hash).unwrap()));
        let unknown = Some(H256::repeat_byte(1));
        assert_ne!(previous, current);
        for code in [previous, current] {
            assert!(reviewed_worker_code(genesis, code, code));
            assert!(!reviewed_worker_code(H256::zero(), code, code));
            assert!(!reviewed_worker_code(genesis, code, None));
            assert!(!reviewed_worker_code(genesis, None, code));
            assert!(!reviewed_worker_code(genesis, code, unknown));
        }
        assert!(!reviewed_worker_code(genesis, previous, current));
        assert!(!reviewed_worker_code(genesis, current, previous));
        assert!(!reviewed_worker_code(genesis, None, None));
        assert!(!reviewed_worker_code(genesis, unknown, unknown));
    }

    #[test]
    fn wasm_import_override_is_installed_once_and_is_context_scoped() {
        use sc_executor::HostFunctions;
        use sc_executor_common::runtime_blob::RuntimeBlob;
        // This tiny fixture calls ext_storage_exists_version_1 with its input
        // and returns a four-byte bool. It executes through the production
        // HostFunctions/Wasmtime path, not the native test helper above.
        let code = include_bytes!("bridge_local_quarantine_probe.wasm");
        let functions = crate::service::HostFunctions::host_functions();
        assert_eq!(
            functions
                .iter()
                .filter(|f| f.name() == "ext_storage_exists_version_1")
                .count(),
            1
        );
        let executor =
            sc_executor::WasmExecutor::<crate::service::HostFunctions>::builder().build();
        for local in [false, true] {
            for worker in [false, true] {
                let mut ext = ext(local, worker);
                for hash in REQUESTS {
                    let output = executor
                        .uncached_call(
                            RuntimeBlob::uncompress_if_needed(code).unwrap(),
                            &mut ext.ext(),
                            false,
                            "probe",
                            &key(hash),
                        )
                        .unwrap();
                    assert_eq!(output, u32::from(local && worker).to_le_bytes());
                }
            }
        }
    }
}
