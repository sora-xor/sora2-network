#!/usr/bin/env python3
"""Prepare/run the pinned native rehearsal using an existing warm Cargo lane."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

if not __debug__:
    raise RuntimeError('Optimized Python disables validation assertions; run without -O/PYTHONOPTIMIZE')

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_feature_graph(text):
    forbidden = {'private-net', 'stage', 'wip', 'runtime-benchmarks', 'reduced-pswap-reward-periods'}
    packages = {}
    for raw in text.splitlines():
        line = raw.strip().removesuffix(' (*)')
        if not line or line in ('[build-dependencies]', '[dev-dependencies]'):
            continue
        if '|' not in line:
            raise RuntimeError(f'Unrecognized Cargo feature graph row: {line}')
        package, _, encoded = line.rpartition('|')
        features = {value for value in encoded.split(',') if value}
        disallowed = features & forbidden
        if disallowed:
            raise RuntimeError(f'Nonproduction features unified into {package}: {sorted(disallowed)}')
        packages.setdefault(package, set()).update(features)
    runtime = [features for package, features in packages.items() if package.split()[0] == 'framenode-runtime']
    if len(runtime) != 1 or not {'std', 'try-runtime'} <= runtime[0]:
        raise RuntimeError('Feature graph must contain exactly one framenode-runtime with std,try-runtime')
    return {'runtimeFeatures': sorted(runtime[0]), 'forbiddenFeatures': sorted(forbidden),
            'nativeTestInstrumentation': {'commonTestEnabled': any(package.split()[0] == 'common' and 'test' in features for package, features in packages.items()),
                                          'meaning': 'common/test uses stricter fixed-point balance conversion and test include fallbacks; exact production semantics are checked by the separate default-production Wasm fork.'},
            'packages': [{'package': name, 'features': sorted(features)} for name, features in sorted(packages.items())]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True, help='Existing warm Cargo target directory; coordinate with its owner before --execute')
    parser.add_argument('--execute', action='store_true', help='Actually compile and run; omission only prepares the exact command')
    parser.add_argument('--source-fingerprint', type=Path, help='Final source fingerprint shared with the Clippy/candidate-build gates; required for execution')
    args = parser.parse_args()
    target = args.target_dir.resolve()
    assert target.is_dir(), 'Reuse an existing warm Cargo target directory'
    if args.execute:
        assert args.source_fingerprint and args.source_fingerprint.is_file(), 'Pass the final --source-fingerprint used by the build owner'
        assert json.loads(args.source_fingerprint.read_text())['status'] == 'final', 'Execution requires an explicitly frozen final-source fingerprint'
    snapshot = json.loads((HERE / 'snapshot.json').read_text())
    scope_path = HERE / 'native-remote-scope.json'
    scope = json.loads(scope_path.read_text())
    assert scope['status'] == 'preflight-passed-scope-prepared'
    assert snapshot['status'] == 'passed'
    assert scope['sourceBlockHash'] == snapshot['blockHash']
    assert scope['inputs']['snapshotSha256'] == digest(HERE / 'snapshot.json')
    assert scope['inputs']['deployedCodeSha256'] == snapshot['code']['sha256']
    assert snapshot['endpoint'] == scope['endpoint'] == 'wss://mof2.sora.org'
    cache = HERE / f"native-{snapshot['blockNumber']}-{digest(scope_path)[:16]}.snap"
    assert not cache.exists(), f'Refusing existing snapshot with unverified scope: {cache}'
    env = os.environ.copy()
    for name in list(env):
        if name.startswith('REMOTE_') or name in ('WS', 'SNAP', 'REQUIRE_REMOTE', 'RUSTFLAGS', 'RUSTDOCFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_ENCODED_RUSTDOCFLAGS'):
            del env[name]
    specific = {**scope['environment'], 'SKIP_WASM_BUILD': '1', 'SNAP': str(cache), 'CARGO_TARGET_DIR': str(target)}
    assert specific['REQUIRE_REMOTE'] == '1'
    assert specific['REMOTE_UPGRADE_CHECKS'] == 'pre-and-post'
    assert specific['REMOTE_STATE_VERSION'] == str(snapshot['runtimeVersion']['stateVersion'])
    env.update(specific)
    base = ['scripts/with_llvm_env.sh', 'cargo', 'test', '--locked', '-p', 'framenode-runtime', '--lib', '--no-default-features', '--features', 'std,try-runtime']
    commands = [base + ['remote_rehearsal_options', '--', '--nocapture'],
                base + ['remote_try_runtime_upgrade_rehearsal', '--', '--exact', '--nocapture']]
    source_paths = ['Cargo.toml', 'Cargo.lock', 'runtime/Cargo.toml', 'runtime/src/lib.rs',
                    'runtime/src/migrations.rs', 'runtime/src/liveness.rs', 'runtime/src/tests/remote.rs']
    sources = {name: digest(ROOT / name) for name in source_paths}
    report_path = HERE / 'native-remote-execution.json'
    report = {'status': 'prepared-not-run', 'createdAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'workingDirectory': str(ROOT), 'commands': commands, 'environment': specific,
              'scopeSha256': digest(scope_path), 'snapshotSha256': digest(HERE / 'snapshot.json'),
              'sourceInputs': sources, 'sourceInputScope': 'Selected entry points plus mandatory full source-fingerprint checks before and after execution.',
              'sourceFingerprint': str(args.source_fingerprint.resolve()) if args.source_fingerprint else None,
              'sourceCommit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'submittedTransactions': 0, 'privateKeysRead': 0,
              'submissionEvidence': {'basis': 'Source review of frame_remote_externalities: only read-only chain/header/runtime-version/state-key/storage RPC methods; Executive hooks execute against local externalities.',
                                     'transportGuardInstalled': False, 'networkRequestsCounted': False,
                                     'scope': 'Zero submissions by the reviewed code path, not an independent network-traffic measurement.'},
              'coverage': scope['execution'], 'limitations': scope['limitations']}
    def save():
        report_path.write_text(json.dumps(report, indent=2) + '\n')
    save()
    if not args.execute:
        print(json.dumps(report, indent=2))
        return 0
    report['status'] = 'running'
    report['results'] = []
    save()
    def check_source(stage):
        proof = HERE / f'native-source-{stage}-check.json'
        command = [sys.executable, str(HERE / 'source-fingerprint.py'), '--check', str(args.source_fingerprint.resolve()), '--source-only', '--output', str(proof)]
        process = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
        report.setdefault('sourceChecks', []).append({'stage': stage, 'command': command, 'exitCode': process.returncode,
                                                     'evidence': proof.name, 'evidenceSha256': digest(proof) if proof.exists() else None,
                                                     'stderr': process.stderr})
        save()
        assert process.returncode == 0, f'Full source identity changed at native {stage}; inspect {proof}'
    check_source('start')
    graph_command = ['scripts/with_llvm_env.sh', 'cargo', 'tree', '--locked', '-p', 'framenode-runtime',
                     '--no-default-features', '--features', 'std,try-runtime', '--edges', 'normal,build,dev',
                     '--prefix', 'none', '--color', 'never', '--format', '{p}|{f}']
    graph = subprocess.run(graph_command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, check=False)
    graph_path = HERE / 'native-production-feature-graph.txt'
    graph_path.write_text(graph.stdout)
    graph_error = HERE / 'native-production-feature-graph.stderr.log'
    graph_error.write_text(graph.stderr)
    report['featureGraph'] = {'command': graph_command, 'exitCode': graph.returncode,
                              'evidence': graph_path.name, 'sha256': digest(graph_path),
                              'stderrEvidence': graph_error.name, 'stderrSha256': digest(graph_error)}
    save()
    if graph.returncode != 0:
        raise RuntimeError('Cargo production feature-graph resolution failed')
    report['featureGraph'].update(validate_feature_graph(graph.stdout))
    report['featureGraph']['status'] = 'passed'
    save()
    for index, command in enumerate(commands):
        log = HERE / ('native-mode-regressions.log' if index == 0 else 'native-remote-rehearsal.log')
        with log.open('wb') as output:
            process = subprocess.run(command, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT, check=False)
        result = {'command': command, 'exitCode': process.returncode, 'log': log.name, 'logSha256': digest(log)}
        report['results'].append(result)
        if process.returncode != 0:
            report['status'] = 'failed'
            save()
            print(json.dumps(result, indent=2))
            return process.returncode
        text = log.read_text(errors='replace')
        assert 'Skipping remote migration test' not in text, 'Remote run silently skipped'
        if index == 1:
            assert '1 passed; 0 failed' in text
            assert f"block={snapshot['blockNumber']}, hash={snapshot['blockHash']}" in text
            assert 'Runtime upgrade rehearsal replay:' in text
        else:
            assert '4 passed; 0 failed' in text
        save()
    assert sources == {name: digest(ROOT / name) for name in source_paths}, 'Native source changed during execution; rerun with a stable source state'
    check_source('end')
    assert cache.is_file(), 'Expected newly written scoped snapshot is missing'
    report['stateSnapshot'] = {'path': cache.name, 'bytes': cache.stat().st_size, 'sha256': digest(cache)}
    report['status'] = 'passed'
    report['finishedAt'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
    save()
    print(json.dumps({'status': report['status'], 'evidence': str(report_path)}))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        evidence = HERE / 'native-remote-execution.json'
        if evidence.is_file():
            failed = json.loads(evidence.read_text())
            failed['status'] = 'failed'
            failed['error'] = str(error)
            evidence.write_text(json.dumps(failed, indent=2) + '\n')
        raise
