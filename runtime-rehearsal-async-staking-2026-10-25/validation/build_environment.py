"""Prepare the fixed local production-build environment without mutating the caller.

Only names of removed inherited variables enter the evidence. In particular, do
not serialize the incoming environment: unrelated credentials must remain private.
The caller creates one fresh run_token per build run and records the returned
metadata together with the artifact freshness checks.
"""
from __future__ import annotations

import hashlib
from pathlib import Path
import re
from typing import Mapping
import tomllib


SCHEMA_VERSION = 1
RUST_OVERRIDES = frozenset({
    "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS",
    "RUSTUP_TOOLCHAIN", "RUSTC", "RUSTDOC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_BOOTSTRAP", "RUSTC_FORCE_INCREMENTAL", "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTDOC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTDOCFLAGS",
})


def is_build_override(key: str) -> bool:
    """Recognize SDK skip controls and inherited compiler/target/profile overrides."""
    return (
        key == "SKIP_WASM_BUILD"
        or (key.startswith("SKIP_") and key.endswith("_WASM_BUILD"))
        or key in {"DOCS_RS", "FORCE_WASM_BUILD", "WASM_TARGET_DIRECTORY",
                   "SUBSTRATE_RUNTIME_TARGET", "CARGO_TARGET_DIR", "PYTHONOPTIMIZE",
                   "PYTHONPATH", "PYTHONHOME"}
        or key.startswith("WASM_BUILD_")
        or key in RUST_OVERRIDES
        or key.startswith("CARGO_PROFILE_")
        or (key.startswith("CARGO_TARGET_") and key.endswith(("_RUSTFLAGS", "_LINKER", "_RUNNER")))
    )


def prepare_build_environment(
    inherited: Mapping[str, str], repo_root: Path, *, run_token: str,
) -> tuple[dict[str, str], dict]:
    """Return a copied environment and evidence for this fixed macOS build lane.

    The function does not modify os.environ, create directories, or run commands.
    Token generation belongs to the caller so repeated calls with identical input
    are deterministic and the runner can bind the token to its unique run.
    """
    if not isinstance(run_token, str) or not re.fullmatch(r"[A-Za-z0-9_-]{16,128}", run_token):
        raise ValueError("run_token must be a fresh 16–128 character build identifier")
    root = Path(repo_root).resolve(strict=True)
    toolchain_bytes = (root / "rust-toolchain.toml").read_bytes()
    toolchain = tomllib.loads(toolchain_bytes.decode("utf-8"))["toolchain"]["channel"]
    if not isinstance(toolchain, str) or not re.fullmatch(
        r"(?:nightly|beta)-\d{4}-\d{2}-\d{2}|\d+\.\d+\.\d+", toolchain,
    ):
        raise ValueError("rust-toolchain.toml must pin a dated or exact Rust toolchain")

    controlled = {
        "LIBRARY_PATH": "/opt/homebrew/opt/llvm@22/lib",
        "LIBCLANG_PATH": "/opt/homebrew/opt/llvm@22/lib",
        "LLVM_CONFIG_PATH": "/opt/homebrew/opt/llvm@22/bin/llvm-config",
        "CARGO_TARGET_DIR": str(root / "target"),
        "RUSTUP_TOOLCHAIN": toolchain,
        "WASM_BUILD_TOOLCHAIN": toolchain,
        "WASM_BUILD_TYPE": "release",
        "WASM_BUILD_NO_COLOR": "1",
        # Match the reviewed .cargo/config.toml host-function import workaround.
        # Explicitly pin it because Cargo's force=false env setting otherwise
        # permits the incoming environment to replace the flag silently.
        "WASM_BUILD_RUSTFLAGS": "-C link-arg=--import-undefined",
        # The SDK accepts only 0/1, not false/true. The pinned toolchain supplies
        # the wasm32v1-none standard library, matching this lane's prior build.
        "WASM_BUILD_STD": "0",
        "SUBSTRATE_RUNTIME_TARGET": "wasm",
        "FORCE_WASM_BUILD": run_token,
    }
    removed = sorted(key for key in inherited if is_build_override(key) or key in controlled)
    environment = {key: value for key, value in inherited.items() if key not in removed}
    environment.update(controlled)
    metadata = {
        "schemaVersion": SCHEMA_VERSION,
        "removedKeys": removed,
        "controlled": controlled,
        "toolchain": toolchain,
        "toolchainFile": "rust-toolchain.toml",
        "toolchainFileSha256": hashlib.sha256(toolchain_bytes).hexdigest(),
    }
    return environment, metadata
