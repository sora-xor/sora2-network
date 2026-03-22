#!/usr/bin/env python3
"""Build and deploy SCCP Solana programs to mainnet-beta.

Safe by default:
- Dry-run unless --execute is provided.
- Mainnet execution additionally requires --ack-mainnet.
- Execute mode enforces mainnet genesis-hash identity and resumable checkpoints.
- Deployment is not marked complete until the governor-gated bootstrap flow is also completed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shlex
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlsplit

ACK_TOKEN = "I_UNDERSTAND_MAINNET_DEPLOY"
DEFAULT_RPC = "https://api.mainnet-beta.solana.com"
MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"
STATE_VERSION = 1

SENSITIVE_FLAGS = {"--url"}


def redact_command(cmd: list[str]) -> str:
    parts: list[str] = []
    redact_next = False
    for token in cmd:
        if redact_next:
            parts.append("<redacted>")
            redact_next = False
            continue
        parts.append(token)
        if token in SENSITIVE_FLAGS:
            redact_next = True
    return shlex.join(parts)


def run(cmd: list[str], cwd: Path, capture: bool = False) -> subprocess.CompletedProcess[str]:
    print(f"+ {redact_command(cmd)}")
    return subprocess.run(
        cmd,
        cwd=str(cwd),
        check=True,
        text=True,
        capture_output=capture,
    )


def run_stdout(cmd: list[str], cwd: Path) -> str:
    cp = run(cmd, cwd=cwd, capture=True)
    out = (cp.stdout or "").strip()
    if not out:
        return (cp.stderr or "").strip()
    return out


def command_exists(name: str) -> bool:
    return shutil.which(name) is not None


def read_file_value(path: str) -> str:
    p = Path(path).expanduser().resolve()
    if not p.exists():
        raise SystemExit(f"File not found: {p}")
    return p.read_text(encoding="utf-8").strip()


def resolve_rpc_url(args: argparse.Namespace) -> str:
    if args.rpc_url:
        return args.rpc_url.strip()
    if args.rpc_url_file:
        v = read_file_value(args.rpc_url_file)
        if v:
            return v
    return DEFAULT_RPC


def sanitize_rpc_host(rpc_url: str) -> str:
    try:
        return urlsplit(rpc_url).netloc or "<redacted>"
    except Exception:
        return "<redacted>"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while True:
            chunk = f.read(8192)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def read_json_file(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def atomic_write_json(path: Path, value: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_name(f"{path.name}.tmp")
    tmp_path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")
    tmp_path.replace(path)


def hash_params(value: dict[str, object]) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def default_state_path(repo_root: Path, payer_pubkey: str | None, payer_path: Path) -> Path:
    suffix = (payer_pubkey or payer_path.stem).replace("/", "_").replace("\\", "_")[:16]
    return (repo_root / "deployments" / "state" / f"mainnet-solana-{suffix}.json").resolve()


def ensure_state_policy(*, execute: bool, resume: bool, state_file: Path) -> None:
    exists = state_file.exists()
    if not execute:
        return
    if resume and not exists:
        raise SystemExit(f"--resume requested but state file does not exist: {state_file}")
    if not resume and exists:
        raise SystemExit(
            f"State file already exists: {state_file}. Use --resume or pass a different --state-file."
        )


def optional_arg(args: argparse.Namespace, key: str) -> str | None:
    value = getattr(args, key)
    if value is None:
        return None
    raw = str(value).strip()
    return raw or None


def keypair_pubkey(path: Path, repo_root: Path) -> str:
    return run_stdout(["solana-keygen", "pubkey", str(path)], cwd=repo_root)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Build + deploy SCCP Solana programs to mainnet-beta")
    p.add_argument("--rpc-url", default=None, help="Solana RPC URL (avoid if URL includes secrets)")
    p.add_argument("--rpc-url-file", default=None, help="File containing Solana RPC URL")
    p.add_argument(
        "--payer-keypair",
        default=None,
        help="Path to payer keypair JSON",
    )
    p.add_argument(
        "--program-keypair",
        default=None,
        help="Path to keypair for SCCP Solana program ID",
    )
    p.add_argument(
        "--verifier-keypair",
        default=None,
        help="Path to keypair for SCCP Solana verifier program ID",
    )
    p.add_argument(
        "--governor-pubkey",
        default=None,
        help="Governor pubkey authorized for router/verifier bootstrap",
    )
    p.add_argument(
        "--governor-keypair",
        default=None,
        help="Optional governor signer keypair used for bootstrap when it differs from --payer-keypair",
    )
    p.add_argument(
        "--latest-beefy-block",
        type=int,
        default=None,
        help="Initial finalized SORA BEEFY block for verifier bootstrap",
    )
    p.add_argument("--current-validator-set-id", type=int, default=None)
    p.add_argument("--current-validator-set-len", type=int, default=None)
    p.add_argument("--current-validator-set-root", default=None, help="32-byte hex root")
    p.add_argument("--next-validator-set-id", type=int, default=None)
    p.add_argument("--next-validator-set-len", type=int, default=None)
    p.add_argument("--next-validator-set-root", default=None, help="32-byte hex root")

    p.add_argument("--program-so", default=None, help="Path to sccp_sol_program.so")
    p.add_argument("--verifier-so", default=None, help="Path to sccp_sol_verifier_program.so")
    p.add_argument("--skip-build", action="store_true", help="Skip program build")
    p.add_argument("--out", default=None, help="Output JSON path")
    p.add_argument("--state-file", default=None, help="Checkpoint state JSON path")
    p.add_argument("--resume", action="store_true", help="Resume from existing --state-file")

    p.add_argument("--execute", action="store_true", help="Send real mainnet deployment transactions")
    p.add_argument("--ack-mainnet", default=None, help=f"Must equal {ACK_TOKEN} with --execute")
    return p.parse_args()


def ensure_file(path: str | None, name: str) -> Path:
    if not path:
        raise SystemExit(f"Missing --{name}")
    p = Path(path).expanduser().resolve()
    if not p.exists():
        raise SystemExit(f"{name} not found: {p}")
    return p


def build_solana_program(repo_root: Path, manifest_path: Path) -> None:
    try:
        run(["cargo", "build-sbf", "--manifest-path", str(manifest_path)], cwd=repo_root)
    except subprocess.CalledProcessError:
        # Older Solana toolchains still use build-bpf.
        run(["cargo", "build-bpf", "--manifest-path", str(manifest_path)], cwd=repo_root)


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    rpc_url = resolve_rpc_url(args)

    payer = ensure_file(args.payer_keypair, "payer-keypair")
    program_kp = ensure_file(args.program_keypair, "program-keypair")
    verifier_kp = ensure_file(args.verifier_keypair, "verifier-keypair")

    if args.execute and args.ack_mainnet != ACK_TOKEN:
        print(
            f"Refusing mainnet deploy: pass --ack-mainnet {ACK_TOKEN}",
            file=sys.stderr,
        )
        return 2

    if not args.skip_build:
        build_solana_program(repo_root, repo_root / "program" / "Cargo.toml")
        build_solana_program(repo_root, repo_root / "verifier-program" / "Cargo.toml")

    program_so = (
        Path(args.program_so).expanduser().resolve()
        if args.program_so
        else (repo_root / "program" / "target" / "deploy" / "sccp_sol_program.so").resolve()
    )
    verifier_so = (
        Path(args.verifier_so).expanduser().resolve()
        if args.verifier_so
        else (repo_root / "verifier-program" / "target" / "deploy" / "sccp_sol_verifier_program.so").resolve()
    )

    if not program_so.exists():
        print(f"Program .so not found: {program_so}", file=sys.stderr)
        return 2
    if not verifier_so.exists():
        print(f"Verifier .so not found: {verifier_so}", file=sys.stderr)
        return 2

    has_solana = command_exists("solana")
    has_solana_keygen = command_exists("solana-keygen")

    if args.execute:
        if not has_solana or not has_solana_keygen:
            print(
                "Missing Solana CLI tools (`solana`, `solana-keygen`) required for --execute",
                file=sys.stderr,
            )
            return 2
        # Dependency checks up front for execute mode.
        run(["solana", "--version"], cwd=repo_root)
        run(["solana-keygen", "--version"], cwd=repo_root)

    payer_pubkey = None
    program_pubkey = None
    verifier_pubkey = None
    if has_solana_keygen:
        payer_pubkey = run_stdout(["solana-keygen", "pubkey", str(payer)], cwd=repo_root)
        program_pubkey = run_stdout(["solana-keygen", "pubkey", str(program_kp)], cwd=repo_root)
        verifier_pubkey = run_stdout(["solana-keygen", "pubkey", str(verifier_kp)], cwd=repo_root)

    governor_keypair = None
    if args.governor_keypair:
        governor_keypair = Path(args.governor_keypair).expanduser().resolve()
        if not governor_keypair.exists():
            print(f"governor-keypair not found: {governor_keypair}", file=sys.stderr)
            return 2

    governor_pubkey = optional_arg(args, "governor_pubkey")
    governor_keypair_pubkey = None
    if governor_keypair and has_solana_keygen:
        governor_keypair_pubkey = keypair_pubkey(governor_keypair, repo_root)
        if governor_pubkey and governor_pubkey != governor_keypair_pubkey:
            print(
                f"Refusing deploy: governor pubkey {governor_pubkey} != governor keypair pubkey {governor_keypair_pubkey}",
                file=sys.stderr,
            )
            return 2
        if not governor_pubkey:
            governor_pubkey = governor_keypair_pubkey

    validator_bootstrap_fields = [
        ("latestBeefyBlock", args.latest_beefy_block),
        ("currentValidatorSetId", args.current_validator_set_id),
        ("currentValidatorSetLen", args.current_validator_set_len),
        ("currentValidatorSetRoot", optional_arg(args, "current_validator_set_root")),
        ("nextValidatorSetId", args.next_validator_set_id),
        ("nextValidatorSetLen", args.next_validator_set_len),
        ("nextValidatorSetRoot", optional_arg(args, "next_validator_set_root")),
    ]
    validator_bootstrap_present = [value is not None for _, value in validator_bootstrap_fields]
    validator_bootstrap_missing = [name for (name, value) in validator_bootstrap_fields if value is None]
    validator_bootstrap_all_present = all(validator_bootstrap_present)
    validator_bootstrap_any_present = any(validator_bootstrap_present)
    validator_bootstrap_partial = validator_bootstrap_any_present and not validator_bootstrap_all_present

    bootstrap_tool_manifest = (repo_root / "tools" / "solana-live-burn-flow" / "Cargo.toml").resolve()
    bootstrap_cmd = None
    if governor_pubkey and program_pubkey and verifier_pubkey:
        bootstrap_cmd = [
            "cargo",
            "run",
            "--manifest-path",
            str(bootstrap_tool_manifest),
            "--",
            "bootstrap",
            "--rpc-url",
            rpc_url,
            "--payer-keypair",
            str(payer),
            "--governor-pubkey",
            governor_pubkey,
            "--router-program-id",
            program_pubkey,
            "--verifier-program-id",
            verifier_pubkey,
        ]
        if governor_keypair:
            bootstrap_cmd += ["--governor-keypair", str(governor_keypair)]
        if args.latest_beefy_block is not None:
            bootstrap_cmd += ["--latest-beefy-block", str(args.latest_beefy_block)]
        if args.current_validator_set_id is not None:
            bootstrap_cmd += ["--current-validator-set-id", str(args.current_validator_set_id)]
        if args.current_validator_set_len is not None:
            bootstrap_cmd += ["--current-validator-set-len", str(args.current_validator_set_len)]
        current_validator_set_root = optional_arg(args, "current_validator_set_root")
        if current_validator_set_root is not None:
            bootstrap_cmd += ["--current-validator-set-root-hex", current_validator_set_root]
        if args.next_validator_set_id is not None:
            bootstrap_cmd += ["--next-validator-set-id", str(args.next_validator_set_id)]
        if args.next_validator_set_len is not None:
            bootstrap_cmd += ["--next-validator-set-len", str(args.next_validator_set_len)]
        next_validator_set_root = optional_arg(args, "next_validator_set_root")
        if next_validator_set_root is not None:
            bootstrap_cmd += ["--next-validator-set-root-hex", next_validator_set_root]

    state_path = Path(args.state_file).expanduser().resolve() if args.state_file else default_state_path(repo_root, payer_pubkey, payer)
    ensure_state_policy(execute=args.execute, resume=args.resume, state_file=state_path)

    actual_genesis_hash = None
    if args.execute:
        actual_genesis_hash = run_stdout(["solana", "genesis-hash", "--url", rpc_url], cwd=repo_root)
        if actual_genesis_hash != MAINNET_GENESIS_HASH:
            print(
                f"Refusing deploy: genesis hash {actual_genesis_hash} != expected mainnet {MAINNET_GENESIS_HASH}",
                file=sys.stderr,
            )
            return 2

    output: dict[str, object] = {
        "chain": "solana",
        "rpcHost": sanitize_rpc_host(rpc_url),
        "mode": "execute" if args.execute else "dry-run",
        "mainnetGenesisHash": MAINNET_GENESIS_HASH,
        "observedGenesisHash": actual_genesis_hash,
        "stateFile": str(state_path),
        "payer": {"keypair": str(payer), "pubkey": payer_pubkey},
        "program": {
            "keypair": str(program_kp),
            "pubkey": program_pubkey,
            "so": str(program_so),
        },
        "verifier": {
            "keypair": str(verifier_kp),
            "pubkey": verifier_pubkey,
            "so": str(verifier_so),
        },
        "bootstrap": {
            "toolManifest": str(bootstrap_tool_manifest),
            "governorPubkey": governor_pubkey,
            "governorKeypair": str(governor_keypair) if governor_keypair else None,
            "governorKeypairPubkey": governor_keypair_pubkey,
            "validatorBootstrapInputsComplete": validator_bootstrap_all_present,
            "validatorBootstrapInputsPartial": validator_bootstrap_partial,
            "missingValidatorBootstrapFields": validator_bootstrap_missing if validator_bootstrap_partial or not validator_bootstrap_all_present else [],
            "commandPreview": redact_command(bootstrap_cmd) if bootstrap_cmd else None,
        },
        "timestamp": datetime.now(timezone.utc).isoformat(),
    }

    default_out_path = (repo_root / "deployments" / f"mainnet-solana-{datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')}.json").resolve()
    out_path = Path(args.out).expanduser().resolve() if args.out else default_out_path
    output["outPath"] = str(out_path)

    if args.execute:
        params_hash = hash_params(
            {
                "version": STATE_VERSION,
                "rpcHost": sanitize_rpc_host(rpc_url),
                "genesisHash": MAINNET_GENESIS_HASH,
                "payerPubkey": payer_pubkey,
                "programPubkey": program_pubkey,
                "verifierPubkey": verifier_pubkey,
                "programSoSha256": sha256_file(program_so),
                "verifierSoSha256": sha256_file(verifier_so),
            }
        )

        now_iso = lambda: datetime.now(timezone.utc).isoformat()
        if args.resume:
            state = read_json_file(state_path)
            if state.get("version") != STATE_VERSION:
                print(f"Invalid state file version in {state_path}", file=sys.stderr)
                return 2
            if state.get("paramsHash") != params_hash:
                print(
                    f"State params hash mismatch for {state_path}. Refusing to resume with different inputs.",
                    file=sys.stderr,
                )
                return 2
        else:
            state = {
                "version": STATE_VERSION,
                "chain": "solana",
                "createdAt": now_iso(),
                "updatedAt": now_iso(),
                "paramsHash": params_hash,
                "steps": {},
            }
            atomic_write_json(state_path, state)

        def persist() -> None:
            state["updatedAt"] = now_iso()
            atomic_write_json(state_path, state)

        steps = state.setdefault("steps", {})

        if not steps.get("programDeployed", {}).get("done"):
            deploy_program_out = run_stdout(
                [
                    "solana",
                    "program",
                    "deploy",
                    "--url",
                    rpc_url,
                    "--keypair",
                    str(payer),
                    "--program-id",
                    str(program_kp),
                    str(program_so),
                ],
                cwd=repo_root,
            )
            steps["programDeployed"] = {
                "done": True,
                "at": now_iso(),
                "deployOutput": deploy_program_out,
            }
            persist()

        if not steps.get("verifierDeployed", {}).get("done"):
            deploy_verifier_out = run_stdout(
                [
                    "solana",
                    "program",
                    "deploy",
                    "--url",
                    rpc_url,
                    "--keypair",
                    str(payer),
                    "--program-id",
                    str(verifier_kp),
                    str(verifier_so),
                ],
                cwd=repo_root,
            )
            steps["verifierDeployed"] = {
                "done": True,
                "at": now_iso(),
                "deployOutput": deploy_verifier_out,
            }
            persist()

        bootstrap_pending: list[str] = []
        bootstrap_ready = False

        if steps.get("bootstrapCompleted", {}).get("done"):
            output["bootstrap"]["result"] = steps["bootstrapCompleted"]["bootstrapOutput"]
            bootstrap_ready = True
        else:
            if not governor_pubkey:
                bootstrap_pending.append(
                    "Provide --governor-pubkey so the router/verifier bootstrap target is explicit."
                )
            if governor_pubkey and governor_pubkey != payer_pubkey and not governor_keypair:
                bootstrap_pending.append(
                    "Provide --governor-keypair because the configured governor differs from --payer-keypair."
                )
            if validator_bootstrap_partial:
                bootstrap_pending.append(
                    "Provide the full verifier bootstrap input set: --latest-beefy-block, current validator set id/len/root, and next validator set id/len/root."
                )
            elif not validator_bootstrap_all_present:
                bootstrap_pending.append(
                    "Provide verifier bootstrap inputs sourced from SORA chain state so the governor can initialize the verifier light client."
                )
            if governor_pubkey and governor_pubkey == payer_pubkey and not governor_keypair:
                output["bootstrap"]["governorSigner"] = payer_pubkey
            elif governor_keypair_pubkey:
                output["bootstrap"]["governorSigner"] = governor_keypair_pubkey

            if not bootstrap_pending and bootstrap_cmd is not None:
                if not command_exists("cargo"):
                    print("Missing `cargo`, required to run the Solana bootstrap helper", file=sys.stderr)
                    return 2
                bootstrap_raw = run_stdout(bootstrap_cmd, cwd=repo_root)
                try:
                    bootstrap_output = json.loads(bootstrap_raw)
                except json.JSONDecodeError as err:
                    print(f"failed to decode bootstrap helper output as JSON: {err}", file=sys.stderr)
                    return 2
                steps["bootstrapCompleted"] = {
                    "done": True,
                    "at": now_iso(),
                    "bootstrapOutput": bootstrap_output,
                }
                persist()
                output["bootstrap"]["result"] = bootstrap_output
                bootstrap_ready = True

        state["completed"] = bootstrap_ready
        if bootstrap_ready:
            state["completedAt"] = now_iso()
        else:
            state.pop("completedAt", None)
        persist()

        output["paramsHash"] = params_hash
        output["resumed"] = args.resume
        output["program"]["deployOutput"] = steps["programDeployed"]["deployOutput"]
        output["verifier"]["deployOutput"] = steps["verifierDeployed"]["deployOutput"]
        if bootstrap_pending:
            output["pendingActions"] = bootstrap_pending
    else:
        output["note"] = (
            "No transactions sent. Re-run with --execute --ack-mainnet I_UNDERSTAND_MAINNET_DEPLOY. "
            "Deployment is only complete after the governor bootstrap flow finishes."
        )
        if not has_solana_keygen:
            output["pubkeyNote"] = "Install `solana-keygen` to auto-populate payer/program pubkeys in dry-run output."
        if has_solana:
            try:
                observed = run_stdout(["solana", "genesis-hash", "--url", rpc_url], cwd=repo_root)
                output["observedGenesisHash"] = observed
                output["genesisHashMatchesMainnet"] = observed == MAINNET_GENESIS_HASH
            except subprocess.CalledProcessError:
                output["genesisHashMatchesMainnet"] = None
        if not governor_pubkey:
            output["pendingActions"] = [
                "Provide --governor-pubkey and the SORA-derived verifier bootstrap inputs so the script can build a complete governor bootstrap command."
            ]
        elif governor_pubkey != payer_pubkey and not governor_keypair:
            output["pendingActions"] = [
                "Provide --governor-keypair because the configured governor differs from --payer-keypair."
            ]
        elif not validator_bootstrap_all_present:
            output["pendingActions"] = [
                "Provide verifier bootstrap inputs sourced from SORA chain state to complete the governor bootstrap flow."
            ]

    if args.execute or args.out:
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps(output, indent=2))

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
