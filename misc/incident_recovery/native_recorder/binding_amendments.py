"""Derive reviewed recorder build metadata from current source/lock inputs.

The binding describes a native source preparation. It does not attest a compiler
run or native execution. The caller must install the returned host lock and embed
this binding before compiling; changing an already-built sidecar is insufficient.
"""
import hashlib
import json
from pathlib import Path


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def reviewed_manifest(assets):
    path = assets / 'UPSTREAM.json'
    raw = path.read_bytes()
    manifest = json.loads(raw)
    require(manifest.get('schema') == 'sora-nexus-production-native-reviewed-assets-v1'
            and isinstance(manifest.get('files'), dict), 'reviewed native asset manifest required')
    files = manifest['files']
    found = set()
    for item in assets.rglob('*'):
        require(not item.is_symlink(), 'reviewed native asset alias')
        if item.is_file():
            found.add(item.relative_to(assets).as_posix())
    require(found == set(files) | {'UPSTREAM.json'}, 'unbound reviewed native asset member')
    for name, row in files.items():
        relative = Path(name)
        require(name and '\\' not in name and not relative.is_absolute()
                and all(part not in ('.', '..') for part in relative.parts), 'unsafe reviewed native asset path')
        item = assets / relative
        require(item.resolve(strict=True) == item and sha(item.read_bytes()) == row['sha256'],
                'reviewed native asset digest differs: ' + name)
    return sha(raw)


def derive(binding, report, source, assets, feature_profile, check_lock):
    """Use the actual Wasm feature profile and a verified additive host lock."""
    version = binding.get('specVersion')
    require(version in (134, 136, 137)
            and binding.get('transactionVersion') == (131 if version == 134 else 132)
            and report.get('specVersion') == version
            and report.get('transactionVersion') == binding['transactionVersion'],
            'unsupported recorder amendment version')
    manifest_sha = reviewed_manifest(assets)
    original = (source / 'Cargo.lock').read_bytes()
    require(original == (assets / 'repaired/lock/original.lock').read_bytes(),
            'unsupported original runtime lock')
    host_lock = (assets / 'repaired/lock/amended.lock').read_bytes()
    amendments = {
        'runtimeFeatureProfile': feature_profile(report, source),
        'hostLockAmendment': check_lock(original, host_lock),
    }
    key = 'sdkCompilationRepair' if version == 134 else 'jsonConstructionRepair'
    amendments[key] = {
        'schema': 'sora-nexus-promoted-reviewed-native-' + ('sdk' if version == 134 else 'json') + '-repair-v1',
        'assetManifestSha256': manifest_sha,
        'runtimeBusinessSourcesChanged': False,
    }
    if version != 134:
        amendments['templateCarriesReviewedSdkAndTestPathRepairs'] = True
    result = dict(binding)
    for key, value in amendments.items():
        require(key not in result or result[key] == value, 'recorder amendment already bound to different evidence: ' + key)
        result[key] = value
    return result, host_lock
