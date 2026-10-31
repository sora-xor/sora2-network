#!/usr/bin/env bash
# Use the supplied Docker context or the current configured context; never change it.
set -euo pipefail
repository=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
mode=${1:---check}
case "$mode" in
  --check) mount_mode=readonly ;;
  --write) mount_mode= ;;
  *) echo 'Usage: rebuild_wasm_in_container.sh [--check|--write]' >&2; exit 2 ;;
esac
docker_in_context() {
  if [[ -n "${SORA_WASM_DOCKER_CONTEXT:-}" ]]; then
    docker --context "$SORA_WASM_DOCKER_CONTEXT" "$@"
  else
    docker "$@"
  fi
}
image=sora-reviewed-wasm-rebuild:2026-10-24
volume=${SORA_WASM_BUILD_VOLUME:-sora-reviewed-wasm-canonical}
docker_in_context build --platform linux/amd64 --file "$repository/misc/dependency_security/Dockerfile.wasm-rebuild" --tag "$image" "$repository/misc/dependency_security"
docker_in_context volume create --label task=sora-reviewed-wasm-rebuild "$volume" >/dev/null
source_mount="type=bind,source=$repository,target=/repo"
if [[ -n "$mount_mode" ]]; then source_mount+=",$mount_mode"; fi
docker_in_context run --rm --platform linux/amd64   --mount "$source_mount" --mount "type=volume,source=$volume,target=/cache"   "$image" bash -c '
    set -euo pipefail
    build_args=()
    if [[ "$1" == --check ]]; then build_args=(--check); fi
    node vendor/chopsticks-executor/scripts/build-reviewed.mjs "${build_args[@]}" --build-tools-dir /opt/build-tools
    node vendor/polkadot-wasm-crypto/scripts/build-reviewed.mjs "${build_args[@]}" --build-tools-dir /opt/build-tools
    python3 vendor/chopsticks-executor/scripts/verify-artifacts.py
    python3 vendor/polkadot-wasm-crypto/scripts/verify-artifacts.py
  ' -- "$mode"
