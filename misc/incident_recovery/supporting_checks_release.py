"""Fresh SORA Nexus supporting checks and exact-artifact rebuild join.

An archived check/build report is never an admission input. Only this package
invocation can aggregate its fresh checks, rebuild and independently verified
rehearsal gates. Publication/restart and overall readiness are unchanged.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from uuid import uuid4

import production_build as build
import repair_reproduction as repair

GATE = 'fullClippyRuntimeAndProductionRehearsals'
SCHEMA = 'sora-nexus-supporting-check-input-v1'
MAX_OUTPUT = 256 * 1024**2
MAX_GRAPH = 64 * 1024**3
TIMEOUT = 24 * 3600
SOURCE_PATHS = list(dict.fromkeys([*build.SOURCE_PATHS,
    'housekeeping/clippy.sh', 'housekeeping/LINTING.md', 'housekeeping/test-clippy.sh',
    'housekeeping/test_lint_policy.py', 'scripts/with_llvm_env.sh']))
CHECKS = ('clippy-mainnet', 'clippy-try-runtime', 'clippy-extended',
          'runtime-tests', 'tool-tests', 'tool-clippy')
require = build.require


def inventory(root):
    names = subprocess.check_output(['git', 'ls-files', '--cached', '--others',
        '--exclude-standard', '-z', *SOURCE_PATHS], cwd=root).decode().split('\0')
    names = sorted(set(filter(None, names)))
    require({'Cargo.lock', 'rust-toolchain.toml', 'housekeeping/clippy.sh',
        'scripts/with_llvm_env.sh'} <= set(names), 'supporting source inventory incomplete')
    return {name: build.digest(build.release.local_file(root, name)) for name in names}


def helper_pins():
    result = build.helper_pins()
    result.update(repair.helper_pins())
    here = Path(__file__).resolve()
    result[str(here)] = build.digest(here)
    # A package hook imports other verifiers in this same process. Their source
    # is also in the complete root inventory, guarded throughout this run.
    return result


def source_binding():
    return build.pin_bytes(build.serialized(helper_pins()))['sha256']


def physical_directory(path):
    require(path.is_absolute() and path.is_dir() and path.resolve(strict=True) == path,
            'existing canonical directory required: ' + str(path))


def ref(root, value):
    require(isinstance(value, dict) and set(value) == {'file', 'sha256'}
        and re.fullmatch('[0-9a-f]{64}', value['sha256']), 'invalid supporting input reference')
    path = build.local(root, value['file'])
    build.read(path, value['sha256'], maximum=64 * 1024**2)
    return path


def bind(manifest, report, paths):
    """Bind data and source; none of these caller fields establishes success."""
    build.check_parents(manifest)
    value = build.parse(build.read(manifest))
    require(set(value) == {'schema', 'sourceInventory', 'fullSourceInventory',
        'buildInputs', 'legacyLock', 'finalLock'} and value['schema'] == SCHEMA,
        'unknown supporting-check manifest fields')
    build.no_fixture(value)
    refs = {key: ref(manifest.parent, value[key]) for key in
        ('sourceInventory', 'fullSourceInventory', 'buildInputs', 'legacyLock')}
    if value['finalLock'] is not None:
        refs['finalLock'] = ref(manifest.parent, value['finalLock'])
    production = build.verify_inventory(refs['sourceInventory'], value['sourceInventory']['sha256'], build.ROOT)
    complete = build.parse(build.read(refs['fullSourceInventory']))
    require(complete == inventory(build.ROOT), 'complete supporting source differs')
    require(all(name in complete and complete[name] == pin for name, pin in production.items()),
            'production and supporting source inventories differ')
    inputs, input_paths, _, commitment = build.load_inputs(refs['buildInputs'], value['buildInputs']['sha256'])
    require(report.get('policy', {}).get('evidenceCommitment') == commitment,
            'production data commitment differs from release policy')
    for name, role in ((inputs['compose']['csv'], 'signedMappingCsv'),
                       (inputs['compose']['inventory'], 'storageInventory')):
        require(role in paths and build.digest(input_paths[name]) == build.digest(paths[role]),
                'production input differs from release: ' + role)
    if 'operatorsScale' in inputs:
        require('operatorsScale' in paths and build.digest(input_paths[inputs['operatorsScale']]) == build.digest(paths['operatorsScale']),
                'optional operator assertion differs from release')
    require({'bootstrapWasm', 'holdWasm', 'finalWasm', 'policyScale'} <= paths.keys(),
            'exact recovery runtime artifacts required')
    return {'value': value, 'refs': refs, 'production': production, 'complete': complete,
            'inputPaths': input_paths, 'dataCommitment': commitment}


def commands(source):
    wrapper = str(source / 'scripts/with_llvm_env.sh')
    return {
        **{'clippy-' + mode: ['/bin/bash', str(source / 'housekeeping/clippy.sh'), mode]
           for mode in ('mainnet', 'try-runtime', 'extended')},
        'runtime-tests': [wrapper, 'cargo', 'test', '--offline', '--locked', '-p',
                          'framenode-runtime', '--lib'],
        'tool-tests': [wrapper, 'cargo', 'test', '--offline', '--locked', '-p',
                       'incident-recovery', '--features', 'bridge-fixture'],
        'tool-clippy': [wrapper, 'cargo', 'clippy', '--offline', '--locked', '-p',
                        'incident-recovery', '--all-targets', '--features', 'bridge-fixture',
                        '--', '-D', 'warnings'],
    }


def environment(target):
    # Keep cache/tool locations, never inherited Rust/Cargo flags or credentials.
    inherited = {key: os.environ[key] for key in
        ('HOME', 'PATH', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME', 'USER', 'LOGNAME') if key in os.environ}
    spec = importlib.util.spec_from_file_location('supporting_environment', build.build_helpers.ENVIRONMENT_HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    env, policy = module.prepare_build_environment(inherited, build.ROOT, run_token=uuid4().hex)
    # Supporting checks are native. The separate fresh production build below
    # owns all Wasm builds and must not inherit SKIP_WASM_BUILD.
    for key in list(env):
        if key.startswith('WASM_BUILD_') or key in ('FORCE_WASM_BUILD', 'SUBSTRATE_RUNTIME_TARGET'):
            env.pop(key)
            policy['controlled'].pop(key, None)
    controlled = {'CARGO_TARGET_DIR': str(target), 'CARGO_NET_OFFLINE': 'true',
                  'SKIP_WASM_BUILD': '1', 'CARGO_TERM_COLOR': 'never'}
    env.update(controlled)
    policy['controlled'].update(controlled)
    policy['inheritedLocations'] = inherited
    policy['cargoConfigurationPolicy'] = 'Only the source-inventory-bound root .cargo config; no ambient home/ancestor config'
    return env, policy


def tools_on_path(env):
    """Pin the resolved Cargo/Rust launchers without reading credential files."""
    result = {}
    for name in ('cargo', 'rustc'):
        found = shutil.which(name, path=env.get('PATH'))
        require(found is not None, 'missing preinstalled supporting tool: ' + name)
        path = Path(found).resolve(strict=True)
        result[name] = {'invokedPath': str(Path(found).absolute()),
                        'resolvedPath': str(path), 'pin': build.digest(path)}
    return result


def reject_ambient_cargo_config(env, root):
    """Check presence only: never open credential or personal config contents."""
    directories = {parent / '.cargo' for parent in root.parents}
    if 'HOME' in env:
        directories.add(Path(env['HOME']) / '.cargo')
    if 'CARGO_HOME' in env:
        directories.add(Path(env['CARGO_HOME']))
    for directory in directories:
        for name in ('config', 'config.toml'):
            path = directory / name
            require(not path.exists() and not path.is_symlink(),
                    'unreviewed ambient Cargo configuration is present: ' + str(path))


def aggregate(gates, rebuild_verified, required_repairs, reproduced_repairs=()):
    expected = set(build.release.GATES) - {GATE}
    require(set(gates) == set(build.release.GATES), 'release gate inventory differs')
    require(len(set(required_repairs)) == len(required_repairs)
        and set(reproduced_repairs) <= set(required_repairs), 'unexpected reproduced repair roles')
    missing = sorted(name for name in expected if gates[name].get('status') != 'verified')
    unreproduced = sorted(set(required_repairs) - set(reproduced_repairs))
    joined = rebuild_verified is True and not unreproduced
    return {'verified': joined and not missing, 'missingPrerequisites': missing,
            'requiredRepairRoles': sorted(required_repairs), 'unreproducedRepairRoles': unreproduced,
            'freshBuildJoinVerified': joined}


def retained_tree(root):
    entries = sorted(root.rglob('*'))
    require(all(not path.is_symlink() and path.resolve(strict=True) == path for path in entries),
            'output graph alias')
    files = {str(path): build.digest(path) for path in entries if path.is_file()
             and path.name != '.production-operation.lock'}
    require(len(files) <= 65536 and sum(pin['bytes'] for pin in files.values()) <= MAX_GRAPH,
            'supporting evidence graph bound')
    return files


def verify_rebuild(directory, result, paths, production):
    """Called only after the fixed, freshly launched production build succeeds."""
    require(result.get('schema') == 'sora-nexus-production-recovery-build-v1'
        and result.get('fixtureOnly') is False and result.get('status') == 'passed'
        and result.get('productionReady') is False, 'fresh production build did not pass')
    require(build.parse(build.read(directory / 'source-inventory.json')) == production,
            'fresh build used another source inventory')
    require(build.digest(directory / 'policy.scale') == build.digest(paths['policyScale']),
            'fresh composed policy differs from release')
    expected = {'134': ('build-134/runtime-134.compact.compressed.wasm', 'bootstrapWasm'),
                '135': ('build-135/runtime-135.compact.compressed.wasm', 'holdWasm'),
                'final138': ('build-final138/runtime-138.compact.compressed.wasm', 'finalWasm')}
    require(set(result['artifacts']) == set(expected), 'fresh build runtime set differs')
    joins = {}
    for name, (relative, role) in expected.items():
        actual = build.digest(directory / relative)
        require(result['artifacts'][name] == {'path': relative, **actual}
            and actual == build.digest(paths[role]), 'fresh rebuilt bytes differ from release: ' + role)
        joins[role] = actual
    require(result.get('files') == {str(Path(name).relative_to(directory)): pin
        for name, pin in retained_tree(directory).items() if Path(name) != directory / 'BUILD-MANIFEST.json'},
        'fresh production build graph changed')
    # Return the already joined producer pins, never a later rescan that could
    # accept changed evidence between validation and permanent registration.
    return joins, {str(directory / name): pin for name, pin in result['files'].items()}


def package_hook(report, paths, manifest, expected_source, output, checks_target,
                 final_target, legacy_target, verifier, verifier_sha256):
    """Run only from package() after fresh independent evidence hooks."""
    require(source_binding() == expected_source, 'fixed supporting verifier source differs')
    manifest, output = Path(manifest).absolute(), Path(output).absolute()
    context = bind(manifest, report, paths)
    require(not output.exists() and output.resolve() == output and not output.is_relative_to(build.ROOT),
            'new isolated supporting output directory required')
    targets = [Path(value).absolute() for value in (checks_target, final_target, legacy_target)]
    require(len(set(targets)) == 3, 'three separate persistent build lanes required')
    require(not any(left.is_relative_to(right) for left in targets for right in targets if left != right),
            'persistent build lanes must not contain one another')
    for target in targets:
        physical_directory(target)
        require(not target.is_relative_to(output) and not output.is_relative_to(target)
            and target != build.ROOT and target not in build.ROOT.parents, 'build lane overlaps source/output')
    require(all(not path.is_relative_to(build.ROOT) for path in targets[1:]),
            'production rebuild lanes must be outside working source')
    build.read(verifier, verifier_sha256, maximum=1024**3)
    records = {row['role']: row for row in report['artifacts']}
    require(len(records) == len(report['artifacts']) and set(records) == set(paths),
            'release artifact/path inventory differs')
    require(all(build.digest(path) == {key: records[role][key]
        for key in ('sha256', 'blake2_256', 'bytes')} for role, path in paths.items()),
        'release artifact changed before supporting admission')
    repairs = repair.plan(report, paths, verifier_sha256)
    inputs = {str(path): build.digest(path) for path in
        [manifest, verifier, *context['refs'].values(), *context['inputPaths'].values(), *paths.values()]}
    helpers = helper_pins()
    inputs.update(helpers)
    source = {str(build.ROOT / name): pin for name, pin in context['complete'].items()}
    pins = dict(inputs, **source)
    output.mkdir(parents=True)
    gates_before = build.serialized(report['gates'])
    launchers = None
    source_guards = []

    def retain(path, pin):
        require(str(path) not in pins or pins[str(path)] == pin, 'retained evidence rebinding')
        pins[str(path)] = pin

    def remember(path, value):
        pin = build.write(path, value)
        retain(path, pin)
        return pin

    def guard():
        build.guard_files(pins)
        for source_guard in source_guards:
            source_guard()
        require(inventory(build.ROOT) == context['complete'], 'complete source set changed')
        require(source_binding() == expected_source, 'supporting helper changed')
        require(build.serialized(report['gates']) == gates_before,
                'prerequisite gate results changed during supporting checks')
        if launchers is not None:
            require(tools_on_path(env) == launchers, 'supporting compiler launcher changed')
            reject_ambient_cargo_config(env, build.ROOT)

    guard()
    remember(output / 'input-manifest.json', build.read(manifest))
    remember(output / 'repair-reproduction-plan.json', repairs)
    remember(output / 'source-inventory.json', context['complete'])
    for name, pin in context['complete'].items():
        remember(output / 'checked-source' / name, build.read(build.ROOT / name, pin))
    remember(output / 'release-joins.json', {
        'artifacts': {role: build.digest(path) for role, path in sorted(paths.items())},
        'policy': report.get('policy'), 'prerequisiteGates': report['gates'],
        'sourceBinding': expected_source, 'productionReady': False})
    # Retain external helpers as bytes, not only paths to a mutable checkout.
    for index, (name, pin) in enumerate(sorted(helpers.items())):
        remember(output / 'helpers' / (str(index).zfill(3) + '-' + Path(name).name),
                 build.read(Path(name), pin, maximum=1024**3))
    env, env_policy = environment(targets[0])
    reject_ambient_cargo_config(env, build.ROOT)
    remember(output / 'environment.json', env_policy)
    launchers = tools_on_path(env)
    remember(output / 'compiler-launchers.json', launchers)
    for index, entry in enumerate(launchers.values()):
        source_path = Path(entry['resolvedPath'])
        pins[str(source_path)] = entry['pin']
        remember(output / 'helpers' / ('launcher-' + str(index)),
                 build.read(source_path, entry['pin'], maximum=1024**3))
    cmd = commands(build.ROOT)
    receipts = {}
    with build.build_helpers.target_lock(targets[0]):
        for name, command in [('rustc-version', ['rustc', '-Vv']), ('cargo-version', ['cargo', '-Vv']),
                              *[(name, cmd[name]) for name in CHECKS]]:
            _, receipt = build.process(command, build.ROOT, output / 'checks' / name, guard,
                env=env, timeout=TIMEOUT, maximum=MAX_OUTPUT, retain=retain)
            require(type(receipt['exitCode']) is int and receipt['exitCode'] == 0,
                    'supporting command failed')
            receipts[name] = build.digest(output / 'checks' / name / 'execution.json')
    refs, value = context['refs'], context['value']
    directory = output / 'rebuilt'
    command = [str(Path(sys.executable).resolve()), '-B', str(Path(build.__file__).resolve()), 'build']
    for flag, path, expected in (
        ('inputs', refs['buildInputs'], value['buildInputs']['sha256']),
        ('source-inventory', refs['sourceInventory'], value['sourceInventory']['sha256']),
        ('tool', verifier, verifier_sha256),
        ('legacy-lock', refs['legacyLock'], value['legacyLock']['sha256'])):
        command += ['--' + flag, str(path), '--' + flag + '-sha256', expected]
    command += ['--output', str(directory), '--final-target', str(targets[1]), '--legacy-target', str(targets[2])]
    if 'finalLock' in refs:
        command += ['--final-lock', str(refs['finalLock']), '--final-lock-sha256', value['finalLock']['sha256']]
    # Three sequential Wasm builds each already has a 24h bound. This parent
    # budget covers all three plus native preparation on a busy machine.
    stdout, receipt = build.process(command, build.ROOT, output / 'rebuild-process', guard,
        env={key: os.environ[key] for key in ('HOME', 'PATH', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME', 'USER', 'LOGNAME')
             if key in os.environ}, timeout=4 * TIMEOUT, maximum=MAX_OUTPUT, retain=retain)
    require(type(receipt['exitCode']) is int and receipt['exitCode'] == 0, 'fresh rebuild failed')
    summary = build.parse(stdout)
    require(set(summary) == {'status', 'manifest', 'manifestPin', 'productionReady'}
        and summary['status'] == 'passed' and summary['productionReady'] is False
        and summary['manifest'] == str(directory / 'BUILD-MANIFEST.json'),
        'fresh production build stdout contract differs')
    result = build.parse(build.read(directory / 'BUILD-MANIFEST.json',
        summary['manifestPin'], maximum=64 * 1024**2))
    joins, rebuilt_pins = verify_rebuild(directory, result, paths, context['production'])
    rebuilt_pins[str(directory / 'BUILD-MANIFEST.json')] = summary['manifestPin']
    for name, pin in rebuilt_pins.items():
        retain(Path(name), pin)
    repair_joins = repair.rebuild(repairs, paths, output / 'repairs',
        directory / 'build-final138/runtime-138.compact.compressed.wasm', context, targets[2],
        guard, remember, retain, source_guards,
        {key: os.environ[key] for key in ('HOME', 'PATH', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME', 'USER', 'LOGNAME')
         if key in os.environ})
    joins.update(repair_joins)
    outcome = aggregate(report['gates'], joins is not None, [row['role'] for row in repairs], repair_joins)
    completion = {'schema': 'sora-nexus-supporting-check-completion-v1', 'status': 'passed',
        'sourceBinding': expected_source, 'sourceInventory': build.digest(output / 'source-inventory.json'),
        'checks': receipts, 'rebuiltArtifacts': joins, 'aggregate': outcome,
        'repairPolicies': [{'version': row['version'], 'originalPolicy': row['originalPolicy'],
                            'activePolicy': row['activePolicy']} for row in repairs], 'productionReady': False}
    remember(output / 'completion.json', completion)
    guard()
    require(retained_tree(output) == {name: pin for name, pin in pins.items()
        if Path(name).is_relative_to(output)}, 'unexpected or changed supporting output graph')
    additions, added_paths = [], {}
    for index, (name, pin) in enumerate(sorted(pins.items())):
        path = Path(name)
        if not path.is_relative_to(output):
            continue
        role = 'supportingCheckEvidence' + str(index).zfill(6)
        require(role not in paths, 'supporting evidence role collision')
        additions.append({'role': role, 'sourcePath': 'generated-supporting-checks/' + str(path.relative_to(output)),
            'object': 'objects/' + pin['sha256'], **pin, 'executable': False})
        added_paths[role] = path
    require(len(additions) + len(report['artifacts']) <= 65536
        and sum(row['bytes'] for row in additions) <= MAX_GRAPH, 'supporting package inventory bound')
    guard()
    completion_pin = pins[str(output / 'completion.json')]
    report['artifacts'].extend(additions)
    paths.update(added_paths)
    report['supportingChecks'] = completion
    if outcome['verified']:
        report['gates'][GATE] = {'status': 'verified', 'sourceBinding': expected_source,
            'reason': 'Fresh full Clippy/runtime checks; freshly rebuilt exact release Wasms; all other independent production gates verified in this package invocation',
            'completionSha256': completion_pin['sha256']}
    report['productionReady'] = False
    return completion


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--inventory-output', type=Path, required=True)
    args = parser.parse_args()
    build.write(args.inventory_output, inventory(build.ROOT))
    print(json.dumps({'sourceBinding': source_binding(), 'productionReady': False}))


if __name__ == '__main__':
    main()
