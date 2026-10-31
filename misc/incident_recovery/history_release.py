"""Fresh retained-history replay for one narrowly scoped SORA Nexus release gate.

Archived reports are never accepted. The caller supplies an independently pinned
native verifier; this hook does not establish its build provenance or finality.
"""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import tempfile
import time

import hashi_codec as scale

ROLE = 'historicalReplayManifest'
GATE = 'retainedHistoryStateRootReplay'
ROOT = '0x79bda41a1db53470463653f358890f2d1fca8ad622c33a1d7095294b0b140452'
BOUNDARIES = {'session', 'epoch', 'upgrade', 'checkpoint'}
MAX_OUTPUT = 4 * 1024**2
MAX_GRAPH = 16 * 1024**3


def require(ok, reason):
    scale.require(ok, 'Historical replay: ' + reason)


def parse(raw):
    def pairs(values):
        result = {}
        for key, value in values:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def invalid(_):
        raise ValueError('Historical replay: nonfinite JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=invalid)


def read(path, maximum=64 * 1024**2):
    require(path.stat().st_size <= maximum, 'JSON size bound')
    with path.open('rb') as stream:
        raw = stream.read(maximum + 1)
    require(len(raw) <= maximum, 'JSON grew beyond bound')
    return parse(raw)


def digest_hex(value, prefix=False):
    require(isinstance(value, str) and re.fullmatch(('0x' if prefix else '') + '[0-9a-f]{64}', value), 'canonical digest required')
    return bytes.fromhex(value[2:] if prefix else value)


def header(value):
    require(isinstance(value, dict) and set(value) == {'parentHash', 'number', 'stateRoot', 'extrinsicsRoot', 'digest'}, 'sealed header fields')
    number = value['number']
    require(isinstance(number, str) and re.fullmatch('0x(?:0|[1-9a-f][0-9a-f]{0,7})', number), 'header number')
    number = int(number, 16)
    digest = value['digest']
    require(isinstance(digest, dict) and set(digest) == {'logs'} and isinstance(digest['logs'], list) and len(digest['logs']) <= 1024, 'header digest bounds')
    encoded = digest_hex(value['parentHash'], True) + scale.compact(number) + digest_hex(value['stateRoot'], True) + digest_hex(value['extrinsicsRoot'], True) + scale.compact(len(digest['logs']))
    for log in digest['logs']:
        require(isinstance(log, str) and len(log) <= 2 * 1024**2 and re.fullmatch('0x(?:[0-9a-f]{2})+', log), 'header digest encoding')
        encoded += bytes.fromhex(log[2:])
    return {'number': number, 'hash': '0x' + hashlib.blake2b(encoded, digest_size=32).hexdigest(), 'parentHash': value['parentHash'], 'stateRoot': value['stateRoot']}


def boundary_set(value):
    require(isinstance(value, list) and len(value) == len(set(value)) and set(value) <= BOUNDARIES, 'boundary set')
    return set(value)


def bind(root, records, paths, local_file, digest_file):
    if ROLE not in paths:
        return None
    required = {'bootstrapWasm', 'bootstrapBinding', 'bootstrapProvenance', 'policyScale', 'operatorsScale', 'holdWasm', 'finalWasm', 'chainSpec'}
    require(required <= paths.keys(), 'complete bootstrap release bindings required')
    provenance = read(paths['bootstrapProvenance'])
    require(provenance.get('fixtureOnly') is not True and 'fixture' not in str(provenance.get('schema', '')).lower(), 'public fixture provenance is not release input')
    # The packager has already checked exact policy/operator/destination hashes,
    # preserved source identity, spec134/tx131 and the chain's substitute bytes.
    source = paths[ROLE]
    manifest = read(source)
    require(isinstance(manifest, dict) and set(manifest) == {'schema', 'checkpointHash', 'candidate', 'ancestry', 'ranges'}, 'manifest fields')
    require(manifest['schema'] == 'sora-nexus-historical-replay-v1' and manifest['checkpointHash'] == scale.CHECKPOINT_HASH, 'manifest identity')
    indexed = {path: role for role, path in paths.items()}
    roles = {ROLE, *required}
    aggregate = 0
    counted = set()

    def add(path, expected=None, size=None):
        nonlocal aggregate
        actual = digest_file(path)
        if expected is not None:
            digest_hex(expected)
            require(actual['sha256'] == expected, 'referenced input hash mismatch')
        if size is not None:
            require(type(size) is int and actual['bytes'] == size, 'referenced input size mismatch')
        if path not in counted:
            aggregate += actual['bytes']
            counted.add(path)
        require(aggregate <= MAX_GRAPH and len(records) < 32768, 'recursive graph bound')
        if path not in indexed:
            role = f'historicalReplayInput-{len(indexed):05d}'
            require(role not in records, 'reserved history input role collision')
            records[role] = {'role': role, **actual, 'object': 'objects/' + actual['sha256'], 'sourcePath': path.relative_to(root).as_posix(), 'executable': False}
            paths[role] = path
            indexed[path] = role
        role = indexed[path]
        require(all(records[role][key] == actual[key] for key in actual), 'existing input pin differs')
        roles.add(role)
        return path

    def ref(value, maximum):
        require(isinstance(value, dict) and set(value) == {'file', 'sha256'}, 'artifact reference fields')
        path = add(local_file(source.parent, value['file']), value['sha256'])
        require(path.stat().st_size <= maximum, 'artifact size bound')
        return path

    candidate = ref(manifest['candidate'], 32 * 1024**2)
    require(digest_file(candidate) == digest_file(paths['bootstrapWasm']), 'candidate is not the packaged bootstrap')
    ancestry = read(ref(manifest['ancestry'], 256 * 1024**2), 256 * 1024**2)
    require(isinstance(ancestry, list) and 1 < len(ancestry) <= 1_000_000, 'ancestry bounds')
    headers = {}
    previous = None
    for value in ancestry:
        current = header(value)
        if previous is not None:
            require(current['number'] == previous['number'] + 1 and current['parentHash'] == previous['hash'], 'ancestry is not contiguous')
        headers[current['number']] = current
        previous = current
    require(previous['number'] == scale.CHECKPOINT and previous['hash'] == scale.CHECKPOINT_HASH and previous['stateRoot'] == ROOT, 'ancestry checkpoint differs')
    ranges = manifest['ranges']
    require(isinstance(ranges, list) and 0 < len(ranges) <= 128, 'range bounds')
    expected = []
    names = set()
    declared = set()
    block_count = 0
    for row in ranges:
        require(isinstance(row, dict) and set(row) == {'name', 'rationale', 'boundaries', 'parentCapture', 'blocks'}, 'range fields')
        require(all(isinstance(row[k], str) and 0 < len(row[k]) <= 4096 for k in ['name', 'rationale', 'parentCapture']) and row['name'] not in names, 'range name/rationale')
        names.add(row['name'])
        boundaries = boundary_set(row['boundaries'])
        require(boundaries, 'range has no declared boundary')
        declared |= boundaries
        capture_path = add(local_file(source.parent, row['parentCapture'] + '/capture-complete.json'))
        capture = read(capture_path)
        require(capture.get('schema') == 'sora-nexus-checkpoint-capture-v1' and capture.get('complete') is True, 'incomplete parent capture')
        point = capture.get('checkpoint', {})
        require(type(point.get('number')) is int and point['number'] in headers, 'parent outside anchored ancestry')
        parent = headers[point['number']]
        require(all(point.get(key) == parent[key] for key in ['hash', 'parentHash', 'stateRoot']), 'capture parent identity differs')
        references = dict(capture.get('artifacts', {}))
        require(references and isinstance(capture.get('top'), dict) and isinstance(capture.get('children'), list), 'capture file inventory')
        for trie in [capture['top'], *capture['children']]:
            require(isinstance(trie, dict) and isinstance(trie.get('pages'), list) and type(trie.get('pageCount')) is int and trie['pageCount'] == len(trie['pages']), 'capture page inventory')
            for page in trie['pages']:
                require(isinstance(page, dict) and isinstance(page.get('file'), str) and page['file'] not in references, 'duplicate capture reference')
                references[page['file']] = {'sha256': page.get('sha256')}
        for relative, pin in references.items():
            require(isinstance(pin, dict) and isinstance(pin.get('sha256'), str), 'capture reference pin')
            add(local_file(capture_path.parent, relative), pin['sha256'], pin.get('bytes'))
        blocks = row['blocks']
        require(isinstance(blocks, list) and 0 < len(blocks) <= 4096, 'range block bounds')
        block_count += len(blocks)
        require(block_count <= 4096, 'total block bound')
        expected_blocks = []
        for block in blocks:
            raw = read(ref(block, 64 * 1024**2))
            require(isinstance(raw, dict) and isinstance(raw.get('block'), dict), 'RPC block required')
            current = header(raw['block'].get('header'))
            require(current == headers.get(current['number']) and current['number'] == parent['number'] + 1 and current['parentHash'] == parent['hash'], 'range block not on anchored parent path')
            expected_blocks.append({'number': current['number'], 'hash': current['hash'], 'parentHash': current['parentHash'], 'parentStateRoot': parent['stateRoot'], 'postStateRoot': current['stateRoot']})
            parent = current
        expected.append({'name': row['name'], 'rationale': row['rationale'], 'parentCaptureManifestSha256': digest_file(capture_path)['sha256'], 'declaredBoundaries': sorted(boundaries), 'blocks': expected_blocks})
    require(declared == BOUNDARIES, 'representative session/epoch/upgrade/checkpoint ranges required')
    for role in list(roles):
        add(paths[role])
    return {'manifestSha256': records[ROLE]['sha256'], 'candidateSha256': records['bootstrapWasm']['sha256'], 'candidateBlake2': '0x' + records['bootstrapWasm']['blake2_256'], 'ancestrySha256': manifest['ancestry']['sha256'], 'inputRoles': sorted(roles), 'ranges': expected, 'productionReady': False}


def validate(result, context):
    require(isinstance(result, dict) and set(result) == {'schema', 'checkpointHash', 'manifestSha256', 'candidateSha256', 'candidateBlake2', 'ancestrySha256', 'ranges', 'representativeBoundaryCoverage', 'consensusSealsVerified', 'productionReady'}, 'report fields')
    require(result['schema'] == 'sora-nexus-historical-replay-witness-v1' and result['checkpointHash'] == scale.CHECKPOINT_HASH, 'report identity')
    require(all(result[key] == context[key] for key in ['manifestSha256', 'candidateSha256', 'candidateBlake2', 'ancestrySha256']), 'report artifact identity')
    require(result['representativeBoundaryCoverage'] is True and result['consensusSealsVerified'] is False and result['productionReady'] is False, 'report scope/coverage')
    require(isinstance(result['ranges'], list) and len(result['ranges']) == len(context['ranges']), 'report range count')
    observed = set()
    for row, expected in zip(result['ranges'], context['ranges']):
        require(isinstance(row, dict) and set(row) == {'name', 'rationale', 'parentCaptureManifestSha256', 'boundaries', 'blocks'}, 'report range fields')
        require(all(row[key] == expected[key] for key in ['name', 'rationale', 'parentCaptureManifestSha256']), 'report range identity')
        require(isinstance(row['blocks'], list) and len(row['blocks']) == len(expected['blocks']), 'report block count')
        boundary_union = set()
        substituted = False
        for block, wanted in zip(row['blocks'], expected['blocks']):
            require(isinstance(block, dict) and set(block) == set(wanted) | {'deployedExecutorHash', 'candidateExecutorHash', 'deployedSpec', 'substituteApplied', 'boundaries', 'identicalStateRoots', 'nativeExecution'}, 'report block fields')
            require(all(block[key] == value for key, value in wanted.items()) and type(block['number']) is int, 'report block/header state-root mismatch')
            require(block['identicalStateRoots'] is True and block['nativeExecution'] is False, 'report execution mode/roots')
            spec = block['deployedSpec']
            require(type(spec) is int and 0 <= spec <= 134 and spec not in [57, 115] and type(block['substituteApplied']) is bool and block['substituteApplied'] == (spec == 134), 'report substitute selection')
            digest_hex(block['deployedExecutorHash'], True)
            digest_hex(block['candidateExecutorHash'], True)
            require(block['candidateExecutorHash'] == (context['candidateBlake2'] if spec == 134 else block['deployedExecutorHash']), 'report executing code mismatch')
            boundaries = boundary_set(block['boundaries'])
            require(('checkpoint' in boundaries) == (block['number'] == scale.CHECKPOINT), 'checkpoint boundary mismatch')
            boundary_union |= boundaries
            substituted |= block['substituteApplied']
        require(substituted and boundary_set(row['boundaries']) == boundary_union and set(expected['declaredBoundaries']) <= boundary_union, 'declared boundary not observed')
        observed |= boundary_union
    require(observed == BOUNDARIES, 'incomplete observed boundary coverage')


def write(path, value):
    raw = (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()
    with path.open('xb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    return {'sha256': hashlib.sha256(raw).hexdigest(), 'blake2_256': hashlib.blake2b(raw, digest_size=32).hexdigest(), 'bytes': len(raw)}


def execute(binary, manifest, directory, expected_sha256, digest_file):
    require(binary.is_file() and not binary.is_symlink() and digest_file(binary)['sha256'] == expected_sha256, 'explicit verifier pin differs')
    target = directory / 'verifier'
    shutil.copyfile(binary, target)
    target.chmod(0o555)
    require(target.stat().st_nlink == 1 and digest_file(target)['sha256'] == expected_sha256, 'independent verifier copy differs')
    command = [str(target), 'replay-history', '--manifest', str(manifest)]
    start = time.monotonic()
    initial = {'schema': 'sora-nexus-package-historical-replay-process-v1', 'command': command, 'cwd': str(directory), 'startedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'verifierSha256': expected_sha256, 'productionReady': False}
    retained = {'launch.json': write(directory / 'launch.json', initial)}
    process = None
    reason = None
    with (directory / 'stdout.json').open('xb') as stdout, (directory / 'stderr.log').open('xb') as stderr:
        try:
            # Do not inherit compiler, loader, proxy, credential or runtime overrides.
            process = subprocess.Popen(command, cwd=directory, env={'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, start_new_session=True)
            retained['started.json'] = write(directory / 'started.json', {**initial, 'pid': process.pid})
            while process.poll() is None:
                if time.monotonic() - start > 10800:
                    reason = 'timeout'
                    break
                if os.fstat(stdout.fileno()).st_size > MAX_OUTPUT or os.fstat(stderr.fileno()).st_size > MAX_OUTPUT:
                    reason = 'output bound'
                    break
                time.sleep(0.05)
        except BaseException as error:
            reason = 'process exception: ' + str(error)
        finally:
            if process is not None:
                if process.poll() is None:
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                process.wait()
    receipt = {**initial, 'pid': None if process is None else process.pid, 'exitCode': None if process is None else process.returncode, 'stopReason': reason, 'elapsedSeconds': time.monotonic() - start, 'finishedAt': datetime.datetime.now(datetime.timezone.utc).isoformat()}
    receipt['outputs'] = {name: digest_file(directory / name) for name in ['stdout.json', 'stderr.log']}
    try:
        receipt['verifierAfter'] = digest_file(target)
    except OSError as error:
        receipt['verifierAfterError'] = str(error)
    retained['execution.json'] = write(directory / 'execution.json', receipt)
    require(reason is None and type(receipt['pid']) is int and receipt['pid'] > 0 and receipt['exitCode'] == 0, 'native process failed; retained execution receipt')
    require(receipt.get('verifierAfter', {}).get('sha256') == expected_sha256 and target.stat().st_nlink == 1 and target.stat().st_mode & 0o777 == 0o555 and not target.is_symlink(), 'verifier changed during replay')
    require(all(pin['bytes'] <= MAX_OUTPUT for pin in receipt['outputs'].values()), 'native output bound')
    retained.update(receipt['outputs'])
    retained['verifier'] = receipt['verifierAfter']
    with (directory / 'stdout.json').open('rb') as stream:
        raw = stream.read(MAX_OUTPUT + 1)
    require(len(raw) <= MAX_OUTPUT and hashlib.sha256(raw).hexdigest() == retained['stdout.json']['sha256'], 'stdout changed before validation')
    return parse(raw), retained


def run(report, paths, binary, expected_sha256, parent, local_file, digest_file):
    context = report.get('historicalReplay')
    if context is None:
        return
    parent.mkdir(parents=True, exist_ok=True)
    directory = Path(tempfile.mkdtemp(prefix='.sora-nexus-history-replay-', dir=parent)).resolve()
    report['historicalReplayOutputDirectory'] = str(directory)
    pins = {row['role']: row for row in report['artifacts']}
    owned = directory / 'inputs'
    owned.mkdir()
    sources = [Path(__file__).resolve(), Path(scale.__file__).resolve(), Path(__file__).with_name('release_package.py').resolve()]
    source_pins = {str(path): digest_file(path) for path in sources}
    originals = {}
    copies = {}
    for role in context['inputRoles']:
        path, pin = paths[role], pins[role]
        expected = {key: pin[key] for key in ['sha256', 'blake2_256', 'bytes']}
        require(not path.is_symlink() and path.resolve(strict=True) == path and digest_file(path) == expected, 'bound input changed before replay')
        target = owned / pin['sourcePath']
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            with path.open('rb') as source, target.open('xb') as output:
                shutil.copyfileobj(source, output)
            target.chmod(0o444)
        require(target.stat().st_nlink == 1 and digest_file(target) == expected, 'owned input copy differs')
        originals[str(path)] = expected
        copies[target.relative_to(directory).as_posix()] = expected
    source_dir = directory / 'verifier-sources'
    source_dir.mkdir()
    for path in sources:
        target = source_dir / path.name
        shutil.copyfile(path, target)
        target.chmod(0o444)
        require(digest_file(target) == source_pins[str(path)], 'source copy changed')
        copies[target.relative_to(directory).as_posix()] = source_pins[str(path)]
    retained = {'input-pins.json': write(directory / 'input-pins.json', {'originals': originals, 'owned': copies, 'verifierSources': source_pins, 'productionReady': False})}
    failure = None
    try:
        result, process_pins = execute(binary, local_file(owned, pins[ROLE]['sourcePath']), directory, expected_sha256, digest_file)
        retained.update(process_pins)
        validate(result, context)
    except BaseException as error:
        failure = error
    changes = []
    for group in [originals, source_pins, {str(directory / name): pin for name, pin in copies.items()}]:
        for name, expected in group.items():
            path = Path(name)
            try:
                if path.is_symlink() or path.resolve(strict=True) != path or digest_file(path) != expected:
                    changes.append(name)
            except OSError:
                changes.append(name)
    retained['guard.json'] = write(directory / 'guard.json', {'schema': 'sora-nexus-package-historical-replay-guard-v1', 'inputChanges': sorted(set(changes)), 'verificationError': None if failure is None else str(failure), 'productionReady': False})
    require(not changes, 'inputs/source changed during replay; retained guard receipt')
    if failure is not None:
        raise failure
    retained.update(copies)
    entries = list(directory.rglob('*'))
    require(all(not path.is_symlink() and path.resolve(strict=True) == path and (path.is_dir() or path.is_file()) for path in entries), 'generated directory alias')
    files = sorted(path for path in entries if path.is_file())
    require({path.relative_to(directory).as_posix() for path in files} == set(retained), 'unexpected generated file')
    require(len(files) <= 32768 and len(files) + len(report['artifacts']) <= 65536, 'output inventory bound')
    aggregate = 0
    additions = []
    added_paths = {}
    for index, path in enumerate(files):
        require(path.resolve(strict=True) == path and not path.is_symlink() and stat.S_ISREG(path.stat().st_mode) and path.stat().st_nlink == 1, 'output alias')
        actual = digest_file(path)
        require(actual == retained[path.relative_to(directory).as_posix()], 'generated artifact changed before binding')
        aggregate += actual['bytes']
        require(aggregate <= MAX_GRAPH + 2 * 1024**3, 'output aggregate bound')
        role = f'historicalReplayOutput-{index:05d}'
        require(role not in paths, 'reserved history output collision')
        additions.append({'role': role, **actual, 'object': 'objects/' + actual['sha256'], 'sourcePath': 'generated-historical-replay/' + path.relative_to(directory).as_posix(), 'executable': path == directory / 'verifier'})
        added_paths[role] = path
    for name, expected in {**originals, **source_pins, **{str(directory / name): pin for name, pin in retained.items()}}.items():
        path = Path(name)
        require(not path.is_symlink() and path.resolve(strict=True) == path and digest_file(path) == expected, 'artifact changed while registering outputs')
    report['artifacts'].extend(additions)
    paths.update(added_paths)
    report['gates'][GATE] = {'status': 'verified', 'reason': 'Fresh pinned Wasm historical replay matched anchored roots across observed session, epoch, upgrade and checkpoint boundaries; consensus seals/finality are not verified', 'manifestSha256': context['manifestSha256'], 'verifierSha256': expected_sha256, 'executionReceiptSha256': digest_file(directory / 'execution.json')['sha256'], 'guardReceiptSha256': digest_file(directory / 'guard.json')['sha256']}
    report['productionReady'] = False
