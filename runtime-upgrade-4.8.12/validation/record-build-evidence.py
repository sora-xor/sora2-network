#!/usr/bin/env python3
"""Bind successful local native/build logs to the frozen release source and locks."""
import hashlib
import json
from pathlib import Path
import re
import shutil
import tomllib

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
TARGET = Path('/Users/takemiyamakoto/dev/.sora2-pr1366-target')
sha = lambda b: hashlib.sha256(b).hexdigest()
provenance = json.loads((HERE / 'source-provenance.json').read_text())
for name, expected in provenance['files'].items():
    assert sha((ROOT / name).read_bytes()) == expected, f'Source changed: {name}'
for source, name in [('/tmp/sora-final-native-tests.log', 'native-tests.log'),
                     ('/tmp/sora-final-fmt.log', 'format-check.log'),
                     ('/tmp/sora-final-wasm-build.log', 'wasm-build.log')]:
    shutil.copyfile(source, HERE / name)
log = (HERE / 'native-tests.log').read_text()
assert 'error: test failed' not in log and 'could not compile' not in log
suite = None
suites = []
for line in log.splitlines():
    found = re.search(r'Running unittests .*?/deps/([\w]+)-[0-9a-f]+', line)
    if found:
        suite = found[1]
    found = re.search(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored', line)
    if found:
        assert suite is not None
        suites.append(dict(zip(['crate', 'passed', 'failed', 'ignored'], [suite, *map(int, found.groups())])))
        suite = None
assert len(suites) == 11, suites
assert all(row['failed'] == 0 for row in suites)
new_tests = re.findall(r'^test tests::liveness::(?:equivocation_fees|equivocation_bridge|feeless_success|funded_keepers|migration_sponsorship|bridge_fees)::.* \.\.\. ok$', log, re.M)
assert len(new_tests) >= 28, len(new_tests)
assert (HERE / 'format-check.log').read_text().strip() == ''
command = 'SKIP_WASM_BUILD=1 CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/.sora2-pr1366-target scripts/with_llvm_env.sh cargo test --release --offline --locked -p framenode-runtime -p pallet-babe -p pallet-grandpa -p pallet-staking -p pallet-multisig -p eth-bridge -p iroha-migration -p xor-fee -p order-book -p rewards -p pallet-preimage --lib'
native = {'status': 'passed', 'sourceTreeAfterPatch': provenance['sourceTreeAfterPatch'], 'command': command,
          'log': 'native-tests.log', 'logSha256': sha((HERE / 'native-tests.log').read_bytes()), 'suites': suites,
          'totalPassed': sum(row['passed'] for row in suites), 'totalFailed': 0,
          'totalIgnored': sum(row['ignored'] for row in suites), 'newRuntimeRegressionsPassed': len(new_tests),
          'cargoFmtAllCheckPassed': True, 'formatCheckLogSha256': sha((HERE / 'format-check.log').read_bytes()),
          'limitations': ['Existing explicitly ignored runtime tests are retained and counted.',
                          'Native Executive regressions use real cryptographic signatures; execution is local.']}
(HERE / 'native-tests.json').write_text(json.dumps(native, indent=2) + '\n')
wasm_dir = TARGET / 'release/wbuild/framenode-runtime'
wasm = (wasm_dir / 'framenode_runtime.compact.compressed.wasm').read_bytes()
assert wasm == (HERE.parent / 'framenode-runtime-4.8.12.compact.compressed.wasm').read_bytes()
packages = lambda file: {(p['name'], p['version'], p.get('source', '')) for p in tomllib.loads(file.read_text())['package']}
root_packages = packages(ROOT / 'Cargo.lock')
wasm_packages = packages(wasm_dir / 'Cargo.lock')
extras = wasm_packages - root_packages
assert extras == {('framenode-runtime-blob', '1.0.0', '')}, extras
build_log = (HERE / 'wasm-build.log').read_text()
assert 'Finished `release`' in build_log and 'could not compile' not in build_log
build = {'status': 'passed', 'sourceTreeAfterPatch': provenance['sourceTreeAfterPatch'],
         'candidateSha256': sha(wasm), 'candidateBytes': len(wasm),
         'cargoLockSha256': sha((ROOT / 'Cargo.lock').read_bytes()),
         'wasmCargoLockSha256': sha((wasm_dir / 'Cargo.lock').read_bytes()),
         'wasmDependencyPackages': len(wasm_packages), 'wasmPackagesMatchSourceLock': True,
         'onlyGeneratedWrapperPackageAdded': sorted(extras),
         'command': 'env -u SKIP_WASM_BUILD CARGO_TARGET_DIR=/Users/takemiyamakoto/dev/.sora2-pr1366-target WASM_BUILD_WORKSPACE_HINT=/Users/takemiyamakoto/dev/sora2-network WASM_BUILD_CARGO_ARGS=--offline FORCE_WASM_BUILD=4.8.12-zero-xor-final CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/clang AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm@21/bin/llvm-ar LIBCLANG_PATH=/opt/homebrew/opt/llvm@21/lib LLVM_CONFIG_PATH=/opt/homebrew/opt/llvm@21/bin/llvm-config scripts/with_llvm_env.sh cargo build --release --offline --locked -p framenode-runtime --features build-wasm-binary',
         'log': 'wasm-build.log', 'logSha256': sha((HERE / 'wasm-build.log').read_bytes()),
         'cargoTargetDirectory': str(TARGET), 'cargoCleanRun': False}
(HERE / 'wasm-build.json').write_text(json.dumps(build, indent=2) + '\n')
print(json.dumps({'nativePassed': native['totalPassed'], 'nativeIgnored': native['totalIgnored'],
                  'newRuntimeRegressions': len(new_tests), 'wasmSha256': sha(wasm),
                  'sourceTree': provenance['sourceTreeAfterPatch']}))
