"""Runs only in a fresh network-none preparation smoke container."""
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

value = json.loads(Path('/preparation/recipe.json').read_bytes())
public_manifest = tomllib.loads(Path('/preparation/rust-manifest.toml').read_text())
inventory_raw = Path('/preparation/source-inventory.json').read_bytes()
if hashlib.sha256(inventory_raw).hexdigest() != value['sourceInventorySha256']:
    raise SystemExit('source inventory identity differs')
for name, pin in json.loads(inventory_raw).items():
    data = Path('/source', name).read_bytes()
    if hashlib.sha256(data).hexdigest() != pin['sha256'] or len(data) != pin['bytes']:
        raise SystemExit('source bytes differ: ' + name)
env = {'PATH': '/opt/cargo/bin:/usr/bin:/bin', 'HOME': '/empty-home', 'CARGO_HOME': '/opt/cargo',
    'RUSTUP_HOME': '/opt/rustup', 'RUSTUP_TOOLCHAIN': value['toolchain'], 'CARGO_NET_OFFLINE': 'true',
    'SKIP_WASM_BUILD': '1', 'LANG': 'C', 'LC_ALL': 'C', 'GIT_CONFIG_NOSYSTEM': '1',
    'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0'}
commands = {
    'rustc': ['/opt/cargo/bin/rustc', '+' + value['toolchain'], '-vV'],
    'cargo': ['/opt/cargo/bin/cargo', '+' + value['toolchain'], '-V'],
    'cc': ['/usr/bin/cc', '--version'], 'cxx': ['/usr/bin/c++', '--version'],
    'cmake': ['/usr/bin/cmake', '--version'], 'protoc': ['/usr/bin/protoc', '--version'],
    'openssl': ['/usr/bin/pkg-config', '--modversion', 'openssl'],
    'clang': ['/usr/bin/llvm-config', '--libdir'],
    'metadata': ['/opt/cargo/bin/cargo', '+' + value['toolchain'], 'metadata', '--offline', '--locked',
        '--frozen', '--format-version', '1', '--filter-platform', value['rustTarget'], '--no-default-features'],
}
receipts = {}
for name, command in commands.items():
    result = subprocess.run(command, cwd='/source', env=env, check=True, timeout=86400, capture_output=True)
    if len(result.stdout) + len(result.stderr) > 256 * 1024**2:
        raise SystemExit('smoke output bound')
    receipts[name] = {'command': command, 'exitCode': result.returncode,
        'stdoutSha256': hashlib.sha256(result.stdout).hexdigest(),
        'stderrSha256': hashlib.sha256(result.stderr).hexdigest()}
    if name == 'rustc' and ('host: ' + value['rustTarget'] + '\n').encode() not in result.stdout:
        raise SystemExit('wrong Rust host')
    if name == 'rustc':
        expected_commit = public_manifest['pkg']['rustc'].get('git_commit_hash')
        if expected_commit is not None and ('commit-hash: ' + expected_commit + '\n').encode() not in result.stdout:
            raise SystemExit('installed rustc commit differs from pinned public manifest')
    if name == 'clang' and not any(Path(result.stdout.decode().strip()).glob('libclang.so*')):
        raise SystemExit('libclang is unavailable')
    if name == 'metadata' and not any(row['name'] == 'framenode' and row['version'] == '4.8.13'
                                    for row in json.loads(result.stdout)['packages']):
        raise SystemExit('locked node metadata differs')
print(json.dumps({'schema': 'sora-nexus-linux-builder-offline-smoke-v1', 'toolchain': value['toolchain'],
    'rustTarget': value['rustTarget'], 'checks': receipts, 'dependencyMetadataVerified': True,
    'sourceInventoryVerified': True, 'sourceInventorySha256': value['sourceInventorySha256'],
    'recipeSha256': hashlib.sha256(Path('/preparation/recipe.json').read_bytes()).hexdigest(),
    'installedDebianPackages': Path('/preparation/apt-evidence/installed.tsv').read_text(),
    'aptEvidenceSha256': {path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(Path('/preparation/apt-evidence').iterdir()) if path.is_file()},
    'nativeBuildVerified': False, 'productionReady': False}, sort_keys=True))
