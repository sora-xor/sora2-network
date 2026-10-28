#!/usr/bin/env python3
"""Fixed local passive SORA Nexus sync and committed-boundary crash corridor.

No consensus keys, authoring, external endpoints, Docker fallback or publication.
All executions are explicit; importing this module performs no process/network work.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import stat
import subprocess
import time
import urllib.request

import node_identity as identity
import production_build as build

SCHEMA = 'sora-nexus-passive-consensus-input-v1'
RESULT = 'sora-nexus-passive-consensus-execution-v1'
CHAIN = 'sora-nexus-recovery-28191817'
CHECKPOINT = 28_191_817
CHECKPOINT_HASH = identity.CHECKPOINT_HASH
TIMEOUT = 24 * 3600
MAX_LOG = 256 * 1024**2
MAX_DB = 4 * 1024**4
MAX_FILES = 2_000_000
COPY_RESERVE_MIN = 8 * 1024**3
COPY_RESERVE_MAX = 32 * 1024**3
ENV = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}


def need(value, message):
    if not value:
        raise ValueError('Passive consensus: ' + message)


def exact(value, names):
    need(isinstance(value, dict) and set(value) == set(names), 'unknown/missing fields')


def digest(path):
    identity.physical(path)
    return build.digest(path)


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    return build.write(path, value)


def inventory(directory):
    identity.physical(directory, directory=True)
    result, total = {}, 0
    for path in sorted(directory.rglob('*')):
        mode = path.lstat().st_mode
        need(not stat.S_ISLNK(mode), 'database alias forbidden')
        if stat.S_ISDIR(mode):
            continue
        need(stat.S_ISREG(mode) and path.stat().st_nlink == 1, 'database must contain unaliased regular files')
        name = path.relative_to(directory).as_posix()
        need(not any(part.lower() in {'keystore', 'network', '.env'} or 'creds' in part.lower()
                     for part in Path(name).parts), 'database input must exclude keys/network/credentials')
        total += path.stat().st_size
        need(len(result) < MAX_FILES and total <= MAX_DB, 'database inventory bound')
        result[name] = digest(path)
    need(result, 'empty database input')
    return result


def copy_space(target, expected):
    """Check the actual destination filesystem; this does not reserve its space."""
    need(isinstance(expected, dict) and 0 < len(expected) <= MAX_FILES,
         'database copy inventory missing or oversized')
    sizes = [pin.get('bytes') for pin in expected.values() if isinstance(pin, dict)]
    need(len(sizes) == len(expected) and all(type(size) is int and size >= 0 for size in sizes),
         'database copy byte inventory invalid')
    required = sum(sizes)
    need(required <= MAX_DB, 'database copy byte budget exceeds bound')
    target = Path(target)
    need(target.is_absolute() and '..' not in target.parts, 'canonical absolute copy target required')
    ancestor = target.parent
    while not ancestor.exists():
        need(not ancestor.is_symlink() and ancestor != ancestor.parent, 'invalid copy target ancestor')
        ancestor = ancestor.parent
    identity.physical(ancestor, directory=True)
    reserve = min(COPY_RESERVE_MAX, max(COPY_RESERVE_MIN, required // 100))
    free = shutil.disk_usage(ancestor).free
    need(type(free) is int and free >= required + reserve,
         f'insufficient destination space: need {required} copy bytes plus {reserve} reserve bytes; {free} free')
    return {'requiredBytes': required, 'reserveBytes': reserve, 'freeBytes': free,
            'filesystemPath': str(ancestor)}


def copy_tree(source, target, expected):
    need(not target.exists(), 'database copy target already exists')
    # Fail before streaming a multi-terabyte source inventory, then recheck
    # immediately before creating output because that scan can take hours.
    copy_space(target, expected)
    need(inventory(source) == expected, 'database pre-copy inventory changed')
    copy_space(target, expected)
    target.mkdir(parents=True)
    for name, pin in expected.items():
        dst = target / name
        dst.parent.mkdir(parents=True, exist_ok=True)
        with (source / name).open('rb') as src, dst.open('xb') as out:
            shutil.copyfileobj(src, out, 1024**2)
        need(digest(dst) == pin, 'database copy differs')
    need(inventory(source) == expected and inventory(target) == expected, 'database changed during copy')


def file_input(root, value):
    exact(value, {'path', 'sha256'})
    path = build.local(root, value['path'])
    pin = digest(path)
    need(pin['sha256'] == value['sha256'], 'input file hash mismatch')
    return path, pin


def require_rocksdb_shape(path):
    # A complete RPC trie capture is not a Substrate client database. This early
    # format check only rejects wrong input kinds; the native stopped-copy
    # verifier still establishes state, lineage and real consensus evidence.
    current = path / 'CURRENT'
    need(current.exists(), 'RocksDB CURRENT marker missing; RPC captures are not client databases')
    identity.physical(current)
    with current.open('rb') as stream:
        raw = stream.read(129)
    need(len(raw) <= 128 and re.fullmatch(rb'MANIFEST-[0-9]+\n', raw) is not None,
         'RocksDB CURRENT must name one local MANIFEST file')
    manifest = path / raw[:-1].decode('ascii')
    need(manifest.exists(), 'RocksDB CURRENT refers to a missing MANIFEST')
    identity.physical(manifest)
    need(manifest.stat().st_size > 0, 'RocksDB MANIFEST is empty')


def db_input(root, value):
    exact(value, {'path', 'inventory'})
    path = root / build.relative(value['path'])
    identity.physical(path, directory=True)
    require_rocksdb_shape(path)
    manifest, pin = file_input(root, value['inventory'])
    expected = identity.strict_json(manifest.read_bytes())
    need(inventory(path) == expected, 'retained database inventory differs')
    return path, expected, manifest, pin


def reject_public_keys(config):
    # RecoveryConfig serializes AuthorityId as canonical SS58 prefix42, not hex.
    keys = [identity.authority_key(key).hex() for key, _ in config['grandpaAuthorities']]
    keys += [identity.authority_key(key).hex() for key in config['approvedGrandpaKeys']]
    need(not build.PUBLIC_SIGNING_KEYS.intersection(keys), 'public fixture GRANDPA keys forbidden')


def target(value):
    exact(value, {'hash', 'number', 'stateRoot', 'codeHash', 'phase', 'cursor', 'operationCounter'})
    for name in ('hash', 'stateRoot', 'codeHash'):
        need(isinstance(value[name], str) and re.fullmatch(r'0x[0-9a-f]{64}', value[name]), 'target hash shape')
    need(type(value['number']) is int and CHECKPOINT < value['number'] <= CHECKPOINT + 100_000, 'target height')
    need(value['phase'] in {'Applying', 'Auditing', 'Ready', 'Hold', 'Complete', 'Failed'}, 'target phase')
    need(all(type(value[k]) is int and 0 <= value[k] < 2**64 for k in ('cursor', 'operationCounter')), 'target recovery counters')
    return value


def validate_manifest(value):
    exact(value, {'schema', 'route', 'node', 'chainSpec', 'recoveryConfig', 'bootstrapWasm',
                  'sourceInventory', 'predecessorDatabase', 'providerDatabase', 'target', 'crashStages', 'ports'})
    need(value['schema'] == SCHEMA and value['route'] == 'native-loopback', 'unsupported schema/route; no fallback')
    target(value['target'])
    ports = value['ports']
    need(isinstance(ports, list) and len(ports) == 10 and len(set(ports)) == 10
         and all(type(p) is int and 1024 <= p <= 65535 for p in ports), 'ten distinct explicit local ports required')
    stages = value['crashStages']
    need(isinstance(stages, list) and 1 <= len(stages) <= 100_000, 'at least one exact crash stage required')
    seen = set()
    for stage in stages:
        exact(stage, {'providerDatabase', 'target'})
        row = target(stage['target'])
        need(row['hash'] not in seen and row['number'] <= value['target']['number'], 'duplicate/out-of-range crash stage')
        seen.add(row['hash'])
    return value


def check_report(report, expected, policy, warp_pid=None, full=False):
    need(report.get('schema') == 'sora-nexus-persisted-recovery-database-v1', 'persisted report schema')
    need(report.get('checkpointHash') == CHECKPOINT_HASH and report.get('policyCommitment') == policy, 'persisted lineage differs')
    need(report.get('bestHash') == report.get('finalizedHash') == expected['hash'], 'persisted exact finality differs')
    for name in ('number', 'stateRoot', 'phase', 'cursor', 'operationCounter'):
        need(report.get(name) == expected[name], 'persisted ' + name + ' differs')
    need(report.get('storage', {}).get('0x3a636f6465', {}).get('blake2') == expected['codeHash'], 'persisted runtime differs')
    need(report.get('storageProofVerified') is True and report.get('finalityProofVerified') is True, 'fresh native proofs failed')
    need(report.get('productionReady') is False and report.get('operatorApprovalVerified') is False, 'unsupported approval claim')
    if warp_pid is not None:
        seen = report.get('observedWarp')
        need(isinstance(seen, dict) and seen.get('pid') == warp_pid and seen.get('target') == expected['hash']
             and seen.get('sdkReverified') is True and type(seen.get('proofs')) is int and seen['proofs'] > 0,
             'missing actual accepted warp proof')
    if full:
        rows = report.get('recoveryHeaders')
        need(isinstance(rows, list) and len(rows) == expected['number'] - CHECKPOINT, 'full persisted history missing')
        previous = CHECKPOINT_HASH
        for number, row in enumerate(rows, CHECKPOINT + 1):
            need(row.get('number') == number and row.get('parentHash') == previous, 'full history ancestry gap')
            previous = row.get('hash')
        need(previous == expected['hash'], 'full history target differs')


def imported_history(directory, pid, report):
    imported = {}
    for path in sorted(directory.glob('*.json')):
        row = identity.strict_json(path.read_bytes())
        need(row.get('schema') == 'sora-nexus-passive-node-observation-v1' and row.get('pid') == pid, 'import observer process differs')
        if row.get('kind') == 'import-result' and row.get('imported') is True:
            need(row.get('origin') != 'WarpSync' and row.get('headerOnly') is False and row.get('badJustification') is not True, 'full lane skipped execution or rejected finality')
            imported[row.get('number')] = row.get('hash')
    need(imported.get(CHECKPOINT) == CHECKPOINT_HASH, 'actual full checkpoint import absent')
    for row in report['recoveryHeaders']:
        need(imported.get(row['number']) == row['hash'], 'actual full recovery import absent')
    return len(imported)


def process_receipt(start, returncode, signal_sent, killed_for_trial):
    need(not killed_for_trial or (signal_sent == signal.SIGKILL and returncode == -signal.SIGKILL),
         'crash trial did not terminate by actual SIGKILL')
    return {**start, 'finishedAt': build.now(), 'exitCode': returncode,
            'signalSent': signal_sent, 'deliberateCrash': killed_for_trial,
            'productionReady': False}


class Harness:
    def __init__(self, manifest, output):
        self.manifest = Path(manifest).absolute()
        identity.physical(self.manifest)
        self.value = validate_manifest(identity.strict_json(self.manifest.read_bytes()))
        self.root, self.output = self.manifest.parent, Path(output).absolute()
        need(not self.output.exists() and self.output.resolve() == self.output
             and not self.output.is_relative_to(self.root) and not self.output.is_relative_to(build.ROOT),
             'fresh isolated output outside input/source trees required')
        self.output.mkdir(parents=True)
        self.pins, self.dbs, self.running, self.sequence = {}, [], [], 0
        self.retain(self.manifest)
        self.files = {}
        for role in ('node', 'chainSpec', 'recoveryConfig', 'bootstrapWasm', 'sourceInventory'):
            path, pin = file_input(self.root, self.value[role]); self.pins[str(path)] = pin
            self.files[role] = self.copy_file(path, self.output / 'inputs' / role)
        self.sources = build.verify_inventory(self.files['sourceInventory'], self.value['sourceInventory']['sha256'], build.ROOT)
        for helper in (Path(__file__).resolve(), Path(identity.__file__).resolve(), Path(build.__file__).resolve(),
                       Path(build.release.__file__).resolve(), Path(build.scale.__file__).resolve()):
            self.copy_file(helper, self.output / 'helpers' / helper.name)
        config = identity.strict_json(self.files['recoveryConfig'].read_bytes())
        self.policy = config['policyCommitment']
        spec = identity.strict_json(self.files['chainSpec'].read_bytes())
        identity.template_inputs(spec, config, self.files['bootstrapWasm'].read_bytes())
        reject_public_keys(config)
        need(spec.get('id') == CHAIN and spec.get('bootNodes') == [] and spec.get('telemetryEndpoints') == [], 'discovery routes must be empty')
        need(spec.get('recovery') == config, 'prepared chain configuration differs')
        for index, (name, pin) in enumerate(sorted(build.helper_pins().items())):
            helper = Path(name)
            need(digest(helper) == pin, 'helper changed before retention')
            self.copy_file(helper, self.output / 'transitive-helpers' / f'{index:02d}-{helper.name}')
        self.predecessor = self.load_db(self.value['predecessorDatabase'])
        self.provider = self.load_db(self.value['providerDatabase'])
        self.crashes = [(self.load_db(row['providerDatabase']), row['target']) for row in self.value['crashStages']]

    def retain(self, path):
        pin = digest(path)
        need(str(path) not in self.pins or self.pins[str(path)] == pin, 'retained artifact rebound')
        self.pins[str(path)] = pin
        return pin

    def save(self, path, value):
        write(path, value); self.retain(path)

    def copy_file(self, source, destination):
        pin = self.retain(source)
        destination.parent.mkdir(parents=True, exist_ok=True)
        with source.open('rb') as src, destination.open('xb') as dst:
            shutil.copyfileobj(src, dst, 1024**2)
        need(digest(destination) == pin, 'copied executable/input differs')
        self.retain(destination)
        return destination

    def load_db(self, row):
        path, expected, manifest, pin = db_input(self.root, row)
        self.pins[str(manifest)] = pin
        self.dbs.append((path, expected))
        return path, expected

    def guard(self):
        build.guard_files(self.pins)
        need(build.source_inventory(build.ROOT) == self.sources, 'source changed during verification')
        # Original database files are never opened by a node. Complete checks are
        # streamed only at copying/stage boundaries, not on every polling tick.

    def originals(self):
        self.guard()
        for path, expected in self.dbs:
            need(inventory(path) == expected, 'retained original database changed')

    def database_path(self, lane):
        return lane / 'base/chains' / CHAIN / 'db/full'

    def lane(self, name, source=None):
        path = self.output / name
        need(not path.exists(), 'lane already exists')
        path.mkdir(parents=True)
        if source:
            copy_tree(source[0], self.database_path(path), source[1])
        return path

    def command(self, lane, mode, rpc, p2p, peers):
        self.sequence += 1
        run = lane / f'process-{self.sequence:05}'
        run.mkdir()
        binary = self.copy_file(self.files['node'], run / 'framenode'); binary.chmod(0o555)
        evidence = run / 'observations'
        argv = [str(binary), '--chain', str(self.files['chainSpec']), '--base-path', str(lane / 'base'),
            '--database', 'rocksdb', '--state-pruning', 'archive', '--blocks-pruning', 'archive',
            '--sync', mode, '--listen-addr', f'/ip4/127.0.0.1/tcp/{p2p}', '--rpc-port', str(rpc),
            '--rpc-methods', 'safe', '--reserved-only', '--allow-private-ip', '--no-mdns',
            '--no-telemetry', '--no-prometheus', '--offchain-worker', 'never', '--disable-beefy',
            '--recovery-evidence-dir', str(evidence)]
        for peer in peers:
            need(re.fullmatch(r'/ip4/127\.0\.0\.1/tcp/[0-9]+/p2p/[A-Za-z0-9]+', peer), 'nonlocal peer forbidden')
            argv += ['--reserved-nodes', peer]
        self.guard()
        stdout, stderr = (run / 'stdout.log').open('xb'), (run / 'stderr.log').open('xb')
        child = subprocess.Popen(argv, cwd=run, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                 env=ENV, start_new_session=True)
        start = {'command': argv, 'pid': child.pid, 'startedAt': build.now(), 'executable': digest(binary),
                 'rpcPort': rpc, 'p2pPort': p2p, 'modeRequested': mode, 'timeoutSeconds': TIMEOUT}
        self.save(run / 'started.json', start)
        record = {'child': child, 'run': run, 'lane': lane, 'start': start, 'files': (stdout, stderr),
                  'deadline': time.monotonic() + TIMEOUT, 'rpc': rpc, 'evidence': evidence}
        self.running.append(record)
        return record

    def check_process(self, record):
        need(record['child'].poll() is None, 'node exited before observation')
        need(time.monotonic() < record['deadline'], 'node stage timed out')
        need(all(os.fstat(f.fileno()).st_size <= MAX_LOG for f in record['files']), 'node output exceeded bound')

    def rpc(self, record, method, params=()):
        self.check_process(record)
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
        request = urllib.request.Request(f"http://127.0.0.1:{record['rpc']}", data=body, headers={'Content-Type': 'application/json'})
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=120) as response:
            raw = response.read(4 * 1024**2 + 1)
        need(len(raw) <= 4 * 1024**2, 'RPC response bound')
        value = identity.strict_json(raw)
        need(value.get('id') == 1 and value.get('jsonrpc') == '2.0' and 'error' not in value, 'local RPC failed')
        return value['result']

    def wait(self, record, expected=None):
        while True:
            self.check_process(record)
            try:
                if expected is None:
                    peer = self.rpc(record, 'system_localPeerId')
                    need(isinstance(peer, str) and re.fullmatch('[A-Za-z0-9]{16,128}', peer), 'peer ID shape')
                    return peer
                finalized = self.rpc(record, 'chain_getFinalizedHead')
                best = self.rpc(record, 'chain_getHeader')
                if finalized == expected['hash'] and int(best['number'], 16) == expected['number']:
                    need(self.rpc(record, 'chain_getBlockHash', [expected['number']]) == expected['hash'], 'wrong local branch')
                    return
                need(int(best['number'], 16) <= expected['number'], 'node passed declared target')
            except (OSError, TimeoutError):
                pass
            time.sleep(1)

    def stop(self, record, crash=False):
        child = record['child']
        need(child.poll() is None, 'node already exited; no deliberate signal evidence')
        sig = signal.SIGKILL if crash else signal.SIGTERM
        self.save(record['run'] / 'signal.json', {'pid':child.pid,'signal':sig,'sentAt':build.now(),'purpose':'committed-boundary-crash' if crash else 'stop'})
        os.killpg(child.pid, sig)
        code = child.wait(timeout=1200)
        need(crash or code in (0, -signal.SIGTERM), 'node failed during requested normal stop')
        for f in record['files']:
            f.close()
        receipt = process_receipt(record['start'], code, sig, crash)
        receipt['stdout'] = self.retain(record['run'] / 'stdout.log')
        receipt['stderr'] = self.retain(record['run'] / 'stderr.log')
        for path in record['evidence'].iterdir():
            self.retain(path)
        self.save(record['run'] / 'execution.json', receipt)
        self.running.remove(record)
        self.guard()
        return receipt

    def verify(self, lane, expected, name, *, observed=None, full=False):
        source = self.database_path(lane)
        before = inventory(source)
        copied = self.output / 'inspections' / name
        copy_tree(source, self.database_path(copied), before)
        self.save(copied / 'database-files.json', before)
        binary = self.copy_file(self.files['node'], copied / 'framenode'); binary.chmod(0o555)
        argv = [str(binary), 'verify-recovery-database', '--chain', str(self.files['chainSpec']),
            '--base-path', str(copied / 'base'), '--database', 'rocksdb', '--state-pruning', 'archive',
            '--blocks-pruning', 'archive', '--expected-hash', expected['hash'], '--output', str(copied / 'proofs')]
        if full:
            argv.append('--complete-history')
        if observed:
            argv += ['--warp-evidence', str(observed['evidence']), '--expected-warp-pid', str(observed['start']['pid'])]
        raw, _ = build.process(argv, copied, copied / 'verification', self.guard, timeout=TIMEOUT,
                               maximum=MAX_LOG, retain=lambda p, pin: self.retain(p))
        report = identity.strict_json(raw)
        check_report(report, expected, self.policy, observed['start']['pid'] if observed else None, full)
        need(report == identity.strict_json((copied / 'proofs/report.json').read_bytes()), 'stdout/file report differs')
        for path in (copied / 'proofs').iterdir(): self.retain(path)
        need(inventory(source) == before, 'stopped original lane changed during inspection')
        self.guard()
        return report

    def cohort(self, name, source, expected, *, crash=False):
        providers = [self.lane(f'{name}/provider-{i}', source) for i in range(3)]
        clients = [self.lane(f'{name}/full', self.predecessor)]
        if not crash: clients.append(self.lane(f'{name}/warp'))
        lanes = providers + clients
        modes = ['full'] * 4 + ([] if crash else ['warp'])
        ports = self.value['ports']
        ids = []
        # Isolated starts generate only owned libp2p identity files. They are never
        # read, copied, printed or supplied as consensus keys.
        for index, (lane, mode) in enumerate(zip(lanes, modes)):
            proc = self.command(lane, mode, ports[2*index], ports[2*index+1], [])
            ids.append(self.wait(proc)); self.stop(proc)
            opened = [identity.strict_json(p.read_bytes()) for p in proc['evidence'].glob('*.json')]
            opened = [row for row in opened if row.get('kind') == 'database-open']
            need(len(opened) == 1 and opened[0].get('pid') == proc['start']['pid'], 'actual database-open receipt missing')
            start = opened[0]
            if index == 3:
                need(start.get('bestNumber') == CHECKPOINT-1 and start.get('bestHash') == self.predecessor_hash
                     and start.get('checkpointMarkerPresent') is False, 'full lane did not start at clean checkpoint predecessor')
            elif index == 4:
                need(start.get('bestNumber') == 0 and start.get('finalizedNumber') == 0
                     and start.get('checkpointMarkerPresent') is False, 'warp lane was not empty genesis')
        addresses = [f'/ip4/127.0.0.1/tcp/{ports[2*i+1]}/p2p/{peer}' for i, peer in enumerate(ids)]
        provider_procs = [self.command(lane, 'full', ports[2*i], ports[2*i+1], addresses[3:])
                          for i, lane in enumerate(providers)]
        client_procs = [self.command(lane, mode, ports[2*(i+3)], ports[2*(i+3)+1], addresses[:3])
                        for i, (lane, mode) in enumerate(zip(clients, modes[3:]))]
        for proc in provider_procs + client_procs:
            self.wait(proc, expected)
        full = client_procs[0]
        # The deliberate crash scope is after this exact accepted import receipt
        # and finality observation. Waiting here excludes a torn observer record
        # from being mistaken for an in-commit crash experiment.
        while True:
            self.check_process(full)
            observed_target = False
            for path in sorted(full['evidence'].glob('*.json')):
                try:
                    row = identity.strict_json(path.read_bytes())
                except (ValueError, OSError):
                    continue
                if row.get('kind') == 'import-result' and row.get('pid') == full['start']['pid'] and row.get('imported') is True and row.get('hash') == expected['hash']:
                    observed_target = True
            if observed_target: break
            time.sleep(1)
        crash_receipt = self.stop(full, crash=crash)
        before = self.verify(clients[0], expected, name + '-full-before', full=True)
        imported_history(full['evidence'], full['start']['pid'], before)
        if crash:
            persisted = inventory(self.database_path(clients[0]))
            self.save(self.output / name / 'crashed-database-files.json', persisted)
            restarted = self.command(clients[0], 'full', ports[6], ports[7], addresses[:3])
            self.wait(restarted, expected); restart_receipt = self.stop(restarted)
            after = self.verify(clients[0], expected, name + '-full-after', full=True)
            need(before == after, 'persisted recovery/consensus evidence changed across restart')
            result = {'target':expected,'crash':crash_receipt,'restart':restart_receipt,
                      'timing':'SIGKILL after observed committed/finalized target; no in-commit claim',
                      'persistedReport':after}
        else:
            warp = client_procs[1]; self.stop(warp)
            warped = self.verify(clients[1], expected, name + '-warp', observed=warp)
            need({k: before[k] for k in ('bestHash','finalizedHash','stateRoot','storage','grandpaSetId','grandpaAuthorities')}
                 == {k: warped[k] for k in ('bestHash','finalizedHash','stateRoot','storage','grandpaSetId','grandpaAuthorities')}, 'full/warp persisted consensus differs')
            result = {'full':before,'warp':warped,'acceptedFullImportCount':len(before['recoveryHeaders'])+1}
        for proc in provider_procs: self.stop(proc)
        self.originals()
        self.save(self.output / name / 'result.json', result)
        return result

    def cleanup(self):
        # Cleanup does not depend on mutable source/input guards, and addresses
        # only process handles launched and retained by this harness instance.
        for record in list(self.running):
            child = record['child']
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=1200)
            for file in record['files']: file.close()
            write(record['run'] / 'cleanup.json', {'pid':child.pid,'exitCode':child.returncode,'deliberateCrash':False,'productionReady':False})
        self.running.clear()

    def run(self):
        try:
            self.originals()
            # Verify provider data first; this command must see real finalized
            # state and prove it before any networking process starts.
            seed = self.lane('provider-admission', self.provider)
            provider = self.verify(seed, self.value['target'], 'provider-admission', full=True)
            self.predecessor_hash = provider['checkpointParentHash']
            sync = self.cohort('synchronization', self.provider, self.value['target'])
            canonical = {row['hash']:row for row in provider['recoveryHeaders']}
            crashes = []
            for index, (source, expected) in enumerate(self.crashes):
                need(expected['hash'] in canonical and canonical[expected['hash']]['stateRoot'] == expected['stateRoot'], 'crash stage outside verified canonical history')
                admitted = self.lane(f'crash-admission-{index}', source)
                self.verify(admitted, expected, f'crash-admission-{index}', full=True)
                crashes.append(self.cohort(f'crash-{index:05}', source, expected, crash=True))
            covered = {row['target']['hash'] for row in crashes}
            result = {'schema':RESULT,'input':digest(self.manifest),'node':digest(self.files['node']),
                'chainSpec':digest(self.files['chainSpec']),'policyCommitment':self.policy,
                'fullAndWarpSynchronization':sync,'committedBoundaryCrashes':crashes,
                'allRecoveryBlocksKilledAfterCommit': covered == set(canonical),
                'inCommitCrashVerified':False,'chunksCrashesAndRestartsGateVerified':False,
                'operatorApprovalVerified':False,'productionReady':False}
            self.originals(); self.save(self.output / 'completion.json', result)
            return result
        except BaseException as error:
            self.cleanup()
            write(self.output / 'failure.json', {'schema':RESULT,'error':str(error),'productionReady':False})
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = Harness(args.manifest, args.output).run()
    print(json.dumps({'completion':str(args.output / 'completion.json'),'schema':RESULT,'productionReady':result['productionReady']}))


if __name__ == '__main__': main()
