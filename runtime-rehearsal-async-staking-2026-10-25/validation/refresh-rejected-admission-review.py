#!/usr/bin/env python3
"""Reexecute rejected-admission verification after a fresh, passed async staking Wasm run.

Archives prior independent checker evidence, runs the 11 regressions and checker,
and rebuilds the BlockSize review only after checking source and run bindings.
No Cargo, network requests, runtime changes, or main rehearsal output edits.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[2]
LANE = ROOT / 'runtime-rehearsal-async-staking-2026-10-25'
V = LANE / 'validation'
OLD = Path('/Users/takemiyamakoto/.cargo/git/checkouts/polkadot-sdk-dee0edd6eefa0594/cd87d0b/substrate/frame/system/src/extensions/check_weight.rs')
NEW = Path('/tmp/sora2-stable2609-migration/upstream/substrate/frame/system/src/extensions/check_weight.rs')
SYSTEM = NEW.parents[1] / 'lib.rs'
REPORT = V / 'rejected-admission-block-size-review.json'
PINNED = {
    str(NEW): 'ff0f736ae85a34847b79ccbcd3a2d44e76b0b7e51ed7037b823870b6c3e6f4f3',
    str(OLD): 'a96221fc2060f3aa9c6b7eb4555eb2ccdd55e949152d2ce8b61dfe61696be706',
    str(SYSTEM): '6b0e3ca0d75a20529f1a007b1ef6663730b5a487d9f56201807b2e4b32e0e8c6',
    'vendor/polkadot-sdk/primitives/runtime/src/generic/checked_extrinsic.rs': '37a5dc10e6e922f8cd54af991cc4db5f8edeaa295ad578f25c0e743b29a86449',
    'runtime/src/lib.rs': 'fbdc748c67f54af08d2e66b8e4eba9417ee5e8623b51efb85c23d16c7c53f56b',
    'runtime/src/authorized_heartbeat_payment.rs': 'c9c9fead939f4b510f8f86aa4048e932e85527769a7f0b85df943f7f09b1761a',
    'runtime-rehearsal-async-staking-2026-10-25/validation/runtime-execution.cjs': '69a4d1867d437d4778dc8c9823f9d6c47b5777e385676d816a3ce2c765558731',
    'runtime-rehearsal-async-staking-2026-10-25/validation/verify-rejected-admission.cjs': 'fd0fe8f6301d9b41c6a0cb5af67f0ba820867b171e8e13918cbd873a59cea879',
    'runtime-rehearsal-async-staking-2026-10-25/validation/test-rejected-admission.cjs': '05a5eb97b161c799071807547c19fb6a1ffbfd2919da48bcec9b5720db6c4ba8',
    'runtime-rehearsal-async-staking-2026-10-25/validation/test-runtime-execution.cjs': 'a43f5b127129454a46138e185bede7e80f59e5d25747b6c1da3505fefabbf435',
}
FUNCTION_HASHES = {
    'do_prepare': '1ccc415d6936960eb0449b774155391cfea0f377c9eb9559459cb8322f750533',
    'bare_validate_and_prepare': 'e0a2de3fdb685a968465911f3eea5f043816bee89fe3bd4f017df91eb3271d5f',
}


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    return json.loads(Path(path).read_text())


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def path_for(name):
    p = Path(name)
    return p if p.is_absolute() else ROOT / p


def within(base, name):
    p = (base / name).resolve()
    require(p.is_relative_to(base.resolve()), 'Evidence path escapes its base: ' + name)
    return p


def function(path, name):
    source = path.read_text()
    start = source.index('fn ' + name + '(')
    opening = source.index('{', start)
    depth, end = 1, opening + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[start:end]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', default='/tmp/sora2-node24/node-v24.22.1-darwin-arm64/bin/node')
    parser.add_argument('--expected-run-id', help='Optional explicit identity of the newly passed Wasm run')
    args = parser.parse_args()
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-' + uuid.uuid4().hex[:8]
    archive = V / 'attempts' / ('rejected-admission-prior-to-' + stamp)
    names = ['rejected-admission-check.json', REPORT.name, 'rejected-admission-check.log',
             'rejected-admission-regressions.log', 'rejected-admission-source-start-check.json',
             'rejected-admission-source-end-check.json']
    archived = []
    for name in names:
        old = V / name
        if old.exists():
            archive.mkdir(parents=True, exist_ok=True)
            destination = archive / name
            shutil.move(str(old), destination)
            archived.append({'path': str(destination.relative_to(V)), 'sha256': sha(destination)})
    report = {'status': 'running', 'createdAt': now(), 'archivedPriorOutputs': archived, 'steps': []}

    def save():
        REPORT.write_text(json.dumps(report, indent=2) + '\n')

    def run_step(name, command):
        log = V / (name + '.log')
        row = {'name': name, 'command': command, 'startedAt': now()}
        report['steps'].append(row)
        save()
        with log.open('wb') as stream:
            result = subprocess.run(command, cwd=ROOT, stdout=stream, stderr=subprocess.STDOUT, check=False)
        row.update(exitCode=result.returncode, finishedAt=now(), log=log.name, logSha256=sha(log))
        save()
        require(result.returncode == 0, name + ' failed; inspect ' + str(log))
        return log.read_text()

    save()
    try:
        run_path = V / 'rehearsal-run.json'
        run = read(run_path)
        require(run['status'] == 'passed', 'Full Wasm run has not passed')
        if args.expected_run_id:
            require(run['runId'] == args.expected_run_id, 'Unexpected Wasm run identity')
        require(len(run['stages']) == 24, 'Expected all 24 Wasm stages')
        require(all(s['status'] == 'passed' and s['exitCode'] == 0 for s in run['stages']), 'A Wasm stage did not pass')
        for stage in run['stages']:
            require(sha(within(LANE, stage['log'])) == stage['logSha256'], 'Wasm log changed: ' + stage['name'])
            if 'result' in stage:
                result = within(LANE, stage['result'])
                require(sha(result) == stage['resultSha256'] and read(result)['status'] == 'passed', 'Wasm result changed: ' + stage['name'])
        for name, expected in run['inputs']['files'].items():
            require(sha(within(ROOT, name)) == expected, 'Main run input changed: ' + name)
        proof_path = V / 'candidate-build-evidence.json'
        require(sha(proof_path) == run['inputs']['buildEvidenceSha256'], 'Candidate build evidence changed')
        proof = read(proof_path)
        require(proof['status'] == 'passed', 'Candidate build did not pass')
        require(sha(LANE / 'candidate.compact.compressed.wasm') == run['inputs']['candidateSha256'] == proof['candidateSha256'], 'Candidate hashes differ')
        fingerprint = within(V, proof['sourceFingerprint']['path'])
        require(sha(fingerprint) == proof['sourceFingerprint']['sha256'], 'Final source fingerprint changed')
        require(read(fingerprint)['status'] == 'final', 'Source fingerprint is not final')
        recorder = V / 'source-fingerprint.py'
        require(sha(recorder) == read(fingerprint)['recorderSha256'], 'Source recorder changed')
        for name, expected in PINNED.items():
            require(sha(path_for(name)) == expected, 'Reviewed source changed; deliberate re-review required: ' + name)
        # The async migration changes other runtime wiring. Independently require the
        # previously reviewed admission order and heartbeat adapter to remain exact.
        runtime_source = (ROOT / 'runtime/src/lib.rs').read_text()
        expected_extensions = [
            'frame_system::AuthorizeCall<Runtime>', 'frame_system::CheckSpecVersion<Runtime>',
            'frame_system::CheckTxVersion<Runtime>', 'frame_system::CheckGenesis<Runtime>',
            'frame_system::CheckEra<Runtime>', 'frame_system::CheckNonce<Runtime>',
            'frame_system::CheckWeight<Runtime>', 'ChargeTxPaymentExtension',
        ]
        signed_extra = re.search(r'pub type SignedExtra = \((.*?)\);', runtime_source, re.S)
        require(signed_extra is not None, 'Missing signed extension pipeline')
        require([part.strip() for part in signed_extra[1].split(',') if part.strip()] == expected_extensions,
                'Reviewed signed extension order changed')
        require('pub type ChargeTxPaymentExtension = authorized_heartbeat_payment::AuthorizedHeartbeatPayment;' in runtime_source,
                'Reviewed heartbeat adapter changed')
        initial = {str(path): sha(path) for path in [run_path, proof_path, fingerprint,
            V / 'wasm-compatibility.json', V / 'candidate-metadata.json', V / 'snapshot.json',
            V / 'equivocation-fee-wasm-rehearsal.json', V / 'fee-policy-wasm-rehearsal.json',
            *(path_for(name) for name in PINNED)]}

        def source_check(label):
            output = V / ('rejected-admission-source-' + label + '-check.json')
            command = [sys.executable, str(recorder), '--check', str(fingerprint), '--source-only', '--output', str(output)]
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, check=False)
            require(result.returncode == 0, 'Current source differs from final build: ' + result.stderr)
            data = read(output)
            require(data['status'] == 'passed' and data['changes'] == [] and data['sourceSha256'] == proof['sourceFingerprint']['sourceSha256'], 'Full source check failed')
            report.setdefault('sourceChecks', []).append({'stage': label, 'command': command, 'exitCode': result.returncode, 'evidence': output.name, 'sha256': sha(output)})
            return output

        start_source = source_check('start')
        text = run_step('rejected-admission-regressions', [args.node, '--test', str(V / 'test-rejected-admission.cjs'), str(V / 'test-runtime-execution.cjs')])
        for label, count in [('tests', 11), ('pass', 11), ('fail', 0), ('skipped', 0), ('cancelled', 0)]:
            require(re.search(r'(?:#|ℹ) ' + label + ' ' + str(count) + r'\b', text), 'Unexpected regression count: ' + label)
        run_step('rejected-admission-check', [args.node, str(V / 'verify-rejected-admission.cjs')])
        check = read(V / 'rejected-admission-check.json')
        require(check['status'] == 'passed' and check['networkRequests'] == 0, 'Independent checker failed or used network')
        for key, name in [('runSha256', 'rehearsal-run.json'), ('compatibilitySha256', 'wasm-compatibility.json'), ('metadataSha256', 'candidate-metadata.json'), ('snapshotSha256', 'snapshot.json'), ('scriptSha256', 'verify-rejected-admission.cjs')]:
            require(check[key] == sha(V / name), 'Independent checker has stale binding: ' + key)
        require(check['candidateSha256'] == proof['candidateSha256'], 'Checker tested wrong candidate')
        require(all(value is True for value in check['checks'].values()), 'Independent checker has an unpassed condition')
        expected = {'equivocation-fee-wasm-rehearsal.json': (4, 12), 'fee-policy-wasm-rehearsal.json': (24, 57)}
        require(len(check['suites']) == 2, 'Expected exactly two rejected-admission suites')
        for suite in check['suites']:
            require((suite['rejectedTransactions'], suite['inspectedRawDiffEntries']) == expected[suite['report']], 'Rejected fixture counts changed')
            require(suite['reportSha256'] == sha(V / suite['report']), 'Rejected suite evidence changed')
            require(suite['rawAccountFeeRewardClaimAndEventWrites'] == suite['committedStateChangesOnInvalidTransaction'] == 0, 'Rejected transaction changed protected state')
        writes = [row for suite in check['suites'] for row in suite['blockSizeWrites']]
        policy = read(V / 'fee-policy-wasm-rehearsal.json')
        fixtures = [row for row in policy['authorizedHeartbeatChecks'] if row['label'] == 'legacy bare heartbeat rejects']
        require(len(fixtures) == 1, 'Expected one legacy bare heartbeat fixture')
        fixture = fixtures[0]
        txhash = hashlib.sha256(fixture['encoded'].encode()).hexdigest()
        require(writes == [{'extrinsicSha256': txhash, 'encodedValue': '0xd9000000', 'decodedBytes': 217}], 'Expected exactly the reviewed 217-byte BlockSize write')
        require(fixture['expectedAccepted'] is False and fixture['apply'] == {'err': {'invalid': {'call': None}}}, 'Bare heartbeat was not rejected at application')
        require(fixture['events'] == [] and fixture['passed'] is True and fixture['accountAndIssuancePreserved'] is True, 'Bare heartbeat modified protected state')
        require(set(fixture['sources']) == {'External', 'Local', 'InBlock'}, 'Missing bare heartbeat admission source')
        require(all(value == {'err': {'unknown': {'noUnsignedValidator': None}}} for value in fixture['sources'].values()), 'Bare heartbeat pool admission changed')
        functions = {}
        for name, expected_hash in FUNCTION_HASHES.items():
            before, after = function(OLD, name), function(NEW, name)
            require(before == after, 'CheckWeight callback differs from stable2603: ' + name)
            actual = hashlib.sha256(after.encode()).hexdigest()
            require(actual == expected_hash, 'Reviewed CheckWeight callback changed: ' + name)
            functions[name] = {'identicalBetween2603And2609': True, 'sha256': actual}
        require('crate::BlockSize::<T>::put(next_len);' in function(NEW, 'do_prepare'), 'Missing accounting write in reviewed callback')
        require('Self::do_prepare(info, len, next_len)' in function(NEW, 'bare_validate_and_prepare'), 'Changed callback linkage')
        require('pub type BlockSize<T: Config> = StorageValue<_, u32>;' in SYSTEM.read_text(), 'Changed BlockSize storage declaration')
        end_source = source_check('end')
        for name, expected_hash in initial.items():
            require(sha(name) == expected_hash, 'Input changed during verification: ' + name)
        paths = [Path(name) for name in initial] + [V / 'rejected-admission-check.json', start_source, end_source, Path(__file__).resolve()]
        report.update(
            finding='The prior verifier omitted System.BlockSize, an existing block-accounting write reached by the new rejected bare heartbeat fixture. This is a verifier scope correction, not a runtime behavior change.',
            upstreamRevisions={'before': 'cd87d0b95e54c8ffc374334386f3f2b2c7a61f54', 'after': '7a8daa68e18718c18823c1109513fdc1c8aa71fe'},
            sourceProof={'checkWeightFunctions': functions, 'executionOrder': [
                'CheckedExtrinsic Bare invokes ValidateUnsigned::pre_dispatch',
                'SignedExtra CheckWeight::bare_validate_and_prepare writes BlockSize and BlockWeight',
                'Following AuthorizedHeartbeatPayment rejects legacy bare ImOnline heartbeat as InvalidTransaction::Call',
                'Runtime API caller discards the entire invalid storage diff before constructing the next local state'],
                'storageDeclaration': 'frame_system::BlockSize = StorageValue<_, u32>', 'runtimeSourcesModified': False},
            fixture={'label': fixture['label'], 'extrinsicSha256': txhash, 'apply': fixture['apply'], 'events': fixture['events'], 'sources': fixture['sources'], 'blockSizeWrite': writes[0]},
            checks={'exactMetadataPlainU32AndOptionalRequired': True, 'exact32ByteBlockSizeKeyRequired': True,
                    'onlyFourByteScaleU32WritesAccepted': True, 'noSystemPrefixOrAccountEventException': True,
                    'allRejectedDiffsStillRolledBack': True, 'negativeAndRollbackTestsPassed': 11},
            inputs={str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else str(p): sha(p) for p in paths},
            runId=run['runId'], status='passed', finishedAt=now())
        save()
        print(json.dumps({'status': 'passed', 'runId': run['runId'], 'rejectedTransactions': 28, 'rawDiffEntries': 69, 'reviewSha256': sha(REPORT), 'checkerSha256': sha(V / 'rejected-admission-check.json')}))
        return 0
    except Exception as error:
        report.update(status='failed', error={'type': type(error).__name__, 'message': str(error)}, finishedAt=now())
        save()
        print(json.dumps({'status': 'failed', 'error': report['error'], 'report': str(REPORT)}), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
