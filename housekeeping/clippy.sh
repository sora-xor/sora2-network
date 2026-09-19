#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: housekeeping/clippy.sh [all|mainnet|try-runtime|extended] [--message-format=json]

Runs all modes by default and returns a failure if any mode fails.
  mainnet      Production defaults, including real runtime migrations.
  try-runtime  Production defaults with migration validation hooks.
  extended     Private/stage/WIP features and runtime benchmarks.

JSON diagnostics go to stdout; progress messages go to stderr.
USAGE
}

mode=all
mode_set=false
json=false
for arg in "$@"; do
    case "$arg" in
        all|mainnet|try-runtime|extended)
            if [[ "$mode_set" == true ]]; then
                printf 'Only one lint mode may be selected.\n' >&2
                exit 2
            fi
            mode="$arg"
            mode_set=true
            ;;
        --message-format=json)
            json=true
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            printf 'Unknown argument: %s\n' "$arg" >&2
            usage >&2
            exit 2
            ;;
    esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

run_clippy() {
    local selected_mode="$1"
    local selection=()
    # Other workspace members' dev-dependencies enable private-net. Keep the
    # production modes scoped so Cargo does not unify those features into them.
    if [[ "$selected_mode" == extended ]]; then
        selection=(--workspace)
    else
        selection=(-p framenode -p framenode-runtime -p eth-bridge)
    fi
    case "$selected_mode" in
        mainnet) ;;
        try-runtime) selection+=(--features try-runtime) ;;
        extended) selection+=(--features private-net,stage,wip,runtime-benchmarks) ;;
    esac

    if [[ "$selected_mode" != extended ]]; then
        local feature_graph
        if feature_graph="$(cargo tree --locked --color never "${selection[@]}" -e features \
            -i framenode-runtime --prefix none)"; then
            :
        else
            local tree_status=$?
            printf 'Cannot verify production runtime features (%s).\n' "$selected_mode" >&2
            return "$tree_status"
        fi

        local line runtime_feature
        local has_try_runtime=false
        local has_runtime_features=false
        while IFS= read -r line; do
            if [[ "$line" == 'framenode-runtime feature "'* ]]; then
                has_runtime_features=true
                runtime_feature="${line#framenode-runtime feature \"}"
                runtime_feature="${runtime_feature%%\"*}"
                case "$runtime_feature" in
                    private-net|stage|wip|runtime-benchmarks)
                        printf 'Refusing %s lint: dependency feature unification enabled framenode-runtime/%s. Inspect cargo tree and remove the development feature from this production selection.\n' \
                            "$selected_mode" "$runtime_feature" >&2
                        return 2
                        ;;
                    try-runtime) has_try_runtime=true ;;
                esac
            fi
        done <<< "$feature_graph"

        if [[ "$has_runtime_features" == false ]]; then
            printf 'Refusing %s lint: cargo tree produced no recognized framenode-runtime feature rows. Inspect its output before relying on this production check.\n' \
                "$selected_mode" >&2
            return 2
        fi

        if [[ "$selected_mode" == mainnet && "$has_try_runtime" == true ]] \
            || [[ "$selected_mode" == try-runtime && "$has_try_runtime" == false ]]; then
            printf 'Refusing %s lint: unexpected framenode-runtime/try-runtime feature state (%s). Inspect cargo tree and the selected feature forwarding.\n' \
                "$selected_mode" "$has_try_runtime" >&2
            return 2
        fi
    fi

    local args=(clippy --locked --all-targets "${selection[@]}")
    if [[ "$json" == true ]]; then
        args+=(--message-format=json)
    fi
    args+=(-- -D warnings)

    printf 'Running Clippy (%s)\n' "$selected_mode" >&2
    SKIP_WASM_BUILD=1 cargo "${args[@]}"
}

modes=("$mode")
if [[ "$mode" == all ]]; then
    modes=(mainnet try-runtime extended)
fi

status=0
for selected_mode in "${modes[@]}"; do
    if run_clippy "$selected_mode"; then
        :
    else
        result=$?
        printf 'Clippy (%s) failed with exit status %s\n' "$selected_mode" "$result" >&2
        if [[ "$status" == 0 ]]; then
            status="$result"
        fi
    fi
done
exit "$status"
