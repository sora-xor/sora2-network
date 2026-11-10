#!/usr/bin/env bash
set -euo pipefail

# Build first, using the repository's warm Cargo target:
# scripts/with_llvm_env.sh cargo build --locked --release -p framenode --features wip,runtime-benchmarks
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../../../.." && pwd)
cd -- "$repo_root"
benchmark_node=${BEEFY_BENCHMARK_NODE:-"$repo_root/target/release/framenode"}
benchmark_wasm=${BEEFY_BENCHMARK_WASM:-"$repo_root/target/release/wbuild/framenode-runtime/framenode_runtime.compact.compressed.wasm"}
evidence_dir=${1:?Usage: benchmark-wasm.sh NEW_EVIDENCE_DIRECTORY}
mkdir -- "$evidence_dir"
evidence_dir=$(cd -- "$evidence_dir" && pwd)
cp -- "$benchmark_wasm" "$evidence_dir/runtime.wasm"
cp -- vendor/sora2-common/pallets/beefy-light-client/src/weights.rs "$evidence_dir/charged-weights.rs"
cp -- vendor/sora2-common/pallets/beefy-light-client/src/benchmarking.rs "$evidence_dir/benchmark-source.rs"

python3 - "$benchmark_node" "$evidence_dir" <<'PY'
import datetime, hashlib, json, os, pathlib, platform, subprocess, sys
node, output = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
sources = [pathlib.Path('Cargo.lock'), pathlib.Path('Cargo.toml'), pathlib.Path('.cargo/config.toml'),
           pathlib.Path('rust-toolchain.toml'), pathlib.Path('runtime/src/lib.rs'),
           pathlib.Path('vendor/sora2-common/pallets/bridge-common/src/bitfield.rs'),
           pathlib.Path('vendor/sora2-common/pallets/bridge-common/src/simplified_proof.rs'),
           *[pathlib.Path('vendor/sora2-common/pallets/beefy-light-client/src')/name
             for name in ('lib.rs', 'weights.rs', 'benchmarking.rs')]]
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
data = {'startedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'host': platform.platform(), 'machine': platform.machine(), 'cpuCount': os.cpu_count(),
        'loadAverage': os.getloadavg(), 'nodeSha256': sha(node), 'runtimeSha256': sha(output/'runtime.wasm'),
        'reference': 'Operator-selected local host; other workloads may run concurrently',
        'sourceSha256': {str(p): sha(p) for p in sources},
        'rustc': subprocess.check_output(['rustc', '-Vv'], text=True)}
if sys.platform == 'darwin':
    data['hardware'] = subprocess.check_output(['sysctl', 'machdep.cpu.brand_string', 'hw.ncpu', 'hw.memsize'], text=True)
(output/'provenance.json').write_text(json.dumps(data, indent=2)+'\n')
# The node's CLI constructs a chain spec even for `--runtime`; its `dev` alias
# requires private-net. An explicit local spec avoids changing runtime features.
spec = {'name': 'SORA Nexus BEEFY benchmark', 'id': 'beefy-benchmark',
        'chainType': 'Development', 'bootNodes': [],
        'genesis': {'raw': {'top': {'0x3a636f6465': '0x'+(output/'runtime.wasm').read_bytes().hex()},
                            'childrenDefault': {}}}}
(output/'chain.json').write_text(json.dumps(spec)+'\n')
PY

benchmark_args=(benchmark pallet --chain "$evidence_dir/chain.json"
  --genesis-builder spec-runtime --genesis-builder-preset benchmark
  --pallet beefy_light_client --extrinsic '*'
  --wasm-execution compiled --wasm-instantiation-strategy pooling-copy-on-write
  --steps 20 --repeat 20 --min-duration 0 --runtime-log error
  --default-pov-mode measured --output-analysis max
  --json-file "$evidence_dir/raw-results.json"
  --output "$evidence_dir/frame-generated-measurements.rs")
printf '%q ' "$benchmark_node" "${benchmark_args[@]}" > "$evidence_dir/command.sh"
printf '\n' >> "$evidence_dir/command.sh"
"$benchmark_node" "${benchmark_args[@]}" > "$evidence_dir/benchmark.log" 2>&1

python3 - "$benchmark_node" "$evidence_dir" <<'PY'
import datetime, hashlib, json, os, pathlib, sys
node, output = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
manifest = json.loads((output/'provenance.json').read_text())
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(node) == manifest['nodeSha256'], 'Benchmark node changed during the run'
for name, expected in manifest['sourceSha256'].items():
    assert sha(pathlib.Path(name)) == expected, f'Benchmark source changed: {name}'
manifest.update(finishedAt=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                finalLoadAverage=os.getloadavg(),
                outputSha256={p.name: sha(p) for p in output.iterdir() if p.name != 'provenance.json'})
(output/'provenance.json').write_text(json.dumps(manifest, indent=2)+'\n')
PY
printf 'BEEFY Wasm benchmark evidence: %s\n' "$evidence_dir"
