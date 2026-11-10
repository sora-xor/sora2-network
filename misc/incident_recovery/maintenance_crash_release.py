"""Fresh committed-snapshot process death; never a database-commit/consensus proof.

This hook has no archived-report or caller-certificate input. Its native process
establishes an opaque incoming certificate and baseline, then SIGKILLs each
importer after the committed snapshot barrier and resumes that exact snapshot.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import shutil

import artifact_identity_release as identity
import forward_repair_release as forward
import history_release as history
import incoming_release
import incoming_replay_release as incoming
import production_build as build

GATE = 'chunksCrashesAndRestarts'
SCOPE = 'committedSnapshotProcessDeath'
DAY = 24 * 3600
MAX_LOG = 64 * 1024**2
MAX_FILES = 131072
MAX_GRAPH = 3 * identity.MAX_GRAPH
WORKER_SCHEMA = 'sora-nexus-maintenance-crash-worker-v1'
COVERAGE = 'SIGKILL after committed, fsynced snapshot; next process continues the killed worker snapshot chain'
require, keys, uint = identity.require, identity.keys, identity.uint


def same(actual, expected, message):
    require(build.serialized(actual) == build.serialized(expected), message)


def helper_pins():
    import release_package as package
    result = build.helper_pins()
    for module in (identity, forward, history, incoming_release, incoming, package, identity.encoding):
        path = Path(module.__file__).resolve()
        result[str(path)] = build.digest(path)
    # Freeze transitive host helpers too; a later package import must not change
    # the implementation during a long rehearsal. This is a bounded source set.
    sources = sorted(Path(__file__).resolve().parent.glob('*.py'))
    require(len(sources) <= 512, 'host helper source bound')
    for path in sources:
        result[str(path)] = build.digest(path)
    # Host source pinning is not a claim that these sources built the executable.
    for name in ('main.rs', 'maintenance.rs', 'maintenance_snapshot.rs', 'maintenance_process_death.rs',
                 'incoming_replay.rs'):
        path = build.ROOT / 'utils/incident-recovery/src' / name
        result[str(path)] = build.digest(path)
    return result


def source_binding():
    return build.pin_bytes(build.serialized(helper_pins()))['sha256']


def prerequisites(report, expected_sha):
    context = report.get('forwardRepairReplay')
    require(isinstance(context, dict), 'fresh forward-repair context required')
    for gate in (identity.GATE, forward.GATE):
        row = report.get('gates', {}).get(gate, {})
        require(row.get('status') == 'verified' and row.get('verifierSha256') == expected_sha,
                'same-invocation exact identity and forward-repair gates required')
    incoming_gate = report['gates'].get('incomingPendingCoverage', {})
    receipt = incoming_gate.get('receipt')
    require(incoming_gate.get('status') == 'verified' and isinstance(receipt, dict)
        and any(receipt is row for row in report.get('nativeVerifierReceipts', []))
        and receipt.get('gate') == 'incomingPendingCoverage'
        and receipt.get('verifierSha256') == expected_sha and type(receipt.get('exitCode')) is int
        and receipt['exitCode'] == 0, 'fresh same-package incoming receipt required; archived reports are not admissible')
    result = receipt.get('result')
    require(isinstance(result, dict) and incoming_release.valid_receipt(result, report,
        context['artifactBindings'], history.ROOT), 'fresh incoming coverage no longer matches package')
    count = result.get('priorPolicyIncomingOperations')
    require(type(count) is int and count > 0, 'positive historical incoming coverage required')
    plan = context['scenarios']['repair']
    scenario = report['gates'][forward.GATE].get('scenarios', {}).get('repair', {})
    require(scenario.get('manifestSha256') == plan['manifestSha256']
        and scenario.get('replayTranscriptSha256') == result['priorPolicyReplay']['replayTranscriptSha256']
        and scenario.get('blocks') == len(plan['steps']), 'fresh incoming and forward executions differ')
    return context, result['priorPolicyReplay'], count


def receipt(evidence, value, name, maximum=MAX_LOG):
    keys(value, {'file', 'sha256'})
    require(value['file'] == name, 'worker receipt path differs')
    return evidence.pinned(evidence.local(name), value['sha256'], maximum)


def worker(row, index, block, prior, context, evidence, executable_sha):
    keys(row, {'index', 'kind', 'ready', 'process', 'snapshotAuthenticatedAfterKill',
        'launchReceipt', 'processReceipt', 'readyReceipt', 'startedReceipt', 'jobReceipt', 'stdoutReceipt', 'stderrReceipt'})
    imported = block is not None
    require(type(row['index']) is int and row['index'] == index
        and row['kind'] == ('import' if imported else 'terminal-restore')
        and row['snapshotAuthenticatedAfterKill'] is True, 'worker count/order/restore evidence differs')
    prefix = f'stages/{index:06d}/'
    job_raw = receipt(evidence, row['jobReceipt'], f'jobs/{index:06d}.json', 64 * 1024)
    job = history.parse(job_raw)
    expected = block if imported else prior
    block_pin = None if not imported else {'file': f'inputs/{block["number"]}.block.scale', 'sha256': block['blockSha256']}
    engine_pin = None
    if imported:
        version = str(context['scenarios']['repair']['steps'][index]['executingSpec'])
        engine = context['wasm'][version]
        engine_pin = {'file': 'inputs/' + engine['blake2'] + '.wasm', 'sha256': engine['sha256']}
        receipt(evidence, engine_pin, engine_pin['file'], 50 * 1024**2)
        receipt(evidence, block_pin, block_pin['file'], 64 * 1024**2)
    expected_job = {'schema': WORKER_SCHEMA, 'index': index, 'parentSnapshot': prior['snapshot'],
        'parentRoot': prior['stateRoot'], 'parentHash': prior['hash'], 'parentNumber': identity.scale.CHECKPOINT + index,
        'block': block_pin, 'engine': engine_pin, 'bootstrapBlake2': context['wasm']['134']['blake2'],
        'deployedBlake2': context['checkpointCodeBlake2'], 'expectedRoot': expected['stateRoot'], 'expectedHash': expected['hash']}
    same(job, expected_job, 'worker job resets/skips prior killed snapshot or changes exact block/code')
    job_sha = row['jobReceipt']['sha256']
    command = [str(evidence.output / 'worker-executable'), 'maintenance-crash-worker', '--job',
               str(evidence.output / f'jobs/{index:06d}.json')]
    launch = history.parse(receipt(evidence, row['launchReceipt'], prefix + 'launch.json'))
    same(launch, {'schema': 'sora-nexus-maintenance-crash-launch-v1', 'executableSha256': executable_sha,
        'jobSha256': job_sha, 'command': command, 'deadlineSeconds': DAY, 'productionReady': False}, 'worker launch differs')
    started = history.parse(receipt(evidence, row['startedReceipt'], prefix + 'started.json'))
    keys(started, {'pid', 'executableSha256', 'command', 'jobSha256', 'deadlineSeconds', 'killScope'})
    require(uint(started['pid'], 2**32 - 1) > 0, 'worker PID invalid')
    same(started, {'pid': started['pid'], 'executableSha256': executable_sha, 'command': command,
        'jobSha256': job_sha, 'deadlineSeconds': DAY,
        'killScope': 'owned importer process; no subprocesses are spawned by the worker'}, 'started worker differs')
    ready = history.parse(receipt(evidence, row['readyReceipt'], prefix + 'ready.json'))
    same(ready, row['ready'], 'observed barrier bytes differ')
    keys(ready, {'schema', 'pid', 'jobSha256', 'parentSnapshot', 'snapshot', 'root', 'blockHash',
        'imported', 'durableSnapshotBarrier', 'standaloneEvidenceAdmissible'})
    next_index = index + int(imported)
    snapshot_name = f'state/snapshots/{next_index:06d}.json'
    receipt(evidence, ready['snapshot'], snapshot_name, 64 * 1024)
    same(ready, {'schema': WORKER_SCHEMA, 'pid': started['pid'], 'jobSha256': job_sha,
        'parentSnapshot': prior['snapshot'], 'snapshot': ready['snapshot'], 'root': expected['stateRoot'],
        'blockHash': expected['hash'], 'imported': imported, 'durableSnapshotBarrier': True,
        'standaloneEvidenceAdmissible': False}, 'worker barrier/root/continuation differs')
    if not imported:
        same(ready['snapshot'], prior['snapshot'], 'terminal restore replaced killed snapshot')
    process = history.parse(receipt(evidence, row['processReceipt'], prefix + 'process.json'))
    same(process, row['process'], 'observed process receipt bytes differ')
    keys(process, {'schema', 'pid', 'deliberateKillAfterBarrier', 'killRequested', 'killError', 'actualExitCode',
        'actualExitSignal', 'elapsedMillis', 'failure', 'jobSha256', 'executableSha256',
        'stdoutSha256', 'stderrSha256', 'stdoutHashError', 'stderrHashError'})
    uint(process['elapsedMillis'])
    for stream in ('stdout', 'stderr'):
        receipt(evidence, row[stream + 'Receipt'], prefix + stream + '.log')
        require(process[stream + 'Sha256'] == row[stream + 'Receipt']['sha256'], 'worker log differs')
    same(process, {'schema': 'sora-nexus-maintenance-process-death-v1', 'pid': started['pid'],
        'deliberateKillAfterBarrier': True, 'killRequested': True, 'killError': None, 'actualExitCode': None,
        'actualExitSignal': 9, 'elapsedMillis': process['elapsedMillis'], 'failure': None, 'jobSha256': job_sha,
        'executableSha256': executable_sha, 'stdoutSha256': row['stdoutReceipt']['sha256'],
        'stderrSha256': row['stderrReceipt']['sha256'], 'stdoutHashError': None, 'stderrHashError': None},
        'worker did not complete actual owned SIGKILL after the durable barrier')
    return {'hash': expected['hash'], 'stateRoot': expected['stateRoot'], 'snapshot': ready['snapshot']}


def validate(result, context, binding, count, output, pins, executable_sha):
    keys(result, {'schema', 'baseline', 'incomingReplayBinding', 'executableSha256', 'finalIdentity', 'workers',
        'coverage', 'allBlocksImportedFromContinuedSnapshots', 'terminalSnapshotRestoredInFreshProcess',
        'inCommitCrashVerified', 'consensusSealsVerified', 'productionReady'})
    require(result['schema'] == 'sora-nexus-maintenance-process-death-rehearsal-v1'
        and result['executableSha256'] == executable_sha and result['coverage'] == COVERAGE
        and all(result[key] is True for key in ('allBlocksImportedFromContinuedSnapshots', 'terminalSnapshotRestoredInFreshProcess'))
        and all(result[key] is False for key in ('inCommitCrashVerified', 'consensusSealsVerified', 'productionReady')),
        'crash rehearsal execution identity/scope differs')
    evidence = identity.OutputEvidence(output, pins)
    same(evidence.document('report.json'), result, 'fresh process stdout differs from retained report')
    worker_exe = evidence.local('worker-executable')
    require(pins[str(worker_exe)]['sha256'] == executable_sha and worker_exe.stat().st_mode & 0o777 == 0o500,
            'worker executable differs')
    baseline = result['baseline']
    forward.validate_baseline(baseline, context, 'repair', output / 'baseline', pins)
    same(result['incomingReplayBinding'], binding, 'fresh opaque certificate differs from same-package incoming coverage')
    same(baseline['incomingReplayBinding'], binding, 'fresh native baseline lost its opaque incoming binding')
    require(len(baseline['priorPolicyIncomingAudits']) == count
        and incoming.proof_joins(baseline, context['incomingContext'], context['policyCommitment']),
        'crash baseline historical incoming audits/final state differ')
    rows = baseline['blocks']
    final = rows[-1]
    expected_final = {'blockHash': final['hash'], 'stateRoot': final['stateRoot'],
        'activePolicyCommitment': context['incomingContext']['activePolicy']['commitment'],
        'originalPolicyCommitment': context['policyCommitment'], 'phase': 'Complete',
        'cursor': final['cursor'], 'operationCounter': final['operationCounter'],
        'runtimeCodeBlake2': context['wasm']['138']['blake2']}
    same(result['finalIdentity'], expected_final, 'continued snapshot final runtime/policy/state differs')
    require(binding['finalPolicyCommitment'] == expected_final['activePolicyCommitment']
        and binding['finalStateRoot'] == final['stateRoot'], 'opaque binding does not cover continued final state')
    workers = result['workers']
    require(isinstance(workers, list) and len(workers) == len(rows) + 1, 'missing block kill or fresh terminal restore')
    initial = evidence.pinned(evidence.local('state/snapshots/000000.json'), maximum=64 * 1024)
    require(initial == evidence.pinned(evidence.local('baseline/snapshots/000000.json'), maximum=64 * 1024),
            'continued chain starts from another checkpoint snapshot')
    prior = {'hash': identity.scale.CHECKPOINT_HASH, 'stateRoot': history.ROOT,
             'snapshot': {'file': 'state/snapshots/000000.json', 'sha256': identity.sha(initial)}}
    continued = []
    for index, row in enumerate(workers):
        block = rows[index] if index < len(rows) else None
        prior = worker(row, index, block, prior, context, evidence, executable_sha)
        if block is not None:
            continued.append({**block, 'diskSnapshotManifestSha256': prior['snapshot']['sha256']})
    identity.OutputEvidence(output / 'state', pins).journal(continued)
    require(prior['hash'] == final['hash'] and prior['stateRoot'] == final['stateRoot'], 'terminal process reset the snapshot chain')
    return {'manifestSha256': baseline['manifestSha256'], 'replayTranscriptSha256': baseline['replayTranscriptSha256'],
        'blocks': len(rows), 'deliberateSigkills': len(workers), 'finalIdentity': expected_final,
        'incomingReplayBinding': binding, 'coverageScope': SCOPE, 'committedSnapshotProcessDeathVerified': True,
        'inCommitCrashVerified': False, 'consensusProcessInterruptionVerified': False,
        'consensusSealsVerified': False, 'productionReady': False}


def tree(directory):
    entries = sorted(directory.rglob('*'))
    require(len(entries) <= MAX_FILES and all(not path.is_symlink() and path.resolve(strict=True) == path
        and (path.is_dir() or path.is_file()) for path in entries), 'generated crash evidence alias/bound')
    pins = {str(path): build.digest(path) for path in entries if path.is_file()}
    require(sum(pin['bytes'] for pin in pins.values()) <= MAX_GRAPH, 'generated crash evidence size bound')
    return pins


def package_hook(report, paths, binary, expected_sha, expected_source, output):
    """The only success path launches the fixed command in this invocation."""
    import release_package as package
    identity.digest_hex(expected_sha); identity.digest_hex(expected_source)
    helpers = helper_pins()
    require(build.pin_bytes(build.serialized(helpers))['sha256'] == expected_source, 'crash hook source binding differs')
    context, binding, count = prerequisites(report, expected_sha)
    require(output.is_absolute() and output.resolve() == output and not output.exists(), 'new absolute crash output required')
    build.check_parents(output.parent)
    original = {row['role']: row for row in report['artifacts']}
    pins, owned = {}, {}

    def retain(path, expected):
        require(str(path) not in pins or pins[str(path)] == expected, 'accepted evidence cannot be rebound')
        pins[str(path)] = expected

    def keep(path, value):
        expected = build.write(path, value)
        retain(path, expected)
        return expected

    def copy(source, target, expected, executable=False):
        require(build.digest(source) == expected, 'source changed before owned copy')
        retain(source, expected)
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            with source.open('rb') as src, target.open('xb') as dst:
                shutil.copyfileobj(src, dst, 1024 * 1024)
            target.chmod(0o555 if executable else 0o444)
        require(build.digest(target) == expected and build.digest(source) == expected
            and target.stat().st_mode & 0o777 == (0o555 if executable else 0o444), 'owned copy changed')
        retain(target, expected)

    # Freeze every original release input, including earlier fresh gate outputs.
    for role, path in paths.items():
        record = original[role]
        pin = {key: record[key] for key in ('sha256', 'blake2_256', 'bytes')}
        require(build.digest(path) == pin, 'release input changed before crash rehearsal')
        retain(path, pin)
    output.mkdir()
    try:
        for role in context['inputRoles']:
            record = original[role]
            source = paths[role]
            relative = record['sourcePath']
            target = output / 'inputs' / relative
            require(not Path(relative).is_absolute() and '..' not in Path(relative).parts
                and target.resolve().is_relative_to(output / 'inputs'), 'unsafe owned input path')
            copy(source, target, pins[str(source)], record['executable'])
            owned[role] = str(target)
        for index, (name, pin) in enumerate(sorted(helpers.items())):
            source = Path(name)
            retain(source, pin)
            copy(source, output / 'sources' / f'{index:03d}-{source.name}', pin)
        verifier_pin = build.digest(binary)
        require(verifier_pin['sha256'] == expected_sha, 'native verifier SHA differs')
        verifier = output / 'verifier'
        copy(binary, verifier, verifier_pin, True)
        keep(output / 'input-pins.json', {'files': dict(pins), 'sourceBinding': expected_source, 'productionReady': False})

        def guard():
            build.guard_files(pins)
            require(source_binding() == expected_source and verifier.stat().st_mode & 0o777 == 0o555,
                    'crash source/executable mode changed')

        plan = context['scenarios']['repair']
        run = output / 'execution'
        command = [str(verifier), 'rehearse-maintenance-crashes', '--manifest', owned[plan['manifestRole']],
                   '--output', str(run / 'replay'), '--require-prior-incoming-evidence']
        # The baseline, every killed importer and terminal restore each receive a
        # full day. This outer budget never silently shortens a native deadline.
        timeout = (len(plan['steps']) + 3) * DAY
        raw, execution = build.process(command, output, run, guard, timeout=timeout, maximum=MAX_LOG, retain=retain)
        for name, pin in tree(run).items():
            retain(Path(name), pin)
        result = history.parse(raw)
        verification = validate(result, {**context, 'inputPaths': owned}, binding, count,
                                run / 'replay', pins, expected_sha)
        same(history.parse(build.read(run / 'execution.json', pins[str(run / 'execution.json')])), execution,
             'actual process receipt changed')
        require(execution['command'] == command and execution['cwd'] == str(output)
            and type(execution['pid']) is int and execution['pid'] > 0 and type(execution['exitCode']) is int
            and execution['exitCode'] == 0 and execution['stopReason'] is None and execution['guardError'] is None
            and execution['cleanupErrors'] == [], 'actual crash rehearsal process did not pass')
        completion = {'schema': 'sora-nexus-maintenance-crash-release-v1', **verification,
            'verifierSha256': expected_sha, 'sourceBinding': expected_source, 'timeoutSeconds': timeout,
            'executionReceipt': pins[str(run / 'execution.json')]}
        keep(output / 'completion.json', completion)
        # Stage every addition and all late digests before the final guard. After
        # that guard registration consists solely of in-memory assignments.
        generated = tree(output)
        require(generated == {name: pin for name, pin in pins.items() if Path(name).is_relative_to(output)},
                'unbound generated crash evidence')
        additions, added_paths = [], {}
        require(len(report['artifacts']) + len(generated) <= MAX_FILES, 'package artifact count bound')
        for index, (name, pin) in enumerate(sorted(generated.items())):
            path = Path(name); role = f'maintenanceCrashOutput-{index:06d}'
            require(role not in paths, 'reserved crash artifact role collision')
            additions.append({'role': role, **pin, 'object': 'objects/' + pin['sha256'],
                'sourcePath': 'generated-maintenance-crashes/' + str(path.relative_to(output)),
                'executable': bool(path.stat().st_mode & 0o111)})
            added_paths[role] = path
        gate = {'status': 'verified', 'reason': 'Fresh exact-Wasm baseline and opaque incoming evidence, then actual SIGKILL after each fsynced committed snapshot and next-child continuation; database-commit interruption and consensus are separate',
            **verification, 'verifierSha256': expected_sha, 'sourceBinding': expected_source,
            'completionSha256': pins[str(output / 'completion.json')]['sha256']}
        guard()
        report['artifacts'].extend(additions)
        paths.update(added_paths)
        report['gates'][GATE] = gate
        report['maintenanceCrashOutputDirectory'] = str(output)
        report['productionReady'] = False
    except BaseException as error:
        build.write(output / 'failure.json', {'schema': 'sora-nexus-maintenance-crash-release-failure-v1',
            'error': str(error), 'coverageScope': SCOPE, 'productionReady': False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--print-source-binding', action='store_true', required=True)
    parser.parse_args()
    print(source_binding())


if __name__ == '__main__':
    main()
