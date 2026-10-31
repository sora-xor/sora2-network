#!/usr/bin/env python3
"""Capture fresh dependency audits and explicitly historical SDK source evidence.

This runs cargo-audit and npm inspection/tests only; it never builds Rust, changes
lockfiles, submits transactions, or rewrites the prior SDK rehearsal evidence.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
HISTORICAL = REPO / 'runtime-rehearsal-sdk2609-2026-10-25/validation/imported-migration-evidence'
LOCKFILES = [
    'Cargo.lock', 'misc/farming_snapshot/convert/Cargo.lock',
    'misc/rewards_snapshot/get_lp_tokens/Cargo.lock', 'vendor/sora2-common/Cargo.lock',
    *[f'misc/runtime_upgrade/vendor/native/{package}/Cargo.lock' for package in
      ('py-bip39-bindings', 'py-ed25519-zebra-bindings', 'py-sr25519-bindings', 'pydantic-core')],
    'vendor/chopsticks-executor/Cargo.lock', 'vendor/polkadot-wasm-crypto/Cargo.lock',
    'vendor/tikv-jemalloc-ctl-0.6.1/Cargo.lock',
]
HARNESSES = ['network', 'signer', 'macro', 'identity', 'smoldot']
INSTALLED_CHECKS = {'executor-source-provenance', 'crypto-source-provenance', 'isolated-install',
    'native-integration', 'schema', 'fetch-blob', 'executor', 'crypto',
    'isolated-dependency-validation', 'rehearsal-dependency-validation'}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


def relative(path):
    return str(Path(path).resolve().relative_to(REPO))


def run(command, directory, output, env):
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open('w') as stdout, output.with_suffix('.stderr.log').open('w') as stderr:
        result = subprocess.run([str(arg) for arg in command], cwd=directory, env=env,
                                stdout=stdout, stderr=stderr, check=False)
    return result.returncode


def import_historical():
    original = read(HISTORICAL / 'import-summary.json')
    assert original['status'] == 'passed'
    for row in original['files']:
        source = (HISTORICAL / row['path']).resolve()
        assert source.is_relative_to(HISTORICAL.resolve())
        assert digest(source) == row['sha256'] == row['sourceSha256']
    destination = HERE / 'imported-migration-evidence'
    if not destination.exists():
        shutil.copytree(HISTORICAL, destination)
    assert digest(destination / 'import-summary.json') == digest(HISTORICAL / 'import-summary.json')
    for row in original['files']:
        assert digest(destination / row['path']) == row['sha256']
    save(destination / 'historical-reference.json', {
        'status': 'verified-historical-source-evidence', 'source': relative(HISTORICAL),
        'summarySha256': digest(destination / 'import-summary.json'),
        'scope': 'Prior SDK2609 source/harness review with unchanged reviewed package sources. Its old root lock audit and whole-workspace compilation are historical; current dependency audits and runtime validation are separate.',
    })
    return destination


def audit_cargo(imported, env, database):
    out = HERE / 'current-cargo-audits'
    summary = {'status': 'running', 'checkedAt': datetime.now(timezone.utc).isoformat(),
               'collectorSha256': digest(__file__), 'audits': []}
    save(out / 'summary.json', summary)
    try:
        def database_identity():
            def git(*args):
                return subprocess.run(['git', '-C', str(database), *args], check=True,
                                      capture_output=True, text=True).stdout.strip()
            assert not git('status', '--porcelain'), 'RustSec database has local changes'
            commit = git('rev-parse', 'HEAD')
            assert len(commit) == 40 and all(c in '0123456789abcdef' for c in commit)
            tracked = git('ls-files').splitlines()
            inventory = [[name, digest(database / name)] for name in tracked]
            return {'path': str(database.resolve()), 'commit': commit,
                    'tree': git('rev-parse', 'HEAD^{tree}'), 'clean': True,
                    'trackedFiles': len(inventory), 'trackedBytesSha256': hashlib.sha256(
                        json.dumps(inventory, separators=(',', ':')).encode()).hexdigest()}
        database_before = database_identity()
        database_proof = out / 'database-source.json'
        inputs = [('repository', REPO / path) for path in LOCKFILES]
        inputs += [('historical-harness', imported / f'{name}-harness/Cargo.lock') for name in HARNESSES]
        for index, (scope, lockfile) in enumerate(inputs):
            before = digest(lockfile)
            output = out / f'{index:02d}.json'
            command = ['cargo', 'audit', '--db', str(database), '--no-fetch', '--deny', 'warnings', '--json', '--file', str(lockfile)]
            exit_code = run(command, REPO, output, env)
            result = read(output)
            assert digest(lockfile) == before, 'Lock changed during audit'
            row = {'scope': scope, 'lockfile': relative(lockfile), 'lockSha256': before,
                   'command': command, 'exit': exit_code, 'audit': relative(output),
                   'auditSha256': digest(output), 'stderr': relative(output.with_suffix('.stderr.log')),
                   'stderrSha256': digest(output.with_suffix('.stderr.log')),
                   'databaseCommit': database_before['commit'],
                   'auditReportedDatabaseCommit': result['database']['last-commit'],
                   'vulnerabilities': result['vulnerabilities']['count'],
                   'warnings': sum(len(value) for value in result['warnings'].values())}
            summary['audits'].append(row)
            save(out / 'summary.json', summary)
            assert exit_code == row['vulnerabilities'] == row['warnings'] == 0
            assert result['settings']['ignore'] == []
            assert row['auditReportedDatabaseCommit'] in (None, database_before['commit'])
        commits = {row['databaseCommit'] for row in summary['audits']}
        assert len(commits) == 1, 'All audits must use the same freshly fetched advisory database'
        database_after = database_identity()
        assert database_after == database_before, 'RustSec source changed during audits'
        save(database_proof, {'status': 'passed', 'before': database_before,
                            'after': database_after, 'unchanged': True,
                            'scope': 'Independent Git and tracked-byte binding; cargo-audit --no-fetch may report a null last-commit.'})
        summary.update(status='passed', databaseCommit=commits.pop(), repositoryLockfiles=11,
                       historicalHarnessLockfiles=5, databaseSource=relative(database_proof),
                       databaseSourceSha256=digest(database_proof))
    except BaseException as error:
        summary.update(status='failed', error=str(error))
        raise
    finally:
        save(out / 'summary.json', summary)


def audit_npm(node, npm, env):
    helper = REPO / 'misc/dependency_security/prepare_npm_audit.py'
    spec = importlib.util.spec_from_file_location('npm_audit_stager', helper)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    out = HERE / 'npm-security-refresh'
    summary = {'status': 'running', 'checked_at_utc': datetime.now(timezone.utc).isoformat(),
               'node': str(node), 'npm': str(npm), 'helper_sha256': digest(helper), 'projects': []}
    installed = {'status': 'running', 'checks': [], 'native_rebuilt': False,
                 'scope': 'Fresh isolated npm install and native artifact/compatibility tests; Rust artifacts verified from their recorded source provenance.'}
    save(out / 'summary.json', summary)
    save(out / 'installed-checks.json', installed)
    stages = {}
    try:
        for index, project in enumerate(sorted(module.OPERATIONAL_PROJECTS)):
            directory = REPO / project
            manifest = 'rehearsal-package.json' if project == 'runtime-upgrade-4.8.9/validation' else 'package.json'
            lockfile = 'rehearsal-package-lock.json' if project == 'runtime-upgrade-4.8.9/validation' else 'package-lock.json'
            staging = Path(tempfile.mkdtemp(prefix='sora2-async-npm-'))
            staged = module.prepare(REPO, directory / manifest, directory / lockfile, staging)
            stages[project] = staged
            evidence = out / f'project-{index:02d}'
            closure = evidence / 'closure.json'
            closure_exit = run([npm, 'ls', '--package-lock-only', '--all', '--json'], staged, closure, env)
            audit = evidence / 'audit.json'
            audit_exit = run([npm, 'audit', '--package-lock-only', '--json'], staged, audit, env)
            result = read(audit)
            row = {'project': project, 'manifest': manifest, 'manifest_sha256': digest(directory / manifest),
                   'lockfile': lockfile, 'lock_sha256': digest(directory / lockfile),
                   'closure_exit': closure_exit, 'audit_exit': audit_exit,
                   'vulnerabilities': result['metadata']['vulnerabilities'],
                   'evidence': relative(evidence), 'audit_sha256': digest(audit),
                   'closure_sha256': digest(closure)}
            summary['projects'].append(row)
            save(out / 'summary.json', summary)
            assert closure_exit == audit_exit == row['vulnerabilities']['total'] == 0
        project = relative(HERE)
        staged = stages[project]
        installed['isolated_installed_project'] = str(staged)
        commands = [
            ('executor-source-provenance', [sys.executable, REPO / 'vendor/chopsticks-executor/scripts/verify-artifacts.py']),
            ('crypto-source-provenance', [sys.executable, REPO / 'vendor/polkadot-wasm-crypto/scripts/verify-artifacts.py']),
            ('isolated-install', [npm, 'ci', '--ignore-scripts', '--install-links', '--no-audit']),
            ('native-integration', [node, REPO / 'misc/dependency_security/test_js_native_integration.cjs', staged]),
            ('schema', [node, REPO / 'vendor/chopsticks-core/tests/zod-migration.cjs', staged / 'node_modules/@acala-network/chopsticks-core']),
            ('fetch-blob', [node, REPO / 'vendor/fetch-blob/tests/native-domexception.mjs', staged / 'node_modules/fetch-blob']),
            ('executor', [node, REPO / 'vendor/chopsticks-executor/scripts/smoke.cjs', staged / 'node_modules/@acala-network/chopsticks-executor/dist/cjs/index.js']),
            ('crypto', [node, REPO / 'vendor/polkadot-wasm-crypto/tests/compatibility.cjs', '--candidate', staged / 'node_modules/@polkadot/wasm-crypto', '--security-regressions', 'true']),
        ]
        for name, directory in [('isolated-dependency-validation', staged), ('rehearsal-dependency-validation', HERE)]:
            commands.append((name, [node, '-e', 'const v=require(process.argv[1]); console.log(JSON.stringify(v.validateDependencies(process.argv[2]),null,2));', HERE / 'validate-dependencies.cjs', directory]))
        for name, command in commands:
            log = out / f'{name}.log'
            exit_code = run(command, staged if name == 'isolated-install' else REPO, log, env)
            installed['checks'].append({'name': name, 'command': [str(arg) for arg in command],
                                       'exit': exit_code, 'log': relative(log), 'log_sha256': digest(log),
                                       'stderr': relative(log.with_suffix('.stderr.log')),
                                       'stderr_sha256': digest(log.with_suffix('.stderr.log'))})
            save(out / 'installed-checks.json', installed)
            assert exit_code == 0, name
        assert {row['name'] for row in installed['checks']} == INSTALLED_CHECKS
        summary['status'] = installed['status'] = 'passed'
    except BaseException as error:
        summary.update(status='failed', error=str(error))
        installed.update(status='failed', error=str(error))
        raise
    finally:
        save(out / 'summary.json', summary)
        save(out / 'installed-checks.json', installed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--npm', type=Path, required=True)
    parser.add_argument('--db', type=Path, required=True, help='Already fetched RustSec advisory repository')
    parser.add_argument('--only', choices=('cargo', 'npm', 'all'), default='all')
    args = parser.parse_args()
    if not __debug__:
        raise RuntimeError('Evidence capture requires assertions')
    env = dict(os.environ)
    env['PATH'] = str(args.node.parent) + os.pathsep + env['PATH']
    imported = import_historical()
    if args.only in ('all', 'cargo'):
        audit_cargo(imported, env, args.db)
    if args.only in ('all', 'npm'):
        audit_npm(args.node, args.npm, env)


if __name__ == '__main__':
    main()
