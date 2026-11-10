#!/usr/bin/env python3
"""Independently reconstruct vendored primitives from pinned SDK Git objects."""
import argparse
import copy
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import tomllib

COMMIT = '7a8daa68e18718c18823c1109513fdc1c8aa71fe'
PACKAGES = ('primitives/core', 'primitives/io', 'primitives/runtime', 'primitives/version',
            'primitives/tracing', 'primitives/wasm-interface', 'utils/wasm-builder')
ROOT = Path(__file__).resolve().parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(command, cwd=None):
    return subprocess.run(command, cwd=cwd, check=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE).stdout


def normalize(manifest, workspace):
    """Resolve inheritance independently of the migration's materializer."""
    manifest = copy.deepcopy(manifest)
    for key, value in manifest['package'].items():
        if isinstance(value, dict) and value.get('workspace'):
            manifest['package'][key] = copy.deepcopy(workspace['package'][key])
    if manifest.get('lints', {}).get('workspace'):
        manifest['lints'] = copy.deepcopy(workspace['lints'])
    for parent in [manifest, *manifest.get('target', {}).values()]:
        for section in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            for name, original in parent.get(section, {}).items():
                if not isinstance(original, dict):
                    continue
                value = copy.deepcopy(original)
                if value.pop('workspace', False):
                    inherited = workspace['dependencies'][name]
                    resolved = {'version': inherited} if isinstance(inherited, str) else copy.deepcopy(inherited)
                    features = list(dict.fromkeys(resolved.get('features', []) + value.get('features', [])))
                    resolved.update(value)
                    if 'features' in value:
                        resolved['features'] = features
                    value = resolved
                if 'path' in value:
                    value.pop('path')
                    value.pop('version', None)
                    value.update(git='https://github.com/paritytech/polkadot-sdk.git', tag='polkadot-stable2609')
                parent[section][name] = value
    return manifest


def overrides(manifest, differences):
    manifest = copy.deepcopy(manifest)
    for row in differences:
        parent = manifest
        for key in row['path'][:-1]:
            parent = parent[key]
        key = row['path'][-1]
        if parent.get(key) != row['upstream']:
            raise ValueError('Manifest override does not match upstream at ' + '.'.join(row['path']))
        if row['local'] is None:
            parent.pop(key)
        else:
            parent[key] = copy.deepcopy(row['local'])
    return manifest


def inventory(root):
    # Upstream sp-runtime/docs points into the repository docs tree. The local
    # standalone vendor copy materializes that directory, so inspect both forms.
    paths = [Path(directory) / name for directory, _, names in os.walk(root, followlinks=True)
             for name in names if name.endswith('.rs')]
    return {str(path.relative_to(root)): digest(path.read_bytes()) for path in sorted(paths)}


def compare_sources(expected, actual):
    if expected != actual:
        names = sorted(set(expected) | set(actual))
        changed = [name for name in names if expected.get(name) != actual.get(name)]
        raise ValueError('Unrecorded Rust source drift: ' + ', '.join(changed))


def verify(upstream):
    commit = run(['git', 'rev-parse', COMMIT + '^{commit}'], upstream).decode().strip()
    if commit != COMMIT:
        raise ValueError('Wrong SDK Git commit')
    # Read committed objects, never potentially edited checkout files.
    archive = run(['git', 'archive', COMMIT, 'Cargo.toml', 'docs',
                   *['substrate/' + package for package in PACKAGES]], upstream)
    rows = []
    with tempfile.TemporaryDirectory(prefix='sdk2609-source-verification-') as temporary:
        temporary = Path(temporary)
        with tarfile.open(fileobj=io.BytesIO(archive)) as package:
            package.extractall(temporary, filter='data')
        workspace = tomllib.loads((temporary / 'Cargo.toml').read_text())['workspace']
        for package in PACKAGES:
            original = temporary / 'substrate' / package
            local = ROOT / 'vendor/polkadot-sdk' / package
            patch = local / 'SORA-2609.patch'
            upstream_rust = inventory(original)
            if patch.stat().st_size:
                run(['git', 'apply', '--check', str(patch)], original)
                run(['git', 'apply', str(patch)], original)
            actual = inventory(local)
            compare_sources(inventory(original), actual)
            policy = local / 'SORA-2609-manifest-overrides.json'
            recorded = json.loads(policy.read_text())
            if recorded['upstream_commit'] != COMMIT or recorded['source_path'] != 'substrate/' + package:
                raise ValueError('Incorrect manifest provenance: ' + package)
            expected = normalize(tomllib.loads((original / 'Cargo.toml').read_text()), workspace)
            expected = overrides(expected, recorded['differences'])
            if expected != tomllib.loads((local / 'Cargo.toml').read_text()):
                raise ValueError('Unrecorded materialized manifest drift: ' + package)
            rows.append({'package': package, 'rustFiles': len(actual),
                         'rustSha256': digest(json.dumps(actual, sort_keys=True).encode()),
                         'upstreamRustSha256': digest(json.dumps(upstream_rust, sort_keys=True).encode()),
                         'sourceFiles': actual, 'patchSha256': digest(patch.read_bytes()),
                         'manifestSha256': digest((local / 'Cargo.toml').read_bytes()),
                         'manifestOverridesSha256': digest(policy.read_bytes()),
                         'manifestOverrides': len(recorded['differences']), 'reconstructedExactly': True})
    return {'status': 'passed', 'upstreamCommit': COMMIT, 'upstreamObjectArchiveSha256': digest(archive),
            'packages': rows, 'allRustSourcesAndNormalizedManifestsReconstructed': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path, default=os.environ.get('SDK2609_UPSTREAM', '/tmp/sora2-stable2609-migration/upstream'))
    parser.add_argument('--output', type=Path, default=Path(__file__).with_name('sdk-primitives-provenance.json'))
    args = parser.parse_args()
    report = {'status': 'running', 'startedAt': datetime.now(timezone.utc).isoformat(),
              'scriptSha256': digest(Path(__file__).read_bytes())}
    try:
        report.update(verify(args.upstream))
    except Exception as error:
        report.update(status='failed', error=str(error))
        if isinstance(error, subprocess.CalledProcessError):
            report['stderr'] = error.stderr.decode(errors='replace')
    report['finishedAt'] = datetime.now(timezone.utc).isoformat()
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'status': report['status'], 'packages': len(report.get('packages', [])),
                      'output': str(args.output), 'error': report.get('error')}))
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
