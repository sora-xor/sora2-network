#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runner="$repo_root/housekeeping/clippy.sh"
test_dir="$(mktemp -d "${TMPDIR:-/tmp}/sora-clippy-test.XXXXXX")"
trap 'rm -rf "$test_dir"' EXIT

cat > "$test_dir/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$PWD" == "$CLIPPY_TEST_ROOT" ]]
mode=mainnet
for arg in "$@"; do
    case "$arg" in
        try-runtime) mode=try-runtime ;;
        private-net,stage,wip,runtime-benchmarks) mode=extended ;;
    esac
done
if [[ "$1" == tree ]]; then
    printf '%s\n' "$*" >> "$CLIPPY_TEST_TREE_LOG"
    if [[ "${CLIPPY_TEST_TREE_FAILURE:-}" == true ]]; then
        exit 43
    fi
    case "${CLIPPY_TEST_GRAPH:-}" in
        empty) exit 0 ;;
        unrecognized)
            printf 'unrecognized runtime graph format\n'
            exit 0
            ;;
    esac
    printf 'framenode-runtime feature "std"\n'
    printf 'framenode-runtime feature "frame-try-runtime"\n'
    if [[ "$mode" == try-runtime && "${CLIPPY_TEST_MISSING_TRY_RUNTIME:-}" != true ]]; then
        printf 'framenode-runtime feature "try-runtime" (*)\n'
    fi
    if [[ -n "${CLIPPY_TEST_EXTRA_FEATURE:-}" ]]; then
        printf 'framenode-runtime feature "%s" (*)\n' "$CLIPPY_TEST_EXTRA_FEATURE"
    fi
    exit 0
fi
printf '%s\n' "$*" >> "$CLIPPY_TEST_LOG"
[[ "$1" == clippy && "${SKIP_WASM_BUILD:-}" == 1 ]]
printf '{"reason":"stub","mode":"%s"}\n' "$mode"
if [[ "${CLIPPY_TEST_FAIL_MODE:-}" == "$mode" ]]; then
    exit 42
fi
STUB
chmod +x "$test_dir/cargo"
export PATH="$test_dir:$PATH"
export CLIPPY_TEST_LOG="$test_dir/calls"
export CLIPPY_TEST_TREE_LOG="$test_dir/tree-calls"
export CLIPPY_TEST_ROOT="$repo_root"

# Invoke from outside the checkout and exercise the default feature coverage.
cd "$test_dir"
"$runner" > "$test_dir/output"
cat > "$test_dir/expected" <<'EXPECTED'
clippy --locked --all-targets -p framenode -p framenode-runtime -p eth-bridge -- -D warnings
clippy --locked --all-targets -p framenode -p framenode-runtime -p eth-bridge --features try-runtime -- -D warnings
clippy --locked --all-targets --workspace --features private-net,stage,wip,runtime-benchmarks -- -D warnings
EXPECTED
diff -u "$test_dir/expected" "$CLIPPY_TEST_LOG"
cat > "$test_dir/expected" <<'EXPECTED'
tree --locked --color never -p framenode -p framenode-runtime -p eth-bridge -e features -i framenode-runtime --prefix none
tree --locked --color never -p framenode -p framenode-runtime -p eth-bridge --features try-runtime -e features -i framenode-runtime --prefix none
EXPECTED
diff -u "$test_dir/expected" "$CLIPPY_TEST_TREE_LOG"

# Each mode can be selected alone, with machine-readable stdout for SARIF.
for mode in mainnet try-runtime extended; do
    : > "$CLIPPY_TEST_LOG"
    "$runner" "$mode" --message-format=json > "$test_dir/output"
    printf '{"reason":"stub","mode":"%s"}\n' "$mode" > "$test_dir/expected"
    diff -u "$test_dir/expected" "$test_dir/output"
    packages='-p framenode -p framenode-runtime -p eth-bridge'
    case "$mode" in
        mainnet) features='' ;;
        try-runtime) features=' --features try-runtime' ;;
        extended)
            packages=--workspace
            features=' --features private-net,stage,wip,runtime-benchmarks'
            ;;
    esac
    printf 'clippy --locked --all-targets %s%s --message-format=json -- -D warnings\n' \
        "$packages" "$features" > "$test_dir/expected"
    diff -u "$test_dir/expected" "$CLIPPY_TEST_LOG"
done

# A failed mode remains a failure after the later modes succeed.
: > "$CLIPPY_TEST_LOG"
status=0
CLIPPY_TEST_FAIL_MODE=mainnet "$runner" all > "$test_dir/output" || status=$?
[[ "$status" == 42 ]]
[[ "$(wc -l < "$CLIPPY_TEST_LOG" | tr -d ' ')" == 3 ]]
[[ "$(tail -n 1 "$CLIPPY_TEST_LOG")" == *'--features private-net,stage,wip,runtime-benchmarks'* ]]

# A report consumer succeeding must not hide a Clippy failure in a CI pipeline.
status=0
CLIPPY_TEST_FAIL_MODE=try-runtime "$runner" try-runtime --message-format=json \
    | cat > "$test_dir/output" || status=$?
[[ "$status" == 42 ]]

# Reject production feature leakage before Clippy can produce a false green.
for mode in mainnet try-runtime; do
    for feature in private-net stage wip runtime-benchmarks; do
        : > "$CLIPPY_TEST_LOG"
        status=0
        CLIPPY_TEST_EXTRA_FEATURE="$feature" "$runner" "$mode" \
            > "$test_dir/output" 2>&1 || status=$?
        [[ "$status" == 2 && ! -s "$CLIPPY_TEST_LOG" ]]
    done
    for graph in empty unrecognized; do
        : > "$CLIPPY_TEST_LOG"
        status=0
        CLIPPY_TEST_GRAPH="$graph" "$runner" "$mode" \
            > "$test_dir/output" 2>&1 || status=$?
        [[ "$status" == 2 && ! -s "$CLIPPY_TEST_LOG" ]]
    done
done
: > "$CLIPPY_TEST_LOG"
status=0
CLIPPY_TEST_EXTRA_FEATURE=try-runtime "$runner" mainnet \
    > "$test_dir/output" 2>&1 || status=$?
[[ "$status" == 2 && ! -s "$CLIPPY_TEST_LOG" ]]
status=0
CLIPPY_TEST_MISSING_TRY_RUNTIME=true "$runner" try-runtime \
    > "$test_dir/output" 2>&1 || status=$?
[[ "$status" == 2 && ! -s "$CLIPPY_TEST_LOG" ]]
status=0
CLIPPY_TEST_TREE_FAILURE=true "$runner" mainnet \
    > "$test_dir/output" 2>&1 || status=$?
[[ "$status" == 43 && ! -s "$CLIPPY_TEST_LOG" ]]

# Reject ambiguous modes and unknown flags before running Cargo.
for invalid in unknown --all-features; do
    : > "$CLIPPY_TEST_LOG"
    status=0
    "$runner" "$invalid" > "$test_dir/output" 2>&1 || status=$?
    [[ "$status" == 2 && ! -s "$CLIPPY_TEST_LOG" ]]
done
: > "$CLIPPY_TEST_LOG"
status=0
"$runner" mainnet extended > "$test_dir/output" 2>&1 || status=$?
[[ "$status" == 2 && ! -s "$CLIPPY_TEST_LOG" ]]

printf 'Clippy runner tests passed.\n'
