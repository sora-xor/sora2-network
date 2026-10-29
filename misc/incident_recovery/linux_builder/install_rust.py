"""Fixed container-side Rust installation from the resolved public recipe."""
import copy
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib


def standalone_artifacts(value, date):
    """Validate the non-component archive index omitted by Rustup 1.29.2.

    Rustup's Manifest type has no `artifacts` field. This permits only the
    reviewed installer/source indexes; package download metadata stays exact.
    """
    targets = {
        'installer-msi': {'aarch64-pc-windows-gnullvm', 'aarch64-pc-windows-msvc',
            'x86_64-pc-windows-gnu', 'x86_64-pc-windows-gnullvm', 'x86_64-pc-windows-msvc'},
        'installer-pkg': {'aarch64-apple-darwin', 'x86_64-apple-darwin'},
        'source-code': {'*'},
    }
    if not isinstance(date, str) or not re.fullmatch(r'\d{4}-\d{2}-\d{2}', date):
        raise ValueError('standalone artifact date differs')
    if not isinstance(value, dict) or not set(value) <= set(targets):
        raise ValueError('unrecognized standalone artifact category')
    for kind, artifact in value.items():
        if (not isinstance(artifact, dict) or set(artifact) != {'target'}
            or not isinstance(artifact['target'], dict) or not artifact['target']
            or not set(artifact['target']) <= targets[kind]):
            raise ValueError('unrecognized standalone artifact target')
        for target, entries in artifact['target'].items():
            names = ({'rustc-nightly-src.tar.gz', 'rustc-nightly-src.tar.xz'} if kind == 'source-code'
                else {f'rust-nightly-{target}.' + ('msi' if kind == 'installer-msi' else 'pkg')})
            allowed = {f'https://static.rust-lang.org/dist/{date}/{name}' for name in names}
            if not isinstance(entries, list) or not 0 < len(entries) <= len(allowed):
                raise ValueError('standalone artifact list bound')
            seen = set()
            for entry in entries:
                if (not isinstance(entry, dict) or set(entry) != {'url', 'hash-sha256'}
                    or not isinstance(entry['url'], str) or entry['url'] not in allowed or entry['url'] in seen
                    or not isinstance(entry['hash-sha256'], str)
                    or not re.fullmatch(r'[0-9a-f]{64}', entry['hash-sha256'])):
                    raise ValueError('unrecognized standalone artifact entry')
                seen.add(entry['url'])


def verify_manifest(public, installed):
    """Allow only Rustup's known metadata serialization normalization.

    The original public manifest remains pinned. Archive hashes/URLs, versions,
    targets, order, profiles and every other field must still agree exactly.
    """
    expected = copy.deepcopy(public)
    if 'artifacts' in expected and 'artifacts' not in installed:
        standalone_artifacts(expected['artifacts'], expected.get('date'))
        del expected['artifacts']
    for name, package in expected.get('pkg', {}).items():
        observed = installed.get('pkg', {}).get(name, {})
        if 'git_commit_hash' in package and 'git_commit_hash' not in observed:
            del package['git_commit_hash']
        for target, fields in package.get('target', {}).items():
            actual = observed.get('target', {}).get(target, {})
            for key, extension in [('components', False), ('extensions', True)]:
                if key not in fields and actual.get(key) == []:
                    fields[key] = []
                if isinstance(fields.get(key), list) and isinstance(actual.get(key), list):
                    for original, emitted in zip(fields[key], actual[key]):
                        if (isinstance(original, dict) and isinstance(emitted, dict)
                            and 'is_extension' not in original
                            and emitted.get('is_extension') is extension):
                            original['is_extension'] = extension
    if json.dumps(expected, sort_keys=True) != json.dumps(installed, sort_keys=True):
        raise ValueError('installed Rust channel manifest differs from the pinned public manifest')


def main():
    ROOT = Path('/preparation')
    value = json.loads((ROOT / 'recipe.json').read_bytes())
    for name in ('rustup-init', 'rust-manifest.toml'):
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != value['files'][name]['sha256']:
            raise SystemExit('pinned Rust input differs: ' + name)
    environment = {'PATH': '/opt/cargo/bin:/usr/bin:/bin', 'HOME': '/empty-home',
        'CARGO_HOME': '/opt/cargo', 'RUSTUP_HOME': '/opt/rustup', 'LANG': 'C', 'LC_ALL': 'C',
        'RUSTUP_DIST_SERVER': 'https://static.rust-lang.org',
        'RUSTUP_UPDATE_ROOT': 'https://static.rust-lang.org/rustup'}
    Path('/empty-home').mkdir(exist_ok=True)
    (ROOT / 'rustup-init').chmod(0o555)
    subprocess.run([str(ROOT / 'rustup-init'), '-y', '--no-modify-path', '--profile', 'minimal',
        '--default-toolchain', 'none'], env=environment, check=True, timeout=86400)
    if hashlib.sha256(Path('/opt/cargo/bin/rustup').read_bytes()).hexdigest() != value['files']['rustup-init']['sha256']:
        raise SystemExit('installed rustup differs from pinned installer')
    subprocess.run(['/opt/cargo/bin/rustup', 'set', 'auto-self-update', 'disable'],
        env=environment, check=True, timeout=86400)
    subprocess.run(['/opt/cargo/bin/rustup', 'toolchain', 'install', value['toolchain'], '--profile', 'minimal',
        '--component', 'rust-src', '--target', 'wasm32-unknown-unknown', '--target', 'wasm32v1-none'],
        env=environment, check=True, timeout=86400)
    manifest = Path('/opt/rustup/toolchains') / (value['toolchain'] + '-' + value['rustTarget']) / 'lib/rustlib/multirust-channel-manifest.toml'
    verify_manifest(tomllib.loads((ROOT / 'rust-manifest.toml').read_text()),
        tomllib.loads(manifest.read_text()))
    subprocess.run(['/opt/cargo/bin/rustup', 'default', value['toolchain']], env=environment, check=True, timeout=86400)


if __name__ == '__main__':
    main()
