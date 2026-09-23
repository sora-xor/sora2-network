#!/usr/bin/env python3
"""Compile one local harness with existing warm release rlibs; never invokes Cargo or network."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SDK = Path.home() / '.cargo/git/checkouts/polkadot-sdk-dee0edd6eefa0594/e373717/substrate/client/consensus/babe/src'
FP = ROOT / 'target/release/.fingerprint'
DEPS = ROOT / 'target/release/deps'
BABE = FP / 'sc-consensus-babe-80614a8e0f3bf09a'

parser = argparse.ArgumentParser()
parser.add_argument('--expect-runtime-acceptance', action='store_true')
args = parser.parse_args()
manifest = json.loads((BABE / 'lib-sc_consensus_babe.json').read_text())
required = {'sp_runtime', 'codec', 'sp_core', 'sp_consensus_babe', 'sp_consensus_slots',
            'sp_application_crypto', 'sp_keystore', 'num_rational', 'num_bigint', 'num_traits',
            'log', 'sc_consensus_slots', 'sc_consensus_epochs', 'sp_crypto_hashing'}
externs = {'sc_consensus_babe': DEPS / 'libsc_consensus_babe-80614a8e0f3bf09a.rlib'}
for _, name, _, value in manifest['deps']:
    if name not in required:
        continue
    crate = 'parity_scale_codec' if name == 'codec' else name
    matching = [p for p in FP.glob('*/lib-' + crate)
                if p.read_text() == value.to_bytes(8, 'little').hex()]
    assert len(matching) == 1, (name, matching)
    suffix = matching[0].parent.name.rsplit('-', 1)[1]
    externs[name] = DEPS / f'lib{crate}-{suffix}.rlib'
assert required.issubset(externs)
serde = sorted(DEPS.glob('libserde_json-*.rlib'))
assert serde
externs['serde_json'] = serde[0]
for file in externs.values():
    assert file.is_file(), file

lane = Path(tempfile.mkdtemp(prefix='sora-babe-verifier-'))
source = lane / 'main.rs'
source.write_text((HERE / 'verify-canonical-header.rs.in').read_text().replace('__SDK_BABE_SOURCE__', str(SDK)))
binary = lane / 'verify-canonical-header'
command = ['rustc', '+nightly-2025-05-08', '--edition=2021', '-L', f'dependency={DEPS}', str(source), '-o', str(binary)]
for name, path in externs.items():
    command.extend(['--extern', f'{name}={path}'])
print(json.dumps({'sdkSource': str(SDK), 'verifierSha256': hashlib.sha256((SDK / 'verification.rs').read_bytes()).hexdigest(),
                  'authorshipSha256': hashlib.sha256((SDK / 'authorship.rs').read_bytes()).hexdigest(),
                  'buildDirectory': str(lane), 'cargoInvoked': False, 'networkAccess': False}), flush=True)
subprocess.run(command, check=True, cwd=ROOT)
evidence = json.loads((HERE / 'babe-config-evidence.json').read_text())
canonical = json.loads((HERE / 'canonical-header.json').read_text())
latest = next(s for s in evidence['states'] if s['label'] == 'latest')
assert canonical['epochStateReference'] == latest['hash']
assert canonical['epoch'] == latest['runtimeApi']['currentEpoch']
evidence['headers'] = [canonical]
fixture = lane / 'fixture.json'
fixture.write_text(json.dumps(evidence))
run = [str(binary), str(fixture)]
if args.expect_runtime_acceptance:
    run.append('--expect-runtime-acceptance')
raise SystemExit(subprocess.run(run, cwd=ROOT).returncode)
