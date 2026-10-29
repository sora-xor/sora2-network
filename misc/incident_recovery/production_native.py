#!/usr/bin/env python3
"""Prepare/build production-bound native parity recorders; never authorize release.

Use actual production Wasm exports and their retained compiler receipts. Commands
are fixed here, sources are copied, and one persistent Cargo target belongs to
each source/engine lane. No network, signing, service or deployment operation.
"""
from __future__ import annotations
import argparse
from types import SimpleNamespace
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
from uuid import uuid4

sys.dont_write_bytecode = True
import production_build as build
import incoming_replay_release as incoming
import node_identity

ROOT = build.ROOT
HERE = Path(__file__).resolve().parent
ASSETS = HERE / 'native_recorder'
PACKAGE = 'sora-nexus-native-operation-recorder'
PACKAGE_PATH = 'utils/native-operation-recorder'
PREPARATION = 'sora-nexus-production-native-preparation-v1'
MAX_EXECUTABLE = 512 * 1024**2
COMMAND_TIMEOUT = 24 * 3600
require = build.require
read = build.read
parse = build.parse
digest = build.digest
write = build.write


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def reviewed_assets():
    manifest = parse(read(ASSETS / 'UPSTREAM.json'))
    require(manifest['schema'] == 'sora-nexus-production-native-reviewed-assets-v1', 'reviewed native assets schema')
    expected = manifest['files']
    found = {p.relative_to(ASSETS).as_posix() for p in ASSETS.rglob('*') if p.is_file()}
    require(found == set(expected) | {'UPSTREAM.json'}, 'unbound native asset file')
    pins = {str(ASSETS / 'UPSTREAM.json'): digest(ASSETS / 'UPSTREAM.json')}
    for name, row in expected.items():
        path = build.release.local_file(ASSETS, name)
        read(path, row['sha256'])
        pins[str(path)] = digest(path)
    lock = load_module('production_native_lock', ASSETS / 'repaired/lock/verify_lock.py')
    # The exact reviewed repaired validator imports this sibling lock module.
    existing = sys.modules.get('verify_lock')
    require(existing is None or Path(existing.__file__) == Path(lock.__file__), 'ambiguous native lock validator')
    sys.modules['verify_lock'] = lock
    original = load_module('production_native_original_validator', ASSETS / 'original/prepare.py')
    repaired = load_module('production_native_repaired_validator', ASSETS / 'repaired/prepare.py')
    pins.update(build.helper_pins())
    for path in (Path(__file__).resolve(), Path(incoming.__file__).resolve(), Path(node_identity.__file__).resolve()):
        pins[str(path)] = digest(path)
    return pins, lock, original, repaired


def nonfixture_policy(raw):
    identity = build.release.policy_identity(raw)
    require(identity['authorizationDomain']['chainSpecId'] == 'sora-nexus-recovery-28191817'
        and identity['authorizationDomain']['releaseNonce'] != '0x' + '46' * 32, 'public fixture or foreign recovery domain rejected')
    controls = incoming.controls(raw)
    reader = build.scale.Reader(raw)
    reader.take(68)
    for _ in range(reader.compact()):
        reader.take(32)
        require(reader.take(32).hex() not in build.PUBLIC_SIGNING_KEYS, 'known public replacement identity')
    council = [reader.take(32).hex() for _ in range(reader.compact())]
    require(not build.PUBLIC_SIGNING_KEYS.intersection(council), "known public recovered council identity")
    return identity, controls


def validate_process(report, receipt_path, source):
    """Join retained actual process fields/logs, never an unlinked success flag."""
    command = [str(ROOT / 'scripts/with_llvm_env.sh'), 'cargo', 'check', '--offline', '--locked',
               '-p', 'framenode-runtime', '--lib', '--features', 'build-wasm-binary']
    require(report.get('command') == command, 'production Wasm command differs')
    directory = receipt_path.parent / 'cargo'
    process = parse(read(directory / 'execution.json', report['processReceipt']))
    require(process.get('schema') == 'sora-nexus-production-build-process-v1'
            and process.get('command') == command and process.get('cwd') == str(source)
            and type(process.get('pid')) is int and process['pid'] > 0
            and type(process.get('exitCode')) is int and process['exitCode'] == 0
            and process.get('stopReason') is None and process.get('guardError') is None
            and process.get('cleanupErrors') == [] and process.get('productionReady') is False,
            'successful retained production Wasm process required')
    launch = parse(read(directory / 'launch.json'))
    started = parse(read(directory / 'started.json'))
    for key in ('schema', 'command', 'cwd', 'startedAt', 'productionReady'):
        require(launch.get(key) == started.get(key) == process.get(key), 'Wasm launch/process join differs')
    require(started.get('pid') == process['pid'], 'Wasm process PID differs')
    require(set(process.get('outputs', {})) == {'stdout.log', 'stderr.log'}, 'actual Wasm logs required')
    paths = [directory / name for name in ('execution.json', 'launch.json', 'started.json')]
    for name, expected in process['outputs'].items():
        read(directory / name, expected, build.MAX_LOG)
        paths.append(directory / name)
    return {str(path): digest(path) for path in paths}


def validate_inputs(args):
    pins, lock, original_validator, repaired_validator = reviewed_assets()
    source = args.source
    require(source.is_absolute() and source.resolve(strict=True) == source, 'canonical actual Wasm source required')
    report = parse(read(args.wasm_build, args.wasm_build_sha256))
    require(report.get('schema') == 'sora-nexus-production-recovery-wasm-build-v1'
            and report.get('fixtureOnly') is False and report.get('productionReady') is False
            and report.get('status') == 'passed' and type(report.get('exitCode')) is int
            and report['exitCode'] == 0 and report.get('source') == str(source), 'nonfixture successful Wasm build required')
    version = report.get('specVersion')
    require(version in (134, 136, 137) and report.get('transactionVersion') == (131 if version == 134 else 132),
            'native engine must be original134 or repaired136/137')
    provenance = parse(read(source / 'RECOVERY-PROVENANCE.json', report['sourceProvenance']))
    require(provenance.get('fixtureOnly') is False and provenance.get('releaseReady') is False
            and not (source / 'NEVER-DEPLOY.txt').exists()
            and not (source / 'FIXTURE-PROVENANCE.json').exists(), 'nonfixture source provenance required')
    require(read(args.wasm_build.parent / 'source-provenance.json', report['sourceProvenance'])
            == read(source / 'RECOVERY-PROVENANCE.json'), 'adjacent/source provenance differs')
    pins.update(validate_process(report, args.wasm_build, source))
    policy = read(args.policy, args.policy_sha256, 2 * 1024**2)
    original = read(args.original_policy, args.original_policy_sha256, 2 * 1024**2)
    _, controls = nonfixture_policy(policy)
    _, original_controls = nonfixture_policy(original)
    require(controls == original_controls, 'active policy changes original checkpoint/mapping/council')
    if version == 134:
        require(policy == original, 'bootstrap policy differs from original')
        inventory, binding = original_validator.validate(source, args.wasm_build, args.wasm_build_sha256,
            args.wasm, args.wasm_sha256, args.policy, args.policy_sha256)
        variant = 'original'
    else:
        inventory, binding = repaired_validator.validate(source, args.wasm_build, args.wasm_build_sha256,
            args.wasm, args.wasm_sha256, args.policy, args.policy_sha256,
            args.original_policy, args.original_policy_sha256)
        variant = 'repaired'
    require(binding['fixtureOnly'] is False, 'fixture engine cannot enter production preparation')
    require(build.fingerprint(source) == inventory, 'actual exported source file set differs')
    build.validate_source_provenance(source, provenance, inventory)
    runtime = build.release.wasm_identity(read(args.wasm, args.wasm_sha256, 50 * 1024**2))
    require((runtime['specVersion'], runtime['transactionVersion']) == (version, report['transactionVersion']),
            'actual Wasm version differs')
    generated = read(source / 'runtime/src/incident_recovery/generated.rs').decode()
    operators = repaired_validator.generated_bytes(generated, 'OPERATORS_SCALE')
    require(not build.PUBLIC_SIGNING_KEYS.intersection(build.release.operator_identity(operators)['fresh']),
            'known public GRANDPA identity')
    require(all(build.pin_bytes(operators)[key] == provenance['bindingEvidence']['operators'][key]
                for key in ('sha256', 'blake2_256')), 'compiled operator binding differs')
    profile = repaired_validator.feature_profile(report, source)
    host_lock = read(ASSETS / 'repaired/lock/amended.lock')
    lock_report = lock.check(read(source / 'Cargo.lock'), host_lock)
    binding['runtimeFeatureProfile'] = profile
    binding['hostLockAmendment'] = lock_report
    if version == 134:
        binding['sdkCompilationRepair'] = {'schema': 'sora-nexus-promoted-reviewed-native-sdk-repair-v1',
            'assetManifestSha256': digest(ASSETS / 'UPSTREAM.json')['sha256'], 'runtimeBusinessSourcesChanged': False}
    else:
        binding['jsonConstructionRepair'] = {'schema': 'sora-nexus-promoted-reviewed-native-json-repair-v1',
            'assetManifestSha256': digest(ASSETS / 'UPSTREAM.json')['sha256'], 'runtimeBusinessSourcesChanged': False}
    for path in (build.build_helpers.ENVIRONMENT_HELPER, ROOT / 'scripts/with_llvm_env.sh',
                 args.wasm_build, args.wasm, args.policy, args.original_policy,
                 args.wasm_build.parent / 'source-files.json', args.wasm_build.parent / 'source-provenance.json',
                 source / 'RECOVERY-PROVENANCE.json', source / 'PRODUCTION-LOCK-AMENDMENT.json'):
        pins[str(path)] = digest(path)
    for name, expected in inventory.items():
        path = build.release.local_file(source, name)
        require(digest(path) == expected, 'actual source pin differs')
        pins[str(path)] = expected
    return inventory, binding, variant, host_lock, pins


def source_inventory(source):
    result = {}
    for directory, children, files in os.walk(source, followlinks=False):
        require(all(not (Path(directory) / name).is_symlink() for name in children), 'native source directory alias')
        children[:] = [name for name in children if name != 'target']
        for name in files:
            if Path(directory) == source and name in ('RECORDER-SOURCE-FILES.json', '.production-operation.lock'):
                continue
            path = Path(directory) / name
            result[path.relative_to(source).as_posix()] = digest(path)
    require(0 < len(result) <= 30000, 'native source inventory bound')
    return dict(sorted(result.items()))


def business_projection(source, original, inventory, variant, host_lock):
    wrapper = read(ASSETS / variant / 'template/runtime-native-wrapper.rs')
    for name, expected in inventory.items():
        before = read(build.release.local_file(original, name), expected)
        after = read(build.release.local_file(source, name))
        if name == 'runtime/src/lib.rs':
            require(after == before + b'\n' + wrapper, 'native wrapper/business source differs')
        elif name == 'Cargo.toml':
            marker = b'[workspace]\nmembers = ['
            require(before.count(marker) == 1 and after == before.replace(marker, marker + b'\n    "utils/native-operation-recorder",', 1),
                    'workspace membership differs')
        elif name == 'Cargo.lock':
            require(after == host_lock, 'reviewed native host lock differs')
        else:
            require(after == before, 'original business source changed: ' + name)
    return {'originalFiles': len(inventory), 'changedOriginalFiles': ['Cargo.toml', 'Cargo.lock', 'runtime/src/lib.rs'],
            'runtimeBusinessSourcesIdentical': True, 'reviewedStdWrapperOnly': True, 'productionReady': False}


def bound_overlay(binding, inventory, variant):
    binding = dict(binding)
    templates = {path.relative_to(ASSETS / variant).as_posix(): digest(path)['sha256']
                 for path in sorted((ASSETS / variant / 'template').rglob('*')) if path.is_file()}
    binding.update(recorderTemplate=templates, preparerSha256=digest(Path(__file__).resolve())['sha256'],
        originalLockSha256=inventory['Cargo.lock']['sha256'], nativeCompilerProvenanceVerified=False,
        nativeExecutionVerified=False, productionReady=False)
    encoded = json.dumps(binding, sort_keys=True, separators=(',', ':'), allow_nan=False)
    require('"####' not in encoded, 'native Rust literal collision')
    def array(value): return '[' + ','.join(str(byte) for byte in bytes.fromhex(value)) + ']'
    generated = f'pub const ENGINE_SHA256: &str = "{binding["engineSha256"]}";\n'
    generated += f'pub const ENGINE_BLAKE2: [u8;32] = {array(binding["engineBlake2"])};\n'
    generated += f'pub const POLICY_COMMITMENT: [u8;32] = {array(binding["policyCommitment"])};\n'
    if variant == 'repaired':
        generated += f'pub const ORIGINAL_POLICY_COMMITMENT: [u8;32] = {array(binding["originalPolicyCommitment"])};\n'
        generated += f'pub const SPEC_VERSION: u32 = {binding["specVersion"]};\n'
        generated += f'pub const TRANSACTION_VERSION: u32 = {binding["transactionVersion"]};\n'
    generated += 'pub const FIXTURE_ONLY: bool = false;\n'
    generated += f'pub const BINDING_JSON: &str = r####"{encoded}"####;\n'
    return binding, generated.encode()


def validate_overlay(source, binding, generated, variant):
    require(read(source / PACKAGE_PATH / 'src/binding.rs') == generated, 'compiled recorder binding differs')
    package_files = {path.relative_to(source / PACKAGE_PATH).as_posix()
                     for path in (source / PACKAGE_PATH).rglob('*') if path.is_file()}
    expected = {'src/binding.rs'}
    for path in sorted((ASSETS / variant / 'template/recorder').rglob('*')):
        if path.is_file():
            name = path.relative_to(ASSETS / variant / 'template/recorder').as_posix()
            expected.add(name)
            require(read(source / PACKAGE_PATH / name) == read(path), 'reviewed recorder source differs: ' + name)
    require(package_files == expected, 'unbound recorder package file')
    require(parse(read(source / 'RECORDER-ENGINE-BINDING.json')) == binding, 'recorder binding document differs')


def separate(path, roots, message):
    require(all(path != root and path not in root.parents and root not in path.parents for root in roots), message)


def prepare(args):
    root_inventory = build.verify_inventory(args.root_inventory, args.root_inventory_sha256, ROOT)
    inventory, binding, variant, host_lock, pins = validate_inputs(args)
    pins[str(args.root_inventory)] = digest(args.root_inventory)
    def guard():
        require(build.source_inventory(ROOT) == root_inventory, 'root source drift')
        build.guard_files(pins)
        require(build.fingerprint(args.source) == inventory, 'export source set drift')
    guard()
    destination = args.destination
    require(destination.is_absolute() and not destination.exists()
            and destination.parent.resolve(strict=True) == destination.parent, 'new canonical native source lane required')
    separate(destination, (args.source, ROOT), 'native source lane overlaps original/root source')
    destination.mkdir()
    for name, expected in inventory.items():
        source = build.release.local_file(args.source, name)
        target = destination / name; target.parent.mkdir(parents=True, exist_ok=True)
        with source.open('rb') as src, target.open('xb') as dst: shutil.copyfileobj(src, dst)
        target.chmod(stat.S_IMODE(source.stat().st_mode) | stat.S_IWUSR)
        require(digest(target) == expected, 'native source copy differs')
    for name in ('source-files.json', 'source-provenance.json', 'build.json'):
        write(destination / ('RECORDER-ORIGINAL-' + name), read(args.wasm_build if name == 'build.json' else args.wasm_build.parent / name))
    package = destination / PACKAGE_PATH
    require(not package.exists(), 'native recorder package collision')
    for path in sorted((ASSETS / variant / 'template').rglob('*')):
        if path.is_file():
            relative = path.relative_to(ASSETS / variant).as_posix()
            if relative.startswith('template/recorder/'):
                target = package / relative.removeprefix('template/recorder/')
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(read(path))
    lib = destination / 'runtime/src/lib.rs'
    require(b'native_recovery_recorder_' not in read(lib), 'native wrapper already present')
    with lib.open('ab') as stream: stream.write(b'\n' + read(ASSETS / variant / 'template/runtime-native-wrapper.rs'))
    workspace = destination / 'Cargo.toml'; text = workspace.read_text(); marker = '[workspace]\nmembers = ['
    require(text.count(marker) == 1, 'unexpected workspace declaration')
    workspace.write_text(text.replace(marker, marker + '\n    "' + PACKAGE_PATH + '",', 1))
    (destination / 'RECORDER-ORIGINAL-Cargo.lock').write_bytes(read(args.source / 'Cargo.lock'))
    (destination / 'Cargo.lock').write_bytes(host_lock)
    binding, generated = bound_overlay(binding, inventory, variant)
    (package / 'src/binding.rs').write_bytes(generated)
    write(destination / 'RECORDER-ENGINE-BINDING.json', binding)
    projection = business_projection(destination, args.source, inventory, variant, host_lock)
    config = {'schema': PREPARATION, 'source': str(destination), 'originalSource': str(args.source),
        'originalSourceInventory': inventory, 'variant': variant, 'rootSourceInventory': root_inventory,
        'rootInventory': {'path': str(args.root_inventory), **digest(args.root_inventory)},
        'admission': {key: str(getattr(args, key)) if isinstance(getattr(args, key), Path) else getattr(args, key)
            for key in ('source', 'wasm_build', 'wasm_build_sha256', 'wasm', 'wasm_sha256', 'policy', 'policy_sha256',
                        'original_policy', 'original_policy_sha256')},
        'inputPins': pins, 'binding': binding, 'businessPreservation': projection,
        'target': str(destination / 'target'), 'fixtureOnly': False, 'productionReady': False}
    write(destination / 'RECORDER-PREPARATION.json', config)
    (destination / 'target').mkdir()
    native_inventory = source_inventory(destination)
    write(destination / 'RECORDER-SOURCE-FILES.json', native_inventory)
    guard()
    require(source_inventory(destination) == native_inventory, 'prepared native source changed')
    return {'source': str(destination), 'configuration': digest(destination / 'RECORDER-PREPARATION.json'),
        'sourceInventory': digest(destination / 'RECORDER-SOURCE-FILES.json'), 'binding': digest(destination / 'RECORDER-ENGINE-BINDING.json'),
        'nativeBuildExecuted': False, 'productionReady': False}


def commands():
    prefix = [str(ROOT / 'scripts/with_llvm_env.sh'), 'cargo']
    package = ['--offline', '--locked', '-p', PACKAGE]
    return [('metadata', prefix + ['metadata', '--offline', '--locked', '--format-version', '1']),
            ('tests', prefix + ['test'] + package + ['--bin', PACKAGE]),
            ('incoming-audit-tests', prefix + ['test', '--offline', '--locked', '-p', 'framenode-runtime', '--lib', 'incident_recovery_incoming::audit']),
            ('incoming-balance-audit-tests', prefix + ['test', '--offline', '--locked', '-p', 'framenode-runtime', '--lib', 'incident_recovery_incoming::balance_audit']),
            ('failed-incoming-tests', prefix + ['test', '--offline', '--locked', '-p', 'framenode-runtime', '--lib', 'incident_recovery::tests::committed_failed_incoming']),
            ('clippy', prefix + ['clippy'] + package + ['--all-targets', '--no-deps', '--', '-D', 'warnings']),
            ('build', prefix + ['build'] + package + ['--message-format', 'json-render-diagnostics'])]


def executable_from_cargo(stdout, source, binding):
    rows = []; finished = []
    for line in stdout.decode().splitlines():
        try: row = parse(line.encode())
        except (ValueError, json.JSONDecodeError): continue
        if row.get('reason') == 'build-finished': finished.append(row.get('success'))
        if row.get('reason') == 'compiler-artifact' and row.get('executable'):
            require(row.get('target', {}).get('name') == PACKAGE and row['target'].get('kind') == ['bin']
                    and row.get('profile', {}).get('test') is False, 'unexpected native Cargo executable')
            path = Path(row['executable'])
            require(path == source / 'target/debug' / PACKAGE, 'native executable outside owned warm target')
            raw = read(path, maximum=MAX_EXECUTABLE); node_identity.native_format(path)
            marker = json.dumps(binding, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
            require(marker in raw, 'native executable lacks exact embedded engine binding')
            rows.append((path, digest(path)))
    require(finished == [True] and len(rows) == 1, 'exactly one successful native Cargo build required')
    return rows[0]


def build_engine_wasm(args):
    """Build an already exported repair engine; never compose/bootstrap a release."""
    require(args.spec_version in (136, 137), 'only forward-repair136/137 use this build entry point')
    root_inventory = build.verify_inventory(args.root_inventory, args.root_inventory_sha256, ROOT)
    pins, _, _, validator = reviewed_assets()
    source = args.source
    require(source.is_absolute() and source.resolve(strict=True) == source, 'canonical exported repair source required')
    separate(source, (ROOT,), 'repair export overlaps root source')
    provenance_path = source / 'RECOVERY-PROVENANCE.json'
    provenance = parse(read(provenance_path))
    require(provenance.get('fixtureOnly') is False and provenance.get('releaseReady') is False
            and provenance.get('bound') is True and provenance.get('specVersion') == args.spec_version
            and provenance.get('transactionVersion') == 132, 'nonfixture bound repair source required')
    build.no_fixture(provenance)
    require(not any((source / name).exists() for name in ('NEVER-DEPLOY.txt', 'FIXTURE-PROVENANCE.json')),
            'public fixture repair source rejected')
    active = read(args.policy, args.policy_sha256, 2 * 1024**2)
    original = read(args.original_policy, args.original_policy_sha256, 2 * 1024**2)
    require(nonfixture_policy(active)[1] == nonfixture_policy(original)[1], 'repair changes original controls')
    generated = read(source / 'runtime/src/incident_recovery/generated.rs').decode()
    for prefix, raw in (('', active), ('ORIGINAL_', original)):
        require(validator.generated_bytes(generated, prefix + 'POLICY_SCALE') == raw
                and validator.generated_bytes(generated, prefix + 'POLICY_COMMITMENT').hex() == build.pin_bytes(raw)['blake2_256'],
                'compiled original/active repair policy differs')
    operators = validator.generated_bytes(generated, 'OPERATORS_SCALE')
    require(not build.PUBLIC_SIGNING_KEYS.intersection(build.release.operator_identity(operators)['fresh']),
            'public repair GRANDPA identity')
    final = read(args.final_wasm, args.final_wasm_sha256, 32 * 1024**2)
    final_identity = build.release.wasm_identity(final)
    require((final_identity['specVersion'], final_identity['transactionVersion']) == (138, 134)
            and validator.generated_bytes(generated, 'FINAL_CODE_HASH').hex() == build.pin_bytes(final)['blake2_256'],
            'repair must preserve pinned final138')
    for name, raw in (('policy', active), ('operators', operators), ('finalWasm', final)):
        require(all(provenance['bindingEvidence'][name][key] == build.pin_bytes(raw)[key]
                    for key in ('sha256', 'blake2_256')), 'repair provenance binding differs')
    before = build.fingerprint(source)
    expected = provenance.get('sourceFiles')
    require(isinstance(expected, dict) and set(before) == set(expected), 'complete repair export inventory required')
    amendment_path = source / 'PRODUCTION-LOCK-AMENDMENT.json'
    if amendment_path.exists():
        build.validate_source_provenance(source, provenance, before)
    else:
        require(all(all(before[name][key] == expected[name][key] for key in ('sha256', 'blake2_256')) for name in before),
                'repair export changed before lock normalization')
    read(args.legacy_lock, args.legacy_lock_sha256)
    require(read(args.legacy_lock) == read(ASSETS / 'repaired/lock/original.lock'), 'unsupported original runtime lock')
    for path in (args.root_inventory, args.policy, args.original_policy, args.final_wasm, args.legacy_lock,
                 provenance_path, build.build_helpers.ENVIRONMENT_HELPER, ROOT / 'scripts/with_llvm_env.sh'):
        pins[str(path)] = digest(path)
    source_sets = {source: before}
    def retain(path, expected):
        require(str(path) not in pins or pins[str(path)] == expected, 'retained build input changed')
        pins[str(path)] = expected
    def retain_source(path, inventory):
        require(path == source and inventory == source_sets[source], 'actual repair build source differs')
    def guard():
        require(build.source_inventory(ROOT) == root_inventory, 'root source drift')
        build.guard_files(pins)
        require(build.fingerprint(source) == source_sets[source], 'repair source drift')
    guard()
    require(args.output.is_absolute() and not args.output.exists()
            and args.output.parent.resolve(strict=True) == args.output.parent, 'fresh repair build output required')
    separate(args.output, (source, ASSETS, args.target), 'repair build output overlaps source/target')
    require(args.target.is_absolute() and args.target.is_dir() and args.target.resolve(strict=True) == args.target,
            'existing canonical persistent repair target required')
    separate(args.target, (source, ROOT), 'repair target overlaps source')
    args.output.mkdir()
    copies = {}
    try:
        copied_inputs = set(build.helper_pins()) | {str(path) for path in (Path(__file__).resolve(), args.root_inventory,
            args.policy, args.original_policy, args.final_wasm, args.legacy_lock, provenance_path)}
        for index, name in enumerate(sorted(copied_inputs)):
            destination = args.output / 'inputs' / f'{index:02d}-{Path(name).name}'
            expected = write(destination, read(Path(name), pins[name]))
            retain(destination, expected)
            copies[name] = {'path': str(destination), **expected}
        guard()
        if not amendment_path.exists():
            with build.build_helpers.operation_lock(source / '.production-operation.lock', 'repair source already active'):
                amendment = build.normalize_lock(source, args.legacy_lock, True)
                write(amendment_path, amendment)
            source_sets[source] = build.fingerprint(source)
        else:
            require(read(source / 'Cargo.lock') == read(args.legacy_lock), 'previous repair lock differs')
        build.validate_source_provenance(source, provenance, source_sets[source])
        retain(amendment_path, digest(amendment_path))
        guard()
        wasm = build.build_wasm(source, args.output / 'wasm', args.target, args.spec_version, guard,
                               retain=retain, retain_source=retain_source)
        guard()
        result = {'schema': 'sora-nexus-production-repair-engine-build-v1', 'specVersion': args.spec_version,
            'transactionVersion': 132, 'status': 'passed', 'wasm': {'path': str(wasm), **digest(wasm)},
            'buildReceipt': {'path': str(args.output / 'wasm/build.json'), **digest(args.output / 'wasm/build.json')},
            'rootInventory': digest(args.root_inventory), 'inputPins': pins, 'inputCopies': copies, 'fixtureOnly': False,
            'approvalVerified': False, 'operatorIdentityVerified': False, 'productionReady': False}
        guard()
        write(args.output / 'completion.json', result)
        return result
    except BaseException as error:
        write(args.output / 'failure.json', {'error': str(error), 'inputPins': pins, 'productionReady': False})
        raise


def run(args):
    source = args.source
    require(source.is_absolute() and source.resolve(strict=True) == source, 'canonical native source lane required')
    config_path = source / 'RECORDER-PREPARATION.json'
    config = parse(read(config_path, args.configuration_sha256))
    require(config.get('schema') == PREPARATION and config.get('source') == str(source)
            and config.get('target') == str(source / 'target') and config.get('fixtureOnly') is False
            and config.get('productionReady') is False, 'native build configuration differs')
    inventory_path = source / 'RECORDER-SOURCE-FILES.json'
    inventory = parse(read(inventory_path, args.source_inventory_sha256))
    binding = parse(read(source / 'RECORDER-ENGINE-BINDING.json'))
    require(binding == config['binding'] and binding.get('fixtureOnly') is False, 'native binding differs')
    require(config['variant'] == ('original' if binding['specVersion'] == 134 else 'repaired')
            and binding['specVersion'] in (134, 136, 137), 'native engine variant differs')
    admission = SimpleNamespace(**{key: Path(value) if key in ('source', 'wasm_build', 'wasm', 'policy', 'original_policy') else value
                                   for key, value in config['admission'].items()})
    original_inventory, base_binding, variant, host_lock, expected_pins = validate_inputs(admission)
    require(original_inventory == config['originalSourceInventory'] and str(admission.source) == config['originalSource']
            and variant == config['variant'], 'prepared original admission differs')
    expected_binding, generated = bound_overlay(base_binding, original_inventory, variant)
    require(binding == expected_binding, 'prepared binding does not match original admission')
    validate_overlay(source, binding, generated, variant)
    for name in ('source-files.json', 'source-provenance.json', 'build.json'):
        require(read(source / ('RECORDER-ORIGINAL-' + name)) == read(admission.wasm_build if name == 'build.json' else admission.wasm_build.parent / name),
                'retained original build evidence differs')
    root_inventory_path = Path(config['rootInventory']['path'])
    root_inventory = build.verify_inventory(root_inventory_path, config['rootInventory']['sha256'], ROOT)
    require(root_inventory == config['rootSourceInventory'], 'prepared root inventory differs')
    expected_pins[str(root_inventory_path)] = digest(root_inventory_path)
    require(config['inputPins'] == expected_pins, 'prepared input pins differ')
    _, lock, _, _ = reviewed_assets()
    pins = dict(expected_pins)
    pins[str(config_path)] = digest(config_path); pins[str(inventory_path)] = digest(inventory_path)
    pins[str(ROOT / 'scripts/with_llvm_env.sh')] = digest(ROOT / 'scripts/with_llvm_env.sh')
    def guard():
        require(build.source_inventory(ROOT) == config['rootSourceInventory'], 'root source changed')
        build.guard_files(pins)
        require(source_inventory(source) == inventory, 'native source changed')
        business_projection(source, Path(config['originalSource']), config['originalSourceInventory'], config['variant'],
                            read(ASSETS / 'repaired/lock/amended.lock'))
        lock.check(read(source / 'RECORDER-ORIGINAL-Cargo.lock'), read(source / 'Cargo.lock'))
        validate_overlay(source, binding, generated, variant)
    guard()
    require(args.output.is_absolute() and not args.output.exists()
            and args.output.parent.resolve(strict=True) == args.output.parent, 'fresh native build attempt required')
    separate(args.output, (source, Path(config['originalSource']), ASSETS), 'build output overlaps source')
    args.output.mkdir()
    def retain(path, expected):
        require(str(path) not in pins or pins[str(path)] == expected, 'retained native artifact changed')
        pins[str(path)] = expected
    helper_copies = {}
    for index, (name, expected) in enumerate(sorted(build.helper_pins().items())):
        copy_path = args.output / 'helpers' / f'{index:02d}-{Path(name).name}'
        pin = write(copy_path, read(Path(name), expected))
        retain(copy_path, pin)
        helper_copies[name] = {'path': str(copy_path), **pin}
    guard()
    with build.build_helpers.operation_lock(source / '.production-operation.lock', 'native source build already active'):
        with build.build_helpers.target_lock(source / 'target'):
            inherited = {key: os.environ[key] for key in ('HOME', 'PATH', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME', 'USER', 'LOGNAME') if key in os.environ}
            module = load_module('production_native_environment', build.build_helpers.ENVIRONMENT_HELPER)
            guard()
            env, environment = module.prepare_build_environment(inherited, source, run_token=uuid4().hex)
            controlled = {'SKIP_WASM_BUILD': '1', 'CARGO_NET_OFFLINE': 'true', 'CARGO_TARGET_DIR': str(source / 'target')}
            env.update(controlled); environment['controlled'].update(controlled)
            started_pin = write(args.output / 'started.json', {'schema': 'sora-nexus-production-native-build-v1',
                'configuration': digest(config_path), 'sourceInventory': digest(inventory_path),
                'environment': environment, 'fixtureOnly': False, 'productionReady': False})
            retain(args.output / 'started.json', started_pin)
            receipts = {}
            try:
                for name, command in commands():
                    stdout, receipt = build.process(command, source, args.output / name, guard, env, timeout=COMMAND_TIMEOUT, retain=retain)
                    receipts[name] = digest(args.output / name / 'execution.json')
                    if name.endswith('tests') or name == 'tests':
                        counts = re.findall(rb'test result: ok\. ([0-9]+) passed; ([0-9]+) failed;', stdout)
                        require(len(counts) == 1 and int(counts[0][0]) > 0 and int(counts[0][1]) == 0,
                                'native test filter ran no successful tests')
                    if name == 'build':
                        executable, expected = executable_from_cargo(stdout, source, binding)
                        destination = args.output / 'pins'; destination.mkdir()
                        for original, filename, mode in [(executable, PACKAGE, 0o555),
                                (source / 'RECORDER-ENGINE-BINDING.json', 'engine-binding.json', 0o444)]:
                            with original.open('rb') as src, (destination / filename).open('xb') as dst: shutil.copyfileobj(src, dst)
                            (destination / filename).chmod(mode)
                            require(digest(destination / filename) == digest(original), 'independent native artifact copy differs')
                            retain(destination / filename, digest(destination / filename))
                        require(digest(destination / PACKAGE) == expected, 'native executable changed after Cargo observation')
                guard()
                result = {'schema': 'sora-nexus-production-native-build-v1', 'status': 'passed', 'exitCode': 0,
                    'source': str(source), 'sourceInventory': digest(inventory_path), 'configuration': digest(config_path),
                    'binding': binding, 'processes': receipts, 'helperCopies': helper_copies,
                    'retainedArtifacts': pins, 'businessPreservation': config['businessPreservation'],
                    'executable': {'file': 'pins/' + PACKAGE, **digest(args.output / 'pins' / PACKAGE)},
                    'engineBinding': {'file': 'pins/engine-binding.json', **digest(args.output / 'pins/engine-binding.json')},
                    'nativeUnitTestsExecuted': True, 'productionNativeReplayExecuted': False,
                    'nativeCompilerAuthenticityVerified': False, 'fixtureOnly': False, 'productionReady': False}
                guard()
                write(args.output / 'completion.json', result)
                return result
            except BaseException as error:
                write(args.output / 'failure.json', {'error': str(error), 'processes': receipts, 'productionReady': False})
                raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='action', required=True)
    prepare_parser = sub.add_parser('prepare')
    for name in ('source', 'wasm-build', 'wasm', 'policy', 'original-policy', 'root-inventory', 'destination'):
        prepare_parser.add_argument('--' + name, type=Path, required=True)
    for name in ('wasm-build', 'wasm', 'policy', 'original-policy', 'root-inventory'):
        prepare_parser.add_argument('--' + name + '-sha256', required=True)
    run_parser = sub.add_parser('build')
    run_parser.add_argument('--source', type=Path, required=True)
    run_parser.add_argument('--configuration-sha256', required=True)
    run_parser.add_argument('--source-inventory-sha256', required=True)
    run_parser.add_argument('--output', type=Path, required=True)
    engine_parser = sub.add_parser('build-engine-wasm', help='build exported repair136/137 using the production Wasm corridor')
    engine_parser.add_argument('--spec-version', type=int, choices=(136, 137), required=True)
    for name in ('source', 'target', 'output', 'root-inventory', 'policy', 'original-policy', 'final-wasm', 'legacy-lock'):
        engine_parser.add_argument('--' + name, type=Path, required=True)
    for name in ('root-inventory', 'policy', 'original-policy', 'final-wasm', 'legacy-lock'):
        engine_parser.add_argument('--' + name + '-sha256', required=True)
    args = parser.parse_args()
    actions = {'prepare': prepare, 'build': run, 'build-engine-wasm': build_engine_wasm}
    print(json.dumps(actions[args.action](args), indent=2))


if __name__ == '__main__': main()
