#!/usr/bin/env bash

BENCHMARK_REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BENCHMARK_STEPS="50"
BENCHMARK_REPEAT="20"
BENCHMARK_HEADER_FILE="${BENCHMARK_REPO_ROOT}/misc/file_header.txt"
BENCHMARK_DEFAULT_TEMPLATE="${BENCHMARK_REPO_ROOT}/misc/pallet-weight-template.hbs"
BENCHMARK_POLKAMARKT_PALLET_TEMPLATE="${BENCHMARK_REPO_ROOT}/misc/polkamarkt-pallet-weight-template.hbs"
BENCHMARK_POLKAMARKT_RUNTIME_TEMPLATE="${BENCHMARK_REPO_ROOT}/misc/polkamarkt-runtime-weight-template.hbs"
BENCHMARK_LOCAL_BINARY="${BENCHMARK_LOCAL_BINARY:-${BENCHMARK_REPO_ROOT}/target/release/framenode}"
BENCHMARK_LOCAL_FEATURES="runtime-benchmarks,private-net,stage,runtime-wasm"
BENCHMARK_RUNTIME_WASM="${BENCHMARK_RUNTIME_WASM:-${BENCHMARK_REPO_ROOT}/target/release/wbuild/framenode-runtime/framenode_runtime.wasm}"
BENCHMARK_GENESIS_PRESET="${BENCHMARK_GENESIS_PRESET:-benchmark}"
BENCHMARK_OVERHEAD_WEIGHT_PATH="${BENCHMARK_REPO_ROOT}/runtime/src/constants/"

benchmark::build_local_binary() {
    echo "[+] Compiling benchmark-capable framenode..."
    (
        cd "${BENCHMARK_REPO_ROOT}" &&
            cargo build --release --locked --features "${BENCHMARK_LOCAL_FEATURES}" --bin framenode
    )
}

benchmark::chain_args() {
    printf '%s\n' "--chain=local"
}

benchmark::require_chain_benchmark_binary() {
    local binary_path="$1"
    if [[ ! -x "${binary_path}" ]]; then
        echo "[-] Benchmark binary not found or not executable: ${binary_path}" >&2
        return 1
    fi
    if ! "${binary_path}" benchmark pallet --help >/dev/null 2>&1; then
        echo "[-] Benchmark subcommand is unavailable in ${binary_path}" >&2
        return 1
    fi
    if ! "${binary_path}" benchmark pallet --list --chain=local >/dev/null 2>&1; then
        echo "[-] Local chain pallet benchmarking is unavailable with ${binary_path}" >&2
        return 1
    fi
}

benchmark::require_runtime_benchmark_binary() {
    local binary_path="$1"
    benchmark::require_chain_benchmark_binary "${binary_path}" || return 1
    if [[ ! -f "${BENCHMARK_RUNTIME_WASM}" ]]; then
        echo "[-] Benchmark runtime wasm not found: ${BENCHMARK_RUNTIME_WASM}" >&2
        return 1
    fi
    if ! "${binary_path}" benchmark pallet --list \
        --runtime="${BENCHMARK_RUNTIME_WASM}" \
        --genesis-builder=runtime \
        --genesis-builder-preset="${BENCHMARK_GENESIS_PRESET}" >/dev/null 2>&1; then
        echo "[-] Runtime preset benchmarking is unavailable with ${binary_path}" >&2
        return 1
    fi
}

benchmark::require_runtime_overhead_benchmark_binary() {
    local binary_path="$1"
    benchmark::require_runtime_benchmark_binary "${binary_path}" || return 1
    if ! benchmark::smoke_overhead "${binary_path}"; then
        echo "[-] Overhead benchmarking smoke test failed for ${binary_path}" >&2
        return 1
    fi
}

benchmark::runtime_args() {
    printf '%s\n' \
        "--runtime=${BENCHMARK_RUNTIME_WASM}" \
        "--genesis-builder=runtime" \
        "--genesis-builder-preset=${BENCHMARK_GENESIS_PRESET}"
}

benchmark::smoke_overhead() {
    local binary_path="$1"
    local smoke_dir
    smoke_dir="$(mktemp -d "${TMPDIR:-/tmp}/framenode-overhead.XXXXXX")"
    local runtime_args=()
    while IFS= read -r arg; do
        runtime_args+=("${arg}")
    done < <(benchmark::runtime_args)
    if ! "${binary_path}" benchmark overhead \
        "${runtime_args[@]}" \
        --wasm-execution=compiled \
        --weight-path="${smoke_dir}" \
        --warmup=1 \
        --repeat=1 \
        --header="${BENCHMARK_HEADER_FILE}" >/dev/null 2>&1; then
        rm -rf "${smoke_dir}"
        return 1
    fi
    rm -rf "${smoke_dir}"
}

benchmark::list_pallets() {
    local binary_path="$1"
    local arg_provider="${2:-benchmark::runtime_args}"
    local benchmark_args=()
    while IFS= read -r arg; do
        benchmark_args+=("${arg}")
    done < <("${arg_provider}")
    "${binary_path}" benchmark pallet --list "${benchmark_args[@]}" | tail -n +2 | cut -d',' -f1 | sort | uniq
}

benchmark::normalize_pallet_key() {
    local pallet_name="$1"
    local normalized="${pallet_name//::/-}"
    normalized="${normalized//_/-}"
    printf '%s\n' "${normalized}"
}

benchmark::default_output_for_key() {
    local key="$1"
    local candidate="${BENCHMARK_REPO_ROOT}/pallets/${key}/src/weights.rs"
    if [[ -d "$(dirname "${candidate}")" ]]; then
        printf '%s\n' "${candidate}"
        return 0
    fi

    if [[ "${key}" == pallet-* ]]; then
        local stripped="${key#pallet-}"
        candidate="${BENCHMARK_REPO_ROOT}/pallets/${stripped}/src/weights.rs"
        if [[ -d "$(dirname "${candidate}")" ]]; then
            printf '%s\n' "${candidate}"
            return 0
        fi
    fi

    printf '%s\n' "${candidate}"
}

benchmark::primary_output_for_key() {
    local key="$1"
    case "${key}" in
        bridge-inbound-channel)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/trustless-bridge/bridge-inbound-channel/src/weights.rs"
            ;;
        bridge-outbound-channel)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/trustless-bridge/bridge-outbound-channel/src/weights.rs"
            ;;
        erc20-app)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/trustless-bridge/erc20-app/src/weights.rs"
            ;;
        eth-app)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/trustless-bridge/eth-app/src/weights.rs"
            ;;
        ethereum-light-client)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/trustless-bridge/ethereum-light-client/src/weights.rs"
            ;;
        evm-bridge-proxy)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/bridge-proxy/src/weights.rs"
            ;;
        migration-app)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/pallets/trustless-bridge/migration-app/src/weights.rs"
            ;;
        multisig-verifier)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/multisig_verifier.rs"
            ;;
        bridge-data-signer)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/bridge_data_signer.rs"
            ;;
        dispatch)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/dispatch.rs"
            ;;
        parachain-bridge-app)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/parachain_bridge_app.rs"
            ;;
        substrate-bridge-app)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/substrate_bridge_app.rs"
            ;;
        substrate-bridge-channel-inbound)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/substrate_inbound_channel.rs"
            ;;
        substrate-bridge-channel-outbound)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/substrate_outbound_channel.rs"
            ;;
        *)
            benchmark::default_output_for_key "${key}"
            ;;
    esac
}

benchmark::primary_template_for_key() {
    local key="$1"
    case "${key}" in
        multisig-verifier|bridge-data-signer|dispatch|substrate-bridge-app|substrate-bridge-channel-inbound|substrate-bridge-channel-outbound)
            printf '\n'
            ;;
        polkamarkt)
            printf '%s\n' "${BENCHMARK_POLKAMARKT_PALLET_TEMPLATE}"
            ;;
        *)
            printf '%s\n' "${BENCHMARK_DEFAULT_TEMPLATE}"
            ;;
    esac
}

benchmark::extra_output_for_key() {
    local key="$1"
    case "${key}" in
        polkamarkt)
            printf '%s\n' "${BENCHMARK_REPO_ROOT}/runtime/src/weights/polkamarkt.rs"
            ;;
    esac
}

benchmark::extra_template_for_key() {
    local key="$1"
    case "${key}" in
        polkamarkt)
            printf '%s\n' "${BENCHMARK_POLKAMARKT_RUNTIME_TEMPLATE}"
            ;;
        *)
            printf '%s\n' "${BENCHMARK_DEFAULT_TEMPLATE}"
            ;;
    esac
}

benchmark::is_runtime_output() {
    local output_path="$1"
    [[ "${output_path}" == "${BENCHMARK_REPO_ROOT}/runtime/src/weights/"* ]]
}

benchmark::run_target() {
    local binary_path="$1"
    local pallet_name="$2"
    local output_path="$3"
    local template_path="$4"
    local arg_provider="${5:-benchmark::runtime_args}"

    if [[ ! -d "$(dirname "${output_path}")" ]]; then
        echo "[-] Output directory not found for ${pallet_name}: $(dirname "${output_path}")" >&2
        return 1
    fi

    echo "[+] Benchmarking ${pallet_name} -> ${output_path}"
    local benchmark_args=()
    while IFS= read -r arg; do
        benchmark_args+=("${arg}")
    done < <("${arg_provider}")
    local cmd=(
        "${binary_path}" benchmark pallet
        "${benchmark_args[@]}"
        --steps="${BENCHMARK_STEPS}"
        --repeat="${BENCHMARK_REPEAT}"
        --pallet="${pallet_name}"
        --extrinsic="*"
        --wasm-execution=compiled
        --header="${BENCHMARK_HEADER_FILE}"
        --output="${output_path}"
    )
    if [[ -n "${template_path}" ]]; then
        cmd+=(--template="${template_path}")
    fi
    "${cmd[@]}"
}

benchmark::run_primary_target() {
    local binary_path="$1"
    local pallet_name="$2"
    local key="$3"
    local arg_provider="${4:-benchmark::runtime_args}"
    local output_path
    output_path="$(benchmark::primary_output_for_key "${key}")"
    local template_path
    template_path="$(benchmark::primary_template_for_key "${key}")"
    benchmark::run_target "${binary_path}" "${pallet_name}" "${output_path}" "${template_path}" "${arg_provider}"
}

benchmark::run_extra_target() {
    local binary_path="$1"
    local pallet_name="$2"
    local key="$3"
    local arg_provider="${4:-benchmark::runtime_args}"
    local output_path
    output_path="$(benchmark::extra_output_for_key "${key}")"
    if [[ -z "${output_path}" ]]; then
        return 0
    fi
    local template_path
    template_path="$(benchmark::extra_template_for_key "${key}")"
    benchmark::run_target "${binary_path}" "${pallet_name}" "${output_path}" "${template_path}" "${arg_provider}"
}

benchmark::run_all_pallet_targets() {
    local binary_path="$1"
    local arg_provider="${2:-benchmark::runtime_args}"
    local benchmark_label="${3:-runtime preset ${BENCHMARK_GENESIS_PRESET}}"
    local pallets=()
    local pallet_name
    while IFS= read -r pallet_name; do
        pallets+=("${pallet_name}")
    done < <(benchmark::list_pallets "${binary_path}" "${arg_provider}")
    echo "[+] Benchmarking ${#pallets[@]} pallets with ${benchmark_label}"
    for pallet_name in "${pallets[@]}"; do
        local key
        key="$(benchmark::normalize_pallet_key "${pallet_name}")"
        local output_path
        output_path="$(benchmark::primary_output_for_key "${key}")"
        if [[ ! -d "$(dirname "${output_path}")" ]]; then
            echo "[-] ${pallet_name} (${key}) not found at $(dirname "${output_path}"), skipping..."
            continue
        fi
        benchmark::run_primary_target "${binary_path}" "${pallet_name}" "${key}" "${arg_provider}"
        benchmark::run_extra_target "${binary_path}" "${pallet_name}" "${key}" "${arg_provider}"
    done
}

benchmark::run_all_runtime_targets() {
    local binary_path="$1"
    local pallets=()
    local pallet_name
    while IFS= read -r pallet_name; do
        pallets+=("${pallet_name}")
    done < <(benchmark::list_pallets "${binary_path}")
    echo "[+] Benchmarking runtime weight targets with runtime preset ${BENCHMARK_GENESIS_PRESET}"
    for pallet_name in "${pallets[@]}"; do
        local key
        key="$(benchmark::normalize_pallet_key "${pallet_name}")"
        local primary_output
        primary_output="$(benchmark::primary_output_for_key "${key}")"
        if benchmark::is_runtime_output "${primary_output}" && [[ -d "$(dirname "${primary_output}")" ]]; then
            benchmark::run_primary_target "${binary_path}" "${pallet_name}" "${key}"
        fi

        local extra_output
        extra_output="$(benchmark::extra_output_for_key "${key}")"
        if [[ -n "${extra_output}" ]] && benchmark::is_runtime_output "${extra_output}" && [[ -d "$(dirname "${extra_output}")" ]]; then
            benchmark::run_extra_target "${binary_path}" "${pallet_name}" "${key}"
        fi
    done
}

benchmark::run_overhead() {
    local binary_path="$1"
    echo "[+] Benchmarking block and extrinsic overheads..."
    local runtime_args=()
    while IFS= read -r arg; do
        runtime_args+=("${arg}")
    done < <(benchmark::runtime_args)
    "${binary_path}" benchmark overhead \
        "${runtime_args[@]}" \
        --wasm-execution=compiled \
        --weight-path="${BENCHMARK_OVERHEAD_WEIGHT_PATH}" \
        --warmup=10 \
        --repeat=100 \
        --header="${BENCHMARK_HEADER_FILE}"
}
