#!/usr/bin/env python3
"""Build isolated SORA Nexus recovery artifacts from reviewed public inputs.

No signing, network service, publication, restart or readiness promotion. The
native compiler and Cargo are executed afresh; archived success JSON is never an
input. This is a build corridor, not evidence of consensus/external reconciliation.
"""
from __future__ import annotations
import argparse
import datetime
import hashlib
import json
import importlib.util
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import tomllib
from uuid import uuid4

sys.dont_write_bytecode = True

import hashi_codec as scale
import release_package as release
from incoming_replay_release import PUBLIC_COUNCIL
from bootstrap import prepare as bootstrap
import build_fixture_wasm as build_helpers
import verify_abandoned_consensus as abandoned

ROOT = bootstrap.ROOT
SCHEMA = 'sora-nexus-production-recovery-build-input-v2'
SOURCE_PATHS = bootstrap.SOURCE_PATHS
MAX_GRAPH = 64 * 1024**3
MAX_FILE = 256 * 1024**2
MAX_LOG = 128 * 1024**2

# Exact known public test keys, never a heuristic over real accounts. The
# operator list is retained operators-v2/operators.json SHA256
# 199db14151c2335b343444aa81a7b681e7300695ba217861f926e10f75769cc3.
PUBLIC_REPLACEMENTS = {'ed51c2f39620ba2de7cf1321f02d5c9a0706943aa7a796c3f8aeb94e89cba45e'}
PUBLIC_GRANDPA = {
    'daa1e4e1ab183d05bde3efa79d53959cd4d5a228f5eab986a0550ab37c365bbe',
    'faf70d250137292cbc70e9a164df33a7a5668a14cace1fcf87e36a0b22b430d6',
    '3481cc0733d0168977a4ad536d10fe983f0376044df5d1a69479db239ef4ab53',
    '08bae1e0850588925daa473517d1fe8ec7c03525276f47ec87076f0fbd5a6bd6',
    'd4d7aedcb8137d9ffe63600a359570c845ef59fc23979a2471523a3b0eacf036',
    '826ea5a12369c61146597cdf31e2763f6b6dd0b7bfcb5e4a63046a679e82fef8',
    'bc2093a26473dc734de2cd4e367f1dc8f75ca8c403300235deb6e309171f545c',
    '69be9cfef075fe28f93ca949f3dd59bcf62da41cbf98ecdcc2402841798757e4',
    'dc2f976b85b7e6e9b5fac7451eec4dc4e9e0a8f220e062d94d4aad06d9f2ae52',
    '294bb1423ae119c7d6bccdf4ebe385db87079cffcbfb0113d9c20448dc6eab19',
    '544bf542ffd851f2cd028b71a457cfe6221cc33792b4ce13628aa004e4c0bb22',
    '3763cad6ef417c2d73581e8592c4c94340dbd5fc12b3de2da70fbf00b10229f0',
    '540ce4402662d8c02b0453019ca0555a84fab9baa5b95b1a9fd3f165fdfb2549',
    'e2687f28d37c05f8514e20acefd66dfea435ca71ba3e68db301c0ec9d67919da',
    'd8da0e13eb798a46c7fde1c388dbf472bcee50e8b2a3e73599229b0935b906db',
}
PUBLIC_SIGNING_KEYS = PUBLIC_GRANDPA | PUBLIC_REPLACEMENTS | {key.hex() for key in PUBLIC_COUNCIL}

REQUIRED_COMPOSE = {'capture', 'csv', 'inventory', 'economic', 'semantic'}
OPTIONAL_COMPOSE = {'bridge-history', 'bridge-signing', 'bridge-hashi', 'elections', 'offences',
    'governance', 'referral', 'staking-controls', 'staking-roles', 'staking-history', 'democracy', 'scheduler'}


def require(value, message):
    if not value:
        raise ValueError(message)


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def duplicate_free(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result


def parse(raw):
    return json.loads(raw, object_pairs_hook=duplicate_free,
        parse_constant=lambda value: (_ for _ in ()).throw(ValueError('non-finite JSON')))


def serialized(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()


def pin_bytes(raw):
    return {'sha256': hashlib.sha256(raw).hexdigest(),
        'blake2_256': hashlib.blake2b(raw, digest_size=32).hexdigest(), 'bytes': len(raw)}


def write(path, value):
    raw = serialized(value) if not isinstance(value, bytes) else value
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    return pin_bytes(raw)


def digest(path):
    require(path.is_file() and not path.is_symlink() and path.resolve(strict=True) == path
        and stat.S_ISREG(path.stat().st_mode) and path.stat().st_nlink == 1, 'file alias: ' + str(path))
    return release.digest_file(path)


def read(path, expected=None, maximum=MAX_FILE):
    require(path.stat().st_size <= maximum, 'file byte bound: ' + str(path))
    before = digest(path)
    raw = path.read_bytes()
    require(pin_bytes(raw) == before and len(raw) <= maximum, 'file changed while reading')
    if expected is not None:
        require((before['sha256'] == expected) if isinstance(expected, str) else before == expected,
            'file pin differs: ' + str(path))
    return raw


def relative(name):
    require(isinstance(name, str) and name and '\\' not in name, 'relative path required')
    path = PurePosixPath(name)
    require(not path.is_absolute() and str(path) == name and all(p not in ('.', '..') for p in path.parts), 'unsafe relative path')
    require(not any(p.lower() in ('fixtures', '.git', 'target', 'node_modules') for p in path.parts), 'fixture/build path is not a production input')
    require(not any(term in name.lower() for term in ('sydneycreds', 'private-key', 'secret-key', 'credentials', 'keystore')), 'private/credential material is not a build input')
    return name


def local(root, name):
    relative(name)
    path = release.local_file(root, name)
    require(path.resolve(strict=True) == path, 'aliased input path')
    return path


def no_fixture(value):
    if isinstance(value, dict):
        require(value.get('fixtureOnly') is not True, 'public fixture input rejected')
        require('fixture' not in str(value.get('schema', '')).lower(), 'fixture schema rejected')
        for item in value.values():
            no_fixture(item)
    elif isinstance(value, list):
        for item in value:
            no_fixture(item)


def check_parents(path):
    require(not any(part.lower() == 'fixtures' for part in path.parts), 'public fixture location rejected')
    for parent in (path.parent, *path.parents):
        require(not (parent / 'FIXTURE-PROVENANCE.json').exists()
            and not (parent / 'NEVER-DEPLOY.txt').exists(), 'public fixture marker rejected')


def public_consensus(value, inputs):
    """Recompute slot selection from the pinned public capture, never caller numbers."""
    config = value['consensus']
    require(isinstance(config, dict) and set(config) == {'publicCapture'}, 'consensus input fields')
    name = relative(config['publicCapture'])
    pins = {}
    for filename in ('started.json', 'headers.json', 'completion.json'):
        item = name + '/' + filename
        require(item in value['files'], 'public consensus capture missing from exact input graph')
        source = local(inputs, item)
        read(source, value['files'][item], 128 * 1024**2)
        pins[filename] = value['files'][item]['sha256']
    report = abandoned.verify_directory(inputs / name)
    require(report.get('schema') == 'sora-nexus-public-abandoned-consensus-v2'
        and report.get('captureSha256') == pins and report.get('checkpointHash') == scale.CHECKPOINT_HASH
        and report.get('finalityProofVerified') is False and report.get('clientVotingHistoryProven') is False
        and report.get('operatorSigningStopVerified') is False and report.get('productionReady') is False,
        'public consensus capture scope or hashes differ')
    fields = {'previousGrandpaSetId': report['setIds']['checkpoint'],
              'lastObservedAbandonedSlot': report['lastObservedAbandonedSlot'],
              'firstRecoverySlot': report['firstRecoverySlot']}
    require(type(fields['previousGrandpaSetId']) is int and fields['previousGrandpaSetId'] == 7956
        and report['setIds']['finalized'] == fields['previousGrandpaSetId'], 'checkpoint GRANDPA set differs')
    require(all(type(fields[k]) is int and 0 <= fields[k] < 2**64 for k in ('firstRecoverySlot', 'lastObservedAbandonedSlot'))
        and fields['firstRecoverySlot'] == fields['lastObservedAbandonedSlot'] + 1
        and fields['lastObservedAbandonedSlot'] == 298871044, 'public abandoned-chain slot differs from the runtime BABE backstop')
    return fields, report


def load_inputs(path, expected):
    check_parents(path)
    raw = read(path, expected, 16 * 1024**2)
    value = parse(raw)
    mandatory = {'schema', 'files', 'compose', 'mixedLayout', 'consensus', 'authorizationDomain'}
    require(mandatory <= set(value) <= mandatory | {'operatorsScale'}, 'unexpected build input fields')
    require(value['schema'] == SCHEMA, 'unknown production input schema')
    no_fixture(value)
    release.authorization_domain_scale(value['authorizationDomain'])
    require(value['authorizationDomain']['chainSpecId'] == 'sora-nexus-recovery-28191817', 'production authorization network differs')
    require(value['authorizationDomain']['releaseNonce'] != '0x' + '46' * 32, 'public fixture nonce rejected')
    files = value['files']
    require(isinstance(files, dict) and 0 < len(files) <= 32768, 'input inventory count bound')
    total = 0
    paths = {}
    for name, binding in files.items():
        require(set(binding) == {'sha256', 'blake2_256', 'bytes'} and type(binding['bytes']) is int
            and 0 <= binding['bytes'] <= MAX_FILE and re.fullmatch('[0-9a-f]{64}', binding['sha256'])
            and re.fullmatch('[0-9a-f]{64}', binding['blake2_256']), 'invalid input file pin')
        source = local(path.parent, name)
        data = read(source, binding)
        total += len(data)
        require(total <= MAX_GRAPH, 'input graph byte bound')
        if source.suffix == '.json':
            no_fixture(parse(data))
        require(b'PUBLIC-FIXTURE-ONLY' not in data and b'PUBLIC FIXTURE ONLY' not in data, 'public fixture bytes rejected')
        paths[name] = source
    compose = value['compose']
    require(isinstance(compose, dict) and REQUIRED_COMPOSE <= set(compose)
        and set(compose) <= REQUIRED_COMPOSE | OPTIONAL_COMPOSE, 'unsupported or missing compose flags')
    for flag, name in compose.items():
        relative(name)
        if flag == 'capture':
            require(name + '/capture-complete.json' in files, 'capture manifest missing from exact input graph')
        else:
            require(name in files, 'compose input absent from file inventory')
    for name in ('operatorsScale',):
        if name in value:
            require(value[name] in files, name + ' missing from input inventory')
    require('staking-roles' in compose, 'production build requires operator-associated staking-role review')
    if value['mixedLayout'] is not None:
        require(value['mixedLayout'] in files, 'mixed layout absent from input inventory')
    public_consensus(value, path.parent)
    if 'operatorsScale' in value:
        operators = release.operator_identity(read(paths[value['operatorsScale']]))
        require(not PUBLIC_SIGNING_KEYS.intersection(operators['approvedGrandpa']), 'public test GRANDPA keys rejected')
    require(not PUBLIC_SIGNING_KEYS.intersection(new for _, new in release.mapping_identity(paths[compose['csv']])), 'public test replacement owner rejected')
    # This is a data commitment only. It contains no policy/Wasm/output hashes.
    return value, paths, pin_bytes(raw), pin_bytes(serialized(value))['blake2_256']


def source_names(root):
    raw = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z', *SOURCE_PATHS], cwd=root)
    names = sorted(set(filter(None, raw.decode().split('\0'))))
    require(names and 'Cargo.lock' in names and 'runtime/src/incident_recovery/generated.rs' in names, 'incomplete source inventory')
    return names


def source_inventory(root):
    result = {}
    for name in source_names(root):
        # Source tests may include fixture code; only the build INPUT graph is nonfixture.
        path = release.local_file(root, name)
        result[name] = digest(path)
    return result


def verify_inventory(path, expected_sha, root):
    expected = parse(read(path, expected_sha, 32 * 1024**2))
    require(isinstance(expected, dict) and expected == source_inventory(root), 'working source does not match pinned complete inventory')
    return expected


def guard_files(pins):
    for name, expected in pins.items():
        require(digest(Path(name)) == expected, 'bound file changed: ' + name)


def process(command, cwd, output, guard, env=None, timeout=3600, maximum=MAX_LOG, retain=None):
    """Retain an actual owned process receipt even for launch failure/timeout."""
    output.mkdir(parents=True, exist_ok=False)
    guard()
    start = time.monotonic()
    initial = {'schema': 'sora-nexus-production-build-process-v1', 'command': command,
        'cwd': str(cwd), 'startedAt': now(), 'productionReady': False}
    def keep(path, data):
        expected = write(path, data)
        if retain is not None:
            retain(path, expected)
        return expected
    keep(output / 'launch.json', initial)
    child = None
    reason = None
    cleanup_errors = []
    with (output / 'stdout.log').open('xb') as stdout, (output / 'stderr.log').open('xb') as stderr:
        try:
            child = subprocess.Popen(command, cwd=cwd,
                env=env or {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'},
                stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, start_new_session=True)
            keep(output / 'started.json', {**initial, 'pid': child.pid})
            while child.poll() is None:
                if time.monotonic() - start > timeout:
                    reason = 'timeout'
                    break
                if max(os.fstat(stdout.fileno()).st_size, os.fstat(stderr.fileno()).st_size) > maximum:
                    reason = 'output bound'
                    break
                time.sleep(0.05)
        except BaseException as error:
            reason = 'process exception: ' + str(error)
        finally:
            if child is not None:
                if child.poll() is None:
                    try:
                        os.killpg(child.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    except OSError as error:
                        cleanup_errors.append('group kill: ' + str(error))
                        try:
                            child.kill()
                        except OSError as fallback:
                            cleanup_errors.append('owned child kill: ' + str(fallback))
                try:
                    child.wait(timeout=120)
                except subprocess.TimeoutExpired:
                    cleanup_errors.append('owned process did not exit after termination')
    changes = None
    try:
        guard()
    except BaseException as error:
        changes = str(error)
    receipt = {**initial, 'pid': None if child is None else child.pid,
        'exitCode': None if child is None else child.returncode, 'stopReason': reason,
        'guardError': changes, 'cleanupErrors': cleanup_errors, 'finishedAt': now(), 'elapsedSeconds': time.monotonic() - start,
        'outputs': {name: digest(output / name) for name in ('stdout.log', 'stderr.log')}}
    if retain is not None:
        for name, expected in receipt['outputs'].items():
            retain(output / name, expected)
    keep(output / 'execution.json', receipt)
    require(reason is None and changes is None and not cleanup_errors and type(receipt['pid']) is int and receipt['pid'] > 0
        and receipt['exitCode'] == 0, 'fresh process failed; see retained execution receipt')
    require(all(row['bytes'] <= maximum for row in receipt['outputs'].values()), 'process output bound')
    return read(output / 'stdout.log', receipt['outputs']['stdout.log'], maximum), receipt


def clear_work_policy(raw):
    """Preserve exact format/checkpoint/mapping/council/evidence; clear hash slots."""
    release.policy_identity(raw)
    reader = scale.Reader(raw)
    reader.take(68)
    for width in (64, 32):
        count = reader.compact()
        reader.take(count * width)
    prefix_end = reader.offset
    for _ in range(2):
        reader.take(reader.compact() * 32)
    domain_start = reader.offset
    reader.take(reader.compact())
    reader.take(32)
    domain = raw[domain_start:reader.offset]
    evidence = reader.take(32)
    reader.take(32)
    reader.end()
    return raw[:prefix_end] + b'\0\0' + domain + evidence + bytes(32)


def remap_index(text, index, allowed):
    raw = release.bounded_hex(text, 2 * 1024**2)
    require(len(raw) >= 14 and raw[:9] == b'snxrec01\x01' and raw[9] in allowed, 'unexpected composed work envelope')
    return '0x' + (raw[:10] + scale.u32(index) + raw[14:]).hex()


def mixed_input(composed, layout):
    require(set(layout) in ({'schema', 'work', 'audits', 'terminalEnvelope'}, {'schema', 'work', 'audits', 'terminalEnvelope', 'terminalPreludeEnvelopes'})
        and layout['schema'] == 'sora-nexus-production-mixed-layout-v1', 'mixed layout fields')
    result = {'schema': 'sora-nexus-mixed-work-input-v1',
        'deriveNativeBalanceAudits': True,
        'policyTemplateScale': '0x' + clear_work_policy(release.bounded_hex(composed['policyScale'], release.MAX_POLICY)).hex()}
    for name, base_name, output_name, tags, inserted in (
        ('work', 'applyEnvelopes', 'workEnvelopes', {1}, {6, 7}),
        ('audits', 'auditEnvelopes', 'auditEnvelopes', {2, 8}, {9})):
        rows, base, seen, values = layout[name], composed[base_name], [], []
        bridge_work_seen = False
        require(isinstance(rows, list) and 0 < len(rows) <= 16384, 'mixed work bound')
        for index, row in enumerate(rows):
            require(isinstance(row, dict) and set(row) in ({'composed'}, {'envelope'}), 'mixed entry fields')
            if 'composed' in row:
                require(name != 'work' or not bridge_work_seen,
                    'all composed storage edits must precede typed bridge work')
                prior = row['composed']
                require(type(prior) is int and 0 <= prior < len(base), 'composed entry index')
                seen.append(prior)
                values.append(remap_index(base[prior], index, tags))
            else:
                raw = release.bounded_hex(row['envelope'], 2 * 1024**2)
                require(len(raw) > 10 and raw[:9] == b'snxrec01\x01' and raw[9] in inserted, 'only typed bridge operations may supplement composed work')
                bridge_work_seen = name == 'work'
                values.append(row['envelope'])
        require(seen == list(range(len(base))), 'all composed work must remain exactly once in original order')
        result[output_name] = values
    # A changed terminal must be committed in raw input and proved by exact replay.
    result['terminalEnvelope'] = layout['terminalEnvelope'] or composed['terminalEnvelope']
    prelude = layout.get('terminalPreludeEnvelopes', composed.get('terminalPreludeEnvelopes', []))
    require(isinstance(prelude, list) and len(prelude) < 1024, 'terminal page prelude bound')
    result['terminalPreludeEnvelopes'] = prelude
    for index, value in enumerate(prelude):
        raw = release.bounded_hex(value, 2 * 1024**2)
        require(raw[:10] == b'snxrec01\x01\x0b' and len(raw) >= 14 and int.from_bytes(raw[10:14], 'little') == index, 'terminal prelude page identity/order')
    raw = release.bounded_hex(result['terminalEnvelope'], 2 * 1024**2)
    require(raw[:10] == (b'snxrec01\x01\x0b' if prelude else b'snxrec01\x01\x05'), 'terminal envelope required')
    if prelude:
        require(len(raw) >= 14 and int.from_bytes(raw[10:14], 'little') == len(prelude), 'terminal final page order')
    return result


def validate_native_balance_derivation(mixed, report):
    """Fresh native compiler may wrap an exact baseline, never replace its values."""
    require(mixed.get('deriveNativeBalanceAudits') is True
        and report.get('nativeBalanceAuditsDerived') is True
        and report.get('baselineAuditEnvelopes') == mixed['auditEnvelopes'],
        'native balance derivation lost original audit baseline')
    indices = report.get('nativeBalanceAuditIndices')
    require(isinstance(indices, list) and all(type(i) is int for i in indices)
        and indices == sorted(set(indices))
        and all(0 <= i < len(mixed['auditEnvelopes']) for i in indices),
        'native balance audit derivation indices differ')
    actual = report.get('auditEnvelopes')
    require(isinstance(actual, list) and len(actual) == len(mixed['auditEnvelopes']),
        'native balance derivation changed audit count')
    for index, (before, after) in enumerate(zip(mixed['auditEnvelopes'], actual)):
        if index not in indices:
            require(before == after, 'unlisted audit changed')
            continue
        baseline = release.bounded_hex(before, 2 * 1024**2)
        derived = release.bounded_hex(after, 2 * 1024**2)
        require(baseline[:10] == b'snxrec01\x01\x02'
            and derived[:10] == b'snxrec01\x01\x0a', 'native balance derivation action differs')
        reader = scale.Reader(derived[10:])
        payload = reader.vector(512 * 1024)
        reader.end()
        require(payload.startswith(baseline[10:]) and len(payload) > len(baseline[10:]),
            'native balance derivation changed exact baseline witness bytes')
        require(int.from_bytes(baseline[10:14], 'little') == index,
            'native balance derivation changed audit index')


def validate_emitted(raw, policy):
    text = raw.decode()
    match = re.search(r'pub const POLICY_SCALE: &\[u8\] = &\[([0-9, ]*)\];', text)
    require(match is not None, 'native final source lacks canonical policy')
    encoded = bytes(int(item.strip()) for item in match.group(1).split(',') if item.strip())
    require(encoded == policy, 'native emitter changed policy bytes')
    require('fixture' not in text.lower(), 'fixture-generated policy rejected')


def checkpoint_consensus_witness(consensus, expected, capture_sha256):
    """Bind freshly executed checkpoint derivation; this is not a finality proof."""
    require(consensus.get('schema') == 'sora-nexus-checkpoint-consensus-v2'
        and type(consensus.get('previousGrandpaSetId')) is int
        and consensus.get('previousGrandpaSetId') == expected['previousGrandpaSetId']
        and consensus.get('checkpointHash') == scale.CHECKPOINT_HASH
        and type(consensus.get('checkpointNumber')) is int and consensus['checkpointNumber'] == scale.CHECKPOINT
        and consensus.get('checkpointStateRoot') == release.CHECKPOINT_ROOT
        and consensus.get('captureManifestSha256') == capture_sha256
        and consensus.get('signingKeysPreserved') is True
        and consensus.get('manualOperatorApprovalRequired') is False
        and consensus.get('productionReady') is False
        and type(consensus.get('checkpointSlot')) is int
        and 0 < consensus['checkpointSlot'] <= expected['lastObservedAbandonedSlot']
        and consensus.get('grandpaAuthorities') == consensus.get('previousGrandpaAuthorities'), 'derived checkpoint consensus differs')
    metadata = consensus.get('checkpointMetadata', {})
    require(metadata.get('schema') == 'sora-nexus-checkpoint-metadata-witness-v1'
        and metadata.get('checkpointHash') == scale.CHECKPOINT_HASH and metadata.get('stateRoot') == release.CHECKPOINT_ROOT
        and metadata.get('wasmExecutionVerified') is True and metadata.get('nativeExecution') is False,
        'derived checkpoint metadata execution differs')
    operators = release.bounded_hex(consensus['operatorsScale'], release.MAX_POLICY)
    operator_info = release.operator_identity(operators)
    require(consensus.get('operatorsBlake2') == '0x' + hashlib.blake2b(operators, digest_size=32).hexdigest()
        and [release.grandpa_key(key).hex() for key in consensus['approvedGrandpaKeys']] == operator_info['approvedGrandpa']
        and not PUBLIC_SIGNING_KEYS.intersection(operator_info['approvedGrandpa']), 'derived checkpoint operator keys differ')
    rows = consensus['grandpaAuthorities']
    require(isinstance(rows, list) and 0 < len(rows) <= 1000
        and all(isinstance(row, list) and len(row) == 2 and type(row[1]) is int and 0 < row[1] < 2**64 for row in rows),
        'derived checkpoint authority list differs')
    active = [release.grandpa_key(row[0]).hex() for row in rows]
    require(len(active) == len(set(active)) and set(active).issubset(operator_info['approvedGrandpa']),
        'derived checkpoint active authorities differ')
    return operators


def compose_inputs(value, inputs, output, binary, guard, commitment, remember=write, retain=None, composition_limit=512 * 1024**2):
    require(type(composition_limit) is int and 128 * 1024**2 <= composition_limit <= 512 * 1024**2, 'composition output bound')
    capture = inputs / value['compose']['capture']
    expected, observed = public_consensus(value, inputs)
    remember(output / 'public-consensus.json', observed)
    consensus_raw, _ = process([str(binary), 'derive-checkpoint-consensus', '--capture', str(capture)],
        output, output / 'checkpoint-consensus', guard, retain=retain)
    consensus = parse(consensus_raw)
    operators = checkpoint_consensus_witness(consensus, expected, digest(capture / 'capture-complete.json')['sha256'])
    if 'operatorsScale' in value:
        require(read(inputs / value['operatorsScale']) == operators, 'supplied operator assertion differs from derived checkpoint')
    operators_path = output / 'checkpoint-operators.scale'
    remember(operators_path, operators)
    # CSV authorization is trusted; state/destination/derived-account checks are
    # executed afresh, without claiming replacement-key signature verification.
    args = [str(binary), 'verify-trusted-mapping', '--csv', str(inputs / value['compose']['csv']),
        '--capture', str(inputs / value['compose']['capture']), '--inventory', str(inputs / value['compose']['inventory']),
        '--semantic', str(inputs / value['compose']['semantic'])]
    mapping_raw, _ = process(args, output, output / 'trusted-mapping', guard, retain=retain)
    mapping_witness = parse(mapping_raw)
    council = consensus.get('checkpointCouncil', [])
    require(isinstance(council, list) and 0 < len(council) <= 100 and len(council) == len(set(council))
        and sorted(council) == mapping_witness.get('checkpointCouncil'),
        'checkpoint consensus/mapping council differs')
    args = [str(binary), 'compose-storage-work']
    for flag, name in sorted(value['compose'].items()):
        args += ['--' + flag, str(inputs / name)]
    args += ['--operators-scale', str(operators_path), '--evidence-commitment', '0x' + commitment,
             '--chain-spec-id', value['authorizationDomain']['chainSpecId'], '--release-nonce', value['authorizationDomain']['releaseNonce']]
    raw, _ = process(args, output, output / 'compose', guard, timeout=6*3600, maximum=composition_limit, retain=retain)
    composed = parse(raw)
    require(composed.get('storageEngineRehearsed') is True and composed.get('productionReady') is False,
        'compiler did not perform storage rehearsal')
    policy = release.bounded_hex(composed['policyScale'], release.MAX_POLICY)
    identity = release.policy_identity(policy)
    require(identity['authorizationDomain'] == value['authorizationDomain'], 'compiler authorization domain differs')
    require(not PUBLIC_SIGNING_KEYS.intersection(new for _, new in identity['mapping']), 'public test replacement policy rejected')
    policy_reader = scale.Reader(policy)
    policy_reader.take(68)
    policy_reader.take(policy_reader.compact() * 64)
    council = [policy_reader.take(32) for _ in range(policy_reader.compact())]
    require(not PUBLIC_SIGNING_KEYS.intersection(key.hex() for key in council), 'public test council policy rejected')
    require(composed.get('trustedMapping') == mapping_witness, 'fresh mapping expansion/council differs from composition')
    require(release.trusted_mapping_witness(mapping_witness, identity,
        {'signedMappingCsv': digest(inputs / value['compose']['csv'])['sha256'],
         'storageInventory': digest(inputs / value['compose']['inventory'])['sha256'],
         'semanticPlan': digest(inputs / value['compose']['semantic'])['sha256'],
         'checkpointCaptureManifest': digest(inputs / value['compose']['capture'] / 'capture-complete.json')['sha256']},
         release.mapping_identity(inputs / value['compose']['csv'])),
         'trusted CSV mapping/checkpoint council witness differs')
    require(identity['evidenceCommitment'] == commitment, 'compiled data commitment differs')
    if value['mixedLayout'] is not None:
        layout = parse(read(inputs / value['mixedLayout']))
        mixed = mixed_input(composed, layout)
        manifest = output / 'mixed-input.json'
        remember(manifest, mixed)
        raw, _ = process([str(binary), 'compile-mixed-work', '--manifest', str(manifest)], output, output / 'mixed', guard, maximum=composition_limit, retain=retain)
        report = parse(raw)
        require(report.get('structuralValidationOnly') is True and all(report.get(k) is False for k in
            ('authenticationVerified', 'economicCompletionVerified', 'storageEngineRehearsed', 'exactArtifactRehearsed', 'productionReady')), 'mixed compiler scope differs')
        require(all(report[k] == mixed[k] for k in ('workEnvelopes', 'terminalEnvelope', 'terminalPreludeEnvelopes')), 'mixed compiler reordered work')
        validate_native_balance_derivation(mixed, report)
        candidate = release.bounded_hex(report['policyScale'], release.MAX_POLICY)
        require(clear_work_policy(candidate) == clear_work_policy(policy), 'mixed compiler changed control/data policy')
        policy = candidate
    remember(output / 'policy.scale', policy)
    raw, _ = process([str(binary), 'emit-final-policy', '--policy', str(output / 'policy.scale')], output, output / 'emit-final', guard, retain=retain)
    validate_emitted(raw, policy)
    remember(output / 'generated.rs', raw)
    return policy, raw


def normalize_lock(source, reference, legacy):
    path = source / 'Cargo.lock'
    before, after = read(path), read(reference)
    if legacy:
        # Package identities stay unchanged. The old-SDK overlay may add only
        # runtime sp-trie/sp-externalities and the HASHI API's already locked
        # scale-info metadata dependency; the shared resolver records each edge.
        evidence = build_helpers.resolve_legacy_lock(source, reference, {'specVersion': 134})
    else:
        require(tomllib.loads(before.decode()) == tomllib.loads(after.decode()), 'final lock amendment changes dependencies')
        evidence = {'original': pin_bytes(before), 'resolved': pin_bytes(after), 'parsedTomlIdentical': True}
        if before != after:
            path.write_bytes(after)
    return {**evidence, 'kind': 'legacy-reviewed-direct-dependencies' if legacy else 'final-TOML-equivalent-format',
        'rootLockChanged': False}


def export_final(destination, inventory, generated, policy, commitment, guard, final_lock=None):
    require(not destination.exists(), 'final source destination must be new')
    guard()
    destination.mkdir(parents=True)
    for name, expected in inventory.items():
        raw = read(ROOT / name, expected)
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
        shutil.copymode(ROOT / name, target)
    lib = (destination / 'runtime/src/lib.rs').read_text()
    require(re.search(r'spec_version:\s*138,', lib) and re.search(r'transaction_version:\s*134,', lib), 'final source identity must be 138/134')
    target = destination / 'runtime/src/incident_recovery/generated.rs'
    target.write_bytes(generated)
    amendment = normalize_lock(destination, final_lock, False) if final_lock else None
    guard()
    provenance = {'schema': 'sora-nexus-production-final-source-v1', 'fixtureOnly': False,
        'productionReady': False, 'bound': True, 'specVersion': 138, 'transactionVersion': 134,
        'sourceFiles': inventory, 'generatedPolicy': pin_bytes(generated), 'policy': pin_bytes(policy),
        'dataCommitment': commitment, 'lockAmendment': amendment,
        'sourceFingerprint': fingerprint(destination)}
    write(destination / 'RECOVERY-PROVENANCE.json', provenance)
    return provenance


def fingerprint(source):
    result = {}
    for name in SOURCE_PATHS:
        path = source / name
        items = path.rglob('*') if path.is_dir() else [path]
        for item in items:
            require(not item.is_symlink(), 'source contains symlink')
            if item.is_file():
                relative_name = item.relative_to(source).as_posix()
                require(not any(p in ('target', 'node_modules', '.git') for p in Path(relative_name).parts), 'build data inside source graph')
                result[relative_name] = digest(item)
    require('Cargo.lock' in result and 'runtime/src/incident_recovery/generated.rs' in result, 'source fingerprint incomplete')
    return dict(sorted(result.items()))


def validate_source_provenance(source, provenance, actual):
    if provenance['specVersion'] == 138:
        require(actual == provenance.get('sourceFingerprint'), 'final export source changed before build')
    else:
        require(provenance.get('deployedSourceTree') == bootstrap.LEGACY_TREE
            and provenance.get('sdk') == 'polkadot-stable2603-3', 'legacy source identity differs')
        expected = provenance.get('sourceFiles')
        require(isinstance(expected, dict) and set(expected) == set(actual), 'complete legacy export inventory required')
        amendment = parse(read(source / 'PRODUCTION-LOCK-AMENDMENT.json'))
        require(amendment['kind'] == 'legacy-reviewed-direct-dependencies'
            and all(amendment['original'][key] == provenance['baseCargoLock'][key] for key in ('sha256', 'blake2_256'))
            and actual['Cargo.lock'] == amendment['resolved'], 'legacy lock amendment differs')
        for name, pin in expected.items():
            if name != 'Cargo.lock':
                require(all(actual[name][key] == pin[key] for key in ('sha256', 'blake2_256')), 'legacy export source changed before build: ' + name)
        for name, pin in provenance['overlay'].items():
            require(name in actual and all(actual[name][key] == pin[key] for key in ('sha256', 'blake2_256')), 'legacy overlay differs')


def copy_cargo_logs(output, receipt, remember=write):
    """Retain exact actual logs for the established repair release admission."""
    return {'cargo.' + name: remember(output / ('cargo.' + name),
        read(output / 'cargo' / name, receipt['outputs'][name], MAX_LOG))
        for name in ('stdout.log', 'stderr.log')}


def build_wasm(source, output, target, spec_version, guard, retain=None, retain_source=None):
    require(target.exists() and target.is_dir() and target.resolve(strict=True) == target, 'explicit persistent target must already exist')
    require(target != source and target not in source.parents and source not in target.parents, 'source/target overlap')
    require(not (source / 'FIXTURE-PROVENANCE.json').exists() and not (source / 'NEVER-DEPLOY.txt').exists(), 'fixture source rejected')
    provenance_path = source / 'RECOVERY-PROVENANCE.json'
    provenance = parse(read(provenance_path))
    no_fixture(provenance)
    require(provenance['specVersion'] == spec_version and provenance.get('bound') is True, 'unbound or wrong-version source')
    output.mkdir(parents=True, exist_ok=False)
    def remember(path, data):
        expected = write(path, data)
        if retain is not None:
            retain(path, expected)
        return expected
    with build_helpers.operation_lock(source / '.production-operation.lock', 'source build already active'):
        # Shared target lock also cooperates with the existing fixture runner.
        with build_helpers.target_lock(target):
            before = fingerprint(source)
            validate_source_provenance(source, provenance, before)
            if retain_source is not None:
                retain_source(source, before)
            if retain is not None:
                for name, expected in before.items():
                    retain(source / name, expected)
                retain(provenance_path, digest(provenance_path))
            pins = {str(source / name): value for name, value in before.items()}
            pins[str(provenance_path)] = digest(provenance_path)
            def current_guard():
                guard()
                guard_files(pins)
                require(fingerprint(source) == before, 'isolated source file set changed')
            current_guard()
            remember(output / 'source-files.json', before)
            remember(output / 'source-provenance.json', read(provenance_path))
            run_id = uuid4().hex
            # Pass only normal tool/cache locations; never propagate loader,
            # compiler wrapper, credential, proxy or inherited Cargo overrides.
            inherited = {k: os.environ[k] for k in ('HOME', 'PATH', 'TMPDIR', 'RUSTUP_HOME', 'CARGO_HOME', 'USER', 'LOGNAME') if k in os.environ}
            module_spec = importlib.util.spec_from_file_location('production_recovery_environment', build_helpers.ENVIRONMENT_HELPER)
            module = importlib.util.module_from_spec(module_spec)
            module_spec.loader.exec_module(module)
            env, environment = module.prepare_build_environment(inherited, source, run_token=run_id)
            controlled = {'CARGO_TARGET_DIR': str(target), 'WASM_BUILD_WORKSPACE_HINT': str(source),
                'WASM_BUILD_CARGO_ARGS': '--offline',
                'CC_wasm32v1_none': '/opt/homebrew/opt/llvm@22/bin/clang',
                'AR_wasm32v1_none': '/opt/homebrew/opt/llvm@22/bin/llvm-ar',
                'CC_wasm32_unknown_unknown': '/opt/homebrew/opt/llvm@22/bin/clang',
                'AR_wasm32_unknown_unknown': '/opt/homebrew/opt/llvm@22/bin/llvm-ar'}
            env.update(controlled)
            environment['controlled'].update(controlled)
            project = target / 'debug/wbuild/framenode-runtime'
            project.mkdir(parents=True, exist_ok=True)
            require(project.resolve(strict=True) == project, 'aliased Wasm project path')
            previous = build_helpers.quarantine(project, output)
            wrapper = build_helpers.LLVM_WRAPPER
            command = [str(wrapper), 'cargo', 'check', '--offline', '--locked', '-p', 'framenode-runtime', '--lib', '--features', 'build-wasm-binary']
            report = {'schema': 'sora-nexus-production-recovery-wasm-build-v1', 'fixtureOnly': False,
                'productionReady': False, 'status': 'running', 'specVersion': spec_version,
                'transactionVersion': 134 if spec_version == 138 else 131 if spec_version == 134 else 132,
                'source': str(source), 'sourceProvenance': digest(provenance_path),
                'sourceFingerprint': digest(output / 'source-files.json'), 'cargoTargetDirectory': str(target),
                'runId': run_id, 'environmentPolicy': environment, 'previousExports': previous, 'command': command,
                'lockBefore': before['Cargo.lock'], 'startedAt': now()}
            remember(output / 'build-started.json', report)
            try:
                version, _ = process(['rustc', '-Vv'], source, output / 'rustc', current_guard, env, retain=retain)
                report['rustc'] = version.decode()
                _, receipt = process(command, source, output / 'cargo', current_guard, env, timeout=24*3600, retain=retain)
                project_bytes = read(project / 'Cargo.toml')
                project_pin = pin_bytes(project_bytes)
                dependency = build_helpers.validate_wasm_project(project, source)
                require(digest(project / 'Cargo.toml') == project_pin, 'Wasm project changed during validation')
                artifacts = {}
                for filename in build_helpers.EXPORTS[1:]:
                    raw = read(project / filename, maximum=32 * 1024**2)
                    identity = release.wasm_identity(raw)
                    require((identity['specVersion'], identity['transactionVersion']) == (report['specVersion'], report['transactionVersion']), 'fresh Wasm identity mismatch')
                    name = filename.replace('framenode_runtime', 'runtime-' + str(spec_version))
                    artifacts[name] = remember(output / name, raw)
                remember(output / 'wasm-project.toml', project_bytes)
                current_guard()
                logs = copy_cargo_logs(output, receipt, remember)
                report.update(status='passed', exitCode=receipt['exitCode'], processReceipt=digest(output / 'cargo/execution.json'),
                    logs=logs,
                    runtimeIdentity=identity, wasmProjectDependency=dependency, artifacts=artifacts, lockAfter=digest(source / 'Cargo.lock'))
            except BaseException as error:
                report.update(status='failed', error=str(error))
                raise
            finally:
                report['finishedAt'] = now()
                remember(output / 'build.json', report)
    return output / f'runtime-{spec_version}.compact.compressed.wasm'


def helper_pins():
    paths = [Path(__file__).resolve(), Path(scale.__file__).resolve(), Path(release.__file__).resolve(), Path(abandoned.__file__).resolve(),
        Path(bootstrap.__file__).resolve(), bootstrap.HERE / 'bridge-recovery.patch',
        Path(build_helpers.__file__).resolve(), build_helpers.ENVIRONMENT_HELPER, build_helpers.LLVM_WRAPPER,
        Path(os.sys.executable).resolve()]
    return {str(path): digest(path) for path in paths}


def build(args):
    value, original_inputs, manifest_pin, commitment = load_inputs(args.inputs, args.inputs_sha256)
    source_files = verify_inventory(args.source_inventory, args.source_inventory_sha256, ROOT)
    require(args.output.is_absolute() and args.output.resolve() == args.output and not args.output.exists(), 'new absolute output directory required')
    require(not args.output.is_relative_to(ROOT), 'production output/source must be isolated outside working tree')
    require(args.final_target != args.legacy_target, 'final and legacy persistent targets must differ')
    for target in (args.final_target, args.legacy_target):
        require(target.is_absolute() and target.is_dir() and target.resolve(strict=True) == target
            and not target.is_relative_to(args.output) and not args.output.is_relative_to(target)
            and not target.is_relative_to(ROOT), 'persistent target must be isolated and existing')
    require((args.final_lock is None) == (args.final_lock_sha256 is None), 'final lock requires explicit hash')
    pins = {str(path): digest(path) for path in original_inputs.values()}
    pins.update({str(ROOT / name): pin for name, pin in source_files.items()})
    helpers = helper_pins()
    pins.update(helpers)
    for path, expected in ((args.inputs, args.inputs_sha256), (args.source_inventory, args.source_inventory_sha256),
        (args.tool, args.tool_sha256), (args.legacy_lock, args.legacy_lock_sha256)):
        read(path, expected, maximum=1024**3)
        pins[str(path)] = digest(path)
    if args.final_lock is not None:
        read(args.final_lock, args.final_lock_sha256)
        pins[str(args.final_lock)] = digest(args.final_lock)
    args.output.mkdir(parents=True)
    output = args.output
    inputs = output / 'inputs'
    for name, source in original_inputs.items():
        target = inputs / name
        write(target, read(source, value['files'][name]))
        target.chmod(0o444)
        pins[str(target)] = value['files'][name]
    def retain(path, expected):
        require(str(path) not in pins or pins[str(path)] == expected, 'attempt to replace retained evidence pin')
        pins[str(path)] = expected
    def remember(path, data):
        expected = write(path, data)
        retain(path, expected)
        return expected
    remember(output / 'input-manifest.json', read(args.inputs, manifest_pin))
    remember(output / 'source-inventory.json', read(args.source_inventory, args.source_inventory_sha256, 32 * 1024**2))
    remember(output / 'resolved-legacy-Cargo.lock', read(args.legacy_lock, args.legacy_lock_sha256))
    if args.final_lock is not None:
        remember(output / 'normalized-final-Cargo.lock', read(args.final_lock, args.final_lock_sha256))
    helper_copies = {}
    for index, (name, expected) in enumerate(sorted(helpers.items())):
        target = output / 'build-helpers' / f'{index:02d}-{Path(name).name}'
        remember(target, read(Path(name), expected, 1024**3))
        helper_copies[name] = {'path': str(target.relative_to(output)), **expected}
    remember(output / 'build-helpers/index.json', helper_copies)
    binary = output / 'compiler'
    write(binary, read(args.tool, args.tool_sha256, 1024**3))
    binary.chmod(0o555)
    pins[str(binary)] = digest(binary)
    source_sets = {}
    def retain_source(source, expected):
        require(str(source) not in source_sets, 'source export already registered')
        source_sets[str(source)] = expected
    def guard():
        guard_files(pins)
        for source, expected in source_sets.items():
            require(fingerprint(Path(source)) == expected, 'completed isolated source file set changed')
        require(source_inventory(ROOT) == source_files, 'live source inventory changed')
    remember(output / 'initial-pins.json', dict(pins))
    result = {'schema': 'sora-nexus-production-recovery-build-v1', 'fixtureOnly': False,
        'productionReady': False, 'status': 'running', 'startedAt': now(), 'dataCommitment': commitment,
        'inputManifest': manifest_pin, 'compiler': digest(binary), 'sourceInventory': digest(args.source_inventory),
        'executingArtifactIdentityVerified': False, 'signingStopVerified': False, 'operatorApprovalsVerified': False}
    remember(output / 'build-started.json', result)
    try:
        policy, generated = compose_inputs(value, inputs, output, binary, guard, commitment, remember, retain)
        result['policy'] = pin_bytes(policy)
        source = output / 'source-final138'
        export_final(source, source_files, generated, policy, commitment, guard, args.final_lock)
        final = build_wasm(source, output / 'build-final138', args.final_target, 138, guard, retain, retain_source)
        binding_dir = output / 'bindings'
        binding_dir.mkdir()
        remember(binding_dir / 'final138.wasm', read(final))
        remember(binding_dir / 'policy.scale', policy)
        operators = read(output / 'checkpoint-operators.scale')
        remember(binding_dir / 'operators.scale', operators)
        selected, _ = public_consensus(value, inputs)
        binding = {'policyScale': '0x' + policy.hex(), 'policyCommitment': '0x' + pin_bytes(policy)['blake2_256'],
            'operatorsScale': '0x' + operators.hex(), 'previousGrandpaSetId': selected['previousGrandpaSetId'],
            'firstRecoverySlot': selected['firstRecoverySlot'], 'finalWasm': 'final138.wasm'}
        artifacts = {'final138': final}
        for version in (135, 134):
            if version == 134:
                binding = {**binding, 'holdWasm': 'hold135.wasm'}
            path = binding_dir / f'binding-{version}.json'
            remember(path, binding)
            bootstrap.bind_inputs(path, version)
            guard()
            isolated = output / f'source-{version}'
            export_command = [str(Path(os.sys.executable).resolve()), '-B', str(Path(bootstrap.__file__).resolve()),
                '--destination', str(isolated), '--spec-version', str(version), '--binding', str(path)]
            process(export_command, output, output / f'export-{version}', guard, timeout=3600, retain=retain)
            provenance_path = isolated / 'RECOVERY-PROVENANCE.json'
            provenance = parse(read(provenance_path))
            no_fixture(provenance)
            require(provenance['bound'] is True and provenance['bindingEvidence']['binding'] == bootstrap.digest(read(path)), 'legacy export binding mismatch')
            amendment = normalize_lock(isolated, args.legacy_lock, True)
            remember(isolated / 'PRODUCTION-LOCK-AMENDMENT.json', amendment)
            wasm = build_wasm(isolated, output / f'build-{version}', args.legacy_target, version, guard, retain, retain_source)
            artifacts[str(version)] = wasm
            if version == 135:
                remember(binding_dir / 'hold135.wasm', read(wasm))
        guard()
        result.update(status='passed', finishedAt=now(), artifacts={name: {'path': str(path.relative_to(output)), **digest(path)} for name, path in artifacts.items()},
            buildReceipts={name: digest(output / name / 'build.json') for name in ('build-final138', 'build-135', 'build-134')},
            remaining='Exact-artifact replay/consensus/operator/external evidence and publication/restart remain separate gates.')
        # Outer inventory binds sources, logs, work, Wasms and receipts after all
        # inner commitments exist; it is never embedded into a runtime.
        output_entries = sorted(output.rglob('*'))
        require(all(not path.is_symlink() and path.resolve(strict=True) == path for path in output_entries), 'output graph contains an alias')
        result['files'] = {path.relative_to(output).as_posix(): digest(path) for path in output_entries
            if path.is_file() and path.name not in ('.production-operation.lock',)}
        guard()
        write(output / 'BUILD-MANIFEST.json', result)
    except BaseException as error:
        write(output / 'failure.json', {**result, 'status': 'failed', 'error': str(error), 'finishedAt': now()})
        raise
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    inv = sub.add_parser('inventory', help='Record the exact current root source set; does not build')
    inv.add_argument('--output', required=True, type=Path)
    run = sub.add_parser('build')
    for name in ('inputs', 'source-inventory', 'tool', 'legacy-lock'):
        run.add_argument('--' + name, required=True, type=Path)
        run.add_argument('--' + name + '-sha256', required=True)
    for name in ('output', 'final-target', 'legacy-target'):
        run.add_argument('--' + name, required=True, type=Path)
    run.add_argument('--final-lock', type=Path)
    run.add_argument('--final-lock-sha256')
    args = parser.parse_args()
    try:
        if args.command == 'inventory':
            write(args.output, source_inventory(ROOT))
            print(json.dumps({'sourceInventory': digest(args.output.resolve()), 'productionReady': False}))
        else:
            report = build(args)
            print(json.dumps({'status': report['status'], 'manifest': str(args.output / 'BUILD-MANIFEST.json'),
                'manifestPin': pin_bytes(serialized(report)), 'productionReady': False}))
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.exit(1, 'Production build rejected: ' + str(error) + '\n')


if __name__ == '__main__':
    main()
