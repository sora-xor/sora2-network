#!/usr/bin/env python3
"""Verify the production build without accepting skipped or stale Wasm outputs."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
from uuid import uuid4
from build_environment import prepare_build_environment

ROOT = Path(__file__).resolve().parents[2]
V = ROOT / 'runtime-rehearsal-async-staking-2026-10-25/validation'
L = V / 'final-build-logs'
F = V / 'source-clippy-start.json'
REPORT = V / 'final-build-verification.json'
EXPORTS = ('Cargo.toml', 'framenode_runtime.compact.wasm',
           'framenode_runtime.compact.compressed.wasm')

def require(condition, message):
    if not condition:
        raise RuntimeError(message)

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def quarantine_wasm_exports(project, archive):
    """Move only generated exports, retaining bloaty Wasm and compiler caches."""
    require(not archive.exists(), 'Build export archive already exists')
    archive.mkdir(parents=True)
    old = []
    for name in EXPORTS:
        path = project / name
        require(not path.is_symlink(), f'Unexpected export symlink: {path}')
        if path.exists():
            require(path.is_file(), f'Generated export is not a file: {path}')
            old.append({'name': name, 'sha256': sha(path), 'bytes': path.stat().st_size,
                        'archivePath': str(archive / name)})
            path.rename(archive / name)
    require(all(not (project / name).exists() for name in EXPORTS), 'Old Wasm exports remain')
    return {'quarantinedAt': now(), 'absentBeforeBuild': list(EXPORTS), 'previousExports': old}

def verify_wasm_exports(project):
    """A successful Cargo exit alone cannot prove that exports were recreated."""
    result = []
    for name in EXPORTS:
        path = project / name
        require(path.is_file() and not path.is_symlink() and path.stat().st_size > 0,
                f'Production build did not recreate {name}')
        result.append({'name': name, 'sha256': sha(path), 'bytes': path.stat().st_size})
    manifest = tomllib.loads((project / 'Cargo.toml').read_text())
    dependency = manifest['dependencies']['wasm-project']
    require(dependency.get('default-features') is False, 'Wasm default features enabled')
    require(dependency.get('features') == ['build-wasm-binary'], 'Nonproduction Wasm features')
    return dependency, result

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reuse-source-fingerprint', action='store_true',
                        help='Check and retain the existing final fingerprint; fail if source changed')
    args = parser.parse_args(argv)
    run_id = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ-') + uuid4().hex
    environment, policy = prepare_build_environment(os.environ, ROOT, run_token=run_id)
    L.mkdir(exist_ok=True)
    report = {'status': 'running', 'runId': run_id, 'startedAt': now(),
              'workingDirectory': str(ROOT), 'environment': policy['controlled'],
              'environmentPolicy': policy, 'sourceFingerprintReused': args.reuse_source_fingerprint,
              'steps': []}

    def save():
        temporary = REPORT.with_suffix('.json.tmp')
        temporary.write_text(json.dumps(report, indent=2) + '\n')
        os.replace(temporary, REPORT)

    def run(label, command):
        out, err = L / (label + '.stdout.log'), L / (label + '.stderr.log')
        row = {'name': label, 'command': command, 'startedAt': now()}
        report['steps'].append(row)
        save()
        print('Running ' + label, flush=True)
        with out.open('wb') as stdout, err.open('wb') as stderr:
            process = subprocess.run(command, cwd=ROOT, env=environment, stdout=stdout, stderr=stderr)
        row.update(exitCode=process.returncode, finishedAt=now(),
                   stdout=str(out.relative_to(V)), stdoutSha256=sha(out),
                   stderr=str(err.relative_to(V)), stderrSha256=sha(err))
        save()
        require(process.returncode == 0, f'{label} failed with {process.returncode}; inspect {err}')
        return out, err

    try:
        require(__debug__, 'Build verification requires Python assertions; do not use -O')
        save()
        if args.reuse_source_fingerprint:
            require(F.is_file(), 'No final source fingerprint to reuse')
            require(json.loads(F.read_text())['status'] == 'final', 'Source fingerprint is not final')
            run('source-start', [sys.executable, str(V / 'source-fingerprint.py'), '--check', str(F),
                                '--source-only', '--output', str(V / 'source-build-start-check.json')])
        else:
            if F.exists():
                archive = V / 'attempts' / ('source-before-' + run_id)
                archive.mkdir(parents=True)
                shutil.copy2(F, archive / F.name)
            run('source-start', [sys.executable, str(V / 'source-fingerprint.py'), '--final',
                                '--label', 'final-clippy-and-production-build', '--output', str(F)])
        source = json.loads(F.read_text())
        reference = {'path': F.name, 'sha256': sha(F), 'sourceSha256': source['source']['sha256'],
                     'sourceFiles': source['source']['fileCount'], 'gitHead': source['git']['head']}
        report['sourceFingerprint'] = reference
        report['buildTools'] = {name: sha(V / name) for name in (
            'run-build-verification.py', 'build_environment.py',
            'test-build-environment.py', 'test-build-verification.py')}
        save()
        run('build-policy-tests', [sys.executable, str(V / 'test-build-verification.py'),
                                   '--include-environment'])
        run('clippy-all', ['housekeeping/clippy.sh', 'all', '--message-format=json'])
        run('source-after-clippy', [sys.executable, str(V / 'source-fingerprint.py'), '--check', str(F),
                                    '--source-only', '--output', str(V / 'source-clippy-end-check.json')])
        run('format', ['cargo', 'fmt', '--all', '--check'])
        graph, _ = run('production-feature-graph', ['cargo', 'tree', '--locked', '-p', 'framenode-runtime',
                       '--features', 'build-wasm-binary', '-e', 'features', '-i', 'framenode-runtime', '--prefix', 'none'])
        text = graph.read_text()
        require('framenode-runtime feature "build-wasm-binary"' in text, 'Wasm feature absent')
        for forbidden in ['private-net', 'stage', 'wip', 'runtime-benchmarks', 'try-runtime']:
            require(f'framenode-runtime feature "{forbidden}"' not in text, forbidden)
        project = ROOT / 'target/debug/wbuild/framenode-runtime'
        freshness = quarantine_wasm_exports(project, V / 'attempts' / ('wasm-exports-before-' + run_id))
        freshness['forceBuildToken'] = run_id
        report['artifactFreshness'] = freshness
        save()
        command = ['scripts/with_llvm_env.sh', 'cargo', 'check', '--locked', '-p',
                   'framenode-runtime', '--lib', '--features', 'build-wasm-binary']
        run('production-wasm', command)
        dependency, recreated = verify_wasm_exports(project)
        freshness.update(recreatedAfterBuild=recreated, verifiedAt=now())
        candidate = V.parent / 'candidate-built.compact.compressed.wasm'
        shutil.copy2(project / EXPORTS[2], candidate)
        run('source-after-build', [sys.executable, str(V / 'source-fingerprint.py'), '--check', str(F),
                                  '--source-only', '--output', str(V / 'source-build-end-check.json')])
        require(sha(F) == reference['sha256'], 'Source fingerprint changed during build')
        for name, digest in report['buildTools'].items():
            require(sha(V / name) == digest, 'Build verification tooling changed during execution')
        toolchain = subprocess.check_output(['rustc', '-Vv'], cwd=ROOT, env=environment, text=True)
        evidence = {'status': 'passed', 'createdAt': now(), 'buildRunId': run_id,
                    'candidateSha256': sha(candidate), 'candidateBytes': candidate.stat().st_size,
                    'candidatePath': str(candidate), 'specVersion': 137, 'transactionVersion': 133,
                    'command': command, 'buildCommand': ' '.join(command),
                    'sourceInputs': {name: sha(ROOT / name) for name in
                                     ['runtime/src/lib.rs', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml']},
                    'environment': policy['controlled'], 'environmentPolicy': policy,
                    'buildTools': report['buildTools'], 'artifactFreshness': freshness,
                    'sourceFingerprint': reference, 'sourceFingerprintReused': args.reuse_source_fingerprint,
                    'rustc': toolchain, 'wasmProjectManifestSha256': sha(project / 'Cargo.toml'),
                    'wasmProjectDependency': dependency, 'verificationReport': REPORT.name}
        (V / 'candidate-build-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
        report.update(candidate=evidence, status='passed', finishedAt=now())
        save()
        print(json.dumps({'status': 'passed', 'candidate': str(candidate), 'sha256': sha(candidate)}), flush=True)
        return 0
    except Exception as error:
        report.update(status='failed', error=str(error), finishedAt=now())
        save()
        print(str(error), file=sys.stderr)
        return 1

if __name__ == '__main__':
    sys.exit(main())
