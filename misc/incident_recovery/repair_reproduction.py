"""Fresh repair-engine reproduction for the owned supporting-check corridor.

Final138 deliberately retains the ORIGINAL policy. Only repair136/137 compile
an ACTIVE revised policy; both policies remain in each repair source export.
"""
from pathlib import Path
import sys

import production_build as build
import production_native as native

require = build.require
MAX_LOG = 256 * 1024**2
TIMEOUT = 24 * 3600


def helper_pins():
    pins, _, _, _ = native.reviewed_assets()
    here = Path(__file__).resolve()
    pins[str(here)] = build.digest(here)
    return pins


def plan(report, paths, verifier_sha256):
    prefixed = {role for role in paths if role.startswith(('repair136', 'repair137'))}
    if not prefixed and 'forwardRepairReplay' not in paths:
        return []
    context = report.get('forwardRepairReplay')
    require(isinstance(context, dict) and 'forwardRepairReplay' in paths
        and paths.get('recoveryParentReplay') == paths['forwardRepairReplay'],
        'repair reproduction requires the exact forward-replay context')
    gate = report['gates']['automaticActivationEscapeAndForwardRepair']
    require(gate.get('status') == 'verified' and gate.get('verifierSha256') == verifier_sha256,
        'repair reproduction requires fresh same-verifier forward execution')
    versions = context['repairs']
    require(versions in ([136], [137], [136, 137])
        and all(type(version) is int for version in versions), 'unsupported repair sequence')
    original_raw = build.read(paths['policyScale'])
    original, controls = native.nonfixture_policy(original_raw)
    require(context['originalPolicy'] == original, 'forward original policy differs')
    base = build.parse(build.read(paths['holdBinding']))
    require(base['policyScale'] == '0x' + original_raw.hex()
        and base['policyCommitment'] == '0x' + original['commitment'],
        'hold must preserve original policy')
    require(bytes.fromhex(base['operatorsScale'].removeprefix('0x')) == build.read(paths['operatorsScale']),
        'reproduction operators differ')
    rows, expected_prefixes = [], set()
    for version in versions:
        prefix = 'repair' + str(version)
        required_roles = {prefix + suffix for suffix in ('PolicyScale', 'Binding', 'Provenance', 'BuildReceipt')}
        require(required_roles <= paths.keys(), 'repair source/policy roles missing')
        expected_prefixes.update(required_roles)
        code = context['wasm'][str(version)]
        role = code['role']
        require(role in paths, 'repair Wasm role missing')
        if role.startswith(prefix):
            expected_prefixes.add(role)
        wasm = build.digest(paths[role])
        require(wasm['sha256'] == code['sha256'] and wasm['blake2_256'] == code['blake2'],
            'rehearsed repair code differs')
        identity = build.release.wasm_identity(build.read(paths[role], maximum=50 * 1024**2))
        require(identity['specVersion'] == version and identity['transactionVersion'] == 132,
            'repair code identity differs')
        raw = build.read(paths[prefix + 'PolicyScale'])
        active, active_controls = native.nonfixture_policy(raw)
        require(active_controls == controls and context['policies'][str(version)]['commitment'] == active['commitment'],
            'repair changes original controls or rehearsed policy')
        binding_path = paths[prefix + 'Binding']
        binding = build.parse(build.read(binding_path))
        require(set(binding) == {'policyScale', 'policyCommitment', 'originalPolicyScale',
            'originalPolicyCommitment', 'operatorsScale', 'previousGrandpaSetId', 'firstRecoverySlot', 'finalWasm'},
            'repair binding fields differ')
        require(binding['policyScale'] == '0x' + raw.hex()
            and binding['policyCommitment'] == '0x' + active['commitment']
            and binding['originalPolicyScale'] == '0x' + original_raw.hex()
            and binding['originalPolicyCommitment'] == '0x' + original['commitment'],
            'active/original repair lineage differs')
        require(all(binding[key] == base[key] for key in
            ('operatorsScale', 'previousGrandpaSetId', 'firstRecoverySlot')),
            'repair consensus/operator lineage differs')
        final = build.release.local_file(binding_path.parent, binding['finalWasm'])
        require(build.digest(final) == build.digest(paths['finalWasm']), 'repair final138 differs')
        rows.append({'version': version, 'role': role, 'policyRole': prefix + 'PolicyScale',
            'binding': binding, 'originalPolicy': build.digest(paths['policyScale']),
            'activePolicy': build.digest(paths[prefix + 'PolicyScale']), 'wasm': wasm})
    require(prefixed == expected_prefixes, 'unused or unknown repair roles')
    require('incomingActivePolicyScale' in paths
        and build.digest(paths['incomingActivePolicyScale']) == rows[-1]['activePolicy'],
        'terminal active policy differs from last repair')
    return rows


def verify_export(stdout, source, binding_pin):
    summary = build.parse(stdout)
    require(set(summary) == {'destination', 'bound', 'releaseReady', 'provenancePin'}
        and summary['destination'] == str(source) and summary['bound'] is True
        and summary['releaseReady'] is False, 'fresh repair export stdout differs')
    path = source / 'RECOVERY-PROVENANCE.json'
    provenance = build.parse(build.read(path, summary['provenancePin'], maximum=64 * 1024**2))
    require(provenance.get('fixtureOnly') is False and provenance.get('bound') is True
        and provenance.get('releaseReady') is False
        and provenance['bindingEvidence']['binding'] == {key: binding_pin[key] for key in ('sha256', 'blake2_256')},
        'fresh repair export binding/provenance differs')
    actual = build.fingerprint(source)
    require(set(actual) == set(provenance['sourceFiles'])
        and all(all(actual[name][key] == provenance['sourceFiles'][name][key]
                    for key in ('sha256', 'blake2_256')) for name in actual),
        'fresh repair export source graph differs')
    return provenance, summary['provenancePin'], actual


def verify_engine(stdout, directory, source, row, root_inventory_pin, expected_source):
    result = build.parse(stdout)
    version = row['version']
    require(result.get('schema') == 'sora-nexus-production-repair-engine-build-v1'
        and type(result.get('specVersion')) is int and result['specVersion'] == version
        and result.get('transactionVersion') == 132 and result.get('status') == 'passed'
        and result.get('fixtureOnly') is False and result.get('productionReady') is False
        and result.get('rootInventory') == root_inventory_pin,
        'fresh repair engine result differs')
    completion = directory / 'completion.json'
    completion_pin = build.pin_bytes(build.serialized(result))
    build.read(completion, completion_pin, maximum=64 * 1024**2)
    wasm = directory / 'wasm' / ('runtime-' + str(version) + '.compact.compressed.wasm')
    require(result['wasm'] == {'path': str(wasm), **row['wasm']}
        and build.digest(wasm) == row['wasm'], 'fresh repair Wasm differs from exact rehearsed artifact')
    receipt = directory / 'wasm/build.json'
    receipt_pin = {key: result['buildReceipt'][key] for key in ('sha256', 'blake2_256', 'bytes')}
    require(result['buildReceipt']['path'] == str(receipt), 'repair build receipt path differs')
    build_report = build.parse(build.read(receipt, receipt_pin))
    require(build_report.get('schema') == 'sora-nexus-production-recovery-wasm-build-v1'
        and build_report.get('fixtureOnly') is False and build_report.get('productionReady') is False
        and build_report.get('source') == str(source) and build_report['specVersion'] == version
        and build_report.get('transactionVersion') == 132 and build_report['status'] == 'passed'
        and type(build_report['exitCode']) is int and build_report['exitCode'] == 0,
        'fresh repair compiler receipt failed')
    process_pins = native.validate_process(build_report, receipt, source)
    source_files = directory / 'wasm/source-files.json'
    source_provenance = directory / 'wasm/source-provenance.json'
    require(build.parse(build.read(source_files, build_report['sourceFingerprint'])) == expected_source,
        'actual compiled repair source inventory differs')
    require(build.read(source_provenance, build_report['sourceProvenance'])
        == build.read(source / 'RECOVERY-PROVENANCE.json'), 'actual compiled repair provenance differs')
    logs = build_report.get('logs')
    require(isinstance(logs, dict) and set(logs) == {'cargo.stdout.log', 'cargo.stderr.log'},
        'fresh release-compatible repair logs missing')
    for name, pin in logs.items():
        require(build.read(directory / 'wasm' / name, pin, build.MAX_LOG)
            == build.read(directory / 'wasm/cargo' / name.removeprefix('cargo.'), maximum=build.MAX_LOG),
            'adjacent repair logs differ from actual process')
    pins = result['inputPins']
    require(isinstance(pins, dict) and len(pins) <= 65536
        and all(pins.get(str(source / name)) == pin for name, pin in expected_source.items()),
        'repair compiler source pins differ from fresh export')
    require(all(pins.get(name) == pin for name, pin in process_pins.items())
        and pins.get(str(source_files)) == build_report['sourceFingerprint']
        and pins.get(str(source_provenance)) == build_report['sourceProvenance'],
        'actual process/source evidence differs from producer pins')
    retained = {name: pin for name, pin in pins.items() if Path(name).is_relative_to(directory)
                or Path(name).is_relative_to(source)}
    require(retained.get(str(receipt)) == receipt_pin and retained.get(str(wasm)) == row['wasm'],
        'repair actual artifact receipts absent from retained compiler pins')
    # Quarantined exports are historical diagnostics, never fresh build outputs.
    # Their pins are taken from the actual, already pinned build receipt.
    for previous in build_report['previousExports']:
        require(previous['file'] in build.build_helpers.EXPORTS, 'unknown quarantined export')
        name = directory / 'wasm/previous-exports' / previous['file']
        require(str(name) not in retained, 'duplicate quarantined export')
        retained[str(name)] = {key: previous[key] for key in ('sha256', 'blake2_256', 'bytes')}
    retained[str(completion)] = completion_pin
    build.guard_files(retained)
    return row['wasm'], retained


def rebuild(rows, paths, directory, base_final, context, target, guard, remember, retain,
            source_guards, env):
    """Reuse exact source exporter and production-native fixed Wasm builder."""
    if not rows:
        return {}
    directory.mkdir()
    original = build.read(paths['policyScale'])
    require(build.digest(base_final) == build.digest(paths['finalWasm']),
        'repair reproduction must use exact freshly rebuilt final138')
    remember(directory / 'bindings/final138.wasm', build.read(base_final, maximum=32 * 1024**2))
    original_path = directory / 'bindings/original-policy.scale'
    remember(original_path, original)
    joins = {}
    for row in rows:
        version = row['version']
        active_path = directory / 'bindings' / ('policy-' + str(version) + '.scale')
        remember(active_path, build.read(paths[row['policyRole']], row['activePolicy']))
        binding = dict(row['binding'], finalWasm='final138.wasm')
        binding_path = directory / 'bindings' / ('binding-' + str(version) + '.json')
        binding_pin = remember(binding_path, binding)
        source = directory / ('source-' + str(version))
        command = [str(Path(sys.executable).resolve()), '-B', str(Path(build.bootstrap.__file__).resolve()),
            '--destination', str(source), '--spec-version', str(version), '--binding', str(binding_path)]
        stdout, _ = build.process(command, build.ROOT, directory / ('export-' + str(version)),
            guard, env=env, timeout=TIMEOUT, maximum=MAX_LOG, retain=retain)
        provenance, provenance_pin, before = verify_export(stdout, source, binding_pin)
        require(provenance['specVersion'] == version and provenance['transactionVersion'] == 132,
            'exported repair version differs')
        retain(source / 'RECOVERY-PROVENANCE.json', provenance_pin)
        for name, pin in before.items():
            if name != 'Cargo.lock':
                retain(source / name, pin)
        resolved_lock = build.digest(context['refs']['legacyLock'])
        allowed_locks = [before['Cargo.lock'], resolved_lock]
        def source_guard(source=source, before=before, allowed_locks=allowed_locks):
            current = build.fingerprint(source)
            require(set(current) == set(before) and current['Cargo.lock'] in allowed_locks
                and all(current[name] == pin for name, pin in before.items() if name != 'Cargo.lock'),
                'repair exported source changed')
        source_guards.append(source_guard)
        output = directory / ('build-' + str(version))
        command = [str(Path(sys.executable).resolve()), '-B', str(Path(native.__file__).resolve()),
            'build-engine-wasm', '--spec-version', str(version), '--source', str(source),
            '--target', str(target), '--output', str(output)]
        for flag, path in [('root-inventory', context['refs']['sourceInventory']),
            ('policy', active_path), ('original-policy', original_path),
            ('final-wasm', directory / 'bindings/final138.wasm'), ('legacy-lock', context['refs']['legacyLock'])]:
            command += ['--' + flag, str(path), '--' + flag + '-sha256', build.digest(path)['sha256']]
        stdout, _ = build.process(command, build.ROOT, directory / ('engine-' + str(version)),
            guard, env=env, timeout=2 * TIMEOUT, maximum=MAX_LOG, retain=retain)
        allowed_locks[:] = [resolved_lock]
        source_guard()
        after = dict(before, **{'Cargo.lock': resolved_lock})
        joined, pins = verify_engine(stdout, output, source, row,
            build.digest(context['refs']['sourceInventory']), after)
        for name, pin in pins.items():
            retain(Path(name), pin)
        joins[row['role']] = joined
        guard()
    return joins
