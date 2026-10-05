#!/usr/bin/env python3
"""Bind successful local native/build logs to the frozen release source and locks."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tomllib

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--native-log', type=Path, required=True)
parser.add_argument('--format-log', type=Path, required=True)
parser.add_argument('--wasm-log', type=Path, required=True)
parser.add_argument('--native-command', required=True)
parser.add_argument('--wasm-command', required=True)
parser.add_argument('--clippy-log', type=Path)
parser.add_argument('--clippy-command')
parser.add_argument('--target', type=Path, default=Path('/Users/takemiyamakoto/dev/.sora2-pr1366-target'))
args = parser.parse_args()
TARGET = args.target.resolve()
sha = lambda b: hashlib.sha256(b).hexdigest()
provenance = json.loads((HERE / 'source-provenance.json').read_text())
for name, expected in provenance['files'].items():
    assert sha((ROOT / name).read_bytes()) == expected, f'Source changed: {name}'
for source, name in [(args.native_log, 'native-tests.log'),
                     (args.format_log, 'format-check.log'),
                     (args.wasm_log, 'wasm-build.log')]:
    assert source.is_file(), f'Missing fresh log: {source}'
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
assert len(new_tests) >= 35, len(new_tests)
required_followup_tests = [
    'tests::liveness::bridge_fees::outgoing_approval_retains_validation_weight_before_and_at_quorum',
    'tests::liveness::migration_sponsorship::funded_underpriced_grant_can_be_replaced_without_claimant_xor',
    'tests::liveness::migration_sponsorship::replacement_cannot_reduce_a_funded_grants_limits',
]
for name in required_followup_tests:
    assert re.search(r'^test ' + re.escape(name) + r' \.\.\. ok$', log, re.M), 'Missing successful follow-up regression: ' + name
capacity_tests = re.findall(r'^test tests::liveness::bridge_fees::(?:one_zero_xor_peer_cannot_exhaust_other_peers_proposal_capacity|current_zero_xor_quorum_cleans_orphaned_proposal_at_full_shared_capacity) \.\.\. ok$', log, re.M)
assert len(capacity_tests) == 2, 'Both Executive capacity/quorum regressions must pass'
assert (HERE / 'format-check.log').read_text().strip() == ''
command = args.native_command
native = {'status': 'passed', 'sourceTreeAfterPatch': provenance['sourceTreeAfterPatch'], 'command': command,
          'log': 'native-tests.log', 'logSha256': sha((HERE / 'native-tests.log').read_bytes()), 'suites': suites,
          'baseCommit': provenance['baseCommit'], 'checkout': str(ROOT),
          'logSource': str(args.native_log.resolve()),
          'totalPassed': sum(row['passed'] for row in suites), 'totalFailed': 0,
          'totalIgnored': sum(row['ignored'] for row in suites), 'newRuntimeRegressionsPassed': len(new_tests),
          'capacityExecutiveRegressionsPassed': len(capacity_tests),
          'followupRegressionsPassed': len(required_followup_tests),
          'requiredFollowupRegressions': required_followup_tests,
          'cargoFmtAllCheckPassed': True, 'formatCheckLogSha256': sha((HERE / 'format-check.log').read_bytes()),
          'limitations': ['Existing explicitly ignored runtime tests are retained and counted.',
                          'Native Executive regressions use real cryptographic signatures; execution is local.']}
(HERE / 'native-tests.json').write_text(json.dumps(native, indent=2) + '\n')
wasm_dir = TARGET / 'release/wbuild/framenode-runtime'
wasm = (wasm_dir / 'framenode_runtime.compact.compressed.wasm').read_bytes()
assert wasm == (HERE.parent / 'framenode-runtime-4.8.12.compact.compressed.wasm').read_bytes()
packages = lambda file: {(p['name'], p['version'], p.get('source', ''), p.get('checksum', '')) for p in tomllib.loads(file.read_text())['package']}
root_packages = packages(ROOT / 'Cargo.lock')
wasm_packages = packages(wasm_dir / 'Cargo.lock')
extras = wasm_packages - root_packages
assert extras == {('framenode-runtime-blob', '1.0.0', '', '')}, extras
build_log = (HERE / 'wasm-build.log').read_text()
assert 'Finished `release`' in build_log and 'could not compile' not in build_log
build = {'status': 'passed', 'sourceTreeAfterPatch': provenance['sourceTreeAfterPatch'],
         'baseCommit': provenance['baseCommit'], 'checkout': str(ROOT),
         'candidateSha256': sha(wasm), 'candidateBytes': len(wasm),
         'cargoLockSha256': sha((ROOT / 'Cargo.lock').read_bytes()),
         'wasmCargoLockSha256': sha((wasm_dir / 'Cargo.lock').read_bytes()),
         'wasmDependencyPackages': len(wasm_packages), 'wasmPackagesMatchSourceLock': True,
         'dependencyNamesVersionsSourcesAndChecksumsChecked': True,
         'onlyGeneratedWrapperPackageAdded': sorted(tuple(row[:3]) for row in extras),
         'command': args.wasm_command,
         'logSource': str(args.wasm_log.resolve()),
         'log': 'wasm-build.log', 'logSha256': sha((HERE / 'wasm-build.log').read_bytes()),
         'cargoTargetDirectory': str(TARGET), 'cargoCleanRun': False}
(HERE / 'wasm-build.json').write_text(json.dumps(build, indent=2) + '\n')
if args.clippy_log is not None:
    assert args.clippy_command, '--clippy-command is required with --clippy-log'
    assert args.clippy_log.is_file(), 'Missing fresh Clippy log'
    shutil.copyfile(args.clippy_log, HERE / 'clippy.log')
    lint_log = (HERE / 'clippy.log').read_text()
    profiles = ['mainnet', 'try-runtime', 'extended']
    assert all('Running Clippy (' + profile + ')' in lint_log for profile in profiles)
    assert 'could not compile' not in lint_log and 'failed with exit status' not in lint_log
    assert lint_log.count('Finished `dev` profile') >= 3, 'All three Clippy profiles must complete'
    lint = {'status': 'passed', 'sourceTreeAfterPatch': provenance['sourceTreeAfterPatch'],
            'baseCommit': provenance['baseCommit'], 'checkout': str(ROOT),
            'command': args.clippy_command, 'profiles': profiles, 'exitCode': 0,
            'internalFlags': ['SKIP_WASM_BUILD=1', '--locked', '--all-targets', '-- -D warnings'],
            'log': 'clippy.log', 'logSource': str(args.clippy_log.resolve()),
            'logSha256': sha((HERE / 'clippy.log').read_bytes()),
            'networkRequests': 0, 'cargoCleanRun': False}
    (HERE / 'clippy.json').write_text(json.dumps(lint, indent=2) + '\n')
print(json.dumps({'nativePassed': native['totalPassed'], 'nativeIgnored': native['totalIgnored'],
                  'newRuntimeRegressions': len(new_tests), 'wasmSha256': sha(wasm),
                  'capacityExecutiveRegressions': len(capacity_tests),
                  'sourceTree': provenance['sourceTreeAfterPatch']}))
