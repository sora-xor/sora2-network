#!/usr/bin/env bash
# Reuse the instrumented lane and its third-party dependencies. Rebuild only the
# measured packages to remove obsolete executable coverage maps from this lane.
set -euo pipefail
cd "$(dirname "$0")/.."
export CARGO_LLVM_COV_TARGET_DIR="${CARGO_LLVM_COV_TARGET_DIR:-$PWD/target/bridge-coverage}"
report_dir="$PWD/target/bridge-coverage-reports"
mkdir -p "$report_dir"
root_packages=(-p eth-bridge -p bridge-proxy -p pallet-multisig)
shared_packages=(
    -p beefy-light-client -p bridge-channel -p bridge-common -p bridge-data-signer
    -p bridge-types -p dispatch -p evm-fungible-app -p jetton-app -p leaf-provider
    -p liberland-bridge-provider -p multisig-verifier -p pallet-multisig
    -p parachain-bridge-app -p substrate-bridge-app -p substrate-bridge-channel
)

# Never use unqualified `cargo llvm-cov clean`: it deletes the entire warm lane.
scripts/with_llvm_env.sh cargo clean --locked --target-dir "$CARGO_LLVM_COV_TARGET_DIR" \
    "${root_packages[@]}"
scripts/with_llvm_env.sh cargo clean --locked --target-dir "$CARGO_LLVM_COV_TARGET_DIR" \
    --manifest-path vendor/sora2-common/Cargo.toml "${shared_packages[@]}"
python3 - "$CARGO_LLVM_COV_TARGET_DIR" "$report_dir" <<'PY'
from pathlib import Path
import sys
# llvm-cov can silently reuse old merged data when no fresh raw profiles exist.
for pattern in ('*.profraw', '*.profdata'):
    for profile in Path(sys.argv[1]).glob(pattern):
        profile.unlink()
for pattern in ('*.lcov', '*.json', '*.html'):
    for report in Path(sys.argv[2]).glob(pattern):
        report.unlink()
PY
# Snapshot tests, fixtures, production sources, manifests and coverage tooling.
python3 scripts/bridge_coverage_snapshot.py "$report_dir/source-hashes.before.json"
python3 -m unittest discover -s scripts -p test_bridge_coverage.py

# --no-report implies --no-clean in cargo-llvm-cov. The explicit cleanup above is
# intentional; both exports below use the same freshly built source maps.
scripts/with_llvm_env.sh cargo llvm-cov test --locked --lib --no-fail-fast --no-report \
    "${root_packages[@]}"
scripts/with_llvm_env.sh cargo llvm-cov report --locked --lcov \
    --output-path "$report_dir/root.lcov"
scripts/with_llvm_env.sh cargo llvm-cov report --locked --json \
    --output-path "$report_dir/root.json"
scripts/with_llvm_env.sh cargo llvm-cov test --locked --lib --no-fail-fast --no-report \
    --manifest-path vendor/sora2-common/Cargo.toml "${shared_packages[@]}"
scripts/with_llvm_env.sh cargo llvm-cov report --locked --lcov \
    --manifest-path vendor/sora2-common/Cargo.toml \
    --output-path "$report_dir/shared.lcov"
scripts/with_llvm_env.sh cargo llvm-cov report --locked --json \
    --manifest-path vendor/sora2-common/Cargo.toml \
    --output-path "$report_dir/shared.json"
python3 scripts/bridge_coverage.py "$report_dir/root.lcov" "$report_dir/shared.lcov" \
    --llvm-json "$report_dir/root.json" "$report_dir/shared.json"
python3 scripts/bridge_coverage_snapshot.py "$report_dir/source-hashes.after.json" \
    "$report_dir/source-hashes.before.json"
